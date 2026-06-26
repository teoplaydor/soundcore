//! Audio session enumeration: walk every endpoint, list the
//! `IAudioSessionControl2` instances and the PIDs behind them.

use crate::Result;

#[derive(Debug, Clone)]
pub struct AudioSession {
    pub session_instance_id: String,
    pub device_id: String,
    pub flow: crate::DataFlow,
    pub pid: u32,
    pub display_name: String,
    pub icon_path: String,
    pub peak: f32,
    pub muted: bool,
}

pub fn list_sessions() -> Result<Vec<AudioSession>> {
    // TODO(audio): IAudioSessionManager2::GetSessionEnumerator on each device.
    Ok(Vec::new())
}
