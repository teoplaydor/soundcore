#include "soundcore_vst_host.h"
#include "HostInternals.h"

#include <juce_audio_processors/juce_audio_processors.h>
#include <juce_audio_basics/juce_audio_basics.h>

#include <algorithm>
#include <cstring>
#include <random>

namespace sc {

namespace {
std::array<uint8_t, 16> randomNodeId()
{
    std::array<uint8_t, 16> id{};
    std::random_device rd;
    for (auto& b : id)
        b = static_cast<uint8_t>(rd() & 0xFF);
    // Mark as our v1 namespace UUID.
    id[6] = (id[6] & 0x0F) | 0x40; // RFC4122 version 4
    id[8] = (id[8] & 0x3F) | 0x80; // variant
    return id;
}
} // namespace

Chain::Chain(double sampleRate, uint32_t maxBlockSamples, uint32_t channels)
    : m_sampleRate(sampleRate),
      m_maxBlockSamples(maxBlockSamples),
      m_channels(channels)
{
    m_scratch.setSize(static_cast<int>(channels), static_cast<int>(maxBlockSamples), false, true, false);
}

Chain::~Chain()
{
    std::lock_guard lock(m_mutex);
    for (auto& node : m_active)
        if (node && node->plugin) node->plugin->releaseResources();
}

bool Chain::append(const juce::PluginDescription& desc, std::array<uint8_t, 16>& outId)
{
    juce::String err;
    juce::AudioPluginFormatManager mgr;
    mgr.addFormat(new juce::VST3PluginFormat());

    auto instance = mgr.createPluginInstance(desc, m_sampleRate,
                                             static_cast<int>(m_maxBlockSamples), err);
    if (instance == nullptr)
    {
        setLastError("createPluginInstance failed: " + err);
        return false;
    }
    instance->setPlayConfigDetails(static_cast<int>(m_channels),
                                   static_cast<int>(m_channels),
                                   m_sampleRate,
                                   static_cast<int>(m_maxBlockSamples));
    instance->prepareToPlay(m_sampleRate, static_cast<int>(m_maxBlockSamples));

    auto node = std::make_unique<ChainNode>();
    node->id = randomNodeId();
    node->plugin = std::move(instance);
    outId = node->id;

    std::lock_guard lock(m_mutex);
    m_active.push_back(std::move(node));
    return true;
}

bool Chain::remove(const std::array<uint8_t, 16>& id)
{
    std::lock_guard lock(m_mutex);
    auto it = std::find_if(m_active.begin(), m_active.end(),
                           [&](const std::unique_ptr<ChainNode>& n){ return n && n->id == id; });
    if (it == m_active.end()) return false;
    if ((*it)->plugin) (*it)->plugin->releaseResources();
    m_active.erase(it);
    return true;
}

int Chain::process(float* interleaved, uint32_t channels, uint32_t frames)
{
    if (frames == 0) return 0;                      // nothing to do, not an error
    if (channels != m_channels) return -2;          // caller's channel layout mismatch
    if (frames > m_maxBlockSamples) return -3;      // exceeds prepared block size

    // De-interleave into the JUCE scratch buffer.
    for (uint32_t ch = 0; ch < channels; ++ch)
    {
        float* dst = m_scratch.getWritePointer(static_cast<int>(ch));
        const float* src = interleaved + ch;
        for (uint32_t i = 0; i < frames; ++i)
        {
            dst[i] = src[i * channels];
        }
    }

    // Process exactly `frames` samples, not the full scratch capacity: a
    // bare processBlock(m_scratch, ...) would run plugins over
    // m_maxBlockSamples every call, processing stale tail samples. This
    // view aliases the scratch channel pointers without allocating.
    juce::AudioBuffer<float> view(m_scratch.getArrayOfWritePointers(),
                                  static_cast<int>(channels),
                                  0,
                                  static_cast<int>(frames));

    juce::MidiBuffer midi;
    {
        std::lock_guard lock(m_mutex);
        for (auto& node : m_active)
        {
            if (!node || !node->plugin) continue;
            node->plugin->processBlock(view, midi);
            midi.clear();
        }
    }

    // Re-interleave back into the caller's buffer.
    for (uint32_t ch = 0; ch < channels; ++ch)
    {
        const float* src = m_scratch.getReadPointer(static_cast<int>(ch));
        float* dst = interleaved + ch;
        for (uint32_t i = 0; i < frames; ++i)
            dst[i * channels] = src[i];
    }
    return 0;
}

bool Chain::setParameter(const std::array<uint8_t, 16>& id, uint32_t index, float value)
{
    std::lock_guard lock(m_mutex);
    for (auto& node : m_active)
    {
        if (!node || node->id != id || !node->plugin) continue;
        auto* params = node->plugin->getParameters().getRawDataPointer();
        const int n = node->plugin->getParameters().size();
        if (static_cast<int>(index) >= n) return false;
        if (auto* p = params[index]) {
            p->setValueNotifyingHost(juce::jlimit(0.0f, 1.0f, value));
            return true;
        }
        return false;
    }
    return false;
}

} // namespace sc

// =============================================================================
//  C ABI surface
// =============================================================================

extern "C" {

int32_t sc_vst_host_chain_new(
    double   sample_rate,
    uint32_t max_block_samples,
    uint32_t channels,
    void**   out_chain)
{
    if (!out_chain) return -1;
    *out_chain = nullptr;
    if (channels == 0 || channels > 16) return -2;
    if (max_block_samples == 0 || max_block_samples > 16384) return -3;
    *out_chain = new sc::Chain(sample_rate, max_block_samples, channels);
    return 0;
}

void sc_vst_host_chain_free(void* chain)
{
    delete static_cast<sc::Chain*>(chain);
}

int32_t sc_vst_host_chain_append(
    void*       chain,
    const char* plugin_uid,
    const char* plugin_path,
    sc_node_id  out_node_id)
{
    if (!chain || !plugin_uid) return -1;
    try
    {
    juce::AudioPluginFormatManager mgr;
    mgr.addFormat(new juce::VST3PluginFormat());

    juce::KnownPluginList scratch;
    juce::OwnedArray<juce::PluginDescription> descs;
    if (plugin_path && *plugin_path)
    {
        scratch.scanAndAddFile(juce::String::fromUTF8(plugin_path),
                               /*dontRescanIfAlreadyInList*/ false,
                               descs,
                               *mgr.getFormat(0));
    }

    const bool matchByPath = (plugin_uid[0] == '\0');
    const juce::String wantUid = juce::String::fromUTF8(plugin_uid);

    juce::PluginDescription chosen{};
    bool found = false;
    for (const auto& type : scratch.getTypes())
    {
        // Empty uid means "match by path": take the first description the
        // scanner found at that file (the scanner emits uids the caller may
        // not yet have written back into the chain config).
        if (matchByPath
            || type.createIdentifierString().equalsIgnoreCase(wantUid))
        {
            chosen = type;
            found = true;
            break;
        }
    }
    if (!found) {
        sc::setLastError(matchByPath ? "no plugin found at path"
                                     : "plugin UID not found at path");
        return -2;
    }

    std::array<uint8_t, 16> id{};
    if (!static_cast<sc::Chain*>(chain)->append(chosen, id))
        return -3;
    std::memcpy(out_node_id, id.data(), 16);
    return 0;
    }
    catch (const std::exception& e)
    {
        sc::setLastError(juce::String("sc_vst_host_chain_append: ") + e.what());
        return -100;
    }
    catch (...)
    {
        sc::setLastError("sc_vst_host_chain_append: unknown C++ exception");
        return -101;
    }
}

int32_t sc_vst_host_chain_remove(void* chain, const sc_node_id node_id)
{
    if (!chain || !node_id) return -1;
    std::array<uint8_t, 16> id{};
    std::memcpy(id.data(), node_id, 16);
    return static_cast<sc::Chain*>(chain)->remove(id) ? 0 : -2;
}

int32_t sc_vst_host_chain_process(
    void*    chain,
    float*   interleaved_in_out,
    uint32_t channels,
    uint32_t frames)
{
    if (!chain || !interleaved_in_out) return -1;
    try
    {
        return static_cast<sc::Chain*>(chain)->process(interleaved_in_out, channels, frames);
    }
    catch (const std::exception& e)
    {
        sc::setLastError(juce::String("sc_vst_host_chain_process: ") + e.what());
        return -100;
    }
    catch (...)
    {
        sc::setLastError("sc_vst_host_chain_process: unknown C++ exception");
        return -101;
    }
}

int32_t sc_vst_host_chain_set_parameter(
    void*            chain,
    const sc_node_id node_id,
    uint32_t         index,
    float            value)
{
    if (!chain || !node_id) return -1;
    std::array<uint8_t, 16> id{};
    std::memcpy(id.data(), node_id, 16);
    return static_cast<sc::Chain*>(chain)->setParameter(id, index, value) ? 0 : -2;
}

} // extern "C"
