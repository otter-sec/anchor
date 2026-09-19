//! Reproduces OR-109: `#[constant]` IDL emission only escapes `"` and `\`,
//! so a custom `Debug` newline splits the JSON fragment and `anchor idl build`
//! cannot parse it.
//!
//! Run: `cargo test -p anchor-lang --test constant_idl_escape`

use std::{fs, path::PathBuf, process::Command};

#[test]
#[cfg_attr(
    miri,
    ignore = "spawns cargo and writes a temporary workspace; covered by normal cargo test"
)]
fn constant_idl_fragment_stays_valid_json_when_debug_contains_a_newline() {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let crate_dir = manifest_dir.join("target/constant-idl-escape");
    let target_dir = manifest_dir.join("target/constant-idl-escape-target");
    let src_dir = crate_dir.join("src");
    fs::create_dir_all(&src_dir).unwrap();
    fs::write(
        crate_dir.join("Cargo.toml"),
        format!(
            r#"[package]
name = "constant-idl-escape"
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

struct Weird;

impl core::fmt::Debug for Weird {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "a\nb")
    }
}

#[constant]
pub const WEIRD: Weird = Weird;
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
            "__anchor_private_print_idl_const_weird",
            "--",
            "--nocapture",
        ])
        .output()
        .unwrap_or_else(|err| panic!("failed to run cargo test for constant-idl-escape: {err}"));

    assert!(
        output.status.success(),
        "constant-idl-escape failed to compile/run:\nstdout:\n{}\n\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    let begin = "--- IDL begin const ---";
    let end = "--- IDL end const ---";
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
        "#[constant] IDL JSON must stay on one line so the builder's marker stream stays intact.\n\
         A raw Debug newline currently splits the fragment:\n{fragment}"
    );
    assert!(
        fragment.contains(r#""value":"a\nb""#),
        "expected JSON-escaped newline in the value field, got:\n{fragment}"
    );
}
