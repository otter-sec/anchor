use {
    super::common::{find_path, get_program_path},
    crate::{parser::context::CrateContext, AnyResult as Result},
    cargo_toml::Manifest,
    quote::ToTokens,
    std::{
        env, fs,
        path::{Path, PathBuf},
    },
};

pub fn get_external_type(name: &str, path: impl AsRef<Path>) -> Result<Option<syn::Type>> {
    let use_path = get_uses(path.as_ref())?
        .into_iter()
        .find(|u| u.rsplit("::").next() == Some(name))
        .ok_or_else(|| format!("`{name}` not found in use statements"))?;

    // Get crate name and version from lock file
    let program_path = get_program_path()?;
    let lock_path = find_path("Cargo.lock", program_path)?;
    let lock_file = parse_lock_file(lock_path)?;
    let registry_path = get_registry_path()?;

    recursively_find_type(name, &use_path, &registry_path, &lock_file)
}

fn recursively_find_type(
    defined_name: &str,
    use_path: &str,
    registry_path: &Path,
    lock_file: &[(String, String)],
) -> Result<Option<syn::Type>> {
    let crate_name = use_path
        .split("::")
        .next()
        .ok_or("Use path should include a crate name")?;
    let (crate_name, version) = lock_file
        .iter()
        .find(|(name, _)| name == crate_name || name == &crate_name.replace('_', "-"))
        .ok_or("Crate should exist in the lock file")?;

    let crate_path = registry_path.join(format!("{crate_name}-{version}"));
    let lib_path = crate_path.join("src").join("lib.rs");
    let ctx = CrateContext::parse(&lib_path)?;

    // TODO: Struct and enum

    let alias = ctx.type_aliases().find(|item| item.ident == defined_name);
    match alias {
        Some(alias) => Ok(Some(*alias.ty.to_owned())),
        None => {
            // Check re-exported deps e.g. `anchor_lang::solana_program::...`
            let cargo_toml_path = find_path("Cargo.toml", &lib_path)?;
            let deps = Manifest::from_path(cargo_toml_path)?.dependencies;
            let paths = use_path.split("::").skip(1).collect::<Vec<_>>();
            let paths = paths.iter().enumerate().filter_map(|(i, path)| {
                if deps.contains_key(*path) || deps.contains_key(&path.replace('_', "-")) {
                    Some(paths.iter().skip(i).copied().collect::<Vec<_>>().join("::"))
                } else {
                    None
                }
            });
            for path in paths {
                let result = recursively_find_type(defined_name, &path, registry_path, lock_file);
                if result.is_ok() {
                    return result;
                }
            }

            Ok(None)
        }
    }
}

fn get_registry_path() -> Result<PathBuf> {
    let cargo_home = env::var_os("CARGO_HOME")
        .map(PathBuf::from)
        .or_else(|| env::var_os("HOME").map(|home| PathBuf::from(home).join(".cargo")))
        .ok_or("Cargo home not found")?;
    let path = cargo_home.join("registry").join("src");
    fs::read_dir(&path)?
        .filter_map(|entry| entry.ok())
        .find_map(|entry| {
            let file_name = entry.file_name();
            if file_name.to_string_lossy().starts_with("index.crates.io") {
                Some(file_name)
            } else {
                None
            }
        })
        .map(|name| path.join(name))
        .ok_or("crates.io registry not found")
        .map_err(Into::into)
}

fn parse_lock_file(path: impl AsRef<Path>) -> Result<Vec<(String, String)>> {
    let parsed = fs::read_to_string(path.as_ref())?
        .split("[[package]]")
        .skip(1)
        .map(|pkg| {
            let get_value = |key: &str| -> Result<String> {
                let line = pkg
                    .lines()
                    .find(|line| line.starts_with(key))
                    .ok_or_else(|| format!("`{key}` line not found"))?;

                line.split('"')
                    .nth(1)
                    .map(ToOwned::to_owned)
                    .ok_or_else(|| format!("`{key}` value not found"))
                    .map_err(Into::into)
            };
            Ok((get_value("name")?, get_value("version")?))
        })
        .collect::<Result<Vec<_>>>()?;
    Ok(parsed)
}

fn get_uses(path: impl AsRef<Path>) -> Result<Vec<String>> {
    let content = fs::read_to_string(path.as_ref())?;
    let uses = syn::parse_file(&content)?
        .items
        .into_iter()
        .filter_map(|item| match item {
            syn::Item::Use(u) => Some(flatten_uses(&u.tree)),
            _ => None,
        })
        .flatten()
        .collect::<Vec<_>>();
    Ok(uses)
}

fn flatten_uses(tree: &syn::UseTree) -> Vec<String> {
    match tree {
        syn::UseTree::Group(group) => group.items.iter().flat_map(flatten_uses).collect(),
        syn::UseTree::Path(path) => flatten_uses(&path.tree)
            .into_iter()
            .map(|item| format!("{}::{}", path.ident, item))
            .collect(),
        syn::UseTree::Glob(glob) => {
            vec![format!("{}", glob.star_token.to_token_stream().to_string())]
        }
        syn::UseTree::Name(name) => vec![name.ident.to_string()],
        syn::UseTree::Rename(rename) => vec![rename.ident.to_string()],
    }
}
