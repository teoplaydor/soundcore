//! Camera enumeration via Media Foundation.

use crate::Result;
use std::ffi::c_void;
use windows::Win32::Media::MediaFoundation::{
    IMFActivate, MFCreateAttributes, MFEnumDeviceSources, MFMediaType_Video,
    MF_DEVSOURCE_ATTRIBUTE_FRIENDLY_NAME, MF_DEVSOURCE_ATTRIBUTE_SOURCE_TYPE,
    MF_DEVSOURCE_ATTRIBUTE_SOURCE_TYPE_VIDCAP_GUID,
    MF_DEVSOURCE_ATTRIBUTE_SOURCE_TYPE_VIDCAP_SYMBOLIC_LINK,
};
use windows::Win32::System::Com::CoTaskMemFree;

#[derive(Debug, Clone)]
pub struct CameraFormat {
    pub width: u32,
    pub height: u32,
    pub frame_rate_num: u32,
    pub frame_rate_den: u32,
    /// FourCC-ish subtype name: "NV12", "YUY2", "MJPG", "RGB24", ...
    pub subtype: String,
}

#[derive(Debug, Clone)]
pub struct CameraSource {
    pub symbolic_link: String,
    pub friendly_name: String,
    pub formats: Vec<CameraFormat>,
}

pub fn enumerate() -> Result<Vec<CameraSource>> {
    // Build an attribute store specifying we want video capture sources.
    let attrs = unsafe {
        let mut a = None;
        MFCreateAttributes(&mut a, 1)?;
        a.expect("MFCreateAttributes succeeded but yielded None")
    };
    unsafe {
        attrs.SetGUID(
            &MF_DEVSOURCE_ATTRIBUTE_SOURCE_TYPE,
            &MF_DEVSOURCE_ATTRIBUTE_SOURCE_TYPE_VIDCAP_GUID,
        )?;
    }

    let mut count: u32 = 0;
    let mut raw_activates: *mut Option<IMFActivate> = std::ptr::null_mut();
    unsafe { MFEnumDeviceSources(&attrs, &mut raw_activates, &mut count)? };

    let mut out = Vec::with_capacity(count as usize);
    for i in 0..count {
        let act = unsafe { (*raw_activates.add(i as usize)).clone() };
        if let Some(act) = act {
            if let Ok(src) = build_camera_source(&act) {
                out.push(src);
            }
        }
    }
    unsafe { CoTaskMemFree(Some(raw_activates as *const c_void)) };
    Ok(out)
}

fn build_camera_source(act: &IMFActivate) -> Result<CameraSource> {
    let symbolic_link = read_string_attr(act, &MF_DEVSOURCE_ATTRIBUTE_SOURCE_TYPE_VIDCAP_SYMBOLIC_LINK)
        .unwrap_or_default();
    let friendly_name = read_string_attr(act, &MF_DEVSOURCE_ATTRIBUTE_FRIENDLY_NAME)
        .unwrap_or_else(|_| "Camera".into());
    // Format enumeration via opening the source is heavy; do it lazily —
    // for the device list we return an empty `formats` and only populate
    // when the user explicitly picks a source.
    Ok(CameraSource {
        symbolic_link,
        friendly_name,
        formats: Vec::new(),
    })
}

fn read_string_attr(
    attrs: &IMFActivate,
    key: &windows::core::GUID,
) -> Result<String> {
    let needed = match unsafe { attrs.GetStringLength(key) } {
        Ok(n) => n,
        Err(_) => return Ok(String::new()),
    };
    if needed == 0 {
        return Ok(String::new());
    }
    let mut buf = vec![0u16; (needed + 1) as usize];
    unsafe {
        attrs.GetString(key, &mut buf, None)?;
    }
    let len = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
    Ok(String::from_utf16_lossy(&buf[..len]))
}

#[allow(dead_code)]
fn major_type_is_video(g: &windows::core::GUID) -> bool {
    g == &MFMediaType_Video
}
