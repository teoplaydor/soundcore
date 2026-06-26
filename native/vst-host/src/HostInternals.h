// Shared internals for soundcore_vst_host. Not exported.

#pragma once

#include <juce_audio_processors/juce_audio_processors.h>
#include <juce_audio_basics/juce_audio_basics.h>

#include <memory>
#include <mutex>
#include <vector>
#include <string>
#include <array>

namespace sc {

void setLastError(juce::String message);
const char* getLastError();

struct PluginIndex {
    juce::AudioPluginFormatManager formatManager;
    juce::KnownPluginList list;
    std::vector<juce::PluginDescription> descriptions;
    // Stable backing storage for the identifier strings handed out by
    // sc_vst_host_index_get. createIdentifierString() returns a temporary,
    // so we must keep an owning copy alive for the lifetime of the index.
    std::vector<std::string> identifierStrings;

    PluginIndex();
};

struct ChainNode {
    std::array<uint8_t, 16> id{};
    std::unique_ptr<juce::AudioPluginInstance> plugin;
};

class Chain {
public:
    Chain(double sampleRate, uint32_t maxBlockSamples, uint32_t channels);
    ~Chain();

    bool append(const juce::PluginDescription& desc, std::array<uint8_t, 16>& outId);
    bool remove(const std::array<uint8_t, 16>& id);
    // Returns 0 on success, negative on a parameter mismatch that caused
    // processing to be skipped (so the caller doesn't mistake a silent
    // no-op for clean audio): -2 channel mismatch, -3 frames too large.
    int process(float* interleaved, uint32_t channels, uint32_t frames);
    bool setParameter(const std::array<uint8_t, 16>& id, uint32_t index, float value);

private:
    double m_sampleRate;
    uint32_t m_maxBlockSamples;
    uint32_t m_channels;

    // The audio thread reads `m_active`; mutations go through `m_mutex`
    // and a copy-on-write swap.
    std::mutex m_mutex;
    std::vector<std::unique_ptr<ChainNode>> m_active;

    juce::AudioBuffer<float> m_scratch; // deinterleaved buffer reused per process()
};

} // namespace sc
