// System DShow GUIDs come from strmiids.lib; our own CLSID is materialised
// in GuidsImpl.cpp. Do NOT define INITGUID in this TU.

#include <windows.h>
#include <unknwn.h>
#include <atomic>
#include <new>

#include "Filter.h"
#include "Guids.h"

extern "C" IMAGE_DOS_HEADER __ImageBase;

static std::atomic<LONG> g_dllRefCount{0};

void DllAddRef()  { g_dllRefCount.fetch_add(1, std::memory_order_relaxed); }
void DllRelease() { g_dllRefCount.fetch_sub(1, std::memory_order_relaxed); }

template <class T>
class CFactory final : public IClassFactory
{
public:
    STDMETHODIMP QueryInterface(REFIID riid, void** out) override
    {
        if (!out) return E_POINTER;
        if (riid == IID_IUnknown || riid == IID_IClassFactory)
            { *out = static_cast<IClassFactory*>(this); AddRef(); return S_OK; }
        *out = nullptr; return E_NOINTERFACE;
    }
    STDMETHODIMP_(ULONG) AddRef() override { return m_ref.fetch_add(1) + 1; }
    STDMETHODIMP_(ULONG) Release() override
    {
        ULONG r = m_ref.fetch_sub(1) - 1;
        if (r == 0) delete this;
        return r;
    }
    STDMETHODIMP CreateInstance(IUnknown* outer, REFIID riid, void** out) override
    {
        if (!out) return E_POINTER;
        *out = nullptr;
        if (outer) return CLASS_E_NOAGGREGATION;
        T* instance = new (std::nothrow) T();
        if (!instance) return E_OUTOFMEMORY;
        instance->AddRef();
        HRESULT hr = instance->QueryInterface(riid, out);
        instance->Release();
        return hr;
    }
    STDMETHODIMP LockServer(BOOL lock) override
    {
        if (lock) DllAddRef(); else DllRelease();
        return S_OK;
    }
private:
    std::atomic<ULONG> m_ref{1};
};

extern "C" {

STDAPI DllCanUnloadNow()
{
    return g_dllRefCount.load(std::memory_order_relaxed) == 0 ? S_OK : S_FALSE;
}

STDAPI DllGetClassObject(REFCLSID rclsid, REFIID riid, LPVOID* out)
{
    if (!out) return E_POINTER;
    *out = nullptr;
    if (rclsid != CLSID_SoundCoreVirtualCamera) return CLASS_E_CLASSNOTAVAILABLE;
    auto* f = new (std::nothrow) CFactory<sc::VirtualCameraFilter>();
    if (!f) return E_OUTOFMEMORY;
    HRESULT hr = f->QueryInterface(riid, out);
    f->Release();
    return hr;
}

} // extern "C"

BOOL APIENTRY DllMain(HMODULE, DWORD reason, LPVOID)
{
    if (reason == DLL_PROCESS_ATTACH)
        DisableThreadLibraryCalls(reinterpret_cast<HMODULE>(&__ImageBase));
    return TRUE;
}
