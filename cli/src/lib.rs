use {
    crate::config::{
        get_default_ledger_path, BootstrapMode, BuildConfig, Config, ConfigOverride, HookType,
        Manifest, PackageManager, ProgramDeployment, ProgramWorkspace, ScriptsConfig,
        SurfnetInfoResponse, SurfpoolConfig, TestValidator, Validator, ValidatorType, WithPath,
        SHUTDOWN_WAIT, STARTUP_WAIT, SURFPOOL_HOST,
    },
    abs_path::AbsolutePath,
    anchor_cli_macros::AbsolutePath,
    anchor_client::Cluster,
    anchor_lang::{
        prelude::UpgradeableLoaderState, solana_program::bpf_loader_upgradeable, AnchorDeserialize,
    },
    anchor_lang_idl::{
        convert::{convert_idl, convert_idl_to_legacy},
        types::{Idl, IdlArrayLen, IdlDefinedFields, IdlType, IdlTypeDefTy},
    },
    anyhow::{anyhow, bail, Context, Result},
    base64::{engine::general_purpose::STANDARD, Engine},
    cargo_metadata::{DependencyKind, MetadataCommand},
    checks::{check_anchor_version, check_deps, check_idl_build_feature, check_overflow},
    clap::{CommandFactory, Parser},
    dirs::home_dir,
    heck::{ToKebabCase, ToLowerCamelCase, ToPascalCase, ToSnakeCase},
    regex::{Regex, RegexBuilder},
    semver::{Version, VersionReq},
    serde::Deserialize,
    serde_json::{json, Map, Value as JsonValue},
    solana_cli_config::Config as SolanaCliConfig,
    solana_commitment_config::CommitmentConfig,
    solana_compute_budget_interface::ComputeBudgetInstruction,
    solana_instruction::Instruction,
    solana_keypair::Keypair,
    solana_pubkey::Pubkey,
    solana_pubsub_client::pubsub_client::{PubsubClient, PubsubClientSubscription},
    solana_rpc_client::rpc_client::RpcClient,
    solana_rpc_client_api::{
        config::{RpcTransactionLogsConfig, RpcTransactionLogsFilter},
        request::RpcRequest,
        response::{Response as RpcResponse, RpcLogsResponse},
    },
    solana_signer::{EncodableKey, Signer},
    std::{
        collections::{BTreeMap, HashMap, HashSet},
        ffi::OsString,
        fs::{self, File},
        io::prelude::*,
        path::{Path, PathBuf},
        process::{Child, ExitStatus, Stdio},
        string::ToString,
        sync::{LazyLock, OnceLock},
    },
    template::{AnchorVersion, ProgramTemplate, TestTemplate},
};

mod abs_path;
mod account;
mod checks;
pub mod codama;
pub mod config;
#[cfg(not(windows))]
pub mod coverage;
#[cfg(not(windows))]
pub mod debugger;
pub mod fetch;
#[cfg(not(windows))]
mod flamegraph;
mod keygen;
mod legacy_idl;
mod metadata;
#[cfg(not(windows))]
mod profile;
mod program;
pub mod template;

// Version of the docker image.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
pub const DOCKER_BUILDER_VERSION: &str = VERSION;
/// Default RPC port
pub const DEFAULT_RPC_PORT: u16 = 8899;
const DEFAULT_FAUCET_PORT: u16 = 9900;

/// WebSocket port offset for solana-test-validator (RPC port + 1)
pub const WEBSOCKET_PORT_OFFSET: u16 = 1;

pub static AVM_HOME: LazyLock<PathBuf> = LazyLock::new(|| {
    if let Ok(avm_home) = std::env::var("AVM_HOME") {
        PathBuf::from(avm_home)
    } else {
        let mut user_home = dirs::home_dir().expect("Could not find home directory");
        user_home.push(".avm");
        user_home
    }
});

pub fn support_version_report() -> String {
    let mut lines = vec![format!("anchor-cli {VERSION}")];

    lines.push(command_version_line("solana-cli", "solana"));
    lines.push(command_version_line("cargo", "cargo"));
    lines.push(format!("OS: {}", os_version()));

    lines.join("\n") + "\n"
}

fn command_version_line(label: &str, command: &str) -> String {
    match command_output(command, &["--version"]) {
        Some(version) if version.starts_with(label) => version,
        Some(version) => format!("{label} {version}"),
        None => format!("{label} unavailable"),
    }
}

fn command_output(command: &str, args: &[&str]) -> Option<String> {
    let output = std::process::Command::new(command)
        .args(args)
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }

    String::from_utf8(output.stdout)
        .ok()
        .and_then(|output| output.lines().next().map(str::trim).map(str::to_owned))
        .filter(|line| !line.is_empty())
}

fn os_version() -> String {
    #[cfg(target_os = "macos")]
    if let Some(version) = macos_version() {
        return version;
    }

    #[cfg(target_os = "linux")]
    {
        if let Some(version) = command_output("lsb_release", &["-ds"]) {
            return version.trim_matches('"').to_owned();
        }
        if let Some(version) = linux_os_release() {
            return version;
        }
    }

    #[cfg(target_os = "windows")]
    if let Some(version) = command_output("cmd", &["/C", "ver"]) {
        return version;
    }

    std::env::consts::OS.to_owned()
}

#[cfg(target_os = "macos")]
fn macos_version() -> Option<String> {
    let name = command_output("sw_vers", &["-productName"])?;
    let version = command_output("sw_vers", &["-productVersion"])?;
    let build = command_output("sw_vers", &["-buildVersion"])?;
    Some(format!("{name} {version} {build}"))
}

#[cfg(target_os = "linux")]
fn linux_os_release() -> Option<String> {
    fs::read_to_string("/etc/os-release")
        .ok()?
        .lines()
        .find_map(|line| line.strip_prefix("PRETTY_NAME="))
        .map(|value| value.trim_matches('"').to_owned())
}

#[derive(Debug, Parser, AbsolutePath)]
#[clap(version = VERSION)]
pub struct Opts {
    #[clap(flatten)]
    pub cfg_override: ConfigOverride,
    #[clap(subcommand)]
    pub command: Command,
}

#[derive(Debug, Parser, AbsolutePath)]
pub enum Command {
    /// Initializes a workspace.
    Init {
        /// Workspace name
        name: String,
        /// Use JavaScript instead of TypeScript
        #[clap(short, long)]
        javascript: bool,
        /// Don't install JavaScript dependencies
        #[clap(long)]
        no_install: bool,
        /// Package Manager to use. If omitted, detection cascades
        /// `pnpm` -> `yarn` -> `npm` and picks the first one on PATH. When
        /// set explicitly, the chosen binary must be installed.
        #[clap(value_enum, long)]
        package_manager: Option<PackageManager>,
        /// Don't initialize git
        #[clap(long)]
        no_git: bool,
        /// Rust program template to use
        #[clap(value_enum, short, long, default_value = "multiple")]
        template: ProgramTemplate,
        /// Anchor template version to generate
        #[clap(value_enum, long, default_value = "v1")]
        anchor_version: AnchorVersion,
        /// Test template to use
        #[clap(value_enum, long, default_value = "litesvm")]
        test_template: TestTemplate,
        /// Initialize even if there are files
        #[clap(long, action)]
        force: bool,
        /// Install Solana agent skills
        #[clap(long)]
        install_agent_skills: bool,
    },
    /// Builds the workspace.
    #[clap(name = "build", alias = "b")]
    Build {
        /// True if the build should not fail even if there are no "CHECK" comments
        #[clap(long)]
        skip_lint: bool,
        /// Skip checking for program ID mismatch between keypair and declare_id
        #[clap(long)]
        ignore_keys: bool,
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
        /// Write to stdout
        #[clap(long)]
        stdout: bool,
        /// Arguments to pass to the underlying `cargo expand` command
        #[clap(required = false, last = true)]
        cargo_args: Vec<String>,
    },
    /// Verifies the on-chain bytecode matches the locally compiled artifact.
    /// Run this command inside a program subdirectory, i.e., in the dir
    /// containing the program's Cargo.toml.
    Verify {
        /// The program ID to verify.
        program_id: Pubkey,
        /// The URL of the repository to verify against. Conflicts with `--current-dir`.
        #[clap(long, conflicts_with = "current_dir")]
        repo_url: Option<String>,
        /// The commit hash to verify against. Requires `--repo-url`.
        #[clap(long, requires = "repo_url")]
        commit_hash: Option<String>,
        /// Verify against the source code in the current directory. Conflicts with `--repo-url`.
        #[clap(long)]
        current_dir: bool,
        /// Name of the program to run the command on. Defaults to the package name.
        #[clap(long)]
        program_name: Option<String>,
        /// Any additional arguments to pass to `solana-verify`.
        #[clap(raw = true)]
        args: Vec<String>,
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
        /// Do not build the IDL
        #[clap(long)]
        no_idl: bool,
        /// Flag to keep the local validator running after tests
        /// to be able to check the transactions.
        #[clap(long)]
        detach: bool,
        /// Run the test suites under the specified path
        #[clap(long)]
        run: Vec<String>,
        /// Name of the script to run from [scripts] section (defaults to "test")
        #[clap(long)]
        script: Option<String>,
        /// Validator type to use for local testing
        #[clap(value_enum, long, default_value = "surfpool")]
        validator: ValidatorType,
        /// Profile each test: record per-test SBF register traces and render flamegraph SVGs under target/anchor-v2-profile.
        #[clap(long)]
        profile: bool,
        args: Vec<String>,
        /// Environment variables to pass into the docker container
        #[clap(short, long, required = false)]
        env: Vec<String>,
        /// Arguments to pass to the underlying `cargo build-sbf` command.
        #[clap(required = false, last = true)]
        cargo_args: Vec<String>,
    },
    /// Coverage-guided fuzzing for Solana programs (powered by Crucible).
    Fuzz(crucible_fuzz_cli::Cli),
    /// Creates a new program.
    New {
        /// Program name
        name: String,
        /// Rust program template to use
        #[clap(value_enum, short, long, default_value = "multiple")]
        template: ProgramTemplate,
        /// Anchor template version to generate
        #[clap(value_enum, long, default_value = "v1")]
        anchor_version: AnchorVersion,
        /// Create new program even if there is already one
        #[clap(long, action)]
        force: bool,
    },
    /// Run tests under an instruction-level debugger.
    #[cfg(not(windows))]
    Debugger {
        /// Filter captured traces to tests whose name contains this substring.
        test_name: Option<String>,
        /// Skip the build+test phase and open the TUI over existing traces.
        #[clap(long)]
        skip_run: bool,
        /// Skip `cargo build-sbf`.
        #[clap(long)]
        skip_build: bool,
        /// Forwarded to the underlying `anchor test` invocation.
        #[clap(long)]
        skip_lint: bool,
        /// Drive tests over sbpf's gdb-stub instead of reading dumped trace files.
        #[clap(long)]
        gdb: bool,
        /// Arguments to pass to the underlying `cargo build-sbf` command.
        #[clap(required = false, last = true)]
        cargo_args: Vec<String>,
    },
    /// Generate source-level coverage from SBF register traces.
    #[cfg(not(windows))]
    Coverage {
        /// Skip the build+test phase and generate coverage from existing traces.
        #[clap(long)]
        skip_run: bool,
        /// Skip `cargo build-sbf`.
        #[clap(long)]
        skip_build: bool,
        /// Output path for the LCOV file.
        #[clap(long, default_value = "target/coverage/sbf.lcov")]
        output: String,
        /// Directory containing register trace files.
        #[clap(long, default_value = "target/coverage/traces")]
        trace_dir: String,
        /// Arguments to pass to the underlying `cargo build-sbf` command.
        #[clap(required = false, last = true)]
        cargo_args: Vec<String>,
    },
    /// Commands for interacting with interface definitions.
    Idl {
        #[clap(subcommand)]
        subcmd: IdlCommand,
    },
    /// Remove all artifacts from the generated directories except program keypairs.
    Clean,
    /// Deploys each program in the workspace.
    #[clap(hide = true)]
    #[deprecated(since = "0.32.0", note = "use `anchor program deploy` instead")]
    Deploy {
        /// Only deploy this program
        #[clap(short, long)]
        program_name: Option<String>,
        /// Keypair of the program (filepath) (requires program-name)
        #[clap(long, requires = "program_name")]
        program_keypair: Option<PathBuf>,
        /// If true, deploy from path target/verifiable
        #[clap(short, long)]
        verifiable: bool,
        /// Don't upload IDL during deployment (IDL is uploaded by default)
        #[clap(long)]
        no_idl: bool,
        /// Arguments to pass to the underlying `solana program deploy` command.
        #[clap(required = false, last = true)]
        solana_args: Vec<String>,
    },
    /// Runs the deploy migration script.
    Migrate,
    /// Deploys, initializes an IDL, and migrates all in one command.
    /// Upgrades a single program. The configured wallet must be the upgrade
    /// authority.
    #[clap(hide = true)]
    #[deprecated(since = "0.32.0", note = "use `anchor program upgrade` instead")]
    Upgrade {
        /// The program to upgrade.
        #[clap(short, long)]
        program_id: Pubkey,
        /// Filepath to the new program binary.
        program_filepath: PathBuf,
        /// Max times to retry on failure.
        #[clap(long, default_value = "0")]
        max_retries: u32,
        /// Arguments to pass to the underlying `solana program deploy` command.
        #[clap(required = false, last = true)]
        solana_args: Vec<String>,
    },
    /// Request an airdrop of SOL
    Airdrop {
        /// Amount of SOL to airdrop
        amount: f64,
        /// Recipient address (defaults to configured wallet)
        pubkey: Option<Pubkey>,
    },
    /// Cluster commands.
    Cluster {
        #[clap(subcommand)]
        subcmd: ClusterCommand,
    },
    /// Configuration management commands.
    Config {
        #[clap(subcommand)]
        subcmd: ConfigCommand,
    },
    /// Starts a node shell with an Anchor client setup according to the local
    /// config.
    Shell,
    /// Runs the script defined by the current workspace's Anchor.toml.
    #[clap(alias = "r")]
    Run {
        /// The name of the script to run.
        script: String,
        /// Argument to pass to the underlying script.
        #[clap(required = false, last = true)]
        script_args: Vec<String>,
    },
    /// Program keypair commands.
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
        /// Skip checking for program ID mismatch between keypair and declare_id
        #[clap(long)]
        ignore_keys: bool,
        /// Validator type to use for local testing
        #[clap(value_enum, long, default_value = "surfpool")]
        validator: ValidatorType,
        /// Environment variables to pass into the docker container
        #[clap(short, long, required = false)]
        env: Vec<String>,
        /// Arguments to pass to the underlying `cargo build-sbf` command.
        #[clap(required = false, last = true)]
        cargo_args: Vec<String>,
    },
    /// Fetch and deserialize an account using the IDL provided.
    Account {
        /// Account struct to deserialize (format: <program_name>.<Account>)
        account_type: String,
        /// Address of the account to deserialize
        address: Pubkey,
        /// Path of IDL to use (defaults to workspace IDL)
        #[clap(long)]
        idl: Option<PathBuf>,
    },
    /// Generates shell completions.
    Completions {
        #[clap(value_enum)]
        shell: clap_complete::Shell,
    },
    /// Get your public key
    Address,
    /// Get your balance
    Balance {
        /// Account to check balance for (defaults to configured wallet)
        pubkey: Option<Pubkey>,
        /// Display balance in lamports instead of SOL
        #[clap(long)]
        lamports: bool,
    },
    /// Get current epoch
    Epoch,
    /// Get information about the current epoch
    #[clap(name = "epoch-info")]
    EpochInfo,
    /// Stream transaction logs
    Logs {
        /// Include vote transactions when monitoring all transactions
        #[clap(long)]
        include_votes: bool,
        /// Addresses to filter logs by
        #[clap(long)]
        address: Option<Vec<Pubkey>>,
    },
    /// Show the contents of an account
    ShowAccount {
        #[clap(flatten)]
        cmd: account::ShowAccountCommand,
    },
    /// Keypair generation and management
    Keygen {
        #[clap(subcommand)]
        subcmd: KeygenCommand,
    },
    /// Program deployment and management commands
    Program {
        #[clap(subcommand)]
        subcmd: ProgramCommand,
    },
    /// Codama IDL integration commands
    Codama {
        #[clap(subcommand)]
        subcmd: codama::CodamaCommand,
    },
    /// [DEPRECATED] Manage legacy on-chain IDL accounts.
    /// These commands interact with the old Anchor IDL instruction protocol and will be removed
    /// in a future release. Migrate to Program Metadata-based IDL management (`anchor idl`).
    LegacyIdl {
        #[clap(subcommand)]
        subcmd: legacy_idl::LegacyIdlCommand,
    },
}

#[derive(Debug, Parser, AbsolutePath)]
pub enum KeygenCommand {
    /// Generate a new keypair
    New {
        /// Path to generated keypair file
        #[clap(short = 'o', long)]
        outfile: Option<PathBuf>,
        /// Overwrite the output file if it exists
        #[clap(short, long)]
        force: bool,
        /// Do not prompt for a passphrase
        #[clap(long)]
        no_passphrase: bool,
        /// Do not display the generated pubkey
        #[clap(long)]
        silent: bool,
        /// Number of words in the mnemonic phrase [possible values: 12, 15, 18, 21, 24]
        #[clap(short = 'w', long, default_value = "12")]
        word_count: usize,
    },
    /// Display the pubkey for a given keypair
    Pubkey {
        /// Keypair filepath
        keypair: Option<PathBuf>,
    },
    /// Recover a keypair from a seed phrase
    Recover {
        /// Path to recovered keypair file
        #[clap(short = 'o', long)]
        outfile: Option<PathBuf>,
        /// Overwrite the output file if it exists
        #[clap(short, long)]
        force: bool,
        /// Skip seed phrase validation
        #[clap(long)]
        skip_seed_phrase_validation: bool,
        /// Do not prompt for a passphrase
        #[clap(long)]
        no_passphrase: bool,
    },
    /// Verify a keypair can sign and verify a message
    Verify {
        /// Public key to verify
        pubkey: Pubkey,
        /// Keypair filepath (defaults to configured wallet)
        keypair: Option<PathBuf>,
    },
}

#[derive(Debug, Parser, AbsolutePath)]
pub enum KeysCommand {
    /// List all of the program keys.
    List,
    /// Sync program `declare_id!` pubkeys with the program's actual pubkey.
    Sync {
        /// Only sync the given program instead of all programs
        #[clap(short, long)]
        program_name: Option<String>,
    },
}

#[derive(Debug, Parser, AbsolutePath)]
pub enum ProgramCommand {
    /// Deploy an upgradeable program
    Deploy {
        /// Program filepath (e.g., target/deploy/my_program.so).
        /// If not provided, discovers programs from workspace
        program_filepath: Option<PathBuf>,
        /// Program name to deploy (from workspace). Used when program_filepath is not provided
        #[clap(short, long)]
        program_name: Option<String>,
        /// Program keypair filepath (defaults to target/deploy/{program_name}-keypair.json)
        #[clap(long)]
        program_keypair: Option<PathBuf>,
        /// Upgrade authority keypair (defaults to configured wallet)
        #[clap(long)]
        upgrade_authority: Option<String>,
        /// Program id to deploy to (derived from program-keypair if not specified)
        #[clap(long)]
        program_id: Option<Pubkey>,
        /// Buffer account to use for deployment
        #[clap(long)]
        buffer: Option<Pubkey>,
        /// Maximum transaction length (BPF loader upgradeable limit)
        #[clap(long)]
        max_len: Option<usize>,
        /// Send write transactions through RPC instead of TPU.
        #[clap(long)]
        use_rpc: bool,
        /// Don't upload IDL during deployment (IDL is uploaded by default)
        #[clap(long)]
        no_idl: bool,
        /// Make the program immutable after deployment (cannot be upgraded)
        #[clap(long = "final")]
        make_final: bool,
        /// Additional arguments to configure deployment (e.g., --with-compute-unit-price 1000)
        #[clap(required = false, last = true)]
        solana_args: Vec<String>,
    },
    /// Write a program into a buffer account
    WriteBuffer {
        /// Program filepath (e.g., target/deploy/my_program.so).
        /// If not provided, discovers program from workspace using program_name
        program_filepath: Option<PathBuf>,
        /// Program name to write (from workspace). Used when program_filepath is not provided
        #[clap(short, long)]
        program_name: Option<String>,
        /// Buffer account keypair (defaults to new keypair)
        #[clap(long)]
        buffer: Option<String>,
        /// Buffer authority (defaults to configured wallet)
        #[clap(long)]
        buffer_authority: Option<String>,
        /// Maximum transaction length
        #[clap(long)]
        max_len: Option<usize>,
    },
    /// Set a new buffer authority
    SetBufferAuthority {
        /// Buffer account address
        buffer: Pubkey,
        /// New buffer authority
        new_buffer_authority: Pubkey,
    },
    /// Set a new program authority
    SetUpgradeAuthority {
        /// Program id
        program_id: Pubkey,
        /// New upgrade authority pubkey
        #[clap(long)]
        new_upgrade_authority: Option<Pubkey>,
        /// New upgrade authority signer (keypair file). Required unless --skip-new-upgrade-authority-signer-check is used.
        /// When provided, both current and new authority will sign (checked mode, recommended)
        #[clap(long)]
        new_upgrade_authority_signer: Option<String>,
        /// Skip new upgrade authority signer check. Allows setting authority with only current authority signature.
        /// WARNING: Less safe - use only if you're confident the pubkey is correct
        #[clap(long)]
        skip_new_upgrade_authority_signer_check: bool,
        /// Make the program immutable (cannot be upgraded)
        #[clap(long = "final")]
        make_final: bool,
        /// Current upgrade authority keypair (defaults to configured wallet)
        #[clap(long)]
        upgrade_authority: Option<String>,
    },
    /// Display information about a buffer or program
    Show {
        /// Account address (buffer or program)
        account: Pubkey,
        /// Get account information from the Solana config file
        #[clap(long)]
        get_programs: bool,
        /// Get account information from the Solana config file
        #[clap(long)]
        get_buffers: bool,
        /// Show all accounts
        #[clap(long)]
        all: bool,
    },
    /// Upgrade an upgradeable program
    Upgrade {
        /// Program id to upgrade
        program_id: Pubkey,
        /// Program filepath (e.g., target/deploy/my_program.so). If not provided, discovers from workspace
        #[clap(long)]
        program_filepath: Option<PathBuf>,
        /// Program name to upgrade (from workspace). Used when program_filepath is not provided
        #[clap(short, long)]
        program_name: Option<String>,
        /// Existing buffer account to upgrade from. If not provided, auto-discovers program from workspace
        #[clap(long)]
        buffer: Option<Pubkey>,
        /// Upgrade authority (defaults to configured wallet)
        #[clap(long)]
        upgrade_authority: Option<String>,
        /// Max times to retry on failure
        #[clap(long, default_value = "0")]
        max_retries: u32,
        /// Send write transactions through RPC instead of TPU.
        #[clap(long)]
        use_rpc: bool,
        /// Additional arguments to configure deployment (e.g., --with-compute-unit-price 1000)
        #[clap(required = false, last = true)]
        solana_args: Vec<String>,
    },
    /// Write the program data to a file
    Dump {
        /// Program account address
        account: Pubkey,
        /// Output file path
        output_file: String,
    },
    /// Close a program or buffer account and withdraw all lamports
    Close {
        /// Account address to close (buffer or program).
        /// If not provided, discovers program from workspace using program_name
        account: Option<Pubkey>,
        /// Program name to close (from workspace). Used when account is not provided
        #[clap(short, long)]
        program_name: Option<String>,
        /// Authority keypair (defaults to configured wallet)
        #[clap(long)]
        authority: Option<String>,
        /// Recipient address for reclaimed lamports (defaults to authority)
        #[clap(long)]
        recipient: Option<Pubkey>,
        /// Bypass warning prompts
        #[clap(long)]
        bypass_warning: bool,
    },
    /// Extend the length of an upgradeable program
    Extend {
        /// Program id to extend.
        /// If not provided, discovers program from workspace using program_name
        program_id: Option<Pubkey>,
        /// Program name to extend (from workspace). Used when program_id is not provided
        #[clap(short, long)]
        program_name: Option<String>,
        /// Additional bytes to allocate
        additional_bytes: usize,
    },
}

#[derive(Debug, Parser, AbsolutePath)]
pub enum IdlCommand {
    /// Initializes a program's IDL account. Can only be run once.
    Init {
        /// Program id to initialize IDL for.
        /// If not provided, discovers program ID from IDL.
        program_id: Option<Pubkey>,
        #[clap(short, long)]
        filepath: PathBuf,
        #[clap(long)]
        priority_fee: Option<u64>,
        /// Create non-canonical metadata account (third-party metadata)
        #[clap(long)]
        non_canonical: bool,
        /// Allow running against a localnet cluster (disabled by default)
        #[clap(long)]
        #[cfg(feature = "idl-localnet-testing")]
        allow_localnet: bool,
    },
    /// Upgrades the IDL to the new file. An alias for first writing and then
    /// then setting the idl buffer account.
    Upgrade {
        /// Program id to upgrade IDL for.
        /// If not provided, discovers program ID from IDL.
        program_id: Option<Pubkey>,
        #[clap(short, long)]
        filepath: PathBuf,
        #[clap(long)]
        priority_fee: Option<u64>,
        /// Allow running against a localnet cluster (disabled by default)
        #[clap(long)]
        #[cfg(feature = "idl-localnet-testing")]
        allow_localnet: bool,
    },
    /// Generates the IDL for the program using the compilation method.
    #[clap(alias = "b")]
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
        /// Arguments to pass to the underlying `cargo test` command
        #[clap(required = false, last = true)]
        cargo_args: Vec<String>,
    },
    /// Fetches an IDL for the given program from a cluster.
    Fetch {
        program_id: Pubkey,
        /// Output file for the IDL (stdout if not specified).
        #[clap(short, long)]
        out: Option<String>,
        /// Fetch non-canonical metadata account (third-party metadata)
        #[clap(long)]
        non_canonical: bool,
    },
    /// Fetches historical IDL versions for the given program from a cluster.
    ///
    /// With no filters, fetches all historical versions.
    FetchHistorical {
        program_id: Pubkey,
        /// Fetch authority-scoped PMP metadata account history for this authority
        #[clap(long)]
        authority: Option<Pubkey>,
        /// Fetch IDL at specific slot
        #[clap(long, conflicts_with_all = ["before", "after"])]
        slot: Option<u64>,
        /// Fetch IDL before this date (YYYY-MM-DD)
        #[clap(long)]
        before: Option<String>,
        /// Fetch IDL after this date (YYYY-MM-DD)
        #[clap(long)]
        after: Option<String>,
        /// Output directory for fetched versions (defaults to the current directory)
        #[clap(long)]
        out_dir: Option<PathBuf>,
        /// Max parallel RPC workers for transaction fetches.
        #[clap(long)]
        rpc_workers: Option<usize>,
        /// Force sequential transaction fetches (equivalent to --rpc-workers 1).
        #[clap(long, conflicts_with = "rpc_workers")]
        no_parallel: bool,
        /// Max retry attempts per transaction on 429/timeout errors.
        #[clap(long, default_value_t = 5)]
        rpc_max_retries: u32,
        /// Base backoff in milliseconds between retries (doubled each attempt).
        #[clap(long, default_value_t = 500)]
        rpc_retry_backoff_ms: u64,
        /// Hard cap on signatures fetched per history source.
        #[clap(long, default_value_t = 1000)]
        max_signatures: usize,
        /// Print diagnostic progress messages.
        #[clap(long)]
        verbose: bool,
    },
    /// Convert legacy IDLs (pre Anchor 0.30) to the new IDL spec
    Convert {
        /// Path to the IDL file
        path: PathBuf,
        /// Output file for the IDL (stdout if not specified)
        #[clap(short, long)]
        out: Option<PathBuf>,
        /// Program id to initialize IDL for.
        /// If not provided, discovers program ID from IDL.
        #[clap(short, long)]
        program_id: Option<Pubkey>,
        /// Convert a current-spec IDL back to the legacy (pre Anchor
        /// v0.30) format. Without this flag the converter runs in the
        /// default direction (legacy -> current).
        #[clap(long)]
        to_legacy: bool,
    },
    /// Generate TypeScript type for the IDL
    Type {
        /// Path to the IDL file
        path: PathBuf,
        /// Output file for the IDL (stdout if not specified)
        #[clap(short, long)]
        out: Option<PathBuf>,
    },
    /// Close a metadata account and recover rent
    Close {
        /// The program ID
        program_id: Pubkey,
        /// The seed used for the metadata account (default: "idl")
        #[clap(long, default_value = "idl")]
        seed: String,
        /// Priority fees in micro-lamports per compute unit
        #[clap(long)]
        priority_fee: Option<u64>,
    },
    /// Create a buffer account for metadata
    CreateBuffer {
        /// Path to the metadata file
        #[clap(short, long)]
        filepath: PathBuf,
        /// Priority fees in micro-lamports per compute unit
        #[clap(long)]
        priority_fee: Option<u64>,
    },
    /// Set a new authority on a buffer account
    SetBufferAuthority {
        /// The buffer account address
        buffer: Pubkey,
        /// The new authority
        #[clap(short, long)]
        new_authority: Pubkey,
        /// Priority fees in micro-lamports per compute unit
        #[clap(long)]
        priority_fee: Option<u64>,
    },
    /// Write metadata using a buffer account
    WriteBuffer {
        /// The program ID
        program_id: Pubkey,
        /// The buffer account address
        #[clap(short, long)]
        buffer: Pubkey,
        /// The seed to use for the metadata account (default: "idl")
        #[clap(long, default_value = "idl")]
        seed: String,
        /// Close the buffer after writing
        #[clap(long)]
        close_buffer: bool,
        /// Priority fees in micro-lamports per compute unit
        #[clap(long)]
        priority_fee: Option<u64>,
    },
}

#[derive(Debug, Parser, AbsolutePath)]
pub enum ClusterCommand {
    /// Prints common cluster urls.
    List,
}

#[derive(Debug, Parser, AbsolutePath)]
pub enum ConfigCommand {
    /// Get configuration settings from the local Anchor.toml
    Get,
    /// Set configuration settings in the local Anchor.toml
    Set {
        /// Cluster to connect to (custom URL). Use -um, -ud, -ut, -ul for standard clusters
        #[clap(short = 'u', long = "url")]
        url: Option<String>,
        /// Path to wallet keypair file to update the Anchor.toml file with
        #[clap(short = 'k', long = "keypair")]
        keypair: Option<PathBuf>,
    },
}

fn get_keypair(path: &Path) -> Result<Keypair> {
    solana_keypair::read_keypair_file(path)
        .map_err(|_| anyhow!("Unable to read keypair file ({})", path.display()))
}

/// Format lamports as SOL with trailing zeros removed
fn format_sol(lamports: u64) -> String {
    let sol = lamports as f64 / 1_000_000_000.0;
    let formatted = format!("{:.8}", sol);

    // Remove trailing zeros and decimal point if not needed
    let trimmed = formatted.trim_end_matches('0').trim_end_matches('.');
    format!("{} SOL", trimmed)
}

/// Get cluster URL and wallet path from Anchor config, CLI overrides, or Solana CLI config
fn get_cluster_and_wallet(cfg_override: &ConfigOverride) -> Result<(String, String)> {
    // Try to get from Anchor workspace config first
    if let Ok(Some(cfg)) = Config::discover(cfg_override) {
        return Ok((
            cfg.provider.cluster.url().to_string(),
            cfg.provider.wallet.to_string(),
        ));
    }

    // Try to load Solana CLI config
    let (cluster_url, wallet_path) =
        if let Some(config_file) = solana_cli_config::CONFIG_FILE.as_ref() {
            match SolanaCliConfig::load(config_file) {
                Ok(cli_config) => (
                    cli_config.json_rpc_url.clone(),
                    cli_config.keypair_path.clone(),
                ),
                Err(_) => {
                    // Fallback to defaults if Solana CLI config doesn't exist
                    (
                        "https://api.mainnet-beta.solana.com".to_string(),
                        dirs::home_dir()
                            .map(|home| {
                                home.join(".config/solana/id.json")
                                    .to_string_lossy()
                                    .to_string()
                            })
                            .unwrap_or_else(|| "~/.config/solana/id.json".to_string()),
                    )
                }
            }
        } else {
            // If CONFIG_FILE is None, use defaults
            (
                "https://api.mainnet-beta.solana.com".to_string(),
                dirs::home_dir()
                    .map(|home| {
                        home.join(".config/solana/id.json")
                            .to_string_lossy()
                            .to_string()
                    })
                    .unwrap_or_else(|| "~/.config/solana/id.json".to_string()),
            )
        };

    // Apply cluster override if provided
    let final_cluster = if let Some(cluster) = &cfg_override.cluster {
        cluster.url().to_string()
    } else {
        cluster_url
    };

    Ok((final_cluster, wallet_path))
}

/// Get the recommended priority fee from the RPC client, falling back to 0 if unavailable.
/// `write_locked_accounts` scopes the query to txs that write-locked all of these
/// accounts in recent blocks — passing the accounts the upcoming tx will lock
/// gives a contention-aware fee. Pass `&[]` for a global median (often too low
/// for hot mainnet windows).
pub fn get_recommended_micro_lamport_fee(
    client: &RpcClient,
    write_locked_accounts: &[Pubkey],
) -> u64 {
    let mut fees = match client.get_recent_prioritization_fees(write_locked_accounts) {
        // Fees may be empty or query may fail, e.g. on localnet
        Err(e) => {
            eprintln!("Warning: failed to fetch prioritization fees, defaulting to 0: {e}");
            return 0;
        }
        Ok(f) if f.is_empty() => {
            return 0;
        }
        Ok(f) => f,
    };

    // Get the median fee from the most recent 150 slots' prioritization fee
    fees.sort_unstable_by_key(|fee| fee.prioritization_fee);
    let median_index = fees.len() / 2;

    if fees.len() % 2 == 0 {
        (fees[median_index - 1].prioritization_fee + fees[median_index].prioritization_fee) / 2
    } else {
        fees[median_index].prioritization_fee
    }
}

/// Prepend a compute unit ix, if the priority fee is greater than 0.
pub fn prepend_compute_unit_ix(
    instructions: Vec<Instruction>,
    client: &RpcClient,
    priority_fee: Option<u64>,
    write_locked_accounts: &[Pubkey],
) -> Vec<Instruction> {
    let priority_fee = priority_fee
        .unwrap_or_else(|| get_recommended_micro_lamport_fee(client, write_locked_accounts));

    if priority_fee > 0 {
        let mut instructions_appended = instructions.clone();
        instructions_appended.insert(
            0,
            ComputeBudgetInstruction::set_compute_unit_price(priority_fee),
        );
        instructions_appended
    } else {
        instructions
    }
}

pub fn entry(opts: Opts) -> Result<()> {
    let opts = opts.absolute();

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
        fn parse_version(text: &str) -> Option<String> {
            Some(
                Regex::new(r"(\d+\.\d+\.\S+)")
                    .unwrap()
                    .captures_iter(text)
                    .next()?
                    .get(0)?
                    .as_str()
                    .to_string(),
            )
        }

        fn get_current_version(cmd_name: &str) -> Result<String> {
            let output = std::process::Command::new(cmd_name)
                .arg("--version")
                .output()?;
            if !output.status.success() {
                return Err(anyhow!("Failed to run `{cmd_name} --version`"));
            }

            let output_version = std::str::from_utf8(&output.stdout)?;
            parse_version(output_version)
                .ok_or_else(|| anyhow!("Failed to parse the version of `{cmd_name}`"))
        }

        if let Some(solana_version) = &cfg.toolchain.solana_version {
            let current_version = get_current_version("solana")?;
            if solana_version != &current_version {
                // We are overriding with `solana-install` command instead of using the binaries
                // from `~/.local/share/solana/install/releases` because we use multiple Solana
                // binaries in various commands.
                fn override_solana_version(version: String) -> Result<bool> {
                    // There is a deprecation warning message starting with `1.18.19` which causes
                    // parsing problems https://github.com/otter-sec/anchor/issues/3147
                    let (cmd_name, domain) =
                        if Version::parse(&version)? < Version::parse("1.18.19")? {
                            ("solana-install", "anza.xyz")
                        } else {
                            ("agave-install", "anza.xyz")
                        };

                    // Install the command if it's not installed
                    if get_current_version(cmd_name).is_err() {
                        // `solana-install` and `agave-install` are not usable at the same time i.e.
                        // using one of them makes the other unusable with the default installation,
                        // causing the installation process to run each time users switch between
                        // `agave` supported versions. For example, if the user's active Solana
                        // version is `1.18.17`, and he specifies `solana_version = "2.0.6"`, this
                        // code path will run each time an Anchor command gets executed.
                        eprintln!(
                            "Command not installed: `{cmd_name}`. \
                            See https://github.com/anza-xyz/agave/wiki/Agave-Transition, \
                            installing..."
                        );
                        let install_script = std::process::Command::new("curl")
                            .args([
                                "-sSfL",
                                &format!("https://release.{domain}/v{version}/install"),
                            ])
                            .output()?;
                        let is_successful = std::process::Command::new("sh")
                            .args(["-c", std::str::from_utf8(&install_script.stdout)?])
                            .spawn()?
                            .wait_with_output()?
                            .status
                            .success();
                        if !is_successful {
                            return Err(anyhow!("Failed to install `{cmd_name}`"));
                        }
                    }

                    let output = std::process::Command::new(cmd_name).arg("list").output()?;
                    if !output.status.success() {
                        return Err(anyhow!("Failed to list installed `solana` versions"));
                    }

                    // Hide the installation progress if the version is already installed
                    let is_installed = std::str::from_utf8(&output.stdout)?
                        .lines()
                        .filter_map(parse_version)
                        .any(|line_version| line_version == version);
                    let (stderr, stdout) = if is_installed {
                        (Stdio::null(), Stdio::null())
                    } else {
                        (Stdio::inherit(), Stdio::inherit())
                    };

                    std::process::Command::new(cmd_name)
                        .arg("init")
                        .arg(&version)
                        .stderr(stderr)
                        .stdout(stdout)
                        .spawn()?
                        .wait()
                        .map(|status| status.success())
                        .map_err(|err| anyhow!("Failed to run `{cmd_name}` command: {err}"))
                }

                match override_solana_version(solana_version.to_owned())? {
                    true => restore_cbs.push(Box::new(|| {
                        match override_solana_version(current_version)? {
                            true => Ok(()),
                            false => Err(anyhow!("Failed to restore `solana` version")),
                        }
                    })),
                    false => eprintln!(
                        "Failed to override `solana` version to {solana_version}, using \
                         {current_version} instead"
                    ),
                }
            }
        }

        // Anchor version override should be handled last.
        //
        // When invoked via the AVM proxy (`AVM_ACTIVE=1`), AVM has already resolved
        // the requested toolchain version and spawned the matching binary. Re-execing
        // here would either be a no-op (binary already matches) or fight AVM's
        // resolution (e.g. when AVM resolved via `anchor-lang` in Cargo.toml). Skip.
        if let Some(anchor_version) = &cfg.toolchain.anchor_version {
            if std::env::var("AVM_ACTIVE").is_ok() {
                return Ok(restore_cbs);
            }
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

                    if let Err(e) = install_with_avm(anchor_version, false) {
                        eprintln!(
                            "Failed to install `anchor`: {e}, using {current_version} instead"
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

/// Installs Anchor using AVM, passing `--force` (and optionally) installing
/// `solana-verify`.
fn install_with_avm(version: &str, verify: bool) -> Result<()> {
    let mut cmd = std::process::Command::new("avm");
    cmd.arg("install");
    cmd.arg(version);
    cmd.arg("--force");
    if verify {
        cmd.arg("--verify");
    }
    let status = cmd.status().context("running AVM")?;
    if !status.success() {
        bail!("failed to install `anchor` {version} with avm");
    }
    Ok(())
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
            no_install,
            package_manager,
            no_git,
            template,
            anchor_version,
            test_template,
            force,
            install_agent_skills,
        } => init(
            &opts.cfg_override,
            name,
            javascript,
            no_install,
            package_manager,
            no_git,
            template,
            anchor_version,
            test_template,
            force,
            install_agent_skills,
        ),
        Command::Fuzz(cli) => crucible_fuzz_cli::run(cli),
        Command::New {
            name,
            template,
            anchor_version,
            force,
        } => new(&opts.cfg_override, name, template, anchor_version, force),
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
            ignore_keys,
            no_docs,
        } => build(
            &opts.cfg_override,
            no_idl,
            idl,
            idl_ts,
            verifiable,
            skip_lint,
            ignore_keys,
            program_name,
            solana_version,
            docker_image,
            bootstrap,
            None,
            None,
            env,
            cargo_args,
            no_docs,
        ),
        Command::Verify {
            program_id,
            repo_url,
            commit_hash,
            current_dir,
            program_name,
            args,
        } => verify(
            program_id,
            repo_url,
            commit_hash,
            current_dir,
            program_name,
            args,
        ),
        Command::Clean => clean(&opts.cfg_override),
        #[allow(deprecated)]
        Command::Deploy {
            program_name,
            program_keypair,
            verifiable,
            no_idl,
            solana_args,
        } => {
            eprintln!(
                "Warning: 'anchor deploy' is deprecated. Use 'anchor program deploy' instead."
            );
            deploy(
                &opts.cfg_override,
                program_name,
                program_keypair,
                verifiable,
                no_idl,
                solana_args,
            )
        }
        Command::Expand {
            program_name,
            stdout,
            cargo_args,
        } => expand(&opts.cfg_override, program_name, stdout, &cargo_args),
        #[allow(deprecated)]
        Command::Upgrade {
            program_id,
            program_filepath,
            max_retries,
            solana_args,
        } => {
            eprintln!(
                "Warning: 'anchor upgrade' is deprecated. Use 'anchor program upgrade' instead."
            );
            upgrade(
                &opts.cfg_override,
                program_id,
                program_filepath,
                max_retries,
                solana_args,
            )
        }
        Command::Idl { subcmd } => idl(&opts.cfg_override, subcmd),
        Command::LegacyIdl { subcmd } => {
            legacy_idl::handle_legacy_idl_command(&opts.cfg_override, subcmd)
        }
        Command::Migrate => migrate(&opts.cfg_override),
        Command::Test {
            program_name,
            skip_deploy,
            skip_local_validator,
            skip_build,
            no_idl,
            detach,
            run,
            script,
            validator,
            profile,
            args,
            env,
            cargo_args,
            skip_lint,
        } => test(
            &opts.cfg_override,
            program_name,
            skip_deploy,
            skip_local_validator,
            skip_build,
            skip_lint,
            no_idl,
            detach,
            run,
            script,
            validator,
            profile,
            false,
            args,
            env,
            cargo_args,
        ),
        #[cfg(not(windows))]
        Command::Debugger {
            test_name,
            skip_run,
            skip_build,
            skip_lint,
            gdb,
            cargo_args,
        } => debugger(
            &opts.cfg_override,
            test_name,
            skip_run,
            skip_build,
            skip_lint,
            gdb,
            cargo_args,
        ),
        #[cfg(not(windows))]
        Command::Coverage {
            skip_run,
            skip_build,
            output,
            trace_dir,
            cargo_args,
        } => run_coverage(
            &opts.cfg_override,
            skip_run,
            skip_build,
            &output,
            &trace_dir,
            cargo_args,
        ),
        Command::Airdrop { amount, pubkey } => airdrop(&opts.cfg_override, amount, pubkey),
        Command::Cluster { subcmd } => cluster(subcmd),
        Command::Config { subcmd } => config_cmd(&opts.cfg_override, subcmd),
        Command::Shell => shell(&opts.cfg_override),
        Command::Run {
            script,
            script_args,
        } => run(&opts.cfg_override, script, script_args),
        Command::Keys { subcmd } => keys(&opts.cfg_override, subcmd),
        Command::Localnet {
            skip_build,
            skip_deploy,
            skip_lint,
            ignore_keys,
            validator,
            env,
            cargo_args,
        } => localnet(
            &opts.cfg_override,
            skip_build,
            skip_deploy,
            skip_lint,
            ignore_keys,
            validator,
            env,
            cargo_args,
        ),
        Command::Account {
            account_type,
            address,
            idl,
        } => account(&opts.cfg_override, account_type, address, idl),
        Command::Completions { shell } => {
            clap_complete::generate(
                shell,
                &mut Opts::command(),
                "anchor",
                &mut std::io::stdout(),
            );
            Ok(())
        }
        Command::Address => address(&opts.cfg_override),
        Command::Balance { pubkey, lamports } => balance(&opts.cfg_override, pubkey, lamports),
        Command::Epoch => epoch(&opts.cfg_override),
        Command::EpochInfo => epoch_info(&opts.cfg_override),
        Command::Logs {
            include_votes,
            address,
        } => logs_subscribe(&opts.cfg_override, include_votes, address),
        Command::ShowAccount { cmd } => account::show_account(&opts.cfg_override, cmd),
        Command::Keygen { subcmd } => keygen::keygen(&opts.cfg_override, subcmd),
        Command::Program { subcmd } => program::program(&opts.cfg_override, subcmd),
        Command::Codama { subcmd } => codama::entry(subcmd),
    }
}

/// Cargo does not support nested workspaces. If `start` lives inside a
/// directory tree containing any `Cargo.toml`, refuse to create a new
/// Anchor workspace here and point at `anchor new`, which is the
/// supported flow for adding a program to an existing project.
fn reject_if_inside_cargo_project(start: PathBuf) -> Result<()> {
    if let Some(parent) = Manifest::discover_from_path(start)? {
        return Err(anyhow!(
            "Cannot run `anchor init` inside an existing Cargo project at `{}`.\nTo add a new \
             program to the existing project, run `anchor new <name>` from the workspace root. To \
             create a fresh Anchor workspace, run `anchor init` outside any Cargo project tree.",
            parent.path().display()
        ));
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn init(
    cfg_override: &ConfigOverride,
    name: String,
    javascript: bool,
    no_install: bool,
    package_manager: Option<PackageManager>,
    no_git: bool,
    template: ProgramTemplate,
    anchor_version: AnchorVersion,
    test_template: TestTemplate,
    force: bool,
    install_agent_skills: bool,
) -> Result<()> {
    if !force {
        if Config::discover(cfg_override)?.is_some() {
            return Err(anyhow!("Workspace already initialized"));
        }
        reject_if_inside_cargo_project(std::env::current_dir()?)?;
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

    let uses_node = test_template.uses_node();
    let package_manager = if uses_node {
        Some(resolve_package_manager(package_manager)?)
    } else {
        None
    };
    let test_script = test_template.get_test_script(javascript, package_manager.as_ref());
    cfg.scripts.insert("test".to_owned(), test_script);

    // In-process test templates drive the Solana VM inside `cargo test`, so
    // auto-starting a validator at `anchor test` time is unnecessary.
    if matches!(test_template, TestTemplate::Litesvm | TestTemplate::Mollusk) {
        cfg.skip_local_validator = Some(true);
    }

    let package_manager_cmd = package_manager.as_ref().map(ToString::to_string);
    if uses_node {
        cfg.toolchain.package_manager = package_manager.clone();
    }

    // Initialize .gitignore file
    fs::write(".gitignore", template::git_ignore())?;

    // Initialize .prettierignore file
    fs::write(".prettierignore", template::prettier_ignore())?;

    // Remove the default program if `--force` is passed
    if force {
        let default_program_dir = std::env::current_dir()?
            .join("programs")
            .join(&project_name);
        if default_program_dir.exists() {
            fs::remove_dir_all(default_program_dir)?;
        }
    }

    // Build the program.
    template::create_program(
        &project_name,
        template,
        Some(&test_template),
        anchor_version,
    )?;

    let program_id = template::get_or_create_program_id(&rust_name, target_dir()?);
    let mut localnet = BTreeMap::new();
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

    if uses_node {
        // Build the migrations directory.
        let migrations_path = Path::new("migrations");
        fs::create_dir_all(migrations_path)?;

        let license = get_npm_init_license()?;

        let jest = TestTemplate::Jest == test_template;
        if javascript {
            // Build javascript config
            let mut package_json = File::create("package.json")?;
            package_json
                .write_all(template::package_json(jest, license, anchor_version).as_bytes())?;

            let mut deploy = File::create(migrations_path.join("deploy.js"))?;
            deploy.write_all(template::deploy_script().as_bytes())?;
        } else {
            // Build typescript config
            let mut ts_config = File::create("tsconfig.json")?;
            ts_config.write_all(template::ts_config(jest).as_bytes())?;

            let mut ts_package_json = File::create("package.json")?;
            ts_package_json
                .write_all(template::ts_package_json(jest, license, anchor_version).as_bytes())?;

            let mut deploy = File::create(migrations_path.join("deploy.ts"))?;
            deploy.write_all(template::ts_deploy_script().as_bytes())?;
        }
    }

    test_template.create_test_files(
        &project_name,
        javascript,
        &program_id.to_string(),
        anchor_version,
    )?;

    if !no_install && uses_node {
        let package_manager_cmd =
            package_manager_cmd.expect("Node templates resolve a package manager");
        let output = install_node_modules(&package_manager_cmd)?;
        if !output.status.success() {
            return Err(anyhow!(
                "`{package_manager_cmd} install` failed (exit code {:?}). Re-run with \
                 `--no-install` to keep the generated files without installing dependencies.",
                output.status.code()
            ));
        }
    }

    if !no_git {
        let git_result = std::process::Command::new("git")
            .arg("init")
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit())
            .output()
            .map_err(|e| anyhow::format_err!("git init failed: {}", e))?;
        if !git_result.status.success() {
            eprintln!("Failed to automatically initialize a new git repository");
        }
    }

    if install_agent_skills {
        install_solana_skill();
    }

    println!("{project_name} initialized");

    Ok(())
}

fn install_solana_skill() {
    const SKILL_REPO: &str = "https://github.com/solana-foundation/solana-dev-skill";
    const SKILL_NAME: &str = "solana-dev";

    // Skip if globally installed (active across all projects already)
    if home_dir().is_some_and(|home| {
        home.join(".agents")
            .join("skills")
            .join(SKILL_NAME)
            .exists()
    }) {
        return;
    }

    // Skip if already project-scoped (could be anchor init --force on existing folder)
    let project_path = Path::new(".agents").join("skills").join(SKILL_NAME);
    if project_path.exists() {
        return;
    }

    println!("Installing Solana dev skill for Agents from {SKILL_REPO}");

    let status = std::process::Command::new("npx")
        .args([
            "--yes",
            "skills@1.4.4",
            "add",
            SKILL_REPO,
            "--skill",
            "*",
            "-y",
        ])
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .status();

    match status {
        Ok(s) if s.success() => {
            println!("Solana dev skill installed successfully");
        }
        _ => {
            eprintln!(
                "Warning: Failed to install Solana dev skill. Install manually with:\n  npx \
                 skills add {SKILL_REPO}"
            );
        }
    }
}

const PACKAGE_MANAGER_WATERFALL: &[PackageManager] = &[
    PackageManager::PNPM,
    PackageManager::Yarn,
    PackageManager::NPM,
];

fn package_manager_available(pm: &PackageManager) -> bool {
    let cmd = pm.to_string();
    let mut command = if cfg!(target_os = "windows") {
        let mut command = std::process::Command::new("cmd");
        command.arg(format!("/C {cmd} --version"));
        command
    } else {
        let mut command = std::process::Command::new(&cmd);
        command.arg("--version");
        command
    };
    command
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
}

fn resolve_package_manager(explicit: Option<PackageManager>) -> Result<PackageManager> {
    if let Some(pm) = explicit {
        if !package_manager_available(&pm) {
            return Err(anyhow!(
                "`{pm}` was requested but is not on PATH. Install it or pick a different package \
                 manager with `--package-manager`."
            ));
        }
        return Ok(pm);
    }

    let mut skipped = Vec::new();
    for candidate in PACKAGE_MANAGER_WATERFALL {
        if package_manager_available(candidate) {
            if !skipped.is_empty() {
                let missing = skipped
                    .iter()
                    .map(|pm: &PackageManager| pm.to_string())
                    .collect::<Vec<_>>()
                    .join(", ");
                eprintln!("warning: {missing} not found on PATH, using `{candidate}` instead");
            }
            return Ok(candidate.clone());
        }
        skipped.push(candidate.clone());
    }

    Err(anyhow!(
        "No supported package manager found on PATH (tried pnpm, yarn, npm). Install one of them, \
         or re-run with `--no-install`."
    ))
}

fn install_node_modules(cmd: &str) -> Result<std::process::Output> {
    if cfg!(target_os = "windows") {
        std::process::Command::new("cmd")
            .arg(format!("/C {cmd} install"))
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit())
            .output()
            .map_err(|e| anyhow::format_err!("{} install failed: {}", cmd, e))
    } else {
        std::process::Command::new(cmd)
            .arg("install")
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit())
            .output()
            .map_err(|e| anyhow::format_err!("{} install failed: {}", cmd, e))
    }
}

// Creates a new program crate in the `programs/<name>` directory.
fn new(
    cfg_override: &ConfigOverride,
    name: String,
    template: ProgramTemplate,
    anchor_version: AnchorVersion,
    force: bool,
) -> Result<()> {
    with_workspace(cfg_override, |cfg| -> Result<()> {
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
                    fs::remove_dir_all(std::env::current_dir()?.join("programs").join(&name))?;
                }

                template::create_program(&name, template, None, anchor_version)?;

                programs.insert(
                    name.clone(),
                    ProgramDeployment {
                        address: template::get_or_create_program_id(&name, target_dir()?),
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
    })?
}

/// Array of (path, content) tuple.
pub type Files = Vec<(PathBuf, String)>;

/// Create files from the given (path, content) tuple array.
///
/// # Example
///
/// ```rust,no_run
/// # use anchor_cli::create_files;
/// # use std::path::PathBuf;
/// # fn main() -> anyhow::Result<()> {
/// let files = vec![(PathBuf::from("programs/my_program/src/lib.rs"), "// Content".to_string())];
/// create_files(&files)?;
/// # Ok(())
/// # }
/// ```
pub fn create_files(files: &Files) -> Result<()> {
    for (path, content) in files {
        let path = path
            .display()
            .to_string()
            .replace('/', std::path::MAIN_SEPARATOR_STR);
        let path = Path::new(&path);
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
/// ```rust,no_run
/// # use anchor_cli::override_or_create_files;
/// # use std::path::PathBuf;
/// # fn main() -> anyhow::Result<()> {
/// let files = vec![(PathBuf::from("test.rs"), "// Content".to_string())];
/// override_or_create_files(&files)?;
/// # Ok(())
/// # }
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
    stdout: bool,
    cargo_args: &[String],
) -> Result<()> {
    // Change to the workspace member directory, if needed.
    if let Some(program_name) = program_name.as_ref() {
        cd_member(cfg_override, program_name)?;
    }

    let workspace_cfg = Config::discover(cfg_override)?
        .ok_or_else(|| anyhow!("The 'anchor expand' command requires an Anchor workspace."))?;
    let cfg_parent = workspace_cfg.path().parent().expect("Invalid Anchor.toml");
    let cargo = Manifest::discover()?;

    let expansions_path = cfg_parent.join(".anchor").join("expanded-macros");
    fs::create_dir_all(&expansions_path)?;

    match cargo {
        // No Cargo.toml found, expand entire workspace
        None => expand_all(&workspace_cfg, expansions_path, stdout, cargo_args),
        // Cargo.toml is at root of workspace, expand entire workspace
        Some(cargo) if cargo.path().parent() == workspace_cfg.path().parent() => {
            expand_all(&workspace_cfg, expansions_path, stdout, cargo_args)
        }
        // Reaching this arm means Cargo.toml belongs to a single package. Expand it.
        Some(cargo) => expand_program(
            // If we found Cargo.toml, it must be in a directory so unwrap is safe
            cargo.path().parent().unwrap().to_path_buf(),
            expansions_path,
            stdout,
            cargo_args,
        ),
    }
}

fn expand_all(
    workspace_cfg: &WithPath<Config>,
    expansions_path: PathBuf,
    stdout: bool,
    cargo_args: &[String],
) -> Result<()> {
    let cur_dir = std::env::current_dir()?;
    for p in workspace_cfg.get_program_list()? {
        expand_program(p, expansions_path.clone(), stdout, cargo_args)?;
    }
    std::env::set_current_dir(cur_dir)?;
    Ok(())
}

fn expand_program(
    program_path: PathBuf,
    expansions_path: PathBuf,
    stdout: bool,
    cargo_args: &[String],
) -> Result<()> {
    let cargo = Manifest::from_path(program_path.join("Cargo.toml"))
        .map_err(|_| anyhow!("Could not find Cargo.toml for program"))?;
    let package_name = &cargo
        .package
        .as_ref()
        .ok_or_else(|| anyhow!("Cargo config is missing a package"))?
        .name;

    let mut cmd = std::process::Command::new("cargo");
    cmd.arg("expand")
        .arg("--target-dir")
        .arg(expansions_path.join("expand-target"))
        .arg("--package")
        .arg(package_name)
        .args(cargo_args);

    let handle_err = |err| anyhow!("Failed to run `cargo expand`: {err}");
    let exit_on_err = |exit_status: ExitStatus| {
        if !exit_status.success() {
            eprintln!("'anchor expand' failed. Perhaps you have not installed 'cargo-expand'? https://github.com/dtolnay/cargo-expand#installation");
            std::process::exit(exit_status.code().unwrap_or(1));
        }
    };

    if stdout {
        let status = cmd.status().map_err(handle_err)?;
        exit_on_err(status);
    } else {
        let output = cmd.stderr(Stdio::inherit()).output().map_err(handle_err)?;
        exit_on_err(output.status);

        let program_expansions_path = expansions_path.join(package_name);
        fs::create_dir_all(&program_expansions_path)?;

        let version = cargo.version();
        let time = chrono::Utc::now().to_string().replace(' ', "_");
        let file_path = program_expansions_path.join(format!("{package_name}-{version}-{time}.rs"));
        fs::write(&file_path, &output.stdout)?;

        println!(
            "Expanded {} into file {}\n",
            package_name,
            file_path.to_string_lossy()
        );
    }

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
    ignore_keys: bool,
    program_name: Option<String>,
    solana_version: Option<String>,
    docker_image: Option<String>,
    bootstrap: BootstrapMode,
    stdout: Option<File>, // Used for the package registry server.
    stderr: Option<File>, // Used for the package registry server.
    env_vars: Vec<String>,
    cargo_args: Vec<String>,
    no_docs: bool,
) -> Result<()> {
    // Change to the workspace member directory, if needed.
    if let Some(program_name) = program_name.as_ref() {
        cd_member(cfg_override, program_name)?;
    }
    let cfg = Config::discover(cfg_override)?
        .ok_or_else(|| anyhow!("The 'anchor build' command requires an Anchor workspace."))?;
    let cfg_parent = cfg.path().parent().expect("Invalid Anchor.toml");

    // Require overflow checks
    let workspace_cargo_toml_path = cfg_parent.join("Cargo.toml");
    if workspace_cargo_toml_path.exists() {
        check_overflow(workspace_cargo_toml_path)?;
    }

    // Check whether there is a mismatch between CLI and crate/package versions
    check_anchor_version(&cfg).ok();
    check_deps(&cfg).ok();

    // Check for program ID mismatches before building (skip if --ignore-keys is used), Always skipped in anchor test
    if !ignore_keys {
        check_program_id_mismatch(&cfg, program_name.clone())?;
    }

    let idl_out = match idl {
        Some(idl) => Some(PathBuf::from(idl)),
        None => Some(target_dir()?.join("idl")),
    };
    fs::create_dir_all(idl_out.as_ref().unwrap())?;

    let idl_ts_out = match idl_ts {
        Some(idl_ts) => Some(PathBuf::from(idl_ts)),
        None => Some(target_dir()?.join("types")),
    };
    fs::create_dir_all(idl_ts_out.as_ref().unwrap())?;

    if !cfg.workspace.idls.is_empty() {
        fs::create_dir_all(cfg_parent.join(&cfg.workspace.idls))?;
    };
    if !cfg.workspace.types.is_empty() {
        fs::create_dir_all(cfg_parent.join(&cfg.workspace.types))?;
    };

    cfg.run_hooks(HookType::PreBuild)?;

    let cargo = Manifest::discover()?;
    let build_config = BuildConfig {
        verifiable,
        solana_version: solana_version.or_else(|| cfg.toolchain.solana_version.clone()),
        docker_image: docker_image.unwrap_or_else(|| cfg.docker()),
        bootstrap,
    };
    let built_idl_paths = match cargo {
        // No Cargo.toml so build the entire workspace.
        None => build_all(
            &cfg,
            cfg.path(),
            no_idl,
            idl_out.clone(),
            idl_ts_out.clone(),
            &build_config,
            stdout,
            stderr,
            env_vars,
            cargo_args,
            skip_lint,
            no_docs,
        )?,
        // If the Cargo.toml is at the root, build the entire workspace.
        Some(cargo) if cargo.path().parent() == cfg.path().parent() => build_all(
            &cfg,
            cfg.path(),
            no_idl,
            idl_out.clone(),
            idl_ts_out.clone(),
            &build_config,
            stdout,
            stderr,
            env_vars,
            cargo_args,
            skip_lint,
            no_docs,
        )?,
        // Cargo.toml represents a single package. Build it.
        Some(cargo) => build_cwd(
            &cfg,
            cargo.path().to_path_buf(),
            no_idl,
            idl_out.clone(),
            idl_ts_out.clone(),
            &build_config,
            stdout,
            stderr,
            env_vars,
            cargo_args,
            skip_lint,
            no_docs,
        )?,
    };
    cfg.run_hooks(HookType::PostBuild)?;

    if cfg.clients.auto && !no_idl {
        // Only pass IDLs produced by this build. The output directory can
        // contain stale JSON from earlier full builds, especially after
        // `anchor build --program-name ...` from inside a program crate.
        codama::auto_generate_for_workspace(&cfg.clients, cfg_parent, &built_idl_paths)?;
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
) -> Result<Vec<PathBuf>> {
    let cur_dir = std::env::current_dir()?;
    let r = (|| -> Result<Vec<PathBuf>> {
        match cfg_path.parent() {
            None => Err(anyhow!("Invalid Anchor.toml at {}", cfg_path.display())),
            Some(_parent) => {
                let mut idl_paths = Vec::new();
                for p in get_metadata_ordered_program_list(cfg)? {
                    idl_paths.extend(build_cwd(
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
                    )?);
                }
                Ok(idl_paths)
            }
        }
    })();
    std::env::set_current_dir(cur_dir)?;
    r
}

fn get_metadata_ordered_program_list(cfg: &WithPath<Config>) -> Result<Vec<PathBuf>> {
    let programs = cfg.get_program_list()?;
    let ordered = order_programs_by_metadata(cfg, &programs);
    Ok(ordered.unwrap_or(programs))
}

fn order_programs_by_metadata(
    cfg: &WithPath<Config>,
    programs: &[PathBuf],
) -> Result<Vec<PathBuf>> {
    let workspace_dir = cfg
        .path()
        .parent()
        .ok_or_else(|| anyhow!("Invalid Anchor.toml at {}", cfg.path().display()))?;
    let metadata = MetadataCommand::new()
        .current_dir(workspace_dir)
        .exec()
        .context("Failed to run `cargo metadata`")?;

    let mut package_dirs = HashMap::new();
    for (idx, package) in metadata.packages.iter().enumerate() {
        if package.source.is_some() {
            continue;
        }
        let manifest_path = package.manifest_path.clone().into_std_path_buf();
        if let Some(package_dir) = manifest_path.parent() {
            if let Ok(package_dir) = package_dir.canonicalize() {
                package_dirs.insert(package_dir, idx);
            }
        }
    }

    let program_indices = programs
        .iter()
        .filter_map(|program| package_dirs.get(program).copied())
        .collect::<HashSet<_>>();
    if program_indices.len() != programs.len() {
        bail!("Failed to match all Anchor programs in `cargo metadata`");
    }

    let mut local_deps = vec![Vec::new(); metadata.packages.len()];
    for (idx, package) in metadata.packages.iter().enumerate() {
        for dep in &package.dependencies {
            if dep.kind == DependencyKind::Development {
                continue;
            }
            let Some(dep_path) = dep.path.as_ref() else {
                continue;
            };
            if let Ok(dep_path) = dep_path.clone().into_std_path_buf().canonicalize() {
                if let Some(dep_idx) = package_dirs.get(&dep_path) {
                    local_deps[idx].push(*dep_idx);
                }
            }
        }
    }

    let mut program_closures = HashMap::new();
    for idx in &program_indices {
        program_closures.insert(*idx, local_dependency_closure(*idx, &local_deps));
    }

    let original_order_by_package = programs
        .iter()
        .enumerate()
        .map(|(idx, program)| (package_dirs[program], idx))
        .collect::<HashMap<_, _>>();
    let program_by_index = programs
        .iter()
        .map(|program| (package_dirs[program], program.clone()))
        .collect::<HashMap<_, _>>();
    let ordered = order_program_indices_by_dependency_cache_heuristic(
        &program_indices,
        &program_closures,
        &original_order_by_package,
    )
    .into_iter()
    .map(|idx| program_by_index[&idx].clone())
    .collect();

    Ok(ordered)
}

fn order_program_indices_by_dependency_cache_heuristic(
    program_indices: &HashSet<usize>,
    program_closures: &HashMap<usize, HashSet<usize>>,
    original_order: &HashMap<usize, usize>,
) -> Vec<usize> {
    let mut reverse_dependents = HashMap::new();
    for idx in program_indices {
        reverse_dependents.insert(*idx, 0usize);
    }
    for (program_idx, deps) in program_closures {
        for dep_idx in deps {
            if program_indices.contains(dep_idx) && dep_idx != program_idx {
                *reverse_dependents.entry(*dep_idx).or_default() += 1;
            }
        }
    }

    let mut ordered = program_indices.iter().copied().collect::<Vec<_>>();
    ordered.sort_by(|a, b| {
        let a_deps = &program_closures[a];
        let b_deps = &program_closures[b];
        let a_program_deps = a_deps
            .iter()
            .filter(|idx| program_indices.contains(idx))
            .count();
        let b_program_deps = b_deps
            .iter()
            .filter(|idx| program_indices.contains(idx))
            .count();
        let a_reverse = reverse_dependents[a];
        let b_reverse = reverse_dependents[b];
        let a_isolated = a_program_deps == 0 && a_reverse == 0;
        let b_isolated = b_program_deps == 0 && b_reverse == 0;

        b_isolated
            .cmp(&a_isolated)
            .then_with(|| b_program_deps.cmp(&a_program_deps))
            .then_with(|| b_deps.len().cmp(&a_deps.len()))
            .then_with(|| a_reverse.cmp(&b_reverse))
            .then_with(|| original_order[a].cmp(&original_order[b]))
    });

    ordered
}

fn local_dependency_closure(start: usize, deps: &[Vec<usize>]) -> HashSet<usize> {
    let mut seen = HashSet::new();
    let mut stack = deps[start].clone();

    while let Some(idx) = stack.pop() {
        if seen.insert(idx) {
            stack.extend(deps[idx].iter().copied());
        }
    }

    seen
}

// Runs the build command outside of a workspace.
#[allow(clippy::too_many_arguments)]
fn build_cwd(
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
) -> Result<Vec<PathBuf>> {
    match cargo_toml.parent() {
        None => return Err(anyhow!("Unable to find parent")),
        Some(p) => std::env::set_current_dir(p)?,
    };
    match build_config.verifiable {
        false => _build_cwd(
            cfg, no_idl, idl_out, idl_ts_out, skip_lint, no_docs, cargo_args,
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
        ),
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
) -> Result<Vec<PathBuf>> {
    // Create output dirs.
    let workspace_dir = cfg.path().parent().unwrap().canonicalize()?;
    let target_dir = target_dir()?;
    fs::create_dir_all(target_dir.join("verifiable"))?;
    fs::create_dir_all(target_dir.join("idl"))?;
    fs::create_dir_all(target_dir.join("types"))?;
    if !&cfg.workspace.idls.is_empty() {
        fs::create_dir_all(workspace_dir.join(&cfg.workspace.idls))?;
    }
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
        cargo_args.clone(),
    );

    match result {
        Err(e) => {
            eprintln!("Error during Docker build: {e:?}");
            Err(e)
        }
        Ok(_) => {
            // Build the idl.
            println!("Extracting the IDL");
            let idl = generate_idl(cfg, skip_lint, no_docs, &cargo_args)?;
            // Write out the JSON file.
            println!("Writing the IDL file");
            let out_file = target_dir
                .join("idl")
                .join(&idl.metadata.name)
                .with_extension("json");
            write_idl(&idl, OutFile::File(out_file.clone()))?;

            if !&cfg.workspace.idls.is_empty() {
                write_idl(
                    &idl,
                    OutFile::File(
                        workspace_dir
                            .join(&cfg.workspace.idls)
                            .join(&idl.metadata.name)
                            .with_extension("json"),
                    ),
                )?;
            }

            // Write out the TypeScript type.
            println!("Writing the .ts file");
            let ts_file = target_dir
                .join("types")
                .join(&idl.metadata.name)
                .with_extension("ts");
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
            Ok(vec![out_file])
        }
    }
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
        .map_err(|e| anyhow::format_err!("Docker build failed: {}", e))?;
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
                &format!("https://release.anza.xyz/v{solana_version}/install",),
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
) -> Result<()> {
    let manifest_path =
        pathdiff::diff_paths(cargo_toml.canonicalize()?, cfg_parent.canonicalize()?)
            .ok_or_else(|| anyhow!("Unable to diff paths"))?;
    println!(
        "Building {} manifest: {:?}",
        binary_name,
        manifest_path.display()
    );

    // Execute the build.
    let exit = std::process::Command::new("docker")
        .args([
            "exec",
            "--env",
            "PATH=/root/.local/share/solana/install/active_release/bin:/root/.cargo/bin:/usr/\
             local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin",
        ])
        .args(
            env_vars
                .iter()
                .map(|x| ["--env", x.as_str()])
                .collect::<Vec<[&str; 2]>>()
                .concat(),
        )
        .args([container_name, "cargo"])
        .args(BUILD_SUBCOMMAND)
        .args(["--manifest-path", &manifest_path.display().to_string()])
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
        .map_err(|e| anyhow::format_err!("Docker build failed: {}", e))?;
    if !exit.status.success() {
        return Err(anyhow!("Failed to build program"));
    }

    // Copy the binary out of the docker image.
    println!("Copying out the build artifacts");
    let out_file = crate::target_dir()?
        .join("verifiable")
        .join(&binary_name)
        .with_extension("so")
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
        .map_err(|e| anyhow::format_err!("{}", e))?;
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
        .map_err(|e| anyhow::format_err!("{}", e))?;
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
fn _build_cwd(
    cfg: &WithPath<Config>,
    no_idl: bool,
    idl_out: Option<PathBuf>,
    idl_ts_out: Option<PathBuf>,
    skip_lint: bool,
    no_docs: bool,
    cargo_args: Vec<String>,
) -> Result<Vec<PathBuf>> {
    let exit = std::process::Command::new("cargo")
        .args(BUILD_SUBCOMMAND)
        .args(cargo_args.clone())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .output()
        .map_err(|e| anyhow::format_err!("{}", e))?;
    if !exit.status.success() {
        std::process::exit(exit.status.code().unwrap_or(1));
    }

    // Generate IDL
    if !no_idl {
        let idl = generate_idl(cfg, skip_lint, no_docs, &cargo_args)?;
        let cfg_parent = cfg.path().parent().expect("Invalid Anchor.toml");

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
        write_idl(&idl, OutFile::File(out.clone()))?;
        if !&cfg.workspace.idls.is_empty() {
            write_idl(
                &idl,
                OutFile::File(
                    cfg_parent
                        .join(&cfg.workspace.idls)
                        .join(&idl.metadata.name)
                        .with_extension("json"),
                ),
            )?;
        }
        // Write out the TypeScript type.
        fs::write(&ts_out, idl_ts(&idl)?)?;

        // Copy out the TypeScript type.
        if !&cfg.workspace.types.is_empty() {
            fs::copy(
                &ts_out,
                cfg_parent
                    .join(&cfg.workspace.types)
                    .join(&idl.metadata.name)
                    .with_extension("ts"),
            )?;
        }
        Ok(vec![out])
    } else {
        Ok(Vec::new())
    }
}

/// Subcommand and any arguments to be passed to cargo
const BUILD_SUBCOMMAND: &[&str] = &["build-sbf", "--tools-version", "v1.52"];

/// Run the configured SBF build command.
pub fn cargo_build_sbf(cwd: Option<&Path>, extra_args: &[String]) -> Result<()> {
    let mut cmd = std::process::Command::new("cargo");
    if let Some(d) = cwd {
        cmd.current_dir(d);
    }
    let status = cmd
        .args(BUILD_SUBCOMMAND)
        .args(extra_args)
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .status()
        .context("running cargo build-sbf")?;
    if !status.success() {
        return Err(anyhow!(
            "`cargo {}` failed with status {status}",
            BUILD_SUBCOMMAND.join(" ")
        ));
    }
    Ok(())
}

pub fn verify(
    program_id: Pubkey,
    repo_url: Option<String>,
    commit_hash: Option<String>,
    current_dir: bool,
    program_name: Option<String>,
    args: Vec<String>,
) -> Result<()> {
    let mut command_args = Vec::new();

    match (current_dir, repo_url) {
        (true, _) => {
            let current_path = std::env::current_dir()?
                .to_str()
                .ok_or_else(|| anyhow!("Invalid current directory path"))?
                .to_owned();
            command_args.push(current_path);
            command_args.push("--current-dir".into());
        }
        (false, Some(url)) => {
            command_args.push(url);
        }
        (false, None) => {
            return Err(anyhow!(
                "You must provide either --repo-url or --current-dir"
            ));
        }
    }

    if let Some(commit) = commit_hash {
        command_args.push("--commit-hash".into());
        command_args.push(commit);
    }

    if let Some(name) = program_name {
        command_args.push("--library-name".into());
        command_args.push(name);
    }

    command_args.push("--program-id".into());
    command_args.push(program_id.to_string());

    command_args.extend(args);

    println!("Verifying program {program_id}");
    let verify_path = AVM_HOME.join("bin").join("solana-verify");
    if !verify_path.exists() {
        install_with_avm(env!("CARGO_PKG_VERSION"), true)
            .context("installing Anchor with solana-verify")?;
    }

    let status = std::process::Command::new(verify_path)
        .arg("verify-from-repo")
        .args(&command_args)
        .stdout(std::process::Stdio::inherit())
        .stderr(std::process::Stdio::inherit())
        .status()
        .with_context(|| "Failed to run `solana-verify`")?;

    if !status.success() {
        return Err(anyhow!("Failed to verify program"));
    }

    Ok(())
}

fn cd_member(cfg_override: &ConfigOverride, program_name: &str) -> Result<()> {
    // Change directories to the given `program_name`, using either Anchor or Cargo workspace
    let programs = program::get_programs_from_workspace(cfg_override, None)?;

    for program in programs {
        let cargo_toml = program.path.join("Cargo.toml");
        if !cargo_toml.exists() {
            return Err(anyhow!(
                "Did not find Cargo.toml at the path: {}",
                program.path.display()
            ));
        }

        let manifest = Manifest::from_path(&cargo_toml)?;
        let pkg_name = manifest.package().name();
        if program_name == pkg_name || program_name == program.lib_name {
            std::env::set_current_dir(&program.path)?;
            return Ok(());
        }
    }

    Err(anyhow!("{} is not part of the workspace", program_name,))
}

fn idl(cfg_override: &ConfigOverride, subcmd: IdlCommand) -> Result<()> {
    match subcmd {
        IdlCommand::Init {
            program_id,
            filepath,
            priority_fee,
            non_canonical,
            #[cfg(feature = "idl-localnet-testing")]
            allow_localnet,
        } => {
            #[cfg(feature = "idl-localnet-testing")]
            let allow_localnet = allow_localnet;
            #[cfg(not(feature = "idl-localnet-testing"))]
            let allow_localnet = false;
            idl_init(
                program_id,
                cfg_override,
                filepath,
                priority_fee,
                non_canonical,
                allow_localnet,
            )
        }
        IdlCommand::Upgrade {
            program_id,
            filepath,
            priority_fee,
            #[cfg(feature = "idl-localnet-testing")]
            allow_localnet,
        } => {
            #[cfg(feature = "idl-localnet-testing")]
            let allow_localnet = allow_localnet;
            #[cfg(not(feature = "idl-localnet-testing"))]
            let allow_localnet = false;
            idl_upgrade(
                program_id,
                cfg_override,
                filepath,
                priority_fee,
                allow_localnet,
            )
        }
        IdlCommand::Build {
            program_name,
            out,
            out_ts,
            no_docs,
            skip_lint,
            cargo_args,
        } => idl_build(
            cfg_override,
            program_name,
            out,
            out_ts,
            no_docs,
            skip_lint,
            cargo_args,
        ),
        IdlCommand::Fetch {
            program_id: address,
            out,
            non_canonical,
        } => idl_fetch(cfg_override, address, out, non_canonical),
        IdlCommand::FetchHistorical {
            program_id: address,
            authority,
            slot,
            before,
            after,
            out_dir,
            rpc_workers,
            no_parallel,
            rpc_max_retries,
            rpc_retry_backoff_ms,
            max_signatures,
            verbose,
        } => fetch::idl_fetch_historical(
            cfg_override,
            address,
            authority,
            slot,
            before,
            after,
            out_dir,
            fetch::FetchTuning {
                workers: rpc_workers,
                no_parallel,
                max_retries: rpc_max_retries,
                retry_backoff_ms: rpc_retry_backoff_ms,
                max_signatures,
                verbose,
            },
        ),
        IdlCommand::Convert {
            path,
            out,
            program_id,
            to_legacy,
        } => idl_convert(path, out, program_id, to_legacy),
        IdlCommand::Type { path, out } => idl_type(path, out),
        IdlCommand::Close {
            program_id,
            seed,
            priority_fee,
        } => idl_close_metadata(cfg_override, program_id, seed, priority_fee),
        IdlCommand::CreateBuffer {
            filepath,
            priority_fee,
        } => idl_create_buffer(cfg_override, filepath, priority_fee),
        IdlCommand::SetBufferAuthority {
            buffer,
            new_authority,
            priority_fee,
        } => idl_set_buffer_authority(cfg_override, buffer, new_authority, priority_fee),
        IdlCommand::WriteBuffer {
            program_id,
            buffer,
            seed,
            close_buffer,
            priority_fee,
        } => idl_write_buffer_metadata(
            cfg_override,
            program_id,
            buffer,
            seed,
            close_buffer,
            priority_fee,
        ),
    }
}

fn idl_init(
    program_id: Option<Pubkey>,
    cfg_override: &ConfigOverride,
    idl_filepath: PathBuf,
    priority_fee: Option<u64>,
    non_canonical: bool,
    allow_localnet: bool,
) -> Result<()> {
    // Get cluster URL and wallet path from Anchor config
    let (cluster_url, wallet_path) = get_cluster_and_wallet(cfg_override)?;

    let is_localnet = cluster_url.contains("localhost") || cluster_url.contains("127.0.0.1");
    if is_localnet && !allow_localnet {
        #[cfg(feature = "idl-localnet-testing")]
        println!(
            "Skipping IDL initialization on localnet. To deploy on localnet, use --allow-localnet"
        );
        #[cfg(not(feature = "idl-localnet-testing"))]
        println!("Skipping IDL initialization on localnet");
        return Ok(());
    }

    let program_id = match program_id {
        Some(id) => id.to_string(),
        _ => {
            let idl = fs::read(&idl_filepath)?;
            let idl = convert_idl(&idl)?;
            idl.address
        }
    };

    let command = metadata::IdlCommand::funded(
        cluster_url,
        wallet_path,
        priority_fee,
        metadata::FundedIdlSubcommand::Write {
            program_id,
            idl_filepath: idl_filepath
                .to_str()
                .ok_or_else(|| anyhow!("IDL filepath is not valid UTF-8"))?
                .to_string(),
            non_canonical,
        },
    );

    if !command.status()?.success() {
        return Err(anyhow!("Failed to initialize IDL"));
    }

    println!("IDL initialized.");
    Ok(())
}

// Currently identical to `idl_init`, other than not accepting `non_canonical`
fn idl_upgrade(
    program_id: Option<Pubkey>,
    cfg_override: &ConfigOverride,
    idl_filepath: PathBuf,
    priority_fee: Option<u64>,
    allow_localnet: bool,
) -> Result<()> {
    // Get cluster URL and wallet path from Anchor config
    let (cluster_url, wallet_path) = get_cluster_and_wallet(cfg_override)?;

    let is_localnet = cluster_url.contains("localhost") || cluster_url.contains("127.0.0.1");
    if is_localnet && !allow_localnet {
        #[cfg(feature = "idl-localnet-testing")]
        println!("Skipping IDL upgrade on localnet. To deploy on localnet, use --allow-localnet");
        #[cfg(not(feature = "idl-localnet-testing"))]
        println!("Skipping IDL upgrade on localnet");
        return Ok(());
    }

    let program_id = match program_id {
        Some(id) => id.to_string(),
        _ => {
            let idl = fs::read(&idl_filepath)?;
            let idl = convert_idl(&idl)?;
            idl.address
        }
    };

    let command = metadata::IdlCommand::funded(
        cluster_url,
        wallet_path,
        priority_fee,
        metadata::FundedIdlSubcommand::Write {
            program_id,
            idl_filepath: idl_filepath
                .to_str()
                .ok_or_else(|| anyhow!("IDL filepath is not valid UTF-8"))?
                .to_string(),
            non_canonical: false,
        },
    );
    if !command.status()?.success() {
        return Err(anyhow!("Failed to upgrade IDL"));
    }

    println!("IDL upgraded.");
    Ok(())
}

fn idl_build(
    cfg_override: &ConfigOverride,
    program_name: Option<String>,
    out: Option<String>,
    out_ts: Option<String>,
    no_docs: bool,
    skip_lint: bool,
    cargo_args: Vec<String>,
) -> Result<()> {
    let cfg = Config::discover(cfg_override)?
        .ok_or_else(|| anyhow!("The 'anchor idl build' command requires an Anchor workspace."))?;
    let current_dir = std::env::current_dir()?;
    let program_path = match program_name {
        Some(name) => cfg.get_program(&name)?.path,
        None => {
            let programs = cfg.read_all_programs()?;
            if programs.len() == 1 {
                programs.into_iter().next().unwrap().path
            } else {
                programs
                    .into_iter()
                    .find(|program| program.path == current_dir)
                    .ok_or_else(|| anyhow!("Not in a program directory"))?
                    .path
            }
        }
    };
    std::env::set_current_dir(program_path)?;
    let idl = generate_idl(&cfg, skip_lint, no_docs, &cargo_args)?;
    std::env::set_current_dir(current_dir)?;

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
fn generate_idl(
    cfg: &WithPath<Config>,
    skip_lint: bool,
    no_docs: bool,
    cargo_args: &[String],
) -> Result<Idl> {
    check_idl_build_feature()?;

    let idl = anchor_lang_idl::build::IdlBuilder::new()
        .resolution(cfg.features.resolution)
        .skip_lint(cfg.features.skip_lint || skip_lint)
        .no_docs(no_docs)
        .cargo_args(cargo_args.into())
        .build()?;

    // Warn users if there is a potential for a conflict between user-defined discriminators and
    // hardcoded `event-cpi` discriminator.
    //
    // Note: Warn independent of whether the user has the `event-cpi` feature enabled to make sure
    // there are no potential conflicts in the future if/when the user decides to enable it.
    idl.instructions
        .iter()
        .filter(|ix| anchor_lang::event::EVENT_IX_TAG_LE.starts_with(&ix.discriminator))
        .for_each(|ix| {
            eprintln!(
                "Warning: Instruction conflicts with `event-cpi` instruction discriminator: `{}`",
                ix.name
            );
        });

    Ok(idl)
}

fn idl_fetch(
    cfg_override: &ConfigOverride,
    address: Pubkey,
    out: Option<String>,
    non_canonical: bool,
) -> Result<()> {
    let (cluster_url, _) = get_cluster_and_wallet(cfg_override)?;
    let command = metadata::IdlCommand::unfunded(
        cluster_url,
        metadata::UnfundedIdlSubcommand::Fetch {
            program_id: address.to_string(),
            out,
            non_canonical,
        },
    );

    if !command.status()?.success() {
        return Err(anyhow!("Failed to fetch IDL"));
    }
    Ok(())
}

/// Apply a `--program-id` override to a raw IDL JSON document. The current
/// and legacy specs store the program address in different places, so we
/// detect the spec by the presence of `metadata.spec` and patch the right
/// field. For the legacy spec we merge into the existing `metadata` object
/// instead of replacing it; replacing it would drop sibling fields, and for
/// a current-spec IDL it would also wipe `metadata.{spec,name,version}` and
/// cause `convert_idl` to mis-detect the file as legacy.
fn apply_program_id_override(idl: &[u8], program_id: Pubkey) -> Result<Vec<u8>> {
    let mut idl = serde_json::from_slice::<serde_json::Value>(idl)?;
    let obj = idl
        .as_object_mut()
        .ok_or_else(|| anyhow!("IDL must be an object"))?;
    let pid = program_id.to_string();
    let is_current_spec = obj.get("metadata").and_then(|m| m.get("spec")).is_some();
    if is_current_spec {
        // Current spec stores the address at the top level.
        obj.insert("address".into(), serde_json::Value::String(pid));
    } else {
        // Legacy spec stores it under `metadata.address`. Merge so we
        // don't drop any sibling metadata fields the file may already
        // carry.
        match obj.get_mut("metadata") {
            Some(serde_json::Value::Object(m)) => {
                m.insert("address".into(), serde_json::Value::String(pid));
            }
            _ => {
                obj.insert("metadata".into(), serde_json::json!({ "address": pid }));
            }
        }
    }
    serde_json::to_vec(&idl).map_err(Into::into)
}

fn idl_convert(
    path: PathBuf,
    out: Option<PathBuf>,
    program_id: Option<Pubkey>,
    to_legacy: bool,
) -> Result<()> {
    let idl = fs::read(path)?;
    let idl = match program_id {
        Some(program_id) => apply_program_id_override(&idl, program_id)?,
        None => idl,
    };

    // Normalize either input spec to a current-spec `Idl`; both output
    // branches need the parsed value.
    let parsed = convert_idl(&idl)?;
    let out = match out {
        None => OutFile::Stdout,
        Some(out) => OutFile::File(out),
    };
    if to_legacy {
        let bytes = convert_idl_to_legacy(&parsed)?;
        match out {
            OutFile::Stdout => {
                let s =
                    std::str::from_utf8(&bytes).context("legacy IDL JSON was not valid UTF-8")?;
                println!("{s}");
                Ok(())
            }
            OutFile::File(path) => fs::write(path, bytes).map_err(Into::into),
        }
    } else {
        write_idl(&parsed, out)
    }
}

fn idl_type(path: PathBuf, out: Option<PathBuf>) -> Result<()> {
    let idl = fs::read(path)?;
    let idl = convert_idl(&idl)?;
    let types = idl_ts(&idl)?;
    match out {
        Some(out) => fs::write(out, types)?,
        _ => println!("{types}"),
    };
    Ok(())
}

fn idl_close_metadata(
    cfg_override: &ConfigOverride,
    program_id: Pubkey,
    seed: String,
    priority_fee: Option<u64>,
) -> Result<()> {
    let (cluster_url, wallet_path) = get_cluster_and_wallet(cfg_override)?;
    let command = metadata::IdlCommand::funded(
        cluster_url,
        wallet_path,
        priority_fee,
        metadata::FundedIdlSubcommand::Close {
            program_id: program_id.to_string(),
            seed,
        },
    );

    if !command.status()?.success() {
        return Err(anyhow!("Failed to close metadata account"));
    }

    println!("Metadata account closed successfully.");
    Ok(())
}

fn idl_create_buffer(
    cfg_override: &ConfigOverride,
    filepath: PathBuf,
    priority_fee: Option<u64>,
) -> Result<()> {
    let (cluster_url, wallet_path) = get_cluster_and_wallet(cfg_override)?;
    let command = metadata::IdlCommand::funded(
        cluster_url,
        wallet_path,
        priority_fee,
        metadata::FundedIdlSubcommand::CreateBuffer {
            filepath: filepath
                .to_str()
                .ok_or_else(|| anyhow!("IDL filepath is not valid UTF-8"))?
                .to_string(),
        },
    );

    if !command.status()?.success() {
        return Err(anyhow!("Failed to create buffer"));
    }

    println!("Buffer created successfully.");
    Ok(())
}

fn idl_set_buffer_authority(
    cfg_override: &ConfigOverride,
    buffer: Pubkey,
    new_authority: Pubkey,
    priority_fee: Option<u64>,
) -> Result<()> {
    let (cluster_url, wallet_path) = get_cluster_and_wallet(cfg_override)?;
    let command = metadata::IdlCommand::funded(
        cluster_url,
        wallet_path,
        priority_fee,
        metadata::FundedIdlSubcommand::SetBufferAuthority {
            buffer: buffer.to_string(),
            new_authority: new_authority.to_string(),
        },
    );

    if !command.status()?.success() {
        return Err(anyhow!("Failed to set buffer authority"));
    }

    println!("Buffer authority set successfully.");
    Ok(())
}

fn idl_write_buffer_metadata(
    cfg_override: &ConfigOverride,
    program_id: Pubkey,
    buffer: Pubkey,
    seed: String,
    close_buffer: bool,
    priority_fee: Option<u64>,
) -> Result<()> {
    let (cluster_url, wallet_path) = get_cluster_and_wallet(cfg_override)?;
    let command = metadata::IdlCommand::funded(
        cluster_url,
        wallet_path,
        priority_fee,
        metadata::FundedIdlSubcommand::WriteBuffer {
            program_id: program_id.to_string(),
            buffer: buffer.to_string(),
            seed,
            close_buffer,
        },
    );

    if !command.status()?.success() {
        return Err(anyhow!("Failed to write metadata using buffer"));
    }

    println!("Metadata written successfully using buffer.");
    Ok(())
}

fn idl_ts(idl: &Idl) -> Result<String> {
    let idl_name = &idl.metadata.name;
    let type_name = idl_name.to_pascal_case();
    let mut camel_idl = serde_json::to_value(idl)?;
    camel_case_idl_identifiers(&mut camel_idl);
    let camel_idl = serde_json::to_string_pretty(&serde_json::from_value::<Idl>(camel_idl)?)?;

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

fn camel_case_idl_identifiers(value: &mut JsonValue) {
    match value {
        JsonValue::Array(values) => {
            for value in values {
                camel_case_idl_identifiers(value);
            }
        }
        JsonValue::Object(map) => {
            for (key, value) in map {
                if is_idl_identifier_key(key) {
                    camel_case_idl_identifier(value);
                } else {
                    camel_case_idl_identifiers(value);
                }
            }
        }
        _ => {}
    }
}

fn camel_case_idl_identifier(value: &mut JsonValue) {
    match value {
        JsonValue::String(s) => {
            if Pubkey::try_from(s.as_str()).is_err() {
                *s = s
                    .split('.')
                    .map(ToLowerCamelCase::to_lower_camel_case)
                    .collect::<Vec<_>>()
                    .join(".");
            }
        }
        JsonValue::Array(values) => {
            for value in values {
                camel_case_idl_identifier(value);
            }
        }
        _ => camel_case_idl_identifiers(value),
    }
}

fn is_idl_identifier_key(key: &str) -> bool {
    matches!(key, "name" | "path" | "account" | "relations" | "generic")
}

fn write_idl(idl: &Idl, out: OutFile) -> Result<()> {
    let idl_json = serde_json::to_string_pretty(idl)?;
    match out {
        OutFile::Stdout => println!("{idl_json}"),
        OutFile::File(out) => fs::write(out, idl_json)?,
    };

    Ok(())
}
fn account(
    cfg_override: &ConfigOverride,
    account_type: String,
    address: Pubkey,
    idl_filepath: Option<PathBuf>,
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
            let config = Config::discover(cfg_override)?.ok_or_else(|| {
                anyhow!(
                    "The 'anchor account' command requires an Anchor workspace with Anchor.toml \
                     for IDL type generation."
                )
            })?;
            let programs = config
                .read_all_programs()
                .expect("Workspace must contain atleast one program.");

            let program = programs
                .iter()
                .find(|p| p.lib_name == *program_name)
                .ok_or_else(|| {
                    let mut available_programs: Vec<String> =
                        programs.iter().map(|p| p.lib_name.clone()).collect();
                    available_programs.sort();

                    if available_programs.is_empty() {
                        anyhow!(
                            "Program '{program_name}' not found in workspace. No programs \
                             available."
                        )
                    } else {
                        anyhow!(
                            "Program '{program_name}' not found in workspace.\n\nAvailable \
                             programs:\n  {}",
                            available_programs.join("\n  ")
                        )
                    }
                })?;

            program.idl.clone().ok_or_else(|| {
                anyhow!("IDL not found. Please build the program atleast once to generate the IDL.")
            })
        },
        |idl_path| {
            let idl = fs::read(idl_path)?;
            let idl = convert_idl(&idl)?;
            if idl.metadata.name != *program_name {
                return Err(anyhow!("IDL does not match program {program_name}."));
            }

            Ok(idl)
        },
    )?;

    let cluster = match &cfg_override.cluster {
        Some(cluster) => cluster.clone(),
        None => Config::discover(cfg_override)?
            .map(|cfg| cfg.provider.cluster.clone())
            .unwrap_or(Cluster::Localnet),
    };

    let data = create_client(cluster.url()).get_account_data(&address)?;
    let disc_len = idl
        .accounts
        .iter()
        .find(|acc| acc.name == *account_type_name)
        .map(|acc| acc.discriminator.len())
        .ok_or_else(|| {
            let mut available_accounts: Vec<String> =
                idl.accounts.iter().map(|acc| acc.name.clone()).collect();
            available_accounts.sort();

            if available_accounts.is_empty() {
                anyhow!(
                    "Account '{account_type_name}' not found in IDL. No accounts available in \
                     program '{program_name}'."
                )
            } else {
                anyhow!(
                    "Account '{account_type_name}' not found in IDL.\n\nAvailable accounts in \
                     program '{program_name}':\n  {}",
                    available_accounts.join("\n  ")
                )
            }
        })?;
    let mut data_view = &data[disc_len..];

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
                .ok_or_else(|| anyhow!("Error while deserializing enum variant {repr}"))?;

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
            json!(<u128 as AnchorDeserialize>::deserialize(data)?)
        }
        IdlType::I128 => {
            json!(<i128 as AnchorDeserialize>::deserialize(data)?)
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
    no_idl: bool,
    detach: bool,
    tests_to_run: Vec<String>,
    script_name: Option<String>,
    validator_type: ValidatorType,
    profile: bool,
    gdb: bool,
    extra_args: Vec<String>,
    env_vars: Vec<String>,
    cargo_args: Vec<String>,
) -> Result<()> {
    let test_paths = tests_to_run
        .iter()
        .map(|path| {
            PathBuf::from(path)
                .canonicalize()
                .map_err(|_| anyhow!("Wrong path {}", path))
        })
        .collect::<Result<Vec<_>, _>>()?;

    with_workspace(cfg_override, |cfg| -> Result<()> {
        // Set validator type based on CLI choice
        cfg.validator = Some(validator_type);

        let cli_skip_local_validator = skip_local_validator;
        let config_skip_local_validator = cfg.skip_local_validator.unwrap_or(false);
        let workspace_root = cfg.path().parent().unwrap().to_owned();

        #[cfg(windows)]
        if profile {
            return Err(anyhow!(
                "`anchor test --profile` is not supported on Windows"
            ));
        }
        #[cfg(windows)]
        let _ = gdb;

        #[cfg(not(windows))]
        let profile_dir = workspace_root.join(crate::profile::DEFAULT_PROFILE_DIR);
        #[cfg(not(windows))]
        let _gdb_guard: Option<crate::debugger::gdb::GdbDriver> = if profile {
            let _ = fs::remove_dir_all(&profile_dir);
            std::env::set_var("ANCHOR_PROFILE_DIR", &profile_dir);
            std::env::set_var("CARGO_PROFILE_RELEASE_DEBUG", "2");

            if let Some(test_script) = cfg.scripts.get_mut("test") {
                if test_script.contains("cargo test") {
                    *test_script =
                        test_script.replacen("cargo test", "cargo test --features profile", 1);
                    if gdb {
                        let sep = if test_script.contains(" -- ") {
                            " "
                        } else {
                            " -- "
                        };
                        *test_script = format!("{test_script}{sep}--test-threads=1");
                    }
                } else {
                    eprintln!(
                        "warning: --profile requires the `test` script in Anchor.toml to invoke \
                         `cargo test`; got: {test_script:?}. Profiling will not activate."
                    );
                }
            } else {
                eprintln!(
                    "warning: --profile requires a [scripts] test entry in Anchor.toml; none \
                     found. Profiling will not activate."
                );
            }

            if gdb {
                let driver = crate::debugger::gdb::start_gdb_driver(&profile_dir)?;
                std::env::set_var(crate::debugger::gdb::SOCKET_ENV, driver.sock_path());
                std::env::set_var("RUST_TEST_THREADS", "1");
                Some(driver)
            } else {
                None
            }
        } else {
            None
        };

        // Build if needed.
        if !skip_build {
            build(
                cfg_override,
                no_idl,
                None,
                None,
                false,
                skip_lint,
                true,
                program_name.clone(),
                None,
                None,
                BootstrapMode::None,
                None,
                None,
                env_vars,
                cargo_args,
                false,
            )?;
        }

        cfg.add_test_config(workspace_root, test_paths)?;

        // Deploy to the cluster unless told to skip. For localnet, preserve
        // explicit `--skip-local-validator` deploys because the validator is
        // already running, but don't let generated in-process templates force
        // an RPC deploy through their persisted config.
        let is_localnet = cfg.provider.cluster == Cluster::Localnet;
        let validator_plan = test_validator_plan(
            skip_deploy,
            is_localnet,
            cli_skip_local_validator,
            config_skip_local_validator,
        );
        if validator_plan.predeploy {
            deploy(cfg_override, None, None, false, true, vec![])?;
        }

        cfg.run_hooks(HookType::PreTest)?;

        let mut is_first_suite = true;
        let script_name_to_use = script_name.as_deref().unwrap_or("test");
        if let Some(test_script) = cfg.scripts.get_mut(script_name_to_use) {
            is_first_suite = false;

            match program_name {
                Some(program_name) => {
                    if let Some((from, to)) = Regex::new("\\s(tests/\\S+\\.(js|ts))")
                        .unwrap()
                        .captures_iter(&test_script.clone())
                        .last()
                        .and_then(|c| c.get(1).zip(c.get(2)))
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
                    "\nFound a '{}' script in the Anchor.toml. Running it as a test suite!",
                    script_name_to_use
                ),
            }

            run_test_suite(
                cfg,
                cfg.path(),
                is_localnet,
                validator_plan.skip_local_validator,
                skip_deploy,
                detach,
                validator_type,
                &cfg.test_validator,
                &cfg.scripts,
                script_name_to_use,
                validator_plan.stream_program_logs,
                &extra_args,
                &cfg.surfpool_config,
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
                    validator_plan.skip_local_validator,
                    skip_deploy,
                    detach,
                    validator_type,
                    &test_suite.1.test,
                    &test_suite.1.scripts,
                    script_name_to_use,
                    validator_plan.stream_program_logs,
                    &extra_args,
                    &cfg.surfpool_config,
                )?;
            }
        }
        cfg.run_hooks(HookType::PostTest)?;

        #[cfg(not(windows))]
        if profile {
            render_profile(cfg, &profile_dir)?;
        }

        Ok(())
    })?
}

fn should_predeploy_before_test(
    skip_deploy: bool,
    is_localnet: bool,
    cli_skip_local_validator: bool,
) -> bool {
    !skip_deploy && (!is_localnet || cli_skip_local_validator)
}

#[derive(Debug, PartialEq, Eq)]
struct TestValidatorPlan {
    skip_local_validator: bool,
    predeploy: bool,
    stream_program_logs: bool,
}

fn test_validator_plan(
    skip_deploy: bool,
    is_localnet: bool,
    cli_skip_local_validator: bool,
    config_skip_local_validator: bool,
) -> TestValidatorPlan {
    TestValidatorPlan {
        skip_local_validator: cli_skip_local_validator || config_skip_local_validator,
        predeploy: should_predeploy_before_test(skip_deploy, is_localnet, cli_skip_local_validator),
        stream_program_logs: true,
    }
}

/// Run the test suite with profile tracing enabled and then launch the SBF instruction stepper.
#[cfg(not(windows))]
#[allow(clippy::too_many_arguments)]
fn debugger(
    cfg_override: &ConfigOverride,
    test_name: Option<String>,
    skip_run: bool,
    skip_build: bool,
    skip_lint: bool,
    gdb: bool,
    cargo_args: Vec<String>,
) -> Result<()> {
    let has_anchor_toml = match Config::discover(cfg_override) {
        Ok(Some(_)) => true,
        Ok(None) => false,
        Err(e) => return Err(anyhow!("failed to probe for Anchor.toml: {e}")),
    };

    if has_anchor_toml {
        debugger_anchor_workspace(
            cfg_override,
            test_name,
            skip_run,
            skip_build,
            skip_lint,
            gdb,
            cargo_args,
        )
    } else {
        debugger_loose(
            cfg_override,
            test_name,
            skip_run,
            skip_build,
            gdb,
            cargo_args,
        )
    }
}

#[cfg(not(windows))]
#[allow(clippy::too_many_arguments)]
fn debugger_anchor_workspace(
    cfg_override: &ConfigOverride,
    test_name: Option<String>,
    skip_run: bool,
    skip_build: bool,
    skip_lint: bool,
    gdb: bool,
    cargo_args: Vec<String>,
) -> Result<()> {
    if !skip_run {
        test(
            cfg_override,
            None,
            true,
            true,
            skip_build,
            skip_lint,
            true,
            false,
            Vec::new(),
            None, // script_name — debugger drives test execution itself
            ValidatorType::Surfpool,
            true,
            gdb,
            Vec::new(),
            Vec::new(),
            cargo_args,
        )?;
    }

    with_workspace(cfg_override, |cfg| -> Result<()> {
        let workspace_root = cfg.path().parent().unwrap().to_owned();
        let profile_dir = workspace_root.join(crate::profile::DEFAULT_PROFILE_DIR);
        let (pubkey_to_so, sources) = resolve_anchor_workspace_programs(cfg);

        if pubkey_to_so.is_empty() {
            return Err(anyhow!(
                "no programs resolved for the debugger.\n\nEither declare them in Anchor.toml:\n  \
                 [programs.localnet]\n  <name> = \"<pubkey>\"\n\nor run `anchor build` so \
                 `target/deploy/<name>-keypair.json` exists."
            ));
        }

        println!("\nResolved programs:");
        for (pk, so) in &pubkey_to_so {
            let src = sources.get(pk).copied().unwrap_or("unknown");
            println!("  {pk}  ->  {}  [{src}]", display_path_relative_to_cwd(so));
        }

        debugger::run(
            &profile_dir,
            &pubkey_to_so,
            Some(&workspace_root),
            None,
            test_name.as_deref(),
        )
    })?
}

#[cfg(not(windows))]
#[allow(clippy::too_many_arguments)]
fn debugger_loose(
    _cfg_override: &ConfigOverride,
    test_name: Option<String>,
    skip_run: bool,
    skip_build: bool,
    gdb: bool,
    cargo_args: Vec<String>,
) -> Result<()> {
    let cwd = std::env::current_dir().context("read current directory")?;
    let ws = debugger::loose::LooseWorkspace::discover(cwd)?;

    if !skip_run {
        ws.check_dev_dep()?;
    }
    let profile_feature = ws.detect_profile_feature()?;
    let profile_dir = ws.root.join(debugger::loose_profile_dir_name());

    if !skip_run {
        debugger::loose::clear_profile_dir(&profile_dir)?;
        std::env::set_var("CARGO_PROFILE_RELEASE_DEBUG", "2");

        let anchor_exe =
            std::env::current_exe().context("resolve anchor binary path for RUSTC_WRAPPER")?;
        std::env::set_var("RUSTC_WRAPPER", &anchor_exe);
        std::env::set_var(debugger::rustc_wrapper::WRAPPER_SENTINEL, "1");

        if !skip_build {
            let build_cwd = ws.cargo_invocation_dir();
            eprintln!("running `cargo build-sbf` from {}", build_cwd.display());
            cargo_build_sbf(Some(build_cwd), &cargo_args)?;
        }

        std::env::remove_var("RUSTC_WRAPPER");
        std::env::remove_var(debugger::rustc_wrapper::WRAPPER_SENTINEL);

        eprintln!(
            "running `cargo test{gdb} --features {profile_feature}{pkg}{filter}` from {dir}",
            gdb = if gdb { " [gdb mode]" } else { "" },
            pkg = ws
                .current_package
                .as_deref()
                .map(|p| format!(" -p {p}"))
                .unwrap_or_default(),
            filter = test_name
                .as_deref()
                .map(|f| format!(" -- {f}"))
                .unwrap_or_default(),
            dir = ws.cargo_invocation_dir().display(),
        );
        if gdb {
            debugger::gdb::run_gdb_mode(
                ws.cargo_invocation_dir(),
                ws.current_package.as_deref(),
                &profile_feature,
                &profile_dir,
                test_name.as_deref(),
            )?;
        } else {
            debugger::loose::run_cargo_test(
                ws.cargo_invocation_dir(),
                ws.current_package.as_deref(),
                &profile_feature,
                &profile_dir,
                test_name.as_deref(),
            )?;
        }
    }

    let pubkey_to_so = debugger::loose::discover_programs(&ws.root, ws.current_package.as_deref())?;
    if pubkey_to_so.is_empty() {
        eprintln!(
            "warning: no programs found under {}/target/deploy/.\nELFs are required for \
             source/disasm symbolication. The debugger will still open but the static disasm pane \
             will be empty.",
            ws.root.display()
        );
    }

    if !profile_dir.exists() {
        return Err(anyhow!(
            "no traces produced at {}.\n\nDid the test actually run? Check that:\n- the test \
             calls `anchor_v2_testing::svm()` (NOT `LiteSVM::new()`)\n- the `{profile_feature}` \
             feature is enabled in the test build\n- the test sent at least one transaction that \
             hit a BPF program",
            profile_dir.display()
        ));
    }

    debugger::run(
        &profile_dir,
        &pubkey_to_so,
        Some(&ws.root),
        Some(&ws.cwd),
        test_name.as_deref(),
    )
}

#[cfg(not(windows))]
fn run_coverage(
    _cfg_override: &ConfigOverride,
    skip_run: bool,
    skip_build: bool,
    output: &str,
    trace_dir: &str,
    cargo_args: Vec<String>,
) -> Result<()> {
    let cwd = std::env::current_dir().context("read current directory")?;
    let ws = debugger::loose::LooseWorkspace::discover(cwd)?;

    let trace_path = ws.root.join(trace_dir);
    let output_path = ws.root.join(output);

    if !skip_run {
        std::env::set_var("CARGO_PROFILE_RELEASE_DEBUG", "2");

        let anchor_exe =
            std::env::current_exe().context("resolve anchor binary path for RUSTC_WRAPPER")?;
        std::env::set_var("RUSTC_WRAPPER", &anchor_exe);
        std::env::set_var(debugger::rustc_wrapper::WRAPPER_SENTINEL, "1");

        if !skip_build {
            let build_cwd = ws.cargo_invocation_dir();
            eprintln!("building programs with DWARF...");
            cargo_build_sbf(Some(build_cwd), &cargo_args)?;
        }

        if trace_path.exists() {
            fs::remove_dir_all(&trace_path)?;
        }
        fs::create_dir_all(&trace_path)?;

        let profile_feature = ws.detect_profile_feature().ok();
        eprintln!("running tests with register tracing...");
        let mut cmd = std::process::Command::new("cargo");
        cmd.current_dir(ws.cargo_invocation_dir()).arg("test");
        if let Some(feature) = &profile_feature {
            cmd.env("ANCHOR_PROFILE_DIR", &trace_path)
                .arg("--features")
                .arg(feature);
        } else {
            cmd.env("SBF_TRACE_DIR", &trace_path);
        }
        if let Some(pkg) = &ws.current_package {
            cmd.arg("-p").arg(pkg);
        }
        let status = cmd.status().context("spawn cargo test")?;
        if !status.success() {
            return Err(anyhow!("cargo test failed"));
        }
    }

    if !trace_path.exists() {
        return Err(anyhow!(
            "no traces at {}. Run without --skip-run first.",
            trace_path.display()
        ));
    }

    let programs = debugger::loose::discover_programs(&ws.root, ws.current_package.as_deref())?;
    if programs.is_empty() {
        return Err(anyhow!(
            "no programs found. Ensure declare_id!() is present in source.",
        ));
    }

    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent)?;
    }
    coverage::generate_lcov(&trace_path, &programs, Some(&ws.root), &output_path)
}

#[cfg(not(windows))]
fn display_path_relative_to_cwd(p: &Path) -> String {
    std::env::current_dir()
        .ok()
        .as_deref()
        .and_then(|c| p.strip_prefix(c).ok())
        .map(|rel| rel.display().to_string())
        .unwrap_or_else(|| p.display().to_string())
}

#[cfg(not(windows))]
fn resolve_anchor_workspace_programs(
    cfg: &WithPath<Config>,
) -> (BTreeMap<String, PathBuf>, BTreeMap<String, &'static str>) {
    let workspace_root = cfg.path().parent().unwrap();
    let deploy_dir = workspace_root.join("target").join("deploy");
    let mut pubkey_to_so: BTreeMap<String, PathBuf> = BTreeMap::new();
    let mut sources: BTreeMap<String, &'static str> = BTreeMap::new();
    for programs in cfg.programs.values() {
        for (name, deployment) in programs {
            let pk = deployment.address.to_string();
            pubkey_to_so.insert(pk.clone(), deploy_dir.join(format!("{name}.so")));
            sources.insert(pk, "Anchor.toml");
        }
    }
    if let Ok(discovered) = debugger::loose::discover_programs(workspace_root, None) {
        for (pk, so) in discovered {
            if !pubkey_to_so.contains_key(&pk) {
                pubkey_to_so.insert(pk.clone(), so);
                sources.insert(pk, "target/deploy");
            }
        }
    }
    (pubkey_to_so, sources)
}

#[cfg(not(windows))]
fn render_profile(cfg: &WithPath<Config>, profile_dir: &Path) -> Result<()> {
    let workspace_root = cfg.path().parent().unwrap().to_owned();
    let (pubkey_to_so, _sources) = resolve_anchor_workspace_programs(cfg);

    let rendered = profile::render_all_tests(profile_dir, Some(&workspace_root), &pubkey_to_so)
        .context("failed to render flamegraphs from trace directory")?;

    if rendered.is_empty() {
        eprintln!(
            "warning: no per-test trace directories found under {}. Did your tests call \
             `anchor_v2_testing::svm()` with the `profile` feature?",
            profile_dir.display()
        );
        return Ok(());
    }

    let mut sorted: Vec<&profile::RenderedTest> = rendered.iter().collect();
    sorted.sort_by(|a, b| a.test_name.cmp(&b.test_name));

    let max_name = sorted
        .iter()
        .filter(|t| t.svg_paths.len() == 1)
        .map(|t| t.test_name.len())
        .max()
        .unwrap_or(0);

    println!("\nFlamegraphs:");
    for test in &sorted {
        if test.svg_paths.len() == 1 {
            println!(
                "  {:<width$}  ->  {}",
                test.test_name,
                display_path_relative_to_cwd(&test.svg_paths[0]),
                width = max_name,
            );
        } else {
            println!("  {}", test.test_name);
            for (i, svg) in test.svg_paths.iter().enumerate() {
                println!("    tx{}  ->  {}", i + 1, display_path_relative_to_cwd(svg));
            }
        }
    }

    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn run_test_suite(
    cfg: &WithPath<Config>,
    test_suite_path: impl AsRef<Path>,
    is_localnet: bool,
    skip_local_validator: bool,
    skip_deploy: bool,
    detach: bool,
    validator_type: ValidatorType,
    test_validator: &Option<TestValidator>,
    scripts: &ScriptsConfig,
    script_name: &str,
    stream_program_logs: bool,
    extra_args: &[String],
    surfpool_config: &Option<SurfpoolConfig>,
) -> Result<()> {
    println!("\nRunning test suite: {:#?}\n", test_suite_path.as_ref());
    let mut validator_handle = None;
    if is_localnet && !skip_local_validator {
        let generated_accounts = generated_validator_accounts(cfg, test_validator)?;
        match validator_type {
            ValidatorType::Surfpool => {
                let full_simnet_mode = false;
                let flags = Some(surfpool_flags(
                    cfg,
                    surfpool_config,
                    full_simnet_mode,
                    skip_deploy,
                    Some(test_suite_path.as_ref()),
                    &generated_accounts,
                )?);
                validator_handle = Some(start_surfpool_validator(
                    flags,
                    surfpool_config,
                    full_simnet_mode,
                )?);
            }
            ValidatorType::Legacy => {
                let flags = Some(validator_flags(
                    cfg,
                    test_validator,
                    skip_deploy,
                    &generated_accounts,
                )?);
                validator_handle = Some(start_solana_test_validator(
                    cfg,
                    test_validator,
                    flags,
                    true,
                )?);
            }
        }
    }
    let url = cluster_url(cfg, test_validator, surfpool_config);

    let node_options = format!(
        "{} {}",
        match std::env::var_os("NODE_OPTIONS") {
            Some(value) => value
                .into_string()
                .map_err(std::env::VarError::NotUnicode)?,
            None => "".to_owned(),
        },
        get_node_dns_option(),
    );

    // Setup log reader - kept alive until end of scope
    let log_streams = if stream_program_logs {
        match stream_logs(cfg, &url) {
            Ok(streams) => Some(streams),
            Err(e) => {
                eprintln!("Warning: Failed to setup program log streaming: {:#}", e);
                eprintln!("Program logs will still be visible in the test output.");
                None
            }
        }
    } else {
        None
    };

    // Run the tests.
    let test_result = {
        let Some(cmd) = scripts.get(script_name) else {
            bail!("Not able to find script for `{}`", script_name);
        };
        let cmd = cmd.clone();
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

    // Explicitly shutdown log streams - closes WebSocket subscriptions
    if let Some(log_streams) = log_streams {
        for handle in log_streams {
            handle.shutdown();
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

// Returns the solana-test-validator flags. When `skip_deploy` is false, this
// embeds the workspace programs in the genesis block so we don't have to deploy
// every time. It also allows control of other solana-test-validator features.
fn validator_flags(
    cfg: &WithPath<Config>,
    test_validator: &Option<TestValidator>,
    skip_deploy: bool,
    generated_accounts: &[GeneratedAccount],
) -> Result<Vec<String>> {
    let mut flags = match skip_deploy {
        true => Vec::new(),
        false => validator_deploy_flags(cfg, test_validator)?,
    };
    for acct in generated_accounts {
        flags.push("--account".to_string());
        flags.push(acct.pubkey.to_string());
        flags.push(acct.file_path.display().to_string());
    }
    flags.extend(validator_config_flags(test_validator)?);
    Ok(flags)
}

/// Returns the rent-exempt minimum for the given account size.
fn rent_exempt_minimum(data_len: u64) -> u64 {
    (128 + data_len) * 3480 * 2
}

const RENT_EPOCH_NEVER: u64 = u64::MAX;

/// Writes a keypair file with restrictive permissions.
fn write_keypair_secure(keypair: &Keypair, path: &Path) -> Result<()> {
    use std::io::Write;
    let bytes = keypair.to_bytes().to_vec();
    let json = serde_json::to_string(&bytes)
        .with_context(|| format!("Failed to serialize keypair for {}", path.display()))?;

    let mut opts = std::fs::OpenOptions::new();
    opts.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        opts.mode(0o600);
    }
    let mut file = opts
        .open(path)
        .with_context(|| format!("Failed to create keypair file: {}", path.display()))?;
    file.write_all(json.as_bytes())
        .with_context(|| format!("Failed to write keypair file: {}", path.display()))?;
    Ok(())
}

/// Packs a `COption<Pubkey>` into the canonical on-chain byte layout.
fn pack_coption_pubkey(buf: &mut Vec<u8>, value: Option<Pubkey>) {
    match value {
        Some(pk) => {
            buf.extend_from_slice(&1u32.to_le_bytes());
            buf.extend_from_slice(pk.as_ref());
        }
        None => {
            buf.extend_from_slice(&0u32.to_le_bytes());
            buf.extend_from_slice(&[0u8; 32]);
        }
    }
}

/// Writes a validator account JSON fixture to disk.
fn write_account_json(path: &Path, value: &JsonValue) -> Result<()> {
    let mut file = File::create(path)
        .with_context(|| format!("Failed to create account file: {}", path.display()))?;
    serde_json::to_writer_pretty(&mut file, value)
        .with_context(|| format!("Failed to write account JSON to: {}", path.display()))?;
    Ok(())
}

/// Returns whether a config field requests a freshly generated address.
fn is_new_address(address: &str) -> bool {
    address.eq_ignore_ascii_case("new")
}

#[derive(Debug, Clone)]
struct GeneratedAccount {
    pubkey: Pubkey,
    file_path: PathBuf,
    surfpool_snapshot_value: JsonValue,
}

/// Resolves a config-relative path against the workspace root.
fn resolve_workspace_path(cfg: &WithPath<Config>, path: &str) -> Result<PathBuf> {
    let workspace_root = cfg
        .path()
        .parent()
        .ok_or_else(|| anyhow!("Anchor.toml path has no parent directory"))?;
    let candidate = Path::new(path);
    Ok(if candidate.is_relative() {
        workspace_root.join(candidate)
    } else {
        candidate.to_path_buf()
    })
}

/// Collects pubkeys declared by `account_dir` JSON fixtures.
fn account_dir_pubkeys(cfg: &WithPath<Config>, validator: &Validator) -> Result<HashSet<Pubkey>> {
    let mut pubkeys = HashSet::new();
    for account_dir in validator.account_dir.iter().flatten() {
        let directory = resolve_workspace_path(cfg, &account_dir.directory)?;
        for entry in fs::read_dir(&directory)
            .with_context(|| format!("Failed to read account directory: {}", directory.display()))?
        {
            let path = entry?.path();
            if path.extension().and_then(|ext| ext.to_str()) != Some("json") {
                continue;
            }

            let fixture: JsonValue =
                serde_json::from_reader(File::open(&path).with_context(|| {
                    format!("Failed to open account fixture: {}", path.display())
                })?)
                .with_context(|| format!("Failed to parse account fixture: {}", path.display()))?;

            let Some(pubkey) = fixture.get("pubkey").and_then(JsonValue::as_str) else {
                continue;
            };
            pubkeys.insert(
                Pubkey::try_from(pubkey)
                    .map_err(|_| anyhow!("Invalid pubkey {} in {}", pubkey, path.display()))?,
            );
        }
    }
    Ok(pubkeys)
}

/// Collects the pubkeys the validator will preload before token-account materialization.
fn validator_supplied_account_pubkeys(
    cfg: &WithPath<Config>,
    validator: &Validator,
    created_mints: &[Pubkey],
) -> Result<HashSet<Pubkey>> {
    let mut pubkeys = created_mints.iter().copied().collect::<HashSet<_>>();

    if let Some(accounts) = &validator.account {
        for account in accounts {
            pubkeys.insert(
                Pubkey::try_from(account.address.as_str())
                    .map_err(|_| anyhow!("Invalid account pubkey: {}", account.address))?,
            );
        }
    }

    if let Some(clones) = &validator.clone {
        for clone in clones {
            pubkeys.insert(
                Pubkey::try_from(clone.address.as_str())
                    .map_err(|_| anyhow!("Invalid clone pubkey: {}", clone.address))?,
            );
        }
    }

    pubkeys.extend(account_dir_pubkeys(cfg, validator)?);
    Ok(pubkeys)
}

/// Materializes generated validator accounts for use by both legacy validator flags and Surfpool.
fn materialize_validator_accounts(
    cfg: &WithPath<Config>,
    validator: &Validator,
) -> Result<Vec<GeneratedAccount>> {
    let mut out = Vec::new();
    let needs_dir = validator.mints.is_some()
        || validator.token_accounts.is_some()
        || validator.fund_accounts.is_some();
    if !needs_dir {
        return Ok(out);
    }

    let workspace_root = cfg
        .path()
        .parent()
        .ok_or_else(|| anyhow!("Anchor.toml path has no parent directory"))?;
    let accounts_dir = workspace_root.join(".anchor").join("generated_accounts");
    fs::create_dir_all(&accounts_dir).with_context(|| {
        format!(
            "Failed to create accounts directory: {}",
            accounts_dir.display()
        )
    })?;

    let mut seen_pubkeys: HashSet<Pubkey> = HashSet::new();
    let mut record_pubkey = |pk: Pubkey, section: &str| -> Result<()> {
        if !seen_pubkeys.insert(pk) {
            bail!(
                "Duplicate pubkey {} across [test.validator] sections (collision detected in \
                 `{}`). Each generated account must have a unique address.",
                pk,
                section
            );
        }
        Ok(())
    };

    let mut created_mints: Vec<Pubkey> = Vec::new();

    if let Some(mints) = &validator.mints {
        for token_mint in mints {
            let pubkey = if is_new_address(&token_mint.address) {
                let keypair = Keypair::new();
                let pubkey = keypair.pubkey();
                let keypair_path = accounts_dir.join(format!("{}.mint.json", pubkey));
                write_keypair_secure(&keypair, &keypair_path)?;
                pubkey
            } else {
                Pubkey::try_from(token_mint.address.as_str())
                    .map_err(|_| anyhow!("Invalid mint pubkey address: {}", token_mint.address))?
            };
            record_pubkey(pubkey, "mints")?;
            created_mints.push(pubkey);

            let parse_authority = |opt: &Option<String>, field: &str| -> Result<Option<Pubkey>> {
                opt.as_ref()
                    .map(|s| {
                        Pubkey::try_from(s.as_str()).map_err(|_| {
                            anyhow!("Invalid {} pubkey for mint {}: {}", field, pubkey, s)
                        })
                    })
                    .transpose()
            };
            let mint_authority = parse_authority(&token_mint.mint_authority, "mint_authority")?;
            let freeze_authority =
                parse_authority(&token_mint.freeze_authority, "freeze_authority")?;

            let mut data = Vec::with_capacity(82);
            pack_coption_pubkey(&mut data, mint_authority);
            data.extend_from_slice(&token_mint.supply.unwrap_or(0).to_le_bytes());
            data.push(token_mint.decimals);
            data.push(1u8); // is_initialized
            pack_coption_pubkey(&mut data, freeze_authority);

            let account_json = json!({
                "pubkey": pubkey.to_string(),
                "account": {
                    "lamports": rent_exempt_minimum(82),
                    "owner": "TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA",
                    "executable": false,
                    "rentEpoch": RENT_EPOCH_NEVER,
                    "data": [STANDARD.encode(&data), "base64"]
                }
            });
            let file_path = accounts_dir.join(format!("{}.json", pubkey));
            write_account_json(&file_path, &account_json)?;
            out.push(GeneratedAccount {
                pubkey,
                file_path,
                surfpool_snapshot_value: json!({
                    "lamports": rent_exempt_minimum(82),
                    "owner": "TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA",
                    "executable": false,
                    "rentEpoch": RENT_EPOCH_NEVER,
                    "data": STANDARD.encode(&data),
                    "parsedData": JsonValue::Null,
                }),
            });
        }
    }

    if let Some(token_accounts) = &validator.token_accounts {
        let validator_supplied_pubkeys =
            validator_supplied_account_pubkeys(cfg, validator, &created_mints)?;
        for token_account in token_accounts {
            let mint_pubkey = if is_new_address(&token_account.mint) {
                *created_mints.last().ok_or_else(|| {
                    anyhow!(
                        "token_account specifies `mint = \"new\"` but no [[test.validator.mints]] \
                         entries are configured"
                    )
                })?
            } else {
                let mint_pubkey = Pubkey::try_from(token_account.mint.as_str()).map_err(|_| {
                    anyhow!(
                        "Invalid mint pubkey in token_account: {}",
                        token_account.mint
                    )
                })?;
                if !validator_supplied_pubkeys.contains(&mint_pubkey) {
                    bail!(
                        "token_account mint {} is not loaded by the validator. Add it via \
                         [[test.validator.mints]], [[test.validator.clone]], \
                         [[test.validator.account]], or [[test.validator.account_dir]].",
                        mint_pubkey
                    );
                }
                mint_pubkey
            };

            let owner_pubkey = if is_new_address(&token_account.owner) {
                let kp = Keypair::new();
                let pk = kp.pubkey();
                let owner_path = accounts_dir.join(format!("{}.owner.json", pk));
                write_keypair_secure(&kp, &owner_path)?;
                pk
            } else {
                Pubkey::try_from(token_account.owner.as_str()).map_err(|_| {
                    anyhow!(
                        "Invalid owner pubkey in token_account: {}",
                        token_account.owner
                    )
                })?
            };

            let token_account_pubkey = match &token_account.address {
                Some(addr) if !is_new_address(addr) => Pubkey::try_from(addr.as_str())
                    .map_err(|_| anyhow!("Invalid token_account address pubkey: {}", addr))?,
                _ => {
                    let kp = Keypair::new();
                    let pk = kp.pubkey();
                    let ta_path = accounts_dir.join(format!("{}.token_account.json", pk));
                    write_keypair_secure(&kp, &ta_path)?;
                    pk
                }
            };
            record_pubkey(token_account_pubkey, "token_accounts")?;

            let mut data = Vec::with_capacity(165);
            data.extend_from_slice(mint_pubkey.as_ref());
            data.extend_from_slice(owner_pubkey.as_ref());
            data.extend_from_slice(&token_account.amount.to_le_bytes());
            pack_coption_pubkey(&mut data, None); // delegate
            data.push(1u8); // state = initialized
            data.extend_from_slice(&0u32.to_le_bytes()); // is_native None tag
            data.extend_from_slice(&[0u8; 8]); // is_native body
            data.extend_from_slice(&0u64.to_le_bytes()); // delegated_amount
            pack_coption_pubkey(&mut data, None); // close_authority

            let account_json = json!({
                "pubkey": token_account_pubkey.to_string(),
                "account": {
                    "lamports": rent_exempt_minimum(165),
                    "owner": "TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA",
                    "executable": false,
                    "rentEpoch": RENT_EPOCH_NEVER,
                    "data": [STANDARD.encode(&data), "base64"]
                }
            });
            let file_path = accounts_dir.join(format!("{}.json", token_account_pubkey));
            write_account_json(&file_path, &account_json)?;
            out.push(GeneratedAccount {
                pubkey: token_account_pubkey,
                file_path,
                surfpool_snapshot_value: json!({
                    "lamports": rent_exempt_minimum(165),
                    "owner": "TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA",
                    "executable": false,
                    "rentEpoch": RENT_EPOCH_NEVER,
                    "data": STANDARD.encode(&data),
                    "parsedData": JsonValue::Null,
                }),
            });
        }
    }

    if let Some(fund_accounts) = &validator.fund_accounts {
        for funded_account in fund_accounts {
            let pubkey = if is_new_address(&funded_account.address) {
                let keypair = Keypair::new();
                let pubkey = keypair.pubkey();
                let keypair_path = accounts_dir.join(format!("{}.keypair.json", pubkey));
                write_keypair_secure(&keypair, &keypair_path)?;
                pubkey
            } else {
                Pubkey::try_from(funded_account.address.as_str())
                    .map_err(|_| anyhow!("Invalid pubkey address: {}", funded_account.address))?
            };
            record_pubkey(pubkey, "fund_accounts")?;

            let lamports = funded_account.lamports.unwrap_or(1_000_000_000);

            let account_json = json!({
                "pubkey": pubkey.to_string(),
                "account": {
                    "lamports": lamports,
                    "owner": "11111111111111111111111111111111",
                    "executable": false,
                    "rentEpoch": RENT_EPOCH_NEVER,
                    "data": ["", "base64"]
                }
            });
            let file_path = accounts_dir.join(format!("{}.json", pubkey));
            write_account_json(&file_path, &account_json)?;
            out.push(GeneratedAccount {
                pubkey,
                file_path,
                surfpool_snapshot_value: json!({
                    "lamports": lamports,
                    "owner": "11111111111111111111111111111111",
                    "executable": false,
                    "rentEpoch": RENT_EPOCH_NEVER,
                    "data": "",
                    "parsedData": JsonValue::Null,
                }),
            });
        }
    }

    Ok(out)
}

/// Computes the generated validator accounts once so all validator backends share them.
fn generated_validator_accounts(
    cfg: &WithPath<Config>,
    test_validator: &Option<TestValidator>,
) -> Result<Vec<GeneratedAccount>> {
    test_validator
        .as_ref()
        .and_then(|test| test.validator.as_ref())
        .map(|validator| materialize_validator_accounts(cfg, validator))
        .transpose()
        .map(|accounts| accounts.unwrap_or_default())
}

/// Writes a Surfpool snapshot file for generated validator accounts and returns its path.
fn write_surfpool_snapshot(
    cfg: &WithPath<Config>,
    generated_accounts: &[GeneratedAccount],
) -> Result<Option<PathBuf>> {
    if generated_accounts.is_empty() {
        return Ok(None);
    }

    let accounts_dir = resolve_workspace_path(cfg, ".anchor/generated_accounts")?;
    fs::create_dir_all(&accounts_dir).with_context(|| {
        format!(
            "Failed to create accounts directory for Surfpool snapshot: {}",
            accounts_dir.display()
        )
    })?;

    let snapshot_path = accounts_dir.join("surfpool.snapshot.json");
    let mut snapshot = Map::new();
    for account in generated_accounts {
        snapshot.insert(
            account.pubkey.to_string(),
            account.surfpool_snapshot_value.clone(),
        );
    }

    let mut file = File::create(&snapshot_path).with_context(|| {
        format!(
            "Failed to create Surfpool snapshot file: {}",
            snapshot_path.display()
        )
    })?;
    serde_json::to_writer_pretty(&mut file, &JsonValue::Object(snapshot)).with_context(|| {
        format!(
            "Failed to write Surfpool snapshot file: {}",
            snapshot_path.display()
        )
    })?;

    Ok(Some(snapshot_path))
}

fn validator_deploy_flags(
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
        let binary_path = program.binary_path(verifiable)?.display().to_string();
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
            let idl_out = target_dir()?
                .join("idl")
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
    }

    Ok(flags)
}

fn validator_config_flags(test_validator: &Option<TestValidator>) -> Result<Vec<String>> {
    let mut flags = Vec::new();

    if let Some(validator) = test_validator
        .as_ref()
        .and_then(|test| test.validator.as_ref())
    {
        let entries = serde_json::to_value(validator)?;
        for (key, value) in entries.as_object().unwrap() {
            if key == "ledger" {
                // Ledger flag is a special case as it is passed separately to the rest of
                // these validator flags.
                continue;
            };
            if key == "fund_accounts" || key == "mints" || key == "token_accounts" {
                continue;
            }
            if key == "extra_args" {
                for arg in value.as_array().unwrap() {
                    flags.push(arg.as_str().unwrap().to_string());
                }
                continue;
            }
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
                        "Validator url for Solana's JSON RPC should be provided in order to clone \
                         accounts from it"
                    ));
                };

                let pubkeys = value
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|entry| {
                        let address = entry["address"].as_str().unwrap();
                        Pubkey::try_from(address).map_err(|_| anyhow!("Invalid pubkey {}", address))
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
                        Pubkey::try_from(feature_flag)
                            .map_err(|_| anyhow!("Invalid pubkey (feature flag) {}", feature_flag))
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

    Ok(flags)
}

// Returns Surfpool flags.
// This flags will be passed to the Surfpool, it allows to configure the validator.
fn surfpool_flags(
    cfg: &WithPath<Config>,
    surfpool_config: &Option<SurfpoolConfig>,
    full_simnet_mode: bool,
    skip_deploy: bool,
    test_suite_path: Option<&Path>,
    generated_accounts: &[GeneratedAccount],
) -> Result<Vec<String>> {
    let programs = cfg.programs.get(&Cluster::Localnet);
    let mut flags = Vec::new();

    for mut program in cfg.read_all_programs()? {
        let address = programs
            .and_then(|m| m.get(&program.lib_name))
            .map(|deployment| Ok(deployment.address.to_string()))
            .unwrap_or_else(|| program.pubkey().map(|p| p.to_string()))?;
        if let Some(idl) = program.idl.as_mut() {
            // Creating the idl files
            idl.address = address;
            let idl_out = target_dir()?
                .join("idl")
                .join(&idl.metadata.name)
                .with_extension("json");
            write_idl(idl, OutFile::File(idl_out))?;
        }
    }

    if let Some(snapshot_path) = write_surfpool_snapshot(cfg, generated_accounts)? {
        flags.push("--snapshot".to_string());
        flags.push(snapshot_path.display().to_string());
    }

    if let Some(config) = &surfpool_config {
        if let Some(airdrop_addresses) = &config.airdrop_addresses {
            for address in airdrop_addresses {
                flags.push("--airdrop".to_string());
                flags.push(address.to_string());
            }
        }
        if let Some(datasource_rpc_url) = &config.datasource_rpc_url {
            flags.push("--rpc-url".to_string());
            flags.push(datasource_rpc_url.to_string());
        }

        let host = &config.host;
        flags.push("--host".to_string());
        flags.push(host.to_string());

        let rpc_port = &config.rpc_port;
        flags.push("--port".to_string());
        flags.push(rpc_port.to_string());

        if let Some(ws_port) = &config.ws_port {
            flags.push("--ws-port".to_string());
            flags.push(ws_port.to_string());
        }

        if let Some(manifest_file_path) = &config.manifest_file_path {
            flags.push("--manifest-file-path".to_string());
            flags.push(manifest_file_path.to_string());
        }

        if let Some(runbooks) = &config.runbooks {
            for runbook in runbooks {
                flags.push("--runbook".to_string());
                flags.push(runbook.to_string());
            }
        }

        if let Some(slot_time) = &config.slot_time {
            flags.push("--slot-time".to_string());
            flags.push(slot_time.to_string());
        }
    }

    let online = surfpool_config
        .as_ref()
        .and_then(|c| c.online)
        .unwrap_or(false);
    if !online {
        flags.push("--offline".to_string());
    }

    let block_production_mode = surfpool_config
        .as_ref()
        .and_then(|c| c.block_production_mode.clone())
        .unwrap_or("transaction".into());
    flags.push("--block-production-mode".to_string());
    flags.push(block_production_mode);

    flags.push("--log-level".to_string());
    flags.push(
        surfpool_config
            .as_ref()
            .and_then(|c| c.log_level.clone())
            .unwrap_or("none".into()),
    );

    if !full_simnet_mode {
        flags.push("--no-tui".to_string());
        flags.push("--disable-instruction-profiling".to_string());
        flags.push("--max-profiles".to_string());
        flags.push("1".to_string());
        flags.push("--no-studio".to_string());
    }

    match skip_deploy {
        true => flags.push("--no-deploy".to_string()),
        false => {
            // automatically generate in-memory runbooks
            flags.push("--legacy-anchor-compatibility".to_string());
            if let Some(test_suite_path) = test_suite_path {
                flags.push("--anchor-test-config-path".to_string());
                flags.push(test_suite_path.display().to_string());
            }
        }
    }

    Ok(flags)
}

/// Handle for a log streaming thread.
///
/// Manages a WebSocket subscription and its associated receiver thread.
/// Call `shutdown()` to cleanly stop the thread.
struct LogStreamHandle {
    subscription: PubsubClientSubscription<RpcResponse<RpcLogsResponse>>,
}

impl LogStreamHandle {
    /// Explicitly shutdown the log stream
    fn shutdown(self) {
        // Send unsubscribe in a background thread to avoid blocking
        // PubsubClientSubscription::send_unsubscribe() can block indefinitely if WebSocket is stuck
        // The receiver threads will exit when the subscription closes
        std::thread::spawn(move || {
            let _ = self.subscription.send_unsubscribe();
        });
    }
}

/// Spawns a thread to receive logs from a subscription and write them to a file
fn spawn_log_receiver_thread<R>(receiver: R, log_file_path: PathBuf)
where
    R: IntoIterator<Item = RpcResponse<RpcLogsResponse>> + Send + 'static,
{
    std::thread::spawn(move || {
        if let Ok(mut file) = File::create(&log_file_path) {
            for response in receiver {
                let _ = writeln!(
                    file,
                    "Transaction executed in slot {}:",
                    response.context.slot
                );
                let _ = writeln!(file, "  Signature: {}", response.value.signature);
                let _ = writeln!(
                    file,
                    "  Status: {}",
                    response
                        .value
                        .err
                        .map(|err| err.to_string())
                        .unwrap_or_else(|| "Ok".to_string())
                );
                let _ = writeln!(file, "  Log Messages:");
                for log in response.value.logs {
                    let _ = writeln!(file, "    {}", log);
                }
                let _ = writeln!(file); // Empty line between transactions
                let _ = file.flush();
            }
        } else {
            eprintln!("Failed to create log file: {:?}", log_file_path);
        }
    });
}

fn stream_logs(config: &WithPath<Config>, rpc_url: &str) -> Result<Vec<LogStreamHandle>> {
    // Determine validator type to use appropriate logging
    match &config.validator {
        Some(ValidatorType::Surfpool) => {
            // For Surfpool, we don't need to stream logs via external commands
            // Surfpool handles its own logging to .surfpool/logs/ directory
            if config
                .surfpool_config
                .as_ref()
                .and_then(|s| {
                    s.log_level
                        .as_ref()
                        .map(|l| l.to_ascii_lowercase().ne("none"))
                })
                .unwrap_or(false)
            {
                println!("Surfpool validator logs: .surfpool/logs/ directory");
            }
            Ok(vec![])
        }
        Some(ValidatorType::Legacy) | None => stream_solana_logs(config, rpc_url),
    }
}

fn stream_solana_logs(config: &WithPath<Config>, rpc_url: &str) -> Result<Vec<LogStreamHandle>> {
    let program_logs_dir = Path::new(".anchor").join("program-logs");
    if program_logs_dir.exists() {
        fs::remove_dir_all(&program_logs_dir)?;
    }
    fs::create_dir_all(&program_logs_dir)?;

    // For solana-test-validator, the WebSocket port is RPC port + WEBSOCKET_PORT_OFFSET
    // Extract port from rpc_url and construct WebSocket URL
    let ws_url = if rpc_url.contains("127.0.0.1") || rpc_url.contains("localhost") {
        // Local validator: increment port by 1 for WebSocket
        let rpc_port = rpc_url
            .rsplit_once(':')
            .and_then(|(_, port)| port.parse::<u16>().ok())
            .unwrap_or(DEFAULT_RPC_PORT);

        let ws_port = rpc_port + WEBSOCKET_PORT_OFFSET;
        let url = format!("ws://127.0.0.1:{}", ws_port);
        url
    } else {
        // Remote cluster: use same URL but replace http(s) with ws(s)
        rpc_url
            .replace("https://", "wss://")
            .replace("http://", "ws://")
    };

    // Give the WebSocket endpoint a moment to be ready (especially for local validators)
    std::thread::sleep(std::time::Duration::from_millis(1500));

    let mut handles = vec![];

    // Subscribe to logs for all workspace programs
    for program in config.read_all_programs()? {
        let idl_path = target_dir()?
            .join("idl")
            .join(&program.lib_name)
            .with_extension("json");
        let idl = fs::read(&idl_path)?;
        let idl = convert_idl(&idl)?;

        let log_file_path =
            program_logs_dir.join(format!("{}.{}.log", idl.address, program.lib_name));
        let program_address = idl.address.clone();

        // Subscribe to logs using PubsubClient
        let (client, receiver) = match PubsubClient::logs_subscribe(
            &ws_url,
            RpcTransactionLogsFilter::Mentions(vec![program_address.clone()]),
            RpcTransactionLogsConfig {
                commitment: Some(CommitmentConfig::confirmed()),
            },
        ) {
            Ok(result) => result,
            Err(e) => {
                eprintln!(
                    "Warning: Failed to subscribe to logs for program {}: {}",
                    program.lib_name, e
                );
                continue;
            }
        };

        // Spawn thread to write logs to file
        spawn_log_receiver_thread(receiver, log_file_path);

        handles.push(LogStreamHandle {
            subscription: client,
        });
    }

    // Also subscribe to logs for genesis programs
    if let Some(test) = config.test_validator.as_ref() {
        if let Some(genesis) = &test.genesis {
            for entry in genesis {
                let log_file_path = program_logs_dir.join(&entry.address).with_extension("log");
                let address = entry.address.clone();

                // Subscribe to logs using PubsubClient
                let (client, receiver) = match PubsubClient::logs_subscribe(
                    &ws_url,
                    RpcTransactionLogsFilter::Mentions(vec![address.clone()]),
                    RpcTransactionLogsConfig {
                        commitment: Some(CommitmentConfig::confirmed()),
                    },
                ) {
                    Ok(result) => result,
                    Err(e) => {
                        eprintln!(
                            "Warning: Failed to subscribe to logs for genesis program {}: {}",
                            &entry.address, e
                        );
                        continue;
                    }
                };

                // Spawn thread to write logs to file
                spawn_log_receiver_thread(receiver, log_file_path);

                handles.push(LogStreamHandle {
                    subscription: client,
                });
            }
        }
    }

    Ok(handles)
}

fn start_surfpool_validator(
    flags: Option<Vec<String>>,
    surfpool_config: &Option<SurfpoolConfig>,
    full_simnet_mode: bool,
) -> Result<Child> {
    let (host, port) = match surfpool_config {
        Some(SurfpoolConfig { host, rpc_port, .. }) => (host.clone(), *rpc_port),
        _ => (SURFPOOL_HOST.to_string(), DEFAULT_RPC_PORT),
    };
    let rpc_url = surfpool_rpc_url(surfpool_config);

    if std::net::TcpStream::connect_timeout(
        &format!("{host}:{port}")
            .parse()
            .map_err(|err| anyhow!("invalid surfpool host:port `{host}:{port}`: {err}"))?,
        std::time::Duration::from_millis(200),
    )
    .is_ok()
    {
        return Err(anyhow!(
            "port {port} on {host} is already in use - another validator is running there. Kill \
             it or set `[surfpool] rpc_port = N` in Anchor.toml to pick a free port."
        ));
    }

    let test_validator_stdout = match full_simnet_mode {
        true => Stdio::inherit(),
        false => Stdio::null(),
    };

    let mut validator_handle = std::process::Command::new("surfpool")
        .arg("start")
        .args(flags.unwrap_or_default())
        .stdout(test_validator_stdout)
        .stderr(Stdio::inherit())
        .spawn()
        .map_err(|e| anyhow!("Failed to spawn `surfpool`: {e}"))?;

    let client = create_client(rpc_url.clone());

    let mut count = 0;

    let ms_wait = surfpool_config
        .as_ref()
        .map(|surfpool| surfpool.startup_wait)
        .unwrap_or(STARTUP_WAIT);

    while count < ms_wait {
        if let Ok(Some(status)) = validator_handle.try_wait() {
            return Err(anyhow!(
                "`surfpool` exited during startup with {status} - see the stderr output above. \
                 Common causes: port {port} in use, missing deploy artifacts in `target/deploy/`, \
                 invalid Anchor.toml config."
            ));
        }
        let r = client.get_latest_blockhash();
        if r.is_ok() {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(100));
        count += 100;
    }

    if count >= ms_wait {
        eprintln!(
            "Unable to get latest blockhash. Surfpool validator does not look started. Check \
             .surfpool/logs/ directory for errors. Consider increasing [surfpool.startup_wait] in \
             Anchor.toml."
        );
        validator_handle.kill()?;
        std::process::exit(1);
    }

    loop {
        let resp = client
            .send::<RpcResponse<SurfnetInfoResponse>>(
                RpcRequest::Custom {
                    method: "surfnet_getSurfnetInfo",
                },
                serde_json::Value::Null,
            )?
            .value;

        // break out if all runbooks are completed
        if resp
            .runbook_executions
            .iter()
            .all(|ex| ex.completed_at.is_some())
        {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(500));
    }
    Ok(validator_handle)
}

fn start_solana_test_validator(
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
            let test_validator_stdout_file =
                File::create(&test_ledger_log_filename).with_context(|| {
                    format!(
                        "Failed to create validator log file {}",
                        test_ledger_log_filename.display()
                    )
                })?;
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
        .unwrap_or(DEFAULT_RPC_PORT);
    if !portpicker::is_free(rpc_port) {
        return Err(anyhow!(
            "Your configured rpc port: {rpc_port} is already in use"
        ));
    }
    let faucet_port = cfg
        .test_validator
        .as_ref()
        .and_then(|test| test.validator.as_ref().and_then(|v| v.faucet_port))
        .unwrap_or(DEFAULT_FAUCET_PORT);
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
        let r = client.get_latest_blockhash();
        if r.is_ok() {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(100));
        count += 100;
    }
    if count >= ms_wait {
        eprintln!(
            "Unable to get latest blockhash. Test validator does not look started. Check \
             {test_ledger_log_filename:?} for errors. Consider increasing [test.startup_wait] in \
             Anchor.toml."
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

// Returns the URL that surfpool should be running for the given configuration
fn surfpool_rpc_url(surfpool_config: &Option<SurfpoolConfig>) -> String {
    match surfpool_config {
        Some(SurfpoolConfig { host, rpc_port, .. }) => format!("http://{}:{}", host, rpc_port),
        _ => format!("http://{}:{}", SURFPOOL_HOST, DEFAULT_RPC_PORT),
    }
}

// Setup and return paths to the solana-test-validator ledger directory and log
// files given the configuration
fn test_validator_file_paths(test_validator: &Option<TestValidator>) -> Result<(PathBuf, PathBuf)> {
    let ledger_path = match test_validator {
        Some(TestValidator {
            validator: Some(validator),
            ..
        }) => PathBuf::from(&validator.ledger),
        _ => get_default_ledger_path(),
    };

    if !ledger_path.is_relative() {
        // Prevent absolute paths to avoid someone using / or similar, as the
        // directory gets removed
        eprintln!("Ledger directory {ledger_path:?} must be relative");
        std::process::exit(1);
    }
    if ledger_path.exists() {
        fs::remove_dir_all(&ledger_path).with_context(|| {
            format!(
                "Failed to remove ledger directory {}",
                ledger_path.display()
            )
        })?;
    }

    fs::create_dir_all(&ledger_path).with_context(|| {
        format!(
            "Failed to create ledger directory {}",
            ledger_path.display()
        )
    })?;

    let log_path = ledger_path.join("test-ledger-log.txt");
    Ok((ledger_path, log_path))
}

pub(crate) fn cluster_url(
    cfg: &Config,
    test_validator: &Option<TestValidator>,
    surfpool_config: &Option<SurfpoolConfig>,
) -> String {
    let is_localnet = cfg.provider.cluster == Cluster::Localnet;
    match is_localnet {
        // Cluster is Localnet, determine which validator to use
        true => match &cfg.validator {
            Some(ValidatorType::Surfpool) => surfpool_rpc_url(surfpool_config),
            Some(ValidatorType::Legacy) | None => test_validator_rpc_url(test_validator),
        },
        false => cfg.provider.cluster.url().to_string(),
    }
}

fn clean(cfg_override: &ConfigOverride) -> Result<()> {
    // Get workspace root - either from Anchor.toml or use current directory
    let workspace_root = if let Ok(Some(cfg)) = Config::discover(cfg_override) {
        cfg.path()
            .parent()
            .expect("Invalid Anchor.toml")
            .to_path_buf()
    } else {
        // No Anchor.toml - use current directory for Cargo workspace
        std::env::current_dir()?
    };

    let dot_anchor_dir = workspace_root.join(".anchor");
    let target_dir = crate::target_dir()?;
    let deploy_dir = target_dir.join("deploy");

    if dot_anchor_dir.exists() {
        fs::remove_dir_all(&dot_anchor_dir)
            .map_err(|e| anyhow!("Could not remove directory {:?}: {}", dot_anchor_dir, e))?;
    }

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
    program_keypair: Option<PathBuf>,
    verifiable: bool,
    no_idl: bool,
    solana_args: Vec<String>,
) -> Result<()> {
    // Execute the code within the workspace
    with_workspace(cfg_override, |cfg| -> Result<()> {
        let url = cluster_url(cfg, &cfg.test_validator, &cfg.surfpool_config);
        let keypair = cfg.provider.wallet.to_string();

        cfg.run_hooks(HookType::PreDeploy)?;
        // Deploy the programs.
        println!("Deploying cluster: {url}");
        println!("Upgrade authority: {keypair}");

        for program in cfg.get_programs(program_name)? {
            let binary_path = program.binary_path(verifiable)?;

            println!("Deploying program {:?}...", program.lib_name);
            println!("Program path: {}...", binary_path.display());

            let program_keypair_filepath = match program_keypair.as_ref() {
                Some(path) => path.clone(),
                None => program.keypair_file()?.path().clone(),
            };

            // Deploy using our native implementation
            program::program_deploy(
                cfg_override,
                Some(strip_workspace_prefix(binary_path)),
                None, // program_name - not needed since we have filepath
                Some(strip_workspace_prefix(program_keypair_filepath)),
                None,  // upgrade_authority - uses wallet from config
                None,  // program_id - derived from program_keypair
                None,  // buffer
                None,  // max_len
                false, // use_rpc
                no_idl,
                false, // make_final
                solana_args.clone(),
            )?;
        }

        println!("Deploy success");
        cfg.run_hooks(HookType::PostDeploy)?;

        Ok(())
    })?
}

fn upgrade(
    cfg_override: &ConfigOverride,
    program_id: Pubkey,
    program_filepath: PathBuf,
    max_retries: u32,
    solana_args: Vec<String>,
) -> Result<()> {
    // Use our native upgrade implementation
    program::program_upgrade(
        cfg_override,
        program_id,
        Some(program_filepath),
        None, // program_name - not needed since we have filepath
        None, // buffer
        None, // upgrade_authority - uses wallet from config
        max_retries,
        false, // use_rpc
        solana_args,
    )
}

fn migrate(cfg_override: &ConfigOverride) -> Result<()> {
    with_workspace(cfg_override, |cfg| -> Result<()> {
        println!("Running migration deploy script");

        let url = cluster_url(cfg, &cfg.test_validator, &cfg.surfpool_config);
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
                template::deploy_ts_script_host(&url, &module_path.display().to_string());
            fs::write(deploy_ts, deploy_script_host_str)?;

            let pkg_manager_cmd =
                resolve_package_manager(cfg.toolchain.package_manager.clone())?.to_string();

            std::process::Command::new(pkg_manager_cmd)
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
                template::deploy_js_script_host(&url, &module_path.display().to_string());
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
    })?
}

fn set_workspace_dir_or_exit() {
    // First try to find Anchor workspace
    let d = match Config::discover(&ConfigOverride::default()) {
        Err(err) => {
            println!("Workspace configuration error: {err}");
            std::process::exit(1);
        }
        Ok(d) => d,
    };

    match d {
        None => {
            // No Anchor.toml found - check for Cargo workspace with Solana programs
            let current_dir = match std::env::current_dir() {
                Ok(dir) => dir,
                Err(_) => {
                    println!("Unable to determine current directory");
                    std::process::exit(1);
                }
            };

            let cargo_toml_path = current_dir.join("Cargo.toml");
            if !cargo_toml_path.exists() {
                println!(
                    "Not in a Solana workspace. This command requires either Anchor.toml or a \
                     Cargo workspace with Solana programs."
                );
                std::process::exit(1);
            }

            // Check if this is a workspace and has Solana programs
            match program::discover_solana_programs(None) {
                Ok(programs) if !programs.is_empty() => {
                    // Found Solana programs in Cargo workspace - stay in current directory
                    // (already in the right place)
                }
                _ => {
                    println!(
                        "Not in a Solana workspace. This command requires either Anchor.toml or a \
                         Cargo workspace with Solana programs."
                    );
                    std::process::exit(1);
                }
            }
        }
        Some(cfg) => {
            // Found Anchor.toml - change to workspace root
            match cfg.path().parent() {
                None => {
                    println!("Unable to make new program");
                }
                Some(parent) => {
                    if std::env::set_current_dir(parent).is_err() {
                        println!(
                            "Not in a Solana workspace. This command requires either Anchor.toml \
                             or a Cargo workspace with Solana programs."
                        );
                        std::process::exit(1);
                    }
                }
            };
        }
    }
}

fn airdrop(cfg_override: &ConfigOverride, amount: f64, pubkey: Option<Pubkey>) -> Result<()> {
    // Get cluster URL and wallet path
    let (cluster_url, wallet_path) = get_cluster_and_wallet(cfg_override)?;

    // Create RPC client with confirmed commitment
    let client = RpcClient::new_with_commitment(cluster_url, CommitmentConfig::confirmed());

    // Determine recipient
    let recipient_pubkey = if let Some(pubkey) = pubkey {
        pubkey
    } else {
        // Load keypair from wallet path and get pubkey
        let keypair = Keypair::read_from_file(&wallet_path)
            .map_err(|e| anyhow!("Failed to read keypair from {}: {}", wallet_path, e))?;
        keypair.pubkey()
    };

    // Convert SOL to lamports
    let lamports = (amount * 1_000_000_000.0) as u64;
    let starting_balance = client
        .get_balance_with_commitment(&recipient_pubkey, CommitmentConfig::confirmed())?
        .value;

    // Get recent blockhash for airdrop
    let recent_blockhash = client
        .get_latest_blockhash()
        .map_err(|e| anyhow!("Failed to get recent blockhash: {}", e))?;

    // Request airdrop with blockhash
    println!("Requesting airdrop of {} SOL...", amount);
    let signature = client
        .request_airdrop_with_blockhash(&recipient_pubkey, lamports, &recent_blockhash)
        .map_err(|e| anyhow!("Airdrop request failed: {}", e))?;

    println!("Signature: {}", signature);

    // Wait for confirmation with the same blockhash used for the airdrop
    client
        .confirm_transaction_with_spinner(&signature, &recent_blockhash, client.commitment())
        .map_err(|e| anyhow!("Transaction confirmation failed: {}", e))?;

    println!("Airdrop confirmed!");

    // Get and display the new balance
    let balance = wait_for_airdrop_balance(&client, &recipient_pubkey, starting_balance, lamports)?;
    println!("Balance: {}", format_sol(balance));

    Ok(())
}

fn wait_for_airdrop_balance(
    client: &RpcClient,
    recipient_pubkey: &Pubkey,
    starting_balance: u64,
    lamports: u64,
) -> Result<u64> {
    let expected_balance = starting_balance.saturating_add(lamports);
    let mut last_balance = starting_balance;

    for attempt in 0..10 {
        let balance = client
            .get_balance_with_commitment(recipient_pubkey, CommitmentConfig::confirmed())?
            .value;
        if balance >= expected_balance {
            return Ok(balance);
        }
        last_balance = balance;

        if attempt < 9 {
            std::thread::sleep(std::time::Duration::from_millis(500));
        }
    }

    eprintln!(
        "warning: confirmed balance has not reflected the airdrop yet; showing latest confirmed \
         balance"
    );
    Ok(last_balance)
}

fn cluster(_cmd: ClusterCommand) -> Result<()> {
    println!("Cluster Endpoints:\n");
    println!("* Mainnet - https://api.mainnet-beta.solana.com");
    println!("* Devnet  - https://api.devnet.solana.com");
    println!("* Testnet - https://api.testnet.solana.com");
    Ok(())
}

fn config_cmd(cfg_override: &ConfigOverride, cmd: ConfigCommand) -> Result<()> {
    match cmd {
        ConfigCommand::Get => config_get(cfg_override),
        ConfigCommand::Set { url, keypair } => config_set(cfg_override, url, keypair),
    }
}

fn config_get(cfg_override: &ConfigOverride) -> Result<()> {
    with_workspace(cfg_override, |cfg| -> Result<()> {
        println!("Anchor Configuration:");
        println!();
        println!("Cluster: {}", cfg.provider.cluster.url());
        println!("Wallet:  {}", cfg.provider.wallet);
        Ok(())
    })?
}

fn config_set(
    cfg_override: &ConfigOverride,
    url: Option<String>,
    keypair: Option<PathBuf>,
) -> Result<()> {
    // Find the Anchor.toml file
    let anchor_toml_path = match Config::discover(cfg_override)? {
        Some(cfg) => cfg.path().parent().unwrap().join("Anchor.toml"),
        None => bail!("Not in an Anchor workspace"),
    };

    // Read the current Anchor.toml
    let mut toml_content =
        fs::read_to_string(&anchor_toml_path).context("Failed to read Anchor.toml")?;
    let mut toml_doc: toml::Value =
        toml::from_str(&toml_content).context("Failed to parse Anchor.toml")?;

    let mut updated = false;

    // Update cluster URL if provided
    if let Some(cluster_url) = url {
        let expanded_url = match cluster_url.as_str() {
            "m" => "https://api.mainnet-beta.solana.com".to_string(),
            "d" => "https://api.devnet.solana.com".to_string(),
            "t" => "https://api.testnet.solana.com".to_string(),
            "l" => "http://127.0.0.1:8899".to_string(),
            _ => cluster_url,
        };

        if let Some(provider) = toml_doc.get_mut("provider").and_then(|v| v.as_table_mut()) {
            provider.insert(
                "cluster".to_string(),
                toml::Value::String(expanded_url.clone()),
            );
            println!("Updated cluster to: {}", expanded_url);
            updated = true;
        }
    }

    // Update wallet path if provided
    if let Some(keypair_path) = keypair {
        let expanded_path = shellexpand::tilde(&keypair_path.to_string_lossy()).to_string();

        // Check if the wallet file exists
        if !Path::new(&expanded_path).exists() {
            eprintln!("Warning: Wallet file does not exist: {}", expanded_path);
        }

        if let Some(provider) = toml_doc.get_mut("provider").and_then(|v| v.as_table_mut()) {
            provider.insert(
                "wallet".to_string(),
                toml::Value::String(expanded_path.clone()),
            );
            println!("Updated wallet to: {}", expanded_path);
            updated = true;
        }
    }

    if updated {
        // Write the updated config back to Anchor.toml
        toml_content =
            toml::to_string_pretty(&toml_doc).context("Failed to serialize Anchor.toml")?;
        fs::write(&anchor_toml_path, toml_content).context("Failed to write Anchor.toml")?;
        println!("\nConfiguration updated successfully!");
    } else {
        println!("No changes made. Use --url or --keypair to update settings.");
    }

    Ok(())
}

fn shell(cfg_override: &ConfigOverride) -> Result<()> {
    with_workspace(cfg_override, |cfg| -> Result<()> {
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
        let url = cluster_url(cfg, &cfg.test_validator, &cfg.surfpool_config);
        let js_code = template::node_shell(&url, &cfg.provider.wallet.to_string(), programs)?;
        let mut child = std::process::Command::new("node")
            .args(["-e", &js_code, "-i", "--experimental-repl-await"])
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit())
            .spawn()
            .map_err(|e| anyhow::format_err!("{}", e))?;

        if !child.wait()?.success() {
            println!("Error running node shell");
            return Ok(());
        }
        Ok(())
    })?
}

fn run(cfg_override: &ConfigOverride, script: String, script_args: Vec<String>) -> Result<()> {
    with_workspace(cfg_override, |cfg| -> Result<()> {
        let url = cluster_url(cfg, &cfg.test_validator, &cfg.surfpool_config);
        let script_cmd = cfg.scripts.get(&script).ok_or_else(|| {
            let mut available_scripts: Vec<String> = cfg.scripts.keys().cloned().collect();
            available_scripts.sort();
            if available_scripts.is_empty() {
                anyhow!("Script '{script}' not found. No scripts defined in Anchor.toml.")
            } else {
                anyhow!(
                    "Script '{script}' not found.\n\nAvailable scripts:\n  {}",
                    available_scripts.join("\n  ")
                )
            }
        })?;
        let script_with_args = format!("{script_cmd} {}", script_args.join(" "));
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
    })?
}

fn keys(cfg_override: &ConfigOverride, cmd: KeysCommand) -> Result<()> {
    match cmd {
        KeysCommand::List => keys_list(cfg_override),
        KeysCommand::Sync { program_name } => keys_sync(cfg_override, program_name),
    }
}

fn keys_list(cfg_override: &ConfigOverride) -> Result<()> {
    with_workspace(cfg_override, |cfg| -> Result<()> {
        for program in cfg.read_all_programs()? {
            let pubkey = program.pubkey()?;
            println!("{}: {}", program.lib_name, pubkey);
        }
        Ok(())
    })?
}

/// Sync program `declare_id!` pubkeys with the pubkey from `target/deploy/<KEYPAIR>.json`.
fn keys_sync(cfg_override: &ConfigOverride, program_name: Option<String>) -> Result<()> {
    with_workspace(cfg_override, |cfg| -> Result<()> {
        let declare_id_regex = RegexBuilder::new(r#"^(([\w]+::)*)declare_id!\("(\w*)"\)"#)
            .multi_line(true)
            .build()
            .unwrap();

        let cfg_cluster = cfg.provider.cluster.to_owned();
        println!("Syncing program ids for the configured cluster ({cfg_cluster})\n");

        let mut changed_src = false;
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

                    changed_src = true;
                    println!("Updated to {actual_program_id}\n");
                    break;
                }
            }

            // Handle declaration in Anchor.toml
            'outer: for (cluster, programs) in &mut cfg.programs {
                // Only change if the configured cluster matches the program's cluster
                if cluster != &cfg_cluster {
                    continue;
                }

                for (name, deployment) in programs {
                    // Skip other programs
                    if name != &program.lib_name {
                        continue;
                    }

                    if deployment.address.to_string() != actual_program_id {
                        println!(
                            "Found incorrect program id declaration in Anchor.toml for the \
                             program `{name}`"
                        );

                        // Update the program id
                        deployment.address = Pubkey::try_from(actual_program_id.as_str()).unwrap();
                        fs::write(cfg.path(), cfg.to_string())?;

                        println!("Updated to {actual_program_id}\n");
                        break 'outer;
                    }
                }
            }
        }

        println!("All program id declarations are synced.");
        if changed_src {
            println!("Please rebuild the program to update the generated artifacts.")
        }

        Ok(())
    })?
}

/// Check if there's a mismatch between the program keypair and the `declare_id!` in the source code.
/// Returns an error if a mismatch is detected, prompting the user to run `anchor keys sync`.
fn check_program_id_mismatch(cfg: &WithPath<Config>, program_name: Option<String>) -> Result<()> {
    let declare_id_regex = RegexBuilder::new(r#"^(([\w]+::)*)declare_id!\("(\w*)"\)"#)
        .multi_line(true)
        .build()
        .unwrap();

    for program in cfg.get_programs(program_name)? {
        // Get the pubkey from the keypair file
        let actual_program_id = program.pubkey()?.to_string();

        // Check declaration in program files
        let src_path = program.path.join("src");
        let files_to_check = vec![src_path.join("lib.rs"), src_path.join("id.rs")];

        for path in files_to_check {
            let content = match fs::read_to_string(&path) {
                Ok(content) => content,
                Err(_) => continue,
            };

            let incorrect_program_id = declare_id_regex
                .captures(&content)
                .and_then(|captures| captures.get(3))
                .filter(|program_id_match| program_id_match.as_str() != actual_program_id);

            if let Some(program_id_match) = incorrect_program_id {
                let declared_id = program_id_match.as_str();
                return Err(anyhow!(
                    "Program ID mismatch detected for program '{}':\n  Keypair file has: {}\n  \
                     Source code has:  {}\n\nPlease run 'anchor keys sync' to update the program \
                     ID in your source code or use the '--ignore-keys' flag to skip this check.",
                    program.lib_name,
                    actual_program_id,
                    declared_id
                ));
            }
        }
    }

    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn localnet(
    cfg_override: &ConfigOverride,
    skip_build: bool,
    skip_deploy: bool,
    skip_lint: bool,
    ignore_keys: bool,
    validator_type: ValidatorType,
    env_vars: Vec<String>,
    cargo_args: Vec<String>,
) -> Result<()> {
    with_workspace(cfg_override, |cfg| -> Result<()> {
        // Build if needed.
        if !skip_build {
            build(
                cfg_override,
                false,
                None,
                None,
                false,
                skip_lint,
                ignore_keys,
                None,
                None,
                None,
                BootstrapMode::None,
                None,
                None,
                env_vars,
                cargo_args,
                false,
            )?;
        }

        let generated_accounts = generated_validator_accounts(cfg, &cfg.test_validator)?;
        let validator_handle: Option<Child> = match validator_type {
            ValidatorType::Surfpool => {
                let full_simnet_mode = true;
                let flags = Some(surfpool_flags(
                    cfg,
                    &cfg.surfpool_config,
                    full_simnet_mode,
                    skip_deploy,
                    None,
                    &generated_accounts,
                )?);
                Some(start_surfpool_validator(
                    flags,
                    &cfg.surfpool_config,
                    full_simnet_mode,
                )?)
            }
            ValidatorType::Legacy => {
                let flags = Some(validator_flags(
                    cfg,
                    &cfg.test_validator,
                    skip_deploy,
                    &generated_accounts,
                )?);
                Some(start_solana_test_validator(
                    cfg,
                    &cfg.test_validator,
                    flags,
                    false,
                )?)
            }
        };

        // Setup log reader.
        let url = test_validator_rpc_url(&cfg.test_validator);
        let log_streams = match stream_logs(cfg, &url) {
            Ok(streams) => {
                println!(
                    "Log streams set up successfully ({} streams)",
                    streams.len()
                );
                Some(streams)
            }
            Err(e) => {
                eprintln!("Warning: Failed to setup program log streaming: {:#}", e);
                eprintln!("  Program logs will still be visible in the validator output.");
                None
            }
        };

        std::io::stdin().lock().lines().next().unwrap().unwrap();

        // Check all errors and shut down.
        if let Some(mut handle) = validator_handle {
            if let Err(err) = handle.kill() {
                println!("Failed to kill subprocess {}: {}", handle.id(), err);
            }
        }

        // Explicitly shutdown log streams - closes WebSocket subscriptions
        if let Some(log_streams) = log_streams {
            for handle in log_streams {
                handle.shutdown();
            }
        }

        Ok(())
    })?
}

/// Return the cargo build artifacts directory. The successful result is
/// cached.
pub fn target_dir() -> Result<&'static Path> {
    static TARGET_DIR: OnceLock<PathBuf> = OnceLock::new();
    if let Some(path) = TARGET_DIR.get() {
        return Ok(path.as_path());
    }
    let path = target_dir_no_cache()?;
    let _ = TARGET_DIR.set(path);
    Ok(TARGET_DIR.get().expect("just set").as_path())
}

/// Return the cargo build artifacts directory.
fn target_dir_no_cache() -> Result<PathBuf> {
    // `cargo metadata` produces a JSON blob from which we extract the
    // `target_directory` field.
    let output = std::process::Command::new("cargo")
        .args(["metadata", "--no-deps", "--format-version=1"])
        .output()
        .context("Failed to execute 'cargo metadata'")?;

    if !output.status.success() {
        let stderr_msg = String::from_utf8_lossy(&output.stderr);
        bail!("'cargo metadata' failed with: {stderr_msg}");
    }

    #[derive(Deserialize)]
    struct CargoMetadata {
        target_directory: PathBuf,
    }

    let metadata: CargoMetadata = serde_json::from_slice(&output.stdout)
        .context("Failed to parse 'cargo metadata' output")?;

    Ok(metadata.target_directory)
}

// with_workspace ensures the current working directory is always the top level
// workspace directory, i.e., where the `Anchor.toml` file is located, before
// and after the closure invocation.
//
// The closure passed into this function must never change the working directory
// to be outside the workspace. Doing so will have undefined behavior.
pub(crate) fn with_workspace<R>(
    cfg_override: &ConfigOverride,
    f: impl FnOnce(&mut WithPath<Config>) -> R,
) -> Result<R> {
    set_workspace_dir_or_exit();

    let mut cfg = Config::discover(cfg_override)
        .map_err(|e| anyhow!("Workspace configuration error: {}", e))?
        .ok_or_else(|| anyhow!("This command requires an Anchor workspace."))?;

    let r = f(&mut cfg);

    set_workspace_dir_or_exit();

    Ok(r)
}

fn is_hidden(entry: &walkdir::DirEntry) -> bool {
    entry
        .file_name()
        .to_str()
        .map(|s| s == "." || s.starts_with('.') || s == "target")
        .unwrap_or(false)
}

fn logs_websocket_url(cfg_override: &ConfigOverride, cluster_url: &str) -> String {
    let ws_scheme_url = cluster_url
        .replace("https://", "wss://")
        .replace("http://", "ws://");

    let is_local = cluster_url.contains("localhost") || cluster_url.contains("127.0.0.1");
    if !is_local {
        return ws_scheme_url;
    }

    let default_ws_port = extract_url_port(cluster_url)
        .map(|port| port.saturating_add(1))
        .unwrap_or(DEFAULT_RPC_PORT + 1);
    let ws_port = Config::discover(cfg_override)
        .ok()
        .flatten()
        .and_then(|cfg| {
            cfg.surfpool_config
                .as_ref()
                .and_then(|surfpool| surfpool.ws_port)
        })
        .unwrap_or(default_ws_port);

    replace_url_port(&ws_scheme_url, ws_port)
}

fn extract_url_port(url: &str) -> Option<u16> {
    let (_, after_scheme) = url.split_once("://")?;
    let host_port_end = after_scheme.find('/').unwrap_or(after_scheme.len());
    let (_, port_str) = after_scheme[..host_port_end].rsplit_once(':')?;
    port_str.parse().ok()
}

fn replace_url_port(url: &str, new_port: u16) -> String {
    let Some((scheme, rest)) = url.split_once("://") else {
        return url.to_string();
    };
    let (host_port_part, tail) = match rest.find('/') {
        Some(index) => (&rest[..index], &rest[index..]),
        None => (rest, ""),
    };
    let host = host_port_part
        .rsplit_once(':')
        .map(|(host, _)| host)
        .unwrap_or(host_port_part);
    format!("{scheme}://{host}:{new_port}{tail}")
}

fn get_node_version() -> Result<Version> {
    let node_version = std::process::Command::new("node")
        .arg("--version")
        .stderr(Stdio::inherit())
        .output()
        .map_err(|e| anyhow::format_err!("node failed: {}", e))?;
    parse_node_version(std::str::from_utf8(&node_version.stdout)?)
}

fn parse_node_version(output: &str) -> Result<Version> {
    let trimmed = output.trim();
    let without_v = trimmed.strip_prefix('v').unwrap_or(trimmed);
    Version::parse(without_v).map_err(Into::into)
}

/// Default re-sign attempts when blockhashes expire mid-deploy.
/// Matches Agave's `solana program deploy` default (agave/cli/src/program.rs).
/// Each blockhash window is ~60s, so 5 → ~5 minutes of resign budget.
pub const DEFAULT_MAX_SIGN_ATTEMPTS: usize = 5;

fn add_recommended_deployment_solana_args(
    client: &RpcClient,
    args: Vec<String>,
    write_locked_accounts: &[Pubkey],
) -> Result<Vec<String>> {
    let mut augmented_args = args.clone();

    // If no priority fee is provided, calculate a recommended fee based on recent txs.
    if !args.contains(&"--with-compute-unit-price".to_string()) {
        let priority_fee = get_recommended_micro_lamport_fee(client, write_locked_accounts);
        augmented_args.push("--with-compute-unit-price".to_string());
        augmented_args.push(priority_fee.to_string());
    }

    if !args.contains(&"--max-sign-attempts".to_string()) {
        augmented_args.push("--max-sign-attempts".to_string());
        augmented_args.push(DEFAULT_MAX_SIGN_ATTEMPTS.to_string());
    }

    // Note: `--buffer` injection is handled by callers (program_deploy /
    // program_upgrade) so the path can be scoped per program
    // (`target/deploy/{name}-upgrade-buffer.json`). Doing it here would either
    // collide across concurrent program deploys or require threading
    // program_name down into this fee/sign-attempts helper.

    Ok(augmented_args)
}

fn get_node_dns_option() -> &'static str {
    let Ok(version) = get_node_version() else {
        return "";
    };
    let req = VersionReq::parse(">=16.4.0").unwrap();
    if req.matches(&version) {
        "--dns-result-order=ipv4first"
    } else {
        ""
    }
}

// Remove the current workspace directory if it prefixes a string.
// This is used as a workaround for the Solana CLI using the uriparse crate to
// parse args but not handling percent encoding/decoding when using the path as
// a local filesystem path. Removing the workspace prefix handles most/all cases
// of spaces in keypair/binary paths, but this should be fixed in the Solana CLI
// and removed here.
fn strip_workspace_prefix(absolute_path: PathBuf) -> PathBuf {
    let workspace_prefix = std::env::current_dir().unwrap();
    absolute_path
        .strip_prefix(&workspace_prefix)
        .unwrap_or(&absolute_path)
        .into()
}

/// Create a new [`RpcClient`] with `confirmed` commitment level instead of the default(finalized).
pub(crate) fn create_client<U: ToString>(url: U) -> RpcClient {
    RpcClient::new_with_commitment(url, CommitmentConfig::confirmed())
}

fn address(cfg_override: &ConfigOverride) -> Result<()> {
    let (_cluster_url, wallet_path) = get_cluster_and_wallet(cfg_override)?;

    // Load keypair and get pubkey
    let keypair = Keypair::read_from_file(&wallet_path)
        .map_err(|e| anyhow!("Failed to read keypair from {}: {}", wallet_path, e))?;

    // Print the public key
    println!("{}", keypair.pubkey());

    Ok(())
}

fn balance(cfg_override: &ConfigOverride, pubkey: Option<Pubkey>, lamports: bool) -> Result<()> {
    let (cluster_url, wallet_path) = get_cluster_and_wallet(cfg_override)?;

    // Create RPC client
    let client = RpcClient::new(cluster_url);

    // Determine which account to check
    let account_pubkey = if let Some(pubkey) = pubkey {
        pubkey
    } else {
        // Load keypair from wallet path and get pubkey
        let keypair = Keypair::read_from_file(&wallet_path)
            .map_err(|e| anyhow!("Failed to read keypair from {}: {}", wallet_path, e))?;
        keypair.pubkey()
    };

    // Get balance
    let balance = client.get_balance(&account_pubkey)?;

    // Format and display output
    if lamports {
        println!("{}", balance);
    } else {
        println!("{}", format_sol(balance));
    }

    Ok(())
}

fn epoch(cfg_override: &ConfigOverride) -> Result<()> {
    let (cluster_url, _wallet_path) = get_cluster_and_wallet(cfg_override)?;

    // Create RPC client
    let client = RpcClient::new(cluster_url);

    // Get epoch info
    let epoch_info = client.get_epoch_info()?;

    // Print just the epoch number
    println!("{}", epoch_info.epoch);

    Ok(())
}

fn epoch_info(cfg_override: &ConfigOverride) -> Result<()> {
    let (cluster_url, _wallet_path) = get_cluster_and_wallet(cfg_override)?;

    // Create RPC client
    let client = RpcClient::new(cluster_url);

    // Get epoch info
    let epoch_info = client.get_epoch_info()?;

    // Calculate epoch slot range
    let first_slot_in_epoch = epoch_info.absolute_slot - epoch_info.slot_index;
    let last_slot_in_epoch = first_slot_in_epoch + epoch_info.slots_in_epoch;

    // Calculate completion stats
    let epoch_completed_percent =
        epoch_info.slot_index as f64 / epoch_info.slots_in_epoch as f64 * 100.0;
    let remaining_slots = epoch_info.slots_in_epoch - epoch_info.slot_index;

    // Display epoch information (matching Solana CLI format)
    println!("Block height: {}", epoch_info.block_height);
    println!("Slot: {}", epoch_info.absolute_slot);
    println!("Epoch: {}", epoch_info.epoch);

    if let Some(tx_count) = epoch_info.transaction_count {
        println!("Transaction Count: {}", tx_count);
    }

    println!(
        "Epoch Slot Range: [{}..{})",
        first_slot_in_epoch, last_slot_in_epoch
    );
    println!("Epoch Completed Percent: {:>3.3}%", epoch_completed_percent);
    println!(
        "Epoch Completed Slots: {}/{} ({} remaining)",
        epoch_info.slot_index, epoch_info.slots_in_epoch, remaining_slots
    );

    // Try to calculate epoch completed time
    // Get average slot time from performance samples (aggregate up to 60 samples)
    if let Ok(samples) = client.get_recent_performance_samples(Some(60)) {
        // Aggregate all samples to calculate average slot time
        let (total_slots, total_secs) =
            samples.iter().fold((0u64, 0u64), |(slots, secs), sample| {
                (
                    slots.saturating_add(sample.num_slots),
                    secs.saturating_add(sample.sample_period_secs as u64),
                )
            });

        if let Some(avg_slot_time_ms) = (total_secs * 1000).checked_div(total_slots) {
            // Calculate time_remaining using average slot time (always estimated)
            let remaining_secs = (remaining_slots * avg_slot_time_ms) / 1000;

            // Calculate time_elapsed - try actual block times first, then estimate
            // Get the first actual block in the epoch and adjust for slot differences
            let start_block_time = client
                .get_blocks_with_limit(first_slot_in_epoch, 1)
                .ok()
                .and_then(|slots| slots.first().cloned())
                .and_then(|first_actual_block| {
                    client.get_block_time(first_actual_block).ok().map(|time| {
                        // Adjust backwards if first actual block is after expected start
                        let slot_diff = first_actual_block.saturating_sub(first_slot_in_epoch);
                        let time_adjustment = (slot_diff * avg_slot_time_ms / 1000) as i64;
                        time.saturating_sub(time_adjustment)
                    })
                });

            let current_block_time = client.get_block_time(epoch_info.absolute_slot).ok();

            let (elapsed_secs, is_estimated) = if let (Some(start_time), Some(current_time)) =
                (start_block_time, current_block_time)
            {
                // Use actual block times for elapsed
                ((current_time - start_time) as u64, false)
            } else {
                // Estimate elapsed using average slot time
                ((epoch_info.slot_index * avg_slot_time_ms) / 1000, true)
            };

            // Total time = elapsed + remaining
            let total_secs = elapsed_secs + remaining_secs;

            let estimated_marker = if is_estimated { "*" } else { "" };
            println!(
                "Epoch Completed Time: {}{}/{} ({} remaining)",
                format_duration_secs(elapsed_secs),
                estimated_marker,
                format_duration_secs(total_secs),
                format_duration_secs(remaining_secs)
            );
        }
    }

    Ok(())
}

/// Format seconds into human-readable duration (e.g., "1day 5h 49m 8s")
fn format_duration_secs(total_seconds: u64) -> String {
    let seconds = total_seconds % 60;
    let total_minutes = total_seconds / 60;
    let minutes = total_minutes % 60;
    let total_hours = total_minutes / 60;
    let hours = total_hours % 24;
    let days = total_hours / 24;

    let mut parts = Vec::new();
    if days > 0 {
        parts.push(format!("{}day", days));
    }
    if hours > 0 {
        parts.push(format!("{}h", hours));
    }
    if minutes > 0 {
        parts.push(format!("{}m", minutes));
    }
    if seconds > 0 || parts.is_empty() {
        parts.push(format!("{}s", seconds));
    }

    parts.join(" ")
}

fn logs_subscribe(
    cfg_override: &ConfigOverride,
    include_votes: bool,
    address: Option<Vec<Pubkey>>,
) -> Result<()> {
    let (cluster_url, _wallet_path) = get_cluster_and_wallet(cfg_override)?;
    let ws_url = logs_websocket_url(cfg_override, &cluster_url);

    println!("Connecting to {}", ws_url);

    let filter = match (include_votes, address) {
        (true, Some(address)) => {
            RpcTransactionLogsFilter::Mentions(address.iter().map(|p| p.to_string()).collect())
        }
        (true, None) => RpcTransactionLogsFilter::AllWithVotes,
        (false, Some(address)) => {
            RpcTransactionLogsFilter::Mentions(address.iter().map(|p| p.to_string()).collect())
        }
        (false, None) => RpcTransactionLogsFilter::All,
    };

    let (_client, receiver) = PubsubClient::logs_subscribe(
        &ws_url,
        filter,
        RpcTransactionLogsConfig {
            commitment: cfg_override.commitment.map(|c| CommitmentConfig {
                commitment: c.into(),
            }),
        },
    )?;

    loop {
        match receiver.recv() {
            Ok(logs) => {
                println!("Transaction executed in slot {}:", logs.context.slot);
                println!("  Signature: {}", logs.value.signature);
                println!(
                    "  Status: {}",
                    logs.value
                        .err
                        .map(|err| err.to_string())
                        .unwrap_or_else(|| "Ok".to_string())
                );
                println!("  Log Messages:");
                for log in logs.value.logs {
                    println!("    {log}");
                }
            }
            Err(err) => {
                return Err(anyhow!("Disconnected: {err}"));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        anchor_lang_idl::types::{
            IdlGenericArg, IdlInstructionAccount, IdlInstructionAccountItem, IdlPda, IdlSeed,
            IdlSeedAccount, IdlTypeDef, IdlTypeDefGeneric,
        },
        std::collections::{HashMap, HashSet},
        tempfile::tempdir,
    };

    #[test]
    fn test_init_accepts_anchor_version() {
        let opts =
            Opts::try_parse_from(["anchor", "init", "example", "--anchor-version", "v2"]).unwrap();

        let Command::Init { anchor_version, .. } = opts.command else {
            panic!("expected init command");
        };

        assert_eq!(anchor_version, AnchorVersion::V2);
    }

    #[test]
    fn test_new_accepts_anchor_version() {
        let opts =
            Opts::try_parse_from(["anchor", "new", "example", "--anchor-version", "v2"]).unwrap();

        let Command::New { anchor_version, .. } = opts.command else {
            panic!("expected new command");
        };

        assert_eq!(anchor_version, AnchorVersion::V2);
    }

    #[test]
    #[cfg(not(windows))]
    fn test_debugger_and_coverage_commands_parse() {
        let opts =
            Opts::try_parse_from(["anchor", "debugger", "initialize", "--skip-run"]).unwrap();
        let Command::Debugger {
            test_name,
            skip_run,
            ..
        } = opts.command
        else {
            panic!("expected debugger command");
        };
        assert_eq!(test_name.as_deref(), Some("initialize"));
        assert!(skip_run);

        let opts =
            Opts::try_parse_from(["anchor", "coverage", "--skip-run", "--output", "lcov.info"])
                .unwrap();
        let Command::Coverage {
            skip_run, output, ..
        } = opts.command
        else {
            panic!("expected coverage command");
        };
        assert!(skip_run);
        assert_eq!(output, "lcov.info");
    }

    #[test]
    fn test_validator_defaults_to_surfpool() {
        let opts = Opts::try_parse_from(["anchor", "test"]).unwrap();
        let Command::Test { validator, .. } = opts.command else {
            panic!("expected test command");
        };
        assert_eq!(validator, ValidatorType::Surfpool);

        let opts = Opts::try_parse_from(["anchor", "localnet"]).unwrap();
        let Command::Localnet { validator, .. } = opts.command else {
            panic!("expected localnet command");
        };
        assert_eq!(validator, ValidatorType::Surfpool);
    }

    #[test]
    fn test_codama_command_parses() {
        let opts = Opts::try_parse_from([
            "anchor",
            "codama",
            "generate",
            "-l",
            "rust,go",
            "-p",
            "clients",
            "target/idl/demo.json",
        ])
        .unwrap();
        let Command::Codama { subcmd } = opts.command else {
            panic!("expected codama command");
        };
        let codama::CodamaCommand::Generate {
            language,
            path,
            idl,
        } = subcmd
        else {
            panic!("expected codama generate command");
        };
        assert_eq!(language, vec![codama::Language::Rust, codama::Language::Go]);
        assert_eq!(path, "clients");
        assert_eq!(idl, "target/idl/demo.json");
    }

    #[test]
    #[should_panic(expected = "Anchor workspace name must be a valid Rust identifier.")]
    fn test_init_reserved_word() {
        init(
            &ConfigOverride {
                cluster: None,
                wallet: None,
                commitment: None,
            },
            "await".to_string(),
            true,
            true,
            None,
            false,
            ProgramTemplate::default(),
            AnchorVersion::default(),
            TestTemplate::default(),
            true,
            true,
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
                commitment: None,
            },
            "fn".to_string(),
            true,
            true,
            None,
            false,
            ProgramTemplate::default(),
            AnchorVersion::default(),
            TestTemplate::default(),
            true,
            true,
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
                commitment: None,
            },
            "1project".to_string(),
            true,
            true,
            None,
            false,
            ProgramTemplate::default(),
            AnchorVersion::default(),
            TestTemplate::default(),
            true,
            true,
        )
        .unwrap();
    }

    fn index_set(indices: &[usize]) -> HashSet<usize> {
        indices.iter().copied().collect()
    }

    #[test]
    fn program_order_prefers_programs_with_more_anchor_program_deps() {
        let program_indices = index_set(&[0, 1]);
        let program_closures = HashMap::from([(0, index_set(&[1])), (1, index_set(&[]))]);
        let original_order = HashMap::from([(0, 0), (1, 1)]);

        let ordered = order_program_indices_by_dependency_cache_heuristic(
            &program_indices,
            &program_closures,
            &original_order,
        );

        assert_eq!(ordered, vec![0, 1]);
    }

    #[test]
    fn program_order_uses_total_dependency_closure_as_tiebreaker() {
        let program_indices = index_set(&[0, 1, 2, 3]);
        let program_closures = HashMap::from([
            (0, index_set(&[2, 4])),
            (1, index_set(&[3])),
            (2, index_set(&[])),
            (3, index_set(&[])),
        ]);
        let original_order = HashMap::from([(0, 1), (1, 0), (2, 2), (3, 3)]);

        let ordered = order_program_indices_by_dependency_cache_heuristic(
            &program_indices,
            &program_closures,
            &original_order,
        );

        assert_eq!(ordered, vec![0, 1, 2, 3]);
    }

    #[test]
    fn program_order_places_isolated_programs_first() {
        let program_indices = index_set(&[0, 1, 2]);
        let program_closures = HashMap::from([
            (0, index_set(&[])),
            (1, index_set(&[2])),
            (2, index_set(&[])),
        ]);
        let original_order = HashMap::from([(0, 0), (1, 1), (2, 2)]);

        let ordered = order_program_indices_by_dependency_cache_heuristic(
            &program_indices,
            &program_closures,
            &original_order,
        );

        assert_eq!(ordered, vec![0, 1, 2]);
    }

    #[test]
    fn program_order_preserves_original_order_for_unrelated_programs() {
        let program_indices = index_set(&[0, 1, 2]);
        let program_closures = HashMap::from([
            (0, index_set(&[3])),
            (1, index_set(&[])),
            (2, index_set(&[])),
        ]);
        let original_order = HashMap::from([(0, 0), (1, 1), (2, 2)]);

        let ordered = order_program_indices_by_dependency_cache_heuristic(
            &program_indices,
            &program_closures,
            &original_order,
        );

        assert_eq!(ordered, vec![0, 1, 2]);
    }

    #[test]
    fn test_predeploy_preserves_explicit_external_validator() {
        assert!(should_predeploy_before_test(false, false, false));
        assert!(should_predeploy_before_test(false, true, true));
        assert!(!should_predeploy_before_test(false, true, false));
        assert!(!should_predeploy_before_test(true, true, true));
    }

    #[test]
    fn test_validator_plan_handles_in_process_template_skip() {
        assert_eq!(
            test_validator_plan(false, true, false, true),
            TestValidatorPlan {
                skip_local_validator: true,
                predeploy: false,
                stream_program_logs: true,
            }
        );
    }

    #[test]
    fn test_validator_plan_preserves_explicit_external_validator() {
        assert_eq!(
            test_validator_plan(false, true, true, false),
            TestValidatorPlan {
                skip_local_validator: true,
                predeploy: true,
                stream_program_logs: true,
            }
        );
    }

    #[test]
    fn surfpool_flags_do_not_force_runtime_features() {
        let dir = tempdir().unwrap();
        let cfg = WithPath::new(Config::default(), dir.path().join("Anchor.toml"));
        let flags = surfpool_flags(&cfg, &None, false, false, None, &[]).unwrap();

        assert!(!flags.iter().any(|flag| flag == "--feature"));
    }

    #[test]
    fn surfpool_flags_include_snapshot_for_generated_accounts() {
        let workspace = tempdir().unwrap();
        let cfg = WithPath::new(Config::default(), workspace.path().join("Anchor.toml"));
        let test_validator = Some(TestValidator {
            validator: Some(crate::config::Validator {
                bind_address: "127.0.0.1".to_string(),
                ledger: ".anchor/test-ledger".to_string(),
                rpc_port: 18999,
                fund_accounts: Some(vec![crate::config::FundedAccount {
                    address: "new".to_string(),
                    lamports: Some(2_000_000_000),
                }]),
                ..Default::default()
            }),
            ..Default::default()
        });

        let generated_accounts = generated_validator_accounts(&cfg, &test_validator).unwrap();
        let flags = surfpool_flags(&cfg, &None, false, false, None, &generated_accounts).unwrap();

        let snapshot_index = flags.iter().position(|flag| flag == "--snapshot").unwrap();
        let snapshot_path = PathBuf::from(&flags[snapshot_index + 1]);
        let snapshot: JsonValue =
            serde_json::from_reader(File::open(&snapshot_path).unwrap()).unwrap();

        assert!(snapshot_path.exists());
        assert!(snapshot
            .get(generated_accounts[0].pubkey.to_string())
            .is_some());
    }

    #[test]
    fn test_jest_package_json_pins_uuid_for_commonjs() {
        for package_json in [
            template::package_json(true, "ISC".to_owned(), AnchorVersion::V1),
            template::ts_package_json(true, "ISC".to_owned(), AnchorVersion::V1),
        ] {
            let package: JsonValue = serde_json::from_str(&package_json).unwrap();

            assert_eq!(package["overrides"]["uuid"], "^9.0.1");
            assert_eq!(package["resolutions"]["uuid"], "^9.0.1");
            assert_eq!(package["pnpm"]["overrides"]["uuid"], "^9.0.1");
        }
    }

    #[test]
    fn parse_node_version_with_v_prefix() {
        let version = parse_node_version("v20.10.0\n").unwrap();
        assert_eq!(version.major, 20);
        assert_eq!(version.minor, 10);
        assert_eq!(version.patch, 0);
    }

    #[test]
    fn parse_node_version_without_v_prefix() {
        let version = parse_node_version("20.10.0").unwrap();
        assert_eq!(version.major, 20);
    }

    #[test]
    fn parse_node_version_ignores_surrounding_whitespace() {
        let version = parse_node_version("  v18.17.1  \n").unwrap();
        assert_eq!(version.major, 18);
        assert_eq!(version.minor, 17);
    }

    #[test]
    fn parse_node_version_errors_on_garbage() {
        assert!(parse_node_version("not a version").is_err());
        assert!(parse_node_version("").is_err());
    }

    #[test]
    fn extract_url_port_common_shapes() {
        assert_eq!(extract_url_port("http://127.0.0.1:8899"), Some(8899));
        assert_eq!(extract_url_port("http://127.0.0.1:8899/"), Some(8899));
        assert_eq!(extract_url_port("ws://localhost:8900/path?q=1"), Some(8900));
        assert_eq!(
            extract_url_port("https://api.mainnet-beta.solana.com"),
            None
        );
        assert_eq!(extract_url_port("http://127.0.0.1"), None);
        assert_eq!(extract_url_port("not a url"), None);
    }

    #[test]
    fn replace_url_port_preserves_structure() {
        assert_eq!(
            replace_url_port("http://127.0.0.1:8899", 9001),
            "http://127.0.0.1:9001"
        );
        assert_eq!(
            replace_url_port("ws://127.0.0.1:8899/path?q=1", 9050),
            "ws://127.0.0.1:9050/path?q=1"
        );
        assert_eq!(
            replace_url_port("http://127.0.0.1", 8900),
            "http://127.0.0.1:8900"
        );
    }

    #[test]
    fn idl_ts_preserves_literal_values() {
        let idl = Idl {
            address: "11111111111111111111111111111111".to_string(),
            metadata: anchor_lang_idl::types::IdlMetadata {
                name: "test_program".to_string(),
                version: "0.1.0".to_string(),
                spec: "0.1.0".to_string(),
                description: None,
                repository: None,
                dependencies: Vec::new(),
                contact: None,
                deployments: None,
            },
            docs: Vec::new(),
            instructions: vec![anchor_lang_idl::types::IdlInstruction {
                name: "do_thing".to_string(),
                docs: Vec::new(),
                discriminator: vec![0, 1, 2, 3, 4, 5, 6, 7],
                accounts: vec![IdlInstructionAccountItem::Single(IdlInstructionAccount {
                    name: "target_account".to_string(),
                    docs: Vec::new(),
                    writable: false,
                    signer: false,
                    optional: false,
                    address: None,
                    pda: Some(IdlPda {
                        seeds: vec![IdlSeed::Account(IdlSeedAccount {
                            path: "source_account.authority".to_string(),
                            account: Some("source_account".to_string()),
                        })],
                        program: None,
                    }),
                    relations: vec!["source_account".to_string()],
                })],
                args: vec![anchor_lang_idl::types::IdlField {
                    name: "some_arg".to_string(),
                    docs: Vec::new(),
                    ty: IdlType::U8,
                }],
                returns: None,
            }],
            accounts: vec![anchor_lang_idl::types::IdlAccount {
                name: "source_account".to_string(),
                discriminator: vec![8, 7, 6, 5, 4, 3, 2, 1],
            }],
            events: Vec::new(),
            errors: vec![anchor_lang_idl::types::IdlErrorCode {
                code: 6000,
                name: "Unauthorized".to_string(),
                msg: Some("Unauthorized".to_string()),
            }],
            types: vec![IdlTypeDef {
                name: "wrapper_type".to_string(),
                docs: Vec::new(),
                serialization: Default::default(),
                repr: None,
                generics: vec![IdlTypeDefGeneric::Type {
                    name: "item_type".to_string(),
                }],
                ty: IdlTypeDefTy::Type {
                    alias: IdlType::Defined {
                        name: "generic_holder".to_string(),
                        generics: vec![
                            IdlGenericArg::Type {
                                ty: IdlType::Generic("item_type".to_string()),
                            },
                            IdlGenericArg::Const {
                                value: "SEED_PREFIX".to_string(),
                            },
                        ],
                    },
                },
            }],
            constants: vec![anchor_lang_idl::types::IdlConst {
                name: "seed_prefix".to_string(),
                docs: Vec::new(),
                ty: IdlType::String,
                value: "SEED_PREFIX".to_string(),
            }],
        };

        let ts = idl_ts(&idl).unwrap();

        assert!(ts.contains(r#""name": "doThing""#));
        assert!(ts.contains(r#""name": "targetAccount""#));
        assert!(ts.contains(r#""path": "sourceAccount.authority""#));
        assert!(ts.contains(r#""account": "sourceAccount""#));
        assert!(ts.contains(r#""sourceAccount""#));
        assert!(ts.contains(r#""name": "someArg""#));
        assert!(ts.contains(r#""name": "sourceAccount""#));
        assert!(ts.contains(r#""name": "unauthorized""#));
        assert!(ts.contains(r#""msg": "Unauthorized""#));
        assert!(ts.contains(r#""name": "wrapperType""#));
        assert!(ts.contains(r#""name": "itemType""#));
        assert!(ts.contains(r#""name": "genericHolder""#));
        assert!(ts.contains(r#""generic": "itemType""#));
        assert!(ts.contains(r#""name": "seedPrefix""#));
        assert!(ts.contains(r#""value": "SEED_PREFIX""#));
    }

    // ---------------------------------------------------------------------
    // `anchor idl convert` regression tests.
    // ---------------------------------------------------------------------

    const TEST_PROGRAM_ID: Pubkey =
        solana_pubkey::pubkey!("Fg6PaFpoGXkYsidMpWTK6W2BeZ7FEfcYkg476zPFsLnS");

    #[test]
    fn apply_program_id_override_current_spec_sets_top_level_address() {
        // Current-spec input. Must patch top-level `address` and keep the
        // `metadata.spec` field so the downstream parser still detects
        // the current spec.
        let idl = serde_json::json!({
            "address": "11111111111111111111111111111111",
            "metadata": { "name": "demo", "version": "0.1.0", "spec": "0.1.0" },
            "instructions": [],
        })
        .to_string();
        let out = apply_program_id_override(idl.as_bytes(), TEST_PROGRAM_ID).unwrap();
        let v: serde_json::Value = serde_json::from_slice(&out).unwrap();
        assert_eq!(v["address"], TEST_PROGRAM_ID.to_string());
        // Sibling metadata fields must survive.
        assert_eq!(v["metadata"]["spec"], "0.1.0");
        assert_eq!(v["metadata"]["name"], "demo");
        assert_eq!(v["metadata"]["version"], "0.1.0");
    }

    #[test]
    fn apply_program_id_override_legacy_merges_into_existing_metadata() {
        // Legacy input with sibling metadata fields. Override must merge
        // into the existing `metadata` object, not replace it.
        let idl = serde_json::json!({
            "version": "0.1.0",
            "name": "demo",
            "instructions": [],
            "metadata": { "origin": "anchor", "address": "11111111111111111111111111111111" },
        })
        .to_string();
        let out = apply_program_id_override(idl.as_bytes(), TEST_PROGRAM_ID).unwrap();
        let v: serde_json::Value = serde_json::from_slice(&out).unwrap();
        assert_eq!(v["metadata"]["address"], TEST_PROGRAM_ID.to_string());
        assert_eq!(v["metadata"]["origin"], "anchor");
    }

    #[test]
    fn apply_program_id_override_legacy_no_metadata_creates_object() {
        // Legacy input without any metadata block. Override should create
        // a fresh `metadata.address` entry.
        let idl = serde_json::json!({
            "version": "0.1.0",
            "name": "demo",
            "instructions": [],
        })
        .to_string();
        let out = apply_program_id_override(idl.as_bytes(), TEST_PROGRAM_ID).unwrap();
        let v: serde_json::Value = serde_json::from_slice(&out).unwrap();
        assert_eq!(v["metadata"]["address"], TEST_PROGRAM_ID.to_string());
    }

    #[test]
    fn skip_deploy_preserves_legacy_validator_config_flags() {
        let cfg = WithPath::new(Config::default(), PathBuf::from("Anchor.toml"));
        let test_validator = Some(TestValidator {
            validator: Some(crate::config::Validator {
                bind_address: "127.0.0.1".to_string(),
                ledger: ".anchor/test-ledger".to_string(),
                rpc_port: 18999,
                warp_slot: Some(42),
                ..Default::default()
            }),
            ..Default::default()
        });

        let flags = validator_flags(&cfg, &test_validator, true, &[]).unwrap();

        assert!(flags
            .windows(2)
            .any(|args| args[0] == "--rpc-port" && args[1] == "18999"));
        assert!(flags
            .windows(2)
            .any(|args| args[0] == "--warp-slot" && args[1] == "42"));
        assert!(!flags.iter().any(|arg| arg == "--bpf-program"));
        assert!(!flags.iter().any(|arg| arg == "--upgradeable-program"));
    }

    #[test]
    fn validator_flags_emits_extra_args() {
        let workspace = tempdir().unwrap();
        let cfg = WithPath::new(Config::default(), workspace.path().join("Anchor.toml"));
        let expected = vec![
            "--rpc-pubsub-enable-block-subscription".to_string(),
            "--geyser-plugin-config".to_string(),
            "geyser.json".to_string(),
        ];
        let test_validator = Some(TestValidator {
            validator: Some(crate::config::Validator {
                bind_address: "127.0.0.1".to_string(),
                ledger: ".anchor/test-ledger".to_string(),
                rpc_port: 18999,
                extra_args: Some(expected.clone()),
                ..Default::default()
            }),
            ..Default::default()
        });

        let flags = validator_flags(&cfg, &test_validator, true, &[]).unwrap();

        assert!(flags
            .windows(expected.len())
            .any(|args| args == expected.as_slice()));
    }

    #[test]
    fn skip_deploy_keeps_generated_account_flags() {
        let workspace = tempdir().unwrap();
        let cfg = WithPath::new(Config::default(), workspace.path().join("Anchor.toml"));
        let funded_pubkey = Pubkey::new_unique();
        let test_validator = Some(TestValidator {
            validator: Some(crate::config::Validator {
                bind_address: "127.0.0.1".to_string(),
                ledger: ".anchor/test-ledger".to_string(),
                rpc_port: 18999,
                fund_accounts: Some(vec![crate::config::FundedAccount {
                    address: funded_pubkey.to_string(),
                    lamports: Some(2_000_000_000),
                }]),
                ..Default::default()
            }),
            ..Default::default()
        });

        let generated_accounts = generated_validator_accounts(&cfg, &test_validator).unwrap();
        let flags = validator_flags(&cfg, &test_validator, true, &generated_accounts).unwrap();
        let expected_path = generated_accounts[0].file_path.display().to_string();

        assert_eq!(generated_accounts.len(), 1);
        assert!(flags.windows(3).any(|args| {
            args[0] == "--account"
                && args[1] == funded_pubkey.to_string()
                && args[2] == expected_path
        }));
    }

    #[test]
    fn token_account_requires_loaded_explicit_mint() {
        let workspace = tempdir().unwrap();
        let cfg = WithPath::new(Config::default(), workspace.path().join("Anchor.toml"));
        let missing_mint = Pubkey::new_unique();
        let owner = Pubkey::new_unique();
        let test_validator = Some(TestValidator {
            validator: Some(crate::config::Validator {
                bind_address: "127.0.0.1".to_string(),
                ledger: ".anchor/test-ledger".to_string(),
                rpc_port: 18999,
                token_accounts: Some(vec![crate::config::TokenAccount {
                    mint: missing_mint.to_string(),
                    owner: owner.to_string(),
                    amount: 1,
                    address: None,
                }]),
                ..Default::default()
            }),
            ..Default::default()
        });

        let err = generated_validator_accounts(&cfg, &test_validator).unwrap_err();

        assert!(err.to_string().contains("token_account mint"));
    }

    #[test]
    fn token_account_accepts_cloned_explicit_mint() {
        let workspace = tempdir().unwrap();
        let cfg = WithPath::new(Config::default(), workspace.path().join("Anchor.toml"));
        let cloned_mint = Pubkey::new_unique();
        let owner = Pubkey::new_unique();
        let test_validator = Some(TestValidator {
            validator: Some(crate::config::Validator {
                bind_address: "127.0.0.1".to_string(),
                ledger: ".anchor/test-ledger".to_string(),
                rpc_port: 18999,
                clone: Some(vec![crate::config::CloneEntry {
                    address: cloned_mint.to_string(),
                }]),
                token_accounts: Some(vec![crate::config::TokenAccount {
                    mint: cloned_mint.to_string(),
                    owner: owner.to_string(),
                    amount: 1,
                    address: None,
                }]),
                ..Default::default()
            }),
            ..Default::default()
        });

        let generated_accounts = generated_validator_accounts(&cfg, &test_validator).unwrap();

        assert_eq!(generated_accounts.len(), 1);
        assert!(generated_accounts[0]
            .file_path
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| name.ends_with(".json")));
    }

    #[test]
    fn multiple_new_funded_accounts_get_distinct_pubkeys() {
        let workspace = tempdir().unwrap();
        let cfg = WithPath::new(Config::default(), workspace.path().join("Anchor.toml"));
        let test_validator = Some(TestValidator {
            validator: Some(crate::config::Validator {
                bind_address: "127.0.0.1".to_string(),
                ledger: ".anchor/test-ledger".to_string(),
                rpc_port: 18999,
                fund_accounts: Some(vec![
                    crate::config::FundedAccount {
                        address: "new".to_string(),
                        lamports: Some(15_000_000_000_000),
                    },
                    crate::config::FundedAccount {
                        address: "new".to_string(),
                        lamports: Some(20_000_000_000_000),
                    },
                ]),
                ..Default::default()
            }),
            ..Default::default()
        });

        let generated_accounts = generated_validator_accounts(&cfg, &test_validator).unwrap();

        assert_eq!(generated_accounts.len(), 2);
        assert_ne!(generated_accounts[0].pubkey, generated_accounts[1].pubkey);
    }
}
