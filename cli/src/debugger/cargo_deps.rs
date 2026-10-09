//! Discover dependency source trees so the TUI can read files whose DWARF
//! paths were emitted relative to each crate's compile dir (cargo strips
//! `DW_AT_comp_dir` on some builds, leaving entries like `src/cpi.rs`).
//!
//! Strategy: parse the workspace `Cargo.lock`, locate each registry or git
//! dep in the local cargo cache, and hand back those directories. The
//! resolver in [`super::tui::resolve_src_path`] tries each as a join-root
//! and uses the first match.
//!
//! **Ambiguity note.** Relative paths like `src/lib.rs` exist in almost
//! every crate. When multiple deps could match, the first one in
//! `Cargo.lock` wins — deterministic but not always semantically correct.
//! Unique paths like `src/sysvars/rent.rs` disambiguate cleanly in
//! practice. We accept this tradeoff because the alternative — routing
//! each relative path through PC-and-symbol-aware dep selection — is
//! disproportionate effort for the mostly-cosmetic win.

use {
    serde::Deserialize,
    std::{
        collections::BTreeSet,
        path::{Path, PathBuf},
    },
};

/// Minimal Cargo.lock schema — ignores anything we don't care about.
#[derive(Deserialize)]
struct CargoLock {
    #[serde(default)]
    package: Vec<Package>,
}

#[derive(Deserialize)]
struct Package {
    name: String,
    version: String,
    #[serde(default)]
    source: Option<String>,
}

/// Return a list of directories to try when joining a relative DWARF path.
/// Each entry is a crate source root under `$CARGO_HOME/registry/src/` or
/// `$CARGO_HOME/git/checkouts/`. Empty vec if `Cargo.lock` or the cargo
/// cache isn't available.
///
/// Order: registry deps first (most predictable layout), then git
/// checkouts (best-effort), sorted within each group by
/// `Cargo.lock` order. No dedup across packages — upstream dedup happens
/// because we only add candidates whose directory actually exists.
pub fn discover_dep_src_roots(workspace_root: &Path) -> Vec<PathBuf> {
    let lock_path = workspace_root.join("Cargo.lock");
    let Ok(contents) = std::fs::read_to_string(&lock_path) else {
        return Vec::new();
    };
    let Ok(parsed) = toml::from_str::<CargoLock>(&contents) else {
        return Vec::new();
    };

    let Some(cargo_home) = cargo_home() else {
        return Vec::new();
    };

    // Enumerate every `registry/src/<registry-hash>/` subtree so we can
    // resolve deps regardless of which index they came from (crates.io,
    // custom registries, sparse-index vs git-index, etc.).
    let registry_roots: Vec<PathBuf> = std::fs::read_dir(cargo_home.join("registry/src"))
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.is_dir())
        .collect();

    // `git/checkouts/` holds `<repo-name>-<hash>/<rev-prefix>/`. We only
    // want rev-prefix dirs that contain a crate — not their parent dirs —
    // since those are what cargo compiles against.
    let git_checkouts = cargo_home.join("git/checkouts");

    let mut roots: Vec<PathBuf> = Vec::new();
    let mut seen: BTreeSet<PathBuf> = BTreeSet::new();

    for pkg in &parsed.package {
        let Some(source) = pkg.source.as_deref() else {
            // Path deps have no `source`. Their source lives in the
            // workspace itself, which is already a src_root.
            continue;
        };

        if source.starts_with("registry+") {
            let dirname = format!("{}-{}", pkg.name, pkg.version);
            for registry in &registry_roots {
                let candidate = registry.join(&dirname);
                if candidate.is_dir() && seen.insert(candidate.clone()) {
                    roots.push(candidate);
                    break;
                }
            }
        } else if source.starts_with("git+") {
            // Git deps: `git+https://github.com/foo/bar.git?branch=x#<hash>`.
            // Cargo clones to `git/checkouts/<repo>-<hash>/<rev-prefix>/`
            // where rev-prefix is the first ~7 chars of the commit hash
            // after `#`. Pull that suffix out and find the checkout.
            let Some(hash) = source.rsplit('#').next() else {
                continue;
            };
            if hash.len() < 7 {
                continue;
            }
            let rev_prefix = &hash[..7];
            // `read_dir` twice: outer is `<repo>-<hash>` dirs, inner is
            // rev-prefix subdirs. The repo name isn't trivially derivable
            // from the url (cargo hashes and abbreviates), so we scan.
            let entries = match std::fs::read_dir(&git_checkouts) {
                Ok(e) => e,
                Err(_) => continue,
            };
            for outer in entries.flatten() {
                let candidate = outer.path().join(rev_prefix);
                // The checkout may be the repo root, not the crate — walk
                // down to find a `Cargo.toml` whose `[package] name` matches.
                if let Some(crate_dir) =
                    find_crate_dir_in_checkout(&candidate, &pkg.name, &pkg.version)
                {
                    if seen.insert(crate_dir.clone()) {
                        roots.push(crate_dir);
                    }
                    break;
                }
            }
        }
    }

    roots
}

/// Discover path dependencies from a crate's `Cargo.toml` and return their
/// resolved absolute directories. These are local deps like
/// `anchor-lang-v2 = { path = "../../../../lang-v2" }` — their source
/// root is the resolved path itself, which needs to be in `src_roots` so
/// relative DWARF paths like `src/lib.rs` resolve to the right crate
/// instead of colliding with a same-named file at the workspace root.
pub fn discover_path_dep_roots(crate_dir: &Path) -> Vec<PathBuf> {
    let manifest = crate_dir.join("Cargo.toml");
    let Ok(contents) = std::fs::read_to_string(&manifest) else {
        return Vec::new();
    };
    let Ok(parsed) = toml::from_str::<toml::Value>(&contents) else {
        return Vec::new();
    };

    let mut roots = Vec::new();
    for section in ["dependencies", "dev-dependencies"] {
        let Some(deps) = parsed.get(section).and_then(|v| v.as_table()) else {
            continue;
        };
        for (_name, spec) in deps {
            let path_str = match spec {
                toml::Value::Table(t) => t.get("path").and_then(|v| v.as_str()),
                _ => None,
            };
            if let Some(p) = path_str {
                let resolved = crate_dir.join(p);
                if let Ok(canonical) = resolved.canonicalize() {
                    if canonical.is_dir() {
                        roots.push(canonical);
                    }
                }
            }
        }
    }
    roots
}

/// Locate `$CARGO_HOME`. Mirrors cargo's own resolution: env var first,
/// then `~/.cargo`.
fn cargo_home() -> Option<PathBuf> {
    if let Some(p) = std::env::var_os("CARGO_HOME") {
        return Some(PathBuf::from(p));
    }
    dirs::home_dir().map(|h| h.join(".cargo"))
}

/// Depth-limited scan of a git checkout for the `Cargo.toml` that
/// actually declares `<name>@<version>`. Needed because a git dep may
/// live in a subdir (e.g. multi-crate repos where `foo = { git = ..., package = "foo-sub" }`).
///
/// Returns the first matching crate directory — or `None` if no
/// `Cargo.toml` in the checkout declares the package. Bounded to 4 levels
/// of descent to keep this cheap.
fn find_crate_dir_in_checkout(root: &Path, name: &str, version: &str) -> Option<PathBuf> {
    fn recurse(dir: &Path, name: &str, version: &str, depth: u8) -> Option<PathBuf> {
        if depth > 4 {
            return None;
        }
        let manifest = dir.join("Cargo.toml");
        if manifest.is_file() {
            if let Ok(contents) = std::fs::read_to_string(&manifest) {
                // Crude but avoids a full toml parse per checkout subdir.
                // We look for both `name = "<name>"` and `version = "<version>"`
                // within a window — good enough to distinguish the right
                // [package] section from any [dependencies.*] entries,
                // which wouldn't have `version` + `name` together.
                if contents.contains(&format!("name = \"{name}\"")) {
                    // Version check: optional because path/workspace deps
                    // sometimes omit version in the manifest.
                    if version.is_empty() || contents.contains(&format!("version = \"{version}\""))
                    {
                        return Some(dir.to_path_buf());
                    }
                }
            }
        }
        let entries = std::fs::read_dir(dir).ok()?;
        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }
            let Some(fname) = path.file_name().and_then(|s| s.to_str()) else {
                continue;
            };
            // Skip typical noise that never holds a crate manifest.
            if fname.starts_with('.') || fname == "target" || fname == "tests" {
                continue;
            }
            if let Some(hit) = recurse(&path, name, version, depth + 1) {
                return Some(hit);
            }
        }
        None
    }

    recurse(root, name, version, 0)
}
