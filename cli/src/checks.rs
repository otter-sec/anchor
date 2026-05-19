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

/// Check whether there is a mismatch between the current CLI version and:
///
/// - `anchor-lang` crate version
/// - `@anchor-lang/core` package version
///
/// This function logs warnings in the case of a mismatch.
pub fn check_anchor_version(cfg: &WithPath<Config>) -> Result<()> {
    let cli_version = Version::parse(VERSION)?;

    // Check lang crate
    let mismatched_lang_version = cfg
        .get_rust_program_list()?
        .into_iter()
        .map(|path| path.join("Cargo.toml"))
        .map(cargo_toml::Manifest::from_path)
        .filter_map(|man| man.ok())
        .filter_map(|man| man.dependencies.get("anchor-lang").map(|d| d.to_owned()))
        .filter_map(|dep| Version::parse(dep.req()).ok())
        .find(|ver| ver != &cli_version); // Only log the warning once

    if let Some(ver) = mismatched_lang_version {
        eprintln!(
            "WARNING: `anchor-lang` version({ver}) and the current CLI version({cli_version}) \
             don't match.\n\n\tThis can lead to unwanted behavior. To use the same CLI version, \
             add:\n\n\t[toolchain]\n\tanchor_version = \"{ver}\"\n\n\tto Anchor.toml\n"
        );
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

    // Check whether the manifest has `idl-build` feature
    let has_idl_build_feature = manifest
        .features
        .iter()
        .any(|(feature, _)| feature == "idl-build");
    if !has_idl_build_feature {
        let anchor_spl_idl_build = if manifest
            .dependencies
            .iter()
            .any(|dep| dep.0 == "anchor-spl")
        {
            r#", "anchor-spl/idl-build""#
        } else {
            ""
        };

        return Err(anyhow!(
            r#"`idl-build` feature is missing. To solve, add

[features]
idl-build = ["anchor-lang/idl-build"{anchor_spl_idl_build}]

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

    // Check `anchor-spl`'s `idl-build` feature
    manifest
        .dependencies
        .get("anchor-spl")
        .and_then(|_| manifest.features.get("idl-build"))
        .map(|feature_list| !feature_list.contains(&"anchor-spl/idl-build".into()))
        .unwrap_or_default()
        .then(|| {
            eprintln!(
                "WARNING: `idl-build` feature of `anchor-spl` is not enabled. This is likely to \
                 result in cryptic compile errors.\n\n\tTo solve, add `anchor-spl/idl-build` to \
                 the `idl-build` feature list:\n\n\t[features]\n\tidl-build = \
                 [\"anchor-spl/idl-build\", ...]\n"
            )
        });

    Ok(())
}
