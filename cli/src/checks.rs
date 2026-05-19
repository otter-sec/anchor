use {
    crate::{
        config::{Config, Manifest, PackageManager, WithPath},
        VERSION,
    },
    anyhow::{anyhow, Result},
    semver::{Version, VersionReq},
    std::{fs, path::Path},
};

/// Check whether `overflow-checks` codegen option is enabled.
///
/// https://doc.rust-lang.org/rustc/codegen-options/index.html#overflow-checks
pub fn check_overflow(cargo_toml_path: impl AsRef<Path>) -> Result<bool> {
    Manifest::from_path(cargo_toml_path)?
        .profile
        .release
        .as_ref()
        .and_then(|profile| profile.overflow_checks)
        .ok_or(anyhow!(
            "`overflow-checks` is not enabled. To enable, \
             add:\n\n[profile.release]\noverflow-checks = true\n\nin workspace root Cargo.toml",
        ))
}

/// Per-manifest detection of which Anchor generation a program targets.
/// Returns `(lang_crate, spl_crate)` — `("anchor-lang", "anchor-spl")` for v1,
/// `("anchor-lang-v2", "anchor-spl-v2")` for v2, `None` if neither lang crate
/// is depended on. Callers use this to surface generation-appropriate crate
/// names in diagnostics instead of hardcoding the v1 names.
fn anchor_crate_names(manifest: &cargo_toml::Manifest) -> Option<(&'static str, &'static str)> {
    if manifest.dependencies.contains_key("anchor-lang-v2") {
        Some(("anchor-lang-v2", "anchor-spl-v2"))
    } else if manifest.dependencies.contains_key("anchor-lang") {
        Some(("anchor-lang", "anchor-spl"))
    } else {
        None
    }
}

/// Check whether there is a mismatch between the current CLI version and:
///
/// - `anchor-lang` / `anchor-lang-v2` crate version
/// - `@anchor-lang/core` package version
///
/// This function logs warnings in the case of a mismatch.
pub fn check_anchor_version(cfg: &WithPath<Config>) -> Result<()> {
    let cli_version = Version::parse(VERSION)?;

    // Check lang crate. Probes both v1 and v2 dep names independently so a
    // mid-migration workspace that contains programs of both generations
    // still gets one warning per mismatched generation.
    let manifests: Vec<cargo_toml::Manifest> = cfg
        .get_rust_program_list()?
        .into_iter()
        .map(|path| path.join("Cargo.toml"))
        .filter_map(|path| cargo_toml::Manifest::from_path(path).ok())
        .collect();

    for dep_name in ["anchor-lang", "anchor-lang-v2"] {
        let mismatched = manifests
            .iter()
            .filter_map(|m| m.dependencies.get(dep_name))
            .filter_map(|dep| Version::parse(dep.req()).ok())
            .find(|ver| ver != &cli_version);
        if let Some(ver) = mismatched {
            eprintln!(
                "WARNING: `{dep_name}` version({ver}) and the current CLI version({cli_version}) \
                 don't match.\n\n\tThis can lead to unwanted behavior. To use the same CLI \
                 version, add:\n\n\t[toolchain]\n\tanchor_version = \"{ver}\"\n\n\tto \
                 Anchor.toml\n"
            );
        }
    }

    // Check TS package
    let package_json = {
        let package_json_path = cfg.path().parent().unwrap().join("package.json");
        let package_json_content = fs::read_to_string(package_json_path)?;
        serde_json::from_str::<serde_json::Value>(&package_json_content)?
    };
    let mismatched_ts_version = package_json
        .get("dependencies")
        .and_then(|deps| deps.get("@anchor-lang/core"))
        .and_then(|ver| ver.as_str())
        .and_then(|ver| VersionReq::parse(ver).ok())
        .filter(|ver| !ver.matches(&cli_version));

    if let Some(ver) = mismatched_ts_version {
        // Cosmetic hint only. Prefer what Anchor.toml says the project uses;
        // otherwise default the suggestion to `npm` since it ships with Node.
        // Not using `resolve_package_manager` here — probing PATH for a
        // warning message would be overkill.
        let update_cmd = match cfg
            .toolchain
            .package_manager
            .clone()
            .unwrap_or(PackageManager::NPM)
        {
            PackageManager::NPM => "npm update",
            PackageManager::Yarn => "yarn upgrade",
            PackageManager::PNPM => "pnpm update",
            PackageManager::Bun => "bun update",
        };

        eprintln!(
            "WARNING: `@anchor-lang/core` version({ver}) and the current CLI \
             version({cli_version}) don't match.\n\n\tThis can lead to unwanted behavior. To fix, \
             upgrade the package by running:\n\n\t{update_cmd} @anchor-lang/core@{cli_version}\n"
        );
    }

    Ok(())
}

/// Check for potential dependency improvements.
///
/// The main problem people will run into with Solana version bumps is that the `solana-program` version
/// specified in users' `Cargo.toml` might be incompatible with `anchor-lang`'s dependency.
/// To fix this and similar problems, users should use the crates exported from `anchor-lang` or
/// `anchor-spl` when possible.
pub fn check_deps(cfg: &WithPath<Config>) -> Result<()> {
    // Check `solana-program`
    /// Check if this version requirement matches the one listed in our workspace
    fn compatible_solana_program(version_req: &str) -> bool {
        let Ok(req) = VersionReq::parse(version_req) else {
            // Assume incompatible if parsing fails
            return false;
        };
        let version = include_str!("../solana-program-version").trim();
        let workspace_solana_prog_version = semver::Version::parse(version).unwrap();
        req.matches(&workspace_solana_prog_version)
    }

    cfg.get_rust_program_list()?
        .into_iter()
        .map(|path| path.join("Cargo.toml"))
        .map(cargo_toml::Manifest::from_path)
        .map(|man| man.map_err(|e| anyhow!("Failed to read manifest: {e}")))
        .collect::<Result<Vec<_>>>()?
        .into_iter()
        .filter(|man| {
            man.dependencies
                .get("solana-program")
                .is_some_and(|dep| match dep {
                    cargo_toml::Dependency::Simple(version) => !compatible_solana_program(version),
                    cargo_toml::Dependency::Detailed(detail) => {
                        if let Some(version) = &detail.version {
                            !compatible_solana_program(version)
                        } else {
                            // Conservatively warn on non-version dependencies
                            true
                        }
                    }
                    // Conservatively warn on inherited dependencies
                    _ => true,
                })
        })
        .for_each(|man| {
            eprintln!(
                "WARNING: Adding `solana-program` as a separate dependency might cause \
                 conflicts.\nTo solve, remove the `solana-program` dependency and use the \
                 exported crate from `anchor-lang`.\n`use solana_program` becomes `use \
                 anchor_lang::solana_program`.\nProgram name: `{}`\n",
                man.package().name()
            )
        });

    Ok(())
}

/// Check whether the `idl-build` feature is being used correctly.
///
/// **Note:** The check expects the current directory to be a program directory.
pub fn check_idl_build_feature() -> Result<()> {
    let manifest_path = Path::new("Cargo.toml").canonicalize()?;
    let manifest = Manifest::from_path(&manifest_path)?;

    // Pick crate names based on the generation this program targets. If
    // neither `anchor-lang` nor `anchor-lang-v2` is a dep, fall back to v1
    // names — that branch's suggestion is harmless (the program has bigger
    // problems than `idl-build` wiring) and keeps the diagnostic flow simple.
    let (lang_crate, spl_crate) =
        anchor_crate_names(&manifest).unwrap_or(("anchor-lang", "anchor-spl"));

    // Check whether the manifest has `idl-build` feature
    let has_idl_build_feature = manifest
        .features
        .iter()
        .any(|(feature, _)| feature == "idl-build");
    if !has_idl_build_feature {
        let anchor_spl_idl_build = if manifest.dependencies.contains_key(spl_crate) {
            format!(r#", "{spl_crate}/idl-build""#)
        } else {
            String::new()
        };

        return Err(anyhow!(
            r#"`idl-build` feature is missing. To solve, add

[features]
idl-build = ["{lang_crate}/idl-build"{anchor_spl_idl_build}]

in `{manifest_path:?}`."#
        ));
    }

    // Check if `idl-build` is enabled by default
    manifest
        .dependencies
        .iter()
        .filter(|(_, dep)| dep.req_features().contains(&"idl-build".into()))
        .for_each(|(name, _)| {
            eprintln!(
                "WARNING: `idl-build` feature of crate `{name}` is enabled by default. This is \
                 not the intended usage.\n\n\tTo solve, do not enable the `idl-build` feature and \
                 include crates that have `idl-build` feature in the `idl-build` feature \
                 list:\n\n\t[features]\n\tidl-build = [\"{name}/idl-build\", ...]\n"
            )
        });

    // Check that the SPL crate's `idl-build` feature is in the feature list.
    let spl_feature = format!("{spl_crate}/idl-build");
    manifest
        .dependencies
        .get(spl_crate)
        .and_then(|_| manifest.features.get("idl-build"))
        .map(|feature_list| !feature_list.contains(&spl_feature))
        .unwrap_or_default()
        .then(|| {
            eprintln!(
                "WARNING: `idl-build` feature of `{spl_crate}` is not enabled. This is likely to \
                 result in cryptic compile errors.\n\n\tTo solve, add `{spl_feature}` to the \
                 `idl-build` feature list:\n\n\t[features]\n\tidl-build = [\"{spl_feature}\", \
                 ...]\n"
            )
        });

    Ok(())
}
