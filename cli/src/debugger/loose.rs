//! "Loose" mode for `anchor debugger` — runs in any cargo workspace
//! without requiring an `Anchor.toml`.
//!
//! Anchor projects ship an `Anchor.toml` that maps program names to
//! deployed pubkeys and pins the `cargo test` invocation. Bench /
//! research workspaces are plain cargo workspaces that happen to use
//! `anchor-lang-v2` as a library — forcing them to author an Anchor.toml
//! just to use the debugger would be friction for no payoff.
//!
//! This module covers everything the Anchor.toml-driven path provides:
//!
//! - **Workspace root** — walk up from cwd looking for the nearest
//!   `Cargo.toml` declaring `[workspace]`.
//! - **Current package** — read `<cwd>/Cargo.toml` for the `[package] name`.
//! - **Test invocation** — `cargo test --features profile -p <pkg>` from
//!   the workspace root.
//! - **Program → ELF map** — pair `target/deploy/*.so` with the matching
//!   `*-keypair.json` to recover the deployed pubkey.
//!
//! Sanity checks fire as early as possible so the user gets actionable
//! errors instead of opaque "no traces" messages later.

use {
    anyhow::{anyhow, Context, Result},
    serde::Deserialize,
    solana_keypair::read_keypair_file,
    solana_pubkey::Pubkey,
    solana_signer::Signer,
    std::{
        collections::BTreeMap,
        path::{Path, PathBuf},
        process::Command,
        str::FromStr,
    },
};

/// Discovered cargo workspace context.
pub struct LooseWorkspace {
    /// Directory containing the `[workspace]` Cargo.toml.
    pub root: PathBuf,
    /// Directory we were invoked from. May or may not be a member crate
    /// — the user might run from the workspace root itself.
    pub cwd: PathBuf,
    /// `[package] name` from `<cwd>/Cargo.toml`, when the cwd is a crate.
    /// `None` means the user ran from a non-crate dir (e.g. workspace root
    /// with no top-level package); we'll skip the `-p <pkg>` filter.
    pub current_package: Option<String>,
}

#[derive(Deserialize)]
struct CargoToml {
    #[serde(default)]
    package: Option<PackageSection>,
    #[serde(default)]
    workspace: Option<WorkspaceSection>,
    #[serde(default)]
    features: BTreeMap<String, Vec<String>>,
    #[serde(default)]
    #[serde(rename = "dev-dependencies")]
    dev_dependencies: BTreeMap<String, toml::Value>,
}

#[derive(Deserialize)]
struct PackageSection {
    name: String,
}

#[derive(Deserialize)]
struct WorkspaceSection {}

impl LooseWorkspace {
    /// Discover the workspace context starting from `cwd`. Errors only on
    /// the unrecoverable case (no enclosing cargo workspace at all).
    pub fn discover(cwd: PathBuf) -> Result<Self> {
        let root = find_workspace_root(&cwd).ok_or_else(|| {
            anyhow!(
                "no cargo workspace found at or above {} — `anchor debugger` needs either an \
                 Anchor.toml or a cargo `[workspace]` Cargo.toml",
                cwd.display()
            )
        })?;

        // Best-effort: read the cwd's Cargo.toml to learn the package name.
        // Failure is non-fatal (the cwd might be the workspace root itself
        // or a bare directory) — we just skip the `-p <pkg>` filter.
        let current_package = read_cargo_toml(&cwd.join("Cargo.toml"))
            .ok()
            .and_then(|m| m.package.map(|p| p.name));

        Ok(Self {
            root,
            cwd,
            current_package,
        })
    }

    /// Return the dir to invoke `cargo` from. We prefer the current
    /// package's dir when present so its `[package.metadata.*]` settings
    /// take effect; otherwise fall back to the workspace root.
    pub fn cargo_invocation_dir(&self) -> &Path {
        if self.current_package.is_some() {
            &self.cwd
        } else {
            &self.root
        }
    }

    /// Sanity-check the cwd's Cargo.toml exposes a `profile` feature that
    /// activates `anchor-v2-testing/profile`. Returns the discovered
    /// feature name (almost always `"profile"`) or a hard error if we
    /// can't find anything that would trigger trace writing.
    ///
    /// Two acceptable shapes:
    /// 1. `profile = ["anchor-v2-testing/profile"]` — the convention.
    /// 2. Any feature that contains `"anchor-v2-testing/profile"` — for
    ///    workspaces that name it differently (e.g. `tracing`).
    pub fn detect_profile_feature(&self) -> Result<String> {
        let pkg_manifest = self.cwd.join("Cargo.toml");
        let manifest = read_cargo_toml(&pkg_manifest)
            .with_context(|| format!("read {}", pkg_manifest.display()))?;

        // Fast path: the conventional name.
        let convention = "profile";
        if manifest.features.get(convention).map_or(false, |v| {
            v.iter().any(|s| s == "anchor-v2-testing/profile")
        }) {
            return Ok(convention.to_owned());
        }

        // Slow path: any feature that propagates to anchor-v2-testing/profile.
        for (name, deps) in &manifest.features {
            if deps.iter().any(|s| s == "anchor-v2-testing/profile") {
                return Ok(name.clone());
            }
        }

        Err(anyhow!(
            "no cargo feature in {pkg} forwards to `anchor-v2-testing/profile`.\n\nAdd this to \
             {manifest_path}:\n  [features]\n  profile = [\"anchor-v2-testing/profile\"]\n\nTests \
             don't need any cfg gates — `anchor_v2_testing::svm()` is\n`LiteSVM::new()` by \
             default and switches to the trace-recording\nvariant automatically when this feature \
             is on.",
            pkg = self
                .current_package
                .as_deref()
                .unwrap_or("the current crate"),
            manifest_path = pkg_manifest.display(),
        ))
    }

    /// Sanity-check that `anchor-v2-testing` is actually a dev-dependency.
    /// Without it, `--features profile` would fail with an opaque cargo
    /// error; we'd rather flag this up front.
    pub fn check_dev_dep(&self) -> Result<()> {
        let pkg_manifest = self.cwd.join("Cargo.toml");
        let manifest = read_cargo_toml(&pkg_manifest)
            .with_context(|| format!("read {}", pkg_manifest.display()))?;
        if !manifest.dev_dependencies.contains_key("anchor-v2-testing") {
            return Err(anyhow!(
                "{} doesn't list `anchor-v2-testing` as a dev-dependency.\nAdd it under \
                 [dev-dependencies] before running `anchor debugger`.",
                pkg_manifest.display()
            ));
        }
        Ok(())
    }
}

/// Walk up from `start` to the nearest `Cargo.toml` declaring a
/// `[workspace]` table. Returns the directory containing it.
///
/// Falls back to the start dir's `Cargo.toml` if it has `[workspace]`,
/// otherwise keeps walking. `None` means we hit the filesystem root with
/// no match — caller treats that as a hard error.
fn find_workspace_root(start: &Path) -> Option<PathBuf> {
    let mut cur: PathBuf = start.to_path_buf();
    loop {
        let manifest = cur.join("Cargo.toml");
        if manifest.is_file() {
            if let Ok(parsed) = read_cargo_toml(&manifest) {
                if parsed.workspace.is_some() {
                    return Some(cur);
                }
            }
        }
        if !cur.pop() {
            return None;
        }
    }
}

fn read_cargo_toml(path: &Path) -> Result<CargoToml> {
    let contents =
        std::fs::read_to_string(path).with_context(|| format!("read {}", path.display()))?;
    toml::from_str(&contents).with_context(|| format!("parse {}", path.display()))
}

/// Walk the workspace's SBF build artifacts and return a base58
/// program-id → `.so` path map. Two location families are considered:
///
/// 1. **`target/deploy/`** — `cargo-build-sbf`'s default output. Always
///    preferred when the same `<lib>.so` exists in both locations,
///    because this is the post-link form solana-sbpf can parse.
/// 2. **`target/sbpf-solana-solana/release/`** — the cargo target dir
///    when SBF builds are driven directly (e.g. bench workspaces). Used
///    as a fallback only.
///
/// For each chosen `.so`, **every** pubkey we can associate with it is
/// added as a key:
///
/// - The pubkey from the sibling `<lib>-keypair.json` (if present).
/// - The pubkey from `declare_id!("...")` in the crate's source (if the
///   crate uses anchor's `declare_id!` macro).
///
/// Mapping all known pubkeys to the same `.so` matters because
/// `cargo-build-sbf` generates a *fresh* random keypair on first build
/// — `declare_id!` (the runtime id used by `Program::id()` and seen in
/// traces) won't match the keypair file unless the user runs
/// `anchor keys sync`. Without dual-mapping, the trace's pubkey
/// resolves through the unstripped fallback path and ELF parse fails.
///
/// Defensive throughout: missing dirs are not errors (just skipped),
/// 0-byte artifacts are skipped, malformed keypairs / declare_id
/// literals are skipped. Empty result is legal — the caller surfaces a
/// clear "build first?" message rather than failing here.
pub fn discover_programs(
    workspace_root: &Path,
    current_package: Option<&str>,
) -> Result<BTreeMap<String, PathBuf>> {
    // Pre-cache the lib_name → pubkey map from `declare_id!` source scan.
    let lib_to_declare_id = scan_declare_ids(workspace_root);

    // Collect candidate (lib_name → preferred .so path). `target/deploy/`
    // always wins because it's the only form solana-sbpf can parse;
    // `target/sbpf-solana-solana/release/` is a fallback for workspaces
    // that haven't run `cargo build-sbf`.
    let mut lib_to_so: BTreeMap<String, PathBuf> = BTreeMap::new();

    let deploy_dir = workspace_root.join("target").join("deploy");
    if deploy_dir.is_dir() {
        collect_so_paths(&deploy_dir, &mut lib_to_so);
    }
    let sbf_release = workspace_root
        .join("target")
        .join("sbpf-solana-solana")
        .join("release");
    if sbf_release.is_dir() {
        // Only fill gaps deploy/ didn't cover.
        collect_so_paths_if_missing(&sbf_release, &mut lib_to_so);
    }

    // Build the final pubkey → .so map. For each chosen .so we associate
    // all known pubkeys (keypair file in deploy/ + declare_id! in source).
    let mut out: BTreeMap<String, PathBuf> = BTreeMap::new();
    for (lib_name, so_path) in &lib_to_so {
        let mut pubkeys: Vec<String> = Vec::with_capacity(2);

        // Source declare_id! — almost always what the runtime sees.
        if let Some(pk) = lib_to_declare_id.get(lib_name) {
            pubkeys.push(pk.clone());
        }

        // Sibling keypair pubkey — present when cargo-build-sbf wrote
        // here, may differ from declare_id! when keys aren't synced.
        let keypair_path = deploy_dir.join(format!("{lib_name}-keypair.json"));
        if keypair_path.is_file() {
            if let Ok(kp) = read_keypair_file(&keypair_path) {
                pubkeys.push(kp.pubkey().to_string());
            }
        }

        // Filename-as-pubkey: the .so stem itself parses as a valid
        // 32-byte base58 pubkey. This is how `solana program dump <pk>`
        // names its output — lets blackbox binaries (mainnet dumps, no
        // source, no keypair) drop straight into `target/deploy/` with
        // no scaffolding. Stricter than a length + alphabet check
        // because `Pubkey::from_str` also enforces the 32-byte decode.
        if pubkeys.is_empty() {
            if let Ok(pk) = Pubkey::from_str(lib_name) {
                pubkeys.push(pk.to_string());
            }
        }

        if pubkeys.is_empty() {
            // No discoverable id — skip rather than poison the map.
            continue;
        }
        // The current package (the crate the user invoked the debugger
        // from) always wins when multiple libs share the same pubkey.
        // This handles the common bench layout where v1 and v2 both
        // declare the same program id for apples-to-apples comparison.
        let is_current = current_package
            .map(|pkg| lib_name.replace('-', "_") == pkg.replace('-', "_"))
            .unwrap_or(false);
        for pk in pubkeys {
            if is_current {
                out.insert(pk, so_path.clone());
            } else {
                out.entry(pk).or_insert_with(|| so_path.clone());
            }
        }
    }
    Ok(out)
}

/// Insert each `.so` in `dir` keyed by its `file_stem` (lib name).
/// Replaces any prior entry — used for the preferred location pass.
fn collect_so_paths(dir: &Path, out: &mut BTreeMap<String, PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|s| s.to_str()) != Some("so") {
            continue;
        }
        if path.metadata().map(|m| m.len() == 0).unwrap_or(true) {
            continue;
        }
        let Some(stem) = path.file_stem().and_then(|s| s.to_str()) else {
            continue;
        };
        out.insert(stem.to_owned(), path);
    }
}

/// Same as [`collect_so_paths`] but only fills entries that aren't
/// already present — used for the fallback location pass.
fn collect_so_paths_if_missing(dir: &Path, out: &mut BTreeMap<String, PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|s| s.to_str()) != Some("so") {
            continue;
        }
        if path.metadata().map(|m| m.len() == 0).unwrap_or(true) {
            continue;
        }
        let Some(stem) = path.file_stem().and_then(|s| s.to_str()) else {
            continue;
        };
        out.entry(stem.to_owned()).or_insert(path);
    }
}

/// Walk every `lib.rs` / `main.rs` under the workspace looking for
/// `declare_id!("...")`. For each match, pair the pubkey with the lib
/// name declared in the enclosing crate's `Cargo.toml` (or the package
/// name with `-` → `_` if `[lib] name` is omitted) so callers can match
/// by `.so` file stem.
///
/// Best-effort: read failures, parse failures, and crates without a
/// `declare_id!` are silently skipped. The map is small (typically ≤ ~20
/// programs in a bench workspace) so even a full traversal is cheap.
fn scan_declare_ids(workspace_root: &Path) -> BTreeMap<String, String> {
    let mut out = BTreeMap::new();
    walk_declare_ids(workspace_root, &mut out, 0);
    out
}

fn walk_declare_ids(dir: &Path, out: &mut BTreeMap<String, String>, depth: u8) {
    // Hard cap on recursion. Workspaces with members 6 levels deep are
    // exotic; this keeps us out of pathological symlink loops.
    if depth > 8 {
        return;
    }
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            // Skip well-known build / vendor dirs that can never contain
            // a workspace member.
            let Some(name) = path.file_name().and_then(|s| s.to_str()) else {
                continue;
            };
            if matches!(name, "target" | "node_modules" | ".git") || name.starts_with('.') {
                continue;
            }
            walk_declare_ids(&path, out, depth + 1);
        } else if path.file_name() == Some(std::ffi::OsStr::new("lib.rs"))
            || path.file_name() == Some(std::ffi::OsStr::new("main.rs"))
        {
            if let Some((lib_name, pubkey)) = extract_id_pair(&path) {
                out.entry(lib_name).or_insert(pubkey);
            }
        }
    }
}

/// Pull `declare_id!("...")` from the source file and the lib name from
/// the nearest enclosing `Cargo.toml`. Returns `None` if either is
/// missing or malformed.
fn extract_id_pair(src_file: &Path) -> Option<(String, String)> {
    let contents = std::fs::read_to_string(src_file).ok()?;
    let pubkey = find_declare_id(&contents)?;

    // Walk up to the nearest Cargo.toml (within this crate's tree).
    let mut cur = src_file.parent()?;
    loop {
        let manifest = cur.join("Cargo.toml");
        if manifest.is_file() {
            let Ok(parsed) = read_cargo_toml(&manifest) else {
                return None;
            };
            // Prefer [lib] name; fall back to package.name with kebab → snake.
            let lib_name = lib_name_from_manifest(&manifest)
                .or_else(|| parsed.package.map(|p| p.name.replace('-', "_")))?;
            return Some((lib_name, pubkey));
        }
        cur = cur.parent()?;
    }
}

/// `[lib] name` is in a separate raw-toml lookup because we don't want to
/// bake every Cargo.toml shape into the strongly-typed `CargoToml`
/// struct — the schema is fluid.
fn lib_name_from_manifest(manifest: &Path) -> Option<String> {
    let s = std::fs::read_to_string(manifest).ok()?;
    let v: toml::Value = toml::from_str(&s).ok()?;
    v.get("lib")?.get("name")?.as_str().map(str::to_owned)
}

/// Find the first `declare_id!("...")` literal in `src`. Tolerant of
/// whitespace and the alternate `declare_id! ( ... )` paren style.
fn find_declare_id(src: &str) -> Option<String> {
    let idx = src.find("declare_id!")?;
    let after = &src[idx + "declare_id!".len()..];
    let after = after.trim_start_matches(|c: char| c.is_whitespace());
    // Accept `(` or `[` or `{` as the macro delimiter; we only need the
    // first quoted string after.
    let after = after.trim_start_matches(['(', '[', '{']);
    let quote = after.find('"')?;
    let body = &after[quote + 1..];
    let end = body.find('"')?;
    let id = &body[..end];
    // Pubkey sanity: base58 alphabet, 32-44 chars.
    if id.len() < 32 || id.len() > 44 {
        return None;
    }
    if !id
        .bytes()
        .all(|b| b.is_ascii_alphanumeric() && b != b'0' && b != b'O' && b != b'I' && b != b'l')
    {
        return None;
    }
    Some(id.to_owned())
}

/// Run `cargo build-sbf -p <pkg>` from the workspace root. This produces
/// the post-linked `.so` + sibling keypair under `target/deploy/` that
/// solana-sbpf can parse — the raw `cargo build --target sbpf-solana-solana`
/// artifact in `target/sbpf-solana-solana/release/` is missing relocation
/// metadata our debugger needs.
///
/// Skipping this step is the most common cause of "the debugger sees the
/// program but the disasm pane is empty". We surface that explicitly via
/// the `target/deploy/` check so users know what to fix.
/// Run `cargo test --features <feature> -p <pkg>` from `cwd`, with the
/// profile-mode env vars set the same way the Anchor.toml flow sets them.
///
/// Inherits stdio so the user sees test output exactly as they would with
/// a direct `cargo test`. Returns an error on non-zero exit so we don't
/// drop into the TUI on a build/test failure (would otherwise confuse
/// users about why nothing's there).
pub fn run_cargo_test(
    cwd: &Path,
    package: Option<&str>,
    feature: &str,
    profile_dir: &Path,
    test_filter: Option<&str>,
) -> Result<()> {
    // Mirror what the Anchor.toml flow sets. The env vars only affect the
    // child cargo invocation; nothing we do here leaks into the user's
    // shell.
    let mut cmd = Command::new("cargo");
    cmd.current_dir(cwd)
        .env("ANCHOR_PROFILE_DIR", profile_dir)
        .env("CARGO_PROFILE_RELEASE_DEBUG", "2")
        .arg("test")
        .arg("--features")
        .arg(feature);
    if let Some(pkg) = package {
        cmd.arg("-p").arg(pkg);
    }
    if let Some(filter) = test_filter {
        // After `cargo test [opts] [--] <filter>`. Cargo passes the filter
        // through to libtest as a substring match — exactly how the user
        // would run `cargo test my_specific_test`.
        cmd.arg("--").arg(filter);
    }

    let status = cmd
        .status()
        .with_context(|| format!("spawn cargo test in {}: is `cargo` on PATH?", cwd.display()))?;
    if !status.success() {
        return Err(anyhow!(
            "cargo test failed (exit {:?}). Fix test errors before stepping into the debugger.",
            status.code()
        ));
    }
    Ok(())
}

/// Wipe the per-test trace dir before a fresh run so stale traces from a
/// previous session don't leak into the new picker. Idempotent — missing
/// dir is fine.
pub fn clear_profile_dir(profile_dir: &Path) -> Result<()> {
    match std::fs::remove_dir_all(profile_dir) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(anyhow::Error::new(e)
            .context(format!("clear stale profile dir {}", profile_dir.display()))),
    }
}
