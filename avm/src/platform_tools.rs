//! Platform-tools version resolution and installation.
//!
//! Resolution: given a Solana version a project targets, return the
//! `platform-tools` version that ships with that Solana release. The map is
//! embedded at compile time from `../platform-tools-map.toml` and ordered by
//! ascending Solana version, so resolution is a linear floor lookup: pick the
//! entry with the largest `solana` key that is `<= requested`. When the
//! project pins no Solana version at all, fall back to the map's `fallback`
//! field (kept equal to the newest entry's `platform_tools`).
//!
//! Installation: download the matching tarball from `anza-xyz/platform-tools`
//! GitHub releases and extract into `$AVM_HOME/platform-tools/<version>/`.
//! Asset naming follows what `cargo-build-sbf` looks for upstream:
//! `platform-tools-{linux|osx|windows}-{x86_64|aarch64}.tar.bz2`.
use {
    crate::{
        resolve::{resolve_solana_version, SolanaResolution, SolanaResolutionSource},
        AVM_HOME, DOWNLOAD_CLIENT,
    },
    anyhow::{anyhow, bail, Context, Result},
    semver::Version,
    serde::Deserialize,
    std::{
        fs,
        path::{Path, PathBuf},
        process::{Command, Stdio},
        sync::LazyLock,
    },
};

const PLATFORM_TOOLS_MAP_TOML: &str = include_str!("../platform-tools-map.toml");

#[derive(Debug, Deserialize)]
struct PlatformToolsMap {
    fallback: String,
    entries: Vec<MapEntry>,
}

#[derive(Debug, Deserialize)]
struct MapEntry {
    solana: String,
    platform_tools: String,
}

/// Parsed and validated form of the static map.
#[derive(Debug)]
struct ParsedMap {
    fallback: String,
    /// Sorted ascending by Solana version.
    entries: Vec<(Version, String)>,
}

static MAP: LazyLock<ParsedMap> = LazyLock::new(|| {
    let raw: PlatformToolsMap = toml::from_str(PLATFORM_TOOLS_MAP_TOML)
        .expect("Built-in platform-tools-map.toml must parse");

    let mut entries: Vec<(Version, String)> = raw
        .entries
        .into_iter()
        .map(|e| {
            let v = Version::parse(&e.solana).unwrap_or_else(|err| {
                panic!("Invalid Solana version `{}` in map: {err}", e.solana)
            });
            (v, e.platform_tools)
        })
        .collect();

    let was_sorted = entries.windows(2).all(|w| w[0].0 <= w[1].0);
    assert!(
        was_sorted,
        "platform-tools-map.toml entries must be sorted by Solana version"
    );
    let _ = was_sorted; // silence unused-variable in release if the assert is stripped
    entries.sort_by(|a, b| a.0.cmp(&b.0)); // defensive

    ParsedMap {
        fallback: raw.fallback,
        entries,
    }
});

/// Where the platform-tools version came from. Combines the upstream Solana
/// source (if any) with the lookup outcome (a specific map row, or the
/// fallback because nothing matched).
#[derive(Debug, Clone)]
pub enum PlatformToolsSource {
    /// Mapped from a project-pinned Solana version via the static map.
    Mapped {
        solana: Version,
        solana_source: SolanaResolutionSource,
    },
    /// Project pinned a Solana version older than the map's earliest entry.
    /// We still return the oldest known platform-tools.
    BelowMap {
        solana: Version,
        solana_source: SolanaResolutionSource,
    },
    /// Project did not pin Solana → use the map's hardcoded fallback.
    Fallback,
}

impl PlatformToolsSource {
    pub fn describe(&self) -> String {
        match self {
            Self::Mapped {
                solana,
                solana_source,
            } => format!("solana {solana} → map ({})", solana_source.describe()),
            Self::BelowMap {
                solana,
                solana_source,
            } => format!(
                "solana {solana} predates map; using earliest entry ({})",
                solana_source.describe()
            ),
            Self::Fallback => "fallback (no Solana version pinned)".to_string(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct PlatformToolsResolution {
    /// e.g. `"v1.54"`. Kept as a string because upstream uses the `v`-prefixed
    /// form everywhere (release tags, archive names, the `DEFAULT_…` constant).
    pub version: String,
    pub source: PlatformToolsSource,
}

/// Resolve the platform-tools version for the project rooted at `start`.
///
/// Walks the same project-detection logic as [`resolve_solana_version`], then
/// performs a floor lookup in the embedded map.
pub fn resolve_platform_tools(start: &Path) -> Result<PlatformToolsResolution> {
    match resolve_solana_version(start)? {
        Some(solana_res) => Ok(resolve_for_solana(&solana_res)),
        None => Ok(PlatformToolsResolution {
            version: MAP.fallback.clone(),
            source: PlatformToolsSource::Fallback,
        }),
    }
}

fn resolve_for_solana(solana_res: &SolanaResolution) -> PlatformToolsResolution {
    let solana = &solana_res.version;
    let entries = &MAP.entries;

    // Floor lookup: largest entry.0 <= solana.
    let pick = entries.iter().rposition(|(v, _)| v <= solana);
    match pick {
        Some(idx) => PlatformToolsResolution {
            version: entries[idx].1.clone(),
            source: PlatformToolsSource::Mapped {
                solana: solana.clone(),
                solana_source: solana_res.source.clone(),
            },
        },
        None => {
            // Requested Solana is older than every entry. Return the earliest
            // known platform-tools rather than the (newer) fallback — older
            // toolchains are closer to what such a project expects.
            let earliest = &entries
                .first()
                .expect("platform-tools map must have at least one entry")
                .1;
            PlatformToolsResolution {
                version: earliest.clone(),
                source: PlatformToolsSource::BelowMap {
                    solana: solana.clone(),
                    solana_source: solana_res.source.clone(),
                },
            }
        }
    }
}

/// Look up the platform-tools version for an explicit Solana version without
/// touching the filesystem. Useful for callers that already have a Solana
/// version in hand.
pub fn lookup_for_solana_version(solana: &Version) -> Result<String> {
    let entries = &MAP.entries;
    entries
        .iter()
        .rposition(|(v, _)| v <= solana)
        .map(|idx| entries[idx].1.clone())
        .ok_or_else(|| {
            anyhow!(
                "Solana {solana} predates the earliest platform-tools map entry ({}).",
                entries[0].0
            )
        })
}

/// The hardcoded fallback platform-tools version. Exposed for callers that
/// want to surface it to the user (e.g. `avm platform-tools resolve`).
pub fn fallback_version() -> &'static str {
    &MAP.fallback
}

// ── Install / storage ────────────────────────────────────────────────────────

/// `$AVM_HOME/platform-tools` — root of installed platform-tools.
pub fn get_platform_tools_dir_path() -> PathBuf {
    AVM_HOME.join("platform-tools")
}

/// Path where the given platform-tools `version` (e.g. `"v1.54"`) is installed.
pub fn platform_tools_version_path(version: &str) -> PathBuf {
    get_platform_tools_dir_path().join(version)
}

/// List installed platform-tools versions, lexicographically ordered.
pub fn read_installed_platform_tools() -> Result<Vec<String>> {
    let dir = get_platform_tools_dir_path();
    if !dir.exists() {
        return Ok(vec![]);
    }
    let mut out: Vec<String> = fs::read_dir(&dir)
        .with_context(|| format!("Reading {}", dir.display()))?
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().map(|t| t.is_dir()).unwrap_or(false))
        .filter_map(|e| e.file_name().into_string().ok())
        .filter(|n| n.starts_with('v'))
        .collect();
    out.sort();
    Ok(out)
}

/// Asset file name to download from anza-xyz/platform-tools releases for the
/// current host (e.g. `"platform-tools-linux-x86_64.tar.bz2"`).
pub fn host_asset_name() -> &'static str {
    // Mirrors cargo-build-sbf's naming. The four supported combinations are
    // baked in so a misconfigured host fails to compile instead of trying to
    // download a non-existent asset at runtime.
    #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
    {
        "platform-tools-linux-x86_64.tar.bz2"
    }
    #[cfg(all(target_os = "linux", target_arch = "aarch64"))]
    {
        "platform-tools-linux-aarch64.tar.bz2"
    }
    #[cfg(all(target_os = "macos", target_arch = "x86_64"))]
    {
        "platform-tools-osx-x86_64.tar.bz2"
    }
    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    {
        "platform-tools-osx-aarch64.tar.bz2"
    }
    #[cfg(all(target_os = "windows", target_arch = "x86_64"))]
    {
        "platform-tools-windows-x86_64.tar.bz2"
    }
    #[cfg(all(target_os = "windows", target_arch = "aarch64"))]
    {
        "platform-tools-windows-aarch64.tar.bz2"
    }
}

/// Full download URL for a given platform-tools version on the host target.
pub fn download_url(version: &str) -> String {
    let version = if version.starts_with('v') {
        version.to_string()
    } else {
        format!("v{version}")
    };
    format!(
        "https://github.com/anza-xyz/platform-tools/releases/download/{version}/{}",
        host_asset_name()
    )
}

/// Download and extract platform-tools `version` into `$AVM_HOME/platform-tools/<version>/`.
///
/// When `force` is false and the target directory already exists with a
/// non-empty `rust/` subdirectory (the expected payload), the install is a
/// no-op. The download is staged in a `.partial` directory next to the target
/// and atomically renamed on success so a failed install never leaves a
/// half-populated directory at the canonical path.
pub fn install_platform_tools(version: &str, force: bool) -> Result<()> {
    let version = if version.starts_with('v') {
        version.to_string()
    } else {
        format!("v{version}")
    };
    let target = platform_tools_version_path(&version);
    if !force && looks_installed(&target) {
        println!(
            "platform-tools {version} is already installed at {}",
            target.display()
        );
        return Ok(());
    }
    let parent = target.parent().expect("platform-tools path has parent");
    fs::create_dir_all(parent).with_context(|| format!("Creating {}", parent.display()))?;

    // Stage download + extract in a sibling directory.
    let staging = parent.join(format!("{version}.partial"));
    if staging.exists() {
        fs::remove_dir_all(&staging)
            .with_context(|| format!("Cleaning up stale {}", staging.display()))?;
    }
    fs::create_dir_all(&staging).with_context(|| format!("Creating {}", staging.display()))?;

    // Cleanup on any error from here on.
    let result = (|| -> Result<()> {
        let url = download_url(&version);
        let archive_path = staging.join(host_asset_name());
        println!("Downloading {url}");
        download_to(&url, &archive_path)?;

        println!("Extracting {}", archive_path.display());
        extract_tar_bz2(&archive_path, &staging)?;

        // Remove the archive so it doesn't end up in the final install dir.
        let _ = fs::remove_file(&archive_path);

        // Sanity check the extracted payload.
        if !looks_installed(&staging) {
            bail!(
                "Extracted archive does not look like a platform-tools install (no `rust/` \
                 subdirectory under {}). Re-run with --force after checking the upstream release.",
                staging.display()
            );
        }
        Ok(())
    })();

    match result {
        Ok(()) => {
            replace_install_dir(&staging, &target)?;
            println!("Installed platform-tools {version} to {}", target.display());
            Ok(())
        }
        Err(e) => {
            let _ = fs::remove_dir_all(&staging);
            Err(e)
        }
    }
}

fn replace_install_dir(staging: &Path, target: &Path) -> Result<()> {
    if target.exists() {
        fs::remove_dir_all(target)
            .with_context(|| format!("Removing existing {}", target.display()))?;
    }
    fs::rename(staging, target)
        .with_context(|| format!("Renaming {} → {}", staging.display(), target.display()))?;
    Ok(())
}

/// Remove an installed platform-tools version.
pub fn uninstall_platform_tools(version: &str) -> Result<()> {
    let version = if version.starts_with('v') {
        version.to_string()
    } else {
        format!("v{version}")
    };
    let target = platform_tools_version_path(&version);
    if !target.exists() {
        bail!(
            "platform-tools {version} is not installed at {}",
            target.display()
        );
    }
    fs::remove_dir_all(&target).with_context(|| format!("Removing {}", target.display()))?;
    println!("Uninstalled platform-tools {version}");
    Ok(())
}

/// Heuristic: an install is "real" when it contains a non-empty `rust/`
/// subdirectory — the canonical layout of the platform-tools archive.
fn looks_installed(dir: &Path) -> bool {
    let rust_dir = dir.join("rust");
    rust_dir.is_dir()
        && fs::read_dir(&rust_dir)
            .map(|mut it| it.next().is_some())
            .unwrap_or(false)
}

fn download_to(url: &str, dest: &Path) -> Result<()> {
    let mut response = DOWNLOAD_CLIENT
        .get(url)
        .send()
        .with_context(|| format!("Sending GET {url}"))?;
    if !response.status().is_success() {
        bail!("Failed to download `{url}` (status {})", response.status());
    }
    let mut file =
        fs::File::create(dest).with_context(|| format!("Creating {}", dest.display()))?;
    response
        .copy_to(&mut file)
        .with_context(|| format!("Writing {}", dest.display()))?;
    Ok(())
}

/// Extract a `.tar.bz2` into `dest_dir` by shelling out to `tar`.
///
/// Using the system `tar` avoids adding a native `libbz2` dependency. `tar` is
/// available out of the box on Linux, macOS, and modern Windows (10+).
fn extract_tar_bz2(archive: &Path, dest_dir: &Path) -> Result<()> {
    let status = Command::new("tar")
        .arg("-xjf")
        .arg(archive)
        .arg("-C")
        .arg(dest_dir)
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .status()
        .context("Spawning `tar`")?;
    if !status.success() {
        bail!(
            "`tar -xjf {} -C {}` exited with status {status}",
            archive.display(),
            dest_dir.display()
        );
    }
    Ok(())
}

/// Force the map to parse at startup, surfacing any embedded-data bugs as a
/// clear error instead of a panic in a random first user.
pub fn validate_embedded_map() -> Result<()> {
    let raw: PlatformToolsMap = toml::from_str(PLATFORM_TOOLS_MAP_TOML)
        .context("Parsing embedded platform-tools-map.toml")?;
    for e in &raw.entries {
        Version::parse(&e.solana)
            .with_context(|| format!("Invalid Solana version `{}` in map", e.solana))?;
    }
    if raw.entries.is_empty() {
        return Err(anyhow!(
            "platform-tools-map.toml must have at least one entry"
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use {super::*, crate::resolve::SolanaResolutionSource, std::path::PathBuf};

    fn v(s: &str) -> Version {
        Version::parse(s).unwrap()
    }

    fn fake_solana(s: &str) -> SolanaResolution {
        SolanaResolution {
            version: v(s),
            source: SolanaResolutionSource::AnchorToml(PathBuf::from("Anchor.toml")),
            version_req: None,
        }
    }

    // ── Embedded map ─────────────────────────────────────────────────────────

    #[test]
    fn embedded_map_parses_and_is_sorted() {
        validate_embedded_map().unwrap();
        let entries = &MAP.entries;
        assert!(entries.len() >= 2);
        assert!(entries.windows(2).all(|w| w[0].0 < w[1].0));
    }

    #[test]
    fn fallback_matches_newest_entry() {
        // Sanity: the fallback should equal the newest entry's platform_tools,
        // per the comment in platform-tools-map.toml.
        let newest = &MAP.entries.last().unwrap().1;
        assert_eq!(&MAP.fallback, newest);
    }

    // ── Floor lookup ────────────────────────────────────────────────────────

    #[test]
    fn exact_entry_match() {
        let res = resolve_for_solana(&fake_solana("3.0.0"));
        assert_eq!(res.version, "v1.51");
        assert!(matches!(res.source, PlatformToolsSource::Mapped { .. }));
    }

    #[test]
    fn between_entries_picks_floor() {
        // 2.2.5 sits between (2.2.3 → v1.45) and (2.2.8 → v1.46) → floor is v1.45.
        let res = resolve_for_solana(&fake_solana("2.2.5"));
        assert_eq!(res.version, "v1.45");
    }

    #[test]
    fn above_all_entries_uses_latest() {
        let res = resolve_for_solana(&fake_solana("99.0.0"));
        let latest = &MAP.entries.last().unwrap().1;
        assert_eq!(&res.version, latest);
    }

    #[test]
    fn below_all_entries_uses_earliest() {
        let res = resolve_for_solana(&fake_solana("1.0.0"));
        let earliest = &MAP.entries.first().unwrap().1;
        assert_eq!(&res.version, earliest);
        assert!(matches!(res.source, PlatformToolsSource::BelowMap { .. }));
    }

    #[test]
    fn lookup_for_solana_version_works() {
        assert_eq!(lookup_for_solana_version(&v("3.0.0")).unwrap(), "v1.51");
        assert_eq!(lookup_for_solana_version(&v("4.5.0")).unwrap(), "v1.54");
        // Below earliest → error from this lower-level helper.
        assert!(lookup_for_solana_version(&v("0.1.0")).is_err());
    }

    // ── Specific known transitions ──────────────────────────────────────────

    #[test]
    fn known_transition_1_18_0_to_v1_39() {
        assert_eq!(lookup_for_solana_version(&v("1.18.0")).unwrap(), "v1.39");
    }

    #[test]
    fn known_transition_1_18_8_to_v1_41() {
        assert_eq!(lookup_for_solana_version(&v("1.18.8")).unwrap(), "v1.41");
    }

    #[test]
    fn known_transition_2_0_5_to_v1_42() {
        assert_eq!(lookup_for_solana_version(&v("2.0.5")).unwrap(), "v1.42");
    }

    #[test]
    fn known_transition_2_1_0_to_v1_43() {
        assert_eq!(lookup_for_solana_version(&v("2.1.0")).unwrap(), "v1.43");
    }

    #[test]
    fn known_transition_3_0_0_to_v1_51() {
        assert_eq!(lookup_for_solana_version(&v("3.0.0")).unwrap(), "v1.51");
    }

    #[test]
    fn known_transition_4_0_0_to_v1_54() {
        assert_eq!(lookup_for_solana_version(&v("4.0.0")).unwrap(), "v1.54");
    }

    // ── URL + asset naming ──────────────────────────────────────────────────

    #[test]
    fn host_asset_name_uses_supported_combo() {
        let name = host_asset_name();
        assert!(name.starts_with("platform-tools-"));
        assert!(name.ends_with(".tar.bz2"));
        let middle = name
            .trim_start_matches("platform-tools-")
            .trim_end_matches(".tar.bz2");
        let (os, arch) = middle.split_once('-').expect("os-arch");
        assert!(matches!(os, "linux" | "osx" | "windows"));
        assert!(matches!(arch, "x86_64" | "aarch64"));
    }

    #[test]
    fn download_url_prepends_v_when_missing() {
        let with_v = download_url("v1.54");
        let without_v = download_url("1.54");
        assert_eq!(with_v, without_v);
        assert!(with_v.contains("/releases/download/v1.54/"));
    }

    #[test]
    fn download_url_targets_anza_platform_tools() {
        let url = download_url("v1.54");
        assert!(url.starts_with("https://github.com/anza-xyz/platform-tools/releases/download/"));
        assert!(url.ends_with(host_asset_name()));
    }

    // ── looks_installed ─────────────────────────────────────────────────────

    #[test]
    fn looks_installed_requires_nonempty_rust_dir() {
        let dir = tempfile::TempDir::new().unwrap();
        assert!(!looks_installed(dir.path()));

        std::fs::create_dir_all(dir.path().join("rust")).unwrap();
        assert!(!looks_installed(dir.path()), "empty rust/ should not count");

        std::fs::write(dir.path().join("rust/marker"), b"").unwrap();
        assert!(looks_installed(dir.path()));
    }

    #[test]
    fn replace_install_dir_swaps_existing_target_after_staging_is_ready() {
        let dir = tempfile::TempDir::new().unwrap();
        let target = dir.path().join("v1.54");
        let staging = dir.path().join("v1.54.partial");

        std::fs::create_dir_all(target.join("rust")).unwrap();
        std::fs::write(target.join("rust/old"), b"old").unwrap();
        std::fs::create_dir_all(staging.join("rust")).unwrap();
        std::fs::write(staging.join("rust/new"), b"new").unwrap();

        replace_install_dir(&staging, &target).unwrap();

        assert!(!staging.exists());
        assert!(!target.join("rust/old").exists());
        assert_eq!(
            std::fs::read_to_string(target.join("rust/new")).unwrap(),
            "new"
        );
    }
}
