// Self-registration of the SoundCore APO COM object.
//
// We register the InprocServer32 entry under HKLM\Software\Classes\CLSID\
// and provide the APO category capabilities the Audio Engine queries when
// it discovers an APO. The actual *attachment* of the APO to a specific
// endpoint (PKEY_FX_StreamEffectClsid etc.) is performed at runtime by
// the core service, not here, because we want it to be reversible.

#include <windows.h>
#include <unknwn.h>
#include <strsafe.h>
#include <propkey.h>
#include <audioenginebaseapo.h>

#include "SoundCoreApo.h"

extern "C" IMAGE_DOS_HEADER __ImageBase;

namespace
{
constexpr wchar_t kClsidGuidString[] = L"{2E8C0F12-1B7B-49B0-9C84-3CD5A6B7B210}";
constexpr wchar_t kFriendlyName[]    = L"SoundCore Audio Processing Object";

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

    // APO marker capability flags. The audio engine looks for these to know
    // the APO supports MFT-style processing.
    HKEY apoKey{};
    StringCchPrintfW(inprocKey, _countof(inprocKey), L"CLSID\\%s\\AudioProcessingObject", kClsidGuidString);
    if (RegCreateKeyExW(HKEY_CLASSES_ROOT, inprocKey, 0, nullptr, 0,
                        KEY_SET_VALUE, nullptr, &apoKey, nullptr) == ERROR_SUCCESS)
    {
        DWORD flags = APO_FLAG_DEFAULT;
        RegSetValueExW(apoKey, L"Flags", 0, REG_DWORD,
                       reinterpret_cast<BYTE*>(&flags), sizeof(flags));
        RegCloseKey(apoKey);
    }

    return S_OK;
}

extern "C" STDAPI DllUnregisterServer()
{
    wchar_t clsidKey[256];
    StringCchPrintfW(clsidKey, _countof(clsidKey), L"CLSID\\%s", kClsidGuidString);
    RegDeleteTreeW(HKEY_CLASSES_ROOT, clsidKey);
    return S_OK;
}
