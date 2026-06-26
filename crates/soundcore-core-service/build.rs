//! Build-time embedding:
//!  * Copies the C++ APO and Virtual Camera DLLs into OUT_DIR so
//!    `include_bytes!` can bundle them into the single .exe.
//!  * Compiles the Windows manifest (admin elevation + DPI awareness).

use std::path::PathBuf;

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=app.manifest");

    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let workspace = manifest_dir
        .ancestors()
        .nth(2)
        .map(PathBuf::from)
        .expect("workspace root above crates/soundcore-core-service");
    let out_dir = PathBuf::from(std::env::var("OUT_DIR").expect("OUT_DIR"));

    // Embed C++ DLLs (Release build).
    let dlls = [
        (
            "SoundCoreApo.dll",
            "native/build/x64/apo/Release/SoundCoreApo.dll",
        ),
        (
            "SoundCoreVirtualCamera.dll",
            "native/build/x64/virtual-camera/Release/SoundCoreVirtualCamera.dll",
        ),
    ];
    for (name, rel) in dlls {
        let src = workspace.join(rel);
        let dst = out_dir.join(name);
        if src.exists() {
            std::fs::copy(&src, &dst).expect("copy embedded DLL");
            println!("cargo:rerun-if-changed={}", src.display());
        } else {
            // Don't fail the build — let the binary compile in
            // "DLLs not yet built" mode so dev iterations on the Rust
            // side are fast. The runtime extractor checks for empty
            // bytes and skips registration in that case.
            std::fs::write(&dst, []).expect("write empty DLL placeholder");
            println!(
                "cargo:warning=SoundCore: embedded DLL placeholder for {} \
                 ({} not built yet)",
                name,
                src.display()
            );
        }
    }

    // Windows resource (manifest + version info).
    let mut res = winresource::WindowsResource::new();
    res.set_manifest_file(
        manifest_dir
            .join("app.manifest")
            .to_str()
            .expect("manifest path utf-8"),
    );
    res.set("FileDescription", "SoundCore — Windows audio & camera control");
    res.set("ProductName", "SoundCore");
    res.set("CompanyName", "SoundCore");
    res.set("LegalCopyright", "Copyright SoundCore");
    if let Err(e) = res.compile() {
        println!("cargo:warning=SoundCore: winresource compile failed: {e}");
    }
}
