use std::{fs, path::PathBuf, process::Command};

fn compile_fail_case(name: &str, source: &str, snippets: &[&str]) {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let crate_dir = manifest_dir.join("target/macro-diagnostics").join(name);
    let src_dir = crate_dir.join("src");
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
anchor-lang-v2 = {{ path = "{}" }}
wincode = {{ version = "0.5", features = ["derive"] }}

[workspace]
"#,
            manifest_dir.display()
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
        !output.status.success(),
        "{name} unexpectedly compiled successfully"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    for snippet in snippets {
        assert!(
            stderr.contains(snippet),
            "{name} stderr did not contain {snippet:?}\n\nstderr:\n{stderr}"
        );
    }
}

#[test]
#[cfg_attr(
    miri,
    ignore = "spawns cargo and writes temporary workspaces; covered by normal cargo test"
)]
fn raw_constraint_rejects_obvious_non_bool_literals() {
    compile_fail_case(
        "raw_constraint_non_bool",
        r#"
use anchor_lang_v2::prelude::*;

#[derive(Accounts)]
pub struct Bad {
    #[account(constraint = "hello")]
    pub data: UncheckedAccount,
}
"#,
        &[
            "`constraint` expects a boolean expression",
            "non-boolean literals",
        ],
    );
}

#[test]
#[cfg_attr(
    miri,
    ignore = "spawns cargo and writes temporary workspaces; covered by normal cargo test"
)]
fn invalid_account_arguments_are_targeted() {
    compile_fail_case(
        "invalid_account_argument",
        r#"
use anchor_lang_v2::prelude::*;

#[derive(Accounts)]
pub struct Bad {
    #[account(singler)]
    pub data: UncheckedAccount,
}
"#,
        &["unknown account constraint `singler`"],
    );
}

#[test]
#[cfg_attr(
    miri,
    ignore = "spawns cargo and writes temporary workspaces; covered by normal cargo test"
)]
fn unsafe_dup_constraint_has_targeted_message() {
    compile_fail_case(
        "unsafe_dup_required",
        r#"
use anchor_lang_v2::prelude::*;

#[derive(Accounts)]
pub struct Bad {
    #[account(dup)]
    pub data: UncheckedAccount,
}
"#,
        &[
            "`dup` bypasses duplicate-account safety checks",
            "unsafe(dup)",
        ],
    );
}

#[test]
#[cfg_attr(
    miri,
    ignore = "spawns cargo and writes temporary workspaces; covered by normal cargo test"
)]
fn init_space_rejects_union_and_unsized_reference_fields() {
    compile_fail_case(
        "init_space_union",
        r#"
use anchor_lang_v2::InitSpace;

#[derive(Copy, Clone, InitSpace)]
union Bad {
    value: u64,
}
"#,
        &["#[derive(InitSpace)] only supports structs and enums"],
    );

    compile_fail_case(
        "init_space_reference",
        r#"
use anchor_lang_v2::InitSpace;

#[derive(InitSpace)]
pub struct Bad<'a> {
    pub name: &'a str,
}
"#,
        &[
            "#[derive(InitSpace)] can't compute size for this type",
            "fixed-size alternative",
        ],
    );
}

#[test]
#[cfg_attr(
    miri,
    ignore = "spawns cargo and writes temporary workspaces; covered by normal cargo test"
)]
fn malformed_discriminator_attribute_has_targeted_message() {
    compile_fail_case(
        "bad_discriminator",
        r#"
use anchor_lang_v2::prelude::*;

declare_id!("11111111111111111111111111111111");

#[program]
pub mod bad_discriminator {
    use super::*;

    #[discrim = "bad"]
    pub fn ix(_ctx: &mut Context<Noop>) -> Result<()> {
        Ok(())
    }
}

#[derive(Accounts)]
pub struct Noop {}
"#,
        &["`#[discrim = ...]` value must be an integer literal or byte array literal"],
    );
}

#[test]
#[cfg_attr(
    miri,
    ignore = "spawns cargo and writes temporary workspaces; covered by normal cargo test"
)]
fn instruction_args_must_match_zero_arg_handler() {
    compile_fail_case(
        "instruction_args_without_handler_args",
        r#"
use anchor_lang_v2::prelude::*;

declare_id!("11111111111111111111111111111111");

#[program]
pub mod instruction_args_without_handler_args {
    use super::*;

    pub fn ix(ctx: &mut Context<Bad>) -> Result<()> {
        let _ = ctx;
        Ok(())
    }
}

#[derive(Accounts)]
#[instruction(value: u64)]
pub struct Bad {
    #[account(constraint = value > 0)]
    pub data: UncheckedAccount,
}
"#,
        &["expected `()`, found `(u64,)`"],
    );
}

#[test]
#[cfg_attr(
    miri,
    ignore = "spawns cargo and writes temporary workspaces; covered by normal cargo test"
)]
fn slot_hashes_is_not_a_supported_sysvar_account() {
    compile_fail_case(
        "unsupported_slot_hashes_sysvar",
        r#"
use anchor_lang_v2::{accounts::Sysvar, pinocchio, AnchorAccount};

type SlotHashes = pinocchio::sysvars::slot_hashes::SlotHashes<&'static [u8]>;

fn assert_anchor_account<T: AnchorAccount>() {}

fn check() {
    assert_anchor_account::<Sysvar<SlotHashes>>();
}
"#,
        &["SlotHashes", "SysvarId"],
    );
}

#[test]
#[cfg_attr(
    miri,
    ignore = "spawns cargo and writes temporary workspaces; covered by normal cargo test"
)]
fn init_payer_must_be_mutable() {
    compile_fail_case(
        "init_payer_must_be_mutable",
        r#"
use anchor_lang_v2::prelude::*;

declare_id!("11111111111111111111111111111111");

#[account]
pub struct Data {
    pub value: u64,
}

#[derive(Accounts)]
pub struct Bad {
    #[account(init, payer = payer, space = 8 + core::mem::size_of::<Data>())]
    pub data: Account<Data>,
    pub payer: Signer,
    pub system_program: Program<System>,
}
"#,
        &["the payer specified for an init constraint must be mutable"],
    );
}

#[test]
#[cfg_attr(
    miri,
    ignore = "spawns cargo and writes temporary workspaces; covered by normal cargo test"
)]
fn init_owner_override_rejects_typed_accounts() {
    fn case(name: &str, account_attr: &str, field_ty: &str) {
        let source = format!(
            r#"
use anchor_lang_v2::prelude::*;

declare_id!("11111111111111111111111111111111");

pub const OTHER_PROGRAM: Address =
    Address::from_str_const("Gue5TpR6sstSyGhSvmVeH2TeKqBYYqmXpRCacB9jAk8u");

{account_attr}
pub struct Data {{
    pub value: u64,
}}

#[derive(Accounts)]
pub struct Bad {{
    #[account(mut)]
    pub payer: Signer,
    #[account(
        init,
        payer = payer,
        space = 8 + core::mem::size_of::<Data>(),
        owner = OTHER_PROGRAM,
    )]
    pub data: {field_ty},
    pub system_program: Program<System>,
}}
"#
        );
        compile_fail_case(name, &source, &["ForeignOwnerInit", "is not implemented"]);
    }

    case(
        "init_owner_override_account",
        "#[account]",
        "Account<Data>",
    );
    case(
        "init_owner_override_boxed_account",
        "#[account]",
        "Box<Account<Data>>",
    );
    case(
        "init_owner_override_interface_account",
        "#[account]",
        "InterfaceAccount<Data>",
    );
    case(
        "init_owner_override_borsh_account",
        "#[account(borsh)]",
        "BorshAccount<Data>",
    );
    case(
        "init_owner_override_boxed_borsh_account",
        "#[account(borsh)]",
        "Box<BorshAccount<Data>>",
    );
}
