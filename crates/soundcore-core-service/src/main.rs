//! SoundCore — single-exe entry point.
//!
//! Default launch (no args): elevates via the embedded manifest, drops
//! the embedded DLLs into `%ProgramData%\SoundCore\bin\`, registers
//! them, starts the audio engine in-process, opens the GUI.
//!
//! Other modes are kept for power-users and the optional Windows Service
//! deployment:
//!   * `--service`        — run as a Windows Service (no GUI).
//!   * `install`          — register the service for auto-start.
//!   * `uninstall`        — remove the service.
//!   * `--register`       — extract & DllRegisterServer the embedded DLLs
//!                          without launching the GUI.
//!   * `--unregister`     — undo `--register`.
//!   * `--console`        — run the service run-loop in the console.

// No console window on double-click. CLI/service modes call
// AttachConsole(ATTACH_PARENT_PROCESS) so `println!` works from cmd.exe.
#![windows_subsystem = "windows"]

use std::ffi::OsString;
use std::time::Duration;

use anyhow::Context;
use tracing_subscriber::{prelude::*, EnvFilter};
use windows_service::{
    define_windows_service,
    service::{
        ServiceAccess, ServiceControl, ServiceControlAccept, ServiceErrorControl, ServiceExitCode,
        ServiceInfo, ServiceStartType, ServiceState, ServiceStatus, ServiceType,
    },
    service_control_handler::{self, ServiceControlHandlerResult},
    service_dispatcher,
    service_manager::{ServiceManager, ServiceManagerAccess},
};

mod audio_ops;
mod config;
mod embed;
mod engine;
mod gui;
mod i18n;
mod runtime;
mod server;
mod service_mgmt;

const SERVICE_NAME: &str = service_mgmt::SERVICE_NAME;
const SERVICE_TYPE: ServiceType = ServiceType::OWN_PROCESS;

define_windows_service!(ffi_service_main, service_main);

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        None => {
            if let Err(e) = run_gui() {
                fatal_dialog("SoundCore failed to start", &format!("{e:#}"));
            }
        }
        Some("--service") | Some("/service") => {
            attach_console();
            if let Err(e) = service_dispatcher::start(SERVICE_NAME, ffi_service_main) {
                eprintln!("not started by SCM, running console run-loop: {e:?}");
                let _ = run_console();
            }
        }
        Some("install") => {
            attach_console();
            match install_service() {
                Ok(_) => println!("SoundCore service installed."),
                Err(e) => eprintln!("install failed: {e:#}"),
            }
        }
        Some("uninstall") => {
            attach_console();
            match uninstall_service() {
                Ok(_) => println!("SoundCore service uninstalled."),
                Err(e) => eprintln!("uninstall failed: {e:#}"),
            }
        }
        Some("--register") => {
            attach_console();
            print_registration(embed::extract_and_register_all());
        }
        Some("--unregister") => {
            attach_console();
            print_registration(embed::unregister_all());
        }
        Some("--console") | Some("-c") => {
            attach_console();
            let _ = run_console();
        }
        Some(other) => {
            attach_console();
            eprintln!("unknown command: {other}");
            eprintln!(
                "usage: SoundCore [install|uninstall|--service|--register|--unregister|--console]"
            );
            std::process::exit(2);
        }
    }
}

fn print_registration(r: std::io::Result<Vec<embed::RegistrationOutcome>>) {
    match r {
        Ok(list) => {
            for o in list {
                let mark = if o.ok { "OK" } else { "!!" };
                println!("[{mark}] {}: {}", o.label, o.message);
            }
        }
        Err(e) => eprintln!("registration error: {e:#}"),
    }
}

// ============================================================================
//  GUI mode (single-exe default)
// ============================================================================

fn run_gui() -> anyhow::Result<()> {
    init_file_tracing()?;
    tracing::info!("SoundCore starting in GUI mode");

    // 1) Extract & register embedded DLLs. We intentionally don't fail
    //    the whole launch if registration fails — the GUI still opens
    //    and the Setup tab lets the user retry.
    match embed::extract_and_register_all() {
        Ok(outcomes) => {
            for o in outcomes {
                if o.ok {
                    tracing::info!(component = %o.label, "{}", o.message);
                } else {
                    tracing::warn!(component = %o.label, "{}", o.message);
                }
            }
        }
        Err(e) => tracing::warn!(error = ?e, "DLL extraction/registration failed"),
    }

    // 2) Start the in-process audio engine + load persisted config.
    let cfg = config::ConfigStore::load_or_default();
    let engine = engine::Engine::start(cfg);

    // 3) Run egui on the main thread.
    let options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_inner_size([960.0, 640.0])
            .with_min_inner_size([720.0, 480.0])
            .with_title("SoundCore"),
        ..Default::default()
    };
    eframe::run_native(
        "SoundCore",
        options,
        Box::new(|cc| {
            // egui's default fonts don't cover Cyrillic/CJK; pull in Segoe UI
            // and YaHei so the localized UI renders instead of showing tofu.
            i18n::install_system_fonts(&cc.egui_ctx);
            Box::new(gui::SoundCoreApp::new(engine))
        }),
    )
    .map_err(|e| anyhow::anyhow!("eframe::run_native failed: {e}"))
}

// ============================================================================
//  Windows Service / console run-loop (optional)
// ============================================================================

fn run_console() -> anyhow::Result<()> {
    init_console_tracing();
    tracing::info!("SoundCore running in console (--service fallback) mode");
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .thread_name("sc-core")
        .build()?;
    runtime.block_on(async move {
        let token = tokio_util::sync::CancellationToken::new();
        let ctrl = token.clone();
        tokio::spawn(async move {
            let _ = tokio::signal::ctrl_c().await;
            ctrl.cancel();
        });
        runtime::run(token).await
    })
}

fn service_main(_args: Vec<OsString>) {
    let _ = run_service();
}

fn run_service() -> anyhow::Result<()> {
    let (status_handle, mut shutdown_rx) = {
        let (tx, rx) = tokio::sync::oneshot::channel::<()>();
        let mut tx_opt = Some(tx);
        let handler = move |control_event| -> ServiceControlHandlerResult {
            match control_event {
                ServiceControl::Stop | ServiceControl::Shutdown => {
                    if let Some(t) = tx_opt.take() {
                        let _ = t.send(());
                    }
                    ServiceControlHandlerResult::NoError
                }
                ServiceControl::Interrogate => ServiceControlHandlerResult::NoError,
                _ => ServiceControlHandlerResult::NotImplemented,
            }
        };
        let h = service_control_handler::register(SERVICE_NAME, handler)
            .context("register service control handler")?;
        (h, rx)
    };

    status_handle.set_service_status(ServiceStatus {
        service_type: SERVICE_TYPE,
        current_state: ServiceState::Running,
        controls_accepted: ServiceControlAccept::STOP | ServiceControlAccept::SHUTDOWN,
        exit_code: ServiceExitCode::Win32(0),
        checkpoint: 0,
        wait_hint: Duration::from_secs(0),
        process_id: None,
    })?;

    init_file_tracing()?;
    tracing::info!("SoundCore service started");

    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .thread_name("sc-core")
        .build()?;

    let token = tokio_util::sync::CancellationToken::new();
    let cancel = token.clone();
    runtime.spawn(async move {
        let _ = (&mut shutdown_rx).await;
        cancel.cancel();
    });

    let result = runtime.block_on(runtime::run(token));

    status_handle.set_service_status(ServiceStatus {
        service_type: SERVICE_TYPE,
        current_state: ServiceState::Stopped,
        controls_accepted: ServiceControlAccept::empty(),
        exit_code: ServiceExitCode::Win32(0),
        checkpoint: 0,
        wait_hint: Duration::from_secs(0),
        process_id: None,
    })?;
    if let Err(e) = &result {
        tracing::error!(error = ?e, "service run loop exited with error");
    }
    result
}

fn install_service() -> anyhow::Result<()> {
    service_mgmt::install_and_start().map_err(|e| anyhow::anyhow!(e))?;
    Ok(())
}

fn uninstall_service() -> anyhow::Result<()> {
    service_mgmt::stop_and_uninstall().map_err(|e| anyhow::anyhow!(e))?;
    Ok(())
}

// ============================================================================
//  Logging
// ============================================================================

fn init_file_tracing() -> anyhow::Result<()> {
    let dir = embed::data_dir().join("logs");
    std::fs::create_dir_all(&dir).ok();
    let file_appender = tracing_appender::rolling::daily(&dir, "soundcore.log");
    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("info,soundcore_=debug"));
    let _ = tracing_subscriber::registry()
        .with(tracing_subscriber::fmt::layer().with_writer(file_appender))
        .with(filter)
        .try_init();
    Ok(())
}

fn init_console_tracing() {
    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("info,soundcore_=debug"));
    let _ = tracing_subscriber::registry()
        .with(tracing_subscriber::fmt::layer().with_target(true))
        .with(filter)
        .try_init();
}

// ============================================================================
//  Misc
// ============================================================================

/// Re-attach to the parent console (cmd.exe) so `println!` works when
/// the binary was launched from a CLI. No-op otherwise.
fn attach_console() {
    use windows::Win32::System::Console::{AttachConsole, ATTACH_PARENT_PROCESS};
    unsafe {
        let _ = AttachConsole(ATTACH_PARENT_PROCESS);
    }
}

fn fatal_dialog(title: &str, body: &str) {
    use windows::Win32::UI::WindowsAndMessaging::{MessageBoxW, MB_ICONERROR, MB_OK};
    let wtitle: Vec<u16> = title.encode_utf16().chain([0]).collect();
    let wbody: Vec<u16> = body.encode_utf16().chain([0]).collect();
    unsafe {
        MessageBoxW(
            None,
            windows::core::PCWSTR(wbody.as_ptr()),
            windows::core::PCWSTR(wtitle.as_ptr()),
            MB_OK | MB_ICONERROR,
        );
    }
}
