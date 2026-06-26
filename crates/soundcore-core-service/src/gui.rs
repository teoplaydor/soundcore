//! egui front-end. Runs on the main thread; talks to [`Engine`] via
//! command sender + shared `Arc<EngineState>`.

use std::sync::Arc;
use std::time::{Duration, Instant};

use eframe::egui;
use parking_lot::Mutex;

use crate::config::MicLockConfig as CfgMicLock;
use crate::embed::{self, RegistrationOutcome};
use crate::engine::{Command, Engine, EngineState};
use crate::service_mgmt::{self, ServiceStatusKind};

#[derive(Default, Clone, Copy, PartialEq, Eq)]
enum Tab {
    #[default]
    Devices,
    PerApp,
    MicLock,
    Camera,
    Vst,
    Setup,
}

pub struct SoundCoreApp {
    engine: Engine,
    tab: Tab,
    process_filter: String,
    mic_volume_pct: f32,
    mic_enabled: bool,
    mic_lock_mute: bool,
    mic_device_id: String,
    mic_whitelist_text: String,
    registration_status: Arc<Mutex<Vec<RegistrationOutcome>>>,
    registration_busy: bool,
    last_refresh: Instant,
    service_status: ServiceStatusKind,
    service_last_status_poll: Instant,
    service_op_message: Arc<Mutex<Option<String>>>,
    service_op_busy: bool,
    camera_enabled: bool,
    camera_source: String,
    camera_width: u32,
    camera_height: u32,
    camera_fps: u32,
}

impl SoundCoreApp {
    pub fn new(engine: Engine) -> Self {
        let snap = engine.config.snapshot();
        let mic_volume_pct = snap.mic_lock.locked_volume.unwrap_or(1.0).clamp(0.0, 1.0) * 100.0;
        let whitelist = snap.mic_lock.allowed_image_globs.join("\n");
        Self {
            engine,
            tab: Tab::Devices,
            process_filter: String::new(),
            mic_volume_pct,
            mic_enabled: snap.mic_lock.enabled,
            mic_lock_mute: snap.mic_lock.also_lock_mute,
            mic_device_id: snap.mic_lock.device_id.clone(),
            mic_whitelist_text: whitelist,
            registration_status: Arc::new(Mutex::new(Vec::new())),
            registration_busy: false,
            last_refresh: Instant::now(),
            service_status: service_mgmt::query_status(),
            service_last_status_poll: Instant::now(),
            service_op_message: Arc::new(Mutex::new(None)),
            service_op_busy: false,
            camera_enabled: snap.camera.enabled,
            camera_source: snap.camera.source_symbolic_link.clone(),
            camera_width: if snap.camera.width == 0 { 1280 } else { snap.camera.width },
            camera_height: if snap.camera.height == 0 { 720 } else { snap.camera.height },
            camera_fps: if snap.camera.frame_rate_num == 0 { 30 } else { snap.camera.frame_rate_num },
        }
    }
}

impl eframe::App for SoundCoreApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        if self.last_refresh.elapsed() > Duration::from_secs(2) {
            self.engine.cmd(Command::RefreshProcesses);
            self.last_refresh = Instant::now();
        }
        if self.service_last_status_poll.elapsed() > Duration::from_secs(2) {
            self.service_status = service_mgmt::query_status();
            self.service_last_status_poll = Instant::now();
        }
        ctx.request_repaint_after(Duration::from_millis(400));

        let state = self.engine.state.clone();

        egui::TopBottomPanel::top("titlebar").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.heading("SoundCore");
                ui.separator();
                if ui.button("Refresh").clicked() {
                    self.engine.cmd(Command::RefreshAll);
                }
                ui.separator();
                if *state.mic_lock_running.read() {
                    ui.colored_label(egui::Color32::LIGHT_GREEN, "🔒 mic-lock active");
                }
                let svc_color = match self.service_status {
                    ServiceStatusKind::Running => egui::Color32::LIGHT_GREEN,
                    ServiceStatusKind::Stopped => egui::Color32::YELLOW,
                    ServiceStatusKind::NotInstalled => egui::Color32::GRAY,
                    _ => egui::Color32::LIGHT_GRAY,
                };
                ui.colored_label(
                    svc_color,
                    format!("service: {}", self.service_status.label()),
                );
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if let Some(err) = state.last_error.read().clone() {
                        ui.colored_label(egui::Color32::LIGHT_RED, err);
                    }
                });
            });
        });

        egui::SidePanel::left("sidebar")
            .resizable(false)
            .min_width(160.0)
            .show(ctx, |ui| {
                ui.add_space(8.0);
                ui.selectable_value(&mut self.tab, Tab::Devices, "🔊  Devices");
                ui.selectable_value(&mut self.tab, Tab::PerApp, "📋  Per-App");
                ui.selectable_value(&mut self.tab, Tab::MicLock, "🎤  Mic lock");
                ui.selectable_value(&mut self.tab, Tab::Camera, "🎥  Camera");
                ui.selectable_value(&mut self.tab, Tab::Vst, "🎛  VST");
                ui.separator();
                ui.selectable_value(&mut self.tab, Tab::Setup, "⚙  Setup");
            });

        egui::CentralPanel::default().show(ctx, |ui| match self.tab {
            Tab::Devices => self.devices_tab(ui, &state),
            Tab::PerApp => self.perapp_tab(ui, &state),
            Tab::MicLock => self.miclock_tab(ui, &state),
            Tab::Camera => self.camera_tab(ui, &state),
            Tab::Vst => self.vst_tab(ui, &state),
            Tab::Setup => self.setup_tab(ui),
        });
    }
}

impl SoundCoreApp {
    fn devices_tab(&mut self, ui: &mut egui::Ui, state: &EngineState) {
        ui.heading("Audio devices");
        ui.add_space(8.0);
        let render = state.render_devices.read().clone();
        let capture = state.capture_devices.read().clone();
        egui::CollapsingHeader::new(format!("Playback ({})", render.len()))
            .default_open(true)
            .show(ui, |ui| device_table(ui, &render));
        ui.add_space(8.0);
        egui::CollapsingHeader::new(format!("Capture ({})", capture.len()))
            .default_open(true)
            .show(ui, |ui| device_table(ui, &capture));
    }

    fn perapp_tab(&mut self, ui: &mut egui::Ui, state: &EngineState) {
        ui.heading("Running processes");
        info_banner(
            ui,
            "Per-application FX (привязка VST-цепочки к процессу) — пока не реализовано. \
             Требуется Process Loopback API + готовый VST scanner. См. Setup → Feature status.",
        );
        ui.add_space(8.0);
        ui.horizontal(|ui| {
            ui.label("Filter:");
            ui.text_edit_singleline(&mut self.process_filter);
            if ui.button("Refresh").clicked() {
                self.engine.cmd(Command::RefreshProcesses);
            }
        });
        ui.separator();
        let procs = state.processes.read().clone();
        let q = self.process_filter.to_lowercase();
        let filtered: Vec<_> = procs
            .iter()
            .filter(|p| q.is_empty() || p.image_name.to_lowercase().contains(&q))
            .collect();
        egui::ScrollArea::vertical().show(ui, |ui| {
            egui::Grid::new("procs")
                .num_columns(3)
                .striped(true)
                .show(ui, |ui| {
                    ui.strong("PID");
                    ui.strong("Image");
                    ui.strong("Path");
                    ui.end_row();
                    for p in filtered {
                        ui.monospace(format!("{}", p.pid));
                        ui.label(&p.image_name);
                        ui.label(&p.image_path).on_hover_text(&p.image_path);
                        ui.end_row();
                    }
                });
        });
    }

    fn miclock_tab(&mut self, ui: &mut egui::Ui, state: &EngineState) {
        ui.heading("Microphone volume lock");
        ui.label(
            "Запрещает любому приложению (AutoGain в Chrome / Google Meet) менять \
             громкость микрофона. Worker-thread на каждом тике 15 ms сравнивает \
             current с target и выставляет target — устойчиво к спаму одного значения.",
        );
        if self.service_status == ServiceStatusKind::Running {
            ok_banner(
                ui,
                "Служба SoundCore запущена → mic-lock работает в фоне даже когда GUI закрыт.",
            );
        } else {
            info_banner(
                ui,
                "Если хотите чтобы блокировка работала без открытого приложения — \
                 поставьте службу в Setup → Background service. Один клик.",
            );
        }
        ui.add_space(8.0);

        ui.checkbox(&mut self.mic_enabled, "Enabled");

        let capture = state.capture_devices.read().clone();
        egui::ComboBox::from_label("Capture device")
            .selected_text(
                capture
                    .iter()
                    .find(|d| d.id == self.mic_device_id)
                    .map(|d| d.friendly_name.clone())
                    .unwrap_or_else(|| "— pick a device —".to_string()),
            )
            .show_ui(ui, |ui| {
                for d in &capture {
                    let label = if d.is_default {
                        format!("{} (default)", d.friendly_name)
                    } else {
                        d.friendly_name.clone()
                    };
                    ui.selectable_value(&mut self.mic_device_id, d.id.clone(), label);
                }
            });

        ui.add_space(4.0);
        ui.add(egui::Slider::new(&mut self.mic_volume_pct, 0.0..=100.0).text("Locked volume %"));
        ui.checkbox(&mut self.mic_lock_mute, "Also lock mute state");

        ui.add_space(8.0);
        ui.label("Whitelist (one image name per line; e.g. OBS64.exe, Streamlabs*.exe):");
        ui.add(
            egui::TextEdit::multiline(&mut self.mic_whitelist_text)
                .desired_rows(4)
                .desired_width(f32::INFINITY),
        );

        ui.add_space(8.0);
        let disable_apply = self.mic_enabled && self.mic_device_id.is_empty();
        ui.add_enabled_ui(!disable_apply, |ui| {
            if ui.button("Apply").clicked() {
                let cfg = CfgMicLock {
                    enabled: self.mic_enabled,
                    device_id: self.mic_device_id.clone(),
                    locked_volume: Some((self.mic_volume_pct / 100.0).clamp(0.0, 1.0)),
                    also_lock_mute: self.mic_lock_mute,
                    revert_immediately: true,
                    allowed_image_globs: self
                        .mic_whitelist_text
                        .lines()
                        .map(|s| s.trim().to_string())
                        .filter(|s| !s.is_empty())
                        .collect(),
                };
                self.engine.cmd(Command::SetMicLock(cfg));
            }
        });

        if disable_apply {
            ui.colored_label(egui::Color32::YELLOW, "Pick a device first.");
        }
    }

    fn camera_tab(&mut self, ui: &mut egui::Ui, state: &EngineState) {
        ui.heading("Virtual camera multiplex");
        if *state.camera_running.read() {
            ok_banner(
                ui,
                "Camera producer запущен. Кадры публикуются в shared-memory канал \
                 `Global\\SoundCore.Camera.0` — DirectShow/MF consumers (виртуальная \
                 камера, которую видят другие приложения) читают оттуда.",
            );
        } else {
            info_banner(
                ui,
                "Выберите реальную камеру ниже и нажмите Apply. SoundCore откроет её \
                 один раз через Media Foundation и будет раздавать кадры всем \
                 приложениям, которые используют SoundCore Virtual Camera.",
            );
        }
        ui.add_space(8.0);

        let snap = self.engine.config.snapshot().camera;
        let cams = state.cameras.read().clone();

        ui.checkbox(&mut self.camera_enabled, "Enable virtual camera producer");
        egui::ComboBox::from_label("Real camera source")
            .selected_text(
                cams.iter()
                    .find(|c| c.symbolic_link == self.camera_source)
                    .map(|c| c.friendly_name.clone())
                    .unwrap_or_else(|| "— pick a camera —".into()),
            )
            .show_ui(ui, |ui| {
                for c in &cams {
                    ui.selectable_value(
                        &mut self.camera_source,
                        c.symbolic_link.clone(),
                        &c.friendly_name,
                    );
                }
            });

        egui::ComboBox::from_label("Resolution / fps")
            .selected_text(format!(
                "{}×{} @ {}fps",
                self.camera_width, self.camera_height, self.camera_fps
            ))
            .show_ui(ui, |ui| {
                for (w, h, f) in [
                    (1920u32, 1080u32, 30u32),
                    (1280, 720, 30),
                    (640, 480, 30),
                ] {
                    let label = format!("{w}×{h} @ {f}fps");
                    if ui
                        .selectable_label(
                            self.camera_width == w
                                && self.camera_height == h
                                && self.camera_fps == f,
                            label,
                        )
                        .clicked()
                    {
                        self.camera_width = w;
                        self.camera_height = h;
                        self.camera_fps = f;
                    }
                }
            });

        ui.horizontal(|ui| {
            if ui.button("Refresh camera list").clicked() {
                self.engine.cmd(Command::RefreshCameras);
            }
            let disabled = self.camera_enabled && self.camera_source.is_empty();
            ui.add_enabled_ui(!disabled, |ui| {
                if ui.button("Apply").clicked() {
                    let cfg = crate::config::CameraConfig {
                        enabled: self.camera_enabled,
                        source_symbolic_link: self.camera_source.clone(),
                        width: self.camera_width,
                        height: self.camera_height,
                        frame_rate_num: self.camera_fps,
                        frame_rate_den: 1,
                    };
                    self.engine.cmd(Command::SetCamera(cfg));
                }
            });
        });

        if let Some(msg) = state.last_info.read().clone() {
            ui.colored_label(egui::Color32::LIGHT_GREEN, msg);
        }
        if snap.enabled && !snap.source_symbolic_link.is_empty() {
            ui.label(format!(
                "Persisted: {} @ {}×{}",
                snap.source_symbolic_link, snap.width, snap.height
            ));
        }

        ui.separator();
        ui.label(format!("{} camera(s) on system:", cams.len()));
        egui::ScrollArea::vertical().show(ui, |ui| {
            for c in &cams {
                ui.group(|ui| {
                    ui.strong(&c.friendly_name);
                    ui.monospace(&c.symbolic_link);
                });
            }
        });
    }

    fn vst_tab(&mut self, ui: &mut egui::Ui, state: &EngineState) {
        ui.heading("VST3 plugins & effect chain");
        info_banner(
            ui,
            "Сканер ищет .vst3 в %CommonProgramFiles%\\VST3, \
             %CommonProgramFiles(x86)%\\VST3, %ProgramFiles%\\VSTPlugins. \
             Метаданные плагина (имя/вендор/UID) подтягиваются APO при первой \
             реальной загрузке.",
        );
        ui.add_space(8.0);
        ui.horizontal(|ui| {
            if ui.button("Rescan").clicked() {
                self.engine.cmd(Command::RefreshVstPlugins);
            }
            let chain_len = state.chain.read().len();
            ui.label(format!("Chain: {chain_len} plugin(s)"));
            if ui.button("Save chain").on_hover_text("writes chain.txt").clicked() {
                let paths = state.chain.read().clone();
                self.engine.cmd(Command::SaveChain(paths));
            }
            if ui.button("Clear chain").clicked() {
                state.chain.write().clear();
            }
        });

        if let Some(msg) = state.last_info.read().clone() {
            ui.colored_label(egui::Color32::LIGHT_GREEN, msg);
        }
        ui.separator();

        ui.columns(2, |cols| {
            cols[0].label(egui::RichText::new("Discovered plugins").strong());
            let plugins = state.vst_plugins.read().clone();
            egui::ScrollArea::vertical().id_source("disc").show(&mut cols[0], |ui| {
                if plugins.is_empty() {
                    ui.label("(нажмите Rescan)");
                }
                for p in &plugins {
                    ui.horizontal(|ui| {
                        if ui.button("+").on_hover_text("Add to chain").clicked() {
                            let mut chain = state.chain.write();
                            if !chain.iter().any(|c| c == &p.path) {
                                chain.push(p.path.clone());
                            }
                        }
                        ui.label(&p.name).on_hover_text(&p.path);
                    });
                }
            });

            cols[1].label(egui::RichText::new("Active chain (top → bottom)").strong());
            let chain_snapshot = state.chain.read().clone();
            let mut to_remove: Option<usize> = None;
            let mut to_move_up: Option<usize> = None;
            let mut to_move_down: Option<usize> = None;
            egui::ScrollArea::vertical().id_source("chain").show(&mut cols[1], |ui| {
                if chain_snapshot.is_empty() {
                    ui.label("(chain пуст)");
                }
                for (i, path) in chain_snapshot.iter().enumerate() {
                    ui.horizontal(|ui| {
                        let name = std::path::Path::new(path)
                            .file_stem()
                            .and_then(|s| s.to_str())
                            .unwrap_or("?");
                        if ui.small_button("✕").on_hover_text("Remove").clicked() {
                            to_remove = Some(i);
                        }
                        if i > 0 && ui.small_button("↑").clicked() {
                            to_move_up = Some(i);
                        }
                        if i + 1 < chain_snapshot.len() && ui.small_button("↓").clicked() {
                            to_move_down = Some(i);
                        }
                        ui.label(name).on_hover_text(path);
                    });
                }
            });
            let mut chain = state.chain.write();
            if let Some(i) = to_remove { if i < chain.len() { chain.remove(i); } }
            if let Some(i) = to_move_up { if i > 0 && i < chain.len() { chain.swap(i, i - 1); } }
            if let Some(i) = to_move_down { if i + 1 < chain.len() { chain.swap(i, i + 1); } }
        });

        ui.add_space(8.0);
        warn_banner(
            ui,
            "После «Save chain» отключите/включите устройство в настройках звука \
             Windows, чтобы audiodg.exe перечитал chain.txt и APO загрузил плагины.",
        );
    }

    fn setup_tab(&mut self, ui: &mut egui::Ui) {
        ui.heading("Setup");
        ui.add_space(8.0);

        // ----- Background service -----
        egui::CollapsingHeader::new("Background service (mic-lock без открытого GUI)")
            .default_open(true)
            .show(ui, |ui| {
                ui.label(
                    "Установите службу — и mic-lock будет работать в фоне даже после \
                     закрытия GUI и переживёт перезагрузку. Это самый близкий путь \
                     к «Windows не сможет менять громкость» без своего \
                     kernel-драйвера.",
                );
                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    ui.label("Status:");
                    let color = match self.service_status {
                        ServiceStatusKind::Running => egui::Color32::LIGHT_GREEN,
                        ServiceStatusKind::Stopped => egui::Color32::YELLOW,
                        ServiceStatusKind::NotInstalled => egui::Color32::GRAY,
                        _ => egui::Color32::LIGHT_GRAY,
                    };
                    ui.colored_label(color, self.service_status.label());
                });
                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    let busy = self.service_op_busy;
                    match self.service_status {
                        ServiceStatusKind::NotInstalled => {
                            if ui
                                .add_enabled(
                                    !busy,
                                    egui::Button::new("Install as Windows Service & start"),
                                )
                                .clicked()
                            {
                                self.run_service_op(|| service_mgmt::install_and_start());
                            }
                        }
                        ServiceStatusKind::Stopped => {
                            if ui
                                .add_enabled(!busy, egui::Button::new("Start service"))
                                .clicked()
                            {
                                self.run_service_op(|| service_mgmt::start());
                            }
                            if ui
                                .add_enabled(!busy, egui::Button::new("Uninstall service"))
                                .clicked()
                            {
                                self.run_service_op(|| service_mgmt::stop_and_uninstall());
                            }
                        }
                        ServiceStatusKind::Running => {
                            if ui
                                .add_enabled(!busy, egui::Button::new("Stop service"))
                                .clicked()
                            {
                                self.run_service_op(|| service_mgmt::stop());
                            }
                            if ui
                                .add_enabled(!busy, egui::Button::new("Uninstall service"))
                                .clicked()
                            {
                                self.run_service_op(|| service_mgmt::stop_and_uninstall());
                            }
                        }
                        _ => {
                            ui.label("(transitioning…)");
                        }
                    }
                });
                if let Some(msg) = self.service_op_message.lock().clone() {
                    ui.add_space(4.0);
                    ui.colored_label(egui::Color32::LIGHT_BLUE, msg);
                }
            });

        // ----- COM DLL registration -----
        egui::CollapsingHeader::new("Embedded DLLs (APO + Virtual Camera)")
            .default_open(false)
            .show(ui, |ui| {
                ui.label(
                    "Эти DLL встроены в .exe и автоматически распакованы + \
                     зарегистрированы при первом запуске. Кнопки — на случай \
                     ручной переустановки или диагностики.",
                );
                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    let busy = self.registration_busy;
                    if ui
                        .add_enabled(!busy, egui::Button::new("Re-register"))
                        .clicked()
                    {
                        self.registration_busy = true;
                        let target = self.registration_status.clone();
                        std::thread::spawn(move || {
                            let r = embed::extract_and_register_all().unwrap_or_else(|e| {
                                vec![RegistrationOutcome {
                                    label: "extract".into(),
                                    ok: false,
                                    message: e.to_string(),
                                }]
                            });
                            *target.lock() = r;
                        });
                    }
                    if ui
                        .add_enabled(!busy, egui::Button::new("Unregister"))
                        .clicked()
                    {
                        self.registration_busy = true;
                        let target = self.registration_status.clone();
                        std::thread::spawn(move || {
                            let r = embed::unregister_all().unwrap_or_else(|e| {
                                vec![RegistrationOutcome {
                                    label: "unregister".into(),
                                    ok: false,
                                    message: e.to_string(),
                                }]
                            });
                            *target.lock() = r;
                        });
                    }
                });
                let status = self.registration_status.lock().clone();
                if !status.is_empty() {
                    self.registration_busy = false;
                    ui.add_space(4.0);
                    for r in status {
                        let color = if r.ok {
                            egui::Color32::LIGHT_GREEN
                        } else {
                            egui::Color32::LIGHT_RED
                        };
                        ui.colored_label(color, format!("{}: {}", r.label, r.message));
                    }
                }
            });

        // ----- AudioPolicyConfig probe -----
        egui::CollapsingHeader::new("AudioPolicyConfig probe (per-app routing)")
            .default_open(false)
            .show(ui, |ui| {
                ui.label(
                    "Тест undocumented IAudioPolicyConfigFactory: пробует открыть \
                     COM-объект через Win10 и Win11 CLSID и вызвать \
                     SetPersistedDefaultAudioEndpoint(self.pid, Render, Console, \"\"). \
                     Если получилось — per-app routing на этой машине реализуем.",
                );
                if ui.button("Run probe").clicked() {
                    let msg = match soundcore_audio::policy_config::PolicyConfig::open() {
                        Ok(pc) => match pc.set_persisted_default(
                            std::process::id(),
                            soundcore_audio::DataFlow::Render,
                            soundcore_audio::policy_config::AppRole::Console,
                            "",
                        ) {
                            Ok(_) => "PolicyConfig opened AND SetPersistedDefault \
                                      succeeded — per-app routing is callable on this OS."
                                .to_string(),
                            Err(e) => format!(
                                "PolicyConfig opened but SetPersistedDefault failed: {e}. \
                                 Probably the vtable slot heuristic is wrong on this build."
                            ),
                        },
                        Err(e) => format!("PolicyConfig::open failed: {e}"),
                    };
                    *self.engine.state.last_info.write() = Some(msg);
                }
                if let Some(msg) = self.engine.state.last_info.read().clone() {
                    ui.colored_label(egui::Color32::LIGHT_BLUE, msg);
                }
            });

        // ----- Feature status (honest) -----
        egui::CollapsingHeader::new("Feature status (что реально работает)")
            .default_open(true)
            .show(ui, |ui| {
                feature_row(ui, FeatureState::Ok, "WASAPI device enumeration");
                feature_row(ui, FeatureState::Ok, "Process enumeration (ToolHelp32)");
                feature_row(ui, FeatureState::Ok, "Camera enumeration (MediaFoundation)");
                feature_row(
                    ui,
                    FeatureState::Ok,
                    "Mic-volume lock (15 ms enforcement loop)",
                );
                feature_row(
                    ui,
                    FeatureState::Ok,
                    "COM auto-registration на первом запуске",
                );
                feature_row(ui, FeatureState::Ok, "Persisted config (config.json)");
                feature_row(ui, FeatureState::Ok, "Windows-Service mode");
                feature_row(
                    ui,
                    FeatureState::Ok,
                    "VST scanner (soundcore-vst-scanner.exe) — enumerates .vst3 files",
                );
                feature_row(
                    ui,
                    FeatureState::Ok,
                    "APO chain wiring — APO reads chain.txt + loads VST3 plugins via JUCE",
                );
                feature_row(
                    ui,
                    FeatureState::Ok,
                    "Camera producer — MF SourceReader → shared memory ring",
                );
                feature_row(
                    ui,
                    FeatureState::Ok,
                    "AudioPolicyConfig (per-app routing) bindings + probe button",
                );
                feature_row(
                    ui,
                    FeatureState::Stub,
                    "Process Loopback per-PID capture — нужен COM completion-handler",
                );
                feature_row(
                    ui,
                    FeatureState::Stub,
                    "Per-app FX chain UI — есть бинды, нет редактора процесс→цепочка",
                );
            });

        ui.add_space(12.0);
        ui.label(format!("Install dir: {}", embed::install_dir().display()));
        ui.label(format!("Config dir : {}", embed::data_dir().display()));
        ui.label(format!(
            "Logs       : {}",
            embed::data_dir().join("logs").display()
        ));
    }

    fn run_service_op<F>(&mut self, op: F)
    where
        F: FnOnce() -> Result<String, String> + Send + 'static,
    {
        self.service_op_busy = true;
        let msg_slot = self.service_op_message.clone();
        std::thread::spawn(move || {
            let r = match op() {
                Ok(s) => s,
                Err(e) => format!("Error: {e}"),
            };
            *msg_slot.lock() = Some(r);
        });
        // The next status poll (~2 s) will refresh the indicator.
        self.service_op_busy = false;
    }
}

// ---------------------------------------------------------------------------

enum FeatureState {
    Ok,
    Stub,
}

fn feature_row(ui: &mut egui::Ui, state: FeatureState, text: &str) {
    let (mark, color) = match state {
        FeatureState::Ok => ("✓", egui::Color32::LIGHT_GREEN),
        FeatureState::Stub => ("◌", egui::Color32::YELLOW),
    };
    ui.horizontal(|ui| {
        ui.colored_label(color, mark);
        ui.label(text);
    });
}

fn info_banner(ui: &mut egui::Ui, text: &str) {
    egui::Frame::none()
        .fill(egui::Color32::from_rgba_unmultiplied(40, 70, 120, 50))
        .stroke(egui::Stroke::new(1.0, egui::Color32::from_rgb(80, 120, 180)))
        .inner_margin(8.0)
        .rounding(4.0)
        .show(ui, |ui| {
            ui.label(text);
        });
}

fn warn_banner(ui: &mut egui::Ui, text: &str) {
    egui::Frame::none()
        .fill(egui::Color32::from_rgba_unmultiplied(140, 90, 30, 60))
        .stroke(egui::Stroke::new(1.0, egui::Color32::from_rgb(200, 140, 60)))
        .inner_margin(8.0)
        .rounding(4.0)
        .show(ui, |ui| {
            ui.label(text);
        });
}

fn ok_banner(ui: &mut egui::Ui, text: &str) {
    egui::Frame::none()
        .fill(egui::Color32::from_rgba_unmultiplied(40, 100, 60, 60))
        .stroke(egui::Stroke::new(1.0, egui::Color32::from_rgb(80, 160, 100)))
        .inner_margin(8.0)
        .rounding(4.0)
        .show(ui, |ui| {
            ui.label(text);
        });
}

fn device_table(ui: &mut egui::Ui, devices: &[soundcore_audio::AudioDevice]) {
    if devices.is_empty() {
        ui.label("(no devices)");
        return;
    }
    egui::Grid::new(format!("dev-grid-{:p}", devices.as_ptr()))
        .num_columns(5)
        .striped(true)
        .show(ui, |ui| {
            ui.strong("Name");
            ui.strong("Sample rate");
            ui.strong("Channels");
            ui.strong("Volume");
            ui.strong("Flags");
            ui.end_row();
            for d in devices {
                ui.label(&d.friendly_name);
                ui.monospace(format!("{} Hz", d.sample_rate));
                ui.monospace(format!("{}", d.channel_count));
                ui.monospace(format!("{:.0}%", d.master_volume * 100.0));
                let mut flags = String::new();
                if d.is_default {
                    flags.push_str("default ");
                }
                if d.is_default_communications {
                    flags.push_str("comms ");
                }
                if d.mute {
                    flags.push_str("muted ");
                }
                ui.label(flags);
                ui.end_row();
            }
        });
}
