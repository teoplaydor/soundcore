// Mirrors the C ABI exported by the Rust crate `soundcore-apo-core`.
//
// The Rust side is the single source of truth; if you change this header
// you must also change the corresponding `#[no_mangle]` declarations in
// crates/soundcore-apo-core/src/lib.rs (and vice versa).

#pragma once

#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

#define SC_APO_CORE_VERSION 1u

typedef struct ScApoStream {
    float    sample_rate;
    uint32_t channels;
    uint32_t max_frames_per_call;
    void*    chain;
} ScApoStream;

uint32_t sc_apo_core_abi_version(void);

int32_t sc_apo_core_open_stream(
    ScApoStream* out,
    float        sample_rate,
    uint32_t     channels,
    uint32_t     max_frames_per_call);

int32_t sc_apo_core_close_stream(ScApoStream* stream);

int32_t sc_apo_core_process(
    ScApoStream* stream,
    float*       interleaved_in_out,
    uint32_t     frames);

#ifdef __cplusplus
} // extern "C"
#endif
