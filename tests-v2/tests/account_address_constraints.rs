use std::{fs, path::PathBuf, process::Command};

#[test]
fn account_address_constraint_fixture_links() {
    let _ = account_address_constraints::ID;
}

#[test]
#[cfg_attr(
    miri,
    ignore = "spawns cargo and writes a temporary workspace; covered by normal cargo test"
)]
fn idl_build_resolves_static_addresses_and_omits_unsupported_exprs() {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let lang_v2 = manifest_dir
        .parent()
        .expect("tests-v2 lives under the workspace root")
        .join("lang-v2");
    let crate_dir = manifest_dir.join("target/address-idl-surface");
    let src_dir = crate_dir.join("src");
    fs::create_dir_all(&src_dir).unwrap();
    fs::write(
        crate_dir.join("Cargo.toml"),
        format!(
            r#"[package]
name = "address-idl-surface"
version = "0.1.0"
edition = "2021"
publish = false

[features]
idl-build = []

[dependencies]
anchor-lang = {{ path = "{}" }}

[workspace]
"#,
            lang_v2.display()
        ),
    )
    .unwrap();
    fs::write(
        src_dir.join("lib.rs"),
        r#"
use anchor_lang::prelude::*;

declare_id!("11111111111111111111111111111111");

const EXPECTED_PROGRAM: anchor_lang::Address =
    anchor_lang::address!("TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA");

#[account]
pub struct Holder {
    pub expected_program: anchor_lang::Address,
}

fn choose_program(data: &Holder) -> anchor_lang::Address {
    data.expected_program
}

#[derive(Accounts)]
pub struct StaticAddress {
    #[account(address = EXPECTED_PROGRAM)]
    pub program: UncheckedAccount,
}

#[derive(Accounts)]
pub struct DynamicAddress {
    pub data: Account<Holder>,
    #[account(address = data.expected_program)]
    pub program: UncheckedAccount,
}

#[derive(Accounts)]
pub struct UnsupportedAddress {
    pub data: Account<Holder>,
    #[account(address = choose_program(&data))]
    pub program: UncheckedAccount,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn static_addresses_emit_base58() {
        let json = StaticAddress::__idl_accounts();
        assert!(json.contains("\"address\":\"TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA\""));
        assert!(!json.contains("EXPECTED_PROGRAM"));
    }

    #[test]
    fn dotted_paths_remain_client_hints() {
        let json = DynamicAddress::__idl_accounts();
        assert!(json.contains("\"address\":\"data.expected_program\""));
    }

    #[test]
    fn unsupported_runtime_expressions_are_omitted() {
        let json = UnsupportedAddress::__idl_accounts();
        assert!(!json.contains("\"address\":"));
        assert!(!json.contains("choose_program"));
    }
}
"#,
    )
    .unwrap();

    let output = Command::new("cargo")
        .args(["test", "--offline", "--manifest-path"])
        .arg(crate_dir.join("Cargo.toml"))
        .args(["--features", "idl-build"])
        .output()
        .unwrap_or_else(|err| panic!("failed to run cargo test for address-idl-surface: {err}"));

    assert!(
        output.status.success(),
        "address-idl-surface failed:\nstdout:\n{}\n\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    );
}
