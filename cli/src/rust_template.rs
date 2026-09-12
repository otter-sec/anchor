use crate::{
    config::ProgramWorkspace, create_files, override_or_create_files, Files, PackageManager,
    VERSION,
};
use anyhow::Result;
use clap::{Parser, ValueEnum};
use heck::{ToLowerCamelCase, ToPascalCase, ToSnakeCase};
use solana_sdk::{
    pubkey::Pubkey,
    signature::{read_keypair_file, write_keypair_file, Keypair},
    signer::Signer,
};
use std::{
    fmt::Write as _,
    fs::{self, File},
    io::Write as _,
    path::Path,
    process::Stdio,
};

const ANCHOR_MSRV: &str = "1.89.0";

/// Program initialization template
#[derive(Clone, Debug, Default, Eq, PartialEq, Parser, ValueEnum)]
pub enum ProgramTemplate {
    /// Program with a single `lib.rs` file
    #[default]
    Single,
    /// Program with multiple files for instructions, state...
    Multiple,
}

/// Create a program from the given name and template.
pub fn create_program(name: &str, template: ProgramTemplate, with_mollusk: bool) -> Result<()> {
    let program_path = Path::new("programs").join(name);
    let common_files = vec![
        ("Cargo.toml".into(), workspace_manifest().into()),
        ("rust-toolchain.toml".into(), rust_toolchain_toml()),
        (
            program_path.join("Cargo.toml"),
            cargo_toml(name, with_mollusk),
        ),
        // Note: Xargo.toml is no longer needed for modern Solana builds using SBF
    ];

    let template_files = match template {
        ProgramTemplate::Single => create_program_template_single(name, &program_path),
        ProgramTemplate::Multiple => create_program_template_multiple(name, &program_path),
    };

    create_files(&[common_files, template_files].concat())
}

/// Helper to create a rust-toolchain.toml at the workspace root
fn rust_toolchain_toml() -> String {
    format!(
        r#"[toolchain]
channel = "{msrv}"
components = ["rustfmt","clippy"]
profile = "minimal"
"#,
        msrv = ANCHOR_MSRV
    )
}

/// Create a program with a single `lib.rs` file.
fn create_program_template_single(name: &str, program_path: &Path) -> Files {
    vec![(
        program_path.join("src").join("lib.rs"),
        format!(
            r#"use anchor_lang::prelude::*;

declare_id!("{}");

#[program]
pub mod {} {{
    use super::*;

    pub fn initialize(ctx: Context<Initialize>) -> Result<()> {{
        msg!("Greetings from: {{:?}}", ctx.program_id);
        Ok(())
    }}
}}

#[derive(Accounts)]
pub struct Initialize {{}}
"#,
            get_or_create_program_id(name),
            name.to_snake_case(),
        ),
    )]
}

/// Create a program with multiple files for instructions, state...
fn create_program_template_multiple(name: &str, program_path: &Path) -> Files {
    let src_path = program_path.join("src");
    vec![
        (
            src_path.join("lib.rs"),
            format!(
                r#"pub mod constants;
pub mod error;
pub mod instructions;
pub mod state;

use anchor_lang::prelude::*;

pub use constants::*;
pub use instructions::*;
pub use state::*;

declare_id!("{}");

#[program]
pub mod {} {{
    use super::*;

    pub fn initialize(ctx: Context<Initialize>) -> Result<()> {{
        initialize::handler(ctx)
    }}
}}
"#,
                get_or_create_program_id(name),
                name.to_snake_case(),
            ),
        ),
        (
            src_path.join("constants.rs"),
            r#"use anchor_lang::prelude::*;

#[constant]
pub const SEED: &str = "anchor";
"#
            .into(),
        ),
        (
            src_path.join("error.rs"),
            r#"use anchor_lang::prelude::*;

#[error_code]
pub enum ErrorCode {
    #[msg("Custom error message")]
    CustomError,
}
"#
            .into(),
        ),
        (
            src_path.join("instructions").join("mod.rs"),
            r#"pub mod initialize;

pub use initialize::*;
"#
            .into(),
        ),
        (
            src_path.join("instructions").join("initialize.rs"),
            r#"use anchor_lang::prelude::*;

#[derive(Accounts)]
pub struct Initialize {}

pub fn handler(ctx: Context<Initialize>) -> Result<()> {
    msg!("Greetings from: {:?}", ctx.program_id);
    Ok(())
}
"#
            .into(),
        ),
        (src_path.join("state").join("mod.rs"), r#""#.into()),
    ]
}

const fn workspace_manifest() -> &'static str {
    r#"[workspace]
members = [
    "programs/*"
]
resolver = "2"

[profile.release]
overflow-checks = true
lto = "fat"
codegen-units = 1
[profile.release.build-override]
opt-level = 3
incremental = false
codegen-units = 1
"#
}

fn cargo_toml(name: &str, with_mollusk: bool) -> String {
    let test_sbf_feature = if with_mollusk { r#"test-sbf = []"# } else { "" };
    let dev_dependencies = if with_mollusk {
        r#"
[dev-dependencies]
mollusk-svm = "~0.4"
"#
    } else {
        ""
    };

    format!(
        r#"[package]
name = "{0}"
version = "0.1.0"
description = "Created with Anchor"
edition = "2021"

[lib]
crate-type = ["cdylib", "lib"]
name = "{1}"

[features]
default = []
cpi = ["no-entrypoint"]
no-entrypoint = []
no-idl = []
no-log-ix-name = []
idl-build = ["anchor-lang/idl-build"]
anchor-debug = []
custom-heap = []
custom-panic = []
{2}

[dependencies]
anchor-lang = "{3}"
{4}

[lints.rust]
unexpected_cfgs = {{ level = "warn", check-cfg = ['cfg(target_os, values("solana"))'] }}
"#,
        name,
        name.to_snake_case(),
        test_sbf_feature,
        VERSION,
        dev_dependencies,
    )
}

/// Read the program keypair file or create a new one if it doesn't exist.
pub fn get_or_create_program_id(name: &str) -> Pubkey {
    let keypair_path = Path::new("target")
        .join("deploy")
        .join(format!("{}-keypair.json", name.to_snake_case()));

    read_keypair_file(&keypair_path)
        .unwrap_or_else(|_| {
            let keypair = Keypair::new();
            write_keypair_file(&keypair, keypair_path).expect("Unable to create program keypair");
            keypair
        })
        .pubkey()
}

pub fn credentials(token: &str) -> String {
    format!(
        r#"[registry]
token = "{token}"
"#
    )
}

pub fn deploy_js_script_host(cluster_url: &str, script_path: &str) -> String {
    format!(
        r#"
const anchor = require('@anchor-lang/core');

// Deploy script defined by the user.
const userScript = require("{script_path}");

async function main() {{
    const connection = new anchor.web3.Connection(
      "{cluster_url}",
      anchor.AnchorProvider.defaultOptions().commitment
    );
    const wallet = anchor.Wallet.local();
    const provider = new anchor.AnchorProvider(connection, wallet);

    // Run the user's deploy script.
    userScript(provider);
}}
main();
"#,
    )
}

pub fn deploy_ts_script_host(cluster_url: &str, script_path: &str) -> String {
    format!(
        r#"import * as anchor from '@anchor-lang/core';

// Deploy script defined by the user.
const userScript = require("{script_path}");

async function main() {{
    const connection = new anchor.web3.Connection(
      "{cluster_url}",
      anchor.AnchorProvider.defaultOptions().commitment
    );
    const wallet = anchor.Wallet.local();
    const provider = new anchor.AnchorProvider(connection, wallet);

    // Run the user's deploy script.
    userScript(provider);
}}
main();
"#,
    )
}

pub fn deploy_script() -> &'static str {
    r#"// Migrations are an early feature. Currently, they're nothing more than this
// single deploy script that's invoked from the CLI, injecting a provider
// configured from the workspace's Anchor.toml.

const anchor = require("@anchor-lang/core");

module.exports = async function (provider) {
  // Configure client to use the provider.
  anchor.setProvider(provider);

  // Add your deploy script here.
};
"#
}

pub fn ts_deploy_script() -> &'static str {
    r#"// Migrations are an early feature. Currently, they're nothing more than this
// single deploy script that's invoked from the CLI, injecting a provider
// configured from the workspace's Anchor.toml.

import * as anchor from "@anchor-lang/core";

module.exports = async function (provider: anchor.AnchorProvider) {
  // Configure client to use the provider.
  anchor.setProvider(provider);

  // Add your deploy script here.
};
"#
}

pub fn mocha(name: &str) -> String {
    format!(
        r#"const anchor = require("@anchor-lang/core");

describe("{}", () => {{
  // Configure the client to use the local cluster.
  anchor.setProvider(anchor.AnchorProvider.env());

  it("Is initialized!", async () => {{
    // Add your test here.
    const program = anchor.workspace.{};
    const tx = await program.methods.initialize().rpc();
    console.log("Your transaction signature", tx);
  }});
}});
"#,
        name,
        name.to_lower_camel_case(),
    )
}

pub fn jest(name: &str) -> String {
    format!(
        r#"const anchor = require("@anchor-lang/core");

describe("{}", () => {{
  // Configure the client to use the local cluster.
  anchor.setProvider(anchor.AnchorProvider.env());

  it("Is initialized!", async () => {{
    // Add your test here.
    const program = anchor.workspace.{};
    const tx = await program.methods.initialize().rpc();
    console.log("Your transaction signature", tx);
  }});
}});
"#,
        name,
        name.to_lower_camel_case(),
    )
}

pub fn package_json(jest: bool, license: String) -> String {
    if jest {
        format!(
            r#"{{
  "license": "{license}",
  "scripts": {{
    "lint:fix": "prettier */*.js \"*/**/*{{.js,.ts}}\" -w",
    "lint": "prettier */*.js \"*/**/*{{.js,.ts}}\" --check"
  }},
  "dependencies": {{
    "@anchor-lang/core": "^{VERSION}"
  }},
  "devDependencies": {{
    "jest": "^29.0.3",
    "prettier": "^2.6.2"
  }}
}}
    "#
        )
    } else {
        format!(
            r#"{{
  "license": "{license}",
  "scripts": {{
    "lint:fix": "prettier */*.js \"*/**/*{{.js,.ts}}\" -w",
    "lint": "prettier */*.js \"*/**/*{{.js,.ts}}\" --check"
  }},
  "dependencies": {{
    "@anchor-lang/core": "^{VERSION}"
  }},
  "devDependencies": {{
    "chai": "^4.3.4",
    "mocha": "^9.0.3",
    "prettier": "^2.6.2"
  }}
}}
"#
        )
    }
}

pub fn ts_package_json(jest: bool, license: String) -> String {
    if jest {
        format!(
            r#"{{
  "license": "{license}",
  "scripts": {{
    "lint:fix": "prettier */*.js \"*/**/*{{.js,.ts}}\" -w",
    "lint": "prettier */*.js \"*/**/*{{.js,.ts}}\" --check"
  }},
  "dependencies": {{
    "@anchor-lang/core": "^{VERSION}"
  }},
  "devDependencies": {{
    "@types/bn.js": "^5.1.0",
    "@types/jest": "^29.0.3",
    "jest": "^29.0.3",
    "prettier": "^2.6.2",
    "ts-jest": "^29.0.2",
    "typescript": "^5.7.3"
  }}
}}
"#
        )
    } else {
        format!(
            r#"{{
  "license": "{license}",
  "scripts": {{
    "lint:fix": "prettier */*.js \"*/**/*{{.js,.ts}}\" -w",
    "lint": "prettier */*.js \"*/**/*{{.js,.ts}}\" --check"
  }},
  "dependencies": {{
    "@anchor-lang/core": "^{VERSION}"
  }},
  "devDependencies": {{
    "chai": "^4.3.4",
    "mocha": "^9.0.3",
    "ts-mocha": "^10.0.0",
    "@types/bn.js": "^5.1.0",
    "@types/chai": "^4.3.0",
    "@types/mocha": "^9.0.0",
    "typescript": "^5.7.3",
    "prettier": "^2.6.2"
  }}
}}
"#
        )
    }
}

pub fn ts_mocha(name: &str) -> String {
    format!(
        r#"import * as anchor from "@anchor-lang/core";
import {{ Program }} from "@anchor-lang/core";
import {{ {} }} from "../target/types/{}";

describe("{}", () => {{
  // Configure the client to use the local cluster.
  anchor.setProvider(anchor.AnchorProvider.env());

  const program = anchor.workspace.{} as Program<{}>;

  it("Is initialized!", async () => {{
    // Add your test here.
    const tx = await program.methods.initialize().rpc();
    console.log("Your transaction signature", tx);
  }});
}});
"#,
        name.to_pascal_case(),
        name.to_snake_case(),
        name,
        name.to_lower_camel_case(),
        name.to_pascal_case(),
    )
}

pub fn ts_jest(name: &str) -> String {
    format!(
        r#"import * as anchor from "@anchor-lang/core";
import {{ Program }} from "@anchor-lang/core";
import {{ {} }} from "../target/types/{}";

describe("{}", () => {{
  // Configure the client to use the local cluster.
  anchor.setProvider(anchor.AnchorProvider.env());

  const program = anchor.workspace.{} as Program<{}>;

  it("Is initialized!", async () => {{
    // Add your test here.
    const tx = await program.methods.initialize().rpc();
    console.log("Your transaction signature", tx);
  }});
}});
"#,
        name.to_pascal_case(),
        name.to_snake_case(),
        name,
        name.to_lower_camel_case(),
        name.to_pascal_case(),
    )
}

pub fn ts_config(jest: bool) -> &'static str {
    if jest {
        r#"{
  "compilerOptions": {
    "types": ["jest"],
    "typeRoots": ["./node_modules/@types"],
    "lib": ["es2015"],
    "module": "commonjs",
    "target": "es6",
    "esModuleInterop": true
  }
}
"#
    } else {
        r#"{
  "compilerOptions": {
    "types": ["mocha", "chai"],
    "typeRoots": ["./node_modules/@types"],
    "lib": ["es2015"],
    "module": "commonjs",
    "target": "es6",
    "esModuleInterop": true
  }
}
"#
    }
}

pub fn git_ignore() -> &'static str {
    r#".anchor
.DS_Store
target
**/*.rs.bk
node_modules
test-ledger
.yarn
"#
}

pub fn prettier_ignore() -> &'static str {
    r#".anchor
.DS_Store
target
node_modules
dist
build
test-ledger
"#
}

pub fn node_shell(
    cluster_url: &str,
    wallet_path: &str,
    programs: Vec<ProgramWorkspace>,
) -> Result<String> {
    let mut eval_string = format!(
        r#"
const anchor = require('@anchor-lang/core');
const web3 = anchor.web3;
const PublicKey = anchor.web3.PublicKey;
const Keypair = anchor.web3.Keypair;

const __wallet = new anchor.Wallet(
  Keypair.fromSecretKey(
    Buffer.from(
      JSON.parse(
        require('fs').readFileSync(
          "{wallet_path}",
          {{
            encoding: "utf-8",
          }},
        ),
      ),
    ),
  ),
);
const __connection = new web3.Connection("{cluster_url}", "processed");
const provider = new anchor.AnchorProvider(__connection, __wallet, {{
  commitment: "processed",
  preflightcommitment: "processed",
}});
anchor.setProvider(provider);
"#,
    );

    for program in programs {
        write!(
            &mut eval_string,
            r#"
anchor.workspace.{} = new anchor.Program({}, provider);
"#,
            program.name.to_lower_camel_case(),
            serde_json::to_string(&program.idl)?,
        )?;
    }

    Ok(eval_string)
}

/// Test initialization template
#[derive(Clone, Debug, Default, Eq, PartialEq, Parser, ValueEnum)]
pub enum TestTemplate {
    /// Generate template for Mocha unit-test
    #[default]
    Mocha,
    /// Generate template for Jest unit-test
    Jest,
    /// Generate template for Rust unit-test
    Rust,
    /// Generate template for Mollusk Rust unit-test
    Mollusk,
}

impl TestTemplate {
    pub fn get_test_script(&self, js: bool, pkg_manager: &PackageManager) -> String {
        let pkg_manager_exec_cmd = match pkg_manager {
            PackageManager::Yarn => "yarn run",
            PackageManager::NPM => "npx",
            PackageManager::PNPM => "pnpm exec",
            PackageManager::Bun => "bunx",
        };

        match &self {
            Self::Mocha => {
                if js {
                    format!("{pkg_manager_exec_cmd} mocha -t 1000000 tests/")
                } else {
                    format!(
                        r#"{pkg_manager_exec_cmd} ts-mocha -p ./tsconfig.json -t 1000000 "tests/**/*.ts""#
                    )
                }
            }
            Self::Jest => {
                if js {
                    format!("{pkg_manager_exec_cmd} jest")
                } else {
                    format!("{pkg_manager_exec_cmd} jest --preset ts-jest")
                }
            }
            Self::Rust => "cargo test".to_owned(),
            Self::Mollusk => "cargo test-sbf".to_owned(),
        }
    }

    pub fn create_test_files(&self, project_name: &str, js: bool, program_id: &str) -> Result<()> {
        match self {
            Self::Mocha => {
                // Build the test suite.
                fs::create_dir_all("tests")?;

                if js {
                    let mut test = File::create(format!("tests/{}.js", &project_name))?;
                    test.write_all(mocha(project_name).as_bytes())?;
                } else {
                    let mut mocha = File::create(format!("tests/{}.ts", &project_name))?;
                    mocha.write_all(ts_mocha(project_name).as_bytes())?;
                }
            }
            Self::Jest => {
                // Build the test suite.
                fs::create_dir_all("tests")?;

                let mut test = File::create(format!("tests/{}.test.js", &project_name))?;
                test.write_all(jest(project_name).as_bytes())?;
            }
            Self::Rust => {
                // Do not initialize git repo
                let exit = std::process::Command::new("cargo")
                    .arg("new")
                    .arg("--vcs")
                    .arg("none")
                    .arg("--lib")
                    .arg("tests")
                    .stderr(Stdio::inherit())
                    .output()
                    .map_err(|e| anyhow::format_err!("{}", e.to_string()))?;
                if !exit.status.success() {
                    eprintln!("'cargo new --lib tests' failed");
                    std::process::exit(exit.status.code().unwrap_or(1));
                }

                let mut files = Vec::new();
                let tests_path = Path::new("tests");
                files.extend(vec![(
                    tests_path.join("Cargo.toml"),
                    tests_cargo_toml(project_name),
                )]);
                files.extend(create_program_template_rust_test(
                    project_name,
                    tests_path,
                    program_id,
                ));
                override_or_create_files(&files)?;
            }
            Self::Mollusk => {
                // Build the test suite.
                let tests_path_str = format!("programs/{}/tests", &project_name);
                let tests_path = Path::new(&tests_path_str);
                fs::create_dir_all(tests_path)?;

                let mut files = Vec::new();
                files.extend(create_program_template_mollusk_test(
                    project_name,
                    tests_path,
                ));
                override_or_create_files(&files)?;
            }
        }

        Ok(())
    }
}

pub fn tests_cargo_toml(name: &str) -> String {
    format!(
        r#"[package]
name = "tests"
version = "0.1.0"
description = "Created with Anchor"
edition = "2021"
rust-version = "{msrv}"

[dependencies]
anchor-client = "{version}"
{name} = {{ version = "0.1.0", path = "../programs/{name}" }}
"#,
        msrv = ANCHOR_MSRV,
        version = VERSION,
        name = name,
    )
}

/// Generate template for Rust unit-test
fn create_program_template_rust_test(name: &str, tests_path: &Path, program_id: &str) -> Files {
    let src_path = tests_path.join("src");
    vec![
        (
            src_path.join("lib.rs"),
            r#"#[cfg(test)]
mod test_initialize;
"#
            .into(),
        ),
        (
            src_path.join("test_initialize.rs"),
            format!(
                r#"use std::str::FromStr;

use anchor_client::{{
    solana_sdk::{{
        commitment_config::CommitmentConfig, pubkey::Pubkey, signature::read_keypair_file,
    }},
    Client, Cluster,
}};

#[test]
fn test_initialize() {{
    let program_id = "{0}";
    let anchor_wallet = std::env::var("ANCHOR_WALLET").unwrap();
    let payer = read_keypair_file(&anchor_wallet).unwrap();

    let client = Client::new_with_options(Cluster::Localnet, &payer, CommitmentConfig::confirmed());
    let program_id = Pubkey::from_str(program_id).unwrap();
    let program = client.program(program_id).unwrap();

    let tx = program
        .request()
        .accounts({1}::accounts::Initialize {{}})
        .args({1}::instruction::Initialize {{}})
        .send()
        .expect("");

    println!("Your transaction signature {{}}", tx);
}}
"#,
                program_id,
                name.to_snake_case(),
            ),
        ),
    ]
}

/// Generate template for Mollusk Rust unit-test
fn create_program_template_mollusk_test(name: &str, tests_path: &Path) -> Files {
    vec![(
        tests_path.join("test_initialize.rs"),
        format!(
            r#"#![cfg(feature = "test-sbf")]

use {{
    anchor_lang::{{solana_program::instruction::Instruction, InstructionData, ToAccountMetas}},
    mollusk_svm::{{result::Check, Mollusk}},
}};

#[test]
fn test_initialize() {{
    let program_id = {0}::id();

    let mollusk = Mollusk::new(&program_id, "{0}");

    let instruction = Instruction::new_with_bytes(
        program_id,
        &{0}::instruction::Initialize {{}}.data(),
        {0}::accounts::Initialize {{}}.to_account_metas(None),
    );

    mollusk.process_and_validate_instruction(&instruction, &[], &[Check::success()]);
}}
"#,
            name.to_snake_case(),
        ),
    )]
}
