use {
    anyhow::{anyhow, bail, Context, Error, Result},
    cargo_toml::Manifest,
    chrono::{TimeZone, Utc},
    reqwest::{header::USER_AGENT, StatusCode},
    semver::{Prerelease, Version},
    serde::{de, Deserialize},
    std::{
        fs,
        io::{BufRead, Write},
        path::PathBuf,
        process::{Command, Stdio},
        sync::LazyLock,
    },
};

/// Checked at most once per hour.
const UPDATE_CHECK_INTERVAL_SECS: i64 = 60 * 60;
/// Shorter HTTP timeout so a slow or unreachable GitHub does not stall the CLI for long.
const HTTP_CLIENT_TIMEOUT_SECS: u64 = 5;
/// Longer timeout for release asset downloads, which can take longer than metadata requests.
const DOWNLOAD_CLIENT_TIMEOUT_SECS: u64 = 60;

/// Shared HTTP client with a short timeout, used for metadata/API requests.
static HTTP_CLIENT: LazyLock<reqwest::blocking::Client> = LazyLock::new(|| {
    reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(HTTP_CLIENT_TIMEOUT_SECS))
        .build()
        .expect("Failed to build HTTP client")
});

/// Shared HTTP client with a longer timeout, used for release asset downloads.
static DOWNLOAD_CLIENT: LazyLock<reqwest::blocking::Client> = LazyLock::new(|| {
    reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(DOWNLOAD_CLIENT_TIMEOUT_SECS))
        .build()
        .expect("Failed to build download HTTP client")
});

/// Storage directory for AVM, customizable by setting the $AVM_HOME, defaults to ~/.avm
pub static AVM_HOME: LazyLock<PathBuf> = LazyLock::new(|| {
    cfg_if::cfg_if! {
        if #[cfg(test)] {
            let dir = tempfile::tempdir().expect("Could not create temporary directory");
            dir.path().join(".avm")
        } else {
            if let Ok(avm_home) = std::env::var("AVM_HOME") {
                PathBuf::from(avm_home)
            } else {
                let mut user_home = dirs::home_dir().expect("Could not find home directory");
                user_home.push(".avm");
                user_home
            }
        }
    }
});

/// Path to the current version file $AVM_HOME/.version
fn current_version_file_path() -> PathBuf {
    AVM_HOME.join(".version")
}

/// Path to the current version file $AVM_HOME/bin
pub fn get_bin_dir_path() -> PathBuf {
    AVM_HOME.join("bin")
}

/// Path to the temporary folder for cargo install
pub fn get_tmp_install_dir_path() -> PathBuf {
    AVM_HOME.join("tmp")
}

/// Path to the temporary bin folder of cargo install
pub fn get_tmp_bin_dir_path() -> PathBuf {
    AVM_HOME.join("tmp").join("bin")
}

/// Path to the binary for the given version
pub fn version_binary_path(version: &Version) -> PathBuf {
    get_bin_dir_path().join(format!("anchor-{version}"))
}

/// Path to the cargo binary directory, defaults to `~/.cargo/bin` if `CARGO_HOME`
#[cfg(not(test))] // this prevents tests from running this function so we don't change the developer environment during tests
fn cargo_bin_dir() -> Option<PathBuf> {
    if let Ok(cargo_home) = std::env::var("CARGO_HOME") {
        return Some(PathBuf::from(cargo_home).join("bin"));
    }
    dirs::home_dir().map(|home| home.join(".cargo").join("bin"))
}

/// Ensure the users home directory is setup with the paths required by AVM.
pub fn ensure_paths() {
    let home_dir = AVM_HOME.to_path_buf();
    if !home_dir.exists() {
        fs::create_dir_all(&home_dir).expect("Could not create .avm directory");
    }

    let bin_dir = get_bin_dir_path();
    if !bin_dir.exists() {
        fs::create_dir_all(&bin_dir).expect("Could not create .avm/bin directory");
    }

    // Copy the `avm` binary to `~/.avm/bin` so we can create symlinks to it.
    let avm_in_bin = bin_dir.join("avm");
    if let Ok(current_avm) = std::env::current_exe() {
        // Only copy if the paths are different
        if current_avm != avm_in_bin {
            if let Err(e) = fs::copy(current_avm, &avm_in_bin) {
                eprintln!("Failed to copy avm binary: {e}");
            }
        }
    }

    // Create a symlink from `anchor` to `avm` so that the user can run `anchor`
    // from the command line.
    #[cfg(unix)]
    {
        let anchor_in_bin = bin_dir.join("anchor");
        if !anchor_in_bin.exists() {
            if let Err(e) = std::os::unix::fs::symlink(&avm_in_bin, anchor_in_bin) {
                eprintln!("Failed to create symlink: {e}");
            }
        }
    }

    // On Windows, we create a symlink named `anchor.exe` pointing to the `avm.exe` binary in the bin directory,
    // so that the user can run `anchor` from the command line.
    // Note: Creating symlinks on Windows may require administrator privileges or that Developer Mode is enabled.

    #[cfg(windows)]
    {
        use std::os::windows::fs::symlink_file;
        let anchor_in_bin = bin_dir.join("anchor.exe");
        if !anchor_in_bin.exists() {
            if let Err(e) = symlink_file(&avm_in_bin, &anchor_in_bin) {
                eprintln!("Failed to create symlink: {}", e);
            }
        }
    }

    // Try to make `anchor` available on PATH by placing it into $CARGO_HOME/bin (or ~/.cargo/bin).
    #[cfg(not(test))]
    {
        if let Some(cargo_bin) = cargo_bin_dir() {
            if cargo_bin.exists() {
                let anchor_in_cargo = cargo_bin.join(if cfg!(target_os = "windows") {
                    "anchor.exe"
                } else {
                    "anchor"
                });
                if !anchor_in_cargo.exists() {
                    let target = avm_in_bin.clone(); // ~/.avm/bin/avm

                    let mut linked = false;
                    #[cfg(unix)]
                    {
                        if let Err(e) = std::os::unix::fs::symlink(&target, &anchor_in_cargo) {
                            eprintln!(
                                "Failed to create cargo-bin symlink: {e}. Falling back to copy."
                            );
                        } else {
                            linked = true;
                        }
                    }
                    #[cfg(windows)]
                    {
                        use std::os::windows::fs::symlink_file;
                        if let Err(e) = symlink_file(&target, &anchor_in_cargo) {
                            eprintln!(
                                "Failed to create cargo-bin symlink: {e}. Falling back to copy."
                            );
                        } else {
                            linked = true;
                        }
                    }

                    if !linked {
                        if let Err(e) = fs::copy(&target, &anchor_in_cargo) {
                            eprintln!(
                                "Failed to place `anchor` in {}: {}.\nAdd {} to your PATH or \
                                 create a symlink manually.",
                                cargo_bin.display(),
                                e,
                                bin_dir.display()
                            );
                        } else {
                            // Ensure executable bit on UNIX when copying.
                            #[cfg(unix)]
                            {
                                use std::os::unix::fs::PermissionsExt;
                                if let Err(e) = fs::set_permissions(
                                    &anchor_in_cargo,
                                    fs::Permissions::from_mode(0o775),
                                ) {
                                    eprintln!(
                                        "Failed to set executable permissions on {}: {}",
                                        anchor_in_cargo.display(),
                                        e
                                    );
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    if !current_version_file_path().exists() {
        fs::File::create(current_version_file_path()).expect("Could not create .version file");
    }
}

/// Read the current version from the version file
pub fn current_version() -> Result<Version> {
    fs::read_to_string(current_version_file_path())
        .map_err(|e| anyhow!("Could not read version file: {}", e))?
        .trim_end_matches('\n')
        .parse::<Version>()
        .map_err(|e| anyhow!("Could not parse version file: {}", e))
}

/// Update the current version to a new version
pub fn use_version(opt_version: Option<Version>) -> Result<()> {
    let version = match opt_version {
        Some(version) => version,
        None => read_anchorversion_file()?,
    };

    // Make sure the requested version is installed
    let installed_versions = read_installed_versions()?;
    if !installed_versions.contains(&version) {
        println!("Version {version} is not installed. Would you like to install? [y/n]");
        let input = std::io::stdin()
            .lock()
            .lines()
            .next()
            .expect("Expected input")?;
        match input.as_str() {
            "y" | "yes" => {
                return install_version(InstallTarget::Version(version), false, false, false)
            }
            _ => return Err(anyhow!("Installation rejected.")),
        };
    }

    let mut current_version_file = fs::File::create(current_version_file_path())?;
    current_version_file.write_all(version.to_string().as_bytes())?;
    println!("Now using anchor version {}.", current_version()?);
    Ok(())
}

#[derive(Clone)]
pub enum InstallTarget {
    Version(Version),
    Commit(String),
    Path(PathBuf),
}

/// Update to the latest version
pub fn update(include_pre_release: bool) -> Result<()> {
    let latest_version = get_latest_version(include_pre_release)?;
    install_version(InstallTarget::Version(latest_version), false, false, false)
}

/// The commit sha provided can be shortened,
///
/// returns the full commit sha3 for unique versioning downstream
pub fn check_and_get_full_commit(commit: &str) -> Result<String> {
    let response = HTTP_CLIENT
        .get(format!(
            "https://api.github.com/repos/otter-sec/anchor/commits/{commit}"
        ))
        .header(
            USER_AGENT,
            "avm https://github.com/otter-sec/anchor",
        )
        .send()?;

    if response.status() != StatusCode::OK {
        return Err(anyhow!(
            "Error checking commit {commit}: {}",
            response.text()?
        ));
    };

    #[derive(Deserialize)]
    struct GetCommitResponse {
        sha: String,
    }

    response
        .json::<GetCommitResponse>()
        .map(|resp| resp.sha)
        .map_err(|err| anyhow!("Failed to parse the response to JSON: {err:?}"))
}

fn fetch_raw(client: &reqwest::blocking::Client, url: &str) -> Result<Option<String>> {
    let response = client
        .get(url)
        .header(
            USER_AGENT,
            "avm https://github.com/otter-sec/anchor",
        )
        .send()?;
    if response.status() == StatusCode::OK {
        Ok(Some(response.text()?))
    } else {
        Ok(None)
    }
}

/// Append `commit` to the version's pre-release field, preserving any existing pre-release info.
/// e.g. `1.0.0-rc.3` + `e1afcbf7...` → `1.0.0-rc.3.e1afcbf7...`
///      `0.28.0`      + `e1afcbf7...` → `0.28.0-e1afcbf7...`
fn append_commit(version: &mut Version, commit: &str) -> Result<()> {
    let pre_str = if version.pre.is_empty() {
        commit.to_string()
    } else {
        format!("{}-{commit}", version.pre)
    };
    version.pre = Prerelease::new(&pre_str)?;
    Ok(())
}

fn get_anchor_version_from_commit(commit: &str) -> Result<Version> {
    let base = format!("https://raw.githubusercontent.com/otter-sec/anchor/{commit}");

    // Newer versions (workspace layout): version lives in [workspace.package] of the root Cargo.toml.
    if let Some(text) = fetch_raw(&HTTP_CLIENT, &format!("{base}/Cargo.toml"))? {
        if let Ok(manifest) = Manifest::from_str(&text) {
            if let Some(version_str) = manifest
                .workspace
                .as_ref()
                .and_then(|ws| ws.package.as_ref())
                .and_then(|pkg| pkg.version.as_deref())
            {
                let mut version = version_str.parse::<Version>()?;
                append_commit(&mut version, commit)?;
                return Ok(version);
            }
        }
    }

    // Older versions: version lives in [package] of cli/Cargo.toml.
    let text = fetch_raw(&HTTP_CLIENT, &format!("{base}/cli/Cargo.toml"))?
        .ok_or_else(|| anyhow!("Could not find anchor-cli version for commit {commit}"))?;
    let manifest = Manifest::from_str(&text)?;
    let mut version = manifest.package().version().parse::<Version>()?;
    append_commit(&mut version, commit)?;

    Ok(version)
}

/// Install a version of anchor-cli
pub fn install_version(
    install_target: InstallTarget,
    force: bool,
    from_source: bool,
    with_solana_verify: bool,
) -> Result<()> {
    let (version, from_source) = match &install_target {
        InstallTarget::Version(version) => (version.to_owned(), from_source),
        InstallTarget::Commit(commit) => (get_anchor_version_from_commit(commit)?, true),
        InstallTarget::Path(path) => {
            let manifest_path = path.join("cli/Cargo.toml");
            let manifest = Manifest::from_path(&manifest_path).map_err(|e| {
                anyhow!(
                    "Failed to read manifest at {}: {}",
                    manifest_path.display(),
                    e
                )
            })?;
            let version = manifest.package().version().parse::<Version>()?;
            (version, true)
        }
    };
    // Return early if version is already installed
    if !force && read_installed_versions()?.contains(&version) {
        eprintln!("Version `{version}` is already installed");
        return Ok(());
    }

    let is_commit = matches!(install_target, InstallTarget::Commit(_));
    let is_older_than_v0_31_0 = version < Version::new(0, 31, 0);
    if from_source || is_commit || is_older_than_v0_31_0 {
        // Build from source using `cargo install`
        let mut args: Vec<String> = vec![
            "install".into(),
            "anchor-cli".into(),
            "--locked".into(),
            "--root".into(),
            // can't install directly to `.avm/` because additional symlinks were
            // added to the .avm/bin folder that can cause cargo to error out during installs
            // simply removing the creation of those links would not remove them from user machines
            // and we don't want to remove them because they may have been created by the user
            get_tmp_install_dir_path().to_str().unwrap().into(),
        ];
        match install_target {
            InstallTarget::Version(version) => {
                args.extend_from_slice(&[
                    "--git".into(),
                    "https://github.com/otter-sec/anchor".into(),
                    "--tag".into(),
                    format!("v{version}"),
                ]);
            }
            InstallTarget::Commit(commit) => {
                args.extend_from_slice(&[
                    "--git".into(),
                    "https://github.com/otter-sec/anchor".into(),
                    "--rev".into(),
                    commit,
                ]);
            }
            InstallTarget::Path(path) => {
                let cli_path = path.join("cli");
                let path_str = cli_path
                    .to_str()
                    .ok_or_else(|| anyhow!("Invalid path string"))?;
                args.extend_from_slice(&[
                    "--path".into(),
                    path_str.to_string(),
                    "--bin".into(),
                    "anchor".into(),
                ]);
            }
        }

        // If the version is older than v0.31, install using `rustc 1.79.0` to get around the problem
        // explained in https://github.com/otter-sec/anchor/pull/3143
        if is_older_than_v0_31_0 {
            const REQUIRED_VERSION: &str = "1.79.0";
            let is_installed = Command::new("rustup")
                .args(["toolchain", "list"])
                .output()
                .map(|output| String::from_utf8(output.stdout))??
                .lines()
                .any(|line| line.starts_with(REQUIRED_VERSION));
            if !is_installed {
                let exit_status = Command::new("rustup")
                    .args(["toolchain", "install", REQUIRED_VERSION])
                    .spawn()?
                    .wait()?;
                if !exit_status.success() {
                    return Err(anyhow!(
                        "Installation of `rustc {REQUIRED_VERSION}` failed. \
                    `rustc <1.80` is required to install Anchor v{version} from source. \
                    See https://github.com/otter-sec/anchor/pull/3143 for more information."
                    ));
                }
            }

            // Prepend the toolchain to use with the `cargo install` command
            args.insert(0, format!("+{REQUIRED_VERSION}"));
        }

        let output = Command::new("cargo")
            .args(args)
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit())
            .output()
            .map_err(|e| anyhow!("`cargo install` for version `{version}` failed: {e}"))?;
        if !output.status.success() {
            return Err(anyhow!(
                "Failed to install {version}, is it a valid version?"
            ));
        }

        let bin_dir = get_tmp_bin_dir_path();
        let bin_name = if cfg!(target_os = "windows") {
            "anchor.exe"
        } else {
            "anchor"
        };
        fs::rename(bin_dir.join(bin_name), version_binary_path(&version))?;
    } else {
        let output = Command::new("rustc").arg("-vV").output()?;
        let target = core::str::from_utf8(&output.stdout)?
            .lines()
            .find(|line| line.starts_with("host:"))
            .and_then(|line| line.split(':').next_back())
            .ok_or_else(|| anyhow!("`host` not found from `rustc -vV` output"))?
            .trim();
        let ext = if cfg!(target_os = "windows") {
            ".exe"
        } else {
            ""
        };
        let res = DOWNLOAD_CLIENT
            .get(format!(
                "https://github.com/otter-sec/anchor/releases/download/v{version}/anchor-{version}-{target}{ext}"
            ))
            .send()?;
        if !res.status().is_success() {
            return Err(anyhow!(
                "Failed to download the binary for version `{version}` (status code: {})",
                res.status()
            ));
        }

        let bin_path = version_binary_path(&version);
        fs::write(&bin_path, res.bytes()?)?;

        // Set file to executable on UNIX
        #[cfg(unix)]
        fs::set_permissions(
            bin_path,
            <fs::Permissions as std::os::unix::fs::PermissionsExt>::from_mode(0o775),
        )?;
    }

    let is_at_least_0_32 = version >= Version::new(0, 32, 0);
    if with_solana_verify {
        if is_at_least_0_32 {
            if !solana_verify_installed().is_ok_and(|v| v) {
                #[cfg(any(target_os = "linux", target_os = "macos"))]
                install_solana_verify()?;
                #[cfg(not(any(target_os = "linux", target_os = "macos")))]
                install_solana_verify_from_source()?;
                println!("solana-verify successfully installed");
            } else {
                println!("solana-verify already installed");
            }
        } else {
            println!("Not installing solana-verify for anchor < 0.32");
        }
    }

    // If .version file is empty or not parseable, write the newly installed version to it
    if current_version().is_err() {
        let mut current_version_file = fs::File::create(current_version_file_path())?;
        current_version_file.write_all(version.to_string().as_bytes())?;
    }

    use_version(Some(version))
}

const SOLANA_VERIFY_VERSION: Version = Version::new(0, 4, 11);

/// Check if `solana-verify` is both installed and >= [`SOLANA_VERIFY_VERSION`].
fn solana_verify_installed() -> Result<bool> {
    let bin_path = get_bin_dir_path().join("solana-verify");
    if !bin_path.exists() {
        return Ok(false);
    }
    let output = Command::new(bin_path)
        .arg("-V")
        .output()
        .context("executing `solana-verify` to check version")?;
    let stdout =
        String::from_utf8(output.stdout).context("expected `solana-verify` to output utf8")?;
    let Some(("solana-verify", version)) = stdout.trim().split_once(" ") else {
        bail!("invalid `solana-verify` output: `{stdout}`");
    };
    if Version::parse(version).with_context(|| "parsing solana-verify version `{version}")?
        >= SOLANA_VERIFY_VERSION
    {
        Ok(true)
    } else {
        Ok(false)
    }
}

/// Install `solana-verify` from binary releases. Only available on Linux and Mac
#[cfg(any(target_os = "linux", target_os = "macos"))]
fn install_solana_verify() -> Result<()> {
    println!("Installing solana-verify...");
    let os = std::env::consts::OS;
    let url = format!(
        "https://github.com/Ellipsis-Labs/solana-verifiable-build/releases/download/v{SOLANA_VERIFY_VERSION}/solana-verify-{os}"
    );
    let res = DOWNLOAD_CLIENT.get(url).send()?;
    if !res.status().is_success() {
        bail!(
            "Failed to download `solana-verify-{os} v{SOLANA_VERIFY_VERSION} (status code: {})",
            res.status()
        );
    } else {
        let bin_path = get_bin_dir_path().join("solana-verify");
        fs::write(&bin_path, res.bytes()?)?;
        #[cfg(unix)]
        fs::set_permissions(
            bin_path,
            <fs::Permissions as std::os::unix::fs::PermissionsExt>::from_mode(0o775),
        )?;
        Ok(())
    }
}

/// Install `solana-verify` by building from Git sources
#[cfg(not(any(target_os = "linux", target_os = "macos")))]
fn install_solana_verify_from_source() -> Result<()> {
    println!("Installing solana-verify from source...");
    let status = Command::new("cargo")
        .args([
            "install",
            "solana-verify",
            "--git",
            "https://github.com/Ellipsis-Labs/solana-verifiable-build",
            "--rev",
            &format!("v{SOLANA_VERIFY_VERSION}"),
            "--root",
            AVM_HOME.to_str().unwrap(),
            "--force",
            "--locked",
        ])
        .status()
        .context("executing `cargo install solana-verify`")?;
    if status.success() {
        Ok(())
    } else {
        bail!("failed to install `solana-verify`");
    }
}

/// Remove an installed version of anchor-cli
pub fn uninstall_version(version: &Version) -> Result<()> {
    let version_path = version_binary_path(version);
    if !version_path.exists() {
        return Err(anyhow!("anchor-cli {} is not installed", version));
    }
    if version == &current_version()? {
        return Err(anyhow!("anchor-cli {} is currently in use", version));
    }
    fs::remove_file(version_path)?;

    Ok(())
}

/// Read version from .anchorversion
pub fn read_anchorversion_file() -> Result<Version> {
    fs::read_to_string(".anchorversion")
        .map_err(|e| anyhow!(".anchorversion file not found: {e}"))
        .map(|content| Version::parse(content.trim()))?
        .map_err(|e| anyhow!("Unable to parse version: {e}"))
}

/// Retrieve a list of installable versions of anchor-cli using the GitHub API and tags on the Anchor
/// repository.
pub fn fetch_versions(include_pre_release: bool) -> Result<Vec<Version>, Error> {
    fetch_versions_with_client(&HTTP_CLIENT, include_pre_release)
}

fn fetch_versions_with_client(
    client: &reqwest::blocking::Client,
    include_pre_release: bool,
) -> Result<Vec<Version>, Error> {
    #[derive(Deserialize)]
    struct Release {
        #[serde(rename = "name", deserialize_with = "version_deserializer")]
        version: Version,
        draft: bool,
        prerelease: bool,
    }

    fn version_deserializer<'de, D>(deserializer: D) -> Result<Version, D::Error>
    where
        D: de::Deserializer<'de>,
    {
        let s: &str = de::Deserialize::deserialize(deserializer)?;
        Version::parse(s.trim_start_matches('v')).map_err(de::Error::custom)
    }

    let response = client
        .get("https://api.github.com/repos/otter-sec/anchor/releases")
        .header(
            USER_AGENT,
            "avm https://github.com/otter-sec/anchor",
        )
        .send()?;

    if response.status().is_success() {
        let releases: Vec<Release> = response.json()?;
        let versions: Vec<Version> = releases
            .into_iter()
            .filter(|r| !r.draft && (include_pre_release || !r.prerelease))
            .map(|r| r.version)
            .collect();
        Ok(versions)
    } else {
        Err(
            if let Some(reset_time) = github_rate_limit_reset_time(response.headers()) {
                anyhow!(
                    "GitHub API rate limit exceeded. Try again after {} UTC.",
                    reset_time
                )
            } else {
                anyhow!("GitHub API rate limit exceeded. Try again later.",)
            },
        )
    }
}

fn github_rate_limit_reset_time(headers: &reqwest::header::HeaderMap) -> Option<String> {
    let timestamp = headers
        .get("X-RateLimit-Reset")?
        .to_str()
        .ok()?
        .parse::<i64>()
        .ok()?;

    Some(
        Utc.timestamp_opt(timestamp, 0)
            .single()?
            .format("%Y-%m-%d %H:%M:%S")
            .to_string(),
    )
}

/// Print available versions and flags indicating installed, current and latest
pub fn list_versions(include_pre_release: bool) -> Result<()> {
    let mut installed_versions = read_installed_versions()?;

    let mut available_versions = fetch_versions(include_pre_release)?;
    available_versions.sort();

    let print_versions =
        |versions: Vec<Version>, installed_versions: &mut Vec<Version>, show_latest: bool| {
            versions.iter().enumerate().for_each(|(i, v)| {
                print!("{v}");
                let mut flags = vec![];
                if i == versions.len() - 1 && show_latest {
                    flags.push("latest");
                }
                if let Some(position) = installed_versions.iter().position(|iv| iv == v) {
                    flags.push("installed");
                    installed_versions.remove(position);
                }
                if current_version().map(|cv| &cv == v).unwrap_or_default() {
                    flags.push("current");
                }

                if flags.is_empty() {
                    println!();
                } else {
                    println!("\t({})", flags.join(", "));
                }
            })
        };
    print_versions(available_versions, &mut installed_versions, true);
    print_versions(installed_versions.clone(), &mut installed_versions, false);

    Ok(())
}

pub fn get_latest_version(include_pre_release: bool) -> Result<Version> {
    get_latest_version_with_client(&HTTP_CLIENT, include_pre_release)
}

fn get_latest_version_with_client(
    client: &reqwest::blocking::Client,
    include_pre_release: bool,
) -> Result<Version> {
    let mut versions = fetch_versions_with_client(client, include_pre_release)?;
    versions.sort();
    versions
        .into_iter()
        .last()
        .ok_or_else(|| anyhow!("No versions found"))
}

/// Read the installed anchor-cli versions by reading the binaries in the AVM_HOME/bin directory.
pub fn read_installed_versions() -> Result<Vec<Version>> {
    const PREFIX: &str = "anchor-";
    let versions = fs::read_dir(get_bin_dir_path())?
        .filter_map(|entry_result| entry_result.ok())
        .filter_map(|entry| entry.file_name().to_str().map(|f| f.to_owned()))
        .filter(|file_name| file_name.starts_with(PREFIX))
        .filter_map(|file_name| file_name.trim_start_matches(PREFIX).parse::<Version>().ok())
        .collect();

    Ok(versions)
}

// ── AVM self-update ───────────────────────────────────────────────────────────

fn update_check_file_path() -> PathBuf {
    AVM_HOME.join(".update-check")
}

/// The cache file stores one of two states:
///   Success: `{unix_ts}\n{semver}`   — a successful check at `unix_ts` that found `semver`.
///   Error:   `{unix_ts}\n0`          — a failed check at `unix_ts` (`"0"` is not valid semver).
enum UpdateCacheState {
    Success(i64, Version),
    Error(i64),
    Missing,
}

fn read_update_cache() -> UpdateCacheState {
    let Ok(content) = fs::read_to_string(update_check_file_path()) else {
        return UpdateCacheState::Missing;
    };
    let mut lines = content.lines();
    let Some(ts) = lines.next().and_then(|l| l.parse::<i64>().ok()) else {
        return UpdateCacheState::Missing;
    };
    match lines.next().and_then(|l| Version::parse(l).ok()) {
        Some(v) => UpdateCacheState::Success(ts, v),
        None => UpdateCacheState::Error(ts),
    }
}

fn write_update_cache_success(version: &Version) {
    let content = format!("{}\n{version}", Utc::now().timestamp());
    let _ = fs::write(update_check_file_path(), content);
}

/// Writes timestamp 0 as an error sentinel so the next invocation knows the last check failed.
fn write_update_cache_error() {
    let content = format!("{}\n0", Utc::now().timestamp());
    let _ = fs::write(update_check_file_path(), content);
}

/// Check whether a newer AVM release is available and print a warning to stderr if so.
/// Results (including failures) are cached in `$AVM_HOME/.update-check` so the network
/// is hit at most once per hour.
pub fn check_avm_version_and_warn() {
    let Ok(current) = Version::parse(env!("CARGO_PKG_VERSION")) else {
        return;
    };

    let now = Utc::now().timestamp();

    match read_update_cache() {
        // Fresh successful cache: just compare and maybe warn.
        UpdateCacheState::Success(ts, latest) if now - ts < UPDATE_CHECK_INTERVAL_SECS => {
            if latest > current {
                eprintln!(
                    "A new version of avm is available: {latest} (you have {current}). Run `avm \
                     self-update` to upgrade."
                );
            }
        }
        // Previous check failed recently: tell the user and skip.
        UpdateCacheState::Error(ts) if now - ts < UPDATE_CHECK_INTERVAL_SECS => {
            let next_attempt_secs = (ts + UPDATE_CHECK_INTERVAL_SECS) - now;
            eprintln!("avm update check failed. Next attempt in {next_attempt_secs}s.");
        }
        // Cache is stale or missing: run a fresh check.
        _ => match get_latest_version_with_client(&HTTP_CLIENT, false) {
            Ok(latest) => {
                write_update_cache_success(&latest);
                if latest > current {
                    eprintln!(
                        "A new version of avm is available: {latest} (you have {current}). Run \
                         `avm self-update` to upgrade."
                    );
                }
            }
            Err(_) => {
                write_update_cache_error();
                eprintln!(
                    "avm update check failed. Next attempt in {UPDATE_CHECK_INTERVAL_SECS}s."
                );
            }
        },
    }
}

/// Update AVM itself by re-running `cargo install`.
///
/// - Default: installs the latest stable release via `--tag`.
/// - `include_pre_release`: installs the latest release including rc/beta/alpha.
/// - `bleeding_edge`: builds from the HEAD of the `master` branch.
pub fn self_update(include_pre_release: bool, bleeding_edge: bool) -> Result<()> {
    let current = Version::parse(env!("CARGO_PKG_VERSION"))
        .map_err(|e| anyhow!("Failed to parse current avm version: {e}"))?;

    let mut args = vec![
        "install".to_string(),
        "--git".to_string(),
        "https://github.com/otter-sec/anchor".to_string(),
        "--locked".to_string(),
    ];

    if bleeding_edge {
        println!("Updating avm to the latest commit on master...");
        args.extend_from_slice(&["--branch".to_string(), "master".to_string()]);
    } else {
        let latest = get_latest_version(include_pre_release)?;
        if latest <= current {
            println!("avm is already up to date ({current})");
            return Ok(());
        }
        println!("Updating avm from {current} to {latest}...");
        args.extend_from_slice(&["--tag".to_string(), format!("v{latest}")]);
    }

    args.extend_from_slice(&["avm".to_string(), "--force".to_string()]);

    let status = Command::new("cargo")
        .args(&args)
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .status()
        .map_err(|e| anyhow!("`cargo install` failed: {e}"))?;

    if !status.success() {
        bail!("Failed to update avm");
    }

    println!("avm successfully updated");
    Ok(())
}

#[cfg(test)]
mod tests {
    use {
        crate::*,
        semver::Version,
        std::{fs, io::Write, path::Path},
    };

    #[test]
    fn test_ensure_paths() {
        ensure_paths();
        assert!(AVM_HOME.exists());
        let bin_dir = get_bin_dir_path();
        assert!(bin_dir.exists());
        let current_version_file = current_version_file_path();
        assert!(current_version_file.exists());
    }

    #[test]
    fn test_version_binary_path() {
        assert_eq!(
            version_binary_path(&Version::parse("0.18.2").unwrap()),
            get_bin_dir_path().join("anchor-0.18.2")
        );
    }

    #[test]
    fn test_read_anchorversion() -> Result<()> {
        ensure_paths();

        let anchorversion_path = Path::new(".anchorversion");
        let test_version = "0.26.0";
        fs::write(anchorversion_path, test_version)?;

        let version = read_anchorversion_file()?;
        assert_eq!(version.to_string(), test_version);

        fs::remove_file(anchorversion_path)?;

        Ok(())
    }

    #[test]
    fn test_current_version() {
        ensure_paths();
        let mut current_version_file = fs::File::create(current_version_file_path()).unwrap();
        current_version_file.write_all("0.18.2".as_bytes()).unwrap();
        // Sync the file to disk before the read in current_version() to
        // mitigate the read not seeing the written version bytes.
        current_version_file.sync_all().unwrap();
        assert_eq!(
            current_version().unwrap(),
            Version::parse("0.18.2").unwrap()
        );
    }

    #[test]
    #[should_panic(expected = "anchor-cli 0.18.1 is not installed")]
    fn test_uninstall_non_installed_version() {
        uninstall_version(&Version::parse("0.18.1").unwrap()).unwrap();
    }

    #[test]
    #[should_panic(expected = "anchor-cli 0.18.2 is currently in use")]
    fn test_uninstalled_in_use_version() {
        ensure_paths();
        let version = Version::parse("0.18.2").unwrap();
        let mut current_version_file = fs::File::create(current_version_file_path()).unwrap();
        current_version_file.write_all("0.18.2".as_bytes()).unwrap();
        // Sync the file to disk before the read in current_version() to
        // mitigate the read not seeing the written version bytes.
        current_version_file.sync_all().unwrap();
        // Create a fake binary for anchor-0.18.2 in the bin directory
        fs::File::create(version_binary_path(&version)).unwrap();
        uninstall_version(&version).unwrap();
    }

    #[test]
    fn test_read_installed_versions() {
        ensure_paths();
        let version = Version::parse("0.18.2").unwrap();

        // Create a fake binary for anchor-0.18.2 in the bin directory
        fs::File::create(version_binary_path(&version)).unwrap();
        let expected = vec![version];
        assert_eq!(read_installed_versions().unwrap(), expected);

        // Should ignore this file because it's not anchor- prefixed
        fs::File::create(AVM_HOME.join("bin").join("garbage").as_path()).unwrap();
        assert_eq!(read_installed_versions().unwrap(), expected);
    }

    #[test]
    fn test_github_rate_limit_reset_time() {
        let mut headers = reqwest::header::HeaderMap::new();
        headers.insert("X-RateLimit-Reset", "1715706000".parse().unwrap());
        assert_eq!(
            github_rate_limit_reset_time(&headers).as_deref(),
            Some("2024-05-14 17:00:00")
        );

        assert!(github_rate_limit_reset_time(&reqwest::header::HeaderMap::new()).is_none());

        let mut headers = reqwest::header::HeaderMap::new();
        headers.insert("X-RateLimit-Reset", "unknown".parse().unwrap());
        assert!(github_rate_limit_reset_time(&headers).is_none());

        let mut headers = reqwest::header::HeaderMap::new();
        headers.insert(
            "X-RateLimit-Reset",
            reqwest::header::HeaderValue::from_bytes(b"\xff").unwrap(),
        );
        assert!(github_rate_limit_reset_time(&headers).is_none());
    }

    #[test]
    fn test_get_anchor_version_from_commit() {
        let version =
            get_anchor_version_from_commit("e1afcbf71e0f2e10fae14525934a6a68479167b9").unwrap();
        assert_eq!(
            version.to_string(),
            "0.28.0-e1afcbf71e0f2e10fae14525934a6a68479167b9"
        )
    }

    #[test]
    fn test_check_and_get_full_commit_when_full_commit() {
        assert_eq!(
            check_and_get_full_commit("e1afcbf71e0f2e10fae14525934a6a68479167b9").unwrap(),
            "e1afcbf71e0f2e10fae14525934a6a68479167b9"
        )
    }

    #[test]
    fn test_check_and_get_full_commit_when_partial_commit() {
        assert_eq!(
            check_and_get_full_commit("e1afcbf").unwrap(),
            "e1afcbf71e0f2e10fae14525934a6a68479167b9"
        )
    }

    #[test]
    fn test_append_commit_stable_version() {
        let mut version = Version::parse("0.28.0").unwrap();
        append_commit(&mut version, "e1afcbf71e0f2e10fae14525934a6a68479167b9").unwrap();
        assert_eq!(
            version.to_string(),
            "0.28.0-e1afcbf71e0f2e10fae14525934a6a68479167b9"
        );
    }

    #[test]
    fn test_append_commit_pre_release_version() {
        let mut version = Version::parse("1.0.0-rc.3").unwrap();
        append_commit(&mut version, "e1afcbf71e0f2e10fae14525934a6a68479167b9").unwrap();
        assert_eq!(
            version.to_string(),
            "1.0.0-rc.3-e1afcbf71e0f2e10fae14525934a6a68479167b9"
        );
    }
}
