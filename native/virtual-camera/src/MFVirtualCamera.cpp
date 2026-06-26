// Windows 11 Media Foundation Virtual Camera shim.
//
// On Windows 11 22000+ the MFVirtualCamera API gives us a much simpler
// integration: register a real MF media source under
// MFVirtualCameraType_SoftwareCameraSource, no DirectShow Source Filter
// gymnastics. Apps that use Media Foundation (the default for most
// modern apps) see it natively; apps that still use DirectShow fall back
// to the in-tree DShow Source Filter declared in Filter.h.
//
// The actual registration is driven from the core service via the IPC
// API; we expose the C entry points here so the service can call them
// without re-implementing the MF plumbing.

#include <windows.h>
#include <mfapi.h>
#include <mfidl.h>
#include <mfvirtualcamera.h>

#include "Guids.h"

// Type alias for the symbol that ships with newer Windows 11 SDKs.
using MFCreateVirtualCameraFn = HRESULT (WINAPI*)(
    MFVirtualCameraType cameraType,
    MFVirtualCameraLifetime lifetime,
    MFVirtualCameraAccess access,
    LPCWSTR friendlyName,
    LPCWSTR sourceId,
    const GUID* categories,
    ULONG categoryCount,
    IMFVirtualCamera** virtualCamera);

extern "C" HRESULT sc_virtcam_register_mf(LPCWSTR friendlyName, LPCWSTR sourceId)
{
    HMODULE mfsensor = LoadLibraryW(L"mfsensorgroup.dll");
    if (!mfsensor) return HRESULT_FROM_WIN32(GetLastError());
    auto pfn = reinterpret_cast<MFCreateVirtualCameraFn>(
        GetProcAddress(mfsensor, "MFCreateVirtualCamera"));
    if (!pfn) { FreeLibrary(mfsensor); return E_NOTIMPL; }

    IMFVirtualCamera* vc = nullptr;
    HRESULT hr = pfn(
        MFVirtualCameraType_SoftwareCameraSource,
        MFVirtualCameraLifetime_System,
        MFVirtualCameraAccess_CurrentUser,
        friendlyName ? friendlyName : L"SoundCore Virtual Camera",
        sourceId    ? sourceId    : L"SoundCore.Camera.0",
        nullptr, 0,
        &vc);
    if (SUCCEEDED(hr) && vc)
    {
        hr = vc->Start(nullptr);
        vc->Release();
    }
    FreeLibrary(mfsensor);
    return hr;
}

extern "C" HRESULT sc_virtcam_unregister_mf(LPCWSTR sourceId)
{
    HMODULE mfsensor = LoadLibraryW(L"mfsensorgroup.dll");
    if (!mfsensor) return HRESULT_FROM_WIN32(GetLastError());
    using FnRemove = HRESULT (WINAPI*)(LPCWSTR);
    auto pfn = reinterpret_cast<FnRemove>(
        GetProcAddress(mfsensor, "MFShutdownVirtualCamera"));
    HRESULT hr = pfn ? pfn(sourceId) : E_NOTIMPL;
    FreeLibrary(mfsensor);
    return hr;
}
