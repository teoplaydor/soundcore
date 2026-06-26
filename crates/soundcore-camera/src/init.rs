//! Media Foundation per-thread init. Mirror of `soundcore_audio::init`
//! but for the MF runtime.

use windows::Win32::Media::MediaFoundation::{
    MFShutdown, MFStartup, MF_API_VERSION, MFSTARTUP_LITE,
};

pub struct MfGuard {
    _private: (),
}

impl Drop for MfGuard {
    fn drop(&mut self) {
        unsafe {
            let _ = MFShutdown();
        }
    }
}

pub fn mf_startup_lite() -> windows::core::Result<MfGuard> {
    unsafe { MFStartup(MF_API_VERSION, MFSTARTUP_LITE)? };
    Ok(MfGuard { _private: () })
}
