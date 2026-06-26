//! Per-thread COM initialisation helper.
//!
//! WASAPI / MMDevice APIs need COM. Calling [`com_init_mta`] from a
//! background worker is the right thing for the core service: it places
//! the thread in the MTA so any later call into Core Audio is free of
//! Single-Threaded-Apartment serialisation.
//!
//! Returns a guard that calls [`CoUninitialize`] on drop. Failing to keep
//! the guard alive means later COM calls on that thread will fail with
//! `CO_E_NOTINITIALIZED`.

use windows::Win32::System::Com::{CoInitializeEx, CoUninitialize, COINIT_MULTITHREADED};

pub struct ComGuard {
    _private: (),
}

impl Drop for ComGuard {
    fn drop(&mut self) {
        unsafe { CoUninitialize() };
    }
}

pub fn com_init_mta() -> windows::core::Result<ComGuard> {
    let hr = unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) };
    hr.ok()?;
    Ok(ComGuard { _private: () })
}
