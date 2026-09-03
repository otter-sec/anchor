use std::{fs, path::PathBuf, process::Command};

#[test]
#[cfg_attr(
    miri,
    ignore = "spawns cargo and writes a temporary workspace; covered by normal cargo test"
)]
fn idl_build_resolves_static_address_constraints() {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
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
            manifest_dir.display()
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
