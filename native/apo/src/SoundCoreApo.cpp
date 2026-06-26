#include "SoundCoreApo.h"
#include "ChainConfig.h"
#include "soundcore_vst_host.h"

#include <propkey.h>
#include <mmreg.h>
#include <mmsystem.h>
#include <ks.h>
#include <ksmedia.h>
#include <new>
#include <cstring>

namespace {
constexpr wchar_t kCopyright[]   = L"Copyright SoundCore.";
constexpr wchar_t kFriendlyName[] = L"SoundCore Audio Processing Object";
} // namespace

CSoundCoreApo::CSoundCoreApo() = default;

CSoundCoreApo::~CSoundCoreApo()
{
    destroyChain();
}

// ---- IUnknown ---------------------------------------------------------------

STDMETHODIMP CSoundCoreApo::QueryInterface(REFIID riid, void** out)
{
    if (!out) return E_POINTER;
    if (riid == IID_IUnknown || riid == __uuidof(IAudioProcessingObject))
        { *out = static_cast<IAudioProcessingObject*>(this); AddRef(); return S_OK; }
    if (riid == __uuidof(IAudioProcessingObjectConfiguration))
        { *out = static_cast<IAudioProcessingObjectConfiguration*>(this); AddRef(); return S_OK; }
    if (riid == __uuidof(IAudioProcessingObjectRT))
        { *out = static_cast<IAudioProcessingObjectRT*>(this); AddRef(); return S_OK; }
    *out = nullptr;
    return E_NOINTERFACE;
}
STDMETHODIMP_(ULONG) CSoundCoreApo::AddRef() { return m_ref.fetch_add(1) + 1; }
STDMETHODIMP_(ULONG) CSoundCoreApo::Release()
{
    ULONG r = m_ref.fetch_sub(1) - 1;
    if (r == 0) delete this;
    return r;
}

// ---- IAudioProcessingObject -------------------------------------------------

STDMETHODIMP CSoundCoreApo::Reset() { return S_OK; }

STDMETHODIMP CSoundCoreApo::GetLatency(HNSTIME* latency)
{
    if (!latency) return E_POINTER;
    *latency = 0;
    return S_OK;
}

STDMETHODIMP CSoundCoreApo::GetRegistrationProperties(APO_REG_PROPERTIES** props)
{
    if (!props) return E_POINTER;
    *props = nullptr;
    auto* p = static_cast<APO_REG_PROPERTIES*>(CoTaskMemAlloc(sizeof(APO_REG_PROPERTIES)));
    if (!p) return E_OUTOFMEMORY;
    std::memset(p, 0, sizeof(*p));
    p->clsid = CLSID_SoundCoreApo;
    p->Flags = static_cast<APO_FLAG>(APO_FLAG_DEFAULT | APO_FLAG_INPLACE);
    p->u32MinInputConnections        = 1;
    p->u32MaxInputConnections        = 1;
    p->u32MinOutputConnections       = 1;
    p->u32MaxOutputConnections       = 1;
    p->u32MaxInstances               = 0;
    p->u32NumAPOInterfaces           = 1;
    p->iidAPOInterfaceList[0]        = __uuidof(IAudioProcessingObjectRT);
    wcsncpy_s(p->szFriendlyName, kFriendlyName, _TRUNCATE);
    wcsncpy_s(p->szCopyrightInfo, kCopyright, _TRUNCATE);
    *props = p;
    return S_OK;
}

STDMETHODIMP CSoundCoreApo::Initialize(UINT32, BYTE*) { return S_OK; }

STDMETHODIMP CSoundCoreApo::IsInputFormatSupported(
    IAudioMediaType* /*outputFormat*/,
    IAudioMediaType* requestedInputFormat,
    IAudioMediaType** supportedInputFormat)
{
    if (!supportedInputFormat || !requestedInputFormat) return E_POINTER;
    *supportedInputFormat = nullptr;

    UNCOMPRESSEDAUDIOFORMAT fmt = {};
    HRESULT hr = requestedInputFormat->GetUncompressedAudioFormat(&fmt);
    if (FAILED(hr)) return hr;
    if (fmt.guidFormatType != KSDATAFORMAT_SUBTYPE_IEEE_FLOAT)
        return APOERR_FORMAT_NOT_SUPPORTED;
    if (fmt.dwBytesPerSampleContainer != sizeof(float))
        return APOERR_FORMAT_NOT_SUPPORTED;
    if (fmt.dwSamplesPerFrame == 0 || fmt.dwSamplesPerFrame > 8)
        return APOERR_FORMAT_NOT_SUPPORTED;

    requestedInputFormat->AddRef();
    *supportedInputFormat = requestedInputFormat;
    return S_OK;
}

STDMETHODIMP CSoundCoreApo::IsOutputFormatSupported(
    IAudioMediaType* /*inputFormat*/,
    IAudioMediaType* requestedOutputFormat,
    IAudioMediaType** supportedOutputFormat)
{
    if (!supportedOutputFormat || !requestedOutputFormat) return E_POINTER;
    requestedOutputFormat->AddRef();
    *supportedOutputFormat = requestedOutputFormat;
    return S_OK;
}

STDMETHODIMP CSoundCoreApo::GetInputChannelCount(UINT32* channelCount)
{
    if (!channelCount) return E_POINTER;
    *channelCount = m_channels ? m_channels : 2;
    return S_OK;
}

// ---- IAudioProcessingObjectConfiguration ------------------------------------

STDMETHODIMP CSoundCoreApo::LockForProcess(
    UINT32 numInputConnections,
    APO_CONNECTION_DESCRIPTOR** inputConnections,
    UINT32 numOutputConnections,
    APO_CONNECTION_DESCRIPTOR** outputConnections)
{
    if (numInputConnections == 0 || numOutputConnections == 0) return E_INVALIDARG;
    if (!inputConnections || !outputConnections) return E_POINTER;

    UNCOMPRESSEDAUDIOFORMAT fmt = {};
    HRESULT hr = inputConnections[0]->pFormat->GetUncompressedAudioFormat(&fmt);
    if (FAILED(hr)) return hr;

    m_channels      = fmt.dwSamplesPerFrame;
    m_maxFrameCount = inputConnections[0]->u32MaxFrameCount;
    m_sampleRate    = fmt.fFramesPerSecond;

    destroyChain();
    buildChainFromConfig();
    m_locked = true;
    return S_OK;
}

STDMETHODIMP CSoundCoreApo::UnlockForProcess()
{
    destroyChain();
    m_locked = false;
    return S_OK;
}

void CSoundCoreApo::buildChainFromConfig()
{
    auto cfg = sc::load_chain_config();
    if (!cfg.enabled || cfg.plugins.empty()) return;

    void* chain = nullptr;
    if (sc_vst_host_chain_new(m_sampleRate, m_maxFrameCount, m_channels, &chain) != 0)
        return;
    if (!chain) return;

    for (const auto& p : cfg.plugins)
    {
        sc_node_id node_id;
        if (sc_vst_host_chain_append(chain, p.uid.c_str(), p.path.c_str(), node_id) != 0)
        {
            // best-effort: skip plugins that fail to load
            continue;
        }
    }
    m_chain = chain;
}

void CSoundCoreApo::destroyChain()
{
    if (m_chain)
    {
        sc_vst_host_chain_free(m_chain);
        m_chain = nullptr;
    }
}

// ---- IAudioProcessingObjectRT -----------------------------------------------

STDMETHODIMP_(void) CSoundCoreApo::APOProcess(
    UINT32 numInputConnections,
    APO_CONNECTION_PROPERTY** inputConnections,
    UINT32 numOutputConnections,
    APO_CONNECTION_PROPERTY** outputConnections)
{
    if (numInputConnections == 0 || numOutputConnections == 0) return;

    APO_CONNECTION_PROPERTY* in  = inputConnections[0];
    APO_CONNECTION_PROPERTY* out = outputConnections[0];

    switch (in->u32BufferFlags)
    {
        case BUFFER_VALID:
        {
            float* inBuf  = reinterpret_cast<float*>(in->pBuffer);
            float* outBuf = reinterpret_cast<float*>(out->pBuffer);
            if (inBuf != outBuf)
                std::memcpy(outBuf, inBuf,
                            sizeof(float) * m_channels * in->u32ValidFrameCount);
            if (m_chain)
                sc_vst_host_chain_process(m_chain, outBuf, m_channels, in->u32ValidFrameCount);
            out->u32ValidFrameCount = in->u32ValidFrameCount;
            out->u32BufferFlags = BUFFER_VALID;
            break;
        }
        case BUFFER_SILENT:
        {
            float* outBuf = reinterpret_cast<float*>(out->pBuffer);
            std::memset(outBuf, 0, sizeof(float) * m_channels * in->u32ValidFrameCount);
            out->u32ValidFrameCount = in->u32ValidFrameCount;
            out->u32BufferFlags = BUFFER_SILENT;
            break;
        }
        default:
            out->u32ValidFrameCount = 0;
            out->u32BufferFlags = BUFFER_INVALID;
            break;
    }
}

STDMETHODIMP_(UINT32) CSoundCoreApo::CalcInputFrames(UINT32 n) { return n; }
STDMETHODIMP_(UINT32) CSoundCoreApo::CalcOutputFrames(UINT32 n) { return n; }
