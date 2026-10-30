use std::{fs, path::PathBuf, process::Command};

fn setup_case() -> (PathBuf, PathBuf) {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let crate_dir = manifest_dir
        .join("target/compile-cases")
        .join("declare_program_idl_errors");
    let anchor_lang = manifest_dir
        .parent()
        .expect("tests-v2 should live under the workspace root")
        .join("lang-v2");

    if crate_dir.exists() {
        fs::remove_dir_all(&crate_dir).unwrap();
    }
    fs::create_dir_all(crate_dir.join("idls")).unwrap();
    fs::create_dir_all(crate_dir.join("src")).unwrap();

    fs::write(
        crate_dir.join("Cargo.toml"),
        format!(
            r#"[package]
name = "declare-program-idl-errors"
version = "0.1.0"
edition = "2021"
publish = false

[features]
idl-build = ["anchor-lang/idl-build"]

[dependencies]
anchor-lang = {{ path = "{}" }}

[workspace]
"#,
            anchor_lang.display()
        ),
    )
    .unwrap();

    fs::write(
        crate_dir.join("idls/external.json"),
        r#"{
  "address": "11111111111111111111111111111111",
  "metadata": { "name": "external", "version": "0.1.0", "spec": "0.1.0" },
  "instructions": [],
  "accounts": [],
  "types": [],
  "errors": [{ "code": 6000, "name": "ExternalFailure", "msg": "external failure" }]
}"#,
    )
    .unwrap();

    fs::write(
        crate_dir.join("src/lib.rs"),
        r#"use anchor_lang::prelude::*;

declare_program!(external);

#[error_code]
pub enum LocalError {
    LocalFailure,
}

#[cfg(not(feature = "idl-build"))]
fn external_error_binding_is_available() -> anchor_lang::Error {
    external::error::ExternalError::ExternalFailure.into()
}
"#,
    )
    .unwrap();

    (crate_dir.clone(), crate_dir.join("target"))
}

fn run_cargo(crate_dir: &std::path::Path, target_dir: &std::path::Path, args: &[&str]) -> String {
    let manifest = crate_dir.join("Cargo.toml");
    let manifest = manifest.to_str().unwrap();
    let mut command_args = Vec::new();
    let mut manifest_added = false;
    for arg in args {
        if *arg == "--" && !manifest_added {
            command_args.extend(["--manifest-path", manifest]);
            manifest_added = true;
        }
        command_args.push(*arg);
    }
    if !manifest_added {
        command_args.extend(["--manifest-path", manifest]);
    }
    let output = Command::new("cargo")
        .args(command_args)
        .env("CARGO_TARGET_DIR", target_dir)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "cargo command failed\n\nstdout:\n{}\n\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).unwrap()
}

fn error_blocks(output: &str) -> Vec<String> {
    output
        .split("--- IDL begin errors ---\n")
        .skip(1)
        .map(|block| {
            block
                .split("\n--- IDL end errors ---")
                .next()
                .unwrap()
                .to_owned()
        })
        .collect()
}

#[test]
#[cfg_attr(
    miri,
    ignore = "spawns cargo and writes a temporary workspace; covered by normal cargo test"
)]
fn declare_program_errors_are_not_added_to_idl_build() {
    let (crate_dir, target_dir) = setup_case();

    run_cargo(&crate_dir, &target_dir, &["check", "--offline"]);
    let first = run_cargo(
        &crate_dir,
        &target_dir,
        &[
            "test",
            "__anchor_private_print_idl",
            "--features",
            "idl-build",
            "--offline",
            "--",
            "--show-output",
            "--quiet",
        ],
    );
    let second = run_cargo(
        &crate_dir,
        &target_dir,
        &[
            "test",
            "__anchor_private_print_idl",
            "--features",
            "idl-build",
            "--offline",
            "--",
            "--show-output",
            "--quiet",
        ],
    );

    let first_blocks = error_blocks(&first);
    let second_blocks = error_blocks(&second);
    assert_eq!(first_blocks, second_blocks);
    assert_eq!(
        first_blocks,
        vec![r#"[{"code":6000,"name":"LocalFailure"}]"#]
    );
    assert!(!first.contains("ExternalFailure"));
}
