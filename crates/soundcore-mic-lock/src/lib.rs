//! Microphone volume lock.
//!
//! Strategy: a dedicated worker thread holds an `IAudioEndpointVolume`
//! and runs a tight enforcement loop. On every tick it reads the current
//! master scalar, compares it to the *target* (not to a "previously
//! observed" value), and pushes the endpoint back to the target if it
//! diverges. This is robust against the case where an external app
//! repeatedly writes the *same* non-target value: change-detection
//! watchers would think "nothing happened since the last poll" and
//! silently skip the revert; this enforcement loop doesn't.
//!
//! Tick interval is 15 ms — well under human perceptual latency for
//! "volume suddenly changed", and a single COM round-trip per tick is
//! negligible CPU.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread::JoinHandle;
use std::time::Duration;

use parking_lot::{Mutex, RwLock};
use thiserror::Error;
use tracing::{info, warn};

use soundcore_audio::{com_init_mta, EndpointVolume};

#[derive(Debug, Error)]
pub enum MicLockError {
    #[error("audio: {0}")]
    Audio(#[from] soundcore_audio::AudioError),

    #[error("not running")]
    NotRunning,

    #[error("failed to spawn worker thread: {0}")]
    Spawn(#[source] std::io::Error),

    #[error("snapshot thread panicked")]
    SnapshotPanicked,
}

pub type Result<T> = std::result::Result<T, MicLockError>;

#[derive(Debug, Clone)]
pub struct LockConfig {
    pub device_id: String,
    /// Target scalar (0..1). `None` means "freeze whatever is current
    /// when `start` is called".
    pub locked_volume: Option<f32>,
    pub also_lock_mute: bool,
    /// Kept for API stability with older callers. The new enforcement
    /// loop always reverts immediately.
    pub revert_immediately: bool,
}

#[derive(Default)]
pub struct MicLock {
    state: Arc<RwLock<State>>,
    /// Serializes `start`/`stop` so two concurrent `start()` calls can't
    /// each spawn a worker and leave one running, unstoppable, fighting the
    /// other over the same endpoint.
    op_lock: Mutex<()>,
}

#[derive(Default)]
struct State {
    running: bool,
    config: Option<LockConfig>,
    cancel: Option<Arc<AtomicBool>>,
    worker: Option<JoinHandle<()>>,
    /// Set to `false` by the worker on every exit path. `is_running()`
    /// checks it so a worker that died (USB unplug, COM failure) is reported
    /// as stopped instead of silently claiming to enforce.
    alive: Option<Arc<AtomicBool>>,
}

impl MicLock {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn start(&self, config: LockConfig) -> Result<()> {
        // Serialize the whole start sequence against concurrent start/stop.
        let _op = self.op_lock.lock();

        // Stop any prior worker so we don't end up with two threads
        // fighting over the same endpoint.
        self.stop_internal();

        // Capture initial volume / mute on a one-shot COM-init'd thread
        // so we know what "freeze current" means.
        let dev_id = config.device_id.clone();
        let initial = std::thread::spawn(move || -> Result<(f32, bool)> {
            let _com = com_init_mta().map_err(soundcore_audio::AudioError::from)?;
            let ep = EndpointVolume::open(&dev_id)?;
            Ok((ep.master_volume_scalar()?, ep.mute()?))
        })
        .join()
        .map_err(|_| MicLockError::SnapshotPanicked)??;

        let target_volume = config
            .locked_volume
            .unwrap_or(initial.0)
            .clamp(0.0, 1.0);
        let target_mute_opt = if config.also_lock_mute {
            Some(initial.1)
        } else {
            None
        };

        let cancel = Arc::new(AtomicBool::new(false));
        let cancel_clone = cancel.clone();
        let alive = Arc::new(AtomicBool::new(true));
        let alive_worker = alive.clone();
        let dev_id_worker = config.device_id.clone();

        let worker = std::thread::Builder::new()
            .name("sc-mic-lock".into())
            .spawn(move || {
                // Clear the alive flag on every exit path so is_running()
                // reflects reality if the worker dies early.
                struct AliveGuard(Arc<AtomicBool>);
                impl Drop for AliveGuard {
                    fn drop(&mut self) {
                        self.0.store(false, Ordering::Relaxed);
                    }
                }
                let _alive_guard = AliveGuard(alive_worker);

                let _com = match com_init_mta() {
                    Ok(g) => g,
                    Err(e) => {
                        warn!(error = ?e, "mic-lock: COM init failed; thread exiting");
                        return;
                    }
                };
                let endpoint = match EndpointVolume::open(&dev_id_worker) {
                    Ok(e) => e,
                    Err(e) => {
                        warn!(
                            error = ?e,
                            device = %dev_id_worker,
                            "mic-lock: open endpoint failed; thread exiting"
                        );
                        return;
                    }
                };

                info!(
                    target_volume,
                    lock_mute = target_mute_opt.is_some(),
                    "mic-lock enforcement started"
                );

                // Immediately push the endpoint to the target so the
                // user sees the lock take effect on start, not only on
                // the first external poke.
                if let Err(e) = endpoint.set_master_volume_scalar(target_volume) {
                    warn!(error = ?e, "mic-lock: initial set failed");
                }
                if let Some(target_mute) = target_mute_opt {
                    let _ = endpoint.set_mute(target_mute);
                }

                // The endpoint quantizes the scalar to a hardware step, so the
                // value we read back is generally NOT bit-equal to the value we
                // wrote. Anchor enforcement to that quantized read-back, and use
                // half a volume step (or a small floor) as tolerance — otherwise
                // we'd rewrite the endpoint and log a "reverted" line on every
                // single tick forever (a self-feedback loop).
                let enforced_target = endpoint
                    .master_volume_scalar()
                    .unwrap_or(target_volume);
                let tolerance = endpoint
                    .volume_step_scalar()
                    .map(|s| (s * 0.5).max(1e-4))
                    .unwrap_or(2e-3);

                let interval = Duration::from_millis(15);
                while !cancel_clone.load(Ordering::Relaxed) {
                    // Enforce volume.
                    match endpoint.master_volume_scalar() {
                        Ok(cur) => {
                            if (cur - enforced_target).abs() > tolerance {
                                match endpoint.set_master_volume_scalar(enforced_target) {
                                    Ok(_) => info!(
                                        observed = cur,
                                        target = enforced_target,
                                        "mic-lock: reverted volume"
                                    ),
                                    Err(e) => warn!(
                                        error = ?e,
                                        "mic-lock: SetMasterVolumeLevelScalar failed"
                                    ),
                                }
                            }
                        }
                        Err(e) => {
                            // Endpoint may have gone away (USB unplug). Bail.
                            warn!(error = ?e, "mic-lock: read volume failed; exiting thread");
                            break;
                        }
                    }
                    // Enforce mute (if configured).
                    if let Some(target_mute) = target_mute_opt {
                        match endpoint.mute() {
                            Ok(cur_mute) => {
                                if cur_mute != target_mute {
                                    if let Err(e) = endpoint.set_mute(target_mute) {
                                        warn!(error = ?e, "mic-lock: SetMute failed");
                                    } else {
                                        info!(
                                            observed = cur_mute,
                                            target = target_mute,
                                            "mic-lock: reverted mute"
                                        );
                                    }
                                }
                            }
                            Err(e) => {
                                warn!(error = ?e, "mic-lock: read mute failed; exiting thread");
                                break;
                            }
                        }
                    }
                    std::thread::sleep(interval);
                }
                info!("mic-lock enforcement stopped");
            })
            .map_err(MicLockError::Spawn)?;

        let mut s = self.state.write();
        s.running = true;
        s.config = Some(config);
        s.cancel = Some(cancel);
        s.worker = Some(worker);
        s.alive = Some(alive);
        Ok(())
    }

    pub fn stop(&self) -> Result<()> {
        let _op = self.op_lock.lock();
        if !self.is_running() {
            return Err(MicLockError::NotRunning);
        }
        self.stop_internal();
        Ok(())
    }

    fn stop_internal(&self) {
        let (cancel, worker) = {
            let mut s = self.state.write();
            let c = s.cancel.take();
            let w = s.worker.take();
            s.running = false;
            s.config = None;
            s.alive = None;
            (c, w)
        };
        if let Some(c) = cancel {
            c.store(true, Ordering::Relaxed);
        }
        if let Some(w) = worker {
            let _ = w.join();
        }
    }

    pub fn is_running(&self) -> bool {
        let s = self.state.read();
        // Running only if we believe we started AND the worker hasn't died.
        s.running
            && s
                .alive
                .as_ref()
                .map(|a| a.load(Ordering::Relaxed))
                .unwrap_or(false)
    }

    pub fn config(&self) -> Option<LockConfig> {
        self.state.read().config.clone()
    }
}
