use std::{fs, path::PathBuf, process::Command};

fn compile_fail_case(name: &str, source: &str, snippets: &[&str]) {
    compile_fail_case_with_features(name, source, &[], snippets);
}

fn compile_fail_case_with_features(name: &str, source: &str, features: &[&str], snippets: &[&str]) {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let crate_dir = manifest_dir.join("target/compile-fail").join(name);
    let src_dir = crate_dir.join("src");
    fs::create_dir_all(&src_dir).unwrap();

    let anchor_lang_v2 = manifest_dir
        .parent()
        .expect("tests-v2 should live under the workspace root")
        .join("lang-v2");

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

[features]
cpi = []

[workspace]
"#,
            anchor_lang_v2.display()
        ),
    )
    .unwrap();
    fs::write(src_dir.join("lib.rs"), source).unwrap();

    let mut command = Command::new("cargo");
    command.args(["check", "--offline", "--manifest-path"]);
    command.arg(crate_dir.join("Cargo.toml"));
    if !features.is_empty() {
        command.arg("--features");
        command.arg(features.join(","));
    }
    let output = command
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

fn compile_pass_case(name: &str, source: &str, features: &[&str]) {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let crate_dir = manifest_dir.join("target/compile-pass").join(name);
    let src_dir = crate_dir.join("src");
    fs::create_dir_all(&src_dir).unwrap();

    let anchor_lang_v2 = manifest_dir
        .parent()
        .expect("tests-v2 should live under the workspace root")
        .join("lang-v2");

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

[features]
cpi = []

[workspace]
"#,
            anchor_lang_v2.display()
        ),
    )
    .unwrap();
    fs::write(src_dir.join("lib.rs"), source).unwrap();

    let mut command = Command::new("cargo");
    command.args(["check", "--offline", "--manifest-path"]);
    command.arg(crate_dir.join("Cargo.toml"));
    if !features.is_empty() {
        command.arg("--features");
        command.arg(features.join(","));
    }
    let output = command
        .output()
        .unwrap_or_else(|err| panic!("failed to run cargo check for {name}: {err}"));

    assert!(
        output.status.success(),
        "{name} did not compile successfully\n\nstderr:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn program_interface_mode_compiles_client_and_cpi_surface() {
    compile_pass_case(
        "program_interface_mode",
        r#"
use anchor_lang_v2::prelude::*;

declare_id!("11111111111111111111111111111111");

pub mod declared {
    use super::*;

    pub const ID: Address =
        Address::from_str_const("Con9ukTn9BRPXWcjS2UBbuN3NnCwy1hcaDNZ9Hb8QMNp");

    #[derive(Accounts)]
    pub struct Invoke {
        #[account(signer)]
        pub authority: Signer,
        pub data: UncheckedAccount,
    }

    #[program(interface, program_id = ID)]
    pub mod program {
        use super::*;

        #[discrim = [1, 2, 3, 4]]
        pub fn invoke(ctx: &mut Context<Invoke>, amount: u64) -> Result<()> {
            let _ = (ctx, amount);
            unreachable!()
        }
    }
}

pub fn build_ix(authority: Address, data: Address) -> anchor_lang_v2::solana_program::instruction::Instruction {
    let accounts = declared::accounts::Invoke { authority, data };
    declared::instruction::Invoke { amount: 5 }.to_instruction(accounts)
}

#[cfg(feature = "cpi")]
pub fn build_cpi<'a>(
    program: &'a Address,
    authority: CpiHandle<'a>,
    data: CpiHandle<'a>,
) {
    let accounts = declared::cpi::accounts::Invoke { authority, data };
    let ctx = CpiContext::new(program, accounts);
    declared::cpi::invoke(ctx, 5);
}
"#,
        &["cpi"],
    );
}

#[test]
fn program_interface_cpi_rejects_optional_accounts_clearly() {
    compile_fail_case_with_features(
        "program_interface_optional_cpi",
        r#"
use anchor_lang_v2::prelude::*;

declare_id!("11111111111111111111111111111111");
const EXTERNAL_ID: Address =
    Address::from_str_const("Con9ukTn9BRPXWcjS2UBbuN3NnCwy1hcaDNZ9Hb8QMNp");

#[derive(Accounts)]
pub struct Maybe {
    pub required: UncheckedAccount,
    pub optional: Option<UncheckedAccount>,
}

#[program(interface, program_id = EXTERNAL_ID)]
pub mod program_interface_optional_cpi {
    use super::*;

    #[discrim = [1]]
    pub fn maybe(ctx: &mut Context<Maybe>) -> Result<()> {
        let _ = ctx;
        unreachable!()
    }
}
"#,
        &["cpi"],
        &["CPI account generation for `Option<_>` accounts is not supported yet"],
    );
}

#[test]
fn program_interface_rejects_empty_discriminator() {
    compile_fail_case(
        "program_interface_empty_discriminator",
        r#"
use anchor_lang_v2::prelude::*;

declare_id!("11111111111111111111111111111111");
const EXTERNAL_ID: Address =
    Address::from_str_const("Con9ukTn9BRPXWcjS2UBbuN3NnCwy1hcaDNZ9Hb8QMNp");

#[derive(Accounts)]
pub struct Empty {}

#[program(interface, program_id = EXTERNAL_ID)]
pub mod program_interface_empty_discriminator {
    use super::*;

    #[discrim = []]
    pub fn ix(ctx: &mut Context<Empty>) -> Result<()> {
        let _ = ctx;
        Ok(())
    }
}
"#,
        &["must contain at least one byte"],
    );
}

#[test]
fn program_interface_rejects_duplicate_discriminators() {
    compile_fail_case(
        "program_interface_duplicate_discriminator",
        r#"
use anchor_lang_v2::prelude::*;

declare_id!("11111111111111111111111111111111");
const EXTERNAL_ID: Address =
    Address::from_str_const("Con9ukTn9BRPXWcjS2UBbuN3NnCwy1hcaDNZ9Hb8QMNp");

#[derive(Accounts)]
pub struct Empty {}

#[program(interface, program_id = EXTERNAL_ID)]
pub mod program_interface_duplicate_discriminator {
    use super::*;

    #[discrim = [1, 2, 3]]
    pub fn first(ctx: &mut Context<Empty>) -> Result<()> {
        let _ = ctx;
        Ok(())
    }

    #[discrim = [1, 2, 3]]
    pub fn second(ctx: &mut Context<Empty>) -> Result<()> {
        let _ = ctx;
        Ok(())
    }
}
"#,
        &["duplicate `#[discrim = ...]`"],
    );
}

#[test]
fn executable_program_rejects_arbitrary_discriminator_bytes() {
    compile_fail_case(
        "executable_program_arbitrary_discriminator",
        r#"
use anchor_lang_v2::prelude::*;

declare_id!("11111111111111111111111111111111");

#[derive(Accounts)]
pub struct Empty {}

#[program]
pub mod executable_program_arbitrary_discriminator {
    use super::*;

    #[discrim = [1, 2]]
    pub fn first(ctx: &mut Context<Empty>) -> Result<()> {
        let _ = ctx;
        Ok(())
    }
}
"#,
        &["custom discriminators must be one byte"],
    );
}

#[test]
fn executable_program_rejects_program_id_override() {
    compile_fail_case(
        "executable_program_id_override",
        r#"
use anchor_lang_v2::prelude::*;

declare_id!("11111111111111111111111111111111");
const EXTERNAL_ID: Address =
    Address::from_str_const("Con9ukTn9BRPXWcjS2UBbuN3NnCwy1hcaDNZ9Hb8QMNp");

#[derive(Accounts)]
pub struct Empty {}

#[program(program_id = EXTERNAL_ID)]
pub mod executable_program_id_override {
    use super::*;

    pub fn ix(ctx: &mut Context<Empty>) -> Result<()> {
        let _ = ctx;
        Ok(())
    }
}
"#,
        &["`program_id` is only supported"],
    );
}

#[test]
fn instruction_args_without_handler_args_do_not_compile() {
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
fn extra_instruction_args_do_not_compile() {
    compile_fail_case(
        "extra_instruction_args",
        r#"
use anchor_lang_v2::prelude::*;

declare_id!("11111111111111111111111111111111");

#[program]
pub mod extra_instruction_args {
    use super::*;

    pub fn ix(ctx: &mut Context<Bad>, value: u64) -> Result<()> {
        let _ = (ctx, value);
        Ok(())
    }
}

#[derive(Accounts)]
#[instruction(value: u64, other: u8)]
pub struct Bad {
    #[account(constraint = value > 0 && other > 0)]
    pub data: UncheckedAccount,
}
"#,
        &["the trait bound", "__AnchorIxArgCoerce"],
    );
}

#[test]
fn missing_instruction_args_do_not_compile() {
    compile_fail_case(
        "missing_instruction_args",
        r#"
use anchor_lang_v2::prelude::*;

declare_id!("11111111111111111111111111111111");

#[program]
pub mod missing_instruction_args {
    use super::*;

    pub fn ix(ctx: &mut Context<Bad>, value: u64, other: u8) -> Result<()> {
        let _ = (ctx, value, other);
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
        &["the trait bound", "__AnchorIxArgCoerce"],
    );
}

#[test]
fn wrong_instruction_arg_type_does_not_compile() {
    compile_fail_case(
        "wrong_instruction_arg_type",
        r#"
use anchor_lang_v2::prelude::*;

declare_id!("11111111111111111111111111111111");

#[program]
pub mod wrong_instruction_arg_type {
    use super::*;

    pub fn ix(ctx: &mut Context<Bad>, value: u64) -> Result<()> {
        let _ = (ctx, value);
        Ok(())
    }
}

#[derive(Accounts)]
#[instruction(value: u8)]
pub struct Bad {
    #[account(constraint = value > 0)]
    pub data: UncheckedAccount,
}
"#,
        &["the trait bound", "__AnchorIxArgCoerce"],
    );
}

#[test]
fn swapped_instruction_arg_types_do_not_compile() {
    compile_fail_case(
        "swapped_instruction_arg_types",
        r#"
use anchor_lang_v2::prelude::*;

declare_id!("11111111111111111111111111111111");

#[program]
pub mod swapped_instruction_arg_types {
    use super::*;

    pub fn ix(ctx: &mut Context<Bad>, amount: u64, flag: u8) -> Result<()> {
        let _ = (ctx, amount, flag);
        Ok(())
    }
}

#[derive(Accounts)]
#[instruction(amount: u8, flag: u64)]
pub struct Bad {
    #[account(constraint = amount > 0 && flag > 0)]
    pub data: UncheckedAccount,
}
"#,
        &["the trait bound", "__AnchorIxArgCoerce"],
    );
}

#[test]
fn close_on_unchecked_account_does_not_compile() {
    compile_fail_case(
        "close_on_unchecked_account",
        r#"
use anchor_lang_v2::prelude::*;

declare_id!("11111111111111111111111111111111");

#[program]
pub mod close_on_unchecked_account {
    use super::*;

    #[discrim = 0]
    pub fn close(ctx: &mut Context<Close>) -> Result<()> {
        let _ = ctx;
        Ok(())
    }
}

#[derive(Accounts)]
pub struct Close {
    #[account(mut, close = receiver)]
    pub data: UncheckedAccount,
    #[account(mut)]
    pub receiver: UncheckedAccount,
}
"#,
        &["`close` cannot be used on `UncheckedAccount`"],
    );
}

#[test]
fn close_on_boxed_unchecked_account_does_not_compile() {
    compile_fail_case(
        "close_on_boxed_unchecked_account",
        r#"
use anchor_lang_v2::prelude::*;

declare_id!("11111111111111111111111111111111");

#[program]
pub mod close_on_boxed_unchecked_account {
    use super::*;

    #[discrim = 0]
    pub fn close(ctx: &mut Context<Close>) -> Result<()> {
        let _ = ctx;
        Ok(())
    }
}

#[derive(Accounts)]
pub struct Close {
    #[account(mut, close = receiver)]
    pub data: Box<UncheckedAccount>,
    #[account(mut)]
    pub receiver: UncheckedAccount,
}
"#,
        &["`close` cannot be used on `UncheckedAccount`"],
    );
}

#[test]
fn close_on_optional_unchecked_account_does_not_compile() {
    compile_fail_case(
        "close_on_optional_unchecked_account",
        r#"
use anchor_lang_v2::prelude::*;

declare_id!("11111111111111111111111111111111");

#[program]
pub mod close_on_optional_unchecked_account {
    use super::*;

    #[discrim = 0]
    pub fn close(ctx: &mut Context<Close>) -> Result<()> {
        let _ = ctx;
        Ok(())
    }
}

#[derive(Accounts)]
pub struct Close {
    #[account(mut, close = receiver)]
    pub data: Option<UncheckedAccount>,
    #[account(mut)]
    pub receiver: UncheckedAccount,
}
"#,
        &["`close` cannot be used on `UncheckedAccount`"],
    );
}

#[test]
fn realloc_on_unchecked_account_does_not_compile() {
    compile_fail_case(
        "realloc_on_unchecked_account",
        r#"
use anchor_lang_v2::prelude::*;

declare_id!("11111111111111111111111111111111");

#[program]
pub mod realloc_on_unchecked_account {
    use super::*;

    #[discrim = 0]
    pub fn resize(ctx: &mut Context<Resize>) -> Result<()> {
        let _ = ctx;
        Ok(())
    }
}

#[derive(Accounts)]
pub struct Resize {
    #[account(mut, realloc = 16, realloc_payer = payer, realloc_zero = false)]
    pub data: UncheckedAccount,
    #[account(mut)]
    pub payer: Signer,
}
"#,
        &["`realloc` cannot be used on `UncheckedAccount`"],
    );
}

#[test]
fn realloc_on_boxed_unchecked_account_does_not_compile() {
    compile_fail_case(
        "realloc_on_boxed_unchecked_account",
        r#"
use anchor_lang_v2::prelude::*;

declare_id!("11111111111111111111111111111111");

#[program]
pub mod realloc_on_boxed_unchecked_account {
    use super::*;

    #[discrim = 0]
    pub fn resize(ctx: &mut Context<Resize>) -> Result<()> {
        let _ = ctx;
        Ok(())
    }
}

#[derive(Accounts)]
pub struct Resize {
    #[account(mut, realloc = 16, realloc_payer = payer, realloc_zero = false)]
    pub data: Box<UncheckedAccount>,
    #[account(mut)]
    pub payer: Signer,
}
"#,
        &["`realloc` cannot be used on `UncheckedAccount`"],
    );
}

#[test]
fn realloc_on_optional_unchecked_account_does_not_compile() {
    compile_fail_case(
        "realloc_on_optional_unchecked_account",
        r#"
use anchor_lang_v2::prelude::*;

declare_id!("11111111111111111111111111111111");

#[program]
pub mod realloc_on_optional_unchecked_account {
    use super::*;

    #[discrim = 0]
    pub fn resize(ctx: &mut Context<Resize>) -> Result<()> {
        let _ = ctx;
        Ok(())
    }
}

#[derive(Accounts)]
pub struct Resize {
    #[account(mut, realloc = 16, realloc_payer = payer, realloc_zero = false)]
    pub data: Option<UncheckedAccount>,
    #[account(mut)]
    pub payer: Signer,
}
"#,
        &["`realloc` cannot be used on `UncheckedAccount`"],
    );
}
