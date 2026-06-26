#include "MediaTypes.h"

#include <uuids.h>
#include <amvideo.h>
#include <mmreg.h>

namespace sc {

std::vector<AdvertisedFormat> defaultAdvertisedFormats()
{
    // We advertise a baseline of resolutions that are universally supported
    // by webcams; per-instance we re-advertise the producer's actual list.
    return {
        { MEDIASUBTYPE_NV12, 1920, 1080, 30, 1, 12 },
        { MEDIASUBTYPE_NV12, 1280,  720, 30, 1, 12 },
        { MEDIASUBTYPE_NV12,  640,  480, 30, 1, 12 },
        { MEDIASUBTYPE_YUY2, 1280,  720, 30, 1, 16 },
        { MEDIASUBTYPE_YUY2,  640,  480, 30, 1, 16 },
    };
}

HRESULT buildAmMediaType(const AdvertisedFormat& f, AM_MEDIA_TYPE& out)
{
    ZeroMemory(&out, sizeof(out));
    out.majortype  = MEDIATYPE_Video;
    out.subtype    = f.subtype;
    out.bFixedSizeSamples = TRUE;
    out.bTemporalCompression = FALSE;
    out.formattype = FORMAT_VideoInfo;
    out.cbFormat   = sizeof(VIDEOINFOHEADER);
    out.pbFormat   = static_cast<BYTE*>(CoTaskMemAlloc(sizeof(VIDEOINFOHEADER)));
    if (!out.pbFormat) return E_OUTOFMEMORY;
    auto* vih = reinterpret_cast<VIDEOINFOHEADER*>(out.pbFormat);
    ZeroMemory(vih, sizeof(*vih));
    vih->AvgTimePerFrame = (REFERENCE_TIME)((10000000LL * f.frame_rate_den) / f.frame_rate_num);
    vih->bmiHeader.biSize        = sizeof(BITMAPINFOHEADER);
    vih->bmiHeader.biWidth       = static_cast<LONG>(f.width);
    vih->bmiHeader.biHeight      = static_cast<LONG>(f.height);
    vih->bmiHeader.biPlanes      = 1;
    vih->bmiHeader.biBitCount    = static_cast<WORD>(f.bits_per_pixel);
    vih->bmiHeader.biCompression = f.subtype.Data1; // FourCC
    vih->bmiHeader.biSizeImage   = (f.width * f.height * f.bits_per_pixel) / 8;
    out.lSampleSize              = vih->bmiHeader.biSizeImage;
    return S_OK;
}

} // namespace sc
