#include "Filter.h"
#include "Guids.h"

#include <strsafe.h>
#include <new>

namespace sc {

namespace {

// Renamed to avoid clashing with the IBaseFilter::EnumPins() method name.
class PinEnumerator final : public IEnumPins
{
public:
    PinEnumerator(IPin* pin) : m_pin(pin) { if (m_pin) m_pin->AddRef(); }
    ~PinEnumerator() { if (m_pin) m_pin->Release(); }

    STDMETHODIMP QueryInterface(REFIID riid, void** out) override
    {
        if (!out) return E_POINTER;
        if (riid == IID_IUnknown || riid == IID_IEnumPins) {
            *out = static_cast<IEnumPins*>(this); AddRef(); return S_OK;
        }
        *out = nullptr; return E_NOINTERFACE;
    }
    STDMETHODIMP_(ULONG) AddRef() override { return m_ref.fetch_add(1) + 1; }
    STDMETHODIMP_(ULONG) Release() override
    {
        ULONG r = m_ref.fetch_sub(1) - 1;
        if (r == 0) delete this;
        return r;
    }

    STDMETHODIMP Next(ULONG cPins, IPin** ppPins, ULONG* pcFetched) override
    {
        if (cPins == 0 || !ppPins) return E_POINTER;
        ULONG fetched = 0;
        if (!m_consumed && cPins >= 1)
        {
            ppPins[0] = m_pin;
            m_pin->AddRef();
            m_consumed = true;
            fetched = 1;
        }
        if (pcFetched) *pcFetched = fetched;
        return fetched == cPins ? S_OK : S_FALSE;
    }
    STDMETHODIMP Skip(ULONG cPins) override
    {
        if (cPins == 0) return S_OK;
        if (m_consumed) return S_FALSE;
        m_consumed = true;
        return cPins == 1 ? S_OK : S_FALSE;
    }
    STDMETHODIMP Reset() override { m_consumed = false; return S_OK; }
    STDMETHODIMP Clone(IEnumPins** out) override
    {
        if (!out) return E_POINTER;
        auto* clone = new (std::nothrow) PinEnumerator(m_pin);
        if (!clone) return E_OUTOFMEMORY;
        clone->m_consumed = m_consumed;
        clone->AddRef();
        *out = clone;
        return S_OK;
    }

private:
    IPin* m_pin;
    bool m_consumed = false;
    std::atomic<ULONG> m_ref{0};
};

} // namespace

VirtualCameraFilter::VirtualCameraFilter()
{
    m_outPin = new (std::nothrow) OutputPin(this);
    if (m_outPin) m_outPin->AddRef();
    m_filterName = L"SoundCore Virtual Camera";
    m_reader.connect(SC_CAMERA_SHARED_NAME);
}

VirtualCameraFilter::~VirtualCameraFilter()
{
    if (m_outPin) { m_outPin->Release(); m_outPin = nullptr; }
    if (m_clock)  { m_clock->Release();  m_clock = nullptr; }
}

STDMETHODIMP VirtualCameraFilter::QueryInterface(REFIID riid, void** out)
{
    if (!out) return E_POINTER;
    if (riid == IID_IUnknown || riid == IID_IBaseFilter ||
        riid == IID_IMediaFilter || riid == IID_IPersist)
    {
        *out = static_cast<IBaseFilter*>(this); AddRef(); return S_OK;
    }
    if (riid == IID_IAMFilterMiscFlags)
    {
        *out = static_cast<IAMFilterMiscFlags*>(this); AddRef(); return S_OK;
    }
    *out = nullptr;
    return E_NOINTERFACE;
}

STDMETHODIMP_(ULONG) VirtualCameraFilter::AddRef()  { return m_ref.fetch_add(1) + 1; }
STDMETHODIMP_(ULONG) VirtualCameraFilter::Release()
{
    ULONG r = m_ref.fetch_sub(1) - 1;
    if (r == 0) delete this;
    return r;
}

STDMETHODIMP VirtualCameraFilter::GetClassID(CLSID* clsid)
{
    if (!clsid) return E_POINTER;
    *clsid = CLSID_SoundCoreVirtualCamera;
    return S_OK;
}

STDMETHODIMP VirtualCameraFilter::Stop()   { m_state.store(State_Stopped); return S_OK; }
STDMETHODIMP VirtualCameraFilter::Pause()  { m_state.store(State_Paused);  return S_OK; }
STDMETHODIMP VirtualCameraFilter::Run(REFERENCE_TIME)
{
    m_state.store(State_Running);
    return S_OK;
}
STDMETHODIMP VirtualCameraFilter::GetState(DWORD, FILTER_STATE* state)
{
    if (!state) return E_POINTER;
    *state = m_state.load();
    return S_OK;
}
STDMETHODIMP VirtualCameraFilter::SetSyncSource(IReferenceClock* clock)
{
    if (m_clock) m_clock->Release();
    m_clock = clock;
    if (m_clock) m_clock->AddRef();
    return S_OK;
}
STDMETHODIMP VirtualCameraFilter::GetSyncSource(IReferenceClock** clock)
{
    if (!clock) return E_POINTER;
    *clock = m_clock;
    if (*clock) (*clock)->AddRef();
    return S_OK;
}

STDMETHODIMP VirtualCameraFilter::EnumPins(IEnumPins** out)
{
    if (!out) return E_POINTER;
    if (!m_outPin) { *out = nullptr; return E_FAIL; }
    auto* e = new (std::nothrow) PinEnumerator(m_outPin);
    if (!e) return E_OUTOFMEMORY;
    e->AddRef();
    *out = e;
    return S_OK;
}

STDMETHODIMP VirtualCameraFilter::FindPin(LPCWSTR id, IPin** out)
{
    if (!out || !id) return E_POINTER;
    if (wcscmp(id, L"Output") == 0 && m_outPin)
    {
        m_outPin->AddRef();
        *out = m_outPin;
        return S_OK;
    }
    *out = nullptr;
    return VFW_E_NOT_FOUND;
}

STDMETHODIMP VirtualCameraFilter::QueryFilterInfo(FILTER_INFO* info)
{
    if (!info) return E_POINTER;
    StringCchCopyW(info->achName, _countof(info->achName), m_filterName.c_str());
    info->pGraph = m_graph;
    if (info->pGraph) info->pGraph->AddRef();
    return S_OK;
}

STDMETHODIMP VirtualCameraFilter::JoinFilterGraph(IFilterGraph* graph, LPCWSTR name)
{
    m_graph = graph;
    if (name) m_filterName = name;
    return S_OK;
}

STDMETHODIMP VirtualCameraFilter::QueryVendorInfo(LPWSTR* info)
{
    if (!info) return E_POINTER;
    static const wchar_t kVendor[] = L"SoundCore";
    size_t bytes = sizeof(kVendor);
    *info = static_cast<LPWSTR>(CoTaskMemAlloc(bytes));
    if (!*info) return E_OUTOFMEMORY;
    memcpy(*info, kVendor, bytes);
    return S_OK;
}

STDMETHODIMP_(ULONG) VirtualCameraFilter::GetMiscFlags()
{
    return AM_FILTER_MISC_FLAGS_IS_SOURCE;
}

} // namespace sc
