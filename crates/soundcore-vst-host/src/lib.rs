//! Safe Rust wrapper over the C++ JUCE-based VST3 host living in
//! `native/vst-host`.

use std::ffi::{c_void, CStr, CString};
use std::ptr::NonNull;
use thiserror::Error;

pub mod ffi;

#[derive(Debug, Error)]
pub enum VstError {
    #[error("plugin scan failed: rc={0}")]
    ScanFailed(i32),
    #[error("plugin load failed: rc={0}")]
    LoadFailed(i32),
    #[error("chain create failed: rc={0}")]
    ChainCreateFailed(i32),
    #[error("native returned null")]
    NullHandle,
    #[error("native error: {0}")]
    Native(String),
    #[error("invalid arg: {0}")]
    InvalidArg(&'static str),
}

pub type Result<T> = std::result::Result<T, VstError>;

#[derive(Debug, Clone)]
pub struct VstPluginDescriptor {
    pub uid: String,
    pub name: String,
    pub vendor: String,
    pub category: String,
    pub path: String,
    pub num_inputs: u32,
    pub num_outputs: u32,
    pub has_editor: bool,
}

pub struct PluginIndex {
    raw: NonNull<c_void>,
}

unsafe impl Send for PluginIndex {}
unsafe impl Sync for PluginIndex {}

impl PluginIndex {
    pub fn scan(search_paths: &[&str]) -> Result<Self> {
        let cstrs: Vec<CString> = search_paths
            .iter()
            .map(|s| CString::new(*s).map_err(|_| VstError::InvalidArg("nul in path")))
            .collect::<Result<_>>()?;
        let ptrs: Vec<*const std::ffi::c_char> = cstrs.iter().map(|s| s.as_ptr()).collect();
        let mut out: *mut c_void = std::ptr::null_mut();
        let rc = unsafe {
            ffi::sc_vst_host_scan(
                if ptrs.is_empty() {
                    std::ptr::null()
                } else {
                    ptrs.as_ptr()
                },
                ptrs.len() as u32,
                &mut out,
            )
        };
        if rc != 0 {
            return Err(VstError::ScanFailed(rc));
        }
        let nn = NonNull::new(out).ok_or(VstError::NullHandle)?;
        Ok(Self { raw: nn })
    }

    pub fn plugins(&self) -> Vec<VstPluginDescriptor> {
        let size = unsafe { ffi::sc_vst_host_index_size(self.raw.as_ptr()) };
        let mut out = Vec::with_capacity(size as usize);
        for i in 0..size {
            let mut info = ffi::ScVstHostPluginInfo::default();
            let rc = unsafe { ffi::sc_vst_host_index_get(self.raw.as_ptr(), i, &mut info) };
            if rc != 0 {
                continue;
            }
            out.push(VstPluginDescriptor {
                uid: cstr_or_empty(info.uid),
                name: cstr_or_empty(info.name),
                vendor: cstr_or_empty(info.vendor),
                category: cstr_or_empty(info.category),
                path: cstr_or_empty(info.path),
                num_inputs: info.num_inputs,
                num_outputs: info.num_outputs,
                has_editor: info.has_editor != 0,
            });
        }
        out
    }
}

impl Drop for PluginIndex {
    fn drop(&mut self) {
        unsafe { ffi::sc_vst_host_index_free(self.raw.as_ptr()) };
    }
}

pub struct ChainInstance {
    raw: NonNull<c_void>,
}

// Send only: the chain may be moved between threads, but it is NOT Sync.
// `process()` de-interleaves into a shared scratch buffer partly outside the
// C++ mutex, so two threads calling through `&ChainInstance` at once would
// race that buffer. Callers that share a chain must serialize it (e.g. wrap
// it in a Mutex) — exactly what the audio engine does.
unsafe impl Send for ChainInstance {}

impl ChainInstance {
    pub fn new(sample_rate: f64, max_block_samples: u32, channels: u32) -> Result<Self> {
        let mut out: *mut c_void = std::ptr::null_mut();
        let rc = unsafe {
            ffi::sc_vst_host_chain_new(sample_rate, max_block_samples, channels, &mut out)
        };
        if rc != 0 {
            return Err(VstError::ChainCreateFailed(rc));
        }
        let nn = NonNull::new(out).ok_or(VstError::NullHandle)?;
        Ok(Self { raw: nn })
    }

    pub fn append(&self, plugin_uid: &str, plugin_path: &str) -> Result<[u8; 16]> {
        let uid = CString::new(plugin_uid).map_err(|_| VstError::InvalidArg("nul in uid"))?;
        let path = CString::new(plugin_path).map_err(|_| VstError::InvalidArg("nul in path"))?;
        let mut node_id = [0u8; 16];
        let rc = unsafe {
            ffi::sc_vst_host_chain_append(self.raw.as_ptr(), uid.as_ptr(), path.as_ptr(), &mut node_id)
        };
        if rc != 0 {
            return Err(VstError::LoadFailed(rc));
        }
        Ok(node_id)
    }

    pub fn remove(&self, node_id: &[u8; 16]) -> Result<()> {
        let rc = unsafe { ffi::sc_vst_host_chain_remove(self.raw.as_ptr(), node_id) };
        if rc != 0 {
            return Err(VstError::LoadFailed(rc));
        }
        Ok(())
    }

    /// Process audio in place. Assumes interleaved layout.
    pub fn process(&self, interleaved: &mut [f32], channels: u32) -> Result<()> {
        if channels == 0 {
            return Err(VstError::InvalidArg("channels=0"));
        }
        if interleaved.len() % channels as usize != 0 {
            return Err(VstError::InvalidArg("len not divisible by channels"));
        }
        let frames = (interleaved.len() / channels as usize) as u32;
        let rc = unsafe {
            ffi::sc_vst_host_chain_process(
                self.raw.as_ptr(),
                interleaved.as_mut_ptr(),
                channels,
                frames,
            )
        };
        if rc != 0 {
            return Err(VstError::LoadFailed(rc));
        }
        Ok(())
    }

    pub fn set_parameter(&self, node_id: &[u8; 16], index: u32, value: f32) -> Result<()> {
        let rc = unsafe {
            ffi::sc_vst_host_chain_set_parameter(self.raw.as_ptr(), node_id, index, value)
        };
        if rc != 0 {
            return Err(VstError::LoadFailed(rc));
        }
        Ok(())
    }
}

impl Drop for ChainInstance {
    fn drop(&mut self) {
        unsafe { ffi::sc_vst_host_chain_free(self.raw.as_ptr()) };
    }
}

fn cstr_or_empty(p: *const std::ffi::c_char) -> String {
    if p.is_null() {
        return String::new();
    }
    unsafe { CStr::from_ptr(p) }
        .to_string_lossy()
        .into_owned()
}

/// Read a host last-error string (set by native code) as a Rust String.
pub fn last_error() -> String {
    cstr_or_empty(unsafe { ffi::sc_vst_host_last_error() })
}
