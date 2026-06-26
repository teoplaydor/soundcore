//! Top-level runtime: brings up the audio engine, mic-lock, camera proxy
//! and the IPC server, then awaits the cancellation token.

use std::sync::Arc;

use tokio_util::sync::CancellationToken;
use tracing::{info, warn};

use crate::config::ConfigStore;
use crate::server;
use soundcore_mic_lock::{LockConfig, MicLock};

#[derive(Clone)]
pub struct Services {
    pub mic_lock: Arc<MicLock>,
    pub config: ConfigStore,
}

impl Services {
    pub fn build() -> anyhow::Result<Self> {
        Ok(Self {
            mic_lock: Arc::new(MicLock::new()),
            config: ConfigStore::load_or_default(),
        })
    }

    /// Apply whatever the on-disk config asks for (mic-lock state, camera
    /// state, etc.). Called once at service startup and after any IPC
    /// write that may have changed config.
    pub fn apply_config(&self) {
        let cfg = self.config.snapshot();
        if cfg.mic_lock.enabled && !cfg.mic_lock.device_id.is_empty() {
            // stop any prior watcher
            let _ = self.mic_lock.stop();
            let lc = LockConfig {
                device_id: cfg.mic_lock.device_id.clone(),
                locked_volume: cfg.mic_lock.locked_volume,
                also_lock_mute: cfg.mic_lock.also_lock_mute,
                revert_immediately: cfg.mic_lock.revert_immediately,
            };
            match self.mic_lock.start(lc) {
                Ok(_) => info!(device = %cfg.mic_lock.device_id, "mic-lock applied from config"),
                Err(e) => warn!(error = ?e, "mic-lock apply failed"),
            }
        } else {
            let _ = self.mic_lock.stop();
        }
    }
}

pub async fn run(cancel: CancellationToken) -> anyhow::Result<()> {
    let services = Arc::new(Services::build()?);
    services.apply_config();

    info!("services constructed; spawning IPC server");

    let server_handle = {
        let cancel = cancel.clone();
        let services = services.clone();
        tokio::spawn(async move {
            if let Err(e) = server::serve(services, cancel).await {
                warn!(error = ?e, "IPC server task ended with error");
            }
        })
    };

    cancel.cancelled().await;
    info!("cancellation requested; shutting down");

    let _ = services.mic_lock.stop();
    server_handle.abort();
    let _ = server_handle.await;
    Ok(())
}
