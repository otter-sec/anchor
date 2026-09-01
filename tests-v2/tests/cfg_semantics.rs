use std::{fs, path::PathBuf, process::Command};

fn cargo_case(name: &str, source: &str, command: &str, features: &[&str]) -> std::process::Output {
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

[features]
idl-build = []
extra = []
live = []

[workspace]
"#,
            anchor_lang.display()
        ),
    )
    .unwrap();
    fs::write(src_dir.join("lib.rs"), source).unwrap();

    let mut cargo = Command::new("cargo");
    cargo.args([command, "--offline", "--manifest-path"]);
    cargo.arg(crate_dir.join("Cargo.toml"));
    if !features.is_empty() {
        cargo.arg("--features");
        cargo.arg(features.join(","));
    }
    cargo
        .output()
        .unwrap_or_else(|err| panic!("failed to run cargo {command} for {name}: {err}"))
}

fn compile_pass_case(name: &str, source: &str) {
    compile_pass_case_with_features(name, source, &[]);
}

fn compile_pass_case_with_features(name: &str, source: &str, features: &[&str]) {
    let output = cargo_case(name, source, "check", features);
    assert!(
        output.status.success(),
        "{name} failed to compile\n\nstdout:\n{}\n\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    );
}

fn cargo_test_pass_case(name: &str, source: &str, features: &[&str]) {
    let output = cargo_case(name, source, "test", features);
    assert!(
        output.status.success(),
        "{name} tests failed\n\nstdout:\n{}\n\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    );
}

#[test]
#[cfg_attr(
    miri,
    ignore = "spawns cargo and writes temporary workspaces; covered by normal cargo test"
)]
fn cfg_gated_public_handlers_compile_without_wrapper_errors() {
    compile_pass_case(
        "tests_v2_cfg_gated_handler",
        r#"
use anchor_lang::prelude::*;

declare_id!("11111111111111111111111111111111");

#[program]
pub mod gated_program {
    use super::*;

    #[cfg(feature = "live")]
    pub fn live(_ctx: &mut Context<Noop>) -> Result<()> {
        Ok(())
    }
}

#[derive(Accounts)]
pub struct Noop {}
"#,
    );
}

#[test]
#[cfg_attr(
    miri,
    ignore = "spawns cargo and writes temporary workspaces; covered by normal cargo test"
)]
fn cfg_attr_inline_policy_compiles_in_both_branches() {
    let source = r#"
use anchor_lang::prelude::*;

declare_id!("11111111111111111111111111111111");

#[program]
pub mod conditional_inline {
    use super::*;

    #[cfg_attr(feature = "live", inline(never))]
    #[cfg_attr(not(feature = "live"), inline(always))]
    pub fn run(_ctx: &mut Context<Noop>) -> Result<()> {
        Ok(())
    }
}

#[derive(Accounts)]
pub struct Noop {}
"#;

    compile_pass_case("tests_v2_cfg_attr_inline_default", source);
    compile_pass_case_with_features("tests_v2_cfg_attr_inline_never", source, &["live"]);
}

#[test]
#[cfg_attr(
    miri,
    ignore = "spawns cargo and writes temporary workspaces; covered by normal cargo test"
)]
fn cfg_disabled_members_drop_out_of_idl_and_error_codes() {
    cargo_test_pass_case(
        "tests_v2_cfg_filtered_idl",
        r#"
use anchor_lang::prelude::*;

declare_id!("11111111111111111111111111111111");

#[account]
pub struct CfgAccount {
    pub always: u64,
    #[cfg(feature = "extra")]
    pub hidden: u64,
}

#[event]
pub struct CfgEvent {
    pub always: u64,
    #[cfg(feature = "extra")]
    pub hidden: u64,
}

#[error_code]
pub enum CfgError {
    First,
    #[cfg(feature = "extra")]
    Hidden,
    Second,
}

#[cfg(all(test, feature = "idl-build"))]
mod tests {
    use super::*;

    #[test]
    fn cfg_disabled_members_are_filtered() {
        let account_json = <CfgAccount as IdlAccountType>::__idl_type_def().unwrap();
        assert!(account_json.contains("\"always\""));
        assert!(!account_json.contains("\"hidden\""));

        let event_json = <CfgEvent as IdlAccountType>::__idl_type_def().unwrap();
        assert!(event_json.contains("\"always\""));
        assert!(!event_json.contains("\"hidden\""));

        let errors_json = CfgError::__idl_errors();
        assert!(errors_json.contains("\"name\":\"First\""));
        assert!(errors_json.contains("\"name\":\"Second\""));
        assert!(!errors_json.contains("Hidden"));
        assert!(errors_json.contains("\"code\":6000"));
        assert!(errors_json.contains("\"code\":6001"));
        assert!(!errors_json.contains("\"code\":6002"));
    }
}
"#,
        &["idl-build"],
    );
}
