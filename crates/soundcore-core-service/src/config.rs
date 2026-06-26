//! On-disk service configuration. Written to
//! `%ProgramData%\SoundCore\config.json`.
//!
//! We persist anything that should survive a service restart: mic-lock
//! settings, per-device FX chains (eventually), per-app routing rules,
//! camera multiplex config.

use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Arc;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ServiceConfig {
    #[serde(default)]
    pub mic_lock: MicLockConfig,
    #[serde(default)]
    pub camera: CameraConfig,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct MicLockConfig {
    pub enabled: bool,
    pub device_id: String,
    /// `None` = freeze current value when enabling.
    pub locked_volume: Option<f32>,
    pub also_lock_mute: bool,
    pub revert_immediately: bool,
    pub allowed_image_globs: Vec<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CameraConfig {
    pub enabled: bool,
    pub source_symbolic_link: String,
    pub width: u32,
    pub height: u32,
    pub frame_rate_num: u32,
    pub frame_rate_den: u32,
}

#[derive(Default, Clone)]
pub struct ConfigStore {
    inner: Arc<RwLock<ServiceConfig>>,
}

impl ConfigStore {
    pub fn load_or_default() -> Self {
        let cfg = read_from_disk().unwrap_or_default();
        Self {
            inner: Arc::new(RwLock::new(cfg)),
        }
    }

    pub fn snapshot(&self) -> ServiceConfig {
        self.inner.read().clone()
    }

    pub fn with_mut<F: FnOnce(&mut ServiceConfig)>(&self, f: F) -> anyhow::Result<()> {
        {
            let mut g = self.inner.write();
            f(&mut g);
        }
        let snap = self.inner.read().clone();
        write_to_disk(&snap)
    }
}

fn config_path() -> PathBuf {
    let base = std::env::var_os("ProgramData")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(r"C:\ProgramData"));
    base.join("SoundCore").join("config.json")
}

fn read_from_disk() -> Option<ServiceConfig> {
    let path = config_path();
    let bytes = std::fs::read(&path).ok()?;
    serde_json::from_slice(&bytes).ok()
}

fn write_to_disk(cfg: &ServiceConfig) -> anyhow::Result<()> {
    let path = config_path();
    if let Some(p) = path.parent() {
        std::fs::create_dir_all(p)?;
    }
    let serialized = serde_json::to_vec_pretty(cfg)?;
    // Write to a sidecar then rename to make the swap atomic on Windows.
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, &serialized)?;
    std::fs::rename(&tmp, &path)?;
    Ok(())
}
