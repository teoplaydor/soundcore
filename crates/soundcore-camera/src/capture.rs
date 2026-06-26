//! Media Foundation capture session.
//!
//! Spawns a dedicated worker thread that owns:
//!   * an `IMFMediaSource` for the user-picked camera,
//!   * an `IMFSourceReader` reading raw NV12 frames,
//!   * a [`Broadcaster`] that publishes each frame into shared memory
//!     for the consumer DLL to pick up.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread::JoinHandle;

use windows::core::PCWSTR;
use windows::Win32::Media::MediaFoundation::{
    MFCreateAttributes, MFCreateDeviceSource, MFCreateSourceReaderFromMediaSource,
    IMFMediaType, MFMediaType_Video, MFVideoFormat_NV12,
    MF_DEVSOURCE_ATTRIBUTE_SOURCE_TYPE, MF_DEVSOURCE_ATTRIBUTE_SOURCE_TYPE_VIDCAP_GUID,
    MF_DEVSOURCE_ATTRIBUTE_SOURCE_TYPE_VIDCAP_SYMBOLIC_LINK,
    MF_MT_FRAME_RATE, MF_MT_FRAME_SIZE, MF_MT_MAJOR_TYPE, MF_MT_SUBTYPE,
    MF_SOURCE_READER_FIRST_VIDEO_STREAM,
};
use tracing::{info, warn};

use crate::broadcaster::Broadcaster;
use crate::init::mf_startup_lite;
use crate::Result;

pub const DEFAULT_CHANNEL: &str = "SoundCore.Camera.0";

/// `MF_DEVSOURCE_ATTRIBUTE_FRAMESERVER_SHARE_MODE` (mfidl.h, Windows 11 build
/// 26100 / 24H2). Setting it to 1 opens the camera in Frame Server *shared*
/// mode instead of taking exclusive control, so other apps can use the same
/// physical camera concurrently — native multi-app sharing with no virtual
/// device. windows-rs 0.58 doesn't export the constant, so we define the
/// documented GUID. On older Windows the attribute is simply ignored (MF
/// reads only the keys it understands), so it's safe to set unconditionally.
/// See docs/camera-sharing-research.md.
const MF_DEVSOURCE_ATTRIBUTE_FRAMESERVER_SHARE_MODE: windows::core::GUID =
    windows::core::GUID::from_values(
        0x44d1a9bc,
        0x2999,
        0x4238,
        [0xae, 0x43, 0x07, 0x30, 0xce, 0xb2, 0xab, 0x1b],
    );

pub struct CaptureSession {
    cancel: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

impl CaptureSession {
    /// Spawn a producer thread streaming frames from `symbolic_link`
    /// at the requested dimensions / fps into the shared-memory channel.
    pub fn start(
        symbolic_link: String,
        width: u32,
        height: u32,
        fps_num: u32,
        fps_den: u32,
    ) -> Self {
        let cancel = Arc::new(AtomicBool::new(false));
        let cancel_clone = cancel.clone();
        let thread = std::thread::Builder::new()
            .name("sc-camera-prod".into())
            .spawn(move || {
                if let Err(e) = run_producer(symbolic_link, width, height, fps_num, fps_den, cancel_clone) {
                    warn!(error = ?e, "camera producer ended with error");
                }
            })
            .expect("spawn camera producer");
        Self {
            cancel,
            thread: Some(thread),
        }
    }
}

impl Drop for CaptureSession {
    fn drop(&mut self) {
        self.cancel.store(true, Ordering::Relaxed);
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}

fn run_producer(
    symbolic_link: String,
    width: u32,
    height: u32,
    fps_num: u32,
    fps_den: u32,
    cancel: Arc<AtomicBool>,
) -> Result<()> {
    let _mf = mf_startup_lite().ok();

    // Build attribute store: source type + symbolic link.
    let attrs = unsafe {
        let mut a = None;
        MFCreateAttributes(&mut a, 3)?;
        a.expect("MFCreateAttributes succeeded but yielded None")
    };
    unsafe {
        attrs.SetGUID(
            &MF_DEVSOURCE_ATTRIBUTE_SOURCE_TYPE,
            &MF_DEVSOURCE_ATTRIBUTE_SOURCE_TYPE_VIDCAP_GUID,
        )?;
        let mut link_w: Vec<u16> = symbolic_link.encode_utf16().collect();
        link_w.push(0);
        attrs.SetString(
            &MF_DEVSOURCE_ATTRIBUTE_SOURCE_TYPE_VIDCAP_SYMBOLIC_LINK,
            PCWSTR(link_w.as_ptr()),
        )?;
        // Open in shared mode on Win11 24H2+ so we don't lock out other apps;
        // ignored on older Windows.
        attrs.SetUINT32(&MF_DEVSOURCE_ATTRIBUTE_FRAMESERVER_SHARE_MODE, 1)?;
    }

    let source = unsafe { MFCreateDeviceSource(&attrs)? };
    let reader = unsafe { MFCreateSourceReaderFromMediaSource(&source, None)? };

    // Configure preferred output as NV12 at requested dims.
    let target = unsafe {
        let t: IMFMediaType = windows::Win32::Media::MediaFoundation::MFCreateMediaType()?;
        t.SetGUID(&MF_MT_MAJOR_TYPE, &MFMediaType_Video)?;
        t.SetGUID(&MF_MT_SUBTYPE, &MFVideoFormat_NV12)?;
        // Pack size: high 32 = width, low 32 = height
        let size = (width as u64) << 32 | (height as u64);
        t.SetUINT64(&MF_MT_FRAME_SIZE, size)?;
        let rate = (fps_num as u64) << 32 | (fps_den as u64);
        t.SetUINT64(&MF_MT_FRAME_RATE, rate)?;
        t
    };
    let stream_index: u32 = MF_SOURCE_READER_FIRST_VIDEO_STREAM.0 as u32;
    unsafe {
        reader.SetCurrentMediaType(stream_index, None, &target)?;
    }

    // NV12 = 12 bits/pixel (Y plane + half-res UV plane). Slot size:
    let slot_bytes = width * height * 3 / 2;
    let mut broadcaster = Broadcaster::create(
        DEFAULT_CHANNEL,
        width,
        height,
        slot_bytes,
        4,
        u32::from_le_bytes(*b"NV12"),
        fps_num,
        fps_den,
    )?;

    info!(
        width,
        height,
        fps_num,
        fps_den,
        slot_bytes,
        "camera producer streaming"
    );

    while !cancel.load(Ordering::Relaxed) {
        let mut actual_stream_index = 0u32;
        let mut stream_flags = 0u32;
        let mut timestamp = 0i64;
        let sample = unsafe {
            let mut sample = None;
            reader.ReadSample(
                stream_index,
                0,
                Some(&mut actual_stream_index),
                Some(&mut stream_flags),
                Some(&mut timestamp),
                Some(&mut sample),
            )?;
            sample
        };
        let Some(sample) = sample else {
            continue;
        };
        let buffer = unsafe { sample.ConvertToContiguousBuffer()? };
        let mut data_ptr: *mut u8 = std::ptr::null_mut();
        let mut max_len: u32 = 0;
        let mut cur_len: u32 = 0;
        unsafe {
            buffer.Lock(&mut data_ptr, Some(&mut max_len), Some(&mut cur_len))?;
        }
        let slice = unsafe { std::slice::from_raw_parts(data_ptr, cur_len as usize) };
        broadcaster.publish(slice);
        unsafe {
            buffer.Unlock()?;
        }
    }

    info!("camera producer stopping");
    let _ = source;
    Ok(())
}
