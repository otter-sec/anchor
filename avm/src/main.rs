use {
    anyhow::{anyhow, Context, Error, Result},
    avm::{InstallTarget, Resolution},
    clap::{CommandFactory, Parser, Subcommand},
    semver::Version,
    std::{
        ffi::OsStr,
        io::IsTerminal,
        path::{Path, PathBuf},
    },
};

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
            verify,
        } => {
            let install_target = if let Some(path) = path {
                InstallTarget::Path(path.into())
            } else {
                parse_install_target(&version_or_commit.unwrap())?
            };
            avm::install_version(install_target, force, from_source, verify)
        }
        Commands::Uninstall { version } => {
            let v = Version::parse(&version)
                .map_err(|e| anyhow!("Invalid version `{version}`: {e}"))?;
            avm::uninstall_version(&v)
        }
        Commands::List { pre_release } => avm::list_versions(pre_release),
        Commands::Update { pre_release } => avm::update(pre_release),
        Commands::SelfUpdate {
            pre_release,
            bleeding_edge,
        } => avm::self_update(pre_release, bleeding_edge),
        Commands::Nightly { disable } => {
            if disable {
                avm::disable_nightly()
            } else {
                avm::enable_nightly()
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
        return spawn_anchor(avm::nightly_anchor_binary_path(), args);
    }

    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    let resolution = avm::resolve_anchor_version(&cwd)?.ok_or_else(|| {
        anyhow!(
            "Anchor version not set. Pin `[toolchain] anchor_version` in `Anchor.toml`, declare \
             `anchor-lang` in your program's `Cargo.toml`, or run `avm use <version>`."
        )
    })?;

    let binary_path = ensure_resolved_binary(&resolution)?;
    ensure_resolved_solana(&cwd, &resolution)?;

    spawn_anchor(binary_path, args)
}

fn spawn_anchor(binary_path: PathBuf, args: Vec<String>) -> Result<()> {
    let exit = std::process::Command::new(binary_path)
        .args(args)
        .env(
            "PATH",
            format!(
                "{}:{}",
                avm::get_bin_dir_path().to_string_lossy(),
                std::env::var("PATH").unwrap_or_default()
            ),
        )
        // Signal to the spawned anchor-cli that AVM has already resolved the
        // toolchain version, so it must not re-exec via `[toolchain] anchor_version`.
        .env("AVM_ACTIVE", "1")
        .env("CARGO_RESOLVER_INCOMPATIBLE_RUST_VERSIONS", "fallback")
        .spawn()?
        .wait_with_output()
        .expect("Failed to run anchor-cli");

    if !exit.status.success() {
        std::process::exit(exit.status.code().unwrap_or(1));
    }

    Ok(())
}

/// Ensure the Solana CLI requested by the same project context is active
/// before spawning the resolved Anchor binary.
fn ensure_resolved_solana(cwd: &Path, resolution: &Resolution) -> Result<()> {
    let Some(solana) = avm::solana::resolve_solana_cli_for_anchor_resolution(cwd, resolution)?
    else {
        return Ok(());
    };

    avm::solana::ensure_solana_cli(&solana.version).with_context(|| {
        format!(
            "setting up Solana {} resolved from {}",
            solana.version,
            solana.source.describe()
        )
    })
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

    avm::install_version(InstallTarget::Version(version.clone()), false, false, false)?;

    if !binary_path.exists() {
        anyhow::bail!(
            "anchor-cli {version} install reported success but binary is still missing at {}",
            binary_path.display()
        );
    }
    Ok(binary_path)
}

fn main() -> Result<()> {
    // If the binary is named `anchor` then run the proxy.
    if let Some(stem) = std::env::args()
        .next()
        .as_ref()
        .and_then(|s| std::path::Path::new(s).file_stem().and_then(OsStr::to_str))
    {
        if stem == "anchor" {
            return anchor_proxy();
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
}
