//! Named-pipe transport built on top of [`tokio::net::windows::named_pipe`].
//!
//! On the **server** side: a long-lived task creates a fresh
//! [`NamedPipeServer`] instance, waits for one client, then immediately
//! spawns a new pending instance so the next client can connect without
//! racing. Each accepted connection is wrapped in [`Framed<_, FrameCodec>`].
//!
//! On the **client** side: a single connect attempt with bounded retry on
//! `ERROR_PIPE_BUSY`.

use std::ffi::OsString;
use std::io;
use std::os::windows::ffi::OsStrExt;
use std::sync::Arc;
use std::time::Duration;
use tokio::net::windows::named_pipe::{
    ClientOptions, NamedPipeClient, NamedPipeServer, PipeMode, ServerOptions,
};
use tokio_util::codec::Framed;
use windows::core::PCWSTR;
use windows::Win32::Foundation::{LocalFree, ERROR_PIPE_BUSY, HLOCAL};
use windows::Win32::Security::Authorization::{
    ConvertStringSecurityDescriptorToSecurityDescriptorW, SDDL_REVISION_1,
};
use windows::Win32::Security::{PSECURITY_DESCRIPTOR, SECURITY_ATTRIBUTES};

use crate::codec::FrameCodec;

/// Pipe DACL: full control for SYSTEM (SY) and Administrators (BA); generic
/// read+write for Authenticated Users (AU, 0x12019F = FILE_GENERIC_READ |
/// FILE_GENERIC_WRITE). Deliberately NOT Everyone. The service runs as
/// LocalSystem, so without this the default DACL would deny the UI client.
const PIPE_SDDL: &str = "D:(A;;FA;;;SY)(A;;FA;;;BA)(A;;0x12019F;;;AU)";

/// Owns the SDDL-built security descriptor for the pipe and frees it on drop.
/// Cloneable handle (Arc) so every re-armed pipe instance shares one SD.
struct PipeSecurity {
    psd: PSECURITY_DESCRIPTOR,
}

unsafe impl Send for PipeSecurity {}
unsafe impl Sync for PipeSecurity {}

impl PipeSecurity {
    fn new() -> io::Result<Self> {
        let wide: Vec<u16> = OsString::from(PIPE_SDDL)
            .encode_wide()
            .chain(std::iter::once(0))
            .collect();
        let mut psd = PSECURITY_DESCRIPTOR::default();
        unsafe {
            ConvertStringSecurityDescriptorToSecurityDescriptorW(
                PCWSTR(wide.as_ptr()),
                SDDL_REVISION_1,
                &mut psd,
                None,
            )
            .map_err(|e| io::Error::other(format!("build pipe SD: {e}")))?;
        }
        Ok(Self { psd })
    }

    fn attributes(&self) -> SECURITY_ATTRIBUTES {
        SECURITY_ATTRIBUTES {
            nLength: std::mem::size_of::<SECURITY_ATTRIBUTES>() as u32,
            lpSecurityDescriptor: self.psd.0,
            bInheritHandle: false.into(),
        }
    }
}

impl Drop for PipeSecurity {
    fn drop(&mut self) {
        if !self.psd.0.is_null() {
            unsafe {
                let _ = LocalFree(HLOCAL(self.psd.0));
            }
        }
    }
}

/// A framed connection from the server's perspective.
pub type ServerConnection = Framed<NamedPipeServer, FrameCodec>;
/// A framed connection from the client's perspective.
pub type ClientConnection = Framed<NamedPipeClient, FrameCodec>;

/// Maximum concurrent UI connections accepted by the core service.
/// We don't expect more than a handful in normal operation (tray + main window).
pub const MAX_SERVER_INSTANCES: usize = 16;

/// Server-side listener.
///
/// Yields each accepted [`ServerConnection`] as the underlying overlapped
/// `ConnectNamedPipe` completes, while keeping a "spare" instance pending
/// so the pipe is always advertised to new clients.
pub struct Listener {
    pipe_name: String,
    security: Arc<PipeSecurity>,
    next_instance: Option<NamedPipeServer>,
}

impl Listener {
    pub fn bind(pipe_name: impl Into<String>) -> io::Result<Self> {
        let pipe_name = pipe_name.into();
        let security = Arc::new(PipeSecurity::new()?);
        let first = create_instance(&pipe_name, &security, true)?;
        Ok(Self {
            pipe_name,
            security,
            next_instance: Some(first),
        })
    }

    /// Wait for the next client and return a framed connection.
    ///
    /// Self-healing: the pending instance is only consumed once the connect
    /// succeeds, and any error leaves a fresh pending instance behind, so a
    /// caller that loops `accept()` on error never hits a poisoned state.
    pub async fn accept(&mut self) -> io::Result<ServerConnection> {
        // Ensure we have a pending instance even if a previous accept() bailed
        // out before re-arming.
        if self.next_instance.is_none() {
            self.next_instance =
                Some(create_instance(&self.pipe_name, &self.security, false)?);
        }
        let server = self
            .next_instance
            .take()
            .expect("just ensured next_instance is Some");

        if let Err(e) = server.connect().await {
            // Connect failed: put a fresh instance back so the next accept()
            // call is well-formed, then report the error.
            self.next_instance =
                create_instance(&self.pipe_name, &self.security, false).ok();
            return Err(e);
        }

        // Re-arm with a fresh pending instance so the pipe remains
        // discoverable for the next client. If this fails (e.g. instance cap),
        // leave None — the next accept() recreates it instead of panicking.
        self.next_instance =
            create_instance(&self.pipe_name, &self.security, false).ok();

        Ok(Framed::new(server, FrameCodec))
    }
}

/// Create one named-pipe server instance with the shared security descriptor.
fn create_instance(
    pipe_name: &str,
    security: &PipeSecurity,
    first: bool,
) -> io::Result<NamedPipeServer> {
    let mut sa = security.attributes();
    let mut opts = ServerOptions::new();
    opts.pipe_mode(PipeMode::Byte)
        .max_instances(MAX_SERVER_INSTANCES)
        .reject_remote_clients(true);
    if first {
        opts.first_pipe_instance(true);
    }
    // SAFETY: `sa` points at a valid SECURITY_ATTRIBUTES whose security
    // descriptor (`security`) outlives this call.
    unsafe {
        opts.create_with_security_attributes_raw(
            pipe_name,
            &mut sa as *mut SECURITY_ATTRIBUTES as *mut std::ffi::c_void,
        )
    }
}

/// Connect a client. Retries on `ERROR_PIPE_BUSY` up to `attempts` times,
/// with a small backoff between tries.
pub async fn connect(pipe_name: &str, attempts: usize) -> io::Result<ClientConnection> {
    let mut last_err: Option<io::Error> = None;
    for i in 0..attempts.max(1) {
        match ClientOptions::new().open(pipe_name) {
            Ok(client) => return Ok(Framed::new(client, FrameCodec)),
            Err(e) if e.raw_os_error() == Some(ERROR_PIPE_BUSY.0 as i32) => {
                last_err = Some(e);
                tokio::time::sleep(Duration::from_millis(50 + (i as u64) * 25)).await;
            }
            Err(e) => return Err(e),
        }
    }
    Err(last_err.unwrap_or_else(|| io::Error::other("connect: exhausted retries")))
}
