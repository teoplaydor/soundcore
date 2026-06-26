//! Undocumented `IAudioPolicyConfig` bindings.
//!
//! Used to override the default render/capture endpoint for a specific
//! application — the mechanism Windows itself uses for
//! "Settings → Sound → App volume and device preferences". Tools like
//! EarTrumpet and SoundVolumeView wrap it; we re-implement directly
//! because the interface isn't in any public SDK header.
//!
//! Vtable layout differs between Windows 10 and Windows 11. Both have a
//! long run of undocumented methods between IUnknown and our methods of
//! interest, so the right offset has to be picked at runtime.

use crate::{AudioError, Result};
use std::ffi::c_void;
use std::sync::atomic::{AtomicUsize, Ordering};
use windows::core::{HRESULT, HSTRING, GUID};

// IIDs reverse-engineered from EarTrumpet / SoundVolumeView source.
const IID_AUDIO_POLICY_CONFIG_FACTORY: GUID =
    GUID::from_u128(0x2A59116D_6C4F_45E0_A74F_707E3FEF9258);

const CLSID_AUDIO_POLICY_CONFIG_WIN10: GUID =
    GUID::from_u128(0x870AF99C_171D_4F9E_AF0D_E63DF40C2BC9);

const CLSID_AUDIO_POLICY_CONFIG_WIN11: GUID =
    GUID::from_u128(0xAB3D4648_E242_459F_B02F_541C70306324);

const CLSCTX_INPROC_SERVER: u32 = 0x1;
const CLSCTX_LOCAL_SERVER: u32 = 0x4;

#[link(name = "ole32")]
extern "system" {
    fn CoCreateInstance(
        rclsid: *const GUID,
        punkouter: *mut c_void,
        dwclscontext: u32,
        riid: *const GUID,
        ppv: *mut *mut c_void,
    ) -> HRESULT;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppRole {
    Console,
    Multimedia,
    Communications,
}

impl AppRole {
    fn as_i32(self) -> i32 {
        match self {
            AppRole::Console => 0,
            AppRole::Multimedia => 1,
            AppRole::Communications => 2,
        }
    }
}

fn data_flow_as_i32(flow: crate::DataFlow) -> i32 {
    match flow {
        crate::DataFlow::Render => 0,
        crate::DataFlow::Capture => 1,
    }
}

/// Wraps the interface pointer + the runtime-detected slot offset for
/// `SetPersistedDefaultAudioEndpoint`.
pub struct PolicyConfig {
    raw: *mut c_void,
    set_slot: AtomicUsize,
}

unsafe impl Send for PolicyConfig {}
unsafe impl Sync for PolicyConfig {}

impl PolicyConfig {
    pub fn open() -> Result<Self> {
        for clsid in [
            &CLSID_AUDIO_POLICY_CONFIG_WIN11,
            &CLSID_AUDIO_POLICY_CONFIG_WIN10,
        ] {
            let mut raw: *mut c_void = std::ptr::null_mut();
            let hr = unsafe {
                CoCreateInstance(
                    clsid,
                    std::ptr::null_mut(),
                    CLSCTX_INPROC_SERVER | CLSCTX_LOCAL_SERVER,
                    &IID_AUDIO_POLICY_CONFIG_FACTORY,
                    &mut raw,
                )
            };
            if hr.is_ok() && !raw.is_null() {
                return Ok(Self {
                    raw,
                    set_slot: AtomicUsize::new(guess_set_slot()),
                });
            }
        }
        Err(AudioError::PolicyConfigUnavailable(
            "neither Win10 nor Win11 AudioPolicyConfig CLSID worked",
        ))
    }

    pub fn set_persisted_default(
        &self,
        pid: u32,
        flow: crate::DataFlow,
        role: AppRole,
        endpoint_id: &str,
    ) -> Result<()> {
        let hstring = HSTRING::from(endpoint_id);
        let slot = self.set_slot.load(Ordering::Relaxed);
        let hr = unsafe {
            invoke_set_persisted_default(
                self.raw,
                slot,
                pid,
                data_flow_as_i32(flow),
                role.as_i32(),
                std::mem::transmute_copy::<HSTRING, *mut c_void>(&hstring),
            )
        };
        if hr.is_err() {
            return Err(AudioError::Com(windows::core::Error::from(hr)));
        }
        Ok(())
    }

    pub fn clear_persisted_default(
        &self,
        pid: u32,
        flow: crate::DataFlow,
        role: AppRole,
    ) -> Result<()> {
        self.set_persisted_default(pid, flow, role, "")
    }

    /// Override the auto-detected slot if the value turns out to be wrong
    /// on a particular Windows build (e.g. Microsoft inserted a new
    /// vtable entry).
    pub fn set_vtable_slot(&self, slot: usize) {
        self.set_slot.store(slot, Ordering::Relaxed);
    }
}

impl Drop for PolicyConfig {
    fn drop(&mut self) {
        if !self.raw.is_null() {
            unsafe {
                let vt = vtable(self.raw);
                if !vt.is_null() {
                    type Release = unsafe extern "system" fn(*mut c_void) -> u32;
                    let release: Release = std::mem::transmute(*vt.add(2));
                    release(self.raw);
                }
            }
        }
    }
}

unsafe fn vtable(this: *mut c_void) -> *const *const c_void {
    if this.is_null() {
        return std::ptr::null();
    }
    *(this as *const *const *const c_void)
}

unsafe fn invoke_set_persisted_default(
    this: *mut c_void,
    slot: usize,
    pid: u32,
    flow: i32,
    role: i32,
    endpoint_hstring: *mut c_void,
) -> HRESULT {
    type Fn = unsafe extern "system" fn(
        this: *mut c_void,
        pid: u32,
        flow: i32,
        role: i32,
        endpoint: *mut c_void,
    ) -> HRESULT;
    let vt = vtable(this);
    if vt.is_null() {
        return windows::core::HRESULT(0x80004001u32 as i32); // E_NOTIMPL
    }
    let entry: Fn = std::mem::transmute(*vt.add(slot));
    entry(this, pid, flow, role, endpoint_hstring)
}

/// Conservative initial guess for the vtable slot of
/// `SetPersistedDefaultAudioEndpoint`.
///
/// * Win10 19041..22000 — slot 25
/// * Win11 22000+       — slot 26
fn guess_set_slot() -> usize {
    if is_windows_11() {
        26
    } else {
        25
    }
}

fn is_windows_11() -> bool {
    use windows::Win32::System::Registry::{RegGetValueW, HKEY_LOCAL_MACHINE, RRF_RT_REG_SZ};
    let mut buf = [0u16; 32];
    let mut size: u32 = (buf.len() * std::mem::size_of::<u16>()) as u32;
    let subkey = wide_z(r"SOFTWARE\Microsoft\Windows NT\CurrentVersion");
    let name = wide_z("CurrentBuildNumber");
    let r = unsafe {
        RegGetValueW(
            HKEY_LOCAL_MACHINE,
            windows::core::PCWSTR(subkey.as_ptr()),
            windows::core::PCWSTR(name.as_ptr()),
            RRF_RT_REG_SZ,
            None,
            Some(buf.as_mut_ptr() as *mut _),
            Some(&mut size),
        )
    };
    if r.0 != 0 {
        return false;
    }
    let len = buf.iter().position(|&c| c == 0).unwrap_or(0);
    let s: String = String::from_utf16_lossy(&buf[..len]);
    s.parse::<u32>().map(|n| n >= 22000).unwrap_or(false)
}

fn wide_z(s: &str) -> Vec<u16> {
    let mut v: Vec<u16> = s.encode_utf16().collect();
    v.push(0);
    v
}
