// soundcore_vst_host: C ABI for the JUCE-based VST3 host.
//
// Mirrored by Rust in crates/soundcore-vst-host/src/ffi.rs. If you change
// any signature here, change the Rust side too.
//
// All functions are reentrant; the *_chain_* functions are NOT safe to
// call from multiple threads concurrently on the same chain handle.

#pragma once

#include <stdint.h>
#include <stddef.h>

#ifdef __cplusplus
extern "C" {
#endif

#define SC_VST_HOST_VERSION 1u
uint32_t sc_vst_host_version(void);

// ===== Plugin index ========================================================

typedef struct sc_vst_host_plugin_info {
    const char* uid;
    const char* name;
    const char* vendor;
    const char* category;
    const char* path;
    uint32_t    num_inputs;
    uint32_t    num_outputs;
    uint32_t    has_editor;       // bool
} sc_vst_host_plugin_info;

// Scan VST3 plugins in the given search paths. Returns 0 on success and
// stores an opaque handle in *out_index. The handle must be freed with
// sc_vst_host_index_free().
int32_t sc_vst_host_scan(
    const char* const* search_paths,
    uint32_t           path_count,
    void**             out_index);

uint32_t sc_vst_host_index_size(const void* index);
int32_t  sc_vst_host_index_get(
    const void*             index,
    uint32_t                slot,
    sc_vst_host_plugin_info* out);
void sc_vst_host_index_free(void* index);

// ===== Processing chain ====================================================

// Stable 128-bit identifier for a node inside a chain.
typedef uint8_t sc_node_id[16];

int32_t sc_vst_host_chain_new(
    double   sample_rate,
    uint32_t max_block_samples,
    uint32_t channels,
    void**   out_chain);

void sc_vst_host_chain_free(void* chain);

int32_t sc_vst_host_chain_append(
    void*       chain,
    const char* plugin_uid,
    const char* plugin_path,
    sc_node_id  out_node_id);

int32_t sc_vst_host_chain_remove(void* chain, const sc_node_id node_id);

int32_t sc_vst_host_chain_process(
    void*    chain,
    float*   interleaved_in_out,
    uint32_t channels,
    uint32_t frames);

int32_t sc_vst_host_chain_set_parameter(
    void*           chain,
    const sc_node_id node_id,
    uint32_t        index,
    float           value);

// ===== Diagnostics =========================================================

const char* sc_vst_host_last_error(void);

#ifdef __cplusplus
} // extern "C"
#endif
