#include "OutputPin.h"
#include "Filter.h"

#include <new>
#include <cstring>

namespace sc {

namespace {

// Renamed to avoid clashing with IPin::EnumMediaTypes() method name.
class MediaTypeEnumerator final : public IEnumMediaTypes
{
public:
    MediaTypeEnumerator(std::vector<AdvertisedFormat> formats)
        : m_formats(std::move(formats)) {}

    STDMETHODIMP QueryInterface(REFIID riid, void** out) override
    {
        if (!out) return E_POINTER;
        if (riid == IID_IUnknown || riid == IID_IEnumMediaTypes)
        { *out = static_cast<IEnumMediaTypes*>(this); AddRef(); return S_OK; }
        *out = nullptr; return E_NOINTERFACE;
    }
    STDMETHODIMP_(ULONG) AddRef() override { return m_ref.fetch_add(1) + 1; }
    STDMETHODIMP_(ULONG) Release() override
    {
        ULONG r = m_ref.fetch_sub(1) - 1;
        if (r == 0) delete this;
        return r;
    }
    STDMETHODIMP Next(ULONG cTypes, AM_MEDIA_TYPE** out, ULONG* pcFetched) override
    {
        if (!out) return E_POINTER;
        ULONG fetched = 0;
        for (ULONG i = 0; i < cTypes && m_cursor < m_formats.size(); ++i)
        {
            auto* mt = static_cast<AM_MEDIA_TYPE*>(CoTaskMemAlloc(sizeof(AM_MEDIA_TYPE)));
            if (!mt) return E_OUTOFMEMORY;
            if (FAILED(buildAmMediaType(m_formats[m_cursor++], *mt))) {
                CoTaskMemFree(mt);
                continue;
            }
            out[fetched++] = mt;
        }
        if (pcFetched) *pcFetched = fetched;
        return fetched == cTypes ? S_OK : S_FALSE;
    }
    STDMETHODIMP Skip(ULONG c) override
    {
        m_cursor = (m_cursor + c <= m_formats.size()) ? (m_cursor + c) : m_formats.size();
        return S_OK;
    }
    STDMETHODIMP Reset() override { m_cursor = 0; return S_OK; }
    STDMETHODIMP Clone(IEnumMediaTypes** out) override
    {
        if (!out) return E_POINTER;
        auto* c = new (std::nothrow) MediaTypeEnumerator(m_formats);
        if (!c) return E_OUTOFMEMORY;
        c->m_cursor = m_cursor;
        c->AddRef();
        *out = c;
        return S_OK;
    }
private:
    std::vector<AdvertisedFormat> m_formats;
    size_t m_cursor = 0;
    std::atomic<ULONG> m_ref{0};
};

void clearMediaType(AM_MEDIA_TYPE& mt)
{
    if (mt.pbFormat) { CoTaskMemFree(mt.pbFormat); mt.pbFormat = nullptr; }
    if (mt.pUnk)     { mt.pUnk->Release(); mt.pUnk = nullptr; }
    mt.cbFormat = 0;
}

} // namespace

OutputPin::OutputPin(VirtualCameraFilter* parent) : m_parent(parent)
{
    m_formats = defaultAdvertisedFormats();
}

OutputPin::~OutputPin()
{
    clearMediaType(m_currentType);
    if (m_connectedTo) m_connectedTo->Release();
}

STDMETHODIMP OutputPin::QueryInterface(REFIID riid, void** out)
{
    if (!out) return E_POINTER;
    if (riid == IID_IUnknown || riid == IID_IPin)
        { *out = static_cast<IPin*>(this); AddRef(); return S_OK; }
    if (riid == IID_IAMStreamConfig)
        { *out = static_cast<IAMStreamConfig*>(this); AddRef(); return S_OK; }
    if (riid == IID_IKsPropertySet)
        { *out = static_cast<IKsPropertySet*>(this); AddRef(); return S_OK; }
    if (riid == IID_IAMPushSource || riid == IID_IAMLatency)
        { *out = static_cast<IAMPushSource*>(this); AddRef(); return S_OK; }
    *out = nullptr;
    return E_NOINTERFACE;
}
STDMETHODIMP_(ULONG) OutputPin::AddRef()  { return m_ref.fetch_add(1) + 1; }
STDMETHODIMP_(ULONG) OutputPin::Release()
{
    ULONG r = m_ref.fetch_sub(1) - 1;
    if (r == 0) delete this;
    return r;
}

// --- IPin -----------------------------------------------------------------

STDMETHODIMP OutputPin::Connect(IPin*, const AM_MEDIA_TYPE*) { return E_NOTIMPL; }
STDMETHODIMP OutputPin::ReceiveConnection(IPin*, const AM_MEDIA_TYPE*) { return E_NOTIMPL; }
STDMETHODIMP OutputPin::Disconnect()
{
    if (m_connectedTo) { m_connectedTo->Release(); m_connectedTo = nullptr; }
    return S_OK;
}
STDMETHODIMP OutputPin::ConnectedTo(IPin** pin)
{
    if (!pin) return E_POINTER;
    *pin = m_connectedTo;
    if (*pin) (*pin)->AddRef();
    return *pin ? S_OK : VFW_E_NOT_CONNECTED;
}
STDMETHODIMP OutputPin::ConnectionMediaType(AM_MEDIA_TYPE*) { return VFW_E_NOT_CONNECTED; }

STDMETHODIMP OutputPin::QueryPinInfo(PIN_INFO* info)
{
    if (!info) return E_POINTER;
    wcscpy_s(info->achName, L"Output");
    info->dir = PINDIR_OUTPUT;
    info->pFilter = m_parent;
    if (info->pFilter) info->pFilter->AddRef();
    return S_OK;
}
STDMETHODIMP OutputPin::QueryDirection(PIN_DIRECTION* dir)
{
    if (!dir) return E_POINTER;
    *dir = PINDIR_OUTPUT;
    return S_OK;
}
STDMETHODIMP OutputPin::QueryId(LPWSTR* id)
{
    if (!id) return E_POINTER;
    static const wchar_t kId[] = L"Output";
    size_t bytes = sizeof(kId);
    *id = static_cast<LPWSTR>(CoTaskMemAlloc(bytes));
    if (!*id) return E_OUTOFMEMORY;
    memcpy(*id, kId, bytes);
    return S_OK;
}
STDMETHODIMP OutputPin::QueryAccept(const AM_MEDIA_TYPE* mt)
{
    if (!mt) return E_POINTER;
    if (mt->majortype != MEDIATYPE_Video) return S_FALSE;
    for (const auto& f : m_formats)
        if (f.subtype == mt->subtype) return S_OK;
    return S_FALSE;
}
STDMETHODIMP OutputPin::EnumMediaTypes(IEnumMediaTypes** out)
{
    if (!out) return E_POINTER;
    auto* e = new (std::nothrow) MediaTypeEnumerator(m_formats);
    if (!e) return E_OUTOFMEMORY;
    e->AddRef();
    *out = e;
    return S_OK;
}
STDMETHODIMP OutputPin::QueryInternalConnections(IPin**, ULONG*) { return E_NOTIMPL; }
STDMETHODIMP OutputPin::EndOfStream()  { return S_OK; }
STDMETHODIMP OutputPin::BeginFlush()   { return S_OK; }
STDMETHODIMP OutputPin::EndFlush()     { return S_OK; }
STDMETHODIMP OutputPin::NewSegment(REFERENCE_TIME, REFERENCE_TIME, double) { return S_OK; }

// --- IAMStreamConfig -------------------------------------------------------

STDMETHODIMP OutputPin::SetFormat(AM_MEDIA_TYPE* mt)
{
    if (!mt) return E_POINTER;
    clearMediaType(m_currentType);
    m_currentType = *mt;
    if (mt->pbFormat && mt->cbFormat)
    {
        m_currentType.pbFormat = static_cast<BYTE*>(CoTaskMemAlloc(mt->cbFormat));
        if (!m_currentType.pbFormat) return E_OUTOFMEMORY;
        memcpy(m_currentType.pbFormat, mt->pbFormat, mt->cbFormat);
    }
    return S_OK;
}
STDMETHODIMP OutputPin::GetFormat(AM_MEDIA_TYPE** mt)
{
    if (!mt) return E_POINTER;
    *mt = static_cast<AM_MEDIA_TYPE*>(CoTaskMemAlloc(sizeof(AM_MEDIA_TYPE)));
    if (!*mt) return E_OUTOFMEMORY;
    **mt = m_currentType;
    if (m_currentType.pbFormat && m_currentType.cbFormat)
    {
        (*mt)->pbFormat = static_cast<BYTE*>(CoTaskMemAlloc(m_currentType.cbFormat));
        if (!(*mt)->pbFormat) { CoTaskMemFree(*mt); *mt = nullptr; return E_OUTOFMEMORY; }
        memcpy((*mt)->pbFormat, m_currentType.pbFormat, m_currentType.cbFormat);
    }
    return S_OK;
}
STDMETHODIMP OutputPin::GetNumberOfCapabilities(int* count, int* size)
{
    if (!count || !size) return E_POINTER;
    *count = static_cast<int>(m_formats.size());
    *size  = sizeof(VIDEO_STREAM_CONFIG_CAPS);
    return S_OK;
}
STDMETHODIMP OutputPin::GetStreamCaps(int index, AM_MEDIA_TYPE** mt, BYTE* scc)
{
    if (!mt || !scc) return E_POINTER;
    if (index < 0 || static_cast<size_t>(index) >= m_formats.size())
        return S_FALSE;
    *mt = static_cast<AM_MEDIA_TYPE*>(CoTaskMemAlloc(sizeof(AM_MEDIA_TYPE)));
    if (!*mt) return E_OUTOFMEMORY;
    if (FAILED(buildAmMediaType(m_formats[index], **mt))) {
        CoTaskMemFree(*mt); *mt = nullptr;
        return E_FAIL;
    }
    auto* caps = reinterpret_cast<VIDEO_STREAM_CONFIG_CAPS*>(scc);
    ZeroMemory(caps, sizeof(*caps));
    caps->guid              = FORMAT_VideoInfo;
    caps->VideoStandard     = AnalogVideo_None;
    caps->InputSize.cx      = static_cast<LONG>(m_formats[index].width);
    caps->InputSize.cy      = static_cast<LONG>(m_formats[index].height);
    caps->MinCroppingSize   = caps->InputSize;
    caps->MaxCroppingSize   = caps->InputSize;
    caps->CropGranularityX  = 1;
    caps->CropGranularityY  = 1;
    caps->MinOutputSize     = caps->InputSize;
    caps->MaxOutputSize     = caps->InputSize;
    caps->OutputGranularityX = 1;
    caps->OutputGranularityY = 1;
    caps->MinFrameInterval  = (10000000LL * m_formats[index].frame_rate_den) / m_formats[index].frame_rate_num;
    caps->MaxFrameInterval  = caps->MinFrameInterval;
    caps->MinBitsPerSecond  = caps->MaxBitsPerSecond =
        m_formats[index].width * m_formats[index].height * m_formats[index].bits_per_pixel
        * m_formats[index].frame_rate_num / m_formats[index].frame_rate_den;
    return S_OK;
}

// --- IKsPropertySet --------------------------------------------------------

STDMETHODIMP OutputPin::Set(REFGUID, DWORD, LPVOID, DWORD, LPVOID, DWORD) { return E_NOTIMPL; }
STDMETHODIMP OutputPin::Get(REFGUID guidPropSet, DWORD dwPropID,
                            LPVOID, DWORD,
                            LPVOID pPropData, DWORD cbPropData,
                            DWORD* pcbReturned)
{
    if (guidPropSet != AMPROPSETID_Pin) return E_PROP_SET_UNSUPPORTED;
    if (dwPropID != AMPROPERTY_PIN_CATEGORY) return E_PROP_ID_UNSUPPORTED;
    if (pcbReturned) *pcbReturned = sizeof(GUID);
    if (!pPropData) return S_OK;
    if (cbPropData < sizeof(GUID)) return E_UNEXPECTED;
    *static_cast<GUID*>(pPropData) = PIN_CATEGORY_CAPTURE;
    return S_OK;
}
STDMETHODIMP OutputPin::QuerySupported(REFGUID guidPropSet, DWORD dwPropID, DWORD* pTypeSupport)
{
    if (guidPropSet != AMPROPSETID_Pin) return E_PROP_SET_UNSUPPORTED;
    if (dwPropID != AMPROPERTY_PIN_CATEGORY) return E_PROP_ID_UNSUPPORTED;
    if (pTypeSupport) *pTypeSupport = KSPROPERTY_SUPPORT_GET;
    return S_OK;
}

// --- IAMPushSource / IAMLatency -------------------------------------------

STDMETHODIMP OutputPin::GetPushSourceFlags(ULONG* flags)
{
    if (!flags) return E_POINTER;
    *flags = AM_PUSHSOURCECAPS_INTERNAL_RM;
    return S_OK;
}
STDMETHODIMP OutputPin::SetPushSourceFlags(ULONG)             { return S_OK; }
STDMETHODIMP OutputPin::SetStreamOffset(REFERENCE_TIME)        { return S_OK; }
STDMETHODIMP OutputPin::GetStreamOffset(REFERENCE_TIME* o)
{ if (!o) return E_POINTER; *o = 0; return S_OK; }
STDMETHODIMP OutputPin::GetMaxStreamOffset(REFERENCE_TIME* o)
{ if (!o) return E_POINTER; *o = 0; return S_OK; }
STDMETHODIMP OutputPin::SetMaxStreamOffset(REFERENCE_TIME)     { return S_OK; }
STDMETHODIMP OutputPin::GetLatency(REFERENCE_TIME* latency)
{ if (!latency) return E_POINTER; *latency = 333333; return S_OK; } // ~33 ms ≈ 1 frame @ 30fps

} // namespace sc
