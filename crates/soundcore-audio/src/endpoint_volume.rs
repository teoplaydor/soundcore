//! Endpoint-volume control + a polling-based watcher that mic-lock uses.
//!
//! Note: a true COM `IAudioEndpointVolumeCallback` would be lower-latency
//! but requires implementing a COM interface from Rust. The current
//! `windows-rs` version we depend on does not expose the `_Impl` trait
//! for this interface, so we poll on a dedicated thread at ~25 ms cadence
//! — fast enough that an external app's volume write is reverted before
//! a human can notice, and slow enough that CPU cost is negligible
//! (a single OS round-trip per tick).

use std::sync::Arc;
use std::time::Duration;

use parking_lot::Mutex;
use windows::core::PCWSTR;
use windows::Win32::Media::Audio::Endpoints::IAudioEndpointVolume;
use windows::Win32::Media::Audio::{IMMDeviceEnumerator, MMDeviceEnumerator};
use windows::Win32::System::Com::{CoCreateInstance, CLSCTX_ALL};

use crate::init::ComGuard;
use crate::Result;

#[derive(Debug, Clone, Copy)]
pub struct VolumeChange {
    pub master_volume: f32,
    pub muted: bool,
}

pub type VolumeListener = Arc<dyn Fn(VolumeChange) + Send + Sync>;

pub struct EndpointVolume {
    raw: IAudioEndpointVolume,
}

impl EndpointVolume {
    pub fn open(device_id: &str) -> Result<Self> {
        let enumerator: IMMDeviceEnumerator =
            unsafe { CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL)? };
        let mut id_wide: Vec<u16> = device_id.encode_utf16().collect();
        id_wide.push(0);
        let imm = unsafe { enumerator.GetDevice(PCWSTR(id_wide.as_ptr()))? };
        let raw: IAudioEndpointVolume = unsafe { imm.Activate(CLSCTX_ALL, None)? };
        Ok(Self { raw })
    }

    pub fn master_volume_scalar(&self) -> Result<f32> {
        Ok(unsafe { self.raw.GetMasterVolumeLevelScalar()? })
    }

    pub fn set_master_volume_scalar(&self, value: f32) -> Result<()> {
        unsafe {
            self.raw
                .SetMasterVolumeLevelScalar(value.clamp(0.0, 1.0), std::ptr::null())?
        };
        Ok(())
    }

    /// Size of one hardware volume step as a 0..1 scalar (1 / step count).
    /// Used by mic-lock to derive an enforcement tolerance, since the
    /// endpoint quantizes any scalar we write to the nearest step.
    pub fn volume_step_scalar(&self) -> Result<f32> {
        let mut step: u32 = 0;
        let mut step_count: u32 = 0;
        unsafe { self.raw.GetVolumeStepInfo(&mut step, &mut step_count)? };
        if step_count > 1 {
            Ok(1.0 / step_count as f32)
        } else {
            Ok(2e-3)
        }
    }

    pub fn mute(&self) -> Result<bool> {
        Ok(unsafe { self.raw.GetMute()? }.as_bool())
    }

    pub fn set_mute(&self, mute: bool) -> Result<()> {
        unsafe { self.raw.SetMute(mute, std::ptr::null())? };
        Ok(())
    }

    pub fn raw(&self) -> &IAudioEndpointVolume {
        &self.raw
    }
}

/// Spawn a watcher thread that polls the endpoint volume at `interval` and
/// invokes `on_change` whenever the value drifts from the most recent
/// observation. The returned handle joins the thread on drop.
pub fn spawn_polling_watcher(
    device_id: String,
    interval: Duration,
    on_change: VolumeListener,
) -> WatcherHandle {
    let cancel = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let cancel_clone = cancel.clone();
    let join = std::thread::Builder::new()
        .name(format!("sc-vol-watch:{}", &device_id[..device_id.len().min(24)]))
        .spawn(move || {
            // Each watcher thread initialises COM itself; the guard lives
            // for the whole thread.
            let _com: ComGuard = match crate::init::com_init_mta() {
                Ok(g) => g,
                Err(e) => {
                    tracing::error!(error = ?e, "watcher: COM init failed; exiting");
                    return;
                }
            };
            let endpoint = match EndpointVolume::open(&device_id) {
                Ok(e) => e,
                Err(e) => {
                    tracing::error!(error = ?e, device_id, "watcher: open failed");
                    return;
                }
            };
            let last = Mutex::new(VolumeChange {
                master_volume: endpoint.master_volume_scalar().unwrap_or(1.0),
                muted: endpoint.mute().unwrap_or(false),
            });
            while !cancel_clone.load(std::sync::atomic::Ordering::Relaxed) {
                let cur = VolumeChange {
                    master_volume: endpoint.master_volume_scalar().unwrap_or(1.0),
                    muted: endpoint.mute().unwrap_or(false),
                };
                let changed = {
                    let mut l = last.lock();
                    if (l.master_volume - cur.master_volume).abs() > f32::EPSILON
                        || l.muted != cur.muted
                    {
                        *l = cur;
                        true
                    } else {
                        false
                    }
                };
                if changed {
                    on_change(cur);
                }
                std::thread::sleep(interval);
            }
        })
        .expect("spawn watcher thread");
    WatcherHandle {
        cancel,
        join: Some(join),
    }
}

pub struct WatcherHandle {
    cancel: Arc<std::sync::atomic::AtomicBool>,
    join: Option<std::thread::JoinHandle<()>>,
}

impl Drop for WatcherHandle {
    fn drop(&mut self) {
        self.cancel.store(true, std::sync::atomic::Ordering::Relaxed);
        if let Some(j) = self.join.take() {
            let _ = j.join();
        }
    }
}
