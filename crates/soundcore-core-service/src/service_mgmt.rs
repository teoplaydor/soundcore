//! Windows-Service management for the GUI Setup tab.
//!
//! Wraps `windows-service` so the egui code stays free of unsafe SCM
//! plumbing. Every operation is synchronous and returns a string that
//! the GUI displays verbatim.

use std::ffi::OsString;
use std::time::Duration;

use windows_service::{
    service::{
        ServiceAccess, ServiceErrorControl, ServiceInfo, ServiceStartType, ServiceState,
        ServiceType,
    },
    service_manager::{ServiceManager, ServiceManagerAccess},
};

pub const SERVICE_NAME: &str = "SoundCore";
pub const SERVICE_DISPLAY_NAME: &str = "SoundCore Audio & Camera Service";
pub const SERVICE_DESCRIPTION: &str =
    "Keeps the SoundCore mic-volume lock active in the background, even when the GUI is closed.";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ServiceStatusKind {
    NotInstalled,
    Stopped,
    Starting,
    Running,
    Stopping,
    Unknown,
}

impl ServiceStatusKind {
    pub fn label(&self) -> &'static str {
        match self {
            ServiceStatusKind::NotInstalled => "not installed",
            ServiceStatusKind::Stopped => "stopped",
            ServiceStatusKind::Starting => "starting…",
            ServiceStatusKind::Running => "running",
            ServiceStatusKind::Stopping => "stopping…",
            ServiceStatusKind::Unknown => "unknown",
        }
    }
}

pub fn query_status() -> ServiceStatusKind {
    let mgr = match ServiceManager::local_computer(None::<&str>, ServiceManagerAccess::CONNECT) {
        Ok(m) => m,
        Err(_) => return ServiceStatusKind::Unknown,
    };
    let svc = match mgr.open_service(SERVICE_NAME, ServiceAccess::QUERY_STATUS) {
        Ok(s) => s,
        Err(_) => return ServiceStatusKind::NotInstalled,
    };
    let status = match svc.query_status() {
        Ok(s) => s,
        Err(_) => return ServiceStatusKind::Unknown,
    };
    match status.current_state {
        ServiceState::Stopped => ServiceStatusKind::Stopped,
        ServiceState::StartPending => ServiceStatusKind::Starting,
        ServiceState::Running => ServiceStatusKind::Running,
        ServiceState::StopPending => ServiceStatusKind::Stopping,
        _ => ServiceStatusKind::Unknown,
    }
}

pub fn install_and_start() -> Result<String, String> {
    let mgr = ServiceManager::local_computer(None::<&str>, ServiceManagerAccess::CREATE_SERVICE)
        .map_err(|e| format!("open SCM (CREATE_SERVICE): {e}"))?;
    let exe_path = std::env::current_exe().map_err(|e| format!("current_exe: {e}"))?;
    let info = ServiceInfo {
        name: OsString::from(SERVICE_NAME),
        display_name: OsString::from(SERVICE_DISPLAY_NAME),
        service_type: ServiceType::OWN_PROCESS,
        start_type: ServiceStartType::AutoStart,
        error_control: ServiceErrorControl::Normal,
        executable_path: exe_path,
        launch_arguments: vec![OsString::from("--service")],
        dependencies: vec![],
        account_name: None,
        account_password: None,
    };
    let svc = mgr
        .create_service(&info, ServiceAccess::CHANGE_CONFIG | ServiceAccess::START)
        .map_err(|e| format!("create_service: {e}"))?;
    let _ = svc.set_description(SERVICE_DESCRIPTION);
    svc.start::<&str>(&[])
        .map_err(|e| format!("start: {e}"))?;
    // Give SCM a moment to flip to Running.
    std::thread::sleep(Duration::from_millis(400));
    Ok("Service installed and started.".into())
}

pub fn stop_and_uninstall() -> Result<String, String> {
    let mgr = ServiceManager::local_computer(None::<&str>, ServiceManagerAccess::CONNECT)
        .map_err(|e| format!("open SCM: {e}"))?;
    let svc = mgr
        .open_service(
            SERVICE_NAME,
            ServiceAccess::STOP | ServiceAccess::DELETE | ServiceAccess::QUERY_STATUS,
        )
        .map_err(|e| format!("open_service: {e}"))?;
    let _ = svc.stop(); // ok if already stopped
    // Wait briefly for clean shutdown.
    for _ in 0..20 {
        if let Ok(st) = svc.query_status() {
            if matches!(st.current_state, ServiceState::Stopped) {
                break;
            }
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    svc.delete().map_err(|e| format!("delete: {e}"))?;
    Ok("Service stopped and uninstalled.".into())
}

pub fn start() -> Result<String, String> {
    let mgr = ServiceManager::local_computer(None::<&str>, ServiceManagerAccess::CONNECT)
        .map_err(|e| format!("open SCM: {e}"))?;
    let svc = mgr
        .open_service(SERVICE_NAME, ServiceAccess::START)
        .map_err(|e| format!("open_service: {e}"))?;
    svc.start::<&str>(&[]).map_err(|e| format!("start: {e}"))?;
    Ok("Service started.".into())
}

pub fn stop() -> Result<String, String> {
    let mgr = ServiceManager::local_computer(None::<&str>, ServiceManagerAccess::CONNECT)
        .map_err(|e| format!("open SCM: {e}"))?;
    let svc = mgr
        .open_service(SERVICE_NAME, ServiceAccess::STOP)
        .map_err(|e| format!("open_service: {e}"))?;
    svc.stop().map_err(|e| format!("stop: {e}"))?;
    Ok("Service stopped.".into())
}
