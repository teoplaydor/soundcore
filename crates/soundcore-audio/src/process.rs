//! Enumerate currently running processes with PID, image name and full
//! path. Used by the UI to populate the per-app FX page.

use std::path::PathBuf;
use windows::Win32::Foundation::CloseHandle;
use windows::Win32::System::Diagnostics::ToolHelp::{
    CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W,
    TH32CS_SNAPPROCESS,
};
use windows::Win32::System::ProcessStatus::GetModuleFileNameExW;
use windows::Win32::System::Threading::{OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION};

use crate::Result;

#[derive(Debug, Clone)]
pub struct ProcessInfo {
    pub pid: u32,
    pub image_name: String,
    pub image_path: String,
}

pub fn list_processes() -> Result<Vec<ProcessInfo>> {
    let snap = unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0)? };

    struct Snap(windows::Win32::Foundation::HANDLE);
    impl Drop for Snap {
        fn drop(&mut self) {
            unsafe {
                let _ = CloseHandle(self.0);
            }
        }
    }
    let _snap_guard = Snap(snap);

    let mut entry = PROCESSENTRY32W {
        dwSize: std::mem::size_of::<PROCESSENTRY32W>() as u32,
        ..Default::default()
    };

    let mut out = Vec::new();
    if unsafe { Process32FirstW(snap, &mut entry) }.is_ok() {
        loop {
            let name = utf16_terminated_to_string(&entry.szExeFile);
            let path = best_effort_image_path(entry.th32ProcessID);
            out.push(ProcessInfo {
                pid: entry.th32ProcessID,
                image_name: name,
                image_path: path,
            });
            if unsafe { Process32NextW(snap, &mut entry) }.is_err() {
                break;
            }
        }
    }

    out.sort_by(|a, b| a.image_name.to_lowercase().cmp(&b.image_name.to_lowercase()));
    Ok(out)
}

fn utf16_terminated_to_string(buf: &[u16]) -> String {
    let len = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
    String::from_utf16_lossy(&buf[..len])
}

fn best_effort_image_path(pid: u32) -> String {
    let handle = match unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) } {
        Ok(h) => h,
        Err(_) => return String::new(),
    };

    let mut buf = [0u16; 1024];
    let n = unsafe { GetModuleFileNameExW(handle, None, &mut buf) };
    let _ = unsafe { CloseHandle(handle) };
    if n == 0 {
        String::new()
    } else {
        String::from_utf16_lossy(&buf[..n as usize])
    }
}

/// Best-effort: returns just the image base name from a full path.
pub fn image_base_name(image_path: &str) -> String {
    PathBuf::from(image_path)
        .file_name()
        .and_then(|s| s.to_str())
        .map(String::from)
        .unwrap_or_default()
}
