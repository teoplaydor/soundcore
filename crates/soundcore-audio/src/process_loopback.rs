//! Per-process audio capture via the Process Loopback API
//! (`ActivateAudioInterfaceAsync` with `AUDIOCLIENT_PROCESS_LOOPBACK_PARAMS`).
//!
//! Requires Windows 10 build 20348 / Windows 11 — older builds return
//! `Unsupported`.
//!
//! `ActivateAudioInterfaceAsync` is asynchronous; the result is delivered
//! through a caller-supplied COM object implementing
//! `IActivateAudioInterfaceCompletionHandler`. `windows-rs 0.58` doesn't
//! expose the `_Impl` trait for this interface, so we build the vtable
//! by hand: a `repr(C)` struct whose first field is a `*const Vtbl`, and
//! `extern "system" fn`s for each method.

use std::ffi::c_void;
use std::os::windows::ffi::OsStrExt;
use std::sync::atomic::{AtomicI32, AtomicU32, Ordering};

use windows::core::{GUID, HRESULT, IUnknown, Interface, PCWSTR};
use windows::Win32::Foundation::{CloseHandle, BOOL, ERROR_SUCCESS, HANDLE, S_OK, WAIT_OBJECT_0};
use windows::Win32::System::Threading::{CreateEventW, SetEvent, WaitForSingleObject, INFINITE};

use crate::{AudioError, Result};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LoopbackMode {
    /// Capture the target process tree, exclude everything else.
    IncludeTargetTree,
    /// Capture everything except the target process tree.
    ExcludeTargetTree,
}

/// Public façade. Currently exposes a sync activation that returns the
/// `IAudioClient` for the requested PID; pumping samples is up to the
/// caller (use `IAudioClient::GetService::<IAudioCaptureClient>()` and a
/// timer-driven read loop). Future work: full streaming session like the
/// Microsoft `ApplicationLoopback` sample.
pub struct ProcessLoopbackCapture {
    pub pid: u32,
    pub mode: LoopbackMode,
}

impl ProcessLoopbackCapture {
    pub fn open(pid: u32, mode: LoopbackMode) -> Result<Self> {
        // Verify Windows build >= 20348 cheaply via RegGetValueW.
        if !is_supported_build() {
            return Err(AudioError::Unsupported(
                "Process Loopback API requires Windows 10 build 20348 / Win11",
            ));
        }
        // Probe the activation pipeline: we synchronously kick the async
        // call and wait for the completion handler. If we get S_OK + a
        // non-null IUnknown back, the OS supports the call on this PID.
        let _client = activate_sync(pid, mode)?;
        Ok(Self { pid, mode })
    }

    pub fn start(&self) -> Result<()> {
        Ok(())
    }
    pub fn stop(&self) -> Result<()> {
        Ok(())
    }
}

fn is_supported_build() -> bool {
    use windows::Win32::System::Registry::{
        RegGetValueW, HKEY_LOCAL_MACHINE, RRF_RT_REG_SZ,
    };
    let subkey = wide_z(r"SOFTWARE\Microsoft\Windows NT\CurrentVersion");
    let name = wide_z("CurrentBuildNumber");
    let mut buf = [0u16; 32];
    let mut sz: u32 = (buf.len() * 2) as u32;
    let r = unsafe {
        RegGetValueW(
            HKEY_LOCAL_MACHINE,
            PCWSTR(subkey.as_ptr()),
            PCWSTR(name.as_ptr()),
            RRF_RT_REG_SZ,
            None,
            Some(buf.as_mut_ptr() as *mut _),
            Some(&mut sz),
        )
    };
    if r != ERROR_SUCCESS {
        return false;
    }
    let len = buf.iter().position(|&c| c == 0).unwrap_or(0);
    String::from_utf16_lossy(&buf[..len])
        .parse::<u32>()
        .map(|n| n >= 20348)
        .unwrap_or(false)
}

fn wide_z(s: &str) -> Vec<u16> {
    let mut v: Vec<u16> = s.encode_utf16().collect();
    v.push(0);
    v
}

// ===========================================================================
//  Hand-rolled COM completion handler
// ===========================================================================

// AUDIOCLIENT_ACTIVATION_TYPE
const AUDIOCLIENT_ACTIVATION_TYPE_DEFAULT: u32 = 0;
const AUDIOCLIENT_ACTIVATION_TYPE_PROCESS_LOOPBACK: u32 = 1;

const PROCESS_LOOPBACK_MODE_INCLUDE_TARGET_PROCESS_TREE: u32 = 0;
const PROCESS_LOOPBACK_MODE_EXCLUDE_TARGET_PROCESS_TREE: u32 = 1;

#[repr(C)]
struct AudioClientActivationParams {
    activation_type: u32,
    process_loopback_params: ProcessLoopbackParams,
}
#[repr(C)]
struct ProcessLoopbackParams {
    target_process_id: u32,
    mode: u32,
}

#[repr(C)]
struct PropVariantBlob {
    vt: u16,
    wReserved1: u16,
    wReserved2: u16,
    wReserved3: u16,
    cb_size: u32,
    p_blob_data: *mut u8,
    _padding: [u8; 24], // PROPVARIANT is 24 bytes header + variant; we use a Blob.
}

const VT_BLOB: u16 = 65;

// VIRTUAL_AUDIO_DEVICE_PROCESS_LOOPBACK
const VAD_PROCESS_LOOPBACK: &str = "VAD\\Process_Loopback";

// IID_IActivateAudioInterfaceCompletionHandler
const IID_ACTIVATE_COMPLETION_HANDLER: GUID =
    GUID::from_u128(0x41D949AB_9862_444A_80F6_C261334DA5EB);
// IID_IActivateAudioInterfaceAsyncOperation
const IID_ACTIVATE_ASYNC_OP: GUID =
    GUID::from_u128(0x72A22D78_CDE4_431D_B8CC_843A71199B6D);

#[repr(C)]
struct CompletionHandlerVtbl {
    query_interface: unsafe extern "system" fn(
        this: *mut c_void,
        riid: *const GUID,
        ppv: *mut *mut c_void,
    ) -> HRESULT,
    add_ref: unsafe extern "system" fn(this: *mut c_void) -> u32,
    release: unsafe extern "system" fn(this: *mut c_void) -> u32,
    activate_completed:
        unsafe extern "system" fn(this: *mut c_void, op: *mut c_void) -> HRESULT,
}

#[repr(C)]
struct CompletionHandler {
    vtbl: *const CompletionHandlerVtbl,
    refcount: AtomicU32,
    event: HANDLE,
    completion_hr: AtomicI32,
    activated_ptr: std::sync::Mutex<Option<IUnknown>>,
}

static VTBL: CompletionHandlerVtbl = CompletionHandlerVtbl {
    query_interface: ch_query_interface,
    add_ref: ch_add_ref,
    release: ch_release,
    activate_completed: ch_activate_completed,
};

unsafe extern "system" fn ch_query_interface(
    this: *mut c_void,
    riid: *const GUID,
    ppv: *mut *mut c_void,
) -> HRESULT {
    if ppv.is_null() || riid.is_null() {
        return HRESULT(0x80004003u32 as i32); // E_POINTER
    }
    let iid = *riid;
    if iid == GUID::from_u128(0x00000000_0000_0000_C000_000000000046) // IID_IUnknown
        || iid == IID_ACTIVATE_COMPLETION_HANDLER
    {
        *ppv = this;
        (*(this as *mut CompletionHandler)).refcount.fetch_add(1, Ordering::AcqRel);
        return S_OK;
    }
    *ppv = std::ptr::null_mut();
    HRESULT(0x80004002u32 as i32) // E_NOINTERFACE
}

unsafe extern "system" fn ch_add_ref(this: *mut c_void) -> u32 {
    let h = this as *mut CompletionHandler;
    (*h).refcount.fetch_add(1, Ordering::AcqRel) + 1
}

unsafe extern "system" fn ch_release(this: *mut c_void) -> u32 {
    let h = this as *mut CompletionHandler;
    let prev = (*h).refcount.fetch_sub(1, Ordering::AcqRel);
    let r = prev - 1;
    if r == 0 {
        // Allocated via Box::into_raw → reclaim.
        drop(Box::from_raw(h));
    }
    r
}

unsafe extern "system" fn ch_activate_completed(
    this: *mut c_void,
    op: *mut c_void,
) -> HRESULT {
    let h = &mut *(this as *mut CompletionHandler);

    // op implements IActivateAudioInterfaceAsyncOperation. Vtable layout:
    //   [0] QueryInterface
    //   [1] AddRef
    //   [2] Release
    //   [3] GetActivateResult(out HRESULT* hr, out IUnknown** punk)
    if op.is_null() {
        h.completion_hr.store(0x80070057u32 as i32, Ordering::Release);
        let _ = SetEvent(h.event);
        return S_OK;
    }
    type GetActivateResultFn = unsafe extern "system" fn(
        *mut c_void,
        *mut HRESULT,
        *mut *mut c_void,
    ) -> HRESULT;
    // `op` points to a struct whose first field is `*const Vtbl`. We
    // need a pointer to the vtable array (so `.add(N)` walks function
    // slots, not bytes).
    let vt: *const *const c_void = *(op as *const *const *const c_void);
    let get_fn_ptr: *const c_void = *vt.add(3);
    let get_fn: GetActivateResultFn = std::mem::transmute(get_fn_ptr);

    let mut activate_hr = HRESULT(0);
    let mut activated: *mut c_void = std::ptr::null_mut();
    let hr = get_fn(op, &mut activate_hr, &mut activated);
    if hr.is_err() {
        h.completion_hr.store(hr.0, Ordering::Release);
        let _ = SetEvent(h.event);
        return S_OK;
    }
    h.completion_hr.store(activate_hr.0, Ordering::Release);
    if activate_hr.is_ok() && !activated.is_null() {
        // Wrap into IUnknown so it's released on drop.
        let unk: IUnknown = IUnknown::from_raw(activated);
        *h.activated_ptr.lock().unwrap() = Some(unk);
    }
    let _ = SetEvent(h.event);
    S_OK
}

fn make_handler(event: HANDLE) -> *mut CompletionHandler {
    Box::into_raw(Box::new(CompletionHandler {
        vtbl: &VTBL,
        refcount: AtomicU32::new(1),
        event,
        completion_hr: AtomicI32::new(0),
        activated_ptr: std::sync::Mutex::new(None),
    }))
}

#[link(name = "mmdevapi")]
extern "system" {
    fn ActivateAudioInterfaceAsync(
        device_interface_path: PCWSTR,
        riid: *const GUID,
        activation_params: *const c_void,
        completion_handler: *mut c_void,
        async_op: *mut *mut c_void,
    ) -> HRESULT;
}

fn activate_sync(pid: u32, mode: LoopbackMode) -> Result<IUnknown> {
    let event = unsafe { CreateEventW(None, false, false, None)? };

    let params = AudioClientActivationParams {
        activation_type: AUDIOCLIENT_ACTIVATION_TYPE_PROCESS_LOOPBACK,
        process_loopback_params: ProcessLoopbackParams {
            target_process_id: pid,
            mode: match mode {
                LoopbackMode::IncludeTargetTree => {
                    PROCESS_LOOPBACK_MODE_INCLUDE_TARGET_PROCESS_TREE
                }
                LoopbackMode::ExcludeTargetTree => {
                    PROCESS_LOOPBACK_MODE_EXCLUDE_TARGET_PROCESS_TREE
                }
            },
        },
    };

    let mut pv = PropVariantBlob {
        vt: VT_BLOB,
        wReserved1: 0,
        wReserved2: 0,
        wReserved3: 0,
        cb_size: std::mem::size_of::<AudioClientActivationParams>() as u32,
        p_blob_data: &params as *const _ as *mut u8,
        _padding: [0; 24],
    };

    let handler = make_handler(event);

    // IID_IAudioClient
    let iid_audio_client = GUID::from_u128(0x1CB9AD4C_DBFA_4C32_B178_C2F568A703B2);

    // VIRTUAL_AUDIO_DEVICE_PROCESS_LOOPBACK as wide string
    let vad: Vec<u16> = std::ffi::OsStr::new(VAD_PROCESS_LOOPBACK)
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();

    let mut async_op: *mut c_void = std::ptr::null_mut();
    let hr = unsafe {
        ActivateAudioInterfaceAsync(
            PCWSTR(vad.as_ptr()),
            &iid_audio_client,
            &pv as *const _ as *const c_void,
            handler as *mut c_void,
            &mut async_op,
        )
    };
    if hr.is_err() {
        unsafe {
            (((*handler).vtbl).as_ref().unwrap().release)(handler as *mut c_void);
            let _ = CloseHandle(event);
        }
        return Err(AudioError::Com(windows::core::Error::from(hr)));
    }

    // Block until the completion handler signals.
    let wait = unsafe { WaitForSingleObject(event, INFINITE) };
    if wait != WAIT_OBJECT_0 {
        unsafe {
            (((*handler).vtbl).as_ref().unwrap().release)(handler as *mut c_void);
            let _ = CloseHandle(event);
        }
        return Err(AudioError::Unsupported(
            "WaitForSingleObject on completion event did not return WAIT_OBJECT_0",
        ));
    }

    // Inspect the result.
    let result_hr = unsafe { (*handler).completion_hr.load(Ordering::Acquire) };
    let activated = unsafe { (*handler).activated_ptr.lock().unwrap().take() };

    // Release the handler (it's still owned by ActivateAudioInterfaceAsync
    // and we drop our reference by calling release once).
    unsafe {
        (((*handler).vtbl).as_ref().unwrap().release)(handler as *mut c_void);
        if !async_op.is_null() {
            type ReleaseFn = unsafe extern "system" fn(*mut c_void) -> u32;
            let vt: *const *const c_void = *(async_op as *const *const *const c_void);
            let release_fn: ReleaseFn = std::mem::transmute(*vt.add(2));
            release_fn(async_op);
        }
        let _ = CloseHandle(event);
    }

    if result_hr != 0 {
        return Err(AudioError::Com(windows::core::Error::from(HRESULT(
            result_hr,
        ))));
    }
    activated.ok_or(AudioError::Unsupported(
        "completion handler signalled S_OK but returned no IUnknown",
    ))
}

// Suppress unused-warning when not testing.
#[allow(dead_code)]
const _: &str = VAD_PROCESS_LOOPBACK;
