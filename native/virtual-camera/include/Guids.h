// CLSIDs for the SoundCore virtual camera.
//
// CLSID_SoundCoreVirtualCamera — the DirectShow Source Filter / MF
// Virtual Camera. Apps see this in their camera list as
// "SoundCore Virtual Camera".

#pragma once

#include <guiddef.h>

// {8B3C2D60-2F47-46FF-A2A9-A52CD49DAD63}
DEFINE_GUID(CLSID_SoundCoreVirtualCamera,
    0x8b3c2d60, 0x2f47, 0x46ff, 0xa2, 0xa9, 0xa5, 0x2c, 0xd4, 0x9d, 0xad, 0x63);

// Shared-memory channel name used by the core service producer and
// consumers in this DLL. Format: "SoundCore.Camera.<index>"; we ship one
// channel for now.
#define SC_CAMERA_SHARED_NAME L"SoundCore.Camera.0"
