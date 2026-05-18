use {
    crate::{
        config::ProgramWorkspace, create_files, override_or_create_files, AbsolutePath, Files,
        PackageManager, VERSION,
    },
    anyhow::Result,
    clap::{Parser, ValueEnum},
    heck::{ToLowerCamelCase, ToPascalCase, ToSnakeCase},
    solana_keypair::{read_keypair_file, write_keypair_file, Keypair},
    solana_pubkey::Pubkey,
    solana_signer::Signer,
    std::{
        fmt::Write as _,
        fs::{self, File},
        io::Write as _,
        path::Path,
        process::Stdio,
    },
};

const ANCHOR_MSRV: &str = "1.89.0";

/// Program initialization template
#[derive(Clone, Debug, Default, Eq, PartialEq, Parser, ValueEnum, AbsolutePath)]
pub enum ProgramTemplate {
    /// Program with a single `lib.rs` file (not recommended for production)
    Single,
    /// Program with multiple files for instructions, state... (recommended)
    #[default]
    Multiple,
}

/// Create a program from the given name and template.
pub fn create_program(
    name: &str,
    template: ProgramTemplate,
    test_template: Option<&TestTemplate>,
) -> Result<()> {
    let program_path = Path::new("programs").join(name);
    let lib_rs_path = program_path.join("src").join("lib.rs");
    let common_files = vec![
        ("Cargo.toml".into(), workspace_manifest()),
        ("rust-toolchain.toml".into(), rust_toolchain_toml()),
        (
            program_path.join("Cargo.toml"),
            cargo_toml(name, test_template),
        ),
        // One of the create_program_template_* functions will write the full
        // lib.rs, but we need an empty stub for now so cargo won't throw an
        // error when asking it where the `target` dir is located.
        (lib_rs_path.clone(), "".into()),
        // Note: Xargo.toml is no longer needed for modern Solana builds using SBF.
    ];

    create_files(&common_files)?;

    let target_path = crate::target_dir()?;

    // Remove the stub version
    fs::remove_file(&lib_rs_path)?;

    let template_files = match template {
        ProgramTemplate::Single => {
            println!(
                "Note: Using single-file template. For better code organization and \
                 maintainability, consider using --template multiple (default)."
            );
            create_program_template_single(name, &program_path, target_path)
        }
        ProgramTemplate::Multiple => {
            create_program_template_multiple(name, &program_path, target_path)
        }
    };

    create_files(&template_files)
}

/// Helper to create a rust-toolchain.toml at the workspace root
fn rust_toolchain_toml() -> String {
    format!(
        r#"[toolchain]
channel = "{ANCHOR_MSRV}"
components = ["rustfmt","clippy"]
profile = "minimal"
"#
    )
}

/// Create a program with a single `lib.rs` file.
fn create_program_template_single(name: &str, program_path: &Path, target_path: &Path) -> Files {
    vec![(
        program_path.join("src").join("lib.rs"),
        format!(
            r#"use anchor_lang_v2::prelude::*;

declare_id!("{}");

#[program]
pub mod {} {{
    use super::*;

    pub fn initialize(ctx: &mut Context<Initialize>) -> Result<()> {{
        ctx.accounts.counter.count = 0;
        ctx.accounts.counter.authority = *ctx.accounts.payer.address();
        msg!("Counter initialized");
        Ok(())
    }}
}}

pub mod state {{
    use super::*;

    #[account]
    pub struct Counter {{
        pub count: u64,
        pub authority: Address,
    }}
}}

use state::Counter;

#[derive(Accounts)]
pub struct Initialize {{
    #[account(mut)]
    pub payer: Signer,
    #[account(init, payer = payer)]
    pub counter: Account<Counter>,
    pub system_program: Program<System>,
}}
"#,
            get_or_create_program_id(name, target_path),
            name.to_snake_case(),
        ),
    )]
}

/// Create a program with multiple files for instructions, state...
fn create_program_template_multiple(name: &str, program_path: &Path, target_path: &Path) -> Files {
    let src_path = program_path.join("src");
    vec![
        (
            src_path.join("lib.rs"),
            format!(
                r#"pub mod constants;
pub mod error;
pub mod instructions;
pub mod state;

use anchor_lang_v2::prelude::*;

pub use instructions::*;

declare_id!("{}");

#[program]
pub mod {} {{
    use super::*;

    pub fn initialize(ctx: &mut Context<Initialize>) -> Result<()> {{
        initialize::initialize(ctx)
    }}

    pub fn increment(ctx: &mut Context<Increment>) -> Result<()> {{
        increment::increment(ctx)
    }}
}}
"#,
                get_or_create_program_id(name, target_path),
                name.to_snake_case(),
            ),
        ),
        (
            src_path.join("constants.rs"),
            r#"use anchor_lang_v2::prelude::*;

#[constant]
pub const COUNTER_SEED: &[u8] = b"counter";

#[constant]
pub const HELLO_WORLD_LAMPORTS: u64 = 1;

#[constant]
pub const MAX_COUNT: u64 = 10;
"#
            .into(),
        ),
        (
            src_path.join("error.rs"),
            r#"use anchor_lang_v2::prelude::*;

#[error_code]
pub enum ErrorCode {
    #[msg("Only the counter authority can update this counter")]
    Unauthorized,
    #[msg("Counter has reached the maximum value")]
    CounterOverflow,
}
"#
            .into(),
        ),
        (
            src_path.join("instructions.rs"),
            r#"pub mod initialize;
pub mod increment;

pub use initialize::*;
pub use increment::*;
"#
            .into(),
        ),
        (
            src_path.join("instructions").join("initialize.rs"),
            r#"use anchor_lang_v2::prelude::*;

use crate::{constants::*, state::Counter};

#[derive(Accounts)]
pub struct Initialize {
    #[account(mut)]
    pub payer: Signer,
    #[account(init, payer = payer, seeds = [COUNTER_SEED], bump)]
    pub counter: Account<Counter>,
    pub system_program: Program<System>,
}

pub fn initialize(ctx: &mut Context<Initialize>) -> Result<()> {
    ctx.accounts.counter.count = 0;
    ctx.accounts.counter.authority = *ctx.accounts.payer.address();

    let cpi_accounts = system_program::Transfer {
        from: ctx.accounts.payer.cpi_handle_mut(),
        to: ctx.accounts.counter.cpi_handle_mut(),
    };
    let cpi_ctx = CpiContext::new(ctx.accounts.system_program.address(), cpi_accounts);
    system_program::transfer(cpi_ctx, HELLO_WORLD_LAMPORTS)?;

    msg!("Hello, world! Counter initialized");
    Ok(())
}
"#
            .into(),
        ),
        (
            src_path.join("instructions").join("increment.rs"),
            r#"use anchor_lang_v2::prelude::*;

use crate::{constants::*, error::ErrorCode, state::Counter};

#[derive(Accounts)]
pub struct Increment {
    #[account(mut, seeds = [COUNTER_SEED], bump)]
    pub counter: Account<Counter>,
    pub authority: Signer,
}

pub fn increment(ctx: &mut Context<Increment>) -> Result<()> {
    require_keys_eq!(
        ctx.accounts.counter.authority,
        *ctx.accounts.authority.address(),
        ErrorCode::Unauthorized,
    );
    require!(
        ctx.accounts.counter.count < MAX_COUNT,
        ErrorCode::CounterOverflow,
    );

    ctx.accounts.counter.count += 1;
    msg!("Hello, world! Counter is now {}", ctx.accounts.counter.count);
    Ok(())
}
"#
            .into(),
        ),
        (
            src_path.join("state.rs"),
            r#"use anchor_lang_v2::prelude::*;

#[account]
pub struct Counter {
    pub count: u64,
    pub authority: Address,
}
"#
            .into(),
        ),
    ]
}

fn workspace_manifest() -> String {
    format!(
        r#"[workspace]
members = [
    "programs/*"
]
resolver = "2"

[workspace.package]
edition = "2021"
rust-version = "{ANCHOR_MSRV}"

[profile.release]
overflow-checks = true
lto = "fat"
codegen-units = 1
[profile.release.build-override]
opt-level = 3
incremental = false
codegen-units = 1
"#
    )
}

fn cargo_toml(name: &str, test_template: Option<&TestTemplate>) -> String {
    // Template-specific features carried into the emitted `[features]` block:
    //   - Mollusk: `test-sbf` for host-mode integration tests.
    //   - LiteSVM: `profile` forwards to `anchor-v2-testing/profile`, the
    //     register-tracing hook that `anchor test --profile` activates.
    let template_features = match test_template {
        Some(TestTemplate::Mollusk) => r#"test-sbf = []"#,
        Some(TestTemplate::Litesvm) => r#"profile = ["anchor-v2-testing/profile"]"#,
        _ => "",
    };
    let dev_dependencies = match test_template {
        Some(TestTemplate::Mollusk) => {
            r#"
[dev-dependencies]
mollusk-svm = "~0.10"
solana-account = "3"
solana-pubkey = "4"
solana-sdk-ids = "3"
bytemuck = "1"
"#
        }
        Some(TestTemplate::Litesvm) => {
            r#"
[dev-dependencies]
anchor-v2-testing = { git = "https://github.com/solana-foundation/anchor.git", branch = "anchor-next" }
"#
        }
        _ => "",
    };

    format!(
        r#"[package]
name = "{0}"
version = "0.1.0"
description = "Created with Anchor"
edition.workspace = true
rust-version.workspace = true

[lib]
crate-type = ["cdylib", "lib"]
name = "{1}"

[features]
default = []
cpi = ["no-entrypoint"]
no-entrypoint = []
no-log-ix-name = []
idl-build = []
{2}

[dependencies]
# Once anchor-lang-v2 is published to crates.io, swap to: anchor-lang-v2 = "{3}"
anchor-lang-v2 = {{ git = "https://github.com/solana-foundation/anchor.git", branch = "anchor-next" }}
solana-program-log = {{ version = "1.1", features = ["macro"] }}
wincode = {{ version = "0.5", features = ["derive"] }}
{4}

[lints.rust]
unexpected_cfgs = {{ level = "warn", check-cfg = ['cfg(target_os, values("solana"))'] }}
"#,
        name,
        name.to_snake_case(),
        template_features,
        VERSION,
        dev_dependencies,
    )
}

/// Read the program keypair file or create a new one if it doesn't exist.
pub fn get_or_create_program_id(name: &str, target_path: impl AsRef<Path>) -> Pubkey {
    let keypair_path = target_path
        .as_ref()
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

  it("Initializes and increments a counter", async () => {{
    const program = anchor.workspace.{};
    const [counter] = anchor.web3.PublicKey.findProgramAddressSync(
      [Buffer.from("counter")],
      program.programId
    );

    const initializeTx = await program.methods
      .initialize()
      .accountsPartial({{ counter }})
      .rpc();
    console.log("Initialize transaction signature", initializeTx);

    const incrementTx = await program.methods
      .increment()
      .accountsPartial({{ counter }})
      .rpc();
    console.log("Increment transaction signature", incrementTx);
  }});
}});
"#,
        name,
        name.to_lower_camel_case(),
    )
}

pub fn js_jest(name: &str) -> String {
    format!(
        r#"const anchor = require("@anchor-lang/core");

describe("{}", () => {{
  // Configure the client to use the local cluster.
  anchor.setProvider(anchor.AnchorProvider.env());

  it("Initializes and increments a counter", async () => {{
    const program = anchor.workspace.{};
    const [counter] = anchor.web3.PublicKey.findProgramAddressSync(
      [Buffer.from("counter")],
      program.programId
    );

    const initializeTx = await program.methods
      .initialize()
      .accountsPartial({{ counter }})
      .rpc();
    console.log("Initialize transaction signature", initializeTx);

    const incrementTx = await program.methods
      .increment()
      .accountsPartial({{ counter }})
      .rpc();
    console.log("Increment transaction signature", incrementTx);
  }});
}});
"#,
        name,
        name.to_lower_camel_case(),
    )
}

// TODO(anchor-next): bump to `^2.0.0` once the TS package is published.
// Pinned at `^1.0.0` because 2.0.0 isn't on npm yet.
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
    "@anchor-lang/core": "^1.0.0"
  }},
  "devDependencies": {{
    "jest": "^30.3.0",
    "prettier": "^3.8.3"
  }},
  "overrides": {{
    "uuid": "^9.0.1"
  }},
  "resolutions": {{
    "uuid": "^9.0.1"
  }},
  "pnpm": {{
    "overrides": {{
      "uuid": "^9.0.1"
    }}
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
    "@anchor-lang/core": "^1.0.0"
  }},
  "devDependencies": {{
    "chai": "^4.5.0",
    "mocha": "^11.7.5",
    "prettier": "^3.8.3"
  }}
}}
"#
        )
    }
}

// TODO(anchor-next): bump to `^2.0.0` once published (same as `package_json`).
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
    "@anchor-lang/core": "^1.0.0"
  }},
  "devDependencies": {{
    "@types/bn.js": "^5.2.0",
    "@types/jest": "^30.0.0",
    "jest": "^30.3.0",
    "prettier": "^3.8.3",
    "ts-jest": "^29.4.9",
    "typescript": "^5.9.3"
  }},
  "overrides": {{
    "uuid": "^9.0.1"
  }},
  "resolutions": {{
    "uuid": "^9.0.1"
  }},
  "pnpm": {{
    "overrides": {{
      "uuid": "^9.0.1"
    }}
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
    "@anchor-lang/core": "^1.0.0"
  }},
  "devDependencies": {{
    "chai": "^4.5.0",
    "mocha": "^11.7.5",
    "ts-mocha": "^11.1.0",
    "ts-node": "^10.9.2",
    "@types/bn.js": "^5.2.0",
    "@types/chai": "^4.3.0",
    "@types/mocha": "^10.0.10",
    "@types/node": "^25.6.0",
    "typescript": "^5.9.3",
    "prettier": "^3.8.3"
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

  it("Initializes and increments a counter", async () => {{
    const [counter] = anchor.web3.PublicKey.findProgramAddressSync(
      [Buffer.from("counter")],
      program.programId
    );

    const initializeTx = await program.methods
      .initialize()
      .accountsPartial({{ counter }})
      .rpc();
    console.log("Initialize transaction signature", initializeTx);

    const incrementTx = await program.methods
      .increment()
      .accountsPartial({{ counter }})
      .rpc();
    console.log("Increment transaction signature", incrementTx);
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

  it("Initializes and increments a counter", async () => {{
    const [counter] = anchor.web3.PublicKey.findProgramAddressSync(
      [Buffer.from("counter")],
      program.programId
    );

    const initializeTx = await program.methods
      .initialize()
      .accountsPartial({{ counter }})
      .rpc();
    console.log("Initialize transaction signature", initializeTx);

    const incrementTx = await program.methods
      .increment()
      .accountsPartial({{ counter }})
      .rpc();
    console.log("Increment transaction signature", incrementTx);
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
.surfpool
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
#[derive(Clone, Debug, Default, Eq, PartialEq, Parser, ValueEnum, AbsolutePath)]
pub enum TestTemplate {
    /// Generate template for Mocha unit-test
    Mocha,
    /// Generate template for Jest unit-test
    Jest,
    /// Generate template for Rust unit-test
    Rust,
    /// Generate template for Mollusk Rust unit-test
    Mollusk,
    /// Generate template for LiteSVM rust unit-test
    #[default]
    Litesvm,
}

impl TestTemplate {
    pub fn uses_node(&self) -> bool {
        matches!(self, Self::Mocha | Self::Jest)
    }

    pub fn get_test_script(&self, js: bool, pkg_manager: Option<&PackageManager>) -> String {
        let pkg_manager_exec_cmd = match pkg_manager {
            Some(PackageManager::Yarn) => "yarn run",
            Some(PackageManager::NPM) => "npx",
            Some(PackageManager::PNPM) => "pnpm exec",
            Some(PackageManager::Bun) => "bunx",
            None => "",
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
            Self::Rust | Self::Litesvm => "cargo test".to_owned(),
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

                if js {
                    let mut test = File::create(format!("tests/{}.test.js", &project_name))?;
                    test.write_all(js_jest(project_name).as_bytes())?;
                } else {
                    let mut test = File::create(format!("tests/{}.test.ts", &project_name))?;
                    test.write_all(ts_jest(project_name).as_bytes())?;
                }
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
                    .map_err(|e| anyhow::format_err!("{}", e))?;
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

            Self::Litesvm => {
                let tests_path_str = format!("programs/{}/tests", &project_name);
                let tests_path = Path::new(&tests_path_str);
                fs::create_dir_all(tests_path)?;
                let mut files = Vec::new();
                files.extend(create_program_template_litesvm_test(
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
rust-version = "{ANCHOR_MSRV}"

[dependencies]
# Once anchor-client v2 is published to crates.io, swap to: anchor-client = "{VERSION}"
anchor-client = {{ git = "https://github.com/solana-foundation/anchor.git", branch = "anchor-next" }}
{name} = {{ version = "0.1.0", path = "../programs/{name}" }}
solana-keypair = "3.0.0"
solana-pubkey = "3.0.0"
solana-sdk-ids = "3"
solana-signer = "3"
"#
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
                r#"use anchor_client::{{
    CommitmentConfig,
    Client, Cluster,
}};
use solana_keypair::read_keypair_file;
use solana_pubkey::Pubkey;
use solana_signer::Signer;

#[test]
fn test_initialize() {{
    let program_id = "{0}";
    let anchor_wallet = std::env::var("ANCHOR_WALLET").unwrap();
    let payer = read_keypair_file(&anchor_wallet).unwrap();

    let client = Client::new_with_options(Cluster::Localnet, &payer, CommitmentConfig::confirmed());
    let program_id = Pubkey::try_from(program_id).unwrap();
    let program = client.program(program_id).unwrap();
    let counter = Pubkey::find_program_address(
        &[{1}::constants::COUNTER_SEED],
        &program_id,
    )
    .0;

    let initialize_tx = program
        .request()
        .accounts({1}::accounts::Initialize {{
            payer: payer.pubkey(),
            counter,
            system_program: solana_sdk_ids::system_program::id(),
        }})
        .args({1}::instruction::Initialize {{}})
        .send()
        .expect("");

    println!("Initialize transaction signature {{}}", initialize_tx);

    let increment_tx = program
        .request()
        .accounts({1}::accounts::Increment {{
            counter,
            authority: payer.pubkey(),
        }})
        .args({1}::instruction::Increment {{}})
        .send()
        .expect("");

    println!("Increment transaction signature {{}}", increment_tx);
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
    anchor_lang_v2::{{
        accounts::Account, solana_program::instruction::Instruction, InstructionData, Space,
        ToAccountMetas,
    }},
    mollusk_svm::{{program::keyed_account_for_system_program, result::Check, Mollusk}},
    solana_account::Account as SolanaAccount,
    solana_pubkey::Pubkey,
}};

#[test]
fn test_initialize() {{
    let program_id = {0}::id();
    let mollusk = Mollusk::new(&program_id, "{0}");

    let payer = Pubkey::new_unique();
    let counter = Pubkey::find_program_address(
        &[{0}::constants::COUNTER_SEED],
        &program_id,
    )
    .0;

    let instruction = Instruction::new_with_bytes(
        program_id,
        &{0}::instruction::Initialize {{}}.data(),
        {0}::accounts::Initialize {{
            payer,
            counter,
            system_program: solana_sdk_ids::system_program::id(),
        }}
        .to_account_metas(None),
    );

    let counter_space = <Account<{0}::state::Counter> as Space>::INIT_SPACE;
    let accounts = vec![
        (
            payer,
            SolanaAccount::new(1_000_000_000, 0, &solana_sdk_ids::system_program::id()),
        ),
        (counter, SolanaAccount::default()),
        keyed_account_for_system_program(),
    ];

    let result = mollusk.process_and_validate_instruction(
        &instruction,
        &accounts,
        &[Check::success()],
    );

    let counter_account = result
        .resulting_accounts
        .iter()
        .find(|(pk, _)| *pk == counter)
        .map(|(_, a)| a)
        .expect("counter account");
    assert_eq!(counter_account.data.len(), counter_space);
    let counter_state: &{0}::state::Counter = bytemuck::from_bytes(&counter_account.data[8..]);
    assert_eq!(counter_state.count, 0);
    assert_eq!(counter_state.authority, payer);
}}
"#,
            name.to_snake_case(),
        ),
    )]
}

/// Generate template for LiteSVM Rust unit-test
fn create_program_template_litesvm_test(name: &str, tests_path: &Path) -> Files {
    vec![(
        tests_path.join("test_initialize.rs"),
        format!(
            r#"
use {{
    anchor_lang_v2::{{
        accounts::Account, bytemuck, programs::System,
        solana_program::instruction::Instruction, Id, InstructionData, Space, ToAccountMetas,
    }},
    anchor_v2_testing::{{Keypair, LiteSVM, Message, Signer, VersionedMessage, VersionedTransaction}},
}};

#[test]
fn test_initialize() {{
    let program_id = {0}::id();
    let payer = Keypair::new();
    let (counter, _) = anchor_lang_v2::find_program_address(
        &[{0}::constants::COUNTER_SEED],
        &program_id,
    );

    // `svm()` is `LiteSVM::new()` by default. When this crate is built
    // with `--features profile` (which `anchor test --profile` and
    // `anchor debugger` set automatically), it also installs the
    // register-tracing callback that writes per-test SBF traces under
    // `target/anchor-v2-profile/`. The cfg switch lives inside
    // `anchor-v2-testing` so test code stays clean either way.
    let mut svm = anchor_v2_testing::svm();
    let bytes = include_bytes!("../../../target/deploy/{0}.so");
    svm.add_program(program_id, bytes).unwrap();
    svm.airdrop(&payer.pubkey(), 1_000_000_000).unwrap();

    let instruction = Instruction::new_with_bytes(
        program_id,
        &{0}::instruction::Initialize {{}}.data(),
        {0}::accounts::Initialize {{
            payer: payer.pubkey(),
            counter,
            system_program: System::id(),
        }}
        .to_account_metas(None),
    );

    let blockhash = svm.latest_blockhash();
    let msg = Message::new_with_blockhash(&[instruction], Some(&payer.pubkey()), &blockhash);
    let tx = VersionedTransaction::try_new(
        VersionedMessage::Legacy(msg),
        &[&payer],
    )
    .unwrap();

    let res = svm.send_transaction(tx);
    assert!(res.is_ok(), "send_transaction failed: {{:?}}", res);

    // Verify the counter account was initialized. Size comes from the same
    // `Space::INIT_SPACE` expression the `init` constraint allocates with,
    // so the assertion doesn't rot if `Counter` gains fields. The payload
    // tail is a `Pod` struct, so we cast directly and read fields by name
    // instead of hand-slicing bytes.
    let account = svm.get_account(&counter).expect("counter account");
    assert_eq!(account.data.len(), <Account<{0}::state::Counter> as Space>::INIT_SPACE);
    let counter_state: &{0}::state::Counter = bytemuck::from_bytes(&account.data[8..]);
    assert_eq!(counter_state.count, 0);
    assert_eq!(counter_state.authority, payer.pubkey());

    let instruction = Instruction::new_with_bytes(
        program_id,
        &{0}::instruction::Increment {{}}.data(),
        {0}::accounts::Increment {{
            counter,
            authority: payer.pubkey(),
        }}
        .to_account_metas(None),
    );

    let blockhash = svm.latest_blockhash();
    let msg = Message::new_with_blockhash(&[instruction], Some(&payer.pubkey()), &blockhash);
    let tx = VersionedTransaction::try_new(
        VersionedMessage::Legacy(msg),
        &[&payer],
    )
    .unwrap();

    let res = svm.send_transaction(tx);
    assert!(res.is_ok(), "send_transaction failed: {{:?}}", res);

    let account = svm.get_account(&counter).expect("counter account");
    let counter_state: &{0}::state::Counter = bytemuck::from_bytes(&account.data[8..]);
    assert_eq!(counter_state.count, 1);
    assert_eq!(counter_state.authority, payer.pubkey());
}}
"#,
            name.to_snake_case(),
        ),
    )]
}
