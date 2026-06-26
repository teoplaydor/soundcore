//! Tokio-friendly wrappers around the WASAPI / COM operations.
//!
//! WASAPI lives in COM-land, and a `tokio` worker thread does not
//! initialise COM automatically. We therefore run every Core-Audio call
//! on the blocking pool, and we lazy-initialise the MTA on each blocking
//! thread the first time it touches audio. The init lives in a
//! thread-local so a thread that's reused services subsequent requests
//! without re-paying the COM-init cost.

use anyhow::Context;
use soundcore_audio::process::ProcessInfo;
use soundcore_audio::{
    com_init_mta, AudioDevice, ComGuard, DataFlow, DeviceEnumerator, Role,
};
use std::cell::OnceCell;

thread_local! {
    static COM_GUARD: OnceCell<ComGuard> = const { OnceCell::new() };
}

fn ensure_com_inited() -> anyhow::Result<()> {
    COM_GUARD.with(|cell| -> anyhow::Result<()> {
        if cell.get().is_none() {
            let g = com_init_mta().context("CoInitializeEx(MTA) on blocking worker")?;
            cell.set(g).map_err(|_| anyhow::anyhow!("COM_GUARD already set"))?;
        }
        Ok(())
    })
}

pub async fn list_devices(flow: DataFlow) -> anyhow::Result<Vec<AudioDevice>> {
    tokio::task::spawn_blocking(move || -> anyhow::Result<Vec<AudioDevice>> {
        ensure_com_inited()?;
        let enumerator = DeviceEnumerator::new()?;
        Ok(enumerator.list(flow)?)
    })
    .await
    .context("spawn_blocking joined with panic")?
}

pub async fn default_device(flow: DataFlow, role: Role) -> anyhow::Result<Option<AudioDevice>> {
    tokio::task::spawn_blocking(move || -> anyhow::Result<Option<AudioDevice>> {
        ensure_com_inited()?;
        let enumerator = DeviceEnumerator::new()?;
        Ok(enumerator.default(flow, role)?)
    })
    .await
    .context("spawn_blocking joined with panic")?
}

pub async fn list_processes() -> anyhow::Result<Vec<ProcessInfo>> {
    tokio::task::spawn_blocking(|| -> anyhow::Result<Vec<ProcessInfo>> {
        Ok(soundcore_audio::process::list_processes()?)
    })
    .await
    .context("spawn_blocking joined with panic")?
}

#[derive(Debug, Clone, Default, serde::Deserialize)]
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

/// Run the bundled `soundcore-vst-scanner.exe` to discover VST3 plugins.
/// The scanner is a thin JUCE-linked process; we keep it out of the core
/// service so the service doesn't have to itself link JUCE.
///
/// The scanner emits a JSON array on stdout. If it isn't installed
/// alongside the service binary yet, returns an empty list.
pub async fn list_vst_plugins() -> anyhow::Result<Vec<VstPluginDescriptor>> {
    tokio::task::spawn_blocking(|| -> anyhow::Result<Vec<VstPluginDescriptor>> {
        let exe = match std::env::current_exe()
            .ok()
            .and_then(|p| p.parent().map(|d| d.join("soundcore-vst-scanner.exe")))
        {
            Some(p) if p.exists() => p,
            _ => return Ok(Vec::new()),
        };
        let output = std::process::Command::new(exe).output()?;
        if !output.status.success() {
            return Ok(Vec::new());
        }
        let parsed: Vec<VstPluginDescriptor> =
            serde_json::from_slice(&output.stdout).unwrap_or_default();
        Ok(parsed)
    })
    .await
    .context("spawn_blocking joined with panic")?
}

pub async fn list_cameras() -> anyhow::Result<Vec<soundcore_camera::CameraSource>> {
    tokio::task::spawn_blocking(|| -> anyhow::Result<Vec<soundcore_camera::CameraSource>> {
        ensure_com_inited()?;
        ensure_mf_inited()?;
        let sources = soundcore_camera::source::enumerate()?;
        Ok(sources)
    })
    .await
    .context("spawn_blocking joined with panic")?
}

thread_local! {
    static MF_GUARD: OnceCell<soundcore_camera::init::MfGuard> = const { OnceCell::new() };
}

fn ensure_mf_inited() -> anyhow::Result<()> {
    MF_GUARD.with(|cell| -> anyhow::Result<()> {
        if cell.get().is_none() {
            let g = soundcore_camera::init::mf_startup_lite()
                .context("MFStartup on blocking worker")?;
            cell.set(g)
                .map_err(|_| anyhow::anyhow!("MF_GUARD already set"))?;
        }
        Ok(())
    })
}
