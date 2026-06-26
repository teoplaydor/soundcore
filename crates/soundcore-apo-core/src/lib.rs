//! APO DSP core, exported as a static library that the C++ APO DLL in
//! `native/apo` links against.
//!
//! The C++ side owns the COM identity (`IAudioProcessingObject`,
//! `IAudioProcessingObjectConfiguration`, `IAudioProcessingObjectRTQueueService`)
//! and the registration plumbing; on every `APOProcess` call it forwards
//! the audio block into Rust here, which routes it through the VST chain
//! configured for the endpoint.
//!
//! All the C entry points are declared in
//! `native/apo/include/soundcore_apo_core.h`.

use std::ffi::{c_int, c_uint};

/// API version, bumped on incompatible C ABI changes.
pub const SC_APO_CORE_VERSION: u32 = 1;

/// Per-stream context. The C++ side allocates one of these per
/// LockForProcess and holds it for the lifetime of the streaming session.
#[repr(C)]
pub struct ScApoStream {
    sample_rate: f32,
    channels: u32,
    max_frames_per_call: u32,
    chain: *mut std::ffi::c_void, // boxed Rust state
}

/// Hand the C side a stream context. Returns 0 on success.
#[no_mangle]
pub unsafe extern "C" fn sc_apo_core_open_stream(
    out: *mut ScApoStream,
    sample_rate: f32,
    channels: c_uint,
    max_frames_per_call: c_uint,
) -> c_int {
    if out.is_null() {
        return -1;
    }
    *out = ScApoStream {
        sample_rate,
        channels,
        max_frames_per_call,
        chain: std::ptr::null_mut(),
    };
    0
}

#[no_mangle]
pub unsafe extern "C" fn sc_apo_core_close_stream(stream: *mut ScApoStream) -> c_int {
    if stream.is_null() {
        return -1;
    }
    let s = &mut *stream;
    if !s.chain.is_null() {
        drop(Box::from_raw(s.chain as *mut ChainState));
        s.chain = std::ptr::null_mut();
    }
    0
}

#[no_mangle]
pub unsafe extern "C" fn sc_apo_core_process(
    stream: *mut ScApoStream,
    interleaved_in_out: *mut f32,
    frames: c_uint,
) -> c_int {
    if stream.is_null() || interleaved_in_out.is_null() {
        return -1;
    }
    let s = &*stream;
    if frames > s.max_frames_per_call {
        return -2;
    }
    // TODO(apo): forward to VST chain. For now this is a passthrough.
    let _ = std::slice::from_raw_parts_mut(
        interleaved_in_out,
        (frames as usize) * (s.channels as usize),
    );
    0
}

/// Reserved for the C++ side to verify ABI compatibility before any
/// `open_stream` call.
#[no_mangle]
pub extern "C" fn sc_apo_core_abi_version() -> u32 {
    SC_APO_CORE_VERSION
}

/// Internal: per-chain state owned by Rust. Boxed and stored as `*mut` in
/// `ScApoStream::chain`.
struct ChainState;
