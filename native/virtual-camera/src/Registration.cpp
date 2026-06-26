// COM self-registration for the SoundCore Virtual Camera DirectShow
// filter. Apps that enumerate the VideoInputDeviceCategory will see the
// virtual camera in their device list after this runs.

#include <windows.h>
#include <unknwn.h>
#include <strsafe.h>
#include <dshow.h>
#include <strmif.h>
#include <uuids.h>

#include "Guids.h"

extern "C" IMAGE_DOS_HEADER __ImageBase;

namespace {

constexpr wchar_t kClsidGuidString[] = L"{8B3C2D60-2F47-46FF-A2A9-A52CD49DAD63}";
constexpr wchar_t kFriendlyName[]    = L"SoundCore Virtual Camera";

LSTATUS WriteString(HKEY parent, const wchar_t* subkey, const wchar_t* name, const wchar_t* value)
{
    HKEY key{};
    LSTATUS r = RegCreateKeyExW(parent, subkey, 0, nullptr, 0, KEY_SET_VALUE, nullptr, &key, nullptr);
    if (r != ERROR_SUCCESS) return r;
    r = RegSetValueExW(key, name, 0, REG_SZ,
                       reinterpret_cast<const BYTE*>(value),
                       (DWORD)((wcslen(value) + 1) * sizeof(wchar_t)));
    RegCloseKey(key);
    return r;
}

HRESULT registerWithFilterMapper(const wchar_t* modulePath)
{
    IFilterMapper2* mapper = nullptr;
    HRESULT hr = CoCreateInstance(CLSID_FilterMapper2, nullptr, CLSCTX_INPROC_SERVER,
                                  IID_IFilterMapper2, reinterpret_cast<void**>(&mapper));
    if (FAILED(hr)) return hr;

    REGFILTER2 rf2 = {};
    rf2.dwVersion = 1;
    rf2.dwMerit   = MERIT_DO_NOT_USE; // we want it discoverable but not auto-picked
    rf2.cPins     = 0;
    rf2.rgPins    = nullptr;
    hr = mapper->RegisterFilter(CLSID_SoundCoreVirtualCamera,
                                kFriendlyName,
                                nullptr,
                                &CLSID_VideoInputDeviceCategory,
                                kFriendlyName,
                                &rf2);
    mapper->Release();
    (void)modulePath;
    return hr;
}

HRESULT unregisterWithFilterMapper()
{
    IFilterMapper2* mapper = nullptr;
    HRESULT hr = CoCreateInstance(CLSID_FilterMapper2, nullptr, CLSCTX_INPROC_SERVER,
                                  IID_IFilterMapper2, reinterpret_cast<void**>(&mapper));
    if (FAILED(hr)) return hr;
    hr = mapper->UnregisterFilter(&CLSID_VideoInputDeviceCategory, kFriendlyName,
                                  CLSID_SoundCoreVirtualCamera);
    mapper->Release();
    return hr;
}

} // namespace

extern "C" STDAPI DllRegisterServer()
{
    wchar_t modulePath[MAX_PATH]{};
    if (GetModuleFileNameW(reinterpret_cast<HMODULE>(&__ImageBase), modulePath, MAX_PATH) == 0)
        return HRESULT_FROM_WIN32(GetLastError());

    wchar_t clsidKey[256];
    StringCchPrintfW(clsidKey, _countof(clsidKey), L"CLSID\\%s", kClsidGuidString);
    LSTATUS r = WriteString(HKEY_CLASSES_ROOT, clsidKey, nullptr, kFriendlyName);
    if (r != ERROR_SUCCESS) return HRESULT_FROM_WIN32(r);

    wchar_t inprocKey[256];
    StringCchPrintfW(inprocKey, _countof(inprocKey), L"CLSID\\%s\\InprocServer32", kClsidGuidString);
    r = WriteString(HKEY_CLASSES_ROOT, inprocKey, nullptr, modulePath);
    if (r != ERROR_SUCCESS) return HRESULT_FROM_WIN32(r);
    r = WriteString(HKEY_CLASSES_ROOT, inprocKey, L"ThreadingModel", L"Both");
    if (r != ERROR_SUCCESS) return HRESULT_FROM_WIN32(r);

    HRESULT hr = CoInitializeEx(nullptr, COINIT_APARTMENTTHREADED);
    if (hr == RPC_E_CHANGED_MODE) hr = CoInitializeEx(nullptr, COINIT_MULTITHREADED);
    HRESULT regHr = registerWithFilterMapper(modulePath);
    if (SUCCEEDED(hr)) CoUninitialize();
    return regHr;
}

extern "C" STDAPI DllUnregisterServer()
{
    HRESULT hr = CoInitializeEx(nullptr, COINIT_APARTMENTTHREADED);
    if (hr == RPC_E_CHANGED_MODE) hr = CoInitializeEx(nullptr, COINIT_MULTITHREADED);
    HRESULT unr = unregisterWithFilterMapper();
    if (SUCCEEDED(hr)) CoUninitialize();

    wchar_t clsidKey[256];
    StringCchPrintfW(clsidKey, _countof(clsidKey), L"CLSID\\%s", kClsidGuidString);
    RegDeleteTreeW(HKEY_CLASSES_ROOT, clsidKey);
    return unr;
}
