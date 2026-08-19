use {
    anyhow::{anyhow, Context, Error, Result},
    avm::{InstallTarget, Resolution},
    clap::{CommandFactory, Parser, Subcommand},
    fs2::FileExt,
    semver::Version,
    std::{
        env,
        ffi::{OsStr, OsString},
        fs,
        io::IsTerminal,
        path::{Path, PathBuf},
        process::Command,
    },
};

const REAL_CARGO_ENV: &str = "AVM_REAL_CARGO";
const CARGO_NEXT_LOCKFILE_BUMP_ENV: &str = "CARGO_UNSTABLE_NEXT_LOCKFILE_BUMP";

#[derive(Parser)]
#[clap(name = "avm", about = "Anchor version manager", version)]
pub struct Cli {
    #[clap(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    #[clap(about = "Use a specific version of Anchor")]
    Use {
        /// Version to use: `latest`, `latest-pre-release`, or a specific version e.g. `1.0.0`, `1.0.0-rc.3`
        #[clap(required = false)]
        version: Option<String>,
    },
    #[clap(about = "Install a version of Anchor", alias = "i")]
    Install {
        /// Anchor version, commit, `latest`, or `latest-pre-release`; conflicts with `--path`
        #[clap(required_unless_present = "path")]
        version_or_commit: Option<String>,
        /// Path to local anchor repo, conflicts with `version_or_commit`
        #[clap(long, conflicts_with = "version_or_commit")]
        path: Option<String>,
        #[clap(long)]
        /// Flag to force installation even if the version is already installed
        force: bool,
        #[clap(long)]
        /// Build from source code rather than downloading prebuilt binaries
        from_source: bool,
        #[clap(long)]
        /// Skip build provenance attestation verification. This is potentially dangerous because
        /// downloaded binaries will not be authenticated
        skip_attestation: bool,
        #[clap(long)]
        /// Install `solana-verify` as well
        verify: bool,
    },
    #[clap(about = "Uninstall a version of Anchor")]
    Uninstall {
        /// Version to uninstall, e.g. `1.0.0` or `1.0.0-rc.3`
        version: String,
    },
    #[clap(about = "List available versions of Anchor", alias = "ls")]
    List {
        #[clap(long)]
        /// Include pre-release versions in the list
        pre_release: bool,
    },
    #[clap(about = "Update to the latest Anchor version")]
    Update {
        #[clap(long)]
        /// Include pre-release versions when selecting the latest
        pre_release: bool,
        #[clap(long)]
        /// Skip build provenance attestation verification. This is potentially dangerous because
        /// downloaded binaries will not be authenticated
        skip_attestation: bool,
    },
    #[clap(about = "Update avm itself to the latest version via cargo install")]
    SelfUpdate {
        #[clap(long)]
        /// Update to the latest pre-release version instead of the latest stable
        pre_release: bool,
        #[clap(long, conflicts_with = "pre_release")]
        /// Build and install from the latest commit on the master branch
        bleeding_edge: bool,
    },
    #[clap(about = "Enable or disable the Anchor nightly channel")]
    Nightly {
        #[clap(long)]
        /// Disable the nightly channel and restore normal version resolution
        disable: bool,
        #[clap(long, conflicts_with = "disable")]
        /// Skip build provenance attestation verification. This is potentially dangerous because
        /// downloaded binaries will not be authenticated
        skip_attestation: bool,
    },
    #[clap(about = "Generate shell completions for AVM")]
    Completions {
        #[clap(value_enum)]
        shell: clap_complete::Shell,
    },
    #[clap(about = "Resolve or install the Solana CLI for the current project")]
    Solana {
        #[clap(subcommand)]
        command: SolanaCommand,
    },
    #[clap(about = "Inspect or manage the Solana platform-tools toolchain")]
    PlatformTools {
        #[clap(subcommand)]
        command: PlatformToolsCommand,
    },
}

#[derive(Subcommand)]
pub enum SolanaCommand {
    /// Resolve which Solana CLI version this project should use.
    ///
    /// Project Solana pins win. If the project does not pin Solana directly,
    /// AVM resolves the Anchor version and maps it to Anchor's recommended
    /// Solana CLI version.
    Resolve,
    /// Install and activate a Solana CLI version. With no argument, installs
    /// the project-resolved version.
    Install {
        /// Solana CLI version, e.g. `4.1.2`.
        version: Option<String>,
        /// Run the installer even if the requested version is already active.
        #[clap(long)]
        force: bool,
    },
}

#[derive(Subcommand)]
pub enum PlatformToolsCommand {
    /// Resolve which platform-tools version this project should use.
    ///
    /// Walks up from the current directory looking at `[toolchain] solana_version`
    /// in `Anchor.toml` and the `solana-program` dep in `Cargo.toml`, then maps
    /// the resolved Solana version to a platform-tools version via the static map
    /// derived from `cargo-build-sbf`'s `DEFAULT_PLATFORM_TOOLS_VERSION`.
    Resolve,
    /// Install a platform-tools version. With no argument, installs the
    /// project-resolved version.
    Install {
        /// Platform-tools version, e.g. `v1.54` (the leading `v` is optional).
        version: Option<String>,
        /// Re-download and replace an existing install.
        #[clap(long)]
        force: bool,
    },
    /// List installed platform-tools versions under `$AVM_HOME/platform-tools`.
    List,
    /// Remove an installed platform-tools version.
    Uninstall {
        /// Platform-tools version to remove, e.g. `v1.54`.
        version: String,
    },
}

/// Returns true if `pre` is a semver pre-release tag (`rc.`, `beta.`, `alpha.`),
/// false if it looks like a git commit hash.
fn is_pre_release(pre: &str) -> bool {
    pre.starts_with("rc.") || pre.starts_with("beta.") || pre.starts_with("alpha.")
}

fn parse_install_target(version_or_commit: &str) -> Result<InstallTarget, Error> {
    match version_or_commit {
        "latest" => return Ok(InstallTarget::Version(avm::get_latest_version(false)?)),
        "latest-pre-release" => return Ok(InstallTarget::Version(avm::get_latest_version(true)?)),
        _ => {}
    }

    if let Ok(version) = Version::parse(version_or_commit) {
        if version.pre.is_empty() {
            return Ok(InstallTarget::Version(version));
        }
        // If the prerelease segment is a bare hex string it was written as a commit, e.g.
        // `avm install 0.28.0-6cf200493a307c01487c7b492b4893e0d6f6cb23`.
        // Otherwise it is a proper semver pre-release tag such as `rc.3` or `alpha.1`.
        if is_pre_release(version.pre.as_str()) {
            return Ok(InstallTarget::Version(version));
        }
        // Prerelease segment looks like a commit hash, e.g.
        // `avm install 0.28.0-6cf200493a307c01487c7b492b4893e0d6f6cb23`
        return Ok(InstallTarget::Commit(version.pre.to_string()));
    }

    avm::check_and_get_full_commit(version_or_commit)
        .map(InstallTarget::Commit)
        .map_err(|e| anyhow!("Not a valid version or commit: {e}"))
}

fn resolve_use_version(version: Option<String>) -> Result<Option<Version>> {
    match version.as_deref() {
        Some("latest") => Ok(Some(avm::get_latest_version(false)?)),
        Some("latest-pre-release") => Ok(Some(avm::get_latest_version(true)?)),
        Some(v) => Ok(Some(
            Version::parse(v).map_err(|e| anyhow!("Invalid version `{v}`: {e}"))?,
        )),
        None => Ok(None),
    }
}

pub fn entry(opts: Cli) -> Result<()> {
    if !matches!(
        opts.command,
        Commands::SelfUpdate { .. } | Commands::Nightly { .. } | Commands::Completions { .. }
    ) {
        avm::check_avm_version_and_warn();
    }

    match opts.command {
        Commands::Use { version } => {
            let resolved = resolve_use_version(version)?;
            avm::use_version(resolved)
        }
        Commands::Install {
            version_or_commit,
            path,
            force,
            from_source,
            skip_attestation,
            verify,
        } => {
            let install_target = if let Some(path) = path {
                InstallTarget::Path(path.into())
            } else {
                parse_install_target(&version_or_commit.unwrap())?
            };
            avm::install_version(install_target, force, from_source, verify, skip_attestation)
        }
        Commands::Uninstall { version } => {
            let v = Version::parse(&version)
                .map_err(|e| anyhow!("Invalid version `{version}`: {e}"))?;
            avm::uninstall_version(&v)
        }
        Commands::List { pre_release } => avm::list_versions(pre_release),
        Commands::Update {
            pre_release,
            skip_attestation,
        } => avm::update(pre_release, skip_attestation),
        Commands::SelfUpdate {
            pre_release,
            bleeding_edge,
        } => avm::self_update(pre_release, bleeding_edge),
        Commands::Nightly {
            disable,
            skip_attestation,
        } => {
            if disable {
                avm::disable_nightly()
            } else {
                avm::enable_nightly(skip_attestation)
            }
        }
        Commands::Completions { shell } => {
            clap_complete::generate(shell, &mut Cli::command(), "avm", &mut std::io::stdout());
            Ok(())
        }
        Commands::Solana { command } => match command {
            SolanaCommand::Resolve => {
                let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
                let res = avm::resolve_solana_cli(&cwd)?.ok_or_else(|| {
                    anyhow!(
                        "Solana version not set. Pin `[toolchain] solana_version` in \
                         `Anchor.toml`, declare `solana-program` in your program's `Cargo.toml`, \
                         or pin an Anchor version AVM can map."
                    )
                })?;
                println!("solana {} ({})", res.version, res.source.describe());
                Ok(())
            }
            SolanaCommand::Install { version, force } => {
                let version = match version {
                    Some(v) => Version::parse(&v)
                        .map_err(|e| anyhow!("Invalid Solana version `{v}`: {e}"))?,
                    None => {
                        let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
                        let res = avm::resolve_solana_cli(&cwd)?.ok_or_else(|| {
                            anyhow!(
                                "Solana version not set. Pin `[toolchain] solana_version` in \
                                 `Anchor.toml`, declare `solana-program` in your program's \
                                 `Cargo.toml`, or pin an Anchor version AVM can map."
                            )
                        })?;
                        println!(
                            "Installing project-resolved Solana {} ({})",
                            res.version,
                            res.source.describe()
                        );
                        res.version
                    }
                };
                avm::solana::install_solana_cli(&version, force)
            }
        },
        Commands::PlatformTools { command } => match command {
            PlatformToolsCommand::Resolve => {
                let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
                let res = avm::resolve_platform_tools(&cwd)?;
                println!(
                    "platform-tools {} (rustc {}, {})",
                    res.version,
                    res.rustc,
                    res.source.describe()
                );
                Ok(())
            }
            PlatformToolsCommand::Install { version, force } => {
                let version = match version {
                    Some(v) => v,
                    None => {
                        let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
                        let res = avm::resolve_platform_tools(&cwd)?;
                        println!(
                            "Installing project-resolved version {} (rustc {}, {})",
                            res.version,
                            res.rustc,
                            res.source.describe()
                        );
                        res.version
                    }
                };
                avm::platform_tools::install_platform_tools(&version, force)
            }
            PlatformToolsCommand::List => {
                let installed = avm::platform_tools::read_installed_platform_tools()?;
                if installed.is_empty() {
                    println!("(no platform-tools installed)");
                } else {
                    for v in installed {
                        println!("{v}");
                    }
                }
                Ok(())
            }
            PlatformToolsCommand::Uninstall { version } => {
                avm::platform_tools::uninstall_platform_tools(&version)
            }
        },
    }
}

fn anchor_proxy() -> Result<()> {
    let args = std::env::args().skip(1).collect::<Vec<String>>();

    if avm::ensure_nightly_active()?.is_some() {
        return spawn_anchor(avm::nightly_anchor_binary_path(), args, false);
    }

    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    let resolution = avm::resolve_anchor_version(&cwd)?.ok_or_else(|| {
        anyhow!(
            "Anchor version not set. Pin `[toolchain] anchor_version` in `Anchor.toml`, declare \
             `anchor-lang` in your program's `Cargo.toml`, or run `avm use <version>`."
        )
    })?;

    let binary_path = ensure_resolved_binary(&resolution)?;
    prepend_solana_bin_to_path()?;
    let platform_tools_guard = ensure_resolved_solana(&cwd, &resolution)?
        .map(|solana| ensure_resolved_platform_tools(&cwd, &solana))
        .transpose()?;

    spawn_anchor(
        binary_path,
        args,
        platform_tools_guard
            .as_ref()
            .is_some_and(|guard| guard.enable_next_lockfile_bump),
    )
}

/// Spawn the resolved Anchor CLI with a temporary Cargo proxy first on `PATH`.
///
/// The proxy lives until the child exits and receives the absolute path to the
/// real Cargo executable through [`REAL_CARGO_ENV`], avoiding recursive proxy
/// invocation when it delegates the command.
fn spawn_anchor(
    binary_path: PathBuf,
    args: Vec<String>,
    enable_next_lockfile_bump: bool,
) -> Result<()> {
    let cargo_proxy = CargoProxy::new()?;
    let path = env::join_paths(
        [
            cargo_proxy.dir.path().to_path_buf(),
            avm::get_bin_dir_path(),
        ]
        .into_iter()
        .chain(env::split_paths(&env::var_os("PATH").unwrap_or_default())),
    )?;

    let mut command = Command::new(binary_path);
    command
        .args(args)
        .env("PATH", path)
        .env(REAL_CARGO_ENV, &cargo_proxy.real_cargo)
        // Signal to the spawned anchor-cli that AVM has already resolved the
        // toolchain version, so it must not re-exec via `[toolchain] anchor_version`.
        .env("AVM_ACTIVE", "1")
        .env("CARGO_RESOLVER_INCOMPATIBLE_RUST_VERSIONS", "fallback");
    if enable_next_lockfile_bump {
        command.env(CARGO_NEXT_LOCKFILE_BUMP_ENV, "true");
    }

    let exit = command
        .spawn()?
        .wait_with_output()
        .expect("Failed to run anchor-cli");

    if !exit.status.success() {
        std::process::exit(exit.status.code().unwrap_or(1));
    }

    Ok(())
}

/// Owns the temporary `cargo` entry point and the Cargo executable it delegates to.
///
/// The entry point is the current AVM executable under Cargo's filename, making
/// AVM a multi-call binary without installing another executable permanently.
struct CargoProxy {
    dir: tempfile::TempDir,
    real_cargo: PathBuf,
}

impl CargoProxy {
    fn new() -> Result<Self> {
        let current_exe = env::current_exe().context("resolving AVM executable")?;
        let real_cargo = find_real_cargo(&current_exe)?;
        let dir = tempfile::tempdir().context("creating temporary Cargo proxy directory")?;
        let proxy = dir
            .path()
            .join(if cfg!(windows) { "cargo.exe" } else { "cargo" });

        #[cfg(unix)]
        std::os::unix::fs::symlink(&current_exe, &proxy)
            .context("creating temporary Cargo proxy")?;
        #[cfg(windows)]
        fs::copy(&current_exe, &proxy).context("creating temporary Cargo proxy")?;

        Ok(Self { dir, real_cargo })
    }
}

/// Find Cargo on the original `PATH`, skipping entries that resolve back to AVM.
fn find_real_cargo(current_exe: &Path) -> Result<PathBuf> {
    let cargo_name = if cfg!(windows) { "cargo.exe" } else { "cargo" };
    let current_exe = fs::canonicalize(current_exe).context("canonicalizing AVM executable")?;

    env::split_paths(&env::var_os("PATH").unwrap_or_default())
        .map(|dir| dir.join(cargo_name))
        .find(|candidate| {
            candidate.is_file()
                && fs::canonicalize(candidate)
                    .map(|candidate| candidate != current_exe)
                    .unwrap_or(false)
        })
        .ok_or_else(|| anyhow!("Could not find `cargo` on PATH"))
}

/// Handle an AVM invocation whose executable name is `cargo`.
///
/// Only an unversioned `+nightly` selector is pinned. Every other invocation,
/// including `cargo build-sbf` and explicitly dated toolchains, is forwarded
/// unchanged to the real Cargo executable.
fn cargo_proxy() -> Result<()> {
    let real_cargo =
        env::var_os(REAL_CARGO_ENV).ok_or_else(|| anyhow!("{REAL_CARGO_ENV} is not set"))?;
    let mut args = env::args_os().skip(1).collect::<Vec<_>>();

    if args
        .first()
        .is_some_and(|toolchain| toolchain == "+nightly")
    {
        let cwd = env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
        let resolution = avm::idl_nightly::resolve_idl_nightly(&cwd)?;
        pin_idl_nightly(&mut args, &resolution.version);
        ensure_idl_nightly_installed(&resolution.version)?;
    }

    let status = Command::new(real_cargo)
        .args(args)
        .status()
        .context("running Cargo through AVM proxy")?;
    if !status.success() {
        std::process::exit(status.code().unwrap_or(1));
    }

    Ok(())
}

/// Rewrite `+nightly` to the pinned IDL nightly and report whether it changed.
fn pin_idl_nightly(args: &mut [OsString], idl_nightly: &str) -> bool {
    let Some(toolchain) = args.first_mut() else {
        return false;
    };
    if toolchain != "+nightly" {
        return false;
    }

    *toolchain = format!("+{idl_nightly}").into();
    true
}

fn ensure_idl_nightly_installed(idl_nightly: &str) -> Result<()> {
    let output = Command::new("rustup")
        .args(["toolchain", "list"])
        .output()
        .context("listing installed Rust toolchains")?;
    let installed = String::from_utf8(output.stdout)?
        .lines()
        .any(|line| line.starts_with(idl_nightly));
    if installed {
        return Ok(());
    }

    let status = Command::new("rustup")
        .args(["toolchain", "install", idl_nightly, "--profile", "minimal"])
        .status()
        .context("installing pinned IDL Rust toolchain")?;
    if !status.success() {
        anyhow::bail!("Failed to install Rust toolchain {idl_nightly}");
    }

    Ok(())
}

/// Make freshly installed Solana executables visible to AVM and the Anchor
/// process without relying on shell-profile changes.
fn prepend_solana_bin_to_path() -> Result<()> {
    let path = env::join_paths(
        std::iter::once(avm::solana::active_release_bin_path()?)
            .chain(env::split_paths(&env::var_os("PATH").unwrap_or_default())),
    )?;
    env::set_var("PATH", path);
    Ok(())
}

/// Ensure the Solana CLI requested by the same project context is active
/// before spawning the resolved Anchor binary.
fn ensure_resolved_solana(
    cwd: &Path,
    resolution: &Resolution,
) -> Result<Option<avm::SolanaCliResolution>> {
    let Some(solana) = avm::solana::resolve_solana_cli_for_anchor_resolution(cwd, resolution)?
    else {
        return Ok(None);
    };

    avm::solana::ensure_solana_cli(&solana.version).with_context(|| {
        format!(
            "setting up Solana {} resolved from {}",
            solana.version,
            solana.source.describe()
        )
    })?;
    Ok(Some(solana))
}

/// Holds the cross-process lock protecting Rustup's mutable Solana toolchain
/// aliases for the duration of the Anchor invocation.
struct PlatformToolsGuard {
    _lock: fs::File,
    enable_next_lockfile_bump: bool,
}

/// Ensure the platform-tools requested by the project are installed in
/// `cargo-build-sbf`'s shared cache and linked under the Rustup name expected by
/// the active Solana generation.
fn ensure_resolved_platform_tools(
    cwd: &Path,
    solana: &avm::SolanaCliResolution,
) -> Result<PlatformToolsGuard> {
    let lock = acquire_platform_tools_lock()?;
    let resolution = avm::platform_tools::resolve_platform_tools_for_solana_cli(cwd, solana)?;
    let platform_tools_path =
        avm::platform_tools::solana_cache_platform_tools_path(&resolution.version)?;

    if !avm::platform_tools::platform_tools_are_installed_at(&platform_tools_path) {
        if cargo_build_sbf_supports_install_only()? {
            let status = Command::new("cargo")
                .args([
                    "build-sbf",
                    "--install-only",
                    "--tools-version",
                    &resolution.version,
                ])
                .status()
                .with_context(|| {
                    format!(
                        "installing platform-tools {} resolved from {}",
                        resolution.version,
                        resolution.source.describe()
                    )
                })?;
            if !status.success() {
                anyhow::bail!(
                    "Failed to install platform-tools {} resolved from {}",
                    resolution.version,
                    resolution.source.describe()
                );
            }
        } else {
            avm::platform_tools::install_platform_tools_in_solana_cache(
                &resolution.version,
                false,
            )?;
        }
    }

    if !avm::platform_tools::platform_tools_are_installed_at(&platform_tools_path) {
        anyhow::bail!(
            "platform-tools {} installation did not create a Rust sysroot at {}",
            resolution.version,
            platform_tools_path.display()
        );
    }

    let toolchain_name = platform_tools_toolchain_name(solana, &resolution);
    ensure_rustup_toolchain_link(&toolchain_name, &platform_tools_path.join("rust"))?;

    let enable_next_lockfile_bump =
        avm::platform_tools::cargo_lock_v4_requires_opt_in(cwd, &resolution)?;

    Ok(PlatformToolsGuard {
        _lock: lock,
        enable_next_lockfile_bump,
    })
}

fn acquire_platform_tools_lock() -> Result<fs::File> {
    fs::create_dir_all(&*avm::AVM_HOME)
        .with_context(|| format!("creating {}", avm::AVM_HOME.display()))?;
    let lock_path = avm::AVM_HOME.join(".platform-tools.lock");
    let lock = fs::OpenOptions::new()
        .create(true)
        .read(true)
        .write(true)
        .truncate(false)
        .open(&lock_path)
        .with_context(|| format!("opening {}", lock_path.display()))?;
    FileExt::lock_exclusive(&lock).with_context(|| format!("locking {}", lock_path.display()))?;
    Ok(lock)
}

fn cargo_build_sbf_supports_install_only() -> Result<bool> {
    let output = match Command::new("cargo-build-sbf").arg("--help").output() {
        Ok(output) => output,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(false),
        Err(err) => return Err(err).context("checking cargo-build-sbf capabilities"),
    };
    if !output.status.success() {
        return Ok(false);
    }

    Ok(
        String::from_utf8_lossy(&output.stdout).contains("--install-only")
            || String::from_utf8_lossy(&output.stderr).contains("--install-only"),
    )
}

fn platform_tools_toolchain_name(
    solana: &avm::SolanaCliResolution,
    platform_tools: &avm::PlatformToolsResolution,
) -> String {
    if solana.version.major < 3 {
        "solana".to_string()
    } else {
        format!(
            "{}-sbpf-solana-{}",
            platform_tools.rustc, platform_tools.version
        )
    }
}

fn ensure_rustup_toolchain_link(name: &str, rust_path: &Path) -> Result<()> {
    let output = Command::new("rustup")
        .args(["toolchain", "list", "-v"])
        .output()
        .context("listing linked Rust toolchains")?;
    if !output.status.success() {
        anyhow::bail!("Failed to list linked Rust toolchains");
    }

    let installed_path = String::from_utf8(output.stdout)?.lines().find_map(|line| {
        let mut fields = line.split_whitespace();
        (fields.next() == Some(name))
            .then(|| line.split_whitespace().last().map(PathBuf::from))
            .flatten()
    });

    if installed_path
        .as_deref()
        .is_some_and(|installed| paths_refer_to_same_directory(installed, rust_path))
    {
        return Ok(());
    }

    if installed_path.is_some() {
        let status = Command::new("rustup")
            .args(["toolchain", "uninstall", name])
            .status()
            .with_context(|| format!("unlinking Rust toolchain {name}"))?;
        if !status.success() {
            anyhow::bail!("Failed to unlink Rust toolchain {name}");
        }
    }

    let status = Command::new("rustup")
        .arg("toolchain")
        .arg("link")
        .arg(name)
        .arg(rust_path)
        .status()
        .with_context(|| format!("linking Rust toolchain {name}"))?;
    if !status.success() {
        anyhow::bail!("Failed to link Rust toolchain {name}");
    }

    Ok(())
}

fn paths_refer_to_same_directory(left: &Path, right: &Path) -> bool {
    match (fs::canonicalize(left), fs::canonicalize(right)) {
        (Ok(left), Ok(right)) => left == right,
        _ => left == right,
    }
}

/// Ensure the binary for `resolution.version` exists on disk, prompting the user
/// to auto-install on a TTY and bailing with a clear hint otherwise.
fn ensure_resolved_binary(resolution: &Resolution) -> Result<PathBuf> {
    let version = &resolution.version;
    let binary_path = avm::version_binary_path(version);
    if binary_path.exists() {
        return Ok(binary_path);
    }

    if !std::io::stdin().is_terminal() {
        anyhow::bail!(
            "anchor-cli {version} (resolved from {}) is not installed.\nRun `avm install \
             {version}` to install it.",
            resolution.source.describe()
        );
    }

    eprintln!(
        "anchor-cli {version} (resolved from {}) is not installed.",
        resolution.source.describe()
    );
    eprint!("Install now? [y/N] ");
    std::io::Write::flush(&mut std::io::stderr()).ok();

    let mut line = String::new();
    std::io::BufRead::read_line(&mut std::io::stdin().lock(), &mut line)
        .context("reading confirmation from stdin")?;
    match line.trim() {
        "y" | "Y" | "yes" => {}
        _ => anyhow::bail!("Installation declined."),
    }

    avm::install_version(
        InstallTarget::Version(version.clone()),
        false,
        false,
        false,
        false,
    )?;

    if !binary_path.exists() {
        anyhow::bail!(
            "anchor-cli {version} install reported success but binary is still missing at {}",
            binary_path.display()
        );
    }
    Ok(binary_path)
}

fn main() -> Result<()> {
    // If the binary is named `anchor` or `cargo` then run the relevant proxy.
    if let Some(stem) = std::env::args()
        .next()
        .as_ref()
        .and_then(|s| std::path::Path::new(s).file_stem().and_then(OsStr::to_str))
    {
        if stem == "anchor" {
            return anchor_proxy();
        }
        if stem == "cargo" {
            return cargo_proxy();
        }
    }

    // Make sure the user's home directory is setup with the paths required by AVM.
    avm::ensure_paths();

    let opt = Cli::parse();
    entry(opt)
}

#[cfg(test)]
mod tests {
    use {super::*, avm::InstallTarget};

    #[test]
    fn pins_only_unversioned_nightly_cargo_invocations() {
        let mut nightly = vec![OsString::from("+nightly"), OsString::from("test")];
        assert!(pin_idl_nightly(&mut nightly, "nightly-2025-04-15"));
        assert_eq!(nightly[0], "+nightly-2025-04-15");

        let mut build_sbf = vec![OsString::from("build-sbf")];
        assert!(!pin_idl_nightly(&mut build_sbf, "nightly-2025-04-15"));
        assert_eq!(build_sbf[0], "build-sbf");

        let mut dated = vec![OsString::from("+nightly-2026-07-01")];
        assert!(!pin_idl_nightly(&mut dated, "nightly-2025-04-15"));
        assert_eq!(dated[0], "+nightly-2026-07-01");
    }

    // --- is_pre_release ---

    #[test]
    fn test_is_pre_release_rc() {
        assert!(is_pre_release("rc.3"));
    }

    #[test]
    fn test_is_pre_release_beta() {
        assert!(is_pre_release("beta.1"));
    }

    #[test]
    fn test_is_pre_release_alpha() {
        assert!(is_pre_release("alpha.2"));
    }

    #[test]
    fn test_is_pre_release_commit_hash() {
        assert!(!is_pre_release("e1afcbf71e0f2e10fae14525934a6a68479167b9"));
    }

    #[test]
    fn test_is_pre_release_short_commit() {
        assert!(!is_pre_release("e1afcbf"));
    }

    // --- parse_install_target (no-network cases) ---

    #[test]
    fn test_parse_install_target_stable_version() {
        let result = parse_install_target("1.0.0").unwrap();
        assert!(
            matches!(result, InstallTarget::Version(v) if v == Version::parse("1.0.0").unwrap())
        );
    }

    #[test]
    fn test_parse_install_target_pre_release_version() {
        let result = parse_install_target("1.0.0-rc.3").unwrap();
        assert!(
            matches!(result, InstallTarget::Version(v) if v == Version::parse("1.0.0-rc.3").unwrap())
        );
    }

    #[test]
    fn test_parse_install_target_alpha_version() {
        let result = parse_install_target("1.0.0-alpha.1").unwrap();
        assert!(
            matches!(result, InstallTarget::Version(v) if v == Version::parse("1.0.0-alpha.1").unwrap())
        );
    }

    #[test]
    fn test_parse_install_target_commit_as_prerelease() {
        // `avm install 0.28.0-<sha>` syntax — pre segment is a commit hash
        let commit = "6cf200493a307c01487c7b492b4893e0d6f6cb23";
        let result = parse_install_target(&format!("0.28.0-{commit}")).unwrap();
        assert!(matches!(result, InstallTarget::Commit(c) if c == commit));
    }

    #[test]
    fn test_parse_install_target_bare_commit_hash() {
        // bare full commit SHA — resolved via GitHub API to the same hash
        let commit = "e1afcbf71e0f2e10fae14525934a6a68479167b9";
        let result = parse_install_target(commit).unwrap();
        assert!(matches!(result, InstallTarget::Commit(c) if c == commit));
    }

    // --- resolve_use_version (no-network cases) ---

    #[test]
    fn test_resolve_use_version_none() {
        assert!(resolve_use_version(None).unwrap().is_none());
    }

    #[test]
    fn test_resolve_use_version_specific_stable() {
        let version = resolve_use_version(Some("1.0.0".to_string()))
            .unwrap()
            .unwrap();
        assert_eq!(version.major, 1);
        assert_eq!(version.minor, 0);
        assert_eq!(version.patch, 0);
        assert!(version.pre.is_empty());
    }

    #[test]
    fn test_resolve_use_version_specific_pre_release() {
        let version = resolve_use_version(Some("1.0.0-rc.3".to_string()))
            .unwrap()
            .unwrap();
        assert_eq!(version.major, 1);
        assert_eq!(version.minor, 0);
        assert_eq!(version.patch, 0);
        assert_eq!(version.pre.as_str(), "rc.3");
    }

    #[test]
    fn test_resolve_use_version_latest_is_stable() {
        let version = resolve_use_version(Some("latest".to_string()))
            .unwrap()
            .unwrap();
        assert!(
            version.pre.is_empty(),
            "latest should resolve to a stable version, got {version}"
        );
    }

    #[test]
    fn test_resolve_use_version_invalid() {
        assert!(resolve_use_version(Some("not-a-version".to_string())).is_err());
    }

    #[test]
    fn test_parse_skip_attestation_for_binary_install_commands() {
        let install =
            Cli::try_parse_from(["avm", "install", "1.1.2", "--skip-attestation"]).unwrap();
        assert!(matches!(
            install.command,
            Commands::Install {
                skip_attestation: true,
                ..
            }
        ));

        let update = Cli::try_parse_from(["avm", "update", "--skip-attestation"]).unwrap();
        assert!(matches!(
            update.command,
            Commands::Update {
                skip_attestation: true,
                ..
            }
        ));

        let nightly = Cli::try_parse_from(["avm", "nightly", "--skip-attestation"]).unwrap();
        assert!(matches!(
            nightly.command,
            Commands::Nightly {
                skip_attestation: true,
                ..
            }
        ));
    }

    #[test]
    fn test_skip_attestation_help_warns_that_it_is_dangerous() {
        for subcommand in ["install", "update", "nightly"] {
            let mut command = Cli::command();
            let help = command
                .find_subcommand_mut(subcommand)
                .unwrap()
                .render_long_help()
                .to_string();
            assert!(help.contains("--skip-attestation"));
            assert!(help.contains("potentially dangerous"));
        }
    }
}
