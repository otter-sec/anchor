//! Rust nightly resolution for Anchor IDL builds.
//!
//! Anchor enables proc-macro2's semver-exempt API while compiling IDLs. That
//! API follows Rust nightly and can change independently of proc-macro2's
//! stable API, so the compatible nightly is resolved from the proc-macro2
//! version in the project's Cargo.lock using `../idl-nightly-map.toml`.
//! Projects without a usable lockfile receive the map's current fallback.

use {
    anyhow::{anyhow, bail, Context, Result},
    semver::Version,
    serde::Deserialize,
    std::{
        collections::BTreeMap,
        fs,
        path::{Path, PathBuf},
        sync::LazyLock,
    },
};

const IDL_NIGHTLY_MAP_TOML: &str = include_str!("../idl-nightly-map.toml");

#[derive(Debug, Deserialize)]
struct IdlNightlyMap {
    fallback: String,
    entries: Vec<MapEntry>,
}

#[derive(Debug, Deserialize)]
struct MapEntry {
    proc_macro2: String,
    nightly: String,
}

#[derive(Debug)]
struct ParsedMap {
    fallback: String,
    entries: Vec<ParsedMapEntry>,
}

#[derive(Debug)]
struct ParsedMapEntry {
    proc_macro2: Version,
    nightly: String,
}

#[derive(Debug, Deserialize)]
struct CargoLock {
    #[serde(default)]
    package: Vec<LockedPackage>,
}

#[derive(Debug, Deserialize)]
struct LockedPackage {
    name: String,
    version: String,
}

static MAP: LazyLock<ParsedMap> = LazyLock::new(|| {
    let raw: IdlNightlyMap =
        toml::from_str(IDL_NIGHTLY_MAP_TOML).expect("Built-in IDL nightly map must parse");
    assert_valid_nightly(&raw.fallback);

    let mut entries = raw
        .entries
        .into_iter()
        .map(|entry| {
            let proc_macro2 = Version::parse(&entry.proc_macro2).unwrap_or_else(|err| {
                panic!(
                    "Invalid proc-macro2 version `{}` in IDL nightly map: {err}",
                    entry.proc_macro2
                )
            });
            assert_valid_nightly(&entry.nightly);
            ParsedMapEntry {
                proc_macro2,
                nightly: entry.nightly,
            }
        })
        .collect::<Vec<_>>();

    let was_sorted = entries
        .windows(2)
        .all(|window| window[0].proc_macro2 < window[1].proc_macro2);
    assert!(
        was_sorted,
        "idl-nightly-map.toml entries must be sorted by proc-macro2 version"
    );
    let _ = was_sorted;
    entries.sort_by(|a, b| a.proc_macro2.cmp(&b.proc_macro2));

    ParsedMap {
        fallback: raw.fallback,
        entries,
    }
});

fn assert_valid_nightly(toolchain: &str) {
    let date = toolchain.strip_prefix("nightly-").unwrap_or_else(|| {
        panic!("Invalid IDL nightly `{toolchain}`: expected nightly-YYYY-MM-DD")
    });
    chrono::NaiveDate::parse_from_str(date, "%Y-%m-%d")
        .unwrap_or_else(|err| panic!("Invalid IDL nightly `{toolchain}`: {err}"));
}

#[derive(Debug, Clone)]
pub struct IdlNightlyResolution {
    pub version: String,
    pub source: IdlNightlySource,
}

#[derive(Debug, Clone)]
pub enum IdlNightlySource {
    CargoLock {
        path: PathBuf,
        proc_macro2_versions: Vec<Version>,
    },
    Fallback,
}

impl IdlNightlySource {
    pub fn describe(&self) -> String {
        match self {
            Self::CargoLock {
                path,
                proc_macro2_versions,
            } => format!(
                "proc-macro2 {} in {}",
                proc_macro2_versions
                    .iter()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>()
                    .join(", "),
                path.display()
            ),
            Self::Fallback => "IDL nightly fallback".to_string(),
        }
    }
}

/// Resolve the Rust nightly for IDL generation from the nearest Cargo.lock.
pub fn resolve_idl_nightly(start: &Path) -> Result<IdlNightlyResolution> {
    let Some(lockfile) = find_ancestor_file(start, "Cargo.lock") else {
        return Ok(fallback_resolution());
    };
    let versions = locked_proc_macro2_versions(&lockfile)?;
    if versions.is_empty() {
        return Ok(fallback_resolution());
    }

    let mut resolved = BTreeMap::<String, Vec<Version>>::new();
    for version in &versions {
        let nightly = lookup_nightly(version).ok_or_else(|| {
            anyhow!(
                "No IDL nightly mapping exists for proc-macro2 {version} from {}",
                lockfile.display()
            )
        })?;
        resolved.entry(nightly).or_default().push(version.clone());
    }

    if resolved.len() != 1 {
        let details = resolved
            .iter()
            .map(|(nightly, versions)| {
                format!(
                    "{nightly} for proc-macro2 {}",
                    versions
                        .iter()
                        .map(ToString::to_string)
                        .collect::<Vec<_>>()
                        .join(", ")
                )
            })
            .collect::<Vec<_>>()
            .join("; ");
        bail!(
            "No single IDL nightly supports every proc-macro2 version in {}: {details}",
            lockfile.display()
        );
    }

    let version = resolved.into_keys().next().unwrap();
    Ok(IdlNightlyResolution {
        version,
        source: IdlNightlySource::CargoLock {
            path: lockfile,
            proc_macro2_versions: versions,
        },
    })
}

fn fallback_resolution() -> IdlNightlyResolution {
    IdlNightlyResolution {
        version: MAP.fallback.clone(),
        source: IdlNightlySource::Fallback,
    }
}

fn lookup_nightly(proc_macro2: &Version) -> Option<String> {
    MAP.entries
        .iter()
        .rposition(|entry| entry.proc_macro2 <= *proc_macro2)
        .map(|idx| MAP.entries[idx].nightly.clone())
}

fn locked_proc_macro2_versions(lockfile: &Path) -> Result<Vec<Version>> {
    let text = fs::read_to_string(lockfile)
        .with_context(|| format!("Reading Cargo lockfile {}", lockfile.display()))?;
    let lock: CargoLock = toml::from_str(&text)
        .with_context(|| format!("Parsing Cargo lockfile {}", lockfile.display()))?;
    let mut versions = lock
        .package
        .into_iter()
        .filter(|package| package.name == "proc-macro2")
        .map(|package| {
            Version::parse(&package.version).with_context(|| {
                format!(
                    "Parsing proc-macro2 version `{}` in {}",
                    package.version,
                    lockfile.display()
                )
            })
        })
        .collect::<Result<Vec<_>>>()?;
    versions.sort();
    versions.dedup();
    Ok(versions)
}

fn find_ancestor_file(start: &Path, name: &str) -> Option<PathBuf> {
    let mut dir = if start.is_file() {
        start.parent()?.to_path_buf()
    } else {
        start.to_path_buf()
    };
    loop {
        let candidate = dir.join(name);
        if candidate.is_file() {
            return Some(candidate);
        }
        if !dir.pop() {
            return None;
        }
    }
}

/// Validate the embedded map without initializing its global cache.
pub fn validate_embedded_map() -> Result<()> {
    let raw: IdlNightlyMap =
        toml::from_str(IDL_NIGHTLY_MAP_TOML).context("Parsing embedded IDL nightly map")?;
    if raw.entries.is_empty() {
        bail!("idl-nightly-map.toml must have at least one entry");
    }
    validate_nightly(&raw.fallback)?;

    let entries = raw
        .entries
        .iter()
        .map(|entry| {
            validate_nightly(&entry.nightly)?;
            Version::parse(&entry.proc_macro2).with_context(|| {
                format!(
                    "Invalid proc-macro2 version `{}` in IDL nightly map",
                    entry.proc_macro2
                )
            })
        })
        .collect::<Result<Vec<_>>>()?;
    if !entries.windows(2).all(|window| window[0] < window[1]) {
        bail!("idl-nightly-map.toml entries must be sorted by proc-macro2 version");
    }
    Ok(())
}

fn validate_nightly(toolchain: &str) -> Result<()> {
    let date = toolchain
        .strip_prefix("nightly-")
        .ok_or_else(|| anyhow!("Invalid IDL nightly `{toolchain}`: expected nightly-YYYY-MM-DD"))?;
    chrono::NaiveDate::parse_from_str(date, "%Y-%m-%d")
        .with_context(|| format!("Invalid IDL nightly `{toolchain}`"))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use {super::*, std::fs, tempfile::TempDir};

    fn v(version: &str) -> Version {
        Version::parse(version).unwrap()
    }

    fn write_lock(dir: &Path, versions: &[&str]) {
        let packages = versions
            .iter()
            .map(|version| {
                format!("[[package]]\nname = \"proc-macro2\"\nversion = \"{version}\"\n")
            })
            .collect::<Vec<_>>()
            .join("\n");
        fs::write(dir.join("Cargo.lock"), format!("version = 4\n\n{packages}"))
            .expect("write lockfile");
    }

    #[test]
    fn embedded_map_parses_and_is_sorted() {
        validate_embedded_map().unwrap();
        assert!(MAP
            .entries
            .windows(2)
            .all(|window| window[0].proc_macro2 < window[1].proc_macro2));
    }

    #[test]
    fn lookup_uses_proc_macro2_release_boundary() {
        assert_eq!(lookup_nightly(&v("1.0.94")).unwrap(), "nightly-2025-04-15");
        assert_eq!(lookup_nightly(&v("1.0.95")).unwrap(), "nightly-2026-06-10");
        assert_eq!(lookup_nightly(&v("1.0.107")).unwrap(), "nightly-2026-06-10");
    }

    #[test]
    fn resolves_from_locked_proc_macro2() {
        let dir = TempDir::new().unwrap();
        write_lock(dir.path(), &["1.0.86"]);
        let resolution = resolve_idl_nightly(dir.path()).unwrap();
        assert_eq!(resolution.version, "nightly-2025-04-15");
        assert!(matches!(
            resolution.source,
            IdlNightlySource::CargoLock { .. }
        ));
    }

    #[test]
    fn falls_back_without_locked_proc_macro2() {
        let dir = TempDir::new().unwrap();
        let resolution = resolve_idl_nightly(dir.path()).unwrap();
        assert_eq!(resolution.version, "nightly-2026-06-10");
        assert!(matches!(resolution.source, IdlNightlySource::Fallback));
    }

    #[test]
    fn mixed_legacy_and_current_proc_macro2_versions_use_common_nightly() {
        let dir = TempDir::new().unwrap();
        write_lock(dir.path(), &["0.4.30", "1.0.86"]);
        let resolution = resolve_idl_nightly(dir.path()).unwrap();
        assert_eq!(resolution.version, "nightly-2025-04-15");
    }

    #[test]
    fn mixed_incompatible_proc_macro2_versions_error() {
        let dir = TempDir::new().unwrap();
        write_lock(dir.path(), &["1.0.94", "1.0.95"]);
        let error = resolve_idl_nightly(dir.path()).unwrap_err().to_string();
        assert!(error.contains("No single IDL nightly"), "{error}");
        assert!(error.contains("nightly-2025-04-15"), "{error}");
        assert!(error.contains("nightly-2026-06-10"), "{error}");
    }
}
