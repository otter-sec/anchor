//! Reproduces OR-074: `#[error_code]` IDL emission only escapes `"` and `\`,
//! so a `#[msg]` newline splits the JSON fragment and `anchor idl build`
//! cannot parse it.
//!
//! Run: `cargo test -p anchor-lang --test error_code_idl_escape`

use std::{fs, path::PathBuf, process::Command};

#[test]
#[cfg_attr(
    miri,
    ignore = "spawns cargo and writes a temporary workspace; covered by normal cargo test"
)]
fn error_code_idl_fragment_stays_valid_json_when_msg_contains_a_newline() {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let crate_dir = manifest_dir.join("target/error-code-idl-escape");
    let target_dir = manifest_dir.join("target/error-code-idl-escape-target");
    let src_dir = crate_dir.join("src");
    fs::create_dir_all(&src_dir).unwrap();
    fs::write(
        crate_dir.join("Cargo.toml"),
        format!(
            r#"[package]
name = "error-code-idl-escape"
version = "0.1.0"
edition = "2021"
publish = false

[features]
idl-build = []

[dependencies]
anchor-lang = {{ path = "{}" }}

[workspace]
"#,
            manifest_dir.display()
        ),
    )
    .unwrap();
    fs::write(
        src_dir.join("lib.rs"),
        r#"
use anchor_lang::prelude::*;

#[error_code]
pub enum WeirdError {
    #[msg("a\nb")]
    Weird,
}
"#,
    )
    .unwrap();

    let output = Command::new("cargo")
        .env("CARGO_TARGET_DIR", &target_dir)
        .args(["test", "--offline", "--manifest-path"])
        .arg(crate_dir.join("Cargo.toml"))
        .args([
            "--features",
            "idl-build",
            "__anchor_private_print_idl_errors_weirderror",
            "--",
            "--nocapture",
        ])
        .output()
        .unwrap_or_else(|err| panic!("failed to run cargo test for error-code-idl-escape: {err}"));

    assert!(
        output.status.success(),
        "error-code-idl-escape failed to compile/run:\nstdout:\n{}\n\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    let begin = "--- IDL begin errors ---";
    let end = "--- IDL end errors ---";
    let start = stdout
        .find(begin)
        .unwrap_or_else(|| panic!("missing {begin} in stdout:\n{stdout}"));
    let finish = stdout
        .find(end)
        .unwrap_or_else(|| panic!("missing {end} in stdout:\n{stdout}"));
    let fragment = stdout[start + begin.len()..finish].trim();

    assert_eq!(
        fragment.lines().count(),
        1,
        "#[error_code] IDL JSON must stay on one line so the builder's marker stream stays intact.\n\
         A raw #[msg] newline currently splits the fragment:\n{fragment}"
    );
    assert!(
        fragment.contains(r#""msg":"a\nb""#),
        "expected JSON-escaped newline in the msg field, got:\n{fragment}"
    );
}
