//! SoundCore audio engine.
//!
//! Responsibilities:
//!   * Enumerate WASAPI render/capture endpoints and the audio sessions on them.
//!   * Run the Process Loopback API to capture mixed audio for a specific PID
//!     (Windows 10 build 20348 / Windows 11 and later).
//!   * Bind to `IAudioPolicyConfigFactory` (undocumented COM) to re-route a
//!     given process to a chosen endpoint.
//!   * Expose endpoint-volume / mute control plus the
//!     [`IAudioEndpointVolumeCallback`] subscription used by `soundcore-mic-lock`.
//!
//! The crate is intentionally I/O-only: no DSP, no VST hosting. Anything that
//! needs to *transform* audio lives in `soundcore-apo-core` and the C++ APO
//! DLL on top of it.
//!
//! All COM use is initialized per-thread by the consuming binary
//! (the core service initialises COM as MTA at startup).

#![allow(unused_imports)] // submodules are still being filled in

pub mod device;
pub mod endpoint_volume;
pub mod init;
pub mod policy_config;
pub mod process;
pub mod process_loopback;
pub mod session;

pub use device::{AudioDevice, DataFlow, DeviceEnumerator, Role};
pub use endpoint_volume::{
    spawn_polling_watcher, EndpointVolume, VolumeChange, VolumeListener, WatcherHandle,
};
pub use init::{com_init_mta, ComGuard};
pub use process_loopback::{LoopbackMode, ProcessLoopbackCapture};

use thiserror::Error;

#[derive(Debug, Error)]
pub enum AudioError {
    #[error("COM error: {0}")]
    Com(#[from] windows::core::Error),

    #[error("device not found: {0}")]
    DeviceNotFound(String),

    #[error("process not found: pid {0}")]
    ProcessNotFound(u32),

    #[error("operation not supported on this Windows build: {0}")]
    Unsupported(&'static str),

    #[error("audio policy config interface not available: {0}")]
    PolicyConfigUnavailable(&'static str),

    #[error("invalid argument: {0}")]
    InvalidArgument(String),

    #[error("io: {0}")]
    Io(#[from] std::io::Error),
}

pub type Result<T> = std::result::Result<T, AudioError>;
