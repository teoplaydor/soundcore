//! C ABI declared by `native/vst-host/include/soundcore_vst_host.h`.

use std::ffi::{c_char, c_int, c_uint, c_void};

#[repr(C)]
#[derive(Default, Clone, Copy)]
pub struct ScVstHostPluginInfo {
    pub uid: *const c_char,
    pub name: *const c_char,
    pub vendor: *const c_char,
    pub category: *const c_char,
    pub path: *const c_char,
    pub num_inputs: c_uint,
    pub num_outputs: c_uint,
    pub has_editor: c_uint,
}

pub type ScNodeId = [u8; 16];

extern "C" {
    pub fn sc_vst_host_version() -> c_uint;
    pub fn sc_vst_host_last_error() -> *const c_char;

    pub fn sc_vst_host_scan(
        search_paths: *const *const c_char,
        path_count: c_uint,
        out_index: *mut *mut c_void,
    ) -> c_int;

    pub fn sc_vst_host_index_size(index: *const c_void) -> c_uint;

    pub fn sc_vst_host_index_get(
        index: *const c_void,
        slot: c_uint,
        out: *mut ScVstHostPluginInfo,
    ) -> c_int;

    pub fn sc_vst_host_index_free(index: *mut c_void);

    pub fn sc_vst_host_chain_new(
        sample_rate: f64,
        max_block_samples: c_uint,
        channels: c_uint,
        out_chain: *mut *mut c_void,
    ) -> c_int;

    pub fn sc_vst_host_chain_free(chain: *mut c_void);

    pub fn sc_vst_host_chain_append(
        chain: *mut c_void,
        plugin_uid: *const c_char,
        plugin_path: *const c_char,
        out_node_id: *mut ScNodeId,
    ) -> c_int;

    pub fn sc_vst_host_chain_remove(chain: *mut c_void, node_id: *const ScNodeId) -> c_int;

    pub fn sc_vst_host_chain_process(
        chain: *mut c_void,
        interleaved_in_out: *mut f32,
        channels: c_uint,
        frames: c_uint,
    ) -> c_int;

    pub fn sc_vst_host_chain_set_parameter(
        chain: *mut c_void,
        node_id: *const ScNodeId,
        index: c_uint,
        value: f32,
    ) -> c_int;
}
