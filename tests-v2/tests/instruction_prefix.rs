use std::{fs, path::PathBuf, process::Command};

fn compile_pass_case(name: &str, source: &str) {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let crate_dir = manifest_dir.join("target/compile-cases").join(name);
    let src_dir = crate_dir.join("src");
    let anchor_lang = manifest_dir
        .parent()
        .expect("tests-v2 should live under the workspace root")
        .join("lang-v2");

    if crate_dir.exists() {
        fs::remove_dir_all(&crate_dir).unwrap();
    }
    fs::create_dir_all(&src_dir).unwrap();

    fs::write(
        crate_dir.join("Cargo.toml"),
        format!(
            r#"[package]
name = "{name}"
version = "0.1.0"
edition = "2021"
publish = false

[dependencies]
anchor-lang = {{ path = "{}" }}

[workspace]
"#,
            anchor_lang.display()
        ),
    )
    .unwrap();
    fs::write(src_dir.join("lib.rs"), source).unwrap();

    let output = Command::new("cargo")
        .args(["check", "--offline", "--manifest-path"])
        .arg(crate_dir.join("Cargo.toml"))
        .output()
        .unwrap_or_else(|err| panic!("failed to run cargo check for {name}: {err}"));

    assert!(
        output.status.success(),
        "{name} failed to compile\n\nstdout:\n{}\n\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    );
}

#[test]
#[cfg_attr(
    miri,
    ignore = "spawns cargo and writes temporary workspaces; covered by normal cargo test"
)]
fn instruction_attr_keeps_declared_prefix_tuple() {
    compile_pass_case(
        "tests_v2_instruction_prefix",
        r#"
#![allow(dead_code)]

use anchor_lang::{prelude::*, TryAccounts};

declare_id!("11111111111111111111111111111111");

#[program]
pub mod prefix_only_instruction_args {
    use super::*;

    pub fn ix(ctx: &mut Context<PrefixOnly>, amount: u64, step: i32) -> Result<()> {
        let _ = ctx;
        let _ = amount;
        let _ = step;
        Ok(())
    }
}

#[derive(Accounts)]
pub struct NoArgs {
    pub account: UncheckedAccount,
}

#[derive(Accounts)]
#[instruction(amount: u64, step: i32)]
pub struct WithArgs {
    pub account: UncheckedAccount,
}

#[derive(Accounts)]
#[instruction(amount: u64)]
pub struct PrefixOnly {
    pub account: UncheckedAccount,
}

fn _no_args_maps_to_unit() {
    let _: <NoArgs as TryAccounts>::IxArgs<'static> = ();
}

fn _instruction_args_are_tuple<'a>(args: <WithArgs as TryAccounts>::IxArgs<'a>) -> (u64, i32) {
    args
}

fn _instruction_args_keep_declared_prefix<'a>(
    args: <PrefixOnly as TryAccounts>::IxArgs<'a>,
) -> (u64,) {
    args
}
"#,
    );
}
