#include "soundcore_vst_host.h"
#include "HostInternals.h"

#include <mutex>

namespace sc {

namespace {
// Thread-local so the pointer returned by getLastError() stays valid until
// the SAME thread next sets an error. A single shared juce::String guarded
// only during the call would hand back an interior pointer that another
// thread could free via copy-on-write reassignment (use-after-free).
thread_local std::string g_lastError;
}

void setLastError(juce::String message)
{
    g_lastError = message.toStdString();
}

const char* getLastError()
{
    // Valid until this thread calls setLastError() again. Callers that need
    // to retain it across that point must copy. Documented in the header.
    return g_lastError.c_str();
}

} // namespace sc

extern "C" {

uint32_t sc_vst_host_version(void) { return SC_VST_HOST_VERSION; }

const char* sc_vst_host_last_error(void) { return sc::getLastError(); }

} // extern "C"
