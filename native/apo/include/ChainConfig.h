// Plugin-chain config loader for the APO.
//
// Reads `%ProgramData%\SoundCore\chain.txt`. Format:
//   # Comment lines start with '#'
//   <plugin uid>|<plugin path>
//   <plugin uid>|<plugin path>

#pragma once

#include <string>
#include <vector>
#include <cstdint>

namespace sc {

struct PluginEntry {
    std::string uid;
    std::string path;
};

struct ChainConfig {
    bool enabled = false;
    std::vector<PluginEntry> plugins;

    // mtime of the file when read, used by the watcher to decide whether
    // to reload. Zero if file didn't exist.
    int64_t mtime = 0;
};

/// Read the chain config from the canonical path. Returns an empty
/// disabled config if the file is missing or unparseable.
ChainConfig load_chain_config();

/// Resolve `%ProgramData%\SoundCore\chain.txt` as a wide string.
std::wstring chain_config_path();

/// Cheap mtime-only check; used by the file watcher.
int64_t chain_config_mtime();

} // namespace sc
