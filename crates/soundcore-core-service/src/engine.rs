//! In-process audio engine: WASAPI enumeration, process enumeration,
//! camera enumeration, mic-lock, and VST chain config management.

use std::sync::Arc;
use std::thread::JoinHandle;

use crossbeam::channel::{unbounded, Receiver, Sender};
use parking_lot::RwLock;
use serde::Deserialize;
use tracing::{info, warn};

use soundcore_audio::process::ProcessInfo;
use soundcore_audio::{com_init_mta, AudioDevice, DataFlow, DeviceEnumerator, Role};
use soundcore_camera::capture::CaptureSession;
use soundcore_camera::CameraSource;
use soundcore_mic_lock::{LockConfig, MicLock};

use crate::config::{CameraConfig as CfgCamera, ConfigStore, MicLockConfig as CfgMicLock};
use crate::embed;

#[derive(Debug, Clone, Default, Deserialize)]
pub struct VstPluginDescriptor {
    pub uid: String,
    pub name: String,
    pub vendor: String,
    pub category: String,
    pub path: String,
    #[serde(default)]
    pub num_inputs: u32,
    #[serde(default)]
    pub num_outputs: u32,
    #[serde(default)]
    pub has_editor: bool,
}

#[derive(Default)]
pub struct EngineState {
    pub render_devices: RwLock<Vec<AudioDevice>>,
    pub capture_devices: RwLock<Vec<AudioDevice>>,
    pub processes: RwLock<Vec<ProcessInfo>>,
    pub cameras: RwLock<Vec<CameraSource>>,
    pub vst_plugins: RwLock<Vec<VstPluginDescriptor>>,
    /// Ordered list of plugin paths currently active in the APO chain.
    pub chain: RwLock<Vec<String>>,
    pub last_error: RwLock<Option<String>>,
    pub last_info: RwLock<Option<String>>,
    pub mic_lock_running: RwLock<bool>,
    pub camera_running: RwLock<bool>,
}

pub enum Command {
    RefreshAll,
    RefreshDevices,
    RefreshProcesses,
    RefreshCameras,
    RefreshVstPlugins,
    SetMicLock(CfgMicLock),
    SetCamera(CfgCamera),
    SaveChain(Vec<String>),
    Shutdown,
}

pub struct Engine {
    pub state: Arc<EngineState>,
    pub config: ConfigStore,
    pub mic_lock: Arc<MicLock>,
    tx: Sender<Command>,
    worker: Option<JoinHandle<()>>,
}

impl Engine {
    pub fn start(config: ConfigStore) -> Self {
        let state = Arc::new(EngineState::default());
        let mic_lock = Arc::new(MicLock::new());
        let (tx, rx) = unbounded();

        let worker_state = state.clone();
        let worker_mic = mic_lock.clone();
        let worker_config = config.clone();
        let worker = std::thread::Builder::new()
            .name("sc-engine".into())
            .spawn(move || worker_loop(rx, worker_state, worker_mic, worker_config))
            .expect("spawn sc-engine thread");

        let engine = Self {
            state,
            config,
            mic_lock,
            tx,
            worker: Some(worker),
        };

        engine.cmd(Command::RefreshAll);
        engine.cmd(Command::RefreshVstPlugins);
        engine.load_chain_from_disk();
        engine.apply_config_from_disk();
        engine
    }

    pub fn cmd(&self, c: Command) {
        let _ = self.tx.send(c);
    }

    pub fn apply_config_from_disk(&self) {
        let snap = self.config.snapshot();
        if snap.mic_lock.enabled && !snap.mic_lock.device_id.is_empty() {
            self.cmd(Command::SetMicLock(snap.mic_lock.clone()));
        }
        if snap.camera.enabled && !snap.camera.source_symbolic_link.is_empty() {
            self.cmd(Command::SetCamera(snap.camera.clone()));
        }
    }

    fn load_chain_from_disk(&self) {
        let path = chain_config_path();
        let chain: Vec<String> = std::fs::read_to_string(&path)
            .unwrap_or_default()
            .lines()
            .filter_map(|l| {
                let t = l.trim();
                if t.is_empty() || t.starts_with('#') {
                    return None;
                }
                Some(t.split_once('|').map(|(_, p)| p).unwrap_or(t).to_string())
            })
            .collect();
        *self.state.chain.write() = chain;
    }
}

impl Drop for Engine {
    fn drop(&mut self) {
        let _ = self.tx.send(Command::Shutdown);
        if let Some(j) = self.worker.take() {
            let _ = j.join();
        }
    }
}

fn chain_config_path() -> std::path::PathBuf {
    embed::data_dir().join("chain.txt")
}

fn worker_loop(
    rx: Receiver<Command>,
    state: Arc<EngineState>,
    mic_lock: Arc<MicLock>,
    config: ConfigStore,
) {
    let _com = match com_init_mta() {
        Ok(g) => g,
        Err(e) => {
            warn!(error = ?e, "engine: COM init failed; worker exiting");
            return;
        }
    };
    let _mf = soundcore_camera::init::mf_startup_lite().ok();

    // The currently running camera capture session lives in this binding
    // so it can be replaced/stopped from inside the worker.
    let mut camera_session: Option<CaptureSession> = None;

    info!("engine worker started");
    while let Ok(cmd) = rx.recv() {
        match cmd {
            Command::Shutdown => break,
            Command::RefreshAll => {
                refresh_devices(&state);
                refresh_processes(&state);
                refresh_cameras(&state);
            }
            Command::RefreshDevices => refresh_devices(&state),
            Command::RefreshProcesses => refresh_processes(&state),
            Command::RefreshCameras => refresh_cameras(&state),
            Command::RefreshVstPlugins => refresh_vst_plugins(&state),
            Command::SetMicLock(cfg) => apply_mic_lock(&mic_lock, &config, &state, cfg),
            Command::SetCamera(cfg) => apply_camera(&config, &state, &mut camera_session, cfg),
            Command::SaveChain(paths) => save_chain(&state, paths),
        }
    }
    drop(camera_session);
    let _ = mic_lock.stop();
    info!("engine worker stopped");
}

fn refresh_devices(state: &EngineState) {
    let enumerator = match DeviceEnumerator::new() {
        Ok(e) => e,
        Err(e) => {
            *state.last_error.write() = Some(format!("DeviceEnumerator: {e}"));
            return;
        }
    };
    if let Ok(d) = enumerator.list(DataFlow::Render) {
        *state.render_devices.write() = d;
    }
    if let Ok(d) = enumerator.list(DataFlow::Capture) {
        *state.capture_devices.write() = d;
    }
    let _ = enumerator.default(DataFlow::Render, Role::Console);
}

fn refresh_processes(state: &EngineState) {
    match soundcore_audio::process::list_processes() {
        Ok(v) => *state.processes.write() = v,
        Err(e) => *state.last_error.write() = Some(format!("list_processes: {e}")),
    }
}

fn refresh_cameras(state: &EngineState) {
    match soundcore_camera::source::enumerate() {
        Ok(v) => *state.cameras.write() = v,
        Err(e) => *state.last_error.write() = Some(format!("list_cameras: {e}")),
    }
}

fn refresh_vst_plugins(state: &EngineState) {
    let exe = match std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|d| d.join("soundcore-vst-scanner.exe")))
    {
        Some(p) if p.exists() => p,
        _ => {
            *state.last_error.write() =
                Some("VST scanner binary not found next to SoundCore.exe".into());
            return;
        }
    };
    let output = match std::process::Command::new(&exe).output() {
        Ok(o) => o,
        Err(e) => {
            *state.last_error.write() = Some(format!("VST scanner spawn failed: {e}"));
            return;
        }
    };
    if !output.status.success() {
        *state.last_error.write() = Some(format!(
            "VST scanner exited with {:?}: {}",
            output.status.code(),
            String::from_utf8_lossy(&output.stderr)
        ));
        return;
    }
    match serde_json::from_slice::<Vec<VstPluginDescriptor>>(&output.stdout) {
        Ok(v) => {
            info!(count = v.len(), "VST scan complete");
            *state.vst_plugins.write() = v;
        }
        Err(e) => {
            *state.last_error.write() = Some(format!("VST scanner JSON parse: {e}"));
        }
    }
}

fn save_chain(state: &EngineState, paths: Vec<String>) {
    *state.chain.write() = paths.clone();
    let path = chain_config_path();
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let mut body = String::new();
    body.push_str("# SoundCore APO chain. One plugin per line: `<uid>|<path>`.\n");
    body.push_str("# uid is optional; the APO will look the plugin up by path.\n");
    for p in &paths {
        body.push_str("|");
        body.push_str(p);
        body.push('\n');
    }
    match std::fs::write(&path, &body) {
        Ok(_) => *state.last_info.write() = Some(format!("Chain saved → {}", path.display())),
        Err(e) => *state.last_error.write() = Some(format!("write chain.txt: {e}")),
    }
}

fn apply_camera(
    config: &ConfigStore,
    state: &EngineState,
    session: &mut Option<CaptureSession>,
    cfg: CfgCamera,
) {
    let _ = config.with_mut(|c| c.camera = cfg.clone());
    // Always stop the existing one first so we don't double-bind the
    // shared-memory channel.
    *session = None;
    if !cfg.enabled || cfg.source_symbolic_link.is_empty() {
        *state.camera_running.write() = false;
        return;
    }
    let width = if cfg.width == 0 { 1280 } else { cfg.width };
    let height = if cfg.height == 0 { 720 } else { cfg.height };
    let fps_num = if cfg.frame_rate_num == 0 { 30 } else { cfg.frame_rate_num };
    let fps_den = if cfg.frame_rate_den == 0 { 1 } else { cfg.frame_rate_den };
    let s = CaptureSession::start(
        cfg.source_symbolic_link.clone(),
        width,
        height,
        fps_num,
        fps_den,
    );
    *session = Some(s);
    *state.camera_running.write() = true;
    *state.last_info.write() = Some(format!(
        "camera streaming {width}×{height}@{fps_num}/{fps_den} from {}",
        cfg.source_symbolic_link
    ));
}

fn apply_mic_lock(
    mic_lock: &MicLock,
    config: &ConfigStore,
    state: &EngineState,
    cfg: CfgMicLock,
) {
    let _ = config.with_mut(|c| c.mic_lock = cfg.clone());
    let _ = mic_lock.stop();
    if cfg.enabled && !cfg.device_id.is_empty() {
        let lc = LockConfig {
            device_id: cfg.device_id.clone(),
            locked_volume: cfg.locked_volume,
            also_lock_mute: cfg.also_lock_mute,
            revert_immediately: cfg.revert_immediately,
        };
        match mic_lock.start(lc) {
            Ok(_) => {
                *state.mic_lock_running.write() = true;
                info!(device = %cfg.device_id, "mic-lock started");
            }
            Err(e) => {
                *state.last_error.write() = Some(format!("mic-lock: {e}"));
                *state.mic_lock_running.write() = false;
            }
        }
    } else {
        *state.mic_lock_running.write() = false;
    }
}
