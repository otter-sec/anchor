use std::{fs, path::PathBuf, process::Command};

fn cargo_case(
    name: &str,
    source: &str,
    command: &str,
    extra_args: &[&str],
) -> std::process::Output {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let crate_dir = manifest_dir.join("target/macro-diagnostics").join(name);
    // Keep sources isolated for clear per-case diagnostics, but share Cargo
    // artifacts across cases. The Rust test harness may invoke these helpers
    // concurrently; Cargo coordinates the target-directory lock and avoids
    // recompiling anchor-lang and its dependencies for every fixture.
    let target_dir = manifest_dir.join("target/macro-diagnostics-target");
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
anchor-lang = {{ path = "{}" }}

[features]
idl-build = []
extra = []
live = []

[workspace]
"#,
            manifest_dir.display()
        ),
    )
    .unwrap();
    fs::write(src_dir.join("lib.rs"), source).unwrap();

    Command::new("cargo")
        .env("CARGO_TARGET_DIR", target_dir)
        .arg(command)
        .arg("--offline")
        .arg("--manifest-path")
        .arg(crate_dir.join("Cargo.toml"))
        .args(extra_args)
        .output()
        .unwrap_or_else(|err| panic!("failed to run cargo {command} for {name}: {err}"))
}

fn compile_fail_case(name: &str, source: &str, snippets: &[&str]) {
    compile_fail_case_with_forbidden(name, source, snippets, &[]);
}

fn compile_fail_case_with_forbidden(
    name: &str,
    source: &str,
    snippets: &[&str],
    forbidden_snippets: &[&str],
) {
    let output = cargo_case(name, source, "check", &[]);

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
    for forbidden in forbidden_snippets {
        assert!(
            !stderr.contains(forbidden),
            "{name} stderr unexpectedly contained {forbidden:?}\n\nstderr:\n{stderr}"
        );
    }
}

fn compile_pass_case(name: &str, source: &str) {
    let output = cargo_case(name, source, "check", &[]);

    assert!(
        output.status.success(),
        "{name} failed to compile\n\nstdout:\n{}\n\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    );
}

fn cargo_test_pass_case(name: &str, source: &str, features: &[&str]) {
    let mut args = Vec::new();
    if !features.is_empty() {
        args.push("--features");
        args.push(features.join(",").leak());
    }

    let output = cargo_case(name, source, "test", &args);

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
fn raw_constraint_rejects_obvious_non_bool_literals() {
    compile_fail_case(
        "raw_constraint_non_bool",
        r#"
use anchor_lang::prelude::*;

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
use anchor_lang::prelude::*;

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
use anchor_lang::prelude::*;

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
use anchor_lang::InitSpace;

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
use anchor_lang::InitSpace;

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
fn init_space_rejects_wincode_field_overrides() {
    compile_fail_case(
        "init_space_wincode_skip",
        r#"
use anchor_lang::{AnchorDeserialize, AnchorSerialize, InitSpace};

#[derive(InitSpace, AnchorDeserialize, AnchorSerialize)]
pub struct Bad {
    #[wincode(skip)]
    pub skipped: u64,
    pub kept: u8,
}
"#,
        &[
            "#[derive(InitSpace)] does not support `#[wincode(skip)]` fields",
            "serialized layout",
        ],
    );

    compile_fail_case(
        "init_space_wincode_skip_default_val",
        r#"
use anchor_lang::{AnchorDeserialize, AnchorSerialize, InitSpace};

#[derive(InitSpace, AnchorDeserialize, AnchorSerialize)]
pub struct Bad {
    #[wincode(skip(default_val = 9))]
    pub skipped: u64,
    pub kept: u8,
}
"#,
        &[
            "#[derive(InitSpace)] does not support `#[wincode(skip)]` fields",
            "serialized layout",
        ],
    );

    compile_fail_case(
        "init_space_wincode_with",
        r#"
use anchor_lang::{AnchorDeserialize, AnchorSerialize, InitSpace};

#[derive(InitSpace, AnchorDeserialize, AnchorSerialize)]
pub struct Bad {
    #[wincode(with = "shim::ByteCodec")]
    pub packed: u64,
}

mod shim {
    pub struct ByteCodec;
}
"#,
        &[
            "#[derive(InitSpace)] does not support `#[wincode(with = ...)]` fields",
            "custom wincode codecs can change the serialized layout",
        ],
    );

    compile_fail_case(
        "init_space_wincode_tag_encoding",
        r#"
use anchor_lang::{AnchorDeserialize, AnchorSerialize, InitSpace};

#[derive(InitSpace, AnchorDeserialize, AnchorSerialize)]
#[wincode(tag_encoding = "u32")]
pub enum Bad {
    A([u8; 32]),
    B(u8),
}
"#,
        &[
            "#[derive(InitSpace)] does not support `#[wincode(tag_encoding = ...)]`",
            "1-byte enum discriminant",
        ],
    );
}

#[test]
#[cfg_attr(
    miri,
    ignore = "spawns cargo and writes temporary workspaces; covered by normal cargo test"
)]
fn idl_generation_rejects_wincode_field_overrides() {
    compile_fail_case(
        "idl_type_wincode_skip",
        r#"
use anchor_lang::{AnchorDeserialize, AnchorSerialize, IdlType};

#[derive(IdlType, AnchorDeserialize, AnchorSerialize)]
pub struct Bad {
    #[wincode(skip)]
    pub skipped: u64,
    pub kept: u8,
}
"#,
        &[
            "`#[derive(IdlType)]` does not support `#[wincode(skip)]` fields",
            "generated IDL would not match the serialized wire layout",
        ],
    );

    compile_fail_case(
        "account_borsh_wincode_skip",
        r#"
use anchor_lang::prelude::*;

declare_id!("11111111111111111111111111111111");

#[account(borsh)]
pub struct Bad {
    #[wincode(skip)]
    pub skipped: u64,
    pub kept: u8,
}
"#,
        &[
            "`#[account(borsh)]` does not support `#[wincode(skip)]` fields",
            "generated IDL would not match the serialized wire layout",
        ],
    );

    compile_fail_case(
        "event_wincode_with",
        r#"
use anchor_lang::prelude::*;

#[event]
pub struct Bad {
    #[wincode(with = "shim::ByteCodec")]
    pub packed: u64,
}

mod shim {
    pub struct ByteCodec;
}
"#,
        &[
            "`#[event]` does not support `#[wincode(with = ...)]` fields",
            "custom wincode codecs can change the serialized wire layout",
        ],
    );
}

#[test]
#[cfg_attr(
    miri,
    ignore = "spawns cargo and writes temporary workspaces; covered by normal cargo test"
)]
fn idl_generation_rejects_lossy_packed_repr_modifiers() {
    compile_fail_case(
        "event_bytemuck_packed_two",
        r#"
use anchor_lang::prelude::*;

#[event(bytemuck)]
#[repr(C, packed(2))]
pub struct Bad {
    pub tag: u16,
    pub wide: u64,
}
"#,
        &[
            "Anchor IDL only supports `#[repr(..., packed)]` or `#[repr(..., packed(1))]`",
            "lossy IDL layout",
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
use anchor_lang::prelude::*;

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
fn unknown_error_code_argument_is_rejected() {
    compile_fail_case(
        "unknown_error_code_argument",
        r#"
use anchor_lang::prelude::*;

#[error_code(unknown = 7000)]
pub enum MyError {
    Problem,
}
"#,
        &[
            "unknown `#[error_code]` argument `unknown`",
            "expected `offset = N`",
        ],
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
use anchor_lang::prelude::*;

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
fn instruction_args_reject_generated_name_namespace() {
    compile_fail_case(
        "reserved_instruction_arg",
        r#"
use anchor_lang::prelude::*;

#[derive(Accounts)]
#[instruction(__base_offset: usize)]
pub struct Bad {
    #[account(mut)]
    pub data: Option<UncheckedAccount>,
}
"#,
        &["instruction argument names beginning with `__` are reserved for generated code"],
    );
}

#[test]
#[cfg_attr(
    miri,
    ignore = "spawns cargo and writes temporary workspaces; covered by normal cargo test"
)]
fn close_target_cannot_also_be_closed() {
    compile_fail_case(
        "close_chain",
        r#"
use anchor_lang::prelude::*;

declare_id!("11111111111111111111111111111111");

#[account(borsh)]
pub struct Data {
    pub value: u64,
}

#[derive(Accounts)]
pub struct Bad {
    #[account(mut, close = receiver)]
    pub first: BorshAccount<Data>,
    #[account(mut, close = first)]
    pub second: BorshAccount<Data>,
    #[account(mut)]
    pub receiver: SystemAccount,
}
"#,
        &[
            "close target `first` is also scheduled to close",
            "close chains can revive an account that was already closed",
        ],
    );
}

#[test]
#[cfg_attr(
    miri,
    ignore = "spawns cargo and writes temporary workspaces; covered by normal cargo test"
)]
fn nested_accounts_reject_instruction_arguments() {
    compile_fail_case(
        "nested_instruction_args",
        r#"
use anchor_lang::prelude::*;

declare_id!("11111111111111111111111111111111");

#[program]
pub mod nested_instruction_args {
    use super::*;

    pub fn ix(_ctx: &mut Context<Outer>, amount: u64) -> Result<()> {
        let _ = amount;
        Ok(())
    }
}

#[derive(Accounts)]
#[instruction(amount: u8)]
pub struct Inner {
    #[account(constraint = amount == 0)]
    pub data: UncheckedAccount,
}

#[derive(Accounts)]
pub struct Outer {
    pub inner: Nested<Inner>,
}
"#,
        &["expected `()`, found `(u8,)`"],
    );
}

#[test]
#[cfg_attr(
    miri,
    ignore = "spawns cargo and writes temporary workspaces; covered by normal cargo test"
)]
fn cfg_gated_public_handlers_do_not_emit_missing_wrappers() {
    compile_pass_case(
        "cfg_gated_handler",
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
fn cfg_disabled_members_are_omitted_from_idl_and_error_codes() {
    cargo_test_pass_case(
        "cfg_filtered_idl",
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

#[test]
#[cfg_attr(
    miri,
    ignore = "spawns cargo and writes temporary workspaces; covered by normal cargo test"
)]
fn slot_hashes_is_not_a_supported_sysvar_account() {
    compile_fail_case(
        "unsupported_slot_hashes_sysvar",
        r#"
use anchor_lang::{accounts::Sysvar, pinocchio, AnchorAccount};

type SlotHashes = pinocchio::sysvars::slot_hashes::SlotHashes<&'static [u8]>;

fn assert_anchor_account<T: AnchorAccount>() {}

fn check() {
    assert_anchor_account::<Sysvar<SlotHashes>>();
}
"#,
        // `SysvarLoad` is the bound `Sysvar<T>` actually requires; the
        // `on_unimplemented` note names `SysvarId` alongside it.
        &[
            "SlotHashes",
            "SysvarLoad",
            "is not a sysvar Anchor can load",
        ],
    );
}

#[test]
#[cfg_attr(
    miri,
    ignore = "spawns cargo and writes temporary workspaces; covered by normal cargo test"
)]
fn qualified_accounts_paths_compile() {
    compile_pass_case(
        "qualified_accounts_paths",
        r#"
use anchor_lang::prelude::*;

declare_id!("11111111111111111111111111111111");

pub mod shared {
    use super::*;

    #[derive(Accounts)]
    pub struct Inner {
        pub signer: Signer,
    }

    #[derive(Accounts)]
    pub struct Outer {
        pub inner: Nested<crate::shared2::Leaf>,
    }
}

pub mod shared2 {
    use super::*;

    #[derive(Accounts)]
    pub struct Leaf {
        pub signer: Signer,
    }
}

#[program]
pub mod qualified_paths {
    use super::*;

    pub fn use_qualified_ctx(_ctx: &mut Context<shared::Inner>) -> Result<()> {
        Ok(())
    }

    pub fn use_nested(_ctx: &mut Context<shared::Outer>) -> Result<()> {
        Ok(())
    }
}
"#,
    );
}

#[test]
#[cfg_attr(
    miri,
    ignore = "spawns cargo and writes temporary workspaces; covered by normal cargo test"
)]
fn seeds_preserve_nontrivial_as_ref_receivers() {
    compile_pass_case(
        "seed_nontrivial_as_ref",
        r#"
use anchor_lang::prelude::*;

declare_id!("11111111111111111111111111111111");

#[derive(anchor_lang::AnchorDeserialize, anchor_lang::AnchorSerialize)]
pub struct SeedBuf(Vec<u8>);

impl SeedBuf {
    pub fn as_ref(&self) -> &[u8] {
        self.0.as_slice()
    }
}

#[derive(anchor_lang::AnchorDeserialize, anchor_lang::AnchorSerialize)]
pub struct SeedConfig {
    pub seed: SeedBuf,
}

impl Owner for SeedConfig {
    const OWNER: Address = crate::ID;
}

impl Discriminator for SeedConfig {
    const DISCRIMINATOR: &'static [u8] = &[0x63, 0x66, 0x67, 0x2d, 0x73, 0x65, 0x65, 0x64];
}

#[derive(Accounts)]
pub struct Good {
    pub config: BorshAccount<SeedConfig>,
    #[account(seeds = [config.seed.as_ref()], bump)]
    pub target: UncheckedAccount,
}
"#,
    );
}

#[test]
#[cfg_attr(
    miri,
    ignore = "spawns cargo and writes temporary workspaces; covered by normal cargo test"
)]
fn init_opaque_seed_expressions_keep_bump_bytes_alive() {
    compile_pass_case(
        "init_opaque_seed_expr",
        r#"
use anchor_lang::prelude::*;

declare_id!("11111111111111111111111111111111");

#[derive(anchor_lang::AnchorDeserialize, anchor_lang::AnchorSerialize)]
pub struct Data {
    pub value: u64,
}

impl Owner for Data {
    const OWNER: Address = crate::ID;
}

impl Discriminator for Data {
    const DISCRIMINATOR: &'static [u8] = &[0x64, 0x61, 0x74, 0x61, 0x2d, 0x62, 0x6f, 0x72];
}

pub struct SeedBundle<'a>([&'a [u8]; 1]);

impl<'a> SeedBundle<'a> {
    pub fn for_payer(payer: &'a [u8]) -> Self {
        Self([payer])
    }
}

impl<'a> AsRef<[&'a [u8]]> for SeedBundle<'a> {
    fn as_ref(&self) -> &[&'a [u8]] {
        &self.0
    }
}

#[derive(Accounts)]
pub struct Good {
    #[account(mut)]
    pub payer: Signer,
    #[account(
        init,
        payer = payer,
        space = 8 + core::mem::size_of::<Data>(),
        seeds = SeedBundle::for_payer(payer.address().as_ref()),
        bump
    )]
    pub data: BorshAccount<Data>,
    pub system_program: Program<System>,
}
"#,
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
use anchor_lang::prelude::*;

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

    compile_fail_case(
        "init_payer_optional_account_is_rejected",
        r#"
use anchor_lang::prelude::*;

declare_id!("11111111111111111111111111111111");

#[account]
pub struct Data {
    pub value: u64,
}

#[derive(Accounts)]
pub struct Bad {
    #[account(init, payer = payer, space = 8 + core::mem::size_of::<Data>())]
    pub data: Account<Data>,
    #[account(mut)]
    pub payer: Option<SystemAccount>,
    pub system_program: Program<System>,
}
"#,
        &["optional accounts cannot be used as init payers"],
    );
}

#[test]
#[cfg_attr(
    miri,
    ignore = "spawns cargo and writes temporary workspaces; covered by normal cargo test"
)]
fn missing_init_and_realloc_payers_are_diagnosed() {
    compile_fail_case_with_forbidden(
        "missing_init_payer",
        r#"
use anchor_lang::prelude::*;

declare_id!("11111111111111111111111111111111");

#[account]
pub struct Data {
    pub value: u64,
}

#[derive(Accounts)]
pub struct MissingInitPayer {
    #[account(init, space = 8 + core::mem::size_of::<Data>())]
    pub data: Account<Data>,
    pub system_program: Program<System>,
}
"#,
        &["`init` and `init_if_needed` require `payer`"],
        &["proc-macro derive panicked"],
    );

    compile_fail_case_with_forbidden(
        "missing_realloc_payer",
        r#"
use anchor_lang::prelude::*;

declare_id!("11111111111111111111111111111111");

#[account]
pub struct Data {
    pub value: u64,
}

#[derive(Accounts)]
pub struct MissingReallocPayer {
    #[account(mut)]
    pub payer: Signer,
    #[account(mut, realloc = 16, realloc_zero = false)]
    pub data: Account<Data>,
    pub system_program: Program<System>,
}
"#,
        &["`realloc` requires `realloc_payer`"],
        &["proc-macro derive panicked"],
    );
}

#[test]
#[cfg_attr(
    miri,
    ignore = "spawns cargo and writes temporary workspaces; covered by normal cargo test"
)]
fn realloc_payer_cannot_be_optional() {
    compile_fail_case(
        "realloc_payer_optional_account_is_rejected",
        r#"
use anchor_lang::prelude::*;

declare_id!("11111111111111111111111111111111");

#[account]
pub struct Data {
    pub value: u64,
}

#[derive(Accounts)]
pub struct Bad {
    #[account(mut, realloc = 16, realloc_payer = payer, realloc_zero = false)]
    pub data: Account<Data>,
    #[account(mut)]
    pub payer: Option<SystemAccount>,
}
"#,
        &[
            "optional accounts cannot be used as realloc payers",
            "realloc_payer",
        ],
    );
}

#[test]
#[cfg_attr(
    miri,
    ignore = "spawns cargo and writes temporary workspaces; covered by normal cargo test"
)]
fn pda_init_payer_must_be_system_account() {
    compile_fail_case(
        "pda_init_payer_must_be_system_account",
        r#"
use anchor_lang::prelude::*;

#[account]
pub struct Data {
    pub value: u64,
}

#[derive(Accounts)]
pub struct Bad {
    #[account(mut, seeds = [b"payer"], bump)]
    pub payer: UncheckedAccount,
    #[account(init, payer = payer, space = 8 + core::mem::size_of::<Data>())]
    pub data: Account<Data>,
    pub system_program: Program<System>,
}
"#,
        &["PDA init payers must be declared as `SystemAccount`"],
    );
}

#[test]
#[cfg_attr(
    miri,
    ignore = "spawns cargo and writes temporary workspaces; covered by normal cargo test"
)]
fn init_and_init_if_needed_reject_seeds_program() {
    compile_fail_case(
        "init_seeds_program_rejected",
        r#"
use anchor_lang::prelude::*;

declare_id!("11111111111111111111111111111111");

pub const OTHER_PROGRAM: Address =
    Address::from_str_const("Gue5TpR6sstSyGhSvmVeH2TeKqBYYqmXpRCacB9jAk8u");

#[account]
pub struct Data {
    pub value: u64,
}

#[derive(Accounts)]
pub struct Bad {
    #[account(
        init,
        payer = payer,
        space = 8 + core::mem::size_of::<Data>(),
        seeds = [b"data"],
        bump,
        seeds::program = OTHER_PROGRAM,
    )]
    pub data: Account<Data>,
    #[account(mut)]
    pub payer: Signer,
    pub system_program: Program<System>,
}
"#,
        &["`seeds::program` cannot be used with `init`"],
    );

    compile_fail_case(
        "init_if_needed_seeds_program_rejected",
        r#"
use anchor_lang::prelude::*;

declare_id!("11111111111111111111111111111111");

pub const OTHER_PROGRAM: Address =
    Address::from_str_const("Gue5TpR6sstSyGhSvmVeH2TeKqBYYqmXpRCacB9jAk8u");

#[account]
pub struct Data {
    pub value: u64,
}

#[derive(Accounts)]
pub struct Bad {
    #[account(
        init_if_needed,
        payer = payer,
        space = 8 + core::mem::size_of::<Data>(),
        seeds = [b"data"],
        bump,
        seeds::program = OTHER_PROGRAM,
    )]
    pub data: Account<Data>,
    #[account(mut)]
    pub payer: Signer,
    pub system_program: Program<System>,
}
"#,
        &["`seeds::program` cannot be used with `init_if_needed`"],
    );
}

#[test]
#[cfg_attr(
    miri,
    ignore = "spawns cargo and writes temporary workspaces; covered by normal cargo test"
)]
fn seeds_program_requires_seeds_and_rejects_duplicates() {
    compile_fail_case(
        "seeds_program_without_seeds",
        r#"
use anchor_lang::prelude::*;

declare_id!("11111111111111111111111111111111");

pub const OTHER_PROGRAM: Address =
    Address::from_str_const("Gue5TpR6sstSyGhSvmVeH2TeKqBYYqmXpRCacB9jAk8u");

#[derive(Accounts)]
pub struct Bad {
    #[account(seeds::program = OTHER_PROGRAM)]
    pub data: UncheckedAccount,
}
"#,
        &["seeds must be provided before seeds::program"],
    );

    compile_fail_case(
        "duplicate_seeds_program_rejected",
        r#"
use anchor_lang::prelude::*;

declare_id!("11111111111111111111111111111111");

pub const OTHER_PROGRAM_A: Address =
    Address::from_str_const("Gue5TpR6sstSyGhSvmVeH2TeKqBYYqmXpRCacB9jAk8u");
pub const OTHER_PROGRAM_B: Address =
    Address::from_str_const("HmbTQ4MSEFuTMdM7x5TW5tsanTzQKB8CS7QdQ8qJbYQL");

#[derive(Accounts)]
pub struct Bad {
    #[account(
        seeds = [b"data"],
        bump,
        seeds::program = OTHER_PROGRAM_A,
        seeds::program = OTHER_PROGRAM_B,
    )]
    pub data: UncheckedAccount,
}
"#,
        &["`seeds::program` already provided"],
    );
}

#[test]
#[cfg_attr(
    miri,
    ignore = "spawns cargo and writes temporary workspaces; covered by normal cargo test"
)]
fn optional_pda_init_payer_is_rejected() {
    compile_fail_case(
        "optional_pda_init_payer_is_rejected",
        r#"
use anchor_lang::prelude::*;

#[account]
pub struct Data {
    pub value: u64,
}

#[derive(Accounts)]
pub struct Bad {
    #[account(mut, seeds = [b"payer"], bump)]
    pub payer: Option<SystemAccount>,
    #[account(init, payer = payer, space = 8 + core::mem::size_of::<Data>())]
    pub data: Account<Data>,
    pub system_program: Program<System>,
}
"#,
        &["optional accounts cannot be used as init payers"],
    );
}

#[test]
#[cfg_attr(
    miri,
    ignore = "spawns cargo and writes temporary workspaces; covered by normal cargo test"
)]
fn close_on_unchecked_account_is_rejected() {
    compile_fail_case(
        "close_on_unchecked_account",
        r#"
use anchor_lang::prelude::*;

declare_id!("11111111111111111111111111111111");

#[derive(Accounts)]
pub struct Bad {
    #[account(mut, close = receiver)]
    pub raw: UncheckedAccount,
    #[account(mut)]
    pub receiver: SystemAccount,
}
"#,
        &[
            "`#[account(close = ...)]` is not supported on `UncheckedAccount`",
            "close the raw account manually",
        ],
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
use anchor_lang::prelude::*;

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

    case("init_owner_override_account", "#[account]", "Account<Data>");
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
