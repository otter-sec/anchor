#![cfg_attr(nightly, feature(proc_macro_span))]

use crate::config::{
    AnchorPackage, BootstrapMode, BuildConfig, Config, ConfigOverride, Manifest, ProgramArch,
    ProgramDeployment, ProgramWorkspace, ScriptsConfig, TestValidator, WithPath,
    DEFAULT_LEDGER_PATH, SHUTDOWN_WAIT, STARTUP_WAIT,
};
use anchor_client::Cluster;
use anchor_lang::idl::{IdlAccount, IdlInstruction, ERASED_AUTHORITY};
use anchor_lang::{AccountDeserialize, AnchorDeserialize, AnchorSerialize};
use anchor_lang_idl::convert::convert_idl;
use anchor_lang_idl::types::{Idl, IdlArrayLen, IdlDefinedFields, IdlType, IdlTypeDefTy};
use anyhow::{anyhow, Context, Result};
use checks::{check_anchor_version, check_overflow};
use clap::Parser;
use dirs::home_dir;
use flate2::read::GzDecoder;
use flate2::read::ZlibDecoder;
use flate2::write::{GzEncoder, ZlibEncoder};
use flate2::Compression;
use heck::{ToKebabCase, ToLowerCamelCase, ToPascalCase, ToSnakeCase};
use regex::{Regex, RegexBuilder};
use reqwest::blocking::multipart::{Form, Part};
use reqwest::blocking::Client;
use rust_template::{ProgramTemplate, TestTemplate};
use semver::{Version, VersionReq};
use serde::Deserialize;
use serde_json::{json, Map, Value as JsonValue};
use solana_client::rpc_client::RpcClient;
use solana_program::instruction::{AccountMeta, Instruction};
use solana_sdk::account_utils::StateMut;
use solana_sdk::bpf_loader;
use solana_sdk::bpf_loader_deprecated;
use solana_sdk::bpf_loader_upgradeable::{self, UpgradeableLoaderState};
use solana_sdk::commitment_config::CommitmentConfig;
use solana_sdk::compute_budget::ComputeBudgetInstruction;
use solana_sdk::pubkey::Pubkey;
use solana_sdk::signature::Keypair;
use solana_sdk::signature::Signer;
use solana_sdk::sysvar;
use solana_sdk::transaction::Transaction;
use std::collections::BTreeMap;
use std::collections::HashMap;
use std::collections::HashSet;
use std::ffi::OsString;
use std::fs::{self, File};
use std::io::prelude::*;
use std::path::{Path, PathBuf};
use std::process::{Child, Stdio};
use std::str::FromStr;
use std::string::ToString;
use tar::Archive;

mod checks;
pub mod config;
pub mod rust_template;
pub mod solidity_template;

const DEFAULT_TOOLS_VERSION: &str = "v1.42.1";
const DEFAULT_BUILD_ARCH: &str = "v3";

/// Platform-tools and SBPF architecture passed to cargo-build-sbf.
#[derive(Debug, Clone)]
pub struct BuildSbfOptions {
    tools_version: String,
    arch: String,
}

impl BuildSbfOptions {
    pub fn new(tools_version: String, arch: String) -> Self {
        Self {
            tools_version,
            arch,
        }
    }

    fn args(&self, backend: &ProgramArch) -> Vec<String> {
        if matches!(backend, ProgramArch::Bpf) {
            return Vec::new();
        }
        fn prefixed(value: &str) -> String {
            if value.starts_with('v') {
                value.to_owned()
            } else {
                format!("v{value}")
            }
        }
        vec![
            "--tools-version".to_owned(),
            prefixed(&self.tools_version),
            "--arch".to_owned(),
            prefixed(&self.arch),
        ]
    }
}

impl Default for BuildSbfOptions {
    fn default() -> Self {
        Self::new(
            DEFAULT_TOOLS_VERSION.to_owned(),
            DEFAULT_BUILD_ARCH.to_owned(),
        )
    }
}

// Version of the docker image.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
pub const DOCKER_BUILDER_VERSION: &str = VERSION;

#[derive(Debug, Parser)]
#[clap(version = VERSION)]
pub struct Opts {
    #[clap(flatten)]
    pub cfg_override: ConfigOverride,
    #[clap(subcommand)]
    pub command: Command,
}

#[derive(Debug, Parser)]
pub enum Command {
    /// Initializes a workspace.
    Init {
        /// Workspace name
        name: String,
        /// Use JavaScript instead of TypeScript
        #[clap(short, long)]
        javascript: bool,
        /// Use Solidity instead of Rust
        #[clap(short, long)]
        solidity: bool,
        /// Don't install JavaScript dependencies
        #[clap(long)]
        no_install: bool,
        /// Don't initialize git
        #[clap(long)]
        no_git: bool,
        /// Rust program template to use
        #[clap(value_enum, short, long, default_value = "single")]
        template: ProgramTemplate,
        /// Test template to use
        #[clap(value_enum, long, default_value = "mocha")]
        test_template: TestTemplate,
        /// Initialize even if there are files
        #[clap(long, action)]
        force: bool,
    },
    /// Builds the workspace.
    #[clap(name = "build", alias = "b")]
    Build {
        /// True if the build should not fail even if there are no "CHECK" comments
        #[clap(long)]
        skip_lint: bool,
        /// Do not build the IDL
        #[clap(long)]
        no_idl: bool,
        /// Output directory for the IDL.
        #[clap(short, long)]
        idl: Option<String>,
        /// Output directory for the TypeScript IDL.
        #[clap(short = 't', long)]
        idl_ts: Option<String>,
        /// True if the build artifact needs to be deterministic and verifiable.
        #[clap(short, long)]
        verifiable: bool,
        /// Name of the program to build
        #[clap(short, long)]
        program_name: Option<String>,
        /// Version of the Solana toolchain to use. For --verifiable builds
        /// only.
        #[clap(short, long)]
        solana_version: Option<String>,
        /// Docker image to use. For --verifiable builds only.
        #[clap(short, long)]
        docker_image: Option<String>,
        /// Bootstrap docker image from scratch, installing all requirements for
        /// verifiable builds. Only works for debian-based images.
        #[clap(value_enum, short, long, default_value = "none")]
        bootstrap: BootstrapMode,
        /// Environment variables to pass into the docker container
        #[clap(short, long, required = false)]
        env: Vec<String>,
        /// Arguments to pass to the underlying `cargo build-sbf` command
        #[clap(required = false, last = true)]
        cargo_args: Vec<String>,
        /// Suppress doc strings in IDL output
        #[clap(long)]
        no_docs: bool,
        /// Platform tools version to pass to cargo build-sbf.
        #[clap(long, default_value = DEFAULT_TOOLS_VERSION)]
        tools_version: String,
        /// SBPF architecture to pass to cargo build-sbf.
        #[clap(long, default_value = DEFAULT_BUILD_ARCH)]
        arch: String,
    },
    /// Expands macros (wrapper around cargo expand)
    ///
    /// Use it in a program folder to expand program
    ///
    /// Use it in a workspace but outside a program
    /// folder to expand the entire workspace
    Expand {
        /// Expand only this program
        #[clap(short, long)]
        program_name: Option<String>,
        /// Arguments to pass to the underlying `cargo expand` command
        #[clap(required = false, last = true)]
        cargo_args: Vec<String>,
    },
    /// Verifies the on-chain bytecode matches the locally compiled artifact.
    /// Run this command inside a program subdirectory, i.e., in the dir
    /// containing the program's Cargo.toml.
    Verify {
        /// The deployed program to compare against.
        program_id: Pubkey,
        #[clap(short, long)]
        program_name: Option<String>,
        /// Version of the Solana toolchain to use. For --verifiable builds
        /// only.
        #[clap(short, long)]
        solana_version: Option<String>,
        /// Docker image to use. For --verifiable builds only.
        #[clap(short, long)]
        docker_image: Option<String>,
        /// Bootstrap docker image from scratch, installing all requirements for
        /// verifiable builds. Only works for debian-based images.
        #[clap(value_enum, short, long, default_value = "none")]
        bootstrap: BootstrapMode,
        /// Architecture to use when building the program
        #[clap(value_enum, long, default_value = "sbf")]
        arch: ProgramArch,
        /// Environment variables to pass into the docker container
        #[clap(short, long, required = false)]
        env: Vec<String>,
        /// Arguments to pass to the underlying `cargo build-sbf` command.
        #[clap(required = false, last = true)]
        cargo_args: Vec<String>,
        /// Flag to skip building the program in the workspace,
        /// use this to save time when running verify and the program code is already built.
        #[clap(long, required = false)]
        skip_build: bool,
    },
    #[clap(name = "test", alias = "t")]
    /// Runs integration tests.
    Test {
        /// Build and test only this program
        #[clap(short, long)]
        program_name: Option<String>,
        /// Use this flag if you want to run tests against previously deployed
        /// programs.
        #[clap(long)]
        skip_deploy: bool,
        /// True if the build should not fail even if there are
        /// no "CHECK" comments where normally required
        #[clap(long)]
        skip_lint: bool,
        /// Flag to skip starting a local validator, if the configured cluster
        /// url is a localnet.
        #[clap(long)]
        skip_local_validator: bool,
        /// Flag to skip building the program in the workspace,
        /// use this to save time when running test and the program code is not altered.
        #[clap(long)]
        skip_build: bool,
        /// Architecture to use when building the program
        #[clap(value_enum, long, default_value = "sbf")]
        arch: ProgramArch,
        /// Flag to keep the local validator running after tests
        /// to be able to check the transactions.
        #[clap(long)]
        detach: bool,
        /// Run the test suites under the specified path
        #[clap(long)]
        run: Vec<String>,
        args: Vec<String>,
        /// Environment variables to pass into the docker container
        #[clap(short, long, required = false)]
        env: Vec<String>,
        /// Arguments to pass to the underlying `cargo build-sbf` command.
        #[clap(required = false, last = true)]
        cargo_args: Vec<String>,
    },
    /// Creates a new program.
    New {
        /// Program name
        name: String,
        /// Use Solidity instead of Rust
        #[clap(short, long)]
        solidity: bool,
        /// Rust program template to use
        #[clap(value_enum, short, long, default_value = "single")]
        template: ProgramTemplate,
        /// Create new program even if there is already one
        #[clap(long, action)]
        force: bool,
    },
    /// Commands for interacting with interface definitions.
    Idl {
        #[clap(subcommand)]
        subcmd: IdlCommand,
    },
    /// Remove all artifacts from the target directory except program keypairs.
    Clean,
    /// Deploys each program in the workspace.
    Deploy {
        /// Only deploy this program
        #[clap(short, long)]
        program_name: Option<String>,
        /// Keypair of the program (filepath) (requires program-name)
        #[clap(long, requires = "program_name")]
        program_keypair: Option<String>,
        /// If true, deploy from path target/verifiable
        #[clap(short, long)]
        verifiable: bool,
        /// Arguments to pass to the underlying `solana program deploy` command.
        #[clap(required = false, last = true)]
        solana_args: Vec<String>,
    },
    /// Runs the deploy migration script.
    Migrate,
    /// Deploys, initializes an IDL, and migrates all in one command.
    /// Upgrades a single program. The configured wallet must be the upgrade
    /// authority.
    Upgrade {
        /// The program to upgrade.
        #[clap(short, long)]
        program_id: Pubkey,
        /// Filepath to the new program binary.
        program_filepath: String,
        /// Arguments to pass to the underlying `solana program deploy` command.
        #[clap(required = false, last = true)]
        solana_args: Vec<String>,
    },
    #[cfg(feature = "dev")]
    /// Runs an airdrop loop, continuously funding the configured wallet.
    Airdrop {
        #[clap(short, long)]
        url: Option<String>,
    },
    /// Cluster commands.
    Cluster {
        #[clap(subcommand)]
        subcmd: ClusterCommand,
    },
    /// Starts a node shell with an Anchor client setup according to the local
    /// config.
    Shell,
    /// Runs the script defined by the current workspace's Anchor.toml.
    Run {
        /// The name of the script to run.
        script: String,
        /// Argument to pass to the underlying script.
        #[clap(required = false, last = true)]
        script_args: Vec<String>,
    },
    /// Saves an api token from the registry locally.
    Login {
        /// API access token.
        token: String,
    },
    /// Publishes a verified build to the Anchor registry.
    Publish {
        /// The name of the program to publish.
        program: String,
        /// Environment variables to pass into the docker container
        #[clap(short, long, required = false)]
        env: Vec<String>,
        /// Arguments to pass to the underlying `cargo build-sbf` command.
        #[clap(required = false, last = true)]
        cargo_args: Vec<String>,
        /// Flag to skip building the program in the workspace,
        /// use this to save time when publishing the program
        #[clap(long)]
        skip_build: bool,
        /// Architecture to use when building the program
        #[clap(value_enum, long, default_value = "sbf")]
        arch: ProgramArch,
    },
    /// Keypair commands.
    Keys {
        #[clap(subcommand)]
        subcmd: KeysCommand,
    },
    /// Localnet commands.
    Localnet {
        /// Flag to skip building the program in the workspace,
        /// use this to save time when running test and the program code is not altered.
        #[clap(long)]
        skip_build: bool,
        /// Use this flag if you want to run tests against previously deployed
        /// programs.
        #[clap(long)]
        skip_deploy: bool,
        /// True if the build should not fail even if there are
        /// no "CHECK" comments where normally required
        #[clap(long)]
        skip_lint: bool,
        /// Architecture to use when building the program
        #[clap(value_enum, long, default_value = "sbf")]
        arch: ProgramArch,
        /// Environment variables to pass into the docker container
        #[clap(short, long, required = false)]
        env: Vec<String>,
        /// Arguments to pass to the underlying `cargo build-sbf` command.
        #[clap(required = false, last = true)]
        cargo_args: Vec<String>,
    },
    /// Fetch and deserialize an account using the IDL provided.
    Account {
        /// Account struct to deserialize
        account_type: String,
        /// Address of the account to deserialize
        address: Pubkey,
        /// IDL to use (defaults to workspace IDL)
        #[clap(long)]
        idl: Option<String>,
    },
}

#[derive(Debug, Parser)]
pub enum KeysCommand {
    /// List all of the program keys.
    List,
    /// Sync the program's `declare_id!` pubkey with the program's actual pubkey.
    Sync {
        /// Only sync the given program instead of all programs
        #[clap(short, long)]
        program_name: Option<String>,
    },
}

#[derive(Debug, Parser)]
pub enum IdlCommand {
    /// Initializes a program's IDL account. Can only be run once.
    Init {
        program_id: Pubkey,
        #[clap(short, long)]
        filepath: String,
        #[clap(long)]
        priority_fee: Option<u64>,
    },
    Close {
        program_id: Pubkey,
        /// The IDL account to close. If none is given, then the IDL account derived from program_id is used.
        #[clap(long)]
        idl_address: Option<Pubkey>,
        /// When used, the content of the instruction will only be printed in base64 form and not executed.
        /// Useful for multisig execution when the local wallet keypair is not available.
        #[clap(long)]
        print_only: bool,
        #[clap(long)]
        priority_fee: Option<u64>,
    },
    /// Writes an IDL into a buffer account. This can be used with SetBuffer
    /// to perform an upgrade.
    WriteBuffer {
        program_id: Pubkey,
        #[clap(short, long)]
        filepath: String,
        #[clap(long)]
        priority_fee: Option<u64>,
    },
    /// Sets a new IDL buffer for the program.
    SetBuffer {
        program_id: Pubkey,
        /// Address of the buffer account to set as the idl on the program.
        #[clap(short, long)]
        buffer: Pubkey,
        /// When used, the content of the instruction will only be printed in base64 form and not executed.
        /// Useful for multisig execution when the local wallet keypair is not available.
        #[clap(long)]
        print_only: bool,
        #[clap(long)]
        priority_fee: Option<u64>,
    },
    /// Upgrades the IDL to the new file. An alias for first writing and then
    /// then setting the idl buffer account.
    Upgrade {
        program_id: Pubkey,
        #[clap(short, long)]
        filepath: String,
        #[clap(long)]
        priority_fee: Option<u64>,
    },
    /// Sets a new authority on the IDL account.
    SetAuthority {
        /// The IDL account buffer to set the authority of. If none is given,
        /// then the canonical IDL account is used.
        address: Option<Pubkey>,
        /// Program to change the IDL authority.
        #[clap(short, long)]
        program_id: Pubkey,
        /// New authority of the IDL account.
        #[clap(short, long)]
        new_authority: Pubkey,
        /// When used, the content of the instruction will only be printed in base64 form and not executed.
        /// Useful for multisig execution when the local wallet keypair is not available.
        #[clap(long)]
        print_only: bool,
        #[clap(long)]
        priority_fee: Option<u64>,
    },
    /// Command to remove the ability to modify the IDL account. This should
    /// likely be used in conjection with eliminating an "upgrade authority" on
    /// the program.
    EraseAuthority {
        #[clap(short, long)]
        program_id: Pubkey,
        #[clap(long)]
        priority_fee: Option<u64>,
    },
    /// Outputs the authority for the IDL account.
    Authority {
        /// The program to view.
        program_id: Pubkey,
    },
    /// Generates the IDL for the program using the compilation method.
    Build {
        // Program name to build the IDL of(current dir's program if not specified)
        #[clap(short, long)]
        program_name: Option<String>,
        /// Output file for the IDL (stdout if not specified)
        #[clap(short, long)]
        out: Option<String>,
        /// Output file for the TypeScript IDL
        #[clap(short = 't', long)]
        out_ts: Option<String>,
        /// Suppress doc strings in output
        #[clap(long)]
        no_docs: bool,
        /// Do not check for safety comments
        #[clap(long)]
        skip_lint: bool,
    },
    /// Fetches an IDL for the given address from a cluster.
    /// The address can be a program, IDL account, or IDL buffer.
    Fetch {
        address: Pubkey,
        /// Output file for the IDL (stdout if not specified).
        #[clap(short, long)]
        out: Option<String>,
    },
    /// Convert legacy IDLs (pre Anchor 0.30) to the new IDL spec
    Convert {
        /// Path to the IDL file
        path: String,
        /// Output file for the IDL (stdout if not specified)
        #[clap(short, long)]
        out: Option<String>,
    },
    /// Generate TypeScript type for the IDL
    Type {
        /// Path to the IDL file
        path: String,
        /// Output file for the IDL (stdout if not specified)
        #[clap(short, long)]
        out: Option<String>,
    },
}

#[derive(Debug, Parser)]
pub enum ClusterCommand {
    /// Prints common cluster urls.
    List,
}

fn get_keypair(path: &str) -> Result<Keypair> {
    solana_sdk::signature::read_keypair_file(path)
        .map_err(|_| anyhow!("Unable to read keypair file ({path})"))
}

pub fn entry(opts: Opts) -> Result<()> {
    let restore_cbs = override_toolchain(&opts.cfg_override)?;
    let result = process_command(opts);
    restore_toolchain(restore_cbs)?;

    result
}

/// Functions to restore toolchain entries
type RestoreToolchainCallbacks = Vec<Box<dyn FnOnce() -> Result<()>>>;

/// Override the toolchain from `Anchor.toml`.
///
/// Returns the previous versions to restore back to.
fn override_toolchain(cfg_override: &ConfigOverride) -> Result<RestoreToolchainCallbacks> {
    let mut restore_cbs: RestoreToolchainCallbacks = vec![];

    let cfg = Config::discover(cfg_override)?;
    if let Some(cfg) = cfg {
        fn parse_version(text: &str) -> String {
            Regex::new(r"(\d+\.\d+\.\S+)")
                .unwrap()
                .captures_iter(text)
                .next()
                .unwrap()
                .get(0)
                .unwrap()
                .as_str()
                .to_string()
        }

        fn get_current_version(cmd_name: &str) -> Result<String> {
            let output = std::process::Command::new(cmd_name)
                .arg("--version")
                .output()?;
            if !output.status.success() {
                return Err(anyhow!("Failed to run `{cmd_name} --version`"));
            }

            let output_version = std::str::from_utf8(&output.stdout)?;
            let version = parse_version(output_version);
            Ok(version)
        }

        if let Some(solana_version) = &cfg.toolchain.solana_version {
            let current_version = get_current_version("solana")?;
            if solana_version != &current_version {
                // We are overriding with `solana-install` command instead of using the binaries
                // from `~/.local/share/solana/install/releases` because we use multiple Solana
                // binaries in various commands.
                fn override_solana_version(version: String) -> Result<bool> {
                    let output = std::process::Command::new("solana-install")
                        .arg("list")
                        .output()?;
                    if !output.status.success() {
                        return Err(anyhow!("Failed to list installed `solana` versions"));
                    }

                    // Hide the installation progress if the version is already installed
                    let is_installed = std::str::from_utf8(&output.stdout)?
                        .lines()
                        .any(|line| parse_version(line) == version);
                    let (stderr, stdout) = if is_installed {
                        (Stdio::null(), Stdio::null())
                    } else {
                        (Stdio::inherit(), Stdio::inherit())
                    };

                    std::process::Command::new("solana-install")
                        .arg("init")
                        .arg(&version)
                        .stderr(stderr)
                        .stdout(stdout)
                        .spawn()?
                        .wait()
                        .map(|status| status.success())
                        .map_err(|err| anyhow!("Failed to run `solana-install` command: {err}"))
                }

                match override_solana_version(solana_version.to_owned())? {
                    true => restore_cbs.push(Box::new(|| {
                        match override_solana_version(current_version)? {
                            true => Ok(()),
                            false => Err(anyhow!("Failed to restore `solana` version")),
                        }
                    })),
                    false => eprintln!(
                        "Failed to override `solana` version to {solana_version}, \
                        using {current_version} instead"
                    ),
                }
            }
        }

        // Anchor version override should be handled last
        if let Some(anchor_version) = &cfg.toolchain.anchor_version {
            // Anchor binary name prefix(applies to binaries that are installed via `avm`)
            const ANCHOR_BINARY_PREFIX: &str = "anchor-";

            // Get the current version from the executing binary name if possible because commit
            // based toolchain overrides do not have version information.
            let current_version = std::env::args()
                .next()
                .expect("First arg should exist")
                .parse::<PathBuf>()?
                .file_name()
                .and_then(|name| name.to_str())
                .expect("File name should be valid Unicode")
                .split_once(ANCHOR_BINARY_PREFIX)
                .map(|(_, version)| version)
                .unwrap_or(VERSION)
                .to_owned();
            if anchor_version != &current_version {
                let binary_path = home_dir()
                    .unwrap()
                    .join(".avm")
                    .join("bin")
                    .join(format!("{ANCHOR_BINARY_PREFIX}{anchor_version}"));

                if !binary_path.exists() {
                    eprintln!(
                        "`anchor` {anchor_version} is not installed with `avm`. Installing...\n"
                    );

                    let exit_status = std::process::Command::new("avm")
                        .arg("install")
                        .arg(anchor_version)
                        .spawn()?
                        .wait()?;
                    if !exit_status.success() {
                        eprintln!(
                            "Failed to install `anchor` {anchor_version}, \
                            using {current_version} instead"
                        );

                        return Ok(restore_cbs);
                    }
                }

                let exit_code = std::process::Command::new(binary_path)
                    .args(std::env::args_os().skip(1))
                    .spawn()?
                    .wait()?
                    .code()
                    .unwrap_or(1);
                restore_toolchain(restore_cbs)?;
                std::process::exit(exit_code);
            }
        }
    }

    Ok(restore_cbs)
}

/// Restore toolchain to how it was before the command was run.
fn restore_toolchain(restore_cbs: RestoreToolchainCallbacks) -> Result<()> {
    for restore_toolchain in restore_cbs {
        if let Err(e) = restore_toolchain() {
            eprintln!("Toolchain error: {e}");
        }
    }

    Ok(())
}

/// Get the system's default license - what 'npm init' would use.
fn get_npm_init_license() -> Result<String> {
    let npm_init_license_output = std::process::Command::new("npm")
        .arg("config")
        .arg("get")
        .arg("init-license")
        .output()?;

    if !npm_init_license_output.status.success() {
        return Err(anyhow!("Failed to get npm init license"));
    }

    let license = String::from_utf8(npm_init_license_output.stdout)?;
    Ok(license.trim().to_string())
}

fn process_command(opts: Opts) -> Result<()> {
    match opts.command {
        Command::Init {
            name,
            javascript,
            solidity,
            no_install,
            no_git,
            template,
            test_template,
            force,
        } => init(
            &opts.cfg_override,
            name,
            javascript,
            solidity,
            no_install,
            no_git,
            template,
            test_template,
            force,
        ),
        Command::New {
            solidity,
            name,
            template,
            force,
        } => new(&opts.cfg_override, solidity, name, template, force),
        Command::Build {
            no_idl,
            idl,
            idl_ts,
            verifiable,
            program_name,
            solana_version,
            docker_image,
            bootstrap,
            cargo_args,
            env,
            skip_lint,
            no_docs,
            tools_version,
            arch,
        } => build(
            &opts.cfg_override,
            no_idl,
            idl,
            idl_ts,
            verifiable,
            skip_lint,
            program_name,
            solana_version,
            docker_image,
            bootstrap,
            BuildSbfOptions::new(tools_version, arch),
            None,
            None,
            env,
            cargo_args,
            no_docs,
            ProgramArch::Sbf,
        ),
        Command::Verify {
            program_id,
            program_name,
            solana_version,
            docker_image,
            bootstrap,
            env,
            cargo_args,
            skip_build,
            arch,
        } => verify(
            &opts.cfg_override,
            program_id,
            program_name,
            solana_version,
            docker_image,
            bootstrap,
            env,
            cargo_args,
            skip_build,
            arch,
        ),
        Command::Clean => clean(&opts.cfg_override),
        Command::Deploy {
            program_name,
            program_keypair,
            verifiable,
            solana_args,
        } => deploy(
            &opts.cfg_override,
            program_name,
            program_keypair,
            verifiable,
            solana_args,
        ),
        Command::Expand {
            program_name,
            cargo_args,
        } => expand(&opts.cfg_override, program_name, &cargo_args),
        Command::Upgrade {
            program_id,
            program_filepath,
            solana_args,
        } => upgrade(
            &opts.cfg_override,
            program_id,
            program_filepath,
            solana_args,
        ),
        Command::Idl { subcmd } => idl(&opts.cfg_override, subcmd),
        Command::Migrate => migrate(&opts.cfg_override),
        Command::Test {
            program_name,
            skip_deploy,
            skip_local_validator,
            skip_build,
            detach,
            run,
            args,
            env,
            cargo_args,
            skip_lint,
            arch,
        } => test(
            &opts.cfg_override,
            program_name,
            skip_deploy,
            skip_local_validator,
            skip_build,
            skip_lint,
            detach,
            run,
            args,
            env,
            cargo_args,
            arch,
        ),
        #[cfg(feature = "dev")]
        Command::Airdrop { .. } => airdrop(&opts.cfg_override),
        Command::Cluster { subcmd } => cluster(subcmd),
        Command::Shell => shell(&opts.cfg_override),
        Command::Run {
            script,
            script_args,
        } => run(&opts.cfg_override, script, script_args),
        Command::Login { token } => login(&opts.cfg_override, token),
        Command::Publish {
            program,
            env,
            cargo_args,
            skip_build,
            arch,
        } => publish(
            &opts.cfg_override,
            program,
            env,
            cargo_args,
            skip_build,
            arch,
        ),
        Command::Keys { subcmd } => keys(&opts.cfg_override, subcmd),
        Command::Localnet {
            skip_build,
            skip_deploy,
            skip_lint,
            env,
            cargo_args,
            arch,
        } => localnet(
            &opts.cfg_override,
            skip_build,
            skip_deploy,
            skip_lint,
            env,
            cargo_args,
            arch,
        ),
        Command::Account {
            account_type,
            address,
            idl,
        } => account(&opts.cfg_override, account_type, address, idl),
    }
}

#[allow(clippy::too_many_arguments)]
fn init(
    cfg_override: &ConfigOverride,
    name: String,
    javascript: bool,
    solidity: bool,
    no_install: bool,
    no_git: bool,
    template: ProgramTemplate,
    test_template: TestTemplate,
    force: bool,
) -> Result<()> {
    if !force && Config::discover(cfg_override)?.is_some() {
        return Err(anyhow!("Workspace already initialized"));
    }

    // We need to format different cases for the dir and the name
    let rust_name = name.to_snake_case();
    let project_name = if name == rust_name {
        rust_name.clone()
    } else {
        name.to_kebab_case()
    };

    // Additional keywords that have not been added to the `syn` crate as reserved words
    // https://github.com/dtolnay/syn/pull/1098
    let extra_keywords = ["async", "await", "try"];
    // Anchor converts to snake case before writing the program name
    if syn::parse_str::<syn::Ident>(&rust_name).is_err()
        || extra_keywords.contains(&rust_name.as_str())
    {
        return Err(anyhow!(
            "Anchor workspace name must be a valid Rust identifier. It may not be a Rust reserved word, start with a digit, or include certain disallowed characters. See https://doc.rust-lang.org/reference/identifiers.html for more detail.",
        ));
    }

    if force {
        fs::create_dir_all(&project_name)?;
    } else {
        fs::create_dir(&project_name)?;
    }
    std::env::set_current_dir(&project_name)?;
    fs::create_dir_all("app")?;

    let mut cfg = Config::default();
    let test_script = test_template.get_test_script(javascript);
    cfg.scripts
        .insert("test".to_owned(), test_script.to_owned());

    let mut localnet = BTreeMap::new();
    let program_id = rust_template::get_or_create_program_id(&rust_name);
    localnet.insert(
        rust_name,
        ProgramDeployment {
            address: program_id,
            path: None,
            idl: None,
        },
    );
    cfg.programs.insert(Cluster::Localnet, localnet);
    let toml = cfg.to_string();
    fs::write("Anchor.toml", toml)?;

    // Initialize .gitignore file
    fs::write(".gitignore", rust_template::git_ignore())?;

    // Initialize .prettierignore file
    fs::write(".prettierignore", rust_template::prettier_ignore())?;

    // Remove the default program if `--force` is passed
    if force {
        fs::remove_dir_all(
            std::env::current_dir()?
                .join(if solidity { "solidity" } else { "programs" })
                .join(&project_name),
        )?;
    }

    // Build the program.
    if solidity {
        solidity_template::create_program(&project_name)?;
    } else {
        rust_template::create_program(&project_name, template)?;
    }

    // Build the migrations directory.
    fs::create_dir_all("migrations")?;

    let license = get_npm_init_license()?;

    let jest = TestTemplate::Jest == test_template;
    if javascript {
        // Build javascript config
        let mut package_json = File::create("package.json")?;
        package_json.write_all(rust_template::package_json(jest, license).as_bytes())?;

        let mut deploy = File::create("migrations/deploy.js")?;

        deploy.write_all(rust_template::deploy_script().as_bytes())?;
    } else {
        // Build typescript config
        let mut ts_config = File::create("tsconfig.json")?;
        ts_config.write_all(rust_template::ts_config(jest).as_bytes())?;

        let mut ts_package_json = File::create("package.json")?;
        ts_package_json.write_all(rust_template::ts_package_json(jest, license).as_bytes())?;

        let mut deploy = File::create("migrations/deploy.ts")?;
        deploy.write_all(rust_template::ts_deploy_script().as_bytes())?;
    }

    test_template.create_test_files(
        &project_name,
        javascript,
        solidity,
        &program_id.to_string(),
    )?;

    if !no_install {
        let yarn_result = install_node_modules("yarn")?;
        if !yarn_result.status.success() {
            println!("Failed yarn install will attempt to npm install");
            install_node_modules("npm")?;
        }
    }

    if !no_git {
        let git_result = std::process::Command::new("git")
            .arg("init")
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit())
            .output()
            .map_err(|e| anyhow::format_err!("git init failed: {}", e.to_string()))?;
        if !git_result.status.success() {
            eprintln!("Failed to automatically initialize a new git repository");
        }
    }

    println!("{project_name} initialized");

    Ok(())
}

fn install_node_modules(cmd: &str) -> Result<std::process::Output> {
    if cfg!(target_os = "windows") {
        std::process::Command::new("cmd")
            .arg(format!("/C {cmd} install"))
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit())
            .output()
            .map_err(|e| anyhow::format_err!("{} install failed: {}", cmd, e.to_string()))
    } else {
        std::process::Command::new(cmd)
            .arg("install")
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit())
            .output()
            .map_err(|e| anyhow::format_err!("{} install failed: {}", cmd, e.to_string()))
    }
}

// Creates a new program crate in the `programs/<name>` directory.
fn new(
    cfg_override: &ConfigOverride,
    solidity: bool,
    name: String,
    template: ProgramTemplate,
    force: bool,
) -> Result<()> {
    with_workspace(cfg_override, |cfg| {
        match cfg.path().parent() {
            None => {
                println!("Unable to make new program");
            }
            Some(parent) => {
                std::env::set_current_dir(parent)?;

                let cluster = cfg.provider.cluster.clone();
                let programs = cfg.programs.entry(cluster).or_default();
                if programs.contains_key(&name) {
                    if !force {
                        return Err(anyhow!("Program already exists"));
                    }

                    // Delete all files within the program folder
                    fs::remove_dir_all(
                        std::env::current_dir()?
                            .join(if solidity { "solidity" } else { "programs" })
                            .join(&name),
                    )?;
                }

                if solidity {
                    solidity_template::create_program(&name)?;
                } else {
                    rust_template::create_program(&name, template)?;
                }

                programs.insert(
                    name.clone(),
                    ProgramDeployment {
                        address: rust_template::get_or_create_program_id(&name),
                        path: None,
                        idl: None,
                    },
                );

                let toml = cfg.to_string();
                fs::write("Anchor.toml", toml)?;

                println!("Created new program.");
            }
        };
        Ok(())
    })
}

/// Array of (path, content) tuple.
pub type Files = Vec<(PathBuf, String)>;

/// Create files from the given (path, content) tuple array.
///
/// # Example
///
/// ```ignore
/// crate_files(vec![("programs/my_program/src/lib.rs".into(), "// Content".into())])?;
/// ```
pub fn create_files(files: &Files) -> Result<()> {
    for (path, content) in files {
        let path = Path::new(path);
        if path.exists() {
            continue;
        }

        match path.extension() {
            Some(_) => {
                fs::create_dir_all(path.parent().unwrap())?;
                fs::write(path, content)?;
            }
            None => fs::create_dir_all(path)?,
        }
    }

    Ok(())
}

/// Override or create files from the given (path, content) tuple array.
///
/// # Example
///
/// ```ignore
/// override_or_create_files(vec![("programs/my_program/src/lib.rs".into(), "// Content".into())])?;
/// ```
pub fn override_or_create_files(files: &Files) -> Result<()> {
    for (path, content) in files {
        let path = Path::new(path);
        if path.exists() {
            let mut f = fs::OpenOptions::new()
                .write(true)
                .truncate(true)
                .open(path)?;
            f.write_all(content.as_bytes())?;
            f.flush()?;
        } else {
            fs::create_dir_all(path.parent().unwrap())?;
            fs::write(path, content)?;
        }
    }

    Ok(())
}

pub fn expand(
    cfg_override: &ConfigOverride,
    program_name: Option<String>,
    cargo_args: &[String],
) -> Result<()> {
    // Change to the workspace member directory, if needed.
    if let Some(program_name) = program_name.as_ref() {
        cd_member(cfg_override, program_name)?;
    }

    let workspace_cfg = Config::discover(cfg_override)?.expect("Not in workspace.");
    let cfg_parent = workspace_cfg.path().parent().expect("Invalid Anchor.toml");
    let cargo = Manifest::discover()?;

    let expansions_path = cfg_parent.join(".anchor/expanded-macros");
    fs::create_dir_all(&expansions_path)?;

    match cargo {
        // No Cargo.toml found, expand entire workspace
        None => expand_all(&workspace_cfg, expansions_path, cargo_args),
        // Cargo.toml is at root of workspace, expand entire workspace
        Some(cargo) if cargo.path().parent() == workspace_cfg.path().parent() => {
            expand_all(&workspace_cfg, expansions_path, cargo_args)
        }
        // Reaching this arm means Cargo.toml belongs to a single package. Expand it.
        Some(cargo) => expand_program(
            // If we found Cargo.toml, it must be in a directory so unwrap is safe
            cargo.path().parent().unwrap().to_path_buf(),
            expansions_path,
            cargo_args,
        ),
    }
}

fn expand_all(
    workspace_cfg: &WithPath<Config>,
    expansions_path: PathBuf,
    cargo_args: &[String],
) -> Result<()> {
    let cur_dir = std::env::current_dir()?;
    for p in workspace_cfg.get_rust_program_list()? {
        expand_program(p, expansions_path.clone(), cargo_args)?;
    }
    std::env::set_current_dir(cur_dir)?;
    Ok(())
}

fn expand_program(
    program_path: PathBuf,
    expansions_path: PathBuf,
    cargo_args: &[String],
) -> Result<()> {
    let cargo = Manifest::from_path(program_path.join("Cargo.toml"))
        .map_err(|_| anyhow!("Could not find Cargo.toml for program"))?;

    let target_dir_arg = {
        let mut target_dir_arg = OsString::from("--target-dir=");
        target_dir_arg.push(expansions_path.join("expand-target"));
        target_dir_arg
    };

    let package_name = &cargo
        .package
        .as_ref()
        .ok_or_else(|| anyhow!("Cargo config is missing a package"))?
        .name;
    let program_expansions_path = expansions_path.join(package_name);
    fs::create_dir_all(&program_expansions_path)?;

    let exit = std::process::Command::new("cargo")
        .arg("expand")
        .arg(target_dir_arg)
        .arg(&format!("--package={package_name}"))
        .args(cargo_args)
        .stderr(Stdio::inherit())
        .output()
        .map_err(|e| anyhow::format_err!("{}", e.to_string()))?;
    if !exit.status.success() {
        eprintln!("'anchor expand' failed. Perhaps you have not installed 'cargo-expand'? https://github.com/dtolnay/cargo-expand#installation");
        std::process::exit(exit.status.code().unwrap_or(1));
    }

    let version = cargo.version();
    let time = chrono::Utc::now().to_string().replace(' ', "_");
    let file_path = program_expansions_path.join(format!("{package_name}-{version}-{time}.rs"));
    fs::write(&file_path, &exit.stdout).map_err(|e| anyhow::format_err!("{}", e.to_string()))?;

    println!(
        "Expanded {} into file {}\n",
        package_name,
        file_path.to_string_lossy()
    );
    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub fn build(
    cfg_override: &ConfigOverride,
    no_idl: bool,
    idl: Option<String>,
    idl_ts: Option<String>,
    verifiable: bool,
    skip_lint: bool,
    program_name: Option<String>,
    solana_version: Option<String>,
    docker_image: Option<String>,
    bootstrap: BootstrapMode,
    build_sbf_options: BuildSbfOptions,
    stdout: Option<File>, // Used for the package registry server.
    stderr: Option<File>, // Used for the package registry server.
    env_vars: Vec<String>,
    cargo_args: Vec<String>,
    no_docs: bool,
    arch: ProgramArch,
) -> Result<()> {
    // Change to the workspace member directory, if needed.
    if let Some(program_name) = program_name.as_ref() {
        cd_member(cfg_override, program_name)?;
    }

    let cfg = Config::discover(cfg_override)?.expect("Not in workspace.");
    let cfg_parent = cfg.path().parent().expect("Invalid Anchor.toml");

    // Require overflow checks
    let workspace_cargo_toml_path = cfg_parent.join("Cargo.toml");
    if workspace_cargo_toml_path.exists() {
        check_overflow(workspace_cargo_toml_path)?;
    }

    // Check whether there is a mismatch between CLI and crate/package versions
    check_anchor_version(&cfg).ok();

    let idl_out = match idl {
        Some(idl) => Some(PathBuf::from(idl)),
        None => Some(cfg_parent.join("target/idl")),
    };
    fs::create_dir_all(idl_out.as_ref().unwrap())?;

    let idl_ts_out = match idl_ts {
        Some(idl_ts) => Some(PathBuf::from(idl_ts)),
        None => Some(cfg_parent.join("target/types")),
    };
    fs::create_dir_all(idl_ts_out.as_ref().unwrap())?;

    if !cfg.workspace.types.is_empty() {
        fs::create_dir_all(cfg_parent.join(&cfg.workspace.types))?;
    };

    let cargo = Manifest::discover()?;
    let build_config = BuildConfig {
        verifiable,
        solana_version: solana_version.or_else(|| cfg.toolchain.solana_version.clone()),
        docker_image: docker_image.unwrap_or_else(|| cfg.docker()),
        bootstrap,
        build_sbf_options,
    };
    match cargo {
        // No Cargo.toml so build the entire workspace.
        None => build_all(
            &cfg,
            cfg.path(),
            no_idl,
            idl_out,
            idl_ts_out,
            &build_config,
            stdout,
            stderr,
            env_vars,
            cargo_args,
            skip_lint,
            no_docs,
            arch,
        )?,
        // If the Cargo.toml is at the root, build the entire workspace.
        Some(cargo) if cargo.path().parent() == cfg.path().parent() => build_all(
            &cfg,
            cfg.path(),
            no_idl,
            idl_out,
            idl_ts_out,
            &build_config,
            stdout,
            stderr,
            env_vars,
            cargo_args,
            skip_lint,
            no_docs,
            arch,
        )?,
        // Cargo.toml represents a single package. Build it.
        Some(cargo) => build_rust_cwd(
            &cfg,
            cargo.path().to_path_buf(),
            no_idl,
            idl_out,
            idl_ts_out,
            &build_config,
            stdout,
            stderr,
            env_vars,
            cargo_args,
            skip_lint,
            no_docs,
            &arch,
        )?,
    }

    set_workspace_dir_or_exit();

    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn build_all(
    cfg: &WithPath<Config>,
    cfg_path: &Path,
    no_idl: bool,
    idl_out: Option<PathBuf>,
    idl_ts_out: Option<PathBuf>,
    build_config: &BuildConfig,
    stdout: Option<File>, // Used for the package registry server.
    stderr: Option<File>, // Used for the package registry server.
    env_vars: Vec<String>,
    cargo_args: Vec<String>,
    skip_lint: bool,
    no_docs: bool,
    arch: ProgramArch,
) -> Result<()> {
    let cur_dir = std::env::current_dir()?;
    let r = match cfg_path.parent() {
        None => Err(anyhow!("Invalid Anchor.toml at {}", cfg_path.display())),
        Some(_parent) => {
            for p in cfg.get_rust_program_list()? {
                build_rust_cwd(
                    cfg,
                    p.join("Cargo.toml"),
                    no_idl,
                    idl_out.clone(),
                    idl_ts_out.clone(),
                    build_config,
                    stdout.as_ref().map(|f| f.try_clone()).transpose()?,
                    stderr.as_ref().map(|f| f.try_clone()).transpose()?,
                    env_vars.clone(),
                    cargo_args.clone(),
                    skip_lint,
                    no_docs,
                    &arch,
                )?;
            }
            for (name, path) in cfg.get_solidity_program_list()? {
                build_solidity_cwd(
                    cfg,
                    name,
                    path,
                    idl_out.clone(),
                    idl_ts_out.clone(),
                    build_config,
                    stdout.as_ref().map(|f| f.try_clone()).transpose()?,
                    stderr.as_ref().map(|f| f.try_clone()).transpose()?,
                    cargo_args.clone(),
                )?;
            }
            Ok(())
        }
    };
    std::env::set_current_dir(cur_dir)?;
    r
}

// Runs the build command outside of a workspace.
#[allow(clippy::too_many_arguments)]
fn build_rust_cwd(
    cfg: &WithPath<Config>,
    cargo_toml: PathBuf,
    no_idl: bool,
    idl_out: Option<PathBuf>,
    idl_ts_out: Option<PathBuf>,
    build_config: &BuildConfig,
    stdout: Option<File>,
    stderr: Option<File>,
    env_vars: Vec<String>,
    cargo_args: Vec<String>,
    skip_lint: bool,
    no_docs: bool,
    arch: &ProgramArch,
) -> Result<()> {
    match cargo_toml.parent() {
        None => return Err(anyhow!("Unable to find parent")),
        Some(p) => std::env::set_current_dir(p)?,
    };
    match build_config.verifiable {
        false => _build_rust_cwd(
            cfg,
            no_idl,
            idl_out,
            idl_ts_out,
            skip_lint,
            no_docs,
            arch,
            &build_config.build_sbf_options,
            cargo_args,
        ),
        true => build_cwd_verifiable(
            cfg,
            cargo_toml,
            build_config,
            stdout,
            stderr,
            skip_lint,
            env_vars,
            cargo_args,
            no_docs,
            arch,
        ),
    }
}

// Runs the build command outside of a workspace.
#[allow(clippy::too_many_arguments)]
fn build_solidity_cwd(
    cfg: &WithPath<Config>,
    name: String,
    path: PathBuf,
    idl_out: Option<PathBuf>,
    idl_ts_out: Option<PathBuf>,
    build_config: &BuildConfig,
    stdout: Option<File>,
    stderr: Option<File>,
    cargo_args: Vec<String>,
) -> Result<()> {
    match path.parent() {
        None => return Err(anyhow!("Unable to find parent")),
        Some(p) => std::env::set_current_dir(p)?,
    };
    match build_config.verifiable {
        false => _build_solidity_cwd(
            cfg, &name, &path, idl_out, idl_ts_out, stdout, stderr, cargo_args,
        ),
        true => panic!("verifiable solidity not supported"),
    }
}

// Builds an anchor program in a docker image and copies the build artifacts
// into the `target/` directory.
#[allow(clippy::too_many_arguments)]
fn build_cwd_verifiable(
    cfg: &WithPath<Config>,
    cargo_toml: PathBuf,
    build_config: &BuildConfig,
    stdout: Option<File>,
    stderr: Option<File>,
    skip_lint: bool,
    env_vars: Vec<String>,
    cargo_args: Vec<String>,
    no_docs: bool,
    arch: &ProgramArch,
) -> Result<()> {
    // Create output dirs.
    let workspace_dir = cfg.path().parent().unwrap().canonicalize()?;
    fs::create_dir_all(workspace_dir.join("target/verifiable"))?;
    fs::create_dir_all(workspace_dir.join("target/idl"))?;
    fs::create_dir_all(workspace_dir.join("target/types"))?;
    if !&cfg.workspace.types.is_empty() {
        fs::create_dir_all(workspace_dir.join(&cfg.workspace.types))?;
    }

    let container_name = "anchor-program";

    // Build the binary in docker.
    let result = docker_build(
        cfg,
        container_name,
        cargo_toml,
        build_config,
        stdout,
        stderr,
        env_vars,
        cargo_args,
        arch,
    );

    match &result {
        Err(e) => {
            eprintln!("Error during Docker build: {e:?}");
        }
        Ok(_) => {
            // Build the idl.
            println!("Extracting the IDL");
            let idl = generate_idl(cfg, skip_lint, no_docs)?;
            // Write out the JSON file.
            println!("Writing the IDL file");
            let out_file = workspace_dir.join(format!("target/idl/{}.json", idl.metadata.name));
            write_idl(&idl, OutFile::File(out_file))?;

            // Write out the TypeScript type.
            println!("Writing the .ts file");
            let ts_file = workspace_dir.join(format!("target/types/{}.ts", idl.metadata.name));
            fs::write(&ts_file, idl_ts(&idl)?)?;

            // Copy out the TypeScript type.
            if !&cfg.workspace.types.is_empty() {
                fs::copy(
                    ts_file,
                    workspace_dir
                        .join(&cfg.workspace.types)
                        .join(idl.metadata.name)
                        .with_extension("ts"),
                )?;
            }

            println!("Build success");
        }
    }

    result
}

#[allow(clippy::too_many_arguments)]
fn docker_build(
    cfg: &WithPath<Config>,
    container_name: &str,
    cargo_toml: PathBuf,
    build_config: &BuildConfig,
    stdout: Option<File>,
    stderr: Option<File>,
    env_vars: Vec<String>,
    cargo_args: Vec<String>,
    arch: &ProgramArch,
) -> Result<()> {
    let binary_name = Manifest::from_path(&cargo_toml)?.lib_name()?;

    // Docker vars.
    let workdir = Path::new("/workdir");
    let volume_mount = format!(
        "{}:{}",
        cfg.path().parent().unwrap().canonicalize()?.display(),
        workdir.to_str().unwrap(),
    );
    println!("Using image {:?}", build_config.docker_image);

    // Start the docker image running detached in the background.
    let target_dir = workdir.join("docker-target");
    println!("Run docker image");
    let exit = std::process::Command::new("docker")
        .args([
            "run",
            "-it",
            "-d",
            "--name",
            container_name,
            "--env",
            &format!(
                "CARGO_TARGET_DIR={}",
                target_dir.as_path().to_str().unwrap()
            ),
            "-v",
            &volume_mount,
            "-w",
            workdir.to_str().unwrap(),
            &build_config.docker_image,
            "bash",
        ])
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .output()
        .map_err(|e| anyhow::format_err!("Docker build failed: {}", e.to_string()))?;
    if !exit.status.success() {
        return Err(anyhow!("Failed to build program"));
    }

    let result = docker_prep(container_name, build_config).and_then(|_| {
        let cfg_parent = cfg.path().parent().unwrap();
        docker_build_bpf(
            container_name,
            cargo_toml.as_path(),
            cfg_parent,
            target_dir.as_path(),
            binary_name,
            stdout,
            stderr,
            env_vars,
            cargo_args,
            arch,
            &build_config.build_sbf_options,
        )
    });

    // Cleanup regardless of errors
    docker_cleanup(container_name, target_dir.as_path())?;

    // Done.
    result
}

fn docker_prep(container_name: &str, build_config: &BuildConfig) -> Result<()> {
    // Set the solana version in the container, if given. Otherwise use the
    // default.
    match build_config.bootstrap {
        BootstrapMode::Debian => {
            // Install build requirements
            docker_exec(container_name, &["apt", "update"])?;
            docker_exec(
                container_name,
                &["apt", "install", "-y", "curl", "build-essential"],
            )?;

            // Install Rust
            docker_exec(
                container_name,
                &["curl", "https://sh.rustup.rs", "-sfo", "rustup.sh"],
            )?;
            docker_exec(container_name, &["sh", "rustup.sh", "-y"])?;
            docker_exec(container_name, &["rm", "-f", "rustup.sh"])?;
        }
        BootstrapMode::None => {}
    }

    if let Some(solana_version) = &build_config.solana_version {
        println!("Using solana version: {solana_version}");

        // Install Solana CLI
        docker_exec(
            container_name,
            &[
                "curl",
                "-sSfL",
                &format!("https://release.solana.com/v{solana_version}/install",),
                "-o",
                "solana_installer.sh",
            ],
        )?;
        docker_exec(container_name, &["sh", "solana_installer.sh"])?;
        docker_exec(container_name, &["rm", "-f", "solana_installer.sh"])?;
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn docker_build_bpf(
    container_name: &str,
    cargo_toml: &Path,
    cfg_parent: &Path,
    target_dir: &Path,
    binary_name: String,
    stdout: Option<File>,
    stderr: Option<File>,
    env_vars: Vec<String>,
    cargo_args: Vec<String>,
    arch: &ProgramArch,
    build_sbf_options: &BuildSbfOptions,
) -> Result<()> {
    let manifest_path =
        pathdiff::diff_paths(cargo_toml.canonicalize()?, cfg_parent.canonicalize()?)
            .ok_or_else(|| anyhow!("Unable to diff paths"))?;
    println!(
        "Building {} manifest: {:?}",
        binary_name,
        manifest_path.display()
    );

    let subcommand = arch.build_subcommand();

    // Execute the build.
    let exit = std::process::Command::new("docker")
        .args([
            "exec",
            "--env",
            "PATH=/root/.local/share/solana/install/active_release/bin:/root/.cargo/bin:/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin",
        ])
        .args(env_vars
            .iter()
            .map(|x| ["--env", x.as_str()])
            .collect::<Vec<[&str; 2]>>()
            .concat())
        .args([
            container_name,
            "cargo",
            subcommand,
            "--manifest-path",
            &manifest_path.display().to_string(),
        ])
        .args(build_sbf_options.args(arch))
        .args(cargo_args)
        .stdout(match stdout {
            None => Stdio::inherit(),
            Some(f) => f.into(),
        })
        .stderr(match stderr {
            None => Stdio::inherit(),
            Some(f) => f.into(),
        })
        .output()
        .map_err(|e| anyhow::format_err!("Docker build failed: {}", e.to_string()))?;
    if !exit.status.success() {
        return Err(anyhow!("Failed to build program"));
    }

    // Copy the binary out of the docker image.
    println!("Copying out the build artifacts");
    let out_file = cfg_parent
        .canonicalize()?
        .join(format!("target/verifiable/{binary_name}.so"))
        .display()
        .to_string();

    // This requires the target directory of any built program to be located at
    // the root of the workspace.
    let mut bin_path = target_dir.join("deploy");
    bin_path.push(format!("{binary_name}.so"));
    let bin_artifact = format!(
        "{}:{}",
        container_name,
        bin_path.as_path().to_str().unwrap()
    );
    let exit = std::process::Command::new("docker")
        .args(["cp", &bin_artifact, &out_file])
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .output()
        .map_err(|e| anyhow::format_err!("{}", e.to_string()))?;
    if !exit.status.success() {
        Err(anyhow!(
            "Failed to copy binary out of docker. Is the target directory set correctly?"
        ))
    } else {
        Ok(())
    }
}

fn docker_cleanup(container_name: &str, target_dir: &Path) -> Result<()> {
    // Wipe the generated docker-target dir.
    println!("Cleaning up the docker target directory");
    docker_exec(container_name, &["rm", "-rf", target_dir.to_str().unwrap()])?;

    // Remove the docker image.
    println!("Removing the docker container");
    let exit = std::process::Command::new("docker")
        .args(["rm", "-f", container_name])
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .output()
        .map_err(|e| anyhow::format_err!("{}", e.to_string()))?;
    if !exit.status.success() {
        println!("Unable to remove the docker container");
        std::process::exit(exit.status.code().unwrap_or(1));
    }
    Ok(())
}

fn docker_exec(container_name: &str, args: &[&str]) -> Result<()> {
    let exit = std::process::Command::new("docker")
        .args([&["exec", container_name], args].concat())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .output()
        .map_err(|e| anyhow!("Failed to run command \"{:?}\": {:?}", args, e))?;
    if !exit.status.success() {
        Err(anyhow!("Failed to run command: {:?}", args))
    } else {
        Ok(())
    }
}

#[allow(clippy::too_many_arguments)]
fn _build_rust_cwd(
    cfg: &WithPath<Config>,
    no_idl: bool,
    idl_out: Option<PathBuf>,
    idl_ts_out: Option<PathBuf>,
    skip_lint: bool,
    no_docs: bool,
    arch: &ProgramArch,
    build_sbf_options: &BuildSbfOptions,
    cargo_args: Vec<String>,
) -> Result<()> {
    let subcommand = arch.build_subcommand();
    let exit = std::process::Command::new("cargo")
        .arg(subcommand)
        .args(build_sbf_options.args(arch))
        .args(cargo_args)
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .output()
        .map_err(|e| anyhow::format_err!("{}", e.to_string()))?;
    if !exit.status.success() {
        std::process::exit(exit.status.code().unwrap_or(1));
    }

    // Generate IDL
    if !no_idl {
        let idl = generate_idl(cfg, skip_lint, no_docs)?;

        // JSON out path.
        let out = match idl_out {
            None => PathBuf::from(".")
                .join(&idl.metadata.name)
                .with_extension("json"),
            Some(o) => PathBuf::from(&o.join(&idl.metadata.name).with_extension("json")),
        };
        // TS out path.
        let ts_out = match idl_ts_out {
            None => PathBuf::from(".")
                .join(&idl.metadata.name)
                .with_extension("ts"),
            Some(o) => PathBuf::from(&o.join(&idl.metadata.name).with_extension("ts")),
        };

        // Write out the JSON file.
        write_idl(&idl, OutFile::File(out))?;
        // Write out the TypeScript type.
        fs::write(&ts_out, idl_ts(&idl)?)?;

        // Copy out the TypeScript type.
        let cfg_parent = cfg.path().parent().expect("Invalid Anchor.toml");
        if !&cfg.workspace.types.is_empty() {
            fs::copy(
                &ts_out,
                cfg_parent
                    .join(&cfg.workspace.types)
                    .join(&idl.metadata.name)
                    .with_extension("ts"),
            )?;
        }
    }

    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn _build_solidity_cwd(
    cfg: &WithPath<Config>,
    name: &str,
    path: &Path,
    idl_out: Option<PathBuf>,
    idl_ts_out: Option<PathBuf>,
    stdout: Option<File>,
    stderr: Option<File>,
    solang_args: Vec<String>,
) -> Result<()> {
    let mut cmd = std::process::Command::new("solang");
    let cmd = cmd.args(["compile", "--target", "solana", "--contract", name]);

    if let Some(idl_out) = &idl_out {
        cmd.arg("--output-meta");
        cmd.arg(idl_out);
    }

    let target_bin = cfg.path().parent().unwrap().join("target").join("deploy");

    cmd.arg("--output");
    cmd.arg(target_bin);
    cmd.arg("--verbose");
    cmd.arg(path);

    let exit = cmd
        .args(solang_args)
        .stdout(match stdout {
            None => Stdio::inherit(),
            Some(f) => f.into(),
        })
        .stderr(match stderr {
            None => Stdio::inherit(),
            Some(f) => f.into(),
        })
        .output()
        .map_err(|e| anyhow::format_err!("{}", e.to_string()))?;
    if !exit.status.success() {
        std::process::exit(exit.status.code().unwrap_or(1));
    }

    // idl is written to idl_out or .
    let idl_path = idl_out
        .unwrap_or(PathBuf::from("."))
        .join(format!("{}.json", name));

    let idl = fs::read_to_string(idl_path)?;

    let idl: Idl = serde_json::from_str(&idl)?;

    // TS out path.
    let ts_out = match idl_ts_out {
        None => PathBuf::from(".")
            .join(&idl.metadata.name)
            .with_extension("ts"),
        Some(o) => PathBuf::from(&o.join(&idl.metadata.name).with_extension("ts")),
    };

    // Write out the TypeScript type.
    fs::write(&ts_out, idl_ts(&idl)?)?;
    // Copy out the TypeScript type.
    let cfg_parent = cfg.path().parent().expect("Invalid Anchor.toml");
    if !&cfg.workspace.types.is_empty() {
        fs::copy(
            &ts_out,
            cfg_parent
                .join(&cfg.workspace.types)
                .join(&idl.metadata.name)
                .with_extension("ts"),
        )?;
    }

    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn verify(
    cfg_override: &ConfigOverride,
    program_id: Pubkey,
    program_name: Option<String>,
    solana_version: Option<String>,
    docker_image: Option<String>,
    bootstrap: BootstrapMode,
    env_vars: Vec<String>,
    cargo_args: Vec<String>,
    skip_build: bool,
    arch: ProgramArch,
) -> Result<()> {
    // Change to the workspace member directory, if needed.
    if let Some(program_name) = program_name.as_ref() {
        cd_member(cfg_override, program_name)?;
    }

    // Proceed with the command.
    let cfg = Config::discover(cfg_override)?.expect("Not in workspace.");
    let cargo = Manifest::discover()?.ok_or_else(|| anyhow!("Cargo.toml not found"))?;

    // Build the program we want to verify.
    let cur_dir = std::env::current_dir()?;
    if !skip_build {
        build(
            cfg_override,
            false,
            None,
            None,
            true,
            true,
            None,
            solana_version.or_else(|| cfg.toolchain.solana_version.clone()),
            docker_image,
            bootstrap,
            BuildSbfOptions::default(),
            None,
            None,
            env_vars,
            cargo_args,
            false,
            arch,
        )?;
    }
    std::env::set_current_dir(cur_dir)?;

    // Verify binary.
    let binary_name = cargo.lib_name()?;
    let bin_path = cfg
        .path()
        .parent()
        .ok_or_else(|| anyhow!("Unable to find workspace root"))?
        .join("target/verifiable/")
        .join(format!("{binary_name}.so"));

    let url = cluster_url(&cfg, &cfg.test_validator);
    let bin_ver = verify_bin(program_id, &bin_path, &url)?;
    if !bin_ver.is_verified {
        println!("Error: Binaries don't match");
        std::process::exit(1);
    }

    // Verify IDL (only if it's not a buffer account).
    let local_idl = generate_idl(&cfg, true, false)?;
    if bin_ver.state != BinVerificationState::Buffer {
        let deployed_idl = fetch_idl(cfg_override, program_id)?;
        if local_idl != deployed_idl {
            println!("Error: IDLs don't match");
            std::process::exit(1);
        }
    }

    println!("{program_id} is verified.");

    Ok(())
}

fn cd_member(cfg_override: &ConfigOverride, program_name: &str) -> Result<()> {
    // Change directories to the given `program_name`, if given.
    let cfg = Config::discover(cfg_override)?.expect("Not in workspace.");

    for program in cfg.read_all_programs()? {
        if program.solidity {
            if let Some(path) = program.path.parent() {
                std::env::set_current_dir(path)?;
                return Ok(());
            }
        } else {
            let cargo_toml = program.path.join("Cargo.toml");
            if !cargo_toml.exists() {
                return Err(anyhow!(
                    "Did not find Cargo.toml at the path: {}",
                    program.path.display()
                ));
            }

            let manifest = Manifest::from_path(&cargo_toml)?;
            let pkg_name = manifest.package().name();
            let lib_name = manifest.lib_name()?;
            if program_name == pkg_name || program_name == lib_name {
                std::env::set_current_dir(&program.path)?;
                return Ok(());
            }
        }
    }

    Err(anyhow!("{} is not part of the workspace", program_name,))
}

pub fn verify_bin(program_id: Pubkey, bin_path: &Path, cluster: &str) -> Result<BinVerification> {
    // Use `finalized` state for verify
    let client = RpcClient::new_with_commitment(cluster, CommitmentConfig::finalized());

    // Get the deployed build artifacts.
    let (deployed_bin, state) = {
        let account = client.get_account(&program_id)?;
        if account.owner == bpf_loader::id() || account.owner == bpf_loader_deprecated::id() {
            let bin = account.data.to_vec();
            let state = BinVerificationState::ProgramData {
                slot: 0, // Need to look through the transaction history.
                upgrade_authority_address: None,
            };
            (bin, state)
        } else if account.owner == bpf_loader_upgradeable::id() {
            match account.state()? {
                UpgradeableLoaderState::Program {
                    programdata_address,
                } => {
                    let account = client.get_account(&programdata_address)?;
                    let bin = account.data
                        [UpgradeableLoaderState::size_of_programdata_metadata()..]
                        .to_vec();

                    if let UpgradeableLoaderState::ProgramData {
                        slot,
                        upgrade_authority_address,
                    } = account.state()?
                    {
                        let state = BinVerificationState::ProgramData {
                            slot,
                            upgrade_authority_address,
                        };
                        (bin, state)
                    } else {
                        return Err(anyhow!("Expected program data"));
                    }
                }
                UpgradeableLoaderState::Buffer { .. } => {
                    let offset = UpgradeableLoaderState::size_of_buffer_metadata();
                    (
                        account.data[offset..].to_vec(),
                        BinVerificationState::Buffer,
                    )
                }
                _ => {
                    return Err(anyhow!(
                        "Invalid program id, not a buffer or program account"
                    ))
                }
            }
        } else {
            return Err(anyhow!(
                "Invalid program id, not owned by any loader program"
            ));
        }
    };
    let mut local_bin = {
        let mut f = File::open(bin_path)?;
        let mut contents = vec![];
        f.read_to_end(&mut contents)?;
        contents
    };

    // The deployed program probably has zero bytes appended. The default is
    // 2x the binary size in case of an upgrade.
    if local_bin.len() < deployed_bin.len() {
        local_bin.append(&mut vec![0; deployed_bin.len() - local_bin.len()]);
    }

    // Finally, check the bytes.
    let is_verified = local_bin == deployed_bin;

    Ok(BinVerification { state, is_verified })
}

#[derive(PartialEq, Eq)]
pub struct BinVerification {
    pub state: BinVerificationState,
    pub is_verified: bool,
}

#[derive(PartialEq, Eq)]
pub enum BinVerificationState {
    Buffer,
    ProgramData {
        slot: u64,
        upgrade_authority_address: Option<Pubkey>,
    },
}

fn idl(cfg_override: &ConfigOverride, subcmd: IdlCommand) -> Result<()> {
    match subcmd {
        IdlCommand::Init {
            program_id,
            filepath,
            priority_fee,
        } => idl_init(cfg_override, program_id, filepath, priority_fee),
        IdlCommand::Close {
            program_id,
            idl_address,
            print_only,
            priority_fee,
        } => {
            let closed_address = idl_close(
                cfg_override,
                program_id,
                idl_address,
                print_only,
                priority_fee,
            )?;
            if !print_only {
                println!("Idl account closed: {closed_address}");
            }
            Ok(())
        }
        IdlCommand::WriteBuffer {
            program_id,
            filepath,
            priority_fee,
        } => {
            let idl_buffer = idl_write_buffer(cfg_override, program_id, filepath, priority_fee)?;
            println!("Idl buffer created: {idl_buffer}");
            Ok(())
        }
        IdlCommand::SetBuffer {
            program_id,
            buffer,
            print_only,
            priority_fee,
        } => idl_set_buffer(cfg_override, program_id, buffer, print_only, priority_fee).map(|_| ()),
        IdlCommand::Upgrade {
            program_id,
            filepath,
            priority_fee,
        } => idl_upgrade(cfg_override, program_id, filepath, priority_fee),
        IdlCommand::SetAuthority {
            program_id,
            address,
            new_authority,
            print_only,
            priority_fee,
        } => idl_set_authority(
            cfg_override,
            program_id,
            address,
            new_authority,
            print_only,
            priority_fee,
        ),
        IdlCommand::EraseAuthority {
            program_id,
            priority_fee,
        } => idl_erase_authority(cfg_override, program_id, priority_fee),
        IdlCommand::Authority { program_id } => idl_authority(cfg_override, program_id),
        IdlCommand::Build {
            program_name,
            out,
            out_ts,
            no_docs,
            skip_lint,
        } => idl_build(cfg_override, program_name, out, out_ts, no_docs, skip_lint),
        IdlCommand::Fetch { address, out } => idl_fetch(cfg_override, address, out),
        IdlCommand::Convert { path, out } => idl_convert(path, out),
        IdlCommand::Type { path, out } => idl_type(path, out),
    }
}

/// Fetch an IDL for the given program id.
fn fetch_idl(cfg_override: &ConfigOverride, idl_addr: Pubkey) -> Result<Idl> {
    let url = match Config::discover(cfg_override)? {
        Some(cfg) => cluster_url(&cfg, &cfg.test_validator),
        None => {
            // If the command is not run inside a workspace,
            // cluster_url will be used from default solana config
            // provider.cluster option can be used to override this

            if let Some(cluster) = cfg_override.cluster.clone() {
                cluster.url().to_string()
            } else {
                config::get_solana_cfg_url()?
            }
        }
    };

    let client = create_client(url);

    let mut account = client.get_account(&idl_addr)?;
    if account.executable {
        let idl_addr = IdlAccount::address(&idl_addr);
        account = client.get_account(&idl_addr)?;
    }

    // Cut off account discriminator.
    let mut d: &[u8] = &account.data[8..];
    let idl_account: IdlAccount = AnchorDeserialize::deserialize(&mut d)?;

    let compressed_len: usize = idl_account.data_len.try_into().unwrap();
    let compressed_bytes = &account.data[44..44 + compressed_len];
    let mut z = ZlibDecoder::new(compressed_bytes);
    let mut s = Vec::new();
    z.read_to_end(&mut s)?;
    serde_json::from_slice(&s[..]).map_err(Into::into)
}

fn get_idl_account(client: &RpcClient, idl_address: &Pubkey) -> Result<IdlAccount> {
    let account = client.get_account(idl_address)?;
    let mut data: &[u8] = &account.data;
    AccountDeserialize::try_deserialize(&mut data).map_err(|e| anyhow!("{:?}", e))
}

fn idl_init(
    cfg_override: &ConfigOverride,
    program_id: Pubkey,
    idl_filepath: String,
    priority_fee: Option<u64>,
) -> Result<()> {
    with_workspace(cfg_override, |cfg| {
        let keypair = cfg.provider.wallet.to_string();

        let bytes = fs::read(idl_filepath)?;
        let idl: Idl = serde_json::from_reader(&*bytes)?;

        let idl_address = create_idl_account(cfg, &keypair, &program_id, &idl, priority_fee)?;

        println!("Idl account created: {idl_address:?}");
        Ok(())
    })
}

fn idl_close(
    cfg_override: &ConfigOverride,
    program_id: Pubkey,
    idl_address: Option<Pubkey>,
    print_only: bool,
    priority_fee: Option<u64>,
) -> Result<Pubkey> {
    with_workspace(cfg_override, |cfg| {
        let idl_address = idl_address.unwrap_or_else(|| IdlAccount::address(&program_id));
        idl_close_account(cfg, &program_id, idl_address, print_only, priority_fee)?;

        Ok(idl_address)
    })
}

fn idl_write_buffer(
    cfg_override: &ConfigOverride,
    program_id: Pubkey,
    idl_filepath: String,
    priority_fee: Option<u64>,
) -> Result<Pubkey> {
    with_workspace(cfg_override, |cfg| {
        let keypair = cfg.provider.wallet.to_string();

        let bytes = fs::read(idl_filepath)?;
        let idl: Idl = serde_json::from_reader(&*bytes)?;

        let idl_buffer = create_idl_buffer(cfg, &keypair, &program_id, &idl, priority_fee)?;
        idl_write(cfg, &program_id, &idl, idl_buffer, priority_fee)?;

        Ok(idl_buffer)
    })
}

fn idl_set_buffer(
    cfg_override: &ConfigOverride,
    program_id: Pubkey,
    buffer: Pubkey,
    print_only: bool,
    priority_fee: Option<u64>,
) -> Result<Pubkey> {
    with_workspace(cfg_override, |cfg| {
        let keypair = get_keypair(&cfg.provider.wallet.to_string())?;
        let url = cluster_url(cfg, &cfg.test_validator);
        let client = create_client(url);

        let idl_address = IdlAccount::address(&program_id);
        let idl_authority = if print_only {
            get_idl_account(&client, &idl_address)?.authority
        } else {
            keypair.pubkey()
        };
        // Instruction to set the buffer onto the IdlAccount.
        let ix = {
            let accounts = vec![
                AccountMeta::new(buffer, false),
                AccountMeta::new(idl_address, false),
                AccountMeta::new(idl_authority, true),
            ];
            let mut data = anchor_lang::idl::IDL_IX_TAG.to_le_bytes().to_vec();
            data.append(&mut IdlInstruction::SetBuffer.try_to_vec()?);
            Instruction {
                program_id,
                accounts,
                data,
            }
        };

        if print_only {
            print_idl_instruction("SetBuffer", &ix, &idl_address)?;
        } else {
            // Build the transaction.
            let instructions = prepend_compute_unit_ix(vec![ix], &client, priority_fee)?;

            // Send the transaction.
            let mut latest_hash = client.get_latest_blockhash()?;
            for retries in 0..20 {
                if !client.is_blockhash_valid(&latest_hash, client.commitment())? {
                    latest_hash = client.get_latest_blockhash()?;
                }
                let tx = Transaction::new_signed_with_payer(
                    &instructions,
                    Some(&keypair.pubkey()),
                    &[&keypair],
                    latest_hash,
                );

                match client.send_and_confirm_transaction_with_spinner(&tx) {
                    Ok(_) => break,
                    Err(e) => {
                        if retries == 19 {
                            return Err(anyhow!("Error: {e}. Failed to send transaction."));
                        }
                        println!("Error: {e}. Retrying transaction.");
                    }
                }
            }
        }

        Ok(idl_address)
    })
}

fn idl_upgrade(
    cfg_override: &ConfigOverride,
    program_id: Pubkey,
    idl_filepath: String,
    priority_fee: Option<u64>,
) -> Result<()> {
    let buffer_address = idl_write_buffer(cfg_override, program_id, idl_filepath, priority_fee)?;
    let idl_address = idl_set_buffer(
        cfg_override,
        program_id,
        buffer_address,
        false,
        priority_fee,
    )?;
    idl_close(
        cfg_override,
        program_id,
        Some(buffer_address),
        false,
        priority_fee,
    )?;
    println!("Idl account {idl_address} successfully upgraded");
    Ok(())
}

fn idl_authority(cfg_override: &ConfigOverride, program_id: Pubkey) -> Result<()> {
    with_workspace(cfg_override, |cfg| {
        let url = cluster_url(cfg, &cfg.test_validator);
        let client = create_client(url);
        let idl_address = {
            let account = client.get_account(&program_id)?;
            if account.executable {
                IdlAccount::address(&program_id)
            } else {
                program_id
            }
        };

        let idl_account = get_idl_account(&client, &idl_address)?;

        println!("{:?}", idl_account.authority);

        Ok(())
    })
}

fn idl_set_authority(
    cfg_override: &ConfigOverride,
    program_id: Pubkey,
    address: Option<Pubkey>,
    new_authority: Pubkey,
    print_only: bool,
    priority_fee: Option<u64>,
) -> Result<()> {
    with_workspace(cfg_override, |cfg| {
        // Misc.
        let idl_address = match address {
            None => IdlAccount::address(&program_id),
            Some(addr) => addr,
        };
        let keypair = get_keypair(&cfg.provider.wallet.to_string())?;
        let url = cluster_url(cfg, &cfg.test_validator);
        let client = create_client(url);

        let idl_authority = if print_only {
            get_idl_account(&client, &idl_address)?.authority
        } else {
            keypair.pubkey()
        };

        // Instruction data.
        let data =
            serialize_idl_ix(anchor_lang::idl::IdlInstruction::SetAuthority { new_authority })?;

        // Instruction accounts.
        let accounts = vec![
            AccountMeta::new(idl_address, false),
            AccountMeta::new_readonly(idl_authority, true),
        ];

        // Instruction.
        let ix = Instruction {
            program_id,
            accounts,
            data,
        };

        if print_only {
            print_idl_instruction("SetAuthority", &ix, &idl_address)?;
        } else {
            let instructions = prepend_compute_unit_ix(vec![ix], &client, priority_fee)?;

            // Send transaction.
            let latest_hash = client.get_latest_blockhash()?;
            let tx = Transaction::new_signed_with_payer(
                &instructions,
                Some(&keypair.pubkey()),
                &[&keypair],
                latest_hash,
            );
            client.send_and_confirm_transaction_with_spinner(&tx)?;

            println!("Authority update complete.");
        }

        Ok(())
    })
}

fn idl_erase_authority(
    cfg_override: &ConfigOverride,
    program_id: Pubkey,
    priority_fee: Option<u64>,
) -> Result<()> {
    println!("Are you sure you want to erase the IDL authority: [y/n]");

    let stdin = std::io::stdin();
    let mut stdin_lines = stdin.lock().lines();
    let input = stdin_lines.next().unwrap().unwrap();
    if input != "y" {
        println!("Not erasing.");
        return Ok(());
    }

    idl_set_authority(
        cfg_override,
        program_id,
        None,
        ERASED_AUTHORITY,
        false,
        priority_fee,
    )?;

    Ok(())
}

fn idl_close_account(
    cfg: &Config,
    program_id: &Pubkey,
    idl_address: Pubkey,
    print_only: bool,
    priority_fee: Option<u64>,
) -> Result<()> {
    let keypair = get_keypair(&cfg.provider.wallet.to_string())?;
    let url = cluster_url(cfg, &cfg.test_validator);
    let client = create_client(url);

    let idl_authority = if print_only {
        get_idl_account(&client, &idl_address)?.authority
    } else {
        keypair.pubkey()
    };
    // Instruction accounts.
    let accounts = vec![
        AccountMeta::new(idl_address, false),
        AccountMeta::new_readonly(idl_authority, true),
        AccountMeta::new(keypair.pubkey(), false),
    ];
    // Instruction.
    let ix = Instruction {
        program_id: *program_id,
        accounts,
        data: { serialize_idl_ix(anchor_lang::idl::IdlInstruction::Close {})? },
    };

    if print_only {
        print_idl_instruction("Close", &ix, &idl_address)?;
    } else {
        let instructions = prepend_compute_unit_ix(vec![ix], &client, priority_fee)?;

        // Send transaction.
        let latest_hash = client.get_latest_blockhash()?;
        let tx = Transaction::new_signed_with_payer(
            &instructions,
            Some(&keypair.pubkey()),
            &[&keypair],
            latest_hash,
        );
        client.send_and_confirm_transaction_with_spinner(&tx)?;
    }

    Ok(())
}

// Write the idl to the account buffer, chopping up the IDL into pieces
// and sending multiple transactions in the event the IDL doesn't fit into
// a single transaction.
fn idl_write(
    cfg: &Config,
    program_id: &Pubkey,
    idl: &Idl,
    idl_address: Pubkey,
    priority_fee: Option<u64>,
) -> Result<()> {
    // Misc.
    let keypair = get_keypair(&cfg.provider.wallet.to_string())?;
    let url = cluster_url(cfg, &cfg.test_validator);
    let client = create_client(url);

    // Serialize and compress the idl.
    let idl_data = {
        let json_bytes = serde_json::to_vec(idl)?;
        let mut e = ZlibEncoder::new(Vec::new(), Compression::default());
        e.write_all(&json_bytes)?;
        e.finish()?
    };

    println!("Idl data length: {:?} bytes", idl_data.len());

    const MAX_WRITE_SIZE: usize = 600;
    let mut offset = 0;
    while offset < idl_data.len() {
        println!("Step {offset}/{} ", idl_data.len());
        // Instruction data.
        let data = {
            let start = offset;
            let end = std::cmp::min(offset + MAX_WRITE_SIZE, idl_data.len());
            serialize_idl_ix(anchor_lang::idl::IdlInstruction::Write {
                data: idl_data[start..end].to_vec(),
            })?
        };
        // Instruction accounts.
        let accounts = vec![
            AccountMeta::new(idl_address, false),
            AccountMeta::new_readonly(keypair.pubkey(), true),
        ];
        // Instruction.
        let ix = Instruction {
            program_id: *program_id,
            accounts,
            data,
        };
        // Send transaction.
        let instructions = prepend_compute_unit_ix(vec![ix], &client, priority_fee)?;

        let mut latest_hash = client.get_latest_blockhash()?;
        for retries in 0..20 {
            if !client.is_blockhash_valid(&latest_hash, client.commitment())? {
                latest_hash = client.get_latest_blockhash()?;
            }
            let tx = Transaction::new_signed_with_payer(
                &instructions,
                Some(&keypair.pubkey()),
                &[&keypair],
                latest_hash,
            );

            match client.send_and_confirm_transaction_with_spinner(&tx) {
                Ok(_) => break,
                Err(e) => {
                    if retries == 19 {
                        return Err(anyhow!("Error: {e}. Failed to send transaction."));
                    }
                    println!("Error: {e}. Retrying transaction.");
                }
            }
        }

        offset += MAX_WRITE_SIZE;
    }
    Ok(())
}

fn idl_build(
    cfg_override: &ConfigOverride,
    program_name: Option<String>,
    out: Option<String>,
    out_ts: Option<String>,
    no_docs: bool,
    skip_lint: bool,
) -> Result<()> {
    let cfg = Config::discover(cfg_override)?.expect("Not in workspace");
    let program_path = match program_name {
        Some(name) => cfg.get_program(&name)?.path,
        None => {
            let current_dir = std::env::current_dir()?;
            cfg.read_all_programs()?
                .into_iter()
                .find(|program| program.path == current_dir)
                .ok_or_else(|| anyhow!("Not in a program directory"))?
                .path
        }
    };
    let idl = anchor_lang_idl::build::build_idl(
        program_path,
        cfg.features.resolution,
        cfg.features.skip_lint || skip_lint,
        no_docs,
    )?;
    let out = match out {
        Some(path) => OutFile::File(PathBuf::from(path)),
        None => OutFile::Stdout,
    };
    write_idl(&idl, out)?;

    if let Some(path) = out_ts {
        fs::write(path, idl_ts(&idl)?)?;
    }

    Ok(())
}

/// Generate IDL with method decided by whether manifest file has `idl-build` feature or not.
fn generate_idl(cfg: &WithPath<Config>, skip_lint: bool, no_docs: bool) -> Result<Idl> {
    // Check whether the manifest has `idl-build` feature
    let manifest = Manifest::discover()?.ok_or_else(|| anyhow!("Cargo.toml not found"))?;
    let is_idl_build = manifest
        .features
        .iter()
        .any(|(feature, _)| feature == "idl-build");
    if !is_idl_build {
        let path = manifest.path().display();
        let anchor_spl_idl_build = manifest
            .dependencies
            .iter()
            .any(|dep| dep.0 == "anchor-spl")
            .then_some(r#", "anchor-spl/idl-build""#)
            .unwrap_or_default();

        return Err(anyhow!(
            r#"`idl-build` feature is missing. To solve, add

[features]
idl-build = ["anchor-lang/idl-build"{anchor_spl_idl_build}]

in `{path}`."#
        ));
    }

    anchor_lang_idl::build::build_idl(
        std::env::current_dir()?,
        cfg.features.resolution,
        cfg.features.skip_lint || skip_lint,
        no_docs,
    )
}

fn idl_fetch(cfg_override: &ConfigOverride, address: Pubkey, out: Option<String>) -> Result<()> {
    let idl = fetch_idl(cfg_override, address)?;
    let out = match out {
        None => OutFile::Stdout,
        Some(out) => OutFile::File(PathBuf::from(out)),
    };
    write_idl(&idl, out)
}

fn idl_convert(path: String, out: Option<String>) -> Result<()> {
    let idl = fs::read(path)?;
    let idl = convert_idl(&idl)?;
    let out = match out {
        None => OutFile::Stdout,
        Some(out) => OutFile::File(PathBuf::from(out)),
    };
    write_idl(&idl, out)
}

fn idl_type(path: String, out: Option<String>) -> Result<()> {
    let idl = fs::read(path)?;
    let idl = convert_idl(&idl)?;
    let types = idl_ts(&idl)?;
    match out {
        Some(out) => fs::write(out, types)?,
        _ => println!("{types}"),
    };
    Ok(())
}

fn idl_ts(idl: &Idl) -> Result<String> {
    let idl_name = &idl.metadata.name;
    let type_name = idl_name.to_pascal_case();
    let idl = serde_json::to_string(idl)?;

    // Convert every field of the IDL to camelCase
    let camel_idl = Regex::new(r#""\w+":"([\w\d]+)""#)?
        .captures_iter(&idl)
        .fold(idl.clone(), |acc, cur| {
            let name = cur.get(1).unwrap().as_str();

            // Do not modify pubkeys
            if Pubkey::from_str(name).is_ok() {
                return acc;
            }

            let camel_name = name.to_lower_camel_case();
            acc.replace(&format!(r#""{name}""#), &format!(r#""{camel_name}""#))
        });

    // Pretty format
    let camel_idl = serde_json::to_string_pretty(&serde_json::from_str::<Idl>(&camel_idl)?)?;

    Ok(format!(
        r#"/**
 * Program IDL in camelCase format in order to be used in JS/TS.
 *
 * Note that this is only a type helper and is not the actual IDL. The original
 * IDL can be found at `target/idl/{idl_name}.json`.
 */
export type {type_name} = {camel_idl};
"#
    ))
}

fn write_idl(idl: &Idl, out: OutFile) -> Result<()> {
    let idl_json = serde_json::to_string_pretty(idl)?;
    match out {
        OutFile::Stdout => println!("{idl_json}"),
        OutFile::File(out) => fs::write(out, idl_json)?,
    };

    Ok(())
}

/// Print `base64+borsh` encoded IDL instruction.
fn print_idl_instruction(ix_name: &str, ix: &Instruction, idl_address: &Pubkey) -> Result<()> {
    use base64::engine::general_purpose::STANDARD;
    use base64::Engine;

    println!("Print only mode. No execution!");
    println!("Instruction: {ix_name}");
    println!("IDL address: {idl_address}");
    println!("Program: {}", ix.program_id);

    // Serialize with `bincode` because `Instruction` does not implement `BorshSerialize`
    let mut serialized_ix = bincode::serialize(ix)?;

    // Remove extra bytes in order to make the serialized instruction `borsh` compatible
    // `bincode` uses 8 bytes(LE) for length meanwhile `borsh` uses 4 bytes(LE)
    let mut remove_extra_vec_bytes = |index: usize| {
        serialized_ix.drain((index + 4)..(index + 8));
    };

    let accounts_index = std::mem::size_of_val(&ix.program_id);
    remove_extra_vec_bytes(accounts_index);
    let data_index = accounts_index + 4 + std::mem::size_of_val(&*ix.accounts);
    remove_extra_vec_bytes(data_index);

    println!(
        "Base64 encoded instruction: {}",
        STANDARD.encode(serialized_ix)
    );

    Ok(())
}

fn account(
    cfg_override: &ConfigOverride,
    account_type: String,
    address: Pubkey,
    idl_filepath: Option<String>,
) -> Result<()> {
    let (program_name, account_type_name) = account_type
        .split_once('.') // Split at first occurrence of dot
        .and_then(|(x, y)| y.find('.').map_or_else(|| Some((x, y)), |_| None)) // ensures no dots in second substring
        .ok_or_else(|| {
            anyhow!(
                "Please enter the account struct in the following format: <program_name>.<Account>",
            )
        })?;

    let idl = idl_filepath.map_or_else(
        || {
            Config::discover(cfg_override)
                .expect("Error when detecting workspace.")
                .expect("Not in workspace.")
                .read_all_programs()
                .expect("Workspace must contain atleast one program.")
                .iter()
                .find(|&p| p.lib_name == *program_name)
                .unwrap_or_else(|| panic!("Program {program_name} not found in workspace."))
                .idl
                .as_ref()
                .expect("IDL not found. Please build the program atleast once to generate the IDL.")
                .clone()
        },
        |idl_path| {
            let bytes = fs::read(idl_path).expect("Unable to read IDL.");
            let idl: Idl = serde_json::from_reader(&*bytes).expect("Invalid IDL format.");

            if idl.metadata.name != program_name {
                panic!("IDL does not match program {program_name}.");
            }

            idl
        },
    );

    let cluster = match &cfg_override.cluster {
        Some(cluster) => cluster.clone(),
        None => Config::discover(cfg_override)?
            .map(|cfg| cfg.provider.cluster.clone())
            .unwrap_or(Cluster::Localnet),
    };

    let data = create_client(cluster.url()).get_account_data(&address)?;
    if data.len() < 8 {
        return Err(anyhow!(
            "The account has less than 8 bytes and is not an Anchor account."
        ));
    }
    let mut data_view = &data[8..];

    let deserialized_json =
        deserialize_idl_defined_type_to_json(&idl, account_type_name, &mut data_view)?;

    println!(
        "{}",
        serde_json::to_string_pretty(&deserialized_json).unwrap()
    );

    Ok(())
}

// Deserializes user defined IDL types by munching the account data(recursively).
fn deserialize_idl_defined_type_to_json(
    idl: &Idl,
    defined_type_name: &str,
    data: &mut &[u8],
) -> Result<JsonValue, anyhow::Error> {
    let defined_type = &idl
        .accounts
        .iter()
        .find(|acc| acc.name == defined_type_name)
        .and_then(|acc| idl.types.iter().find(|ty| ty.name == acc.name))
        .or_else(|| idl.types.iter().find(|ty| ty.name == defined_type_name))
        .ok_or_else(|| anyhow!("Type `{}` not found in IDL.", defined_type_name))?
        .ty;

    let mut deserialized_fields = Map::new();

    match defined_type {
        IdlTypeDefTy::Struct { fields } => {
            if let Some(fields) = fields {
                match fields {
                    IdlDefinedFields::Named(fields) => {
                        for field in fields {
                            deserialized_fields.insert(
                                field.name.clone(),
                                deserialize_idl_type_to_json(&field.ty, data, idl)?,
                            );
                        }
                    }
                    IdlDefinedFields::Tuple(fields) => {
                        let mut values = Vec::new();
                        for field in fields {
                            values.push(deserialize_idl_type_to_json(field, data, idl)?);
                        }
                        deserialized_fields
                            .insert(defined_type_name.to_owned(), JsonValue::Array(values));
                    }
                }
            }
        }
        IdlTypeDefTy::Enum { variants } => {
            let repr = <u8 as AnchorDeserialize>::deserialize(data)?;

            let variant = variants
                .get(repr as usize)
                .unwrap_or_else(|| panic!("Error while deserializing enum variant {repr}"));

            let mut value = json!({});

            if let Some(enum_field) = &variant.fields {
                match enum_field {
                    IdlDefinedFields::Named(fields) => {
                        let mut values = Map::new();
                        for field in fields {
                            values.insert(
                                field.name.clone(),
                                deserialize_idl_type_to_json(&field.ty, data, idl)?,
                            );
                        }
                        value = JsonValue::Object(values);
                    }
                    IdlDefinedFields::Tuple(fields) => {
                        let mut values = Vec::new();
                        for field in fields {
                            values.push(deserialize_idl_type_to_json(field, data, idl)?);
                        }
                        value = JsonValue::Array(values);
                    }
                }
            }

            deserialized_fields.insert(variant.name.clone(), value);
        }
        IdlTypeDefTy::Type { alias } => {
            return deserialize_idl_type_to_json(alias, data, idl);
        }
    }

    Ok(JsonValue::Object(deserialized_fields))
}

// Deserializes a primitive type using AnchorDeserialize
fn deserialize_idl_type_to_json(
    idl_type: &IdlType,
    data: &mut &[u8],
    parent_idl: &Idl,
) -> Result<JsonValue, anyhow::Error> {
    if data.is_empty() {
        return Err(anyhow::anyhow!("Unable to parse from empty bytes"));
    }

    Ok(match idl_type {
        IdlType::Bool => json!(<bool as AnchorDeserialize>::deserialize(data)?),
        IdlType::U8 => {
            json!(<u8 as AnchorDeserialize>::deserialize(data)?)
        }
        IdlType::I8 => {
            json!(<i8 as AnchorDeserialize>::deserialize(data)?)
        }
        IdlType::U16 => {
            json!(<u16 as AnchorDeserialize>::deserialize(data)?)
        }
        IdlType::I16 => {
            json!(<i16 as AnchorDeserialize>::deserialize(data)?)
        }
        IdlType::U32 => {
            json!(<u32 as AnchorDeserialize>::deserialize(data)?)
        }
        IdlType::I32 => {
            json!(<i32 as AnchorDeserialize>::deserialize(data)?)
        }
        IdlType::F32 => json!(<f32 as AnchorDeserialize>::deserialize(data)?),
        IdlType::U64 => {
            json!(<u64 as AnchorDeserialize>::deserialize(data)?)
        }
        IdlType::I64 => {
            json!(<i64 as AnchorDeserialize>::deserialize(data)?)
        }
        IdlType::F64 => json!(<f64 as AnchorDeserialize>::deserialize(data)?),
        IdlType::U128 => {
            // TODO: Remove to_string once serde_json supports u128 deserialization
            json!(<u128 as AnchorDeserialize>::deserialize(data)?.to_string())
        }
        IdlType::I128 => {
            // TODO: Remove to_string once serde_json supports i128 deserialization
            json!(<i128 as AnchorDeserialize>::deserialize(data)?.to_string())
        }
        IdlType::U256 => todo!("Upon completion of u256 IDL standard"),
        IdlType::I256 => todo!("Upon completion of i256 IDL standard"),
        IdlType::Bytes => JsonValue::Array(
            <Vec<u8> as AnchorDeserialize>::deserialize(data)?
                .iter()
                .map(|i| json!(*i))
                .collect(),
        ),
        IdlType::String => json!(<String as AnchorDeserialize>::deserialize(data)?),
        IdlType::Pubkey => {
            json!(<Pubkey as AnchorDeserialize>::deserialize(data)?.to_string())
        }
        IdlType::Array(ty, size) => match size {
            IdlArrayLen::Value(size) => {
                let mut array_data: Vec<JsonValue> = Vec::with_capacity(*size);

                for _ in 0..*size {
                    array_data.push(deserialize_idl_type_to_json(ty, data, parent_idl)?);
                }

                JsonValue::Array(array_data)
            }
            // TODO:
            IdlArrayLen::Generic(_) => unimplemented!("Generic array length is not yet supported"),
        },
        IdlType::Option(ty) => {
            let is_present = <u8 as AnchorDeserialize>::deserialize(data)?;

            if is_present == 0 {
                JsonValue::String("None".to_string())
            } else {
                deserialize_idl_type_to_json(ty, data, parent_idl)?
            }
        }
        IdlType::Vec(ty) => {
            let size: usize = <u32 as AnchorDeserialize>::deserialize(data)?
                .try_into()
                .unwrap();

            let mut vec_data: Vec<JsonValue> = Vec::with_capacity(size);

            for _ in 0..size {
                vec_data.push(deserialize_idl_type_to_json(ty, data, parent_idl)?);
            }

            JsonValue::Array(vec_data)
        }
        IdlType::Defined {
            name,
            generics: _generics,
        } => {
            // TODO: Generics
            deserialize_idl_defined_type_to_json(parent_idl, name, data)?
        }
        IdlType::Generic(generic) => json!(generic),
        _ => unimplemented!("{idl_type:?}"),
    })
}

enum OutFile {
    Stdout,
    File(PathBuf),
}

// Builds, deploys, and tests all workspace programs in a single command.
#[allow(clippy::too_many_arguments)]
fn test(
    cfg_override: &ConfigOverride,
    program_name: Option<String>,
    skip_deploy: bool,
    skip_local_validator: bool,
    skip_build: bool,
    skip_lint: bool,
    detach: bool,
    tests_to_run: Vec<String>,
    extra_args: Vec<String>,
    env_vars: Vec<String>,
    cargo_args: Vec<String>,
    arch: ProgramArch,
) -> Result<()> {
    let test_paths = tests_to_run
        .iter()
        .map(|path| {
            PathBuf::from(path)
                .canonicalize()
                .map_err(|_| anyhow!("Wrong path {}", path))
        })
        .collect::<Result<Vec<_>, _>>()?;

    with_workspace(cfg_override, |cfg| {
        // Build if needed.
        if !skip_build {
            build(
                cfg_override,
                false,
                None,
                None,
                false,
                skip_lint,
                program_name.clone(),
                None,
                None,
                BootstrapMode::None,
                BuildSbfOptions::default(),
                None,
                None,
                env_vars,
                cargo_args,
                false,
                arch,
            )?;
        }

        let root = cfg.path().parent().unwrap().to_owned();
        cfg.add_test_config(root, test_paths)?;

        // Run the deploy against the cluster in two cases:
        //
        // 1. The cluster is not localnet.
        // 2. The cluster is localnet, but we're not booting a local validator.
        //
        // In either case, skip the deploy if the user specifies.
        let is_localnet = cfg.provider.cluster == Cluster::Localnet;
        if (!is_localnet || skip_local_validator) && !skip_deploy {
            deploy(cfg_override, None, None, false, vec![])?;
        }
        let mut is_first_suite = true;
        if let Some(test_script) = cfg.scripts.get_mut("test") {
            is_first_suite = false;

            match program_name {
                Some(program_name) => {
                    if let Some((from, to)) = Regex::new("\\s(tests/\\S+\\.(js|ts))")
                        .unwrap()
                        .captures_iter(&test_script.clone())
                        .last()
                        .and_then(|c| c.get(1).and_then(|mtch| c.get(2).map(|ext| (mtch, ext))))
                        .map(|(mtch, ext)| {
                            (
                                mtch.as_str(),
                                format!("tests/{program_name}.{}", ext.as_str()),
                            )
                        })
                    {
                        println!("\nRunning tests of program `{program_name}`!");
                        // Replace the last path to the program name's path
                        *test_script = test_script.replace(from, &to);
                    }
                }
                _ => println!(
                    "\nFound a 'test' script in the Anchor.toml. Running it as a test suite!"
                ),
            }

            run_test_suite(
                cfg,
                cfg.path(),
                is_localnet,
                skip_local_validator,
                skip_deploy,
                detach,
                &cfg.test_validator,
                &cfg.scripts,
                &extra_args,
            )?;
        }
        if let Some(test_config) = &cfg.test_config {
            for test_suite in test_config.iter() {
                if !is_first_suite {
                    std::thread::sleep(std::time::Duration::from_millis(
                        test_suite
                            .1
                            .test
                            .as_ref()
                            .map(|val| val.shutdown_wait)
                            .unwrap_or(SHUTDOWN_WAIT) as u64,
                    ));
                } else {
                    is_first_suite = false;
                }

                run_test_suite(
                    cfg,
                    test_suite.0,
                    is_localnet,
                    skip_local_validator,
                    skip_deploy,
                    detach,
                    &test_suite.1.test,
                    &test_suite.1.scripts,
                    &extra_args,
                )?;
            }
        }
        Ok(())
    })
}

#[allow(clippy::too_many_arguments)]
fn run_test_suite(
    cfg: &WithPath<Config>,
    test_suite_path: impl AsRef<Path>,
    is_localnet: bool,
    skip_local_validator: bool,
    skip_deploy: bool,
    detach: bool,
    test_validator: &Option<TestValidator>,
    scripts: &ScriptsConfig,
    extra_args: &[String],
) -> Result<()> {
    println!("\nRunning test suite: {:#?}\n", test_suite_path.as_ref());
    // Start local test validator, if needed.
    let mut validator_handle = None;
    if is_localnet && (!skip_local_validator) {
        let flags = match skip_deploy {
            true => None,
            false => Some(validator_flags(cfg, test_validator)?),
        };
        validator_handle = Some(start_test_validator(cfg, test_validator, flags, true)?);
    }

    let url = cluster_url(cfg, test_validator);

    let node_options = format!(
        "{} {}",
        match std::env::var_os("NODE_OPTIONS") {
            Some(value) => value
                .into_string()
                .map_err(std::env::VarError::NotUnicode)?,
            None => "".to_owned(),
        },
        get_node_dns_option()?,
    );

    // Setup log reader.
    let log_streams = stream_logs(cfg, &url);

    // Run the tests.
    let test_result = {
        let cmd = scripts
            .get("test")
            .expect("Not able to find script for `test`")
            .clone();
        let script_args = format!("{cmd} {}", extra_args.join(" "));
        std::process::Command::new("bash")
            .arg("-c")
            .arg(script_args)
            .env("ANCHOR_PROVIDER_URL", url)
            .env("ANCHOR_WALLET", cfg.provider.wallet.to_string())
            .env("NODE_OPTIONS", node_options)
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit())
            .output()
            .map_err(anyhow::Error::from)
            .context(cmd)
    };

    // Keep validator running if needed.
    if test_result.is_ok() && detach {
        println!("Local validator still running. Press Ctrl + C quit.");
        std::io::stdin().lock().lines().next().unwrap().unwrap();
    }

    // Check all errors and shut down.
    if let Some(mut child) = validator_handle {
        if let Err(err) = child.kill() {
            println!("Failed to kill subprocess {}: {}", child.id(), err);
        }
    }
    for mut child in log_streams? {
        if let Err(err) = child.kill() {
            println!("Failed to kill subprocess {}: {}", child.id(), err);
        }
    }

    // Must exist *after* shutting down the validator and log streams.
    match test_result {
        Ok(exit) => {
            if !exit.status.success() {
                std::process::exit(exit.status.code().unwrap());
            }
        }
        Err(err) => {
            println!("Failed to run test: {err:#}");
            return Err(err);
        }
    }

    Ok(())
}

// Returns the solana-test-validator flags. This will embed the workspace
// programs in the genesis block so we don't have to deploy every time. It also
// allows control of other solana-test-validator features.
fn validator_flags(
    cfg: &WithPath<Config>,
    test_validator: &Option<TestValidator>,
) -> Result<Vec<String>> {
    let programs = cfg.programs.get(&Cluster::Localnet);

    let test_upgradeable_program = test_validator
        .as_ref()
        .map(|test_validator| test_validator.upgradeable)
        .unwrap_or(false);

    let mut flags = Vec::new();
    for mut program in cfg.read_all_programs()? {
        let verifiable = false;
        let binary_path = program.binary_path(verifiable).display().to_string();

        // Use the [programs.cluster] override and fallback to the keypair
        // files if no override is given.
        let address = programs
            .and_then(|m| m.get(&program.lib_name))
            .map(|deployment| Ok(deployment.address.to_string()))
            .unwrap_or_else(|| program.pubkey().map(|p| p.to_string()))?;

        if test_upgradeable_program {
            flags.push("--upgradeable-program".to_string());
            flags.push(address.clone());
            flags.push(binary_path);
            flags.push(cfg.wallet_kp()?.pubkey().to_string());
        } else {
            flags.push("--bpf-program".to_string());
            flags.push(address.clone());
            flags.push(binary_path);
        }

        if let Some(idl) = program.idl.as_mut() {
            // Add program address to the IDL.
            idl.address = address;

            // Persist it.
            let idl_out = PathBuf::from("target/idl")
                .join(&idl.metadata.name)
                .with_extension("json");
            write_idl(idl, OutFile::File(idl_out))?;
        }
    }

    if let Some(test) = test_validator.as_ref() {
        if let Some(genesis) = &test.genesis {
            for entry in genesis {
                let program_path = Path::new(&entry.program);
                if !program_path.exists() {
                    return Err(anyhow!(
                        "Program in genesis configuration does not exist at path: {}",
                        program_path.display()
                    ));
                }
                if entry.upgradeable.unwrap_or(false) {
                    flags.push("--upgradeable-program".to_string());
                    flags.push(entry.address.clone());
                    flags.push(entry.program.clone());
                    flags.push(cfg.wallet_kp()?.pubkey().to_string());
                } else {
                    flags.push("--bpf-program".to_string());
                    flags.push(entry.address.clone());
                    flags.push(entry.program.clone());
                }
            }
        }
        if let Some(validator) = &test.validator {
            let entries = serde_json::to_value(validator)?;
            for (key, value) in entries.as_object().unwrap() {
                if key == "ledger" {
                    // Ledger flag is a special case as it is passed separately to the rest of
                    // these validator flags.
                    continue;
                };
                if key == "account" {
                    for entry in value.as_array().unwrap() {
                        // Push the account flag for each array entry
                        flags.push("--account".to_string());
                        flags.push(entry["address"].as_str().unwrap().to_string());
                        flags.push(entry["filename"].as_str().unwrap().to_string());
                    }
                } else if key == "account_dir" {
                    for entry in value.as_array().unwrap() {
                        flags.push("--account-dir".to_string());
                        flags.push(entry["directory"].as_str().unwrap().to_string());
                    }
                } else if key == "clone" {
                    // Client for fetching accounts data
                    let client = if let Some(url) = entries["url"].as_str() {
                        create_client(url)
                    } else {
                        return Err(anyhow!(
                            "Validator url for Solana's JSON RPC should be provided in order to clone accounts from it"
                        ));
                    };

                    let pubkeys = value
                        .as_array()
                        .unwrap()
                        .iter()
                        .map(|entry| {
                            let address = entry["address"].as_str().unwrap();
                            Pubkey::from_str(address)
                                .map_err(|_| anyhow!("Invalid pubkey {}", address))
                        })
                        .collect::<Result<HashSet<Pubkey>>>()?
                        .into_iter()
                        .collect::<Vec<_>>();
                    let accounts = client.get_multiple_accounts(&pubkeys)?;

                    for (pubkey, account) in pubkeys.into_iter().zip(accounts) {
                        match account {
                            Some(account) => {
                                // Use a different flag for program accounts to fix the problem
                                // described in https://github.com/anza-xyz/agave/issues/522
                                if account.owner == bpf_loader_upgradeable::id()
                                // Only programs are supported with `--clone-upgradeable-program`
                                    && matches!(
                                        account.deserialize_data::<UpgradeableLoaderState>()?,
                                        UpgradeableLoaderState::Program { .. }
                                    )
                                {
                                    flags.push("--clone-upgradeable-program".to_string());
                                    flags.push(pubkey.to_string());
                                } else {
                                    flags.push("--clone".to_string());
                                    flags.push(pubkey.to_string());
                                }
                            }
                            _ => return Err(anyhow!("Account {} not found", pubkey)),
                        }
                    }
                } else if key == "deactivate_feature" {
                    // Verify that the feature flags are valid pubkeys
                    let pubkeys_result: Result<Vec<Pubkey>, _> = value
                        .as_array()
                        .unwrap()
                        .iter()
                        .map(|entry| {
                            let feature_flag = entry.as_str().unwrap();
                            Pubkey::from_str(feature_flag).map_err(|_| {
                                anyhow!("Invalid pubkey (feature flag) {}", feature_flag)
                            })
                        })
                        .collect();
                    let features = pubkeys_result?;
                    for feature in features {
                        flags.push("--deactivate-feature".to_string());
                        flags.push(feature.to_string());
                    }
                } else {
                    // Remaining validator flags are non-array types
                    flags.push(format!("--{}", key.replace('_', "-")));
                    if let serde_json::Value::String(v) = value {
                        flags.push(v.to_string());
                    } else {
                        flags.push(value.to_string());
                    }
                }
            }
        }
    }

    Ok(flags)
}

fn stream_logs(config: &WithPath<Config>, rpc_url: &str) -> Result<Vec<std::process::Child>> {
    let program_logs_dir = ".anchor/program-logs";
    if Path::new(program_logs_dir).exists() {
        fs::remove_dir_all(program_logs_dir)?;
    }
    fs::create_dir_all(program_logs_dir)?;
    let mut handles = vec![];
    for program in config.read_all_programs()? {
        let mut file = File::open(format!("target/idl/{}.json", program.lib_name))?;
        let mut contents = vec![];
        file.read_to_end(&mut contents)?;
        let idl: Idl = serde_json::from_slice(&contents)?;

        let log_file = File::create(format!(
            "{}/{}.{}.log",
            program_logs_dir, idl.address, program.lib_name,
        ))?;
        let stdio = std::process::Stdio::from(log_file);
        let child = std::process::Command::new("solana")
            .arg("logs")
            .arg(idl.address)
            .arg("--url")
            .arg(rpc_url)
            .stdout(stdio)
            .spawn()?;
        handles.push(child);
    }
    if let Some(test) = config.test_validator.as_ref() {
        if let Some(genesis) = &test.genesis {
            for entry in genesis {
                let log_file = File::create(format!("{}/{}.log", program_logs_dir, entry.address))?;
                let stdio = std::process::Stdio::from(log_file);
                let child = std::process::Command::new("solana")
                    .arg("logs")
                    .arg(entry.address.clone())
                    .arg("--url")
                    .arg(rpc_url)
                    .stdout(stdio)
                    .spawn()?;
                handles.push(child);
            }
        }
    }

    Ok(handles)
}

// A validator can answer `getLatestBlockhash` before its forwarding stage is
// ready to accept transactions. Starting test scripts at that point can drop
// their first transaction, particularly with Solana 2.x validators.
const MINIMUM_TEST_VALIDATOR_BLOCK_HEIGHT: u64 = 25;

fn start_test_validator(
    cfg: &Config,
    test_validator: &Option<TestValidator>,
    flags: Option<Vec<String>>,
    test_log_stdout: bool,
) -> Result<Child> {
    let (test_ledger_directory, test_ledger_log_filename) =
        test_validator_file_paths(test_validator)?;

    // Start a validator for testing.
    let (test_validator_stdout, test_validator_stderr) = match test_log_stdout {
        true => {
            let test_validator_stdout_file = File::create(&test_ledger_log_filename)?;
            let test_validator_sterr_file = test_validator_stdout_file.try_clone()?;
            (
                Stdio::from(test_validator_stdout_file),
                Stdio::from(test_validator_sterr_file),
            )
        }
        false => (Stdio::inherit(), Stdio::inherit()),
    };

    let rpc_url = test_validator_rpc_url(test_validator);

    let rpc_port = cfg
        .test_validator
        .as_ref()
        .and_then(|test| test.validator.as_ref().map(|v| v.rpc_port))
        .unwrap_or(solana_sdk::rpc_port::DEFAULT_RPC_PORT);
    if !portpicker::is_free(rpc_port) {
        return Err(anyhow!(
            "Your configured rpc port: {rpc_port} is already in use"
        ));
    }
    let faucet_port = cfg
        .test_validator
        .as_ref()
        .and_then(|test| test.validator.as_ref().and_then(|v| v.faucet_port))
        .unwrap_or(solana_faucet::faucet::FAUCET_PORT);
    if !portpicker::is_free(faucet_port) {
        return Err(anyhow!(
            "Your configured faucet port: {faucet_port} is already in use"
        ));
    }

    let mut validator_handle = std::process::Command::new("solana-test-validator")
        .arg("--ledger")
        .arg(test_ledger_directory)
        .arg("--mint")
        .arg(cfg.wallet_kp()?.pubkey().to_string())
        .args(flags.unwrap_or_default())
        .stdout(test_validator_stdout)
        .stderr(test_validator_stderr)
        .spawn()
        .map_err(|e| anyhow!("Failed to spawn `solana-test-validator`: {e}"))?;

    // Wait for the validator to be ready.
    let client = create_client(rpc_url);
    let mut count = 0;
    let ms_wait = test_validator
        .as_ref()
        .map(|test| test.startup_wait)
        .unwrap_or(STARTUP_WAIT);
    while count < ms_wait {
        let latest_blockhash = client.get_latest_blockhash();
        let block_height = client.get_block_height();
        if latest_blockhash.is_ok()
            && matches!(block_height, Ok(height) if height >= MINIMUM_TEST_VALIDATOR_BLOCK_HEIGHT)
        {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(100));
        count += 100;
    }
    if count >= ms_wait {
        eprintln!(
            "Unable to get latest blockhash. Test validator does not look started. \
            Check {test_ledger_log_filename:?} for errors. Consider increasing [test.startup_wait] in Anchor.toml."
        );
        validator_handle.kill()?;
        std::process::exit(1);
    }
    Ok(validator_handle)
}

// Return the URL that solana-test-validator should be running on given the
// configuration
fn test_validator_rpc_url(test_validator: &Option<TestValidator>) -> String {
    match test_validator {
        Some(TestValidator {
            validator: Some(validator),
            ..
        }) => format!("http://{}:{}", validator.bind_address, validator.rpc_port),
        _ => "http://127.0.0.1:8899".to_string(),
    }
}

// Setup and return paths to the solana-test-validator ledger directory and log
// files given the configuration
fn test_validator_file_paths(test_validator: &Option<TestValidator>) -> Result<(PathBuf, PathBuf)> {
    let ledger_path = match test_validator {
        Some(TestValidator {
            validator: Some(validator),
            ..
        }) => &validator.ledger,
        _ => DEFAULT_LEDGER_PATH,
    };
    let ledger_path = Path::new(ledger_path);

    if !ledger_path.is_relative() {
        // Prevent absolute paths to avoid someone using / or similar, as the
        // directory gets removed
        eprintln!("Ledger directory {ledger_path:?} must be relative");
        std::process::exit(1);
    }
    if ledger_path.exists() {
        fs::remove_dir_all(ledger_path)?;
    }

    fs::create_dir_all(ledger_path)?;

    Ok((
        ledger_path.to_owned(),
        ledger_path.join("test-ledger-log.txt"),
    ))
}

fn cluster_url(cfg: &Config, test_validator: &Option<TestValidator>) -> String {
    let is_localnet = cfg.provider.cluster == Cluster::Localnet;
    match is_localnet {
        // Cluster is Localnet, assume the intent is to use the configuration
        // for solana-test-validator
        true => test_validator_rpc_url(test_validator),
        false => cfg.provider.cluster.url().to_string(),
    }
}

fn clean(cfg_override: &ConfigOverride) -> Result<()> {
    let cfg = Config::discover(cfg_override)?.expect("Not in workspace.");
    let cfg_parent = cfg.path().parent().expect("Invalid Anchor.toml");
    let target_dir = cfg_parent.join("target");
    let deploy_dir = target_dir.join("deploy");

    if target_dir.exists() {
        for entry in fs::read_dir(target_dir)? {
            let path = entry?.path();
            if path.is_dir() && path != deploy_dir {
                fs::remove_dir_all(&path)
                    .map_err(|e| anyhow!("Could not remove directory {}: {}", path.display(), e))?;
            } else if path.is_file() {
                fs::remove_file(&path)
                    .map_err(|e| anyhow!("Could not remove file {}: {}", path.display(), e))?;
            }
        }
    } else {
        println!("skipping target directory: not found")
    }

    if deploy_dir.exists() {
        for file in fs::read_dir(deploy_dir)? {
            let path = file?.path();
            if path.extension() != Some(&OsString::from("json")) {
                fs::remove_file(&path)
                    .map_err(|e| anyhow!("Could not remove file {}: {}", path.display(), e))?;
            }
        }
    } else {
        println!("skipping deploy directory: not found")
    }

    Ok(())
}

fn deploy(
    cfg_override: &ConfigOverride,
    program_name: Option<String>,
    program_keypair: Option<String>,
    verifiable: bool,
    solana_args: Vec<String>,
) -> Result<()> {
    // Execute the code within the workspace
    with_workspace(cfg_override, |cfg| {
        let url = cluster_url(cfg, &cfg.test_validator);
        let keypair = cfg.provider.wallet.to_string();

        // Deploy the programs.
        println!("Deploying cluster: {}", url);
        println!("Upgrade authority: {}", keypair);

        for mut program in cfg.get_programs(program_name)? {
            let binary_path = program.binary_path(verifiable).display().to_string();

            println!("Deploying program {:?}...", program.lib_name);
            println!("Program path: {}...", binary_path);

            let (program_keypair_filepath, program_id) = match &program_keypair {
                Some(path) => (path.clone(), get_keypair(path)?.pubkey()),
                None => (
                    program.keypair_file()?.path().display().to_string(),
                    program.pubkey()?,
                ),
            };

            // Send deploy transactions using the Solana CLI
            let exit = std::process::Command::new("solana")
                .arg("program")
                .arg("deploy")
                .arg("--url")
                .arg(&url)
                .arg("--keypair")
                .arg(&keypair)
                .arg("--program-id")
                .arg(strip_workspace_prefix(program_keypair_filepath))
                .arg(strip_workspace_prefix(binary_path))
                .args(&solana_args)
                .stdout(Stdio::inherit())
                .stderr(Stdio::inherit())
                .output()
                .expect("Must deploy");

            // Check if deployment was successful
            if !exit.status.success() {
                println!("There was a problem deploying: {exit:?}.");
                std::process::exit(exit.status.code().unwrap_or(1));
            }

            if let Some(idl) = program.idl.as_mut() {
                // Add program address to the IDL.
                idl.address = program_id.to_string();

                // Persist it.
                let idl_out = PathBuf::from("target/idl")
                    .join(&idl.metadata.name)
                    .with_extension("json");
                write_idl(idl, OutFile::File(idl_out))?;
            }
        }

        println!("Deploy success");

        Ok(())
    })
}

fn upgrade(
    cfg_override: &ConfigOverride,
    program_id: Pubkey,
    program_filepath: String,
    solana_args: Vec<String>,
) -> Result<()> {
    let path: PathBuf = program_filepath.parse().unwrap();
    let program_filepath = path.canonicalize()?.display().to_string();

    with_workspace(cfg_override, |cfg| {
        let url = cluster_url(cfg, &cfg.test_validator);
        let exit = std::process::Command::new("solana")
            .arg("program")
            .arg("deploy")
            .arg("--url")
            .arg(url)
            .arg("--keypair")
            .arg(&cfg.provider.wallet.to_string())
            .arg("--program-id")
            .arg(strip_workspace_prefix(program_id.to_string()))
            .arg(strip_workspace_prefix(program_filepath))
            .args(&solana_args)
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit())
            .output()
            .expect("Must deploy");
        if !exit.status.success() {
            println!("There was a problem deploying: {exit:?}.");
            std::process::exit(exit.status.code().unwrap_or(1));
        }
        Ok(())
    })
}

fn create_idl_account(
    cfg: &Config,
    keypair_path: &str,
    program_id: &Pubkey,
    idl: &Idl,
    priority_fee: Option<u64>,
) -> Result<Pubkey> {
    // Misc.
    let idl_address = IdlAccount::address(program_id);
    let keypair = get_keypair(keypair_path)?;
    let url = cluster_url(cfg, &cfg.test_validator);
    let client = create_client(url);
    let idl_data = serialize_idl(idl)?;

    // Run `Create instruction.
    {
        let pda_max_growth = 60_000;
        let idl_header_size = 44;
        let idl_data_len = idl_data.len() as u64;
        // We're only going to support up to 6 instructions in one transaction
        // because will anyone really have a >60kb IDL?
        if idl_data_len > pda_max_growth {
            return Err(anyhow!(
                "Your IDL is over 60kb and this isn't supported right now"
            ));
        }
        // Double for future growth.
        let data_len = (idl_data_len * 2).min(pda_max_growth - idl_header_size);

        let num_additional_instructions = data_len / 10000;
        let mut instructions = Vec::new();
        let data = serialize_idl_ix(anchor_lang::idl::IdlInstruction::Create { data_len })?;
        let program_signer = Pubkey::find_program_address(&[], program_id).0;
        let accounts = vec![
            AccountMeta::new_readonly(keypair.pubkey(), true),
            AccountMeta::new(idl_address, false),
            AccountMeta::new_readonly(program_signer, false),
            AccountMeta::new_readonly(solana_program::system_program::ID, false),
            AccountMeta::new_readonly(*program_id, false),
            AccountMeta::new_readonly(solana_program::sysvar::rent::ID, false),
        ];
        instructions.push(Instruction {
            program_id: *program_id,
            accounts,
            data,
        });

        for _ in 0..num_additional_instructions {
            let data = serialize_idl_ix(anchor_lang::idl::IdlInstruction::Resize { data_len })?;
            instructions.push(Instruction {
                program_id: *program_id,
                accounts: vec![
                    AccountMeta::new(idl_address, false),
                    AccountMeta::new_readonly(keypair.pubkey(), true),
                    AccountMeta::new_readonly(solana_program::system_program::ID, false),
                ],
                data,
            });
        }
        instructions = prepend_compute_unit_ix(instructions, &client, priority_fee)?;

        let mut latest_hash = client.get_latest_blockhash()?;
        for retries in 0..20 {
            if !client.is_blockhash_valid(&latest_hash, client.commitment())? {
                latest_hash = client.get_latest_blockhash()?;
            }

            let tx = Transaction::new_signed_with_payer(
                &instructions,
                Some(&keypair.pubkey()),
                &[&keypair],
                latest_hash,
            );

            match client.send_and_confirm_transaction_with_spinner(&tx) {
                Ok(_) => break,
                Err(err) => {
                    if retries == 19 {
                        return Err(anyhow!("Error creating IDL account: {}", err));
                    }
                    println!("Error creating IDL account: {}. Retrying...", err);
                }
            }
        }
    }

    // Write directly to the IDL account buffer.
    idl_write(
        cfg,
        program_id,
        idl,
        IdlAccount::address(program_id),
        priority_fee,
    )?;

    Ok(idl_address)
}

fn create_idl_buffer(
    cfg: &Config,
    keypair_path: &str,
    program_id: &Pubkey,
    idl: &Idl,
    priority_fee: Option<u64>,
) -> Result<Pubkey> {
    let keypair = get_keypair(keypair_path)?;
    let url = cluster_url(cfg, &cfg.test_validator);
    let client = create_client(url);

    let buffer = Keypair::new();

    // Creates the new buffer account with the system program.
    let create_account_ix = {
        let space = 8 + 32 + 4 + serialize_idl(idl)?.len();
        let lamports = client.get_minimum_balance_for_rent_exemption(space)?;
        solana_sdk::system_instruction::create_account(
            &keypair.pubkey(),
            &buffer.pubkey(),
            lamports,
            space as u64,
            program_id,
        )
    };

    // Program instruction to create the buffer.
    let create_buffer_ix = {
        let accounts = vec![
            AccountMeta::new(buffer.pubkey(), false),
            AccountMeta::new_readonly(keypair.pubkey(), true),
            AccountMeta::new_readonly(sysvar::rent::ID, false),
        ];
        let mut data = anchor_lang::idl::IDL_IX_TAG.to_le_bytes().to_vec();
        data.append(&mut IdlInstruction::CreateBuffer.try_to_vec()?);
        Instruction {
            program_id: *program_id,
            accounts,
            data,
        }
    };

    let instructions = prepend_compute_unit_ix(
        vec![create_account_ix, create_buffer_ix],
        &client,
        priority_fee,
    )?;

    let mut latest_hash = client.get_latest_blockhash()?;
    for retries in 0..20 {
        if !client.is_blockhash_valid(&latest_hash, client.commitment())? {
            latest_hash = client.get_latest_blockhash()?;
        }
        let tx = Transaction::new_signed_with_payer(
            &instructions,
            Some(&keypair.pubkey()),
            &[&keypair, &buffer],
            latest_hash,
        );
        match client.send_and_confirm_transaction_with_spinner(&tx) {
            Ok(_) => break,
            Err(err) => {
                if retries == 19 {
                    return Err(anyhow!("Error creating buffer account: {}", err));
                }
                println!("Error creating buffer account: {}. Retrying...", err);
            }
        }
    }

    Ok(buffer.pubkey())
}

// Serialize and compress the idl.
fn serialize_idl(idl: &Idl) -> Result<Vec<u8>> {
    let json_bytes = serde_json::to_vec(idl)?;
    let mut e = ZlibEncoder::new(Vec::new(), Compression::default());
    e.write_all(&json_bytes)?;
    e.finish().map_err(Into::into)
}

fn serialize_idl_ix(ix_inner: anchor_lang::idl::IdlInstruction) -> Result<Vec<u8>> {
    let mut data = Vec::with_capacity(256);
    data.extend_from_slice(&anchor_lang::idl::IDL_IX_TAG.to_le_bytes());
    ix_inner.serialize(&mut data)?;
    Ok(data)
}

fn migrate(cfg_override: &ConfigOverride) -> Result<()> {
    with_workspace(cfg_override, |cfg| {
        println!("Running migration deploy script");

        let url = cluster_url(cfg, &cfg.test_validator);
        let cur_dir = std::env::current_dir()?;
        let migrations_dir = cur_dir.join("migrations");
        let deploy_ts = Path::new("deploy.ts");

        let use_ts = Path::new("tsconfig.json").exists() && migrations_dir.join(deploy_ts).exists();

        if !Path::new(".anchor").exists() {
            fs::create_dir(".anchor")?;
        }
        std::env::set_current_dir(".anchor")?;

        let exit = if use_ts {
            let module_path = migrations_dir.join(deploy_ts);
            let deploy_script_host_str =
                rust_template::deploy_ts_script_host(&url, &module_path.display().to_string());
            fs::write(deploy_ts, deploy_script_host_str)?;

            std::process::Command::new("yarn")
                .args([
                    "run",
                    "ts-node",
                    &fs::canonicalize(deploy_ts)?.to_string_lossy(),
                ])
                .env("ANCHOR_WALLET", cfg.provider.wallet.to_string())
                .stdout(Stdio::inherit())
                .stderr(Stdio::inherit())
                .output()?
        } else {
            let deploy_js = deploy_ts.with_extension("js");
            let module_path = migrations_dir.join(&deploy_js);
            let deploy_script_host_str =
                rust_template::deploy_js_script_host(&url, &module_path.display().to_string());
            fs::write(&deploy_js, deploy_script_host_str)?;

            std::process::Command::new("node")
                .arg(&deploy_js)
                .env("ANCHOR_WALLET", cfg.provider.wallet.to_string())
                .stdout(Stdio::inherit())
                .stderr(Stdio::inherit())
                .output()?
        };

        if !exit.status.success() {
            eprintln!("Deploy failed.");
            std::process::exit(exit.status.code().unwrap());
        }

        println!("Deploy complete.");
        Ok(())
    })
}

fn set_workspace_dir_or_exit() {
    let d = match Config::discover(&ConfigOverride::default()) {
        Err(err) => {
            println!("Workspace configuration error: {err}");
            std::process::exit(1);
        }
        Ok(d) => d,
    };
    match d {
        None => {
            println!("Not in anchor workspace.");
            std::process::exit(1);
        }
        Some(cfg) => {
            match cfg.path().parent() {
                None => {
                    println!("Unable to make new program");
                }
                Some(parent) => {
                    if std::env::set_current_dir(parent).is_err() {
                        println!("Not in anchor workspace.");
                        std::process::exit(1);
                    }
                }
            };
        }
    }
}

#[cfg(feature = "dev")]
fn airdrop(cfg_override: &ConfigOverride) -> Result<()> {
    let url = cfg_override
        .cluster
        .as_ref()
        .unwrap_or_else(|| &Cluster::Devnet)
        .url();
    loop {
        let exit = std::process::Command::new("solana")
            .arg("airdrop")
            .arg("10")
            .arg("--url")
            .arg(&url)
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit())
            .output()
            .expect("Must airdrop");
        if !exit.status.success() {
            println!("There was a problem airdropping: {:?}.", exit);
            std::process::exit(exit.status.code().unwrap_or(1));
        }
        std::thread::sleep(std::time::Duration::from_millis(10000));
    }
}

fn cluster(_cmd: ClusterCommand) -> Result<()> {
    println!("Cluster Endpoints:\n");
    println!("* Mainnet - https://api.mainnet-beta.solana.com");
    println!("* Devnet  - https://api.devnet.solana.com");
    println!("* Testnet - https://api.testnet.solana.com");
    Ok(())
}

fn shell(cfg_override: &ConfigOverride) -> Result<()> {
    with_workspace(cfg_override, |cfg| {
        let programs = {
            // Create idl map from all workspace programs.
            let mut idls: HashMap<String, Idl> = cfg
                .read_all_programs()?
                .iter()
                .filter(|program| program.idl.is_some())
                .map(|program| {
                    (
                        program.idl.as_ref().unwrap().metadata.name.clone(),
                        program.idl.clone().unwrap(),
                    )
                })
                .collect();
            // Insert all manually specified idls into the idl map.
            if let Some(programs) = cfg.programs.get(&cfg.provider.cluster) {
                let _ = programs
                    .iter()
                    .map(|(name, pd)| {
                        if let Some(idl_fp) = &pd.idl {
                            let file_str =
                                fs::read_to_string(idl_fp).expect("Unable to read IDL file");
                            let idl = serde_json::from_str(&file_str).expect("Idl not readable");
                            idls.insert(name.clone(), idl);
                        }
                    })
                    .collect::<Vec<_>>();
            }

            // Finalize program list with all programs with IDLs.
            match cfg.programs.get(&cfg.provider.cluster) {
                None => Vec::new(),
                Some(programs) => programs
                    .iter()
                    .filter_map(|(name, program_deployment)| {
                        Some(ProgramWorkspace {
                            name: name.to_string(),
                            program_id: program_deployment.address,
                            idl: match idls.get(name) {
                                None => return None,
                                Some(idl) => idl.clone(),
                            },
                        })
                    })
                    .collect::<Vec<ProgramWorkspace>>(),
            }
        };
        let url = cluster_url(cfg, &cfg.test_validator);
        let js_code = rust_template::node_shell(&url, &cfg.provider.wallet.to_string(), programs)?;
        let mut child = std::process::Command::new("node")
            .args(["-e", &js_code, "-i", "--experimental-repl-await"])
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit())
            .spawn()
            .map_err(|e| anyhow::format_err!("{}", e.to_string()))?;

        if !child.wait()?.success() {
            println!("Error running node shell");
            return Ok(());
        }
        Ok(())
    })
}

fn run(cfg_override: &ConfigOverride, script: String, script_args: Vec<String>) -> Result<()> {
    with_workspace(cfg_override, |cfg| {
        let url = cluster_url(cfg, &cfg.test_validator);
        let script = cfg
            .scripts
            .get(&script)
            .ok_or_else(|| anyhow!("Unable to find script"))?;
        let script_with_args = format!("{script} {}", script_args.join(" "));
        let exit = std::process::Command::new("bash")
            .arg("-c")
            .arg(&script_with_args)
            .env("ANCHOR_PROVIDER_URL", url)
            .env("ANCHOR_WALLET", cfg.provider.wallet.to_string())
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit())
            .output()
            .unwrap();
        if !exit.status.success() {
            std::process::exit(exit.status.code().unwrap_or(1));
        }
        Ok(())
    })
}

fn login(_cfg_override: &ConfigOverride, token: String) -> Result<()> {
    let dir = shellexpand::tilde("~/.config/anchor");
    if !Path::new(&dir.to_string()).exists() {
        fs::create_dir(dir.to_string())?;
    }

    std::env::set_current_dir(dir.to_string())?;

    // Freely overwrite the entire file since it's not used for anything else.
    let mut file = File::create("credentials")?;
    file.write_all(rust_template::credentials(&token).as_bytes())?;
    Ok(())
}

fn publish(
    cfg_override: &ConfigOverride,
    program_name: String,
    env_vars: Vec<String>,
    cargo_args: Vec<String>,
    skip_build: bool,
    arch: ProgramArch,
) -> Result<()> {
    // Discover the various workspace configs.
    let cfg = Config::discover(cfg_override)?.expect("Not in workspace.");

    let program = cfg.get_program(&program_name)?;

    let program_cargo_lock = pathdiff::diff_paths(
        program.path.join("Cargo.lock"),
        cfg.path().parent().unwrap(),
    )
    .ok_or_else(|| anyhow!("Unable to diff Cargo.lock path"))?;
    let cargo_lock = Path::new("Cargo.lock");

    // There must be a Cargo.lock
    if !program_cargo_lock.exists() && !cargo_lock.exists() {
        return Err(anyhow!("Cargo.lock must exist for a verifiable build"));
    }

    println!("Publishing will make your code public. Are you sure? Enter (yes)/no:");

    let answer = std::io::stdin().lock().lines().next().unwrap().unwrap();
    if answer != "yes" {
        println!("Aborting");
        return Ok(());
    }

    let anchor_package = AnchorPackage::from(program_name.clone(), &cfg)?;
    let anchor_package_bytes = serde_json::to_vec(&anchor_package)?;

    // Set directory to top of the workspace.
    let workspace_dir = cfg.path().parent().unwrap();
    std::env::set_current_dir(workspace_dir)?;

    // Create the workspace tarball.
    let dot_anchor = workspace_dir.join(".anchor");
    fs::create_dir_all(&dot_anchor)?;
    let tarball_filename = dot_anchor.join(format!("{program_name}.tar.gz"));
    let tar_gz = File::create(&tarball_filename)?;
    let enc = GzEncoder::new(tar_gz, Compression::default());
    let mut tar = tar::Builder::new(enc);

    // Files that will always be included if they exist.
    println!("PACKING: Anchor.toml");
    tar.append_path("Anchor.toml")?;
    if cargo_lock.exists() {
        println!("PACKING: Cargo.lock");
        tar.append_path(cargo_lock)?;
    }
    if Path::new("Cargo.toml").exists() {
        println!("PACKING: Cargo.toml");
        tar.append_path("Cargo.toml")?;
    }
    if Path::new("LICENSE").exists() {
        println!("PACKING: LICENSE");
        tar.append_path("LICENSE")?;
    }
    if Path::new("README.md").exists() {
        println!("PACKING: README.md");
        tar.append_path("README.md")?;
    }
    if Path::new("idl.json").exists() {
        println!("PACKING: idl.json");
        tar.append_path("idl.json")?;
    }

    // All workspace programs.
    for path in cfg.get_rust_program_list()? {
        let mut dirs = walkdir::WalkDir::new(path)
            .into_iter()
            .filter_entry(|e| !is_hidden(e));

        // Skip the parent dir.
        let _ = dirs.next().unwrap()?;

        for entry in dirs {
            let e = entry.map_err(|e| anyhow!("{:?}", e))?;

            let e = pathdiff::diff_paths(e.path(), cfg.path().parent().unwrap())
                .ok_or_else(|| anyhow!("Unable to diff paths"))?;

            let path_str = e.display().to_string();

            // Skip target dir.
            if !path_str.contains("target/") && !path_str.contains("/target") {
                // Only add the file if it's not empty.
                let metadata = fs::File::open(&e)?.metadata()?;
                if metadata.len() > 0 {
                    println!("PACKING: {}", e.display());
                    if e.is_dir() {
                        tar.append_dir_all(&e, &e)?;
                    } else {
                        tar.append_path(&e)?;
                    }
                }
            }
        }
    }

    // Tar pack complete.
    tar.into_inner()?;

    // Create tmp directory for workspace.
    let ws_dir = dot_anchor.join("workspace");
    if Path::exists(&ws_dir) {
        fs::remove_dir_all(&ws_dir)?;
    }
    fs::create_dir_all(&ws_dir)?;

    // Unpack the archive into the new workspace directory.
    std::env::set_current_dir(&ws_dir)?;
    unpack_archive(&tarball_filename)?;

    // Build the program before sending it to the server.
    if !skip_build {
        build(
            cfg_override,
            false,
            None,
            None,
            true,
            false,
            Some(program_name),
            None,
            None,
            BootstrapMode::None,
            BuildSbfOptions::default(),
            None,
            None,
            env_vars,
            cargo_args,
            true,
            arch,
        )?;
    }

    // Upload the tarball to the server.
    let token = registry_api_token(cfg_override)?;
    let form = Form::new()
        .part("manifest", Part::bytes(anchor_package_bytes))
        .part("workspace", {
            let file = File::open(&tarball_filename)?;
            Part::reader(file)
        });
    let client = Client::new();
    let resp = client
        .post(format!("{}/api/v0/build", cfg.registry.url))
        .bearer_auth(token)
        .multipart(form)
        .send()?;

    if resp.status() == 200 {
        println!("Build triggered");
    } else {
        println!(
            "{:?}",
            resp.text().unwrap_or_else(|_| "Server error".to_string())
        );
    }

    Ok(())
}

// Unpacks the tarball into the current directory.
fn unpack_archive(tar_path: impl AsRef<Path>) -> Result<()> {
    let tar = GzDecoder::new(std::fs::File::open(tar_path)?);
    let mut archive = Archive::new(tar);
    archive.unpack(".")?;
    archive.into_inner();

    Ok(())
}

fn registry_api_token(_cfg_override: &ConfigOverride) -> Result<String> {
    #[derive(Debug, Deserialize)]
    struct Registry {
        token: String,
    }
    #[derive(Debug, Deserialize)]
    struct Credentials {
        registry: Registry,
    }
    let filename = shellexpand::tilde("~/.config/anchor/credentials");
    let mut file = File::open(filename.to_string())?;
    let mut contents = String::new();
    file.read_to_string(&mut contents)?;

    let credentials_toml: Credentials = toml::from_str(&contents)?;

    Ok(credentials_toml.registry.token)
}

fn keys(cfg_override: &ConfigOverride, cmd: KeysCommand) -> Result<()> {
    match cmd {
        KeysCommand::List => keys_list(cfg_override),
        KeysCommand::Sync { program_name } => keys_sync(cfg_override, program_name),
    }
}

fn keys_list(cfg_override: &ConfigOverride) -> Result<()> {
    with_workspace(cfg_override, |cfg| {
        for program in cfg.read_all_programs()? {
            let pubkey = program.pubkey()?;
            println!("{}: {}", program.lib_name, pubkey);
        }
        Ok(())
    })
}

/// Sync the program's `declare_id!` pubkey with the pubkey from `target/deploy/<KEYPAIR>.json`.
fn keys_sync(cfg_override: &ConfigOverride, program_name: Option<String>) -> Result<()> {
    with_workspace(cfg_override, |cfg| {
        let declare_id_regex = RegexBuilder::new(r#"^(([\w]+::)*)declare_id!\("(\w*)"\)"#)
            .multi_line(true)
            .build()
            .unwrap();

        for program in cfg.get_programs(program_name)? {
            // Get the pubkey from the keypair file
            let actual_program_id = program.pubkey()?.to_string();

            // Handle declaration in program files
            let src_path = program.path.join("src");
            let files_to_check = vec![src_path.join("lib.rs"), src_path.join("id.rs")];

            for path in files_to_check {
                let mut content = match fs::read_to_string(&path) {
                    Ok(content) => content,
                    Err(_) => continue,
                };

                let incorrect_program_id = declare_id_regex
                    .captures(&content)
                    .and_then(|captures| captures.get(3))
                    .filter(|program_id_match| program_id_match.as_str() != actual_program_id);
                if let Some(program_id_match) = incorrect_program_id {
                    println!("Found incorrect program id declaration in {path:?}");

                    // Update the program id
                    content.replace_range(program_id_match.range(), &actual_program_id);
                    fs::write(&path, content)?;

                    println!("Updated to {actual_program_id}\n");
                    break;
                }
            }

            // Handle declaration in Anchor.toml
            'outer: for programs in cfg.programs.values_mut() {
                for (name, deployment) in programs {
                    // Skip other programs
                    if name != &program.lib_name {
                        continue;
                    }

                    if deployment.address.to_string() != actual_program_id {
                        println!("Found incorrect program id declaration in Anchor.toml for the program `{name}`");

                        // Update the program id
                        deployment.address = Pubkey::from_str(&actual_program_id).unwrap();
                        fs::write(cfg.path(), cfg.to_string())?;

                        println!("Updated to {actual_program_id}\n");
                        break 'outer;
                    }
                }
            }
        }

        println!("All program id declarations are synced.");

        Ok(())
    })
}

fn localnet(
    cfg_override: &ConfigOverride,
    skip_build: bool,
    skip_deploy: bool,
    skip_lint: bool,
    env_vars: Vec<String>,
    cargo_args: Vec<String>,
    arch: ProgramArch,
) -> Result<()> {
    with_workspace(cfg_override, |cfg| {
        // Build if needed.
        if !skip_build {
            build(
                cfg_override,
                false,
                None,
                None,
                false,
                skip_lint,
                None,
                None,
                None,
                BootstrapMode::None,
                BuildSbfOptions::default(),
                None,
                None,
                env_vars,
                cargo_args,
                false,
                arch,
            )?;
        }

        let flags = match skip_deploy {
            true => None,
            false => Some(validator_flags(cfg, &cfg.test_validator)?),
        };

        let validator_handle = &mut start_test_validator(cfg, &cfg.test_validator, flags, false)?;

        // Setup log reader.
        let url = test_validator_rpc_url(&cfg.test_validator);
        let log_streams = stream_logs(cfg, &url);

        std::io::stdin().lock().lines().next().unwrap().unwrap();

        // Check all errors and shut down.
        if let Err(err) = validator_handle.kill() {
            println!(
                "Failed to kill subprocess {}: {}",
                validator_handle.id(),
                err
            );
        }

        for mut child in log_streams? {
            if let Err(err) = child.kill() {
                println!("Failed to kill subprocess {}: {}", child.id(), err);
            }
        }

        Ok(())
    })
}

// with_workspace ensures the current working directory is always the top level
// workspace directory, i.e., where the `Anchor.toml` file is located, before
// and after the closure invocation.
//
// The closure passed into this function must never change the working directory
// to be outside the workspace. Doing so will have undefined behavior.
fn with_workspace<R>(
    cfg_override: &ConfigOverride,
    f: impl FnOnce(&mut WithPath<Config>) -> R,
) -> R {
    set_workspace_dir_or_exit();

    let mut cfg = Config::discover(cfg_override)
        .expect("Previously set the workspace dir")
        .expect("Anchor.toml must always exist");

    let r = f(&mut cfg);

    set_workspace_dir_or_exit();

    r
}

fn is_hidden(entry: &walkdir::DirEntry) -> bool {
    entry
        .file_name()
        .to_str()
        .map(|s| s == "." || s.starts_with('.') || s == "target")
        .unwrap_or(false)
}

fn get_node_version() -> Result<Version> {
    let node_version = std::process::Command::new("node")
        .arg("--version")
        .stderr(Stdio::inherit())
        .output()
        .map_err(|e| anyhow::format_err!("node failed: {}", e.to_string()))?;
    let output = std::str::from_utf8(&node_version.stdout)?
        .strip_prefix('v')
        .unwrap()
        .trim();
    Version::parse(output).map_err(Into::into)
}

fn get_recommended_micro_lamport_fee(client: &RpcClient, priority_fee: Option<u64>) -> Result<u64> {
    if let Some(priority_fee) = priority_fee {
        return Ok(priority_fee);
    }

    let mut fees = client.get_recent_prioritization_fees(&[])?;

    // Get the median fee from the most recent recent 150 slots' prioritization fee
    fees.sort_unstable_by_key(|fee| fee.prioritization_fee);
    let median_index = fees.len() / 2;

    let median_priority_fee = if fees.len() % 2 == 0 {
        (fees[median_index - 1].prioritization_fee + fees[median_index].prioritization_fee) / 2
    } else {
        fees[median_index].prioritization_fee
    };

    Ok(median_priority_fee)
}
/// Prepend a compute unit ix, if the priority fee is greater than 0.
/// This helps to improve the chances that the transaction will land.
fn prepend_compute_unit_ix(
    instructions: Vec<Instruction>,
    client: &RpcClient,
    priority_fee: Option<u64>,
) -> Result<Vec<Instruction>> {
    let priority_fee = get_recommended_micro_lamport_fee(client, priority_fee)?;

    if priority_fee > 0 {
        let mut instructions_appended = instructions.clone();
        instructions_appended.insert(
            0,
            ComputeBudgetInstruction::set_compute_unit_price(priority_fee),
        );
        Ok(instructions_appended)
    } else {
        Ok(instructions)
    }
}

fn get_node_dns_option() -> Result<&'static str> {
    let version = get_node_version()?;
    let req = VersionReq::parse(">=16.4.0").unwrap();
    let option = match req.matches(&version) {
        true => "--dns-result-order=ipv4first",
        false => "",
    };
    Ok(option)
}

// Remove the current workspace directory if it prefixes a string.
// This is used as a workaround for the Solana CLI using the uriparse crate to
// parse args but not handling percent encoding/decoding when using the path as
// a local filesystem path. Removing the workspace prefix handles most/all cases
// of spaces in keypair/binary paths, but this should be fixed in the Solana CLI
// and removed here.
fn strip_workspace_prefix(absolute_path: String) -> String {
    let workspace_prefix = std::env::current_dir().unwrap().display().to_string() + "/";
    absolute_path
        .strip_prefix(&workspace_prefix)
        .unwrap_or(&absolute_path)
        .into()
}

/// Create a new [`RpcClient`] with `confirmed` commitment level instead of the default(finalized).
fn create_client<U: ToString>(url: U) -> RpcClient {
    RpcClient::new_with_commitment(url, CommitmentConfig::confirmed())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[should_panic(expected = "Anchor workspace name must be a valid Rust identifier.")]
    fn test_init_reserved_word() {
        init(
            &ConfigOverride {
                cluster: None,
                wallet: None,
            },
            "await".to_string(),
            true,
            false,
            true,
            false,
            ProgramTemplate::default(),
            TestTemplate::default(),
            false,
        )
        .unwrap();
    }

    #[test]
    #[should_panic(expected = "Anchor workspace name must be a valid Rust identifier.")]
    fn test_init_reserved_word_from_syn() {
        init(
            &ConfigOverride {
                cluster: None,
                wallet: None,
            },
            "fn".to_string(),
            true,
            false,
            true,
            false,
            ProgramTemplate::default(),
            TestTemplate::default(),
            false,
        )
        .unwrap();
    }

    #[test]
    #[should_panic(expected = "Anchor workspace name must be a valid Rust identifier.")]
    fn test_init_starting_with_digit() {
        init(
            &ConfigOverride {
                cluster: None,
                wallet: None,
            },
            "1project".to_string(),
            true,
            false,
            true,
            false,
            ProgramTemplate::default(),
            TestTemplate::default(),
            false,
        )
        .unwrap();
    }
}
