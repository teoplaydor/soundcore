// Materialise the SoundCore APO's own CLSID exactly once. Without this TU
// the DEFINE_GUID macro in SoundCoreApo.h just declares an extern const
// GUID — and dllmain.cpp / SoundCoreApo.cpp link against a missing symbol.

#define INITGUID
#include <guiddef.h>

// {2E8C0F12-1B7B-49B0-9C84-3CD5A6B7B210}
DEFINE_GUID(CLSID_SoundCoreApo,
    0x2e8c0f12, 0x1b7b, 0x49b0, 0x9c, 0x84, 0x3c, 0xd5, 0xa6, 0xb7, 0xb2, 0x10);
