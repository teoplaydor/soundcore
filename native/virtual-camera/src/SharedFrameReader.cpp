#include "SharedFrameReader.h"

#include <string>

namespace sc {

SharedFrameReader::SharedFrameReader() = default;

SharedFrameReader::~SharedFrameReader()
{
    disconnect();
}

bool SharedFrameReader::connect(const wchar_t* name)
{
    disconnect();
    if (!name || !*name) return false;

    // Mapping: Global\<name>. The producer creates with Global\ so we can
    // attach from any session.
    std::wstring full = L"Global\\";
    full += name;
    m_mapping = OpenFileMappingW(FILE_MAP_READ, FALSE, full.c_str());
    if (!m_mapping) return false;

    m_view = MapViewOfFile(m_mapping, FILE_MAP_READ, 0, 0, 0);
    if (!m_view)
    {
        disconnect();
        return false;
    }
    m_header = static_cast<SharedHeader*>(m_view);
    if (m_header->magic != 0x53434652 /* 'SCFR' */ || m_header->version != 1)
    {
        disconnect();
        return false;
    }

    m_name = name;
    m_generation = m_header->generation;
    m_lastSequence = 0;
    ensureEvent(); // may be null if producer hasn't created it yet
    return true;
}

bool SharedFrameReader::ensureEvent()
{
    if (m_frameEvent) return true;
    if (m_name.empty()) return false;
    std::wstring eventName = L"Global\\";
    eventName += m_name;
    eventName += L".Frame";
    m_frameEvent = OpenEventW(SYNCHRONIZE, FALSE, eventName.c_str());
    return m_frameEvent != nullptr;
}

void SharedFrameReader::disconnect()
{
    if (m_frameEvent) { CloseHandle(m_frameEvent); m_frameEvent = nullptr; }
    if (m_view)       { UnmapViewOfFile(m_view); m_view = nullptr; }
    if (m_mapping)    { CloseHandle(m_mapping); m_mapping = nullptr; }
    m_header = nullptr;
    m_name.clear();
    m_generation = 0;
    m_lastSequence = 0;
}

const uint8_t* SharedFrameReader::waitForNextFrame(DWORD timeoutMs)
{
    if (!m_header) return nullptr;

    // A producer restart that reused the live section restarts sequence
    // numbers; detect it via the generation counter and reset our
    // high-water mark so we don't reject the new producer's frames.
    uint32_t gen = m_header->generation;
    if (gen != m_generation)
    {
        m_generation = gen;
        m_lastSequence = 0;
    }

    if (ensureEvent())
    {
        DWORD r = WaitForSingleObject(m_frameEvent, timeoutMs);
        if (r == WAIT_FAILED) return nullptr;
        // WAIT_TIMEOUT falls through to the seq scan: with a manual-reset
        // event the scan is the source of truth and recovers any pulse we
        // raced past.
    }
    else
    {
        // Producer hasn't published the event yet; avoid a tight spin.
        Sleep(timeoutMs ? (timeoutMs < 15 ? timeoutMs : 15) : 0);
    }
    // Read sequence numbers and pick the freshest. Layout: slot data,
    // then `slot_count` uint64_t sequence numbers.
    const uint8_t* base = static_cast<const uint8_t*>(m_view);
    const uint8_t* slots = base + sizeof(SharedHeader);
    const uint64_t* seqs = reinterpret_cast<const uint64_t*>(
        slots + (size_t)m_header->slot_count * m_header->slot_bytes);

    uint32_t winner = 0;
    uint64_t bestSeq = 0;
    for (uint32_t i = 0; i < m_header->slot_count; ++i)
    {
        if (seqs[i] > bestSeq) { bestSeq = seqs[i]; winner = i; }
    }
    if (bestSeq <= m_lastSequence) return nullptr;
    m_lastSequence = bestSeq;
    return slots + (size_t)winner * m_header->slot_bytes;
}

} // namespace sc
