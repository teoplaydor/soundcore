//! Standalone VST3 scanner.
//!
//! Recurses into the default VST3 locations on Windows
//! (`%CommonProgramFiles%\VST3`, `%CommonProgramFiles(x86)%\VST3`, plus any
//! custom paths passed as command-line args) and emits a JSON array of
//! discovered `.vst3` bundles to stdout.
//!
//! It intentionally does NOT load each plugin — that requires linking
//! JUCE and JUCE 8 has Direct2D/DComp ordinal requirements that aren't
//! satisfied on every Windows 10 build. Loading happens later in the
//! APO when the user actually selects a plugin; loading failure there
//! is reported back via a status file.
//!
//! JSON schema (one object per plugin):
//!   { "uid": "", "name": "ReverbX", "vendor": "", "category": "",
//!     "path": "C:\\Program Files\\Common Files\\VST3\\ReverbX.vst3",
//!     "num_inputs": 0, "num_outputs": 0, "has_editor": false }
//!
//! `uid` is empty in this stage; the APO discovers it on first load and
//! writes it back into the chain config (which is the form the user
//! actually cares about).

use serde::Serialize;
use std::path::PathBuf;

#[derive(Serialize)]
struct ScannedPlugin {
    uid: String,
    name: String,
    vendor: String,
    category: String,
    path: String,
    num_inputs: u32,
    num_outputs: u32,
    has_editor: bool,
}

fn default_paths() -> Vec<PathBuf> {
    let mut out = Vec::new();
    if let Some(p) = std::env::var_os("CommonProgramFiles") {
        out.push(PathBuf::from(p).join("VST3"));
    }
    if let Some(p) = std::env::var_os("CommonProgramFiles(x86)") {
        out.push(PathBuf::from(p).join("VST3"));
    }
    if let Some(p) = std::env::var_os("ProgramFiles") {
        let base = PathBuf::from(p);
        out.push(base.join("Common Files").join("VST3"));
        out.push(base.join("VSTPlugins"));
    }
    if let Some(p) = std::env::var_os("LOCALAPPDATA") {
        out.push(PathBuf::from(p).join("Programs").join("Common").join("VST3"));
    }
    out
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let paths: Vec<PathBuf> = if args.is_empty() {
        default_paths()
    } else {
        args.iter().map(PathBuf::from).collect()
    };

    let mut found: Vec<ScannedPlugin> = Vec::new();
    for root in &paths {
        if !root.exists() {
            continue;
        }
        // Iterate manually so we can stop descending once we hit a `.vst3`
        // bundle directory. Otherwise WalkDir walks into it and finds the
        // inner `Contents\x86_64-win\<name>.vst3` DLL — a different path, so
        // a path-based dedup wouldn't catch it — emitting the same plugin
        // twice (once as the bundle dir, once as the inner module).
        let mut it = walkdir::WalkDir::new(root)
            .follow_links(false)
            .max_depth(6)
            .into_iter();
        while let Some(entry) = it.next() {
            let entry = match entry {
                Ok(e) => e,
                Err(_) => continue,
            };
            let path = entry.path();
            let is_vst3 = path
                .extension()
                .and_then(|e| e.to_str())
                .map(|s| s.eq_ignore_ascii_case("vst3"))
                .unwrap_or(false);
            if !is_vst3 {
                continue;
            }
            // Record the topmost `.vst3` entry and don't descend into it.
            if entry.file_type().is_dir() {
                it.skip_current_dir();
            }
            let name = path
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("(unnamed)")
                .to_string();
            let path_str = path.to_string_lossy().into_owned();
            // De-duplicate by absolute path (search roots can overlap).
            if found.iter().any(|p| p.path == path_str) {
                continue;
            }
            found.push(ScannedPlugin {
                uid: String::new(),
                name,
                vendor: String::new(),
                category: String::new(),
                path: path_str,
                num_inputs: 0,
                num_outputs: 0,
                has_editor: false,
            });
        }
    }

    found.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));

    match serde_json::to_string_pretty(&found) {
        Ok(s) => println!("{s}"),
        Err(e) => {
            eprintln!("json encode failed: {e}");
            std::process::exit(3);
        }
    }
}
