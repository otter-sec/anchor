use std::{fs, path::PathBuf, process::Command};

#[derive(Clone, Copy)]
enum CargoMode {
    Check,
    Build,
}

struct CompileCase<'a> {
    name: &'a str,
    source: &'a str,
    files: Vec<(&'a str, &'a str)>,
    features: Vec<&'a str>,
    mode: CargoMode,
}

impl<'a> CompileCase<'a> {
    fn new(name: &'a str, source: &'a str) -> Self {
        Self {
            name,
            source,
            files: Vec::new(),
            features: Vec::new(),
            mode: CargoMode::Check,
        }
    }

    fn file(mut self, relative_path: &'a str, contents: &'a str) -> Self {
        self.files.push((relative_path, contents));
        self
    }

    fn features(mut self, features: &'a [&'a str]) -> Self {
        self.features.extend_from_slice(features);
        self
    }

    fn build(mut self) -> Self {
        self.mode = CargoMode::Build;
        self
    }

    fn expect_pass(self) {
        self.run(Expectation::Pass);
    }

    fn expect_fail(self, snippets: &'a [&'a str]) {
        self.run(Expectation::Fail(snippets));
    }

    fn run(self, expectation: Expectation<'a>) {
        let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let crate_dir = manifest_dir.join("target/compile-cases").join(self.name);
        let src_dir = crate_dir.join("src");

        if crate_dir.exists() {
            fs::remove_dir_all(&crate_dir).unwrap();
        }
        fs::create_dir_all(&src_dir).unwrap();

        let anchor_lang_v2 = manifest_dir
            .parent()
            .expect("tests-v2 should live under the workspace root")
            .join("lang-v2");

        fs::write(
            crate_dir.join("Cargo.toml"),
            format!(
                r#"[package]
name = "{}"
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
                self.name,
                anchor_lang_v2.display()
            ),
        )
        .unwrap();
        fs::write(src_dir.join("lib.rs"), self.source).unwrap();
        for (relative_path, contents) in self.files {
            let path = crate_dir.join(relative_path);
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent).unwrap();
            }
            fs::write(path, contents).unwrap();
        }

        let mut command = Command::new("cargo");
        match self.mode {
            CargoMode::Check => command.arg("check"),
            CargoMode::Build => command.arg("build"),
        };
        command.args(["--offline", "--manifest-path"]);
        command.arg(crate_dir.join("Cargo.toml"));
        if !self.features.is_empty() {
            command.arg("--features");
            command.arg(self.features.join(","));
        }

        let output = command
            .output()
            .unwrap_or_else(|err| panic!("failed to run cargo for {}: {err}", self.name));
        let stderr = String::from_utf8_lossy(&output.stderr);
        let stdout = String::from_utf8_lossy(&output.stdout);

        match expectation {
            Expectation::Pass => assert!(
                output.status.success(),
                "{} did not compile successfully\n\nstdout:\n{stdout}\n\nstderr:\n{stderr}",
                self.name
            ),
            Expectation::Fail(snippets) => {
                assert!(
                    !output.status.success(),
                    "{} unexpectedly compiled successfully",
                    self.name
                );

                let rendered = format!("{stdout}\n{stderr}");
                for snippet in snippets {
                    assert!(
                        rendered.contains(snippet),
                        "{} output did not contain {snippet:?}\n\nstdout:\n{stdout}\n\nstderr:\n{stderr}",
                        self.name
                    );
                }
            }
        }
    }
}

enum Expectation<'a> {
    Pass,
    Fail(&'a [&'a str]),
}

fn declare_program_case<'a>(name: &'a str, idl: &'a str) -> CompileCase<'a> {
    CompileCase::new(
        name,
        r#"
use anchor_lang_v2::prelude::*;

declare_program!(bad);
"#,
    )
    .file("idls/bad.json", idl)
}

fn declare_program_compile_fail_case(name: &str, idl: &str, snippets: &[&str]) {
    declare_program_case(name, idl).expect_fail(snippets);
}

#[test]
fn program_interface_mode_compiles_client_and_cpi_surface() {
    CompileCase::new(
        "program_interface_mode",
        r#"
use anchor_lang_v2::prelude::*;

declare_id!("11111111111111111111111111111111");

pub mod declared {
    use super::*;

    pub const ID: Address =
        anchor_lang_v2::address!("Con9ukTn9BRPXWcjS2UBbuN3NnCwy1hcaDNZ9Hb8QMNp");

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
    )
    .features(&["cpi"])
    .expect_pass();
}

#[test]
fn program_interface_cpi_optional_accounts_compile() {
    CompileCase::new(
        "program_interface_optional_cpi",
        r#"
use anchor_lang_v2::prelude::*;

declare_id!("11111111111111111111111111111111");
const EXTERNAL_ID: Address =
    anchor_lang_v2::address!("Con9ukTn9BRPXWcjS2UBbuN3NnCwy1hcaDNZ9Hb8QMNp");

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
    )
    .features(&["cpi"])
    .expect_pass();
}

#[test]
fn program_interface_rejects_empty_discriminator() {
    CompileCase::new(
        "program_interface_empty_discriminator",
        r#"
use anchor_lang_v2::prelude::*;

declare_id!("11111111111111111111111111111111");
const EXTERNAL_ID: Address =
    anchor_lang_v2::address!("Con9ukTn9BRPXWcjS2UBbuN3NnCwy1hcaDNZ9Hb8QMNp");

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
    )
    .expect_fail(&["must contain at least one byte"]);
}

#[test]
fn program_interface_rejects_duplicate_discriminators() {
    CompileCase::new(
        "program_interface_duplicate_discriminator",
        r#"
use anchor_lang_v2::prelude::*;

declare_id!("11111111111111111111111111111111");
const EXTERNAL_ID: Address =
    anchor_lang_v2::address!("Con9ukTn9BRPXWcjS2UBbuN3NnCwy1hcaDNZ9Hb8QMNp");

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
    )
    .expect_fail(&["duplicate `#[discrim = ...]`"]);
}

#[test]
fn associated_token_rejects_unknown_constraint_key() {
    CompileCase::new(
        "associated_token_unknown_constraint_key",
        r#"
use anchor_lang_v2::prelude::*;

declare_id!("11111111111111111111111111111111");

#[derive(Accounts)]
pub struct BadAta {
    #[account(mut)]
    pub payer: Signer,
    pub mint: UncheckedAccount,
    pub authority: UncheckedAccount,
    #[account(
        init,
        payer = payer,
        associated_token::mint = mint,
        associated_token::authority = authority,
        associated_token::program = token_program,
    )]
    pub token_account: UncheckedAccount,
    pub token_program: UncheckedAccount,
    pub associated_token_program: UncheckedAccount,
    pub system_program: UncheckedAccount,
}
"#,
    )
    .expect_fail(&["unknown `associated_token` constraint `program`"]);
}

#[test]
fn declare_program_missing_idls_directory_fails_clearly() {
    CompileCase::new(
        "declare_program_missing_idls_directory",
        r#"
use anchor_lang_v2::prelude::*;

declare_program!(bad);
"#,
    )
    .expect_fail(&["`idls` directory not found"]);
}

#[test]
fn declare_program_invalid_json_fails_clearly() {
    declare_program_compile_fail_case(
        "declare_program_invalid_json",
        "{",
        &["failed to parse IDL"],
    );
}

#[test]
fn declare_program_legacy_idl_conversion_compiles() {
    CompileCase::new(
        "declare_program_legacy_idl_conversion",
        r#"
use anchor_lang_v2::{prelude::*, Event as _};

declare_program!(legacy);

pub fn build_ix(authority: Address, data: Address, owner: Address) -> anchor_lang_v2::solana_program::instruction::Instruction {
    let _event_data = legacy::events::LegacyEvent { value: 7 }.data();
    let _constant = legacy::constants::LEGACY_BYTES;
    let _error = legacy::error::LegacyError::LegacyError as u32;
    legacy::instruction::DoIt { amount: 5, owner }
        .to_instruction(legacy::accounts::DoIt { authority, data })
}
"#,
    )
    .file(
        "idls/legacy.json",
            r#"{
  "version": "0.1.0",
  "name": "legacy",
  "metadata": {
    "address": "11111111111111111111111111111111"
  },
  "instructions": [
    {
      "name": "doIt",
      "accounts": [
        { "name": "authority", "isMut": false, "isSigner": true },
        { "name": "data", "isMut": true, "isSigner": false }
      ],
      "args": [
        { "name": "amount", "type": "u64" },
        { "name": "owner", "type": "publicKey" }
      ]
    }
  ],
  "events": [
    {
      "name": "LegacyEvent",
      "fields": [
        { "name": "value", "type": "u64", "index": false }
      ]
    }
  ],
  "errors": [
    { "code": 6000, "name": "LegacyError", "msg": "legacy error" }
  ],
  "constants": [
    { "name": "LEGACY_BYTES", "type": "bytes", "value": "[1, 2]" }
  ]
}"#,
    )
    .expect_pass();
}

#[test]
fn declare_program_missing_accounts_array_fails_clearly() {
    declare_program_compile_fail_case(
        "declare_program_missing_accounts_array",
        r#"{
  "address": "11111111111111111111111111111111",
  "metadata": { "name": "bad", "version": "0.1.0", "spec": "0.1.0" },
  "instructions": [
    { "name": "ix", "discriminator": [1], "args": [] }
  ]
}"#,
        &["missing field `accounts`"],
    );
}

#[test]
fn declare_program_rejects_invalid_discriminator_byte() {
    declare_program_compile_fail_case(
        "declare_program_invalid_discriminator_byte",
        r#"{
  "address": "11111111111111111111111111111111",
  "metadata": { "name": "bad", "version": "0.1.0", "spec": "0.1.0" },
  "instructions": [
    {
      "name": "ix",
      "discriminator": [300],
      "accounts": [],
      "args": []
    }
  ]
}"#,
        &["invalid value: integer `300`, expected u8"],
    );
}

#[test]
fn declare_program_rejects_empty_discriminator() {
    declare_program_compile_fail_case(
        "declare_program_empty_discriminator",
        r#"{
  "address": "11111111111111111111111111111111",
  "metadata": { "name": "bad", "version": "0.1.0", "spec": "0.1.0" },
  "instructions": [
    {
      "name": "ix",
      "discriminator": [],
      "accounts": [],
      "args": []
    }
  ]
}"#,
        &["IDL discriminator must not be empty"],
    );
}

#[test]
fn declare_program_rejects_instruction_discriminator_prefix_overlap() {
    declare_program_compile_fail_case(
        "declare_program_instruction_discriminator_prefix_overlap",
        r#"{
  "address": "11111111111111111111111111111111",
  "metadata": { "name": "bad", "version": "0.1.0", "spec": "0.1.0" },
  "instructions": [
    {
      "name": "short",
      "discriminator": [1, 2],
      "accounts": [],
      "args": []
    },
    {
      "name": "long",
      "discriminator": [1, 2, 3],
      "accounts": [],
      "args": []
    }
  ]
}"#,
        &[
            "Ambiguous discriminators for instructions",
            "`short` discriminator [1, 2] is a prefix of `long` discriminator [1, 2, 3]",
        ],
    );
}

#[test]
fn declare_program_rejects_account_discriminator_prefix_overlap() {
    declare_program_compile_fail_case(
        "declare_program_account_discriminator_prefix_overlap",
        r#"{
  "address": "11111111111111111111111111111111",
  "metadata": { "name": "bad", "version": "0.1.0", "spec": "0.1.0" },
  "instructions": [],
  "accounts": [
    {
      "name": "ShortAccount",
      "discriminator": [7]
    },
    {
      "name": "LongAccount",
      "discriminator": [7, 8]
    }
  ]
}"#,
        &[
            "Ambiguous discriminators for accounts",
            "`ShortAccount` discriminator [7] is a prefix of `LongAccount` discriminator [7, 8]",
        ],
    );
}

#[test]
fn declare_program_rejects_event_discriminator_prefix_overlap() {
    declare_program_compile_fail_case(
        "declare_program_event_discriminator_prefix_overlap",
        r#"{
  "address": "11111111111111111111111111111111",
  "metadata": { "name": "bad", "version": "0.1.0", "spec": "0.1.0" },
  "instructions": [],
  "events": [
    {
      "name": "ShortEvent",
      "discriminator": [9, 9]
    },
    {
      "name": "LongEvent",
      "discriminator": [9, 9, 9]
    }
  ]
}"#,
        &[
            "Ambiguous discriminators for events",
            "`ShortEvent` discriminator [9, 9] is a prefix of `LongEvent` discriminator [9, 9, 9]",
        ],
    );
}

#[test]
fn declare_program_missing_args_array_fails_clearly() {
    declare_program_compile_fail_case(
        "declare_program_missing_args_array",
        r#"{
  "address": "11111111111111111111111111111111",
  "metadata": { "name": "bad", "version": "0.1.0", "spec": "0.1.0" },
  "instructions": [
    { "name": "ix", "discriminator": [1], "accounts": [] }
  ]
}"#,
        &["missing field `args`"],
    );
}

#[test]
fn declare_program_rejects_unsupported_argument_type() {
    declare_program_compile_fail_case(
        "declare_program_unsupported_argument_type",
        r#"{
  "address": "11111111111111111111111111111111",
  "metadata": { "name": "bad", "version": "0.1.0", "spec": "0.1.0" },
  "instructions": [
    {
      "name": "ix",
      "discriminator": [1],
      "accounts": [],
      "args": [
        { "name": "amount", "type": "u256" }
      ]
    }
  ]
}"#,
        &["unsupported IDL type string `u256`"],
    );
}

#[test]
fn declare_program_rejects_error_without_u32_code() {
    declare_program_compile_fail_case(
        "declare_program_error_without_code",
        r#"{
  "address": "11111111111111111111111111111111",
  "metadata": { "name": "bad", "version": "0.1.0", "spec": "0.1.0" },
  "instructions": [],
  "errors": [
    { "name": "bad" }
  ]
}"#,
        &["missing field `code`"],
    );
}

#[test]
fn declare_program_rejects_bad_constant_byte_length() {
    declare_program_compile_fail_case(
        "declare_program_bad_constant_byte_length",
        r#"{
  "address": "11111111111111111111111111111111",
  "metadata": { "name": "bad", "version": "0.1.0", "spec": "0.1.0" },
  "instructions": [],
  "constants": [
    {
      "name": "BAD_BYTES",
      "type": { "array": ["u8", 2] },
      "value": "[1, 2, 3]"
    }
  ]
}"#,
        &["constant `BAD_BYTES` has 3 bytes, expected 2"],
    );
}

#[test]
fn declare_program_rejects_bytemuck_enum_type() {
    declare_program_compile_fail_case(
        "declare_program_bytemuck_enum",
        r#"{
  "address": "11111111111111111111111111111111",
  "metadata": { "name": "bad", "version": "0.1.0", "spec": "0.1.0" },
  "instructions": [],
  "types": [
    {
      "name": "Mode",
      "serialization": "bytemuck",
      "type": {
        "kind": "enum",
        "variants": [
          { "name": "Active" }
        ]
      }
    }
  ]
}"#,
        &["declare_program! does not support bytemuck enum type `Mode`"],
    );
}

#[test]
fn declare_program_return_wrapper_compiles_for_returning_cpi() {
    CompileCase::new(
        "declare_program_return_wrapper",
        r#"
use anchor_lang_v2::prelude::*;

declare_program!(bad);

pub fn use_return<'a>(program: &'a Address, data: CpiHandle<'a>) {
    let accounts = bad::cpi::accounts::Ix { data };
    let ctx = CpiContext::new(program, accounts);
    let _ = bad::cpi::ix(ctx).unwrap().get();
}
"#,
    )
    .features(&["cpi"])
    .file(
        "idls/bad.json",
        r#"{
  "address": "11111111111111111111111111111111",
  "metadata": { "name": "bad", "version": "0.1.0", "spec": "0.1.0" },
  "instructions": [
    {
      "name": "ix",
      "discriminator": [1],
      "accounts": [
        { "name": "data" }
      ],
      "args": [],
      "returns": "u64"
    }
  ]
}"#,
    )
    .expect_pass();
}

#[test]
fn declare_program_non_returning_cpi_has_no_return_wrapper() {
    CompileCase::new(
        "declare_program_non_return_wrapper",
        r#"
use anchor_lang_v2::prelude::*;

declare_program!(bad);

pub fn misuse_return<'a>(program: &'a Address, data: CpiHandle<'a>) {
    let accounts = bad::cpi::accounts::Ix { data };
    let ctx = CpiContext::new(program, accounts);
    let _ = bad::cpi::ix(ctx).get();
}
"#,
    )
    .features(&["cpi"])
    .file(
        "idls/bad.json",
        r#"{
  "address": "11111111111111111111111111111111",
  "metadata": { "name": "bad", "version": "0.1.0", "spec": "0.1.0" },
  "instructions": [
    {
      "name": "ix",
      "discriminator": [1],
      "accounts": [
        { "name": "data" }
      ],
      "args": []
    }
  ]
}"#,
    )
    .expect_fail(&["no method named `get`"]);
}

#[test]
fn executable_program_rejects_arbitrary_discriminator_bytes() {
    CompileCase::new(
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
    )
    .expect_fail(&["custom discriminators must be one byte"]);
}

#[test]
fn executable_program_rejects_program_id_override() {
    CompileCase::new(
        "executable_program_id_override",
        r#"
use anchor_lang_v2::prelude::*;

declare_id!("11111111111111111111111111111111");
const EXTERNAL_ID: Address =
    anchor_lang_v2::address!("Con9ukTn9BRPXWcjS2UBbuN3NnCwy1hcaDNZ9Hb8QMNp");

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
    )
    .expect_fail(&["`program_id` is only supported"]);
}

#[test]
fn instruction_args_without_handler_args_do_not_compile() {
    CompileCase::new(
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
    )
    .expect_fail(&["expected `()`, found `(u64,)`"]);
}

#[test]
fn extra_instruction_args_do_not_compile() {
    CompileCase::new(
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
    )
    .expect_fail(&["the trait bound", "__AnchorIxArgCoerce"]);
}

#[test]
fn missing_instruction_args_do_not_compile() {
    CompileCase::new(
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
    )
    .expect_fail(&["the trait bound", "__AnchorIxArgCoerce"]);
}

#[test]
fn wrong_instruction_arg_type_does_not_compile() {
    CompileCase::new(
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
    )
    .expect_fail(&["the trait bound", "__AnchorIxArgCoerce"]);
}

#[test]
fn swapped_instruction_arg_types_do_not_compile() {
    CompileCase::new(
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
    )
    .expect_fail(&["the trait bound", "__AnchorIxArgCoerce"]);
}

#[test]
fn close_on_unchecked_account_compiles() {
    CompileCase::new(
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
    )
    .expect_pass();
}

#[test]
fn close_on_boxed_unchecked_account_compiles() {
    CompileCase::new(
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
    )
    .expect_pass();
}

#[test]
fn close_on_optional_unchecked_account_compiles() {
    CompileCase::new(
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
    )
    .expect_pass();
}

#[test]
fn close_requires_mut_on_source_account() {
    CompileCase::new(
        "close_requires_mut_on_source_account",
        r#"
use anchor_lang_v2::prelude::*;

declare_id!("11111111111111111111111111111111");

#[program]
pub mod close_requires_mut_on_source_account {
    use super::*;

    #[discrim = 0]
    pub fn close(ctx: &mut Context<Close>) -> Result<()> {
        let _ = ctx;
        Ok(())
    }
}

#[derive(Accounts)]
pub struct Close {
    #[account(close = receiver)]
    pub data: UncheckedAccount,
    #[account(mut)]
    pub receiver: UncheckedAccount,
}
"#,
    )
    .expect_fail(&["mut must be provided when using close"]);
}

#[test]
fn account_attrs_on_nested_field_do_not_compile() {
    CompileCase::new(
        "account_attrs_on_nested_field",
        r#"
use anchor_lang_v2::prelude::*;

declare_id!("11111111111111111111111111111111");

#[program]
pub mod account_attrs_on_nested_field {
    use super::*;

    #[discrim = 0]
    pub fn run(ctx: &mut Context<Outer>) -> Result<()> {
        let _ = ctx;
        Ok(())
    }
}

#[derive(Accounts)]
pub struct Inner {
    pub data: UncheckedAccount,
}

#[derive(Accounts)]
pub struct Outer {
    #[account(constraint = missing_symbol_that_should_not_compile())]
    pub inner: Nested<Inner>,
}
"#,
    )
    .expect_fail(&["`#[account(...)]` attributes are not supported on `Nested<T>` fields"]);
}

#[test]
fn slab_overaligned_header_does_not_compile() {
    CompileCase::new(
        "slab_overaligned_header",
        r#"
use anchor_lang_v2::{
    accounts::{Slab, SlabSchema},
    prelude::*,
    AccountView,
};

#[repr(C, align(16))]
#[derive(Clone, Copy)]
pub struct BadHeader {
    bytes: [u8; 16],
}

unsafe impl anchor_lang_v2::bytemuck::Zeroable for BadHeader {}
unsafe impl anchor_lang_v2::bytemuck::Pod for BadHeader {}

impl SlabSchema for BadHeader {
    const DATA_OFFSET: usize = 0;
    const MIN_DATA_LEN: usize = 16;

    fn validate(
        _view: &AccountView,
        _data: &[u8],
    ) -> core::result::Result<(), ProgramError> {
        Ok(())
    }
}

pub unsafe fn load_bad(view: AccountView) {
    let _ = <Slab<BadHeader> as AnchorAccount>::load_mut(view);
}
"#,
    )
    .build()
    .expect_fail(&["Slab header alignment exceeds Solana's 8-byte account data alignment"]);
}

#[test]
fn slab_misaligned_header_offset_does_not_compile() {
    CompileCase::new(
        "slab_misaligned_header_offset",
        r#"
use anchor_lang_v2::{
    accounts::{Slab, SlabSchema},
    prelude::*,
    AccountView,
};

#[repr(C)]
#[derive(Clone, Copy)]
pub struct BadHeader {
    value: u64,
}

unsafe impl anchor_lang_v2::bytemuck::Zeroable for BadHeader {}
unsafe impl anchor_lang_v2::bytemuck::Pod for BadHeader {}

impl SlabSchema for BadHeader {
    const DATA_OFFSET: usize = 1;
    const MIN_DATA_LEN: usize = 9;

    fn validate(
        _view: &AccountView,
        _data: &[u8],
    ) -> core::result::Result<(), ProgramError> {
        Ok(())
    }
}

pub unsafe fn load_bad(view: AccountView) {
    let _ = <Slab<BadHeader> as AnchorAccount>::load_mut(view);
}
"#,
    )
    .build()
    .expect_fail(&["Slab header DATA_OFFSET is not aligned for the header type"]);
}

#[test]
fn realloc_on_unchecked_account_does_not_compile() {
    CompileCase::new(
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
    )
    .expect_fail(&["AccountRealloc", "UncheckedAccount"]);
}

#[test]
fn realloc_on_boxed_unchecked_account_does_not_compile() {
    CompileCase::new(
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
    )
    .expect_fail(&["AccountRealloc", "UncheckedAccount"]);
}

#[test]
fn realloc_on_optional_unchecked_account_does_not_compile() {
    CompileCase::new(
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
    )
    .expect_fail(&["AccountRealloc", "UncheckedAccount"]);
}

#[test]
fn realloc_on_unchecked_account_alias_does_not_compile() {
    CompileCase::new(
        "realloc_on_unchecked_account_alias",
        r#"
use anchor_lang_v2::prelude::*;

type UA = UncheckedAccount;

#[derive(Accounts)]
pub struct Resize {
    #[account(mut, realloc = 16, realloc_payer = payer, realloc_zero = false)]
    pub data: UA,
    #[account(mut)]
    pub payer: Signer,
}
"#,
    )
    .expect_fail(&["AccountRealloc", "UncheckedAccount"]);
}

#[test]
fn realloc_on_renamed_unchecked_account_does_not_compile() {
    CompileCase::new(
        "realloc_on_renamed_unchecked_account",
        r#"
use anchor_lang_v2::prelude::*;
use anchor_lang_v2::accounts::UncheckedAccount as UA;

#[derive(Accounts)]
pub struct Resize {
    #[account(mut, realloc = 16, realloc_payer = payer, realloc_zero = false)]
    pub data: UA,
    #[account(mut)]
    pub payer: Signer,
}
"#,
    )
    .expect_fail(&["AccountRealloc", "UncheckedAccount"]);
}

#[test]
fn realloc_on_box_alias_unchecked_account_does_not_compile() {
    CompileCase::new(
        "realloc_on_box_alias_unchecked_account",
        r#"
use anchor_lang_v2::prelude::*;

type MyBox<T> = Box<T>;

#[derive(Accounts)]
pub struct Resize {
    #[account(mut, realloc = 16, realloc_payer = payer, realloc_zero = false)]
    pub data: MyBox<UncheckedAccount>,
    #[account(mut)]
    pub payer: Signer,
}
"#,
    )
    .expect_fail(&["AccountRealloc", "UncheckedAccount"]);
}

#[test]
fn realloc_on_borsh_account_alias_compiles() {
    CompileCase::new(
        "realloc_on_borsh_account_alias",
        r#"
use anchor_lang_v2::prelude::*;

#[derive(wincode::SchemaRead, wincode::SchemaWrite, Default)]
pub struct Data {
    pub value: u64,
}

impl Owner for Data {
    const OWNER: Address = Address::from_str_const("11111111111111111111111111111111");
}

impl Discriminator for Data {
    const DISCRIMINATOR: &'static [u8] = &[1, 2, 3, 4, 5, 6, 7, 8];
}

type BA<T> = BorshAccount<T>;

#[derive(Accounts)]
pub struct Resize {
    #[account(mut, realloc = 16, realloc_payer = payer, realloc_zero = false)]
    pub data: BA<Data>,
    #[account(mut)]
    pub payer: Signer,
}
"#,
    )
    .expect_pass();
}
