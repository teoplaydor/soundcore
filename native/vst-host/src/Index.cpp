#include "soundcore_vst_host.h"
#include "HostInternals.h"

#include <juce_audio_processors/juce_audio_processors.h>

namespace sc {

PluginIndex::PluginIndex()
{
    formatManager.addFormat(new juce::VST3PluginFormat());
}

} // namespace sc

extern "C" {

int32_t sc_vst_host_scan(
    const char* const* search_paths,
    uint32_t           path_count,
    void**             out_index)
{
    if (!out_index) return -1;
    *out_index = nullptr;

    try
    {
    auto index = std::make_unique<sc::PluginIndex>();

    juce::FileSearchPath searchPath;
    if (search_paths != nullptr && path_count > 0)
    {
        for (uint32_t i = 0; i < path_count; ++i)
        {
            if (search_paths[i] == nullptr) continue;
            searchPath.add(juce::File(juce::String::fromUTF8(search_paths[i])));
        }
    }
    else
    {
        // Default Windows VST3 search paths. JUCE 8 doesn't expose
        // begin/end iterators on FileSearchPath, so index manually.
        juce::FileSearchPath defaults = juce::VST3PluginFormat().getDefaultLocationsToSearch();
        for (int i = 0; i < defaults.getNumPaths(); ++i)
            searchPath.add(defaults[i]);
    }

    juce::VST3PluginFormat format;
    juce::PluginDirectoryScanner scanner(
        index->list,
        format,
        searchPath,
        /*recursive*/ true,
        /*deadMansPedalFile*/ juce::File(),
        /*allowAsync*/ false);

    juce::String currentName;
    while (scanner.scanNextFile(true, currentName))
    {
        // pump
    }

    for (const auto& type : index->list.getTypes())
    {
        index->descriptions.push_back(type);
        index->identifierStrings.push_back(
            type.createIdentifierString().toStdString());
    }

    *out_index = index.release();
    return 0;
    }
    catch (const std::exception& e)
    {
        sc::setLastError(juce::String("sc_vst_host_scan: ") + e.what());
        return -100;
    }
    catch (...)
    {
        sc::setLastError("sc_vst_host_scan: unknown C++ exception");
        return -101;
    }
}

uint32_t sc_vst_host_index_size(const void* index)
{
    if (!index) return 0;
    const auto* p = static_cast<const sc::PluginIndex*>(index);
    return static_cast<uint32_t>(p->descriptions.size());
}

int32_t sc_vst_host_index_get(
    const void*              index,
    uint32_t                 slot,
    sc_vst_host_plugin_info* out)
{
    if (!index || !out) return -1;
    const auto* p = static_cast<const sc::PluginIndex*>(index);
    if (slot >= p->descriptions.size()) return -2;
    const auto& d = p->descriptions[slot];

    // We point to index-owned strings (stable for the index's lifetime).
    // `uid` comes from identifierStrings — created from a temporary, so it
    // must NOT be read straight off createIdentifierString().toRawUTF8().
    out->uid          = (slot < p->identifierStrings.size())
                            ? p->identifierStrings[slot].c_str()
                            : "";
    out->name         = d.name.toRawUTF8();
    out->vendor       = d.manufacturerName.toRawUTF8();
    out->category     = d.category.toRawUTF8();
    out->path         = d.fileOrIdentifier.toRawUTF8();
    out->num_inputs   = static_cast<uint32_t>(d.numInputChannels);
    out->num_outputs  = static_cast<uint32_t>(d.numOutputChannels);
    // PluginDescription doesn't carry an "has editor" bit at scan time;
    // we conservatively assume yes and the UI confirms by attempting a load.
    out->has_editor   = 1u;
    return 0;
}

void sc_vst_host_index_free(void* index)
{
    delete static_cast<sc::PluginIndex*>(index);
}

} // extern "C"
