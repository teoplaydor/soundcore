//! Shared-memory broadcaster used by the camera producer.
//!
//! Producer (this side) writes frames into a fixed ring of slots backed
//! by a Win32 file mapping; consumers (the DShow/MF virtual-camera DLL
//! instances in other processes) read the freshest slot using sequence
//! numbers + a manual-reset Win32 event for wakeup.
//!
//! The producer runs inside the core service (LocalSystem). Consumers run
//! inside ordinary user-session apps (and sometimes AppContainer-sandboxed
//! ones). The kernel objects are therefore created with an explicit
//! security descriptor that grants read/synchronize to Authenticated Users
//! and ALL APPLICATION PACKAGES — without it, consumers get ACCESS_DENIED
//! because a SYSTEM token's default DACL only admits SYSTEM/Administrators.

use std::ffi::c_void;
use std::os::windows::ffi::OsStrExt;
use std::ptr::NonNull;

use windows::core::PCWSTR;
use windows::Win32::Foundation::{
    CloseHandle, LocalFree, ERROR_ALREADY_EXISTS, HANDLE, HLOCAL,
};
use windows::Win32::Security::Authorization::{
    ConvertStringSecurityDescriptorToSecurityDescriptorW, SDDL_REVISION_1,
};
use windows::Win32::Security::{PSECURITY_DESCRIPTOR, SECURITY_ATTRIBUTES};
use windows::Win32::System::Memory::{
    CreateFileMappingW, MapViewOfFile, UnmapViewOfFile, FILE_MAP_ALL_ACCESS,
    MEMORY_MAPPED_VIEW_ADDRESS, PAGE_READWRITE,
};
use windows::Win32::System::Threading::{CreateEventW, ResetEvent, SetEvent};

use crate::Result;

#[repr(C)]
pub struct SharedHeader {
    pub magic: u32,
    pub version: u32,
    pub slot_count: u32,
    pub slot_bytes: u32,
    pub width: u32,
    pub height: u32,
    pub stride: u32,
    pub subtype_fourcc: u32,
    pub frame_rate_num: u32,
    pub frame_rate_den: u32,
    pub producer_alive: u32,
    /// Bumped every time a producer (re)creates the channel. Consumers
    /// reset their sequence high-water mark when this changes, so a
    /// service restart that reuses the live named section doesn't freeze
    /// attached consumers for the old uptime's worth of frames.
    pub generation: u32,
    pub _padding: [u32; 4],
}

const MAGIC: u32 = 0x53434652; // 'SCFR'
const VERSION: u32 = 1;

/// DACL: full control for SYSTEM (SY) and Administrators (BA); read + query
/// for Authenticated Users (AU) and ALL APPLICATION PACKAGES (AC).
/// Section rights: SECTION_MAP_READ (0x4) | SECTION_QUERY (0x1) = 0x5.
const SDDL_SECTION: &str = "D:(A;;0xF001F;;;SY)(A;;0xF001F;;;BA)(A;;0x5;;;AU)(A;;0x5;;;AC)";
/// Event rights: EVENT_ALL_ACCESS for producer; SYNCHRONIZE (0x100000) for
/// consumers so they can only wait, not signal/reset.
const SDDL_EVENT: &str = "D:(A;;0x1F0003;;;SY)(A;;0x1F0003;;;BA)(A;;0x100000;;;AU)(A;;0x100000;;;AC)";

/// Owns a self-relative security descriptor allocated by
/// `ConvertStringSecurityDescriptorToSecurityDescriptorW` and frees it on
/// drop. Keep it alive for as long as the `SECURITY_ATTRIBUTES` that points
/// into it is in use.
struct SecurityDescriptor {
    psd: PSECURITY_DESCRIPTOR,
}

impl SecurityDescriptor {
    fn from_sddl(sddl: &str) -> Result<Self> {
        let wide: Vec<u16> = std::ffi::OsString::from(sddl)
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
            )?;
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

impl Drop for SecurityDescriptor {
    fn drop(&mut self) {
        if !self.psd.0.is_null() {
            unsafe {
                let _ = LocalFree(HLOCAL(self.psd.0));
            }
        }
    }
}

pub struct Broadcaster {
    mapping: HANDLE,
    view: NonNull<u8>,
    pub frame_event: HANDLE,
    pub slot_count: u32,
    pub slot_bytes: u32,
    next_seq: u64,
    current_slot: u32,
}

unsafe impl Send for Broadcaster {}

impl Broadcaster {
    pub fn create(
        name: &str,
        width: u32,
        height: u32,
        slot_bytes: u32,
        slot_count: u32,
        fourcc: u32,
        fps_num: u32,
        fps_den: u32,
    ) -> Result<Self> {
        let total = std::mem::size_of::<SharedHeader>()
            + (slot_count as usize) * (slot_bytes as usize)
            + (slot_count as usize) * std::mem::size_of::<u64>();

        let section_sd = SecurityDescriptor::from_sddl(SDDL_SECTION)?;
        let mut section_sa = section_sd.attributes();

        let mapping_name = encode_global(name);
        let mapping = unsafe {
            CreateFileMappingW(
                windows::Win32::Foundation::INVALID_HANDLE_VALUE,
                Some(&mut section_sa),
                PAGE_READWRITE,
                ((total as u64) >> 32) as u32,
                (total as u64 & 0xFFFFFFFF) as u32,
                PCWSTR(mapping_name.as_ptr()),
            )?
        };
        // GetLastError must be read immediately: a non-error Ok handle still
        // sets ERROR_ALREADY_EXISTS when the named section already lived.
        let already_existed = unsafe {
            windows::Win32::Foundation::GetLastError() == ERROR_ALREADY_EXISTS
        };
        if mapping.is_invalid() {
            return Err(crate::CameraError::Unsupported(
                "CreateFileMappingW returned invalid handle",
            ));
        }

        // Map the whole section (len 0 = to end), so reuse of a pre-existing
        // section of unknown size still gives us its full extent.
        let view: MEMORY_MAPPED_VIEW_ADDRESS =
            unsafe { MapViewOfFile(mapping, FILE_MAP_ALL_ACCESS, 0, 0, 0) };
        let view_ptr = view.Value as *mut u8;
        if view_ptr.is_null() {
            unsafe {
                let _ = CloseHandle(mapping);
            }
            return Err(crate::CameraError::Unsupported("MapViewOfFile returned NULL"));
        }
        let view_nn = NonNull::new(view_ptr).unwrap();

        let mut next_seq = 1u64;

        unsafe {
            let header = view_ptr as *mut SharedHeader;
            if already_existed {
                // Reuse path: a consumer is still holding the section alive
                // across our restart. Refuse to stomp a differing geometry —
                // we can't resize a section other processes have mapped — and
                // continue the sequence instead of restarting at 1.
                if (*header).magic != MAGIC
                    || (*header).version != VERSION
                    || (*header).slot_count != slot_count
                    || (*header).slot_bytes != slot_bytes
                {
                    let _ = UnmapViewOfFile(MEMORY_MAPPED_VIEW_ADDRESS {
                        Value: view_ptr as *mut c_void,
                    });
                    let _ = CloseHandle(mapping);
                    return Err(crate::CameraError::Busy);
                }
                let seqs = sequence_array_ptr(view_ptr, slot_count, slot_bytes);
                let mut max_seq = 0u64;
                for i in 0..slot_count as usize {
                    max_seq = max_seq.max(*seqs.add(i));
                }
                next_seq = max_seq.wrapping_add(1);
                (*header).generation = (*header).generation.wrapping_add(1);
                (*header).width = width;
                (*header).height = height;
                (*header).stride = width * 4;
                (*header).subtype_fourcc = fourcc;
                (*header).frame_rate_num = fps_num;
                (*header).frame_rate_den = fps_den;
                (*header).producer_alive = 1;
            } else {
                (*header) = SharedHeader {
                    magic: MAGIC,
                    version: VERSION,
                    slot_count,
                    slot_bytes,
                    width,
                    height,
                    stride: width * 4, // best-effort; consumer interprets fourcc
                    subtype_fourcc: fourcc,
                    frame_rate_num: fps_num,
                    frame_rate_den: fps_den,
                    producer_alive: 1,
                    generation: 1,
                    _padding: [0; 4],
                };
                let seqs = sequence_array_ptr(view_ptr, slot_count, slot_bytes);
                for i in 0..slot_count as usize {
                    *seqs.add(i) = 0;
                }
            }
        }

        // Manual-reset event consumers wait on. Manual-reset is required so a
        // single SetEvent wakes ALL attached consumers, not just one.
        let event_sd = SecurityDescriptor::from_sddl(SDDL_EVENT)?;
        let mut event_sa = event_sd.attributes();
        let event_name = encode_global(&format!("{name}.Frame"));
        let frame_event = unsafe {
            CreateEventW(
                Some(&mut event_sa),
                true,
                false,
                PCWSTR(event_name.as_ptr()),
            )?
        };

        Ok(Self {
            mapping,
            view: view_nn,
            frame_event,
            slot_count,
            slot_bytes,
            next_seq,
            current_slot: 0,
        })
    }

    /// Publish a frame. `frame_bytes` must be <= `slot_bytes`. Returns
    /// the assigned sequence number.
    pub fn publish(&mut self, frame_bytes: &[u8]) -> u64 {
        let copy_len = frame_bytes.len().min(self.slot_bytes as usize);
        let slot_idx = self.current_slot;
        unsafe {
            // Clear the wakeup before writing so consumers that wake on the
            // SetEvent below observe this frame, and consumers between frames
            // actually block (a never-reset manual event makes every wait
            // return instantly, turning the consumer into a 100% CPU spin).
            let _ = ResetEvent(self.frame_event);

            let view_ptr = self.view.as_ptr();
            let slot_ptr = slot_data_ptr(view_ptr, slot_idx, self.slot_bytes);
            std::ptr::copy_nonoverlapping(frame_bytes.as_ptr(), slot_ptr, copy_len);
            // Update sequence number AFTER copy so a consumer reading
            // strictly the highest seq sees only fully-written data.
            let seqs = sequence_array_ptr(view_ptr, self.slot_count, self.slot_bytes);
            let seq = self.next_seq;
            *seqs.add(slot_idx as usize) = seq;
            // Memory fence to publish writes.
            std::sync::atomic::fence(std::sync::atomic::Ordering::Release);
            let _ = SetEvent(self.frame_event);
        }
        let seq = self.next_seq;
        self.next_seq = self.next_seq.wrapping_add(1);
        self.current_slot = (self.current_slot + 1) % self.slot_count;
        seq
    }
}

impl Drop for Broadcaster {
    fn drop(&mut self) {
        unsafe {
            // Mark producer dead in header so consumers stop waiting.
            let header = self.view.as_ptr() as *mut SharedHeader;
            (*header).producer_alive = 0;
            let _ = SetEvent(self.frame_event);
            let _ = UnmapViewOfFile(MEMORY_MAPPED_VIEW_ADDRESS {
                Value: self.view.as_ptr() as *mut c_void,
            });
            let _ = CloseHandle(self.mapping);
            let _ = CloseHandle(self.frame_event);
        }
    }
}

fn encode_global(name: &str) -> Vec<u16> {
    let full = format!("Global\\{name}");
    std::ffi::OsString::from(full)
        .encode_wide()
        .chain(std::iter::once(0))
        .collect()
}

unsafe fn slot_data_ptr(base: *mut u8, slot: u32, slot_bytes: u32) -> *mut u8 {
    base.add(std::mem::size_of::<SharedHeader>() + (slot as usize) * (slot_bytes as usize))
}

unsafe fn sequence_array_ptr(base: *mut u8, slot_count: u32, slot_bytes: u32) -> *mut u64 {
    let after_slots =
        std::mem::size_of::<SharedHeader>() + (slot_count as usize) * (slot_bytes as usize);
    base.add(after_slots) as *mut u64
}
