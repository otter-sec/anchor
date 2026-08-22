use std::{path::Path, process::Command};

fn build_fixture(manifest_dir: &Path, force_inline: bool) -> String {
    let target_dir = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("target")
        .join("handler-inline-policy");
    let mut command = Command::new("cargo");
    command
        .env("CARGO_TARGET_DIR", target_dir)
        .args(["build-sbf", "--tools-version", "v1.52", "--manifest-path"])
        .arg(manifest_dir.join("Cargo.toml"));
    if force_inline {
        command.args(["--features", "force-inline"]);
    }

    let output = command.output().expect("run cargo build-sbf");
    let build_log = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        output.status.success(),
        "handler inline policy fixture did not build:\n{build_log}"
    );
    build_log
}

#[test]
fn handler_inline_false_prevents_dispatcher_stack_overflow() {
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("programs")
        .join("handler-inline-policy");

    let forced_inline_log = build_fixture(&manifest_dir, true);
    assert!(
        forced_inline_log.contains("__anchor_dispatch_internal")
            && forced_inline_log.contains("exceeded max offset of 4096"),
        "forced-inline fixture must reproduce the dispatcher overflow:\n{forced_inline_log}"
    );

    let isolated_log = build_fixture(&manifest_dir, false);
    assert!(
        !isolated_log.contains("exceeded max offset of 4096"),
        "isolated handlers must stay within the SBF frame limit:\n{isolated_log}"
    );
}
