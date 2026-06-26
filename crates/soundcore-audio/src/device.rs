//! WASAPI device enumeration via `IMMDeviceEnumerator`.
//!
//! The caller is responsible for initialising COM on the current thread
//! before constructing a [`DeviceEnumerator`]. See [`crate::init`].

use std::ffi::c_void;

use windows::core::{PCWSTR, PWSTR};
use windows::Win32::Devices::FunctionDiscovery::{
    PKEY_DeviceInterface_FriendlyName, PKEY_Device_FriendlyName,
};
use windows::Win32::Media::Audio::Endpoints::IAudioEndpointVolume;
use windows::Win32::Media::Audio::{
    eCapture, eCommunications, eConsole, eMultimedia, eRender,
    EDataFlow, ERole, IAudioClient, IMMDevice, IMMDeviceEnumerator,
    MMDeviceEnumerator, DEVICE_STATE_ACTIVE,
};
use windows::Win32::System::Com::{CoCreateInstance, CoTaskMemFree, CLSCTX_ALL, STGM_READ};
use windows::Win32::UI::Shell::PropertiesSystem::{IPropertyStore, PROPERTYKEY};

use crate::Result;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DataFlow {
    Render,
    Capture,
}

impl DataFlow {
    fn to_win(self) -> EDataFlow {
        match self {
            DataFlow::Render => eRender,
            DataFlow::Capture => eCapture,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Role {
    Console,
    Multimedia,
    Communications,
}

impl Role {
    fn to_win(self) -> ERole {
        match self {
            Role::Console => eConsole,
            Role::Multimedia => eMultimedia,
            Role::Communications => eCommunications,
        }
    }
}

#[derive(Debug, Clone)]
pub struct AudioDevice {
    pub id: String,
    pub friendly_name: String,
    pub interface_name: String,
    pub flow: DataFlow,
    pub is_default: bool,
    pub is_default_communications: bool,
    pub sample_rate: u32,
    pub channel_count: u32,
    pub bits_per_sample: u32,
    pub master_volume: f32,
    pub mute: bool,
}

pub struct DeviceEnumerator {
    raw: IMMDeviceEnumerator,
}

impl DeviceEnumerator {
    pub fn new() -> Result<Self> {
        let raw: IMMDeviceEnumerator =
            unsafe { CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL)? };
        Ok(Self { raw })
    }

    pub fn list(&self, flow: DataFlow) -> Result<Vec<AudioDevice>> {
        let default_id = self
            .default_endpoint_id(flow, Role::Console)
            .unwrap_or_default();
        let default_comms_id = self
            .default_endpoint_id(flow, Role::Communications)
            .unwrap_or_default();

        let collection = unsafe {
            self.raw
                .EnumAudioEndpoints(flow.to_win(), DEVICE_STATE_ACTIVE)?
        };
        let count = unsafe { collection.GetCount()? };

        let mut out = Vec::with_capacity(count as usize);
        for i in 0..count {
            let imm = unsafe { collection.Item(i)? };
            match build_device(&imm, flow, &default_id, &default_comms_id) {
                Ok(d) => out.push(d),
                Err(e) => {
                    tracing::warn!(error = ?e, index = i, "skipping device that failed to introspect");
                }
            }
        }
        Ok(out)
    }

    pub fn default(&self, flow: DataFlow, role: Role) -> Result<Option<AudioDevice>> {
        let imm = match unsafe { self.raw.GetDefaultAudioEndpoint(flow.to_win(), role.to_win()) }
        {
            Ok(d) => d,
            Err(e) if e.code().0 == HRESULT_ERROR_NOT_FOUND => return Ok(None),
            Err(e) => return Err(e.into()),
        };
        let default_id = self
            .default_endpoint_id(flow, Role::Console)
            .unwrap_or_default();
        let default_comms_id = self
            .default_endpoint_id(flow, Role::Communications)
            .unwrap_or_default();
        Ok(Some(build_device(
            &imm,
            flow,
            &default_id,
            &default_comms_id,
        )?))
    }

    pub fn open(&self, device_id: &str) -> Result<IMMDevice> {
        let mut id_wide: Vec<u16> = device_id.encode_utf16().collect();
        id_wide.push(0);
        let imm = unsafe { self.raw.GetDevice(PCWSTR(id_wide.as_ptr()))? };
        Ok(imm)
    }

    fn default_endpoint_id(&self, flow: DataFlow, role: Role) -> Result<String> {
        let imm = unsafe { self.raw.GetDefaultAudioEndpoint(flow.to_win(), role.to_win())? };
        Ok(read_device_id(&imm))
    }
}

impl Default for DeviceEnumerator {
    fn default() -> Self {
        Self::new().expect("DeviceEnumerator::new must succeed in default()")
    }
}

const HRESULT_ERROR_NOT_FOUND: i32 = 0x80070490u32 as i32;

// ---------------------------------------------------------------------------
//  helpers
// ---------------------------------------------------------------------------

fn build_device(
    imm: &IMMDevice,
    flow: DataFlow,
    default_id: &str,
    default_comms_id: &str,
) -> Result<AudioDevice> {
    let id = read_device_id(imm);

    let props: IPropertyStore = unsafe { imm.OpenPropertyStore(STGM_READ)? };
    let friendly_name = read_string_prop(&props, &PKEY_Device_FriendlyName).unwrap_or_default();
    let interface_name =
        read_string_prop(&props, &PKEY_DeviceInterface_FriendlyName).unwrap_or_default();

    let (sample_rate, channel_count, bits_per_sample) = read_mix_format(imm).unwrap_or((0, 0, 0));
    let (master_volume, mute) = read_volume_and_mute(imm).unwrap_or((1.0, false));

    Ok(AudioDevice {
        is_default: id.eq_ignore_ascii_case(default_id),
        is_default_communications: id.eq_ignore_ascii_case(default_comms_id),
        id,
        friendly_name,
        interface_name,
        flow,
        sample_rate,
        channel_count,
        bits_per_sample,
        master_volume,
        mute,
    })
}

fn read_device_id(imm: &IMMDevice) -> String {
    match unsafe { imm.GetId() } {
        Ok(pwstr) => {
            let s = pwstr_to_string(pwstr);
            unsafe { CoTaskMemFree(Some(pwstr.0 as *const c_void)) };
            s
        }
        Err(_) => String::new(),
    }
}

fn pwstr_to_string(p: PWSTR) -> String {
    if p.0.is_null() {
        return String::new();
    }
    unsafe {
        let mut len = 0usize;
        while *p.0.add(len) != 0 {
            len += 1;
        }
        let slice = std::slice::from_raw_parts(p.0, len);
        String::from_utf16_lossy(slice)
    }
}

fn read_string_prop(store: &IPropertyStore, key: &PROPERTYKEY) -> Result<String> {
    let pv = unsafe { store.GetValue(key)? };
    // `windows::core::PROPVARIANT` wraps the raw union and exposes
    // `to_string()` which calls `PropVariantToStringAlloc` under the
    // hood for us. Empty/non-string values come back as "".
    Ok(pv.to_string())
}

fn read_mix_format(imm: &IMMDevice) -> Result<(u32, u32, u32)> {
    let client: IAudioClient = unsafe { imm.Activate(CLSCTX_ALL, None)? };
    let fmt_ptr = unsafe { client.GetMixFormat()? };
    if fmt_ptr.is_null() {
        return Ok((0, 0, 0));
    }
    let result = unsafe {
        let fmt = *fmt_ptr;
        (
            fmt.nSamplesPerSec,
            fmt.nChannels as u32,
            fmt.wBitsPerSample as u32,
        )
    };
    unsafe { CoTaskMemFree(Some(fmt_ptr as *const c_void)) };
    Ok(result)
}

fn read_volume_and_mute(imm: &IMMDevice) -> Result<(f32, bool)> {
    let vol: IAudioEndpointVolume = unsafe { imm.Activate(CLSCTX_ALL, None)? };
    let v = unsafe { vol.GetMasterVolumeLevelScalar()? };
    let m = unsafe { vol.GetMute()? };
    Ok((v, m.as_bool()))
}
