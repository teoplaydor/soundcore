use std::path::PathBuf;

fn main() {
    // The proto is at workspace-root /protos/soundcore.proto so the C#
    // UI build can reference the same source-of-truth file.
    let workspace = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .map(PathBuf::from)
        .expect("workspace root above crates/soundcore-ipc");
    let proto_dir: PathBuf = workspace.join("protos");
    let proto_file = proto_dir.join("soundcore.proto");

    println!("cargo:rerun-if-changed={}", proto_file.display());

    // Use a vendored protoc so we don't require the user to install one.
    let protoc = protoc_bin_vendored::protoc_bin_path()
        .expect("protoc-bin-vendored: protoc_bin_path");
    std::env::set_var("PROTOC", &protoc);

    let mut cfg = prost_build::Config::new();
    cfg.bytes(["."]);
    cfg.compile_protos(&[proto_file], &[proto_dir])
        .expect("prost-build: failed to compile soundcore.proto");
}
