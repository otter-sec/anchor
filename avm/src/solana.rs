//! Solana CLI resolution and installation for AVM.
//!
//! Project-pinned Solana versions win. If the project does not pin Solana,
//! AVM resolves the Anchor version and maps it to Anchor's recommended Solana
//! CLI version using `../anchor-solana-map.toml`. `solana-program` dependency
//! requirements are matched against Anza's hosted installer versions so AVM
//! does not try to install unavailable lower-bound releases.
use {
    crate::{
        current_version, read_installed_versions,
        resolve::{
            resolve_anchor_version_with, resolve_solana_version, Resolution, ResolutionSource,
            SolanaResolution, SolanaResolutionSource,
        },
        DOWNLOAD_CLIENT,
    },
    anyhow::{anyhow, bail, Context, Result},
    reqwest::StatusCode,
    semver::{Version, VersionReq},
    serde::Deserialize,
    std::{
        fmt::Display,
        fs,
        io::ErrorKind,
        path::Path,
        process::{Command, Output, Stdio},
        sync::LazyLock,
        thread,
        time::Duration,
    },
};

const ANCHOR_SOLANA_MAP_TOML: &str = include_str!("../anchor-solana-map.toml");
const SOLANA_CLI_VERSIONS_TOML: &str = include_str!("../solana-cli-versions.toml");
const AGAVE_INSTALL_MIN_VERSION_STR: &str = "1.18.19";
const INSTALLER_DOWNLOAD_MAX_ATTEMPTS: usize = 4;
const INSTALLER_DOWNLOAD_INITIAL_BACKOFF_MS: u64 = 500;
const INSTALLER_DOWNLOAD_MAX_BACKOFF_MS: u64 = 4_000;

static AGAVE_INSTALL_MIN_VERSION: LazyLock<Version> = LazyLock::new(|| {
    Version::parse(AGAVE_INSTALL_MIN_VERSION_STR)
        .expect("AGAVE_INSTALL_MIN_VERSION_STR must be valid semver")
});

#[derive(Debug, Deserialize)]
struct AnchorSolanaMap {
    entries: Vec<MapEntry>,
}

#[derive(Debug, Deserialize)]
struct MapEntry {
    anchor: String,
    solana: String,
}

#[derive(Debug, Deserialize)]
struct SolanaCliVersions {
    versions: Vec<String>,
}

#[derive(Debug)]
struct ParsedMap {
    entries: Vec<(Version, Version)>,
}

static MAP: LazyLock<ParsedMap> = LazyLock::new(|| {
    let raw: AnchorSolanaMap =
        toml::from_str(ANCHOR_SOLANA_MAP_TOML).expect("Built-in anchor-solana map must parse");

    let mut entries: Vec<(Version, Version)> = raw
        .entries
        .into_iter()
        .map(|e| {
            let anchor = Version::parse(&e.anchor).unwrap_or_else(|err| {
                panic!("Invalid Anchor version `{}` in map: {err}", e.anchor)
            });
            let solana = Version::parse(&e.solana).unwrap_or_else(|err| {
                panic!("Invalid Solana version `{}` in map: {err}", e.solana)
            });
            (anchor, solana)
        })
        .collect();

    let was_sorted = entries.windows(2).all(|w| w[0].0 <= w[1].0);
    assert!(
        was_sorted,
        "anchor-solana-map.toml entries must be sorted by Anchor version"
    );
    let _ = was_sorted;
    entries.sort_by(|a, b| a.0.cmp(&b.0));

    ParsedMap { entries }
});

static INSTALLABLE_SOLANA_CLI_VERSIONS: LazyLock<Vec<Version>> = LazyLock::new(|| {
    let raw: SolanaCliVersions =
        toml::from_str(SOLANA_CLI_VERSIONS_TOML).expect("Built-in Solana CLI versions must parse");

    let versions = raw
        .versions
        .into_iter()
        .map(|v| {
            Version::parse(&v)
                .unwrap_or_else(|err| panic!("Invalid Solana CLI version `{v}`: {err}"))
        })
        .collect::<Vec<_>>();

    let was_sorted = versions.windows(2).all(|w| w[0] < w[1]);
    assert!(
        was_sorted,
        "solana-cli-versions.toml entries must be sorted by semver"
    );
    let _ = was_sorted;

    versions
});

/// Where a resolved Solana CLI version came from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SolanaCliResolutionSource {
    /// A project-pinned Solana version, either from `Anchor.toml` or
    /// `solana-program` in `Cargo.toml`.
    Project(SolanaResolutionSource),
    /// Derived from a resolved Anchor version through the static map.
    AnchorMap {
        anchor: Version,
        anchor_source: ResolutionSource,
    },
}

impl SolanaCliResolutionSource {
    pub fn describe(&self) -> String {
        match self {
            Self::Project(source) => source.describe(),
            Self::AnchorMap {
                anchor,
                anchor_source,
            } => format!(
                "recommended Solana for anchor {anchor} ({})",
                anchor_source.describe()
            ),
        }
    }
}

#[derive(Debug, Clone)]
pub struct SolanaCliResolution {
    pub version: Version,
    pub source: SolanaCliResolutionSource,
}

/// Which upstream installer manages the requested Solana CLI version.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SolanaInstaller {
    SolanaInstall,
    AgaveInstall,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum InstallerSetup {
    CommandAvailable,
    RequestedVersionInstalled,
}

impl SolanaInstaller {
    pub fn command(self) -> &'static str {
        match self {
            Self::SolanaInstall => "solana-install",
            Self::AgaveInstall => "agave-install",
        }
    }

    pub fn domain(self) -> &'static str {
        "anza.xyz"
    }

    pub fn install_url(self, version: &Version) -> String {
        format!("https://release.{}/v{version}/install", self.domain())
    }
}

/// Resolve the Solana CLI version AVM should install for `start`.
///
/// Precedence:
/// 1. Project Solana pin (`[toolchain] solana_version`, then `solana-program`).
/// 2. Resolved Anchor version mapped through `anchor-solana-map.toml`.
pub fn resolve_solana_cli(start: &Path) -> Result<Option<SolanaCliResolution>> {
    let installed = read_installed_versions().unwrap_or_default();
    resolve_solana_cli_with(start, &installed, current_version().ok())
}

/// Resolve the Solana CLI version for an already-resolved Anchor CLI.
///
/// This is used by the AVM `anchor` proxy so it installs Solana for the exact
/// Anchor version it is about to spawn, while still letting project Solana pins
/// take precedence.
pub fn resolve_solana_cli_for_anchor_resolution(
    start: &Path,
    anchor_res: &Resolution,
) -> Result<Option<SolanaCliResolution>> {
    if let Some(res) = resolve_project_solana_cli(start)? {
        return Ok(Some(res));
    }

    resolve_solana_from_anchor_resolution(anchor_res).map(Some)
}

fn resolve_solana_cli_with(
    start: &Path,
    installed_anchor_versions: &[Version],
    global_anchor_default: Option<Version>,
) -> Result<Option<SolanaCliResolution>> {
    if let Some(res) = resolve_project_solana_cli(start)? {
        return Ok(Some(res));
    }

    let Some(anchor_res) =
        resolve_anchor_version_with(start, installed_anchor_versions, global_anchor_default)?
    else {
        return Ok(None);
    };

    resolve_solana_from_anchor_resolution(&anchor_res).map(Some)
}

fn resolve_project_solana_cli(start: &Path) -> Result<Option<SolanaCliResolution>> {
    let Some(res) = resolve_solana_version(start)? else {
        return Ok(None);
    };
    let version = project_solana_cli_version(&res)?;
    Ok(Some(SolanaCliResolution {
        version,
        source: SolanaCliResolutionSource::Project(res.source),
    }))
}

fn project_solana_cli_version(res: &SolanaResolution) -> Result<Version> {
    match res.version_req.as_deref() {
        Some(req) => resolve_installable_solana_cli_req(req, &res.source),
        _ => Ok(res.version.clone()),
    }
}

fn resolve_installable_solana_cli_req(
    req_str: &str,
    source: &SolanaResolutionSource,
) -> Result<Version> {
    installable_solana_cli_versions_for_req(req_str, source).map(|versions| {
        versions
            .last()
            .expect("installable_solana_cli_versions_for_req returns non-empty matches")
            .clone()
    })
}

pub(crate) fn installable_solana_cli_versions_for_req(
    req_str: &str,
    source: &SolanaResolutionSource,
) -> Result<Vec<Version>> {
    let req = VersionReq::parse(req_str)
        .with_context(|| format!("Parsing Solana version requirement `{req_str}`"))?;
    let versions = INSTALLABLE_SOLANA_CLI_VERSIONS
        .iter()
        .filter(|version| req.matches(version))
        .cloned()
        .collect::<Vec<_>>();
    if versions.is_empty() {
        bail!(
            "No installable Solana CLI version hosted by Anza satisfies Solana requirement \
             `{req_str}` from {}. Pin `[toolchain] solana_version` in `Anchor.toml` to an exact \
             hosted version to choose manually.",
            source.describe()
        );
    }
    Ok(versions)
}

fn resolve_solana_from_anchor_resolution(anchor_res: &Resolution) -> Result<SolanaCliResolution> {
    let Some(solana) = lookup_solana_for_anchor_version(&anchor_res.version) else {
        let earliest = earliest_mapped_anchor_version();
        bail!(
            "No Solana CLI mapping exists for Anchor {}. The earliest mapped Anchor release is \
             {earliest}. Pin `[toolchain] solana_version` in `Anchor.toml` to choose manually.",
            anchor_res.version
        );
    };

    Ok(SolanaCliResolution {
        version: solana,
        source: SolanaCliResolutionSource::AnchorMap {
            anchor: anchor_res.version.clone(),
            anchor_source: anchor_res.source.clone(),
        },
    })
}

/// Map an Anchor version to Anchor's recommended Solana CLI version.
///
/// Pre-release Anchor versions use their release triplet for lookup, e.g.
/// `1.0.0-rc.1` maps as `1.0.0`.
pub fn lookup_solana_for_anchor_version(anchor: &Version) -> Option<Version> {
    let anchor = Version::new(anchor.major, anchor.minor, anchor.patch);
    MAP.entries
        .iter()
        .rposition(|(floor, _)| floor <= &anchor)
        .map(|idx| MAP.entries[idx].1.clone())
}

fn earliest_mapped_anchor_version() -> &'static Version {
    &MAP.entries
        .first()
        .expect("anchor-solana map must have at least one entry")
        .0
}

/// Return the upstream installer that owns `version`.
pub fn installer_for_version(version: &Version) -> SolanaInstaller {
    if version < &*AGAVE_INSTALL_MIN_VERSION {
        SolanaInstaller::SolanaInstall
    } else {
        SolanaInstaller::AgaveInstall
    }
}

/// Install and activate a Solana CLI version using `solana-install` or
/// `agave-install`, matching Anchor CLI's installer split at `1.18.19`.
pub fn install_solana_cli(version: &Version, force: bool) -> Result<()> {
    install_solana_cli_with_options(version, force, true, true)
}

/// Ensure a Solana CLI version is active, without printing on no-op.
///
/// The AVM `anchor` proxy uses this for transparent setup before spawning the
/// resolved `anchor-cli` binary.
pub fn ensure_solana_cli(version: &Version) -> Result<()> {
    install_solana_cli_with_options(version, false, false, false)
}

fn install_solana_cli_with_options(
    version: &Version,
    force: bool,
    report_already_active: bool,
    report_success: bool,
) -> Result<()> {
    let installer = installer_for_version(version);
    if !force && read_command_version("solana")?.as_ref() == Some(version) {
        if report_already_active {
            println!("solana {version} is already active");
        }
        return Ok(());
    }

    if ensure_installer_command(version, installer)? == InstallerSetup::RequestedVersionInstalled {
        if report_success {
            println!("Now using Solana {version} via `{}`.", installer.command());
        }
        return Ok(());
    }

    let installed = match read_installed_solana_versions(installer) {
        Ok(installed) => installed,
        Err(err) => {
            eprintln!(
                "Failed to list installed Solana versions with `{}`; continuing as if none are \
                 installed: {err:#}",
                installer.command()
            );
            Vec::new()
        }
    };
    let quiet = installed.iter().any(|installed| installed == version);
    let mut cmd = Command::new(installer.command());
    cmd.arg("init").arg(version.to_string());
    if quiet {
        cmd.stdout(Stdio::null()).stderr(Stdio::null());
    } else {
        cmd.stdout(Stdio::inherit()).stderr(Stdio::inherit());
    }

    let status = cmd
        .status()
        .with_context(|| format!("Running `{}`", installer.command()))?;
    if !status.success() {
        if installer == SolanaInstaller::SolanaInstall {
            install_legacy_solana_from_github_release(
                version,
                format!(
                    "Failed to activate Solana {version} with `{}` (status {status})",
                    installer.command()
                ),
            )?;
            if report_success {
                println!("Now using Solana {version} via `{}`.", installer.command());
            }
            return Ok(());
        }
        bail!(
            "Failed to activate Solana {version} with `{}`",
            installer.command()
        );
    }

    if report_success {
        println!("Now using Solana {version} via `{}`.", installer.command());
    }
    Ok(())
}

fn ensure_installer_command(
    version: &Version,
    installer: SolanaInstaller,
) -> Result<InstallerSetup> {
    if installer_command_available(installer)? {
        return Ok(InstallerSetup::CommandAvailable);
    }

    let command = installer.command();
    let url = installer.install_url(version);
    eprintln!("Command not installed: `{command}`. Installing from {url}");

    let script = match download_installer_script(&url) {
        Ok(script) => script,
        Err(err) if installer == SolanaInstaller::SolanaInstall => {
            install_legacy_solana_from_github_release(version, format!("{url}: {err}"))?;
            return Ok(InstallerSetup::RequestedVersionInstalled);
        }
        Err(err) => return Err(err),
    };
    let status = Command::new("sh")
        .arg("-c")
        .arg(script)
        .status()
        .with_context(|| format!("Running installer from {url}"))?;
    if !status.success() {
        if installer == SolanaInstaller::SolanaInstall {
            install_legacy_solana_from_github_release(
                version,
                format!("Failed to install `{command}` from {url} (status {status})"),
            )?;
            return Ok(InstallerSetup::RequestedVersionInstalled);
        }
        bail!("Failed to install `{command}` from {url}");
    }
    let active_solana = read_command_version("solana")?;
    match installer_setup_after_bootstrap(
        installer_command_available(installer)?,
        active_solana.as_ref(),
        version,
    ) {
        Some(setup) => Ok(setup),
        None => {
            let active = active_solana
                .map(|version| format!("active `solana` is {version}"))
                .unwrap_or_else(|| "active `solana` is unavailable".to_string());
            bail!(
                "Ran installer from {url}, but `{command}` is still unavailable and {active}; \
                 cannot activate Solana {version}"
            );
        }
    }
}

fn installer_setup_after_bootstrap(
    command_available: bool,
    active_solana_version: Option<&Version>,
    requested: &Version,
) -> Option<InstallerSetup> {
    if active_solana_version == Some(requested) {
        Some(InstallerSetup::RequestedVersionInstalled)
    } else if command_available {
        Some(InstallerSetup::CommandAvailable)
    } else {
        None
    }
}

fn install_legacy_solana_from_github_release(
    version: &Version,
    primary_error: String,
) -> Result<()> {
    let target = legacy_solana_release_target(std::env::consts::OS, std::env::consts::ARCH)?;
    let url = legacy_solana_release_url(version, target);
    eprintln!("Installing legacy Solana {version} from GitHub release asset {url}");

    let installer = std::env::temp_dir().join(format!("solana-install-init-{target}-{version}"));
    let bytes = download_installer_bytes(&url)
        .with_context(|| format!("Primary installer error: {primary_error}"))?;
    fs::write(&installer, bytes)
        .with_context(|| format!("Writing legacy Solana installer to {}", installer.display()))?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&installer, fs::Permissions::from_mode(0o755)).with_context(|| {
            format!(
                "Marking legacy Solana installer executable at {}",
                installer.display()
            )
        })?;
    }

    let data_dir = solana_install_data_dir()?;
    let output = Command::new(&installer)
        .arg("--data-dir")
        .arg(&data_dir)
        .arg("--no-modify-path")
        .arg(version.to_string())
        .output()
        .with_context(|| format!("Running legacy Solana installer {}", installer.display()))?;
    if !output.status.success() {
        bail!(
            "Failed to install legacy Solana {version} from {url}:\n{}\nPrimary installer error: \
             {primary_error}",
            command_failure_message(
                installer.to_string_lossy().as_ref(),
                &["--data-dir", "<data-dir>", "--no-modify-path", "<version>"],
                &output,
            )
        );
    }

    Ok(())
}

fn legacy_solana_release_target(os: &str, arch: &str) -> Result<&'static str> {
    match (os, arch) {
        ("macos", "aarch64") => Ok("aarch64-apple-darwin"),
        ("macos", "x86_64") => Ok("x86_64-apple-darwin"),
        ("linux", "x86_64") => Ok("x86_64-unknown-linux-gnu"),
        _ => bail!("Unsupported platform for legacy Solana GitHub release installer: {arch}-{os}"),
    }
}

fn legacy_solana_release_url(version: &Version, target: &str) -> String {
    format!(
        "https://github.com/solana-labs/solana/releases/download/v{version}/\
         solana-install-init-{target}"
    )
}

fn solana_install_data_dir() -> Result<std::path::PathBuf> {
    Ok(dirs::home_dir()
        .ok_or_else(|| anyhow!("Could not find home directory"))?
        .join(".local")
        .join("share")
        .join("solana")
        .join("install"))
}

fn installer_command_available(installer: SolanaInstaller) -> Result<bool> {
    if read_command_version(installer.command())?.is_some() {
        return Ok(true);
    }

    match Command::new(installer.command()).arg("list").output() {
        Ok(output) => Ok(output.status.success()),
        Err(err) if err.kind() == ErrorKind::NotFound => Ok(false),
        Err(err) => Err(err).with_context(|| format!("Running `{}`", installer.command())),
    }
}

fn read_installed_solana_versions(installer: SolanaInstaller) -> Result<Vec<Version>> {
    let output = Command::new(installer.command())
        .arg("list")
        .output()
        .with_context(|| format!("Running `{} list`", installer.command()))?;
    if !output.status.success() {
        bail!(
            "Failed to list installed Solana versions with `{}`:\n{}",
            installer.command(),
            command_failure_message(installer.command(), &["list"], &output)
        );
    }

    let mut text = String::from_utf8_lossy(&output.stdout).into_owned();
    text.push_str(&String::from_utf8_lossy(&output.stderr));
    Ok(parse_versions(&text))
}

fn read_command_version(command: &str) -> Result<Option<Version>> {
    let output = match Command::new(command).arg("--version").output() {
        Ok(output) => output,
        Err(err) if err.kind() == ErrorKind::NotFound => return Ok(None),
        Err(err) => return Err(err).with_context(|| format!("Running `{command} --version`")),
    };
    if !output.status.success() {
        return Ok(None);
    }

    let mut text = String::from_utf8_lossy(&output.stdout).into_owned();
    text.push_str(&String::from_utf8_lossy(&output.stderr));
    Ok(parse_version(&text))
}

fn command_failure_message(command: &str, args: &[&str], output: &Output) -> String {
    command_failure_message_parts(command, args, output.status, &output.stdout, &output.stderr)
}

fn command_failure_message_parts(
    command: &str,
    args: &[&str],
    status: impl Display,
    stdout: &[u8],
    stderr: &[u8],
) -> String {
    let mut command_line = command.to_string();
    for arg in args {
        command_line.push(' ');
        command_line.push_str(arg);
    }

    format!(
        "Command `{command_line}` failed with status {status}\nstdout:\n{}\nstderr:\n{}",
        format_command_output(stdout),
        format_command_output(stderr),
    )
}

fn format_command_output(output: &[u8]) -> String {
    let text = String::from_utf8_lossy(output);
    let text = text.trim();
    if text.is_empty() {
        "(empty)".to_string()
    } else {
        text.to_string()
    }
}

fn download_installer_script(url: &str) -> Result<String> {
    for attempt in 1..=INSTALLER_DOWNLOAD_MAX_ATTEMPTS {
        match try_download_installer_script(url) {
            Ok(script) => return Ok(script),
            Err(err) if err.retryable && attempt < INSTALLER_DOWNLOAD_MAX_ATTEMPTS => {
                let delay = installer_download_backoff(attempt);
                eprintln!(
                    "Failed to download Solana installer script from {url} (attempt \
                     {attempt}/{INSTALLER_DOWNLOAD_MAX_ATTEMPTS}): {}. Retrying in {}ms...",
                    err.error,
                    delay.as_millis()
                );
                thread::sleep(delay);
            }
            Err(err) => {
                if attempt > 1 {
                    return Err(err.error).with_context(|| {
                        format!(
                            "Downloading Solana installer script from {url} failed after \
                             {attempt} attempts"
                        )
                    });
                }
                return Err(err.error);
            }
        }
    }

    Err(anyhow!(
        "Downloading Solana installer script from {url} exhausted retry attempts"
    ))
}

fn download_installer_bytes(url: &str) -> Result<Vec<u8>> {
    for attempt in 1..=INSTALLER_DOWNLOAD_MAX_ATTEMPTS {
        match try_download_installer_bytes(url) {
            Ok(bytes) => return Ok(bytes),
            Err(err) if err.retryable && attempt < INSTALLER_DOWNLOAD_MAX_ATTEMPTS => {
                let delay = installer_download_backoff(attempt);
                eprintln!(
                    "Failed to download Solana installer asset from {url} (attempt \
                     {attempt}/{INSTALLER_DOWNLOAD_MAX_ATTEMPTS}): {}. Retrying in {}ms...",
                    err.error,
                    delay.as_millis()
                );
                thread::sleep(delay);
            }
            Err(err) => {
                if attempt > 1 {
                    return Err(err.error).with_context(|| {
                        format!(
                            "Downloading Solana installer asset from {url} failed after {attempt} \
                             attempts"
                        )
                    });
                }
                return Err(err.error);
            }
        }
    }

    Err(anyhow!(
        "Downloading Solana installer asset from {url} exhausted retry attempts"
    ))
}

struct InstallerDownloadError {
    error: anyhow::Error,
    retryable: bool,
}

fn try_download_installer_script(url: &str) -> std::result::Result<String, InstallerDownloadError> {
    let response = DOWNLOAD_CLIENT
        .get(url)
        .send()
        .map_err(|err| InstallerDownloadError {
            error: anyhow!("Sending GET {url}: {err}"),
            retryable: true,
        })?;
    if !response.status().is_success() {
        let status = response.status();
        return Err(InstallerDownloadError {
            error: anyhow!("Failed to download `{url}` (status {status})"),
            retryable: should_retry_installer_download_status(status),
        });
    }
    response.text().map_err(|err| InstallerDownloadError {
        error: anyhow!("Reading installer script from {url}: {err}"),
        retryable: true,
    })
}

fn try_download_installer_bytes(url: &str) -> std::result::Result<Vec<u8>, InstallerDownloadError> {
    let response = DOWNLOAD_CLIENT
        .get(url)
        .send()
        .map_err(|err| InstallerDownloadError {
            error: anyhow!("Sending GET {url}: {err}"),
            retryable: true,
        })?;
    if !response.status().is_success() {
        let status = response.status();
        return Err(InstallerDownloadError {
            error: anyhow!("Failed to download `{url}` (status {status})"),
            retryable: should_retry_installer_download_status(status),
        });
    }
    response
        .bytes()
        .map(|bytes| bytes.to_vec())
        .map_err(|err| InstallerDownloadError {
            error: anyhow!("Reading installer asset from {url}: {err}"),
            retryable: true,
        })
}

fn should_retry_installer_download_status(status: StatusCode) -> bool {
    status == StatusCode::REQUEST_TIMEOUT
        || status == StatusCode::TOO_MANY_REQUESTS
        || status.is_server_error()
}

fn installer_download_backoff(failed_attempt: usize) -> Duration {
    let shift = failed_attempt.saturating_sub(1).min(10);
    let multiplier = 1u64 << shift;
    let millis = INSTALLER_DOWNLOAD_INITIAL_BACKOFF_MS
        .saturating_mul(multiplier)
        .min(INSTALLER_DOWNLOAD_MAX_BACKOFF_MS);
    Duration::from_millis(millis)
}

fn parse_versions(text: &str) -> Vec<Version> {
    text.lines().filter_map(parse_version).collect()
}

fn parse_version(text: &str) -> Option<Version> {
    text.split_whitespace().find_map(parse_version_token)
}

fn parse_version_token(token: &str) -> Option<Version> {
    let token =
        token.trim_matches(|c: char| !(c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '+')));
    let token = token.strip_prefix('v').unwrap_or(token);
    Version::parse(token).ok()
}

/// Force the map to parse at startup, surfacing embedded-data bugs clearly.
pub fn validate_embedded_map() -> Result<()> {
    let raw: AnchorSolanaMap =
        toml::from_str(ANCHOR_SOLANA_MAP_TOML).context("Parsing embedded anchor-solana map")?;
    if raw.entries.is_empty() {
        bail!("anchor-solana-map.toml must have at least one entry");
    }
    for e in &raw.entries {
        Version::parse(&e.anchor)
            .with_context(|| format!("Invalid Anchor version `{}` in map", e.anchor))?;
        Version::parse(&e.solana)
            .with_context(|| format!("Invalid Solana version `{}` in map", e.solana))?;
    }

    let raw: SolanaCliVersions =
        toml::from_str(SOLANA_CLI_VERSIONS_TOML).context("Parsing embedded Solana CLI versions")?;
    if raw.versions.is_empty() {
        bail!("solana-cli-versions.toml must have at least one entry");
    }
    let versions = raw
        .versions
        .iter()
        .map(|v| Version::parse(v).with_context(|| format!("Invalid Solana CLI version `{v}`")))
        .collect::<Result<Vec<_>>>()?;
    if !versions.windows(2).all(|w| w[0] < w[1]) {
        bail!("solana-cli-versions.toml entries must be sorted by semver");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        std::{fs, path::PathBuf},
        tempfile::TempDir,
    };

    fn v(s: &str) -> Version {
        Version::parse(s).unwrap()
    }

    fn write(p: &Path, contents: &str) {
        if let Some(parent) = p.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(p, contents).unwrap();
    }

    #[test]
    fn embedded_map_parses() {
        validate_embedded_map().unwrap();
    }

    #[test]
    fn lookup_solana_for_anchor_version_uses_floor() {
        assert_eq!(lookup_solana_for_anchor_version(&v("0.28.9")), None);
        assert_eq!(
            lookup_solana_for_anchor_version(&v("0.29.0")).unwrap(),
            v("1.17.25")
        );
        assert_eq!(
            lookup_solana_for_anchor_version(&v("0.30.2")).unwrap(),
            v("1.18.17")
        );
        assert_eq!(
            lookup_solana_for_anchor_version(&v("1.0.0-rc.1")).unwrap(),
            v("3.1.10")
        );
        assert_eq!(
            lookup_solana_for_anchor_version(&v("1.0.2")).unwrap(),
            v("3.1.10")
        );
    }

    #[test]
    fn explicit_project_solana_wins_over_anchor_mapping() {
        let dir = TempDir::new().unwrap();
        write(
            &dir.path().join("Anchor.toml"),
            "[toolchain]\nanchor_version = \"0.31.0\"\nsolana_version = \"2.3.0\"\n",
        );

        let res = resolve_solana_cli_with(dir.path(), &[], None)
            .unwrap()
            .unwrap();

        assert_eq!(res.version, v("2.3.0"));
        assert!(matches!(
            res.source,
            SolanaCliResolutionSource::Project(SolanaResolutionSource::AnchorToml(_))
        ));
    }

    #[test]
    fn toolchain_solana_req_uses_newest_hosted_semver_compatible_cli() {
        let dir = TempDir::new().unwrap();
        write(
            &dir.path().join("Anchor.toml"),
            "[toolchain]\nsolana_version = \"^2.2.1\"\n",
        );

        let res = resolve_solana_cli_with(dir.path(), &[], None)
            .unwrap()
            .unwrap();

        assert_eq!(res.version, v("2.3.13"));
        assert!(matches!(
            res.source,
            SolanaCliResolutionSource::Project(SolanaResolutionSource::AnchorToml(_))
        ));
    }

    #[test]
    fn exact_toolchain_solana_req_stays_exact_when_hosted() {
        let dir = TempDir::new().unwrap();
        write(
            &dir.path().join("Anchor.toml"),
            "[toolchain]\nsolana_version = \"=2.2.1\"\n",
        );

        let res = resolve_solana_cli_with(dir.path(), &[], None)
            .unwrap()
            .unwrap();

        assert_eq!(res.version, v("2.2.1"));
    }

    #[test]
    fn exact_toolchain_solana_req_errors_when_not_hosted() {
        let dir = TempDir::new().unwrap();
        write(
            &dir.path().join("Anchor.toml"),
            "[toolchain]\nsolana_version = \"=1.17.18\"\n",
        );

        let err = resolve_solana_cli_with(dir.path(), &[], None).unwrap_err();

        assert!(err
            .to_string()
            .contains("No installable Solana CLI version hosted by Anza"));
    }

    #[test]
    fn solana_program_req_uses_newest_hosted_semver_compatible_cli() {
        let dir = TempDir::new().unwrap();
        write(&dir.path().join("Anchor.toml"), "");
        write(
            &dir.path().join("programs/foo/Cargo.toml"),
            "[package]\nname = \"foo\"\nversion = \"0.1.0\"\nedition = \"2021\"\n[lib]\npath = \
             \"src/lib.rs\"\n[dependencies]\nsolana-program = \"2.2.1\"\n",
        );
        write(&dir.path().join("programs/foo/src/lib.rs"), "");

        let res = resolve_solana_cli_with(dir.path(), &[], None)
            .unwrap()
            .unwrap();

        assert_eq!(res.version, v("2.3.13"));
        assert!(matches!(
            res.source,
            SolanaCliResolutionSource::Project(SolanaResolutionSource::CargoToml(_))
        ));
    }

    #[test]
    fn old_solana_program_req_uses_hosted_compatible_cli() {
        let dir = TempDir::new().unwrap();
        write(&dir.path().join("Anchor.toml"), "");
        write(
            &dir.path().join("programs/foo/Cargo.toml"),
            "[package]\nname = \"foo\"\nversion = \"0.1.0\"\nedition = \"2021\"\n[lib]\npath = \
             \"src/lib.rs\"\n[dependencies]\nsolana-program = \"1.17.1\"\n",
        );
        write(&dir.path().join("programs/foo/src/lib.rs"), "");

        let res = resolve_solana_cli_with(dir.path(), &[], None)
            .unwrap()
            .unwrap();

        assert_eq!(res.version, v("1.18.26"));
        assert!(matches!(
            res.source,
            SolanaCliResolutionSource::Project(SolanaResolutionSource::CargoToml(_))
        ));
    }

    #[test]
    fn exact_solana_program_req_stays_exact_when_hosted() {
        let dir = TempDir::new().unwrap();
        write(&dir.path().join("Anchor.toml"), "");
        write(
            &dir.path().join("programs/foo/Cargo.toml"),
            "[package]\nname = \"foo\"\nversion = \"0.1.0\"\nedition = \"2021\"\n[lib]\npath = \
             \"src/lib.rs\"\n[dependencies]\nsolana-program = \"=2.2.1\"\n",
        );
        write(&dir.path().join("programs/foo/src/lib.rs"), "");

        let res = resolve_solana_cli_with(dir.path(), &[], None)
            .unwrap()
            .unwrap();

        assert_eq!(res.version, v("2.2.1"));
    }

    #[test]
    fn exact_solana_program_req_errors_when_not_hosted() {
        let dir = TempDir::new().unwrap();
        write(&dir.path().join("Anchor.toml"), "");
        write(
            &dir.path().join("programs/foo/Cargo.toml"),
            "[package]\nname = \"foo\"\nversion = \"0.1.0\"\nedition = \"2021\"\n[lib]\npath = \
             \"src/lib.rs\"\n[dependencies]\nsolana-program = \"=1.17.18\"\n",
        );
        write(&dir.path().join("programs/foo/src/lib.rs"), "");

        let err = resolve_solana_cli_with(dir.path(), &[], None).unwrap_err();

        assert!(err
            .to_string()
            .contains("No installable Solana CLI version hosted by Anza"));
    }

    #[test]
    fn anchor_proxy_resolution_uses_already_resolved_anchor_version() {
        let dir = TempDir::new().unwrap();
        let anchor_res = Resolution {
            version: v("0.31.1"),
            source: ResolutionSource::AnchorToml(PathBuf::from("Anchor.toml")),
        };

        let res = resolve_solana_cli_for_anchor_resolution(dir.path(), &anchor_res)
            .unwrap()
            .unwrap();

        assert_eq!(res.version, v("2.1.0"));
        assert!(matches!(
            res.source,
            SolanaCliResolutionSource::AnchorMap {
                anchor,
                anchor_source: ResolutionSource::AnchorToml(_)
            } if anchor == v("0.31.1")
        ));
    }

    #[test]
    fn anchor_proxy_resolution_still_prefers_project_solana_pin() {
        let dir = TempDir::new().unwrap();
        write(
            &dir.path().join("Anchor.toml"),
            "[toolchain]\nsolana_version = \"2.3.0\"\n",
        );
        let anchor_res = Resolution {
            version: v("0.31.1"),
            source: ResolutionSource::GlobalDefault,
        };

        let res = resolve_solana_cli_for_anchor_resolution(dir.path(), &anchor_res)
            .unwrap()
            .unwrap();

        assert_eq!(res.version, v("2.3.0"));
        assert!(matches!(
            res.source,
            SolanaCliResolutionSource::Project(SolanaResolutionSource::AnchorToml(_))
        ));
    }

    #[test]
    fn derives_solana_from_anchor_toml_version() {
        let dir = TempDir::new().unwrap();
        write(
            &dir.path().join("Anchor.toml"),
            "[toolchain]\nanchor_version = \"0.32.1\"\n",
        );

        let res = resolve_solana_cli_with(dir.path(), &[], None)
            .unwrap()
            .unwrap();

        assert_eq!(res.version, v("2.3.0"));
        assert!(matches!(
            res.source,
            SolanaCliResolutionSource::AnchorMap {
                anchor,
                anchor_source: ResolutionSource::AnchorToml(_)
            } if anchor == v("0.32.1")
        ));
    }

    #[test]
    fn derives_solana_from_anchor_lang_dependency() {
        let dir = TempDir::new().unwrap();
        write(&dir.path().join("Anchor.toml"), "");
        write(
            &dir.path().join("programs/foo/Cargo.toml"),
            "[package]\nname = \"foo\"\nversion = \"0.1.0\"\nedition = \"2021\"\n[lib]\npath = \
             \"src/lib.rs\"\n[dependencies]\nanchor-lang = \"0.31.0\"\n",
        );
        write(&dir.path().join("programs/foo/src/lib.rs"), "");

        let res = resolve_solana_cli_with(dir.path(), &[], None)
            .unwrap()
            .unwrap();

        assert_eq!(res.version, v("2.1.0"));
        assert!(matches!(
            res.source,
            SolanaCliResolutionSource::AnchorMap {
                anchor,
                anchor_source: ResolutionSource::CargoToml(_)
            } if anchor == v("0.31.0")
        ));
    }

    #[test]
    fn derives_solana_from_global_anchor_default() {
        let dir = TempDir::new().unwrap();

        let res = resolve_solana_cli_with(dir.path(), &[], Some(v("1.0.2")))
            .unwrap()
            .unwrap();

        assert_eq!(res.version, v("3.1.10"));
        assert!(matches!(
            res.source,
            SolanaCliResolutionSource::AnchorMap {
                anchor,
                anchor_source: ResolutionSource::GlobalDefault
            } if anchor == v("1.0.2")
        ));
    }

    #[test]
    fn anchor_version_below_map_errors() {
        let dir = TempDir::new().unwrap();
        write(
            &dir.path().join("Anchor.toml"),
            "[toolchain]\nanchor_version = \"0.28.0\"\n",
        );

        let err = resolve_solana_cli_with(dir.path(), &[], None).unwrap_err();
        assert!(err.to_string().contains("No Solana CLI mapping exists"));
    }

    #[test]
    fn installer_for_version_switches_at_agave_cutover() {
        assert_eq!(
            installer_for_version(&v("1.18.18")),
            SolanaInstaller::SolanaInstall
        );
        assert_eq!(
            installer_for_version(&v("1.18.19")),
            SolanaInstaller::AgaveInstall
        );
        assert_eq!(
            installer_for_version(&v("3.1.10")),
            SolanaInstaller::AgaveInstall
        );
    }

    #[test]
    fn installer_urls_match_upstream_domains() {
        assert_eq!(
            SolanaInstaller::SolanaInstall.install_url(&v("1.18.17")),
            "https://release.anza.xyz/v1.18.17/install"
        );
        assert_eq!(
            SolanaInstaller::AgaveInstall.install_url(&v("3.1.10")),
            "https://release.anza.xyz/v3.1.10/install"
        );
    }

    #[test]
    fn legacy_solana_github_release_url_uses_target_asset() {
        assert_eq!(
            legacy_solana_release_url(&v("1.17.18"), "x86_64-unknown-linux-gnu"),
            "https://github.com/solana-labs/solana/releases/download/v1.17.18/\
             solana-install-init-x86_64-unknown-linux-gnu"
        );
    }

    #[test]
    fn legacy_solana_release_target_matches_supported_hosts() {
        assert_eq!(
            legacy_solana_release_target("linux", "x86_64").unwrap(),
            "x86_64-unknown-linux-gnu"
        );
        assert_eq!(
            legacy_solana_release_target("macos", "aarch64").unwrap(),
            "aarch64-apple-darwin"
        );
        assert_eq!(
            legacy_solana_release_target("macos", "x86_64").unwrap(),
            "x86_64-apple-darwin"
        );
        assert!(legacy_solana_release_target("windows", "x86_64").is_err());
    }

    #[test]
    fn parse_versions_extracts_cli_and_installer_output() {
        assert_eq!(
            parse_version("solana-cli 3.1.10 (src:7bc9c805; feat:1620780344, client:Agave)")
                .unwrap(),
            v("3.1.10")
        );
        assert_eq!(
            parse_versions("1.18.17 (current)\nv2.1.0\n3.1.10"),
            vec![v("1.18.17"), v("2.1.0"), v("3.1.10")]
        );
    }

    #[test]
    fn command_failure_message_includes_status_stdout_and_stderr() {
        let msg = command_failure_message_parts(
            "agave-install",
            &["list"],
            "exit status: 1",
            b"",
            b"error: invalid active_release path\n",
        );

        assert!(msg.contains("Command `agave-install list` failed"));
        assert!(msg.contains("status exit status: 1"));
        assert!(msg.contains("stdout:\n(empty)"));
        assert!(msg.contains("stderr:\nerror: invalid active_release path"));
    }

    #[test]
    fn bootstrap_treats_requested_active_solana_as_installed_even_without_command() {
        assert_eq!(
            installer_setup_after_bootstrap(false, Some(&v("1.17.34")), &v("1.17.34")),
            Some(InstallerSetup::RequestedVersionInstalled)
        );
    }

    #[test]
    fn bootstrap_uses_installer_command_when_active_solana_is_different() {
        assert_eq!(
            installer_setup_after_bootstrap(true, Some(&v("3.1.10")), &v("1.17.34")),
            Some(InstallerSetup::CommandAvailable)
        );
    }

    #[test]
    fn bootstrap_errors_when_command_missing_and_active_solana_is_different() {
        assert_eq!(
            installer_setup_after_bootstrap(false, Some(&v("3.1.10")), &v("1.17.34")),
            None
        );
    }

    #[test]
    fn installer_download_status_retry_policy_is_transient_only() {
        assert!(should_retry_installer_download_status(
            StatusCode::REQUEST_TIMEOUT
        ));
        assert!(should_retry_installer_download_status(
            StatusCode::TOO_MANY_REQUESTS
        ));
        assert!(should_retry_installer_download_status(
            StatusCode::BAD_GATEWAY
        ));
        assert!(!should_retry_installer_download_status(
            StatusCode::NOT_FOUND
        ));
        assert!(!should_retry_installer_download_status(
            StatusCode::BAD_REQUEST
        ));
    }

    #[test]
    fn installer_download_backoff_grows_and_caps() {
        assert_eq!(installer_download_backoff(1), Duration::from_millis(500));
        assert_eq!(installer_download_backoff(2), Duration::from_millis(1_000));
        assert_eq!(installer_download_backoff(3), Duration::from_millis(2_000));
        assert_eq!(installer_download_backoff(4), Duration::from_millis(4_000));
        assert_eq!(
            installer_download_backoff(99),
            Duration::from_millis(INSTALLER_DOWNLOAD_MAX_BACKOFF_MS)
        );
    }
}
