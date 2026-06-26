//! Embedded DLL extraction + COM registration.
//!
//! The single SoundCore.exe ships with `SoundCoreApo.dll` and
//! `SoundCoreVirtualCamera.dll` baked in as `include_bytes!`. On first
//! launch we drop them under `%ProgramData%\SoundCore\bin\` and call
//! their `DllRegisterServer` exports directly — no `regsvr32.exe` shell
//! out, no Visual C++ tooling required at install time.

use std::ffi::CString;
use std::fs;
use std::io;
use std::os::windows::ffi::OsStrExt;
use std::path::{Path, PathBuf};
use windows::core::{PCSTR, PCWSTR};
use windows::Win32::Foundation::{FreeLibrary, HMODULE};
use windows::Win32::System::LibraryLoader::{GetProcAddress, LoadLibraryW};

pub const APO_DLL_BYTES: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/SoundCoreApo.dll"));
pub const VIRTCAM_DLL_BYTES: &[u8] =
    include_bytes!(concat!(env!("OUT_DIR"), "/SoundCoreVirtualCamera.dll"));

pub fn install_dir() -> PathBuf {
    PathBuf::from(
        std::env::var_os("ProgramData").unwrap_or_else(|| r"C:\ProgramData".into()),
    )
    .join("SoundCore")
    .join("bin")
}

pub fn data_dir() -> PathBuf {
    PathBuf::from(
        std::env::var_os("ProgramData").unwrap_or_else(|| r"C:\ProgramData".into()),
    )
    .join("SoundCore")
}

/// Extract every embedded DLL into the install dir. Idempotent — if the
/// target already has the same bytes, skip the write.
pub fn extract_dlls() -> io::Result<ExtractedDlls> {
    let dir = install_dir();
    fs::create_dir_all(&dir)?;
    let apo = write_if_different(&dir.join("SoundCoreApo.dll"), APO_DLL_BYTES)?;
    let vc = write_if_different(&dir.join("SoundCoreVirtualCamera.dll"), VIRTCAM_DLL_BYTES)?;
    Ok(ExtractedDlls {
        apo_dll: apo,
        virtcam_dll: vc,
    })
}

#[derive(Debug, Clone)]
pub struct ExtractedDlls {
    /// `None` when the binary was built without the C++ DLLs (early
    /// dev build); the GUI shows a banner in that case.
    pub apo_dll: Option<PathBuf>,
    pub virtcam_dll: Option<PathBuf>,
}

fn write_if_different(path: &Path, bytes: &[u8]) -> io::Result<Option<PathBuf>> {
    if bytes.is_empty() {
        return Ok(None);
    }
    match fs::read(path) {
        Ok(existing) if existing == bytes => Ok(Some(path.to_path_buf())),
        _ => {
            fs::write(path, bytes)?;
            Ok(Some(path.to_path_buf()))
        }
    }
}

/// Call `DllRegisterServer` on the DLL at `path` without shelling out
/// to `regsvr32.exe`. We `LoadLibrary` the DLL, resolve the export, and
/// invoke it directly. Works just as well for un-registration via
/// `DllUnregisterServer`.
pub fn register_dll(path: &Path, register: bool) -> io::Result<()> {
    let wide: Vec<u16> = path.as_os_str().encode_wide().chain(std::iter::once(0)).collect();
    let hmod: HMODULE = unsafe {
        LoadLibraryW(PCWSTR(wide.as_ptr())).map_err(io::Error::other)?
    };
    struct Guard(HMODULE);
    impl Drop for Guard {
        fn drop(&mut self) {
            unsafe {
                let _ = FreeLibrary(self.0);
            }
        }
    }
    let _guard = Guard(hmod);

    let proc_name = if register {
        c"DllRegisterServer"
    } else {
        c"DllUnregisterServer"
    };
    let cname = CString::new(proc_name.to_bytes()).expect("static cstr");
    let proc = unsafe { GetProcAddress(hmod, PCSTR(cname.as_ptr() as *const u8)) };
    let proc = proc.ok_or_else(|| {
        io::Error::other(format!(
            "{} export not found in {}",
            proc_name.to_string_lossy(),
            path.display()
        ))
    })?;
    let f: unsafe extern "system" fn() -> windows::core::HRESULT =
        unsafe { std::mem::transmute(proc) };
    let hr = unsafe { f() };
    if hr.is_err() {
        return Err(io::Error::other(format!(
            "{} returned 0x{:08x}",
            proc_name.to_string_lossy(),
            hr.0 as u32
        )));
    }
    Ok(())
}

/// One-shot self-registration of every embedded DLL. Returns Ok with a
/// vector of (name, succeeded, message) per DLL so the GUI can show
/// granular status.
pub fn extract_and_register_all() -> io::Result<Vec<RegistrationOutcome>> {
    let dlls = extract_dlls()?;
    let mut out = Vec::new();
    for (label, path) in [
        ("SoundCore APO", dlls.apo_dll.clone()),
        ("SoundCore Virtual Camera", dlls.virtcam_dll.clone()),
    ] {
        let outcome = match path {
            None => RegistrationOutcome {
                label: label.into(),
                ok: false,
                message: "Not bundled in this build (native DLL was missing at compile time)".into(),
            },
            Some(p) => match register_dll(&p, true) {
                Ok(_) => RegistrationOutcome {
                    label: label.into(),
                    ok: true,
                    message: format!("Registered → {}", p.display()),
                },
                Err(e) => RegistrationOutcome {
                    label: label.into(),
                    ok: false,
                    message: format!("Register failed: {e}"),
                },
            },
        };
        out.push(outcome);
    }
    Ok(out)
}

pub fn unregister_all() -> io::Result<Vec<RegistrationOutcome>> {
    let dir = install_dir();
    let mut out = Vec::new();
    for (label, name) in [
        ("SoundCore APO", "SoundCoreApo.dll"),
        ("SoundCore Virtual Camera", "SoundCoreVirtualCamera.dll"),
    ] {
        let p = dir.join(name);
        if !p.exists() {
            out.push(RegistrationOutcome {
                label: label.into(),
                ok: true,
                message: "Already absent".into(),
            });
            continue;
        }
        let outcome = match register_dll(&p, false) {
            Ok(_) => RegistrationOutcome {
                label: label.into(),
                ok: true,
                message: "Unregistered".into(),
            },
            Err(e) => RegistrationOutcome {
                label: label.into(),
                ok: false,
                message: format!("Unregister failed: {e}"),
            },
        };
        out.push(outcome);
    }
    Ok(out)
}

#[derive(Debug, Clone)]
pub struct RegistrationOutcome {
    pub label: String,
    pub ok: bool,
    pub message: String,
}
