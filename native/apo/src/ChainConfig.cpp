#include "ChainConfig.h"

#include <windows.h>
#include <shlobj.h>
#include <fstream>
#include <sys/stat.h>

namespace sc {

namespace {

std::wstring program_data_dir()
{
    PWSTR p = nullptr;
    std::wstring out;
    if (SUCCEEDED(SHGetKnownFolderPath(FOLDERID_ProgramData, 0, nullptr, &p)))
    {
        out = p;
        CoTaskMemFree(p);
    }
    else
    {
        out = L"C:\\ProgramData";
    }
    return out;
}

std::string utf8_from_wide(const std::wstring& w)
{
    if (w.empty()) return {};
    int n = WideCharToMultiByte(CP_UTF8, 0, w.data(), (int)w.size(),
                                nullptr, 0, nullptr, nullptr);
    if (n <= 0) return {};
    std::string s(n, '\0');
    WideCharToMultiByte(CP_UTF8, 0, w.data(), (int)w.size(),
                        s.data(), n, nullptr, nullptr);
    return s;
}

} // namespace

std::wstring chain_config_path()
{
    return program_data_dir() + L"\\SoundCore\\chain.txt";
}

int64_t chain_config_mtime()
{
    auto path = chain_config_path();
    struct _stat64 st {};
    if (_wstat64(path.c_str(), &st) != 0) return 0;
    return static_cast<int64_t>(st.st_mtime);
}

ChainConfig load_chain_config()
{
    ChainConfig cfg;
    cfg.mtime = chain_config_mtime();

    auto path = chain_config_path();
    std::ifstream f(path);
    if (!f) return cfg;

    std::string line;
    while (std::getline(f, line))
    {
        // trim \r leftover from CRLF
        while (!line.empty() && (line.back() == '\r' || line.back() == ' '))
            line.pop_back();

        if (line.empty() || line[0] == '#') continue;

        auto sep = line.find('|');
        if (sep == std::string::npos) continue;

        PluginEntry e;
        e.uid = line.substr(0, sep);
        e.path = line.substr(sep + 1);

        // Trim whitespace.
        auto trim = [](std::string& s) {
            while (!s.empty() && s.front() == ' ') s.erase(s.begin());
            while (!s.empty() && s.back() == ' ') s.pop_back();
        };
        trim(e.uid);
        trim(e.path);

        if (!e.path.empty()) cfg.plugins.push_back(std::move(e));
    }

    cfg.enabled = !cfg.plugins.empty();
    return cfg;
}

} // namespace sc
