//! Locate `soundcore_vst_host.lib` produced by CMake under `native/vst-host`,
//! then emit link directives for it and JUCE's transitive Windows deps so
//! any downstream Rust binary links cleanly.

use std::env;
use std::path::PathBuf;

fn main() {
    println!("cargo:rerun-if-changed=build.rs");

    let workspace = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .map(PathBuf::from)
        .expect("workspace root above crates/soundcore-vst-host");

    // JUCE in Debug config references the debug CRT (`_CrtDbgReport` and
    // friends) which is incompatible with Rust's MSVC target (Rust always
    // links the release CRT). We therefore always consume the Release-mode
    // C++ static lib — for both Rust debug and Rust release builds.
    let _ = env::var("PROFILE");
    let candidate = workspace.join("native/build/x64/vst-host/Release");
    let lib_path = candidate.join("soundcore_vst_host.lib");

    // Re-run when the native lib appears or changes. Cargo treats a watched
    // path that doesn't exist as always-dirty, so this also forces a re-run
    // on the build *after* the C++ lib is first produced — otherwise the
    // "not found" warning state would stick until an unrelated change.
    println!("cargo:rerun-if-changed={}", lib_path.display());

    if lib_path.exists() {
        // Normalize all-backslash form so msvc link.exe is happy on every
        // tested toolchain (some versions silently drop search paths that
        // mix forward and backslash separators).
        let search_path = candidate
            .to_string_lossy()
            .replace('/', "\\");
        println!("cargo:rustc-link-search=native={search_path}");
        println!("cargo:rustc-link-lib=static=soundcore_vst_host");

        // JUCE pulls in a long tail of system libraries on Windows. The
        // C++ static lib has them as inputs at compile time, but Rust's
        // linker needs them explicitly when consuming our static lib.
        let system_libs = [
            "gdi32", "user32", "kernel32", "advapi32", "ole32", "oleaut32",
            "uuid", "comdlg32", "shell32", "winmm", "version", "imm32",
            "shlwapi", "ws2_32", "wininet", "rpcrt4", "dxgi",
            "dwmapi", "msimg32", "gdiplus", "winspool", "mfuuid", "mfplat",
            "mf", "mfreadwrite", "ksuser", "Propsys", "Pathcch",
        ];
        for lib in system_libs {
            println!("cargo:rustc-link-lib=dylib={lib}");
        }
    } else {
        println!(
            "cargo:warning=soundcore_vst_host.lib not found at {} \u{2014} build native/vst-host first \
             (scripts/build.ps1 or `cmake --build native/build/x64 --target soundcore_vst_host`).",
            candidate.display()
        );
    }
}
