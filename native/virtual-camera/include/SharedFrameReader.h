// Consumer side of the shared-memory frame ring published by the
// `soundcore-camera` crate in the core service.
//
// Layout matches `SharedHeader` in `crates/soundcore-camera/src/broadcaster.rs`.

#pragma once

#include <windows.h>
#include <cstdint>
#include <string>

namespace sc {

struct SharedHeader {
    uint32_t magic;            // 'SCFR' (0x53434652)
    uint32_t version;          // 1
    uint32_t slot_count;
    uint32_t slot_bytes;
    uint32_t width;
    uint32_t height;
    uint32_t stride;
    uint32_t subtype_fourcc;
    uint32_t frame_rate_num;
    uint32_t frame_rate_den;
    uint32_t producer_alive;
    uint32_t generation;       // bumped on every producer (re)create
    uint32_t _padding[4];
    // Then `slot_count` slots of `slot_bytes` each, with per-slot
    // sequence numbers in a parallel array at the end.
};

class SharedFrameReader {
public:
    SharedFrameReader();
    ~SharedFrameReader();

    // Connect to the shared mapping. Returns true on success.
    bool connect(const wchar_t* name);

    // Disconnect; safe to call repeatedly.
    void disconnect();

    bool isConnected() const { return m_view != nullptr; }
    const SharedHeader* header() const { return m_header; }

    // Block (up to `timeoutMs`) for the next produced frame and return a
    // pointer to its slot bytes. Returns nullptr on timeout / disconnect.
    const uint8_t* waitForNextFrame(DWORD timeoutMs);

private:
    HANDLE m_mapping = nullptr;
    void* m_view = nullptr;
    SharedHeader* m_header = nullptr;
    HANDLE m_frameEvent = nullptr;
    uint64_t m_lastSequence = 0;
    uint32_t m_generation = 0;     // last seen producer generation
    std::wstring m_name;           // for lazy event re-open

    // Lazily (re)open the wakeup event if we connected before the producer
    // created it. Returns true once the handle is valid.
    bool ensureEvent();
};

} // namespace sc
