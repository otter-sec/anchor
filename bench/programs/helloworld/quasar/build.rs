use std::{path::PathBuf, process::Command};

fn main() {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let program_manifest = manifest_dir.join("program/Cargo.toml");

    // Write .so to <bench>/../target/deploy/ so both the bench harness and
    // the anchor-debugger smoke test read from the same place.
    // manifest_dir = bench/programs/helloworld/quasar
    //   → ancestors[4] = bench/..  (worktree root)
    let bench_parent = manifest_dir
        .ancestors()
        .nth(4)
        .expect("crate not at expected depth");
    let deploy_dir = bench_parent.join("target/deploy");

    let status = Command::new("cargo")
        .args([
            "build-sbf",
            "--tools-version",
            "v1.57",
            "--arch",
            "v3",
            "--manifest-path",
            program_manifest.to_str().unwrap(),
            "--sbf-out-dir",
            deploy_dir.to_str().unwrap(),
        ])
        .status()
        .expect("failed to launch cargo build-sbf");

    if !status.success() {
        panic!("cargo build-sbf failed for nested quasar program");
    }

    println!("cargo:rerun-if-changed=program/src/lib.rs");
    println!("cargo:rerun-if-changed=program/Cargo.toml");
}
