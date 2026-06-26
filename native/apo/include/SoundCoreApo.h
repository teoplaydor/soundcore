// SoundCoreApo: COM object loaded by audiodg.exe as an APO.
// Implements IAudioProcessingObject + Configuration + RT (no ATL).
// Runs the JUCE VST3 chain via the C ABI in soundcore_vst_host.h.

#pragma once

#include <windows.h>
#include <unknwn.h>
#include <objbase.h>
#include <atomic>

#include <audioenginebaseapo.h>
#include <audioengineextensionapo.h>

#include "soundcore_vst_host.h"

// {2E8C0F12-1B7B-49B0-9C84-3CD5A6B7B210}
DEFINE_GUID(CLSID_SoundCoreApo,
    0x2e8c0f12, 0x1b7b, 0x49b0, 0x9c, 0x84, 0x3c, 0xd5, 0xa6, 0xb7, 0xb2, 0x10);

class CSoundCoreApo final
    : public IAudioProcessingObject
    , public IAudioProcessingObjectConfiguration
    , public IAudioProcessingObjectRT
{
public:
    CSoundCoreApo();
    ~CSoundCoreApo();

    // IUnknown
    STDMETHODIMP QueryInterface(REFIID riid, void** out) override;
    STDMETHODIMP_(ULONG) AddRef() override;
    STDMETHODIMP_(ULONG) Release() override;

    // IAudioProcessingObject
    STDMETHODIMP Reset() override;
    STDMETHODIMP GetLatency(HNSTIME* latency) override;
    STDMETHODIMP GetRegistrationProperties(APO_REG_PROPERTIES** props) override;
    STDMETHODIMP Initialize(UINT32 cbDataSize, BYTE* pbyData) override;
    STDMETHODIMP IsInputFormatSupported(IAudioMediaType* outputFormat,
                                        IAudioMediaType* requestedInputFormat,
                                        IAudioMediaType** supportedInputFormat) override;
    STDMETHODIMP IsOutputFormatSupported(IAudioMediaType* inputFormat,
                                         IAudioMediaType* requestedOutputFormat,
                                         IAudioMediaType** supportedOutputFormat) override;
    STDMETHODIMP GetInputChannelCount(UINT32* channelCount) override;

    // IAudioProcessingObjectConfiguration
    STDMETHODIMP LockForProcess(UINT32 numInputConnections,
                                APO_CONNECTION_DESCRIPTOR** inputConnections,
                                UINT32 numOutputConnections,
                                APO_CONNECTION_DESCRIPTOR** outputConnections) override;
    STDMETHODIMP UnlockForProcess() override;

    // IAudioProcessingObjectRT
    STDMETHODIMP_(void) APOProcess(UINT32 numInputConnections,
                                   APO_CONNECTION_PROPERTY** inputConnections,
                                   UINT32 numOutputConnections,
                                   APO_CONNECTION_PROPERTY** outputConnections) override;
    STDMETHODIMP_(UINT32) CalcInputFrames(UINT32 outputFrameCount) override;
    STDMETHODIMP_(UINT32) CalcOutputFrames(UINT32 inputFrameCount) override;

private:
    void buildChainFromConfig();
    void destroyChain();

    std::atomic<ULONG> m_ref{0};
    bool m_locked = false;
    UINT32 m_channels = 0;
    UINT32 m_maxFrameCount = 0;
    double m_sampleRate = 0.0;

    // VST3 chain handle from soundcore_vst_host (`void*`). nullptr means
    // pass-through. Accessed only between LockForProcess and
    // UnlockForProcess so no atomics needed for the pointer itself.
    void* m_chain = nullptr;
};
