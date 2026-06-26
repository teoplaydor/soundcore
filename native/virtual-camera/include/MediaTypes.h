// Helpers for advertising the camera formats our virtual camera supports.

#pragma once

#include <windows.h>
#include <dshow.h>
#include <strmif.h>
#include <vector>

namespace sc {

struct AdvertisedFormat {
    GUID         subtype;          // MEDIASUBTYPE_NV12 etc.
    uint32_t     width;
    uint32_t     height;
    uint32_t     frame_rate_num;   // numerator (e.g. 30)
    uint32_t     frame_rate_den;   // denominator (e.g. 1)
    uint32_t     bits_per_pixel;
};

// Default catalogue we advertise when the producer hasn't yet told us
// what the real camera supports. Common resolutions only.
std::vector<AdvertisedFormat> defaultAdvertisedFormats();

// Fill a DirectShow AM_MEDIA_TYPE for the given AdvertisedFormat.
// Caller owns the resulting `pbFormat` and must `CoTaskMemFree` it.
HRESULT buildAmMediaType(const AdvertisedFormat& f, AM_MEDIA_TYPE& out);

} // namespace sc
