//! Codama IDL integration for the Anchor CLI.
//!
//! - `anchor codama convert <path>` translates a (post-0.30, v0.1.x spec)
//!   Anchor IDL JSON file into a Codama IDL JSON tree rooted at a `rootNode`.
//!   The conversion mirrors the reference TypeScript implementation shipped
//!   in `@codama/nodes-from-anchor` (`v01/`), so the output should be
//!   byte-stable against the JS toolchain modulo property ordering.
//! - `anchor codama generate -l <langs> -p <path> <idl>` first runs the same
//!   conversion in-process, then drives `@codama/cli` to render clients in
//!   the requested languages (`js`, `js-umi`, `rust`, `go`).

use {
    crate::AbsolutePath,
    anyhow::{anyhow, bail, Context, Result},
    clap::{Parser, ValueEnum},
    serde_json::{json, Map, Value as JsonValue},
    std::{
        collections::{BTreeSet, HashMap},
        fs,
        path::{Path, PathBuf},
        process::{Command, Stdio},
    },
};

/// The `@codama/nodes` package version we target. The Codama IDL is versioned
/// independently from the Anchor IDL, and the JS converter stamps the running
/// `@codama/nodes` version into `rootNode.version`. We pin a known good value
/// so consumers that key off this field have a deterministic input.
const CODAMA_VERSION: &str = "1.6.0";

#[derive(Debug, Parser, AbsolutePath)]
pub enum CodamaCommand {
    /// Convert an Anchor IDL JSON file (post-0.30 spec) into a Codama IDL
    /// rooted at a `rootNode`.
    Convert {
        /// Path to the Anchor IDL JSON file.
        path: String,
        /// Output file (stdout if not specified).
        #[clap(short, long)]
        out: Option<String>,
    },
    /// Convert an Anchor IDL and run Codama renderers to produce client
    /// libraries in one or more languages.
    ///
    /// The IDL is converted in-process; the resulting Codama IDL is handed to
    /// `@codama/cli` (run via `npx --yes codama` by default), which loads the
    /// per-language renderer packages and writes generated sources under
    /// `<path>/<language>`.
    Generate {
        /// Languages to generate clients for. Repeat the flag or comma-
        /// separate values: `-l js,go -l rust`.
        #[clap(
            short = 'l',
            long = "language",
            value_delimiter = ',',
            value_enum,
            required = true
        )]
        language: Vec<Language>,
        /// Base output directory; per-language clients are written to
        /// `<path>/<language>`.
        #[clap(short = 'p', long = "path", default_value = "clients")]
        path: String,
        /// Path to the Anchor IDL JSON file.
        idl: String,
    },
}

/// Languages with an officially-published `@codama/renderers-*` package.
#[derive(Debug, Clone, Copy, ValueEnum, Eq, Ord, PartialEq, PartialOrd, AbsolutePath)]
#[clap(rename_all = "kebab-case")]
pub enum Language {
    Js,
    JsUmi,
    Rust,
    Go,
}

impl Language {
    /// Stable identifier used both as the Codama script name and the
    /// per-language output subdirectory.
    pub fn id(self) -> &'static str {
        match self {
            Language::Js => "js",
            Language::JsUmi => "js-umi",
            Language::Rust => "rust",
            Language::Go => "go",
        }
    }

    /// Inverse of [`Self::id`]. Returns `None` for unknown ids so callers
    /// can decide whether to error or silently skip.
    pub fn from_id(id: &str) -> Option<Self> {
        match id {
            "js" => Some(Language::Js),
            "js-umi" => Some(Language::JsUmi),
            "rust" => Some(Language::Rust),
            "go" => Some(Language::Go),
            _ => None,
        }
    }

    /// npm package providing the renderer's default export.
    pub fn renderer_package(self) -> &'static str {
        match self {
            Language::Js => "@codama/renderers-js",
            Language::JsUmi => "@codama/renderers-js-umi",
            Language::Rust => "@codama/renderers-rust",
            Language::Go => "@codama/renderers-go",
        }
    }
}

pub fn entry(cmd: CodamaCommand) -> Result<()> {
    match cmd {
        CodamaCommand::Convert { path, out } => convert(path, out),
        CodamaCommand::Generate {
            language,
            path,
            idl,
        } => generate(idl, path, language),
    }
}

pub fn convert(path: String, out: Option<String>) -> Result<()> {
    let bytes = fs::read(&path).with_context(|| format!("Failed to read IDL file `{path}`"))?;
    let idl: JsonValue = serde_json::from_slice(&bytes)
        .with_context(|| format!("Failed to parse IDL JSON at `{path}`"))?;
    let root = root_node_from_anchor(&idl)?;
    let json = serde_json::to_string_pretty(&root)?;
    match out {
        Some(out) => fs::write(out, json)?,
        None => println!("{json}"),
    }
    Ok(())
}

/// Convert the Anchor IDL at `idl_path` to a Codama IDL, then drive the
/// Codama CLI to render clients for each requested language under
/// `<base_path>/<language>`.
///
/// Implementation notes:
/// - Conversion happens in-process via [`root_node_from_anchor`]. The
///   converted Codama IDL is staged under `<base_path>/.codama/idl.json`
///   alongside a generated `codama.json` config so the user can inspect or
///   re-run the rendering manually with `codama run --all`.
/// - The Codama CLI is invoked as `npx --yes codama run --config <cfg>
///   --all` by default. Set `ANCHOR_CODAMA_CMD` to override the binary
///   (e.g. when `codama` is already on `PATH`); arguments are appended as
///   given. Codama itself installs missing renderer packages on demand.
pub fn generate(idl_path: String, base_path: String, languages: Vec<Language>) -> Result<()> {
    if languages.is_empty() {
        bail!("`anchor codama generate` requires at least one --language");
    }
    // Dedup while keeping a deterministic order for the generated config so
    // re-runs produce identical files.
    let unique: BTreeSet<Language> = languages.into_iter().collect();
    let base = PathBuf::from(&base_path);
    let targets: Vec<(Language, PathBuf)> =
        unique.iter().map(|l| (*l, base.join(l.id()))).collect();
    let stage_dir = base.join(".codama");
    render_targets(Path::new(&idl_path), &stage_dir, &targets)
}

/// Convenience entry point for the `anchor build` integration: reads the
/// `[clients]` section of `Anchor.toml`, expands it into resolved
/// `(Language, output_path)` targets, and runs Codama for each IDL produced
/// by the build.
///
/// `workspace_dir` is the root of the workspace (the directory containing
/// `Anchor.toml`); IDL files are expected at `<workspace_dir>/target/idl/*.json`
/// (the standard `anchor build` output). When the workspace ships more than
/// one program the configured per-language path is treated as a *base*
/// directory and clients land at `<base>/<program>` to avoid clobbering.
pub fn auto_generate_for_workspace(
    clients_cfg: &crate::config::ClientsConfig,
    workspace_dir: &Path,
    idl_paths: &[PathBuf],
) -> Result<()> {
    if !clients_cfg.auto {
        return Ok(());
    }
    let base = workspace_dir.join("clients");
    let entries = clients_cfg.enabled(&base);
    if entries.is_empty() {
        eprintln!(
            "warning: `[clients] auto = true` but no language is enabled — nothing to generate.",
        );
        return Ok(());
    }
    if idl_paths.is_empty() {
        eprintln!(
            "warning: `[clients] auto = true` but no IDL files were produced by the build — \
             nothing to generate.",
        );
        return Ok(());
    }

    let multi_program = idl_paths.len() > 1;
    let codama_stage_root = workspace_dir.join("target").join("codama");
    for idl_path in idl_paths {
        let stem = idl_path
            .file_stem()
            .and_then(|s| s.to_str())
            .ok_or_else(|| anyhow!("Invalid IDL filename: {}", idl_path.display()))?;
        let targets: Vec<(Language, PathBuf)> = entries
            .iter()
            .filter_map(|(id, path)| {
                let lang = Language::from_id(id)?;
                let out = if multi_program {
                    path.join(stem)
                } else {
                    path.clone()
                };
                Some((lang, out))
            })
            .collect();
        if targets.is_empty() {
            continue;
        }
        let stage_dir = codama_stage_root.join(stem);
        render_targets(idl_path, &stage_dir, &targets)?;
    }
    Ok(())
}

/// Shared rendering backend used by both the `anchor codama generate`
/// subcommand and the `anchor build` auto-generation hook.
///
/// Steps:
/// 1. Convert the Anchor IDL at `idl_path` to a Codama IDL JSON tree.
/// 2. Stage the converted IDL + a generated `codama.json` config under
///    `stage_dir/`. Keeping the stage on disk (rather than in `$TMPDIR`)
///    means failures leave a debuggable artifact and `codama run --all`
///    can be re-invoked manually.
/// 3. Spawn the Codama CLI (`npx --yes codama` by default; see
///    [`run_codama`] for the override knob) which loads each renderer
///    package and writes generated sources into the per-language paths.
///
/// All paths in the generated config are absolute: Codama forwards visitor
/// args to the renderer verbatim and the renderer's `node:fs` calls resolve
/// relative paths against the *runtime* cwd, which would otherwise depend
/// on where the user invoked `anchor` from.
fn render_targets(
    idl_path: &Path,
    stage_dir: &Path,
    targets: &[(Language, PathBuf)],
) -> Result<()> {
    if targets.is_empty() {
        return Ok(());
    }

    let bytes = fs::read(idl_path)
        .with_context(|| format!("Failed to read IDL file `{}`", idl_path.display()))?;
    let idl: JsonValue = serde_json::from_slice(&bytes)
        .with_context(|| format!("Failed to parse IDL JSON at `{}`", idl_path.display()))?;
    let root = root_node_from_anchor(&idl)?;

    fs::create_dir_all(stage_dir).with_context(|| {
        format!(
            "Failed to create staging directory `{}`",
            stage_dir.display()
        )
    })?;
    let staged_idl = stage_dir.join("idl.json");
    fs::write(&staged_idl, serde_json::to_string_pretty(&root)?)
        .with_context(|| format!("Failed to write `{}`", staged_idl.display()))?;

    // Build per-target output dirs eagerly so `canonicalize` succeeds; the
    // renderers themselves will (re)create + clean them, but they must exist
    // for path resolution.
    for (_lang, out) in targets {
        fs::create_dir_all(out)
            .with_context(|| format!("Failed to create output directory `{}`", out.display()))?;
    }

    let abs_idl = staged_idl
        .canonicalize()
        .with_context(|| format!("Failed to resolve `{}`", staged_idl.display()))?;
    let mut scripts = Map::new();
    for (lang, out) in targets {
        let abs_out = out
            .canonicalize()
            .with_context(|| format!("Failed to resolve `{}`", out.display()))?;
        scripts.insert(
            lang.id().to_string(),
            json!({
                "from": lang.renderer_package(),
                "args": [abs_out.to_string_lossy()],
            }),
        );
    }
    let config = json!({
        "idl": abs_idl.to_string_lossy(),
        "scripts": scripts,
    });
    let config_path = stage_dir.join("codama.json");
    fs::write(&config_path, serde_json::to_string_pretty(&config)?)
        .with_context(|| format!("Failed to write `{}`", config_path.display()))?;

    let labels: Vec<&str> = targets.iter().map(|(l, _)| l.id()).collect();
    eprintln!(
        "Generating Codama clients [{}] for `{}` ...",
        labels.join(", "),
        idl_path.display(),
    );
    run_codama(&config_path)?;
    Ok(())
}

fn run_codama(config_path: &Path) -> Result<()> {
    let (program, leading_args) = match std::env::var("ANCHOR_CODAMA_CMD") {
        Ok(s) if !s.trim().is_empty() => {
            // Allow the override to bake in flags (e.g. `pnpm codama`).
            let mut parts = s.split_whitespace().map(str::to_owned);
            let program = parts.next().expect("non-empty after trim");
            (program, parts.collect::<Vec<_>>())
        }
        _ => (
            "npx".to_string(),
            vec!["--yes".to_string(), "codama".to_string()],
        ),
    };

    let mut cmd = Command::new(&program);
    for arg in &leading_args {
        cmd.arg(arg);
    }
    cmd.arg("run")
        .arg("--config")
        .arg(config_path.as_os_str())
        .arg("--all")
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit());

    let status = cmd.status().map_err(|e| {
        anyhow!(
            "Failed to spawn `{}`: {e}. Install Node.js + npm, or set ANCHOR_CODAMA_CMD to point \
             at your Codama binary.",
            display_command(&program, &leading_args),
        )
    })?;
    if !status.success() {
        bail!(
            "`{} run --config {} --all` failed with {status}",
            display_command(&program, &leading_args),
            config_path.display(),
        );
    }
    Ok(())
}

fn display_command(program: &str, args: &[String]) -> String {
    if args.is_empty() {
        program.to_string()
    } else {
        format!("{program} {}", args.join(" "))
    }
}

// ---------------------------------------------------------------------------
// Top-level node builders.
// ---------------------------------------------------------------------------

fn root_node_from_anchor(idl: &JsonValue) -> Result<JsonValue> {
    let program = program_node_from_anchor(idl)?;
    Ok(json!({
        "kind": "rootNode",
        "standard": "codama",
        "version": CODAMA_VERSION,
        "program": program,
        "additionalPrograms": [],
    }))
}

fn program_node_from_anchor(idl: &JsonValue) -> Result<JsonValue> {
    let metadata = idl
        .get("metadata")
        .and_then(JsonValue::as_object)
        .ok_or_else(|| anyhow!("IDL is missing `metadata`"))?;
    let name = metadata
        .get("name")
        .and_then(JsonValue::as_str)
        .ok_or_else(|| anyhow!("IDL is missing `metadata.name`"))?;
    let version = metadata
        .get("version")
        .and_then(JsonValue::as_str)
        .unwrap_or("0.0.0");
    let public_key = idl
        .get("address")
        .and_then(JsonValue::as_str)
        .ok_or_else(|| anyhow!("IDL is missing `address`"))?;

    let types = idl
        .get("types")
        .and_then(JsonValue::as_array)
        .cloned()
        .unwrap_or_default();
    let accounts = idl
        .get("accounts")
        .and_then(JsonValue::as_array)
        .cloned()
        .unwrap_or_default();
    let events = idl
        .get("events")
        .and_then(JsonValue::as_array)
        .cloned()
        .unwrap_or_default();
    let instructions = idl
        .get("instructions")
        .and_then(JsonValue::as_array)
        .cloned()
        .unwrap_or_default();
    let errors = idl
        .get("errors")
        .and_then(JsonValue::as_array)
        .cloned()
        .unwrap_or_default();

    let (non_generic_types, generics) = extract_generics(&types);

    // Anchor stuffs account- and event-backing structs into `types`. Codama
    // promotes them to first-class `accountNode`/`eventNode`s instead, so we
    // must filter the duplicates out before exporting `definedTypes`.
    let account_names: Vec<&str> = accounts.iter().filter_map(named).collect();
    let event_names: Vec<&str> = events.iter().filter_map(named).collect();
    let mut defined_types = Vec::new();
    for ty in &non_generic_types {
        let n = match named(ty) {
            Some(n) => n,
            None => continue,
        };
        if account_names.contains(&n) || event_names.contains(&n) {
            continue;
        }
        defined_types.push(defined_type_node_from_anchor(ty, &generics)?);
    }

    let account_nodes: Vec<JsonValue> = accounts
        .iter()
        .map(|a| account_node_from_anchor(a, &types, &generics))
        .collect::<Result<_>>()?;
    let event_nodes: Vec<JsonValue> = events
        .iter()
        .map(|e| event_node_from_anchor(e, &types, &generics))
        .collect::<Result<_>>()?;
    let instruction_nodes: Vec<JsonValue> = instructions
        .iter()
        .map(|i| instruction_node_from_anchor(i, &generics))
        .collect::<Result<_>>()?;
    let error_nodes: Vec<JsonValue> = errors.iter().map(error_node_from_anchor).collect();

    Ok(json!({
        "kind": "programNode",
        "name": camel_case(name),
        "publicKey": public_key,
        "version": version,
        "origin": "anchor",
        "docs": [],
        "accounts": account_nodes,
        "instructions": instruction_nodes,
        "definedTypes": defined_types,
        "pdas": [],
        "events": event_nodes,
        "errors": error_nodes,
    }))
}

fn named(v: &JsonValue) -> Option<&str> {
    v.get("name").and_then(JsonValue::as_str)
}

// ---------------------------------------------------------------------------
// Generics handling — Anchor's `types` may declare generic parameters that we
// must substitute when a `defined { name, generics }` reference is reached.
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Default)]
struct Generics {
    /// Generic type-defs keyed by name (only those that declare `generics`).
    types: HashMap<String, JsonValue>,
    /// Type-arg substitutions in the current scope, *pre-resolved* in the
    /// outer scope at substitution time. Mapping: name → Codama type node.
    type_args: HashMap<String, JsonValue>,
    /// Const-arg substitutions in the current scope, pre-resolved to a value
    /// string at substitution time. Mapping: name → numeric literal.
    const_args: HashMap<String, String>,
}

fn extract_generics(types: &[JsonValue]) -> (Vec<JsonValue>, Generics) {
    let mut non_generic = Vec::new();
    let mut generic_types = HashMap::new();
    for t in types {
        let has_generics = t
            .get("generics")
            .and_then(JsonValue::as_array)
            .is_some_and(|a| !a.is_empty());
        if has_generics {
            if let Some(n) = named(t) {
                generic_types.insert(n.to_string(), t.clone());
            }
        } else {
            non_generic.push(t.clone());
        }
    }
    (
        non_generic,
        Generics {
            types: generic_types,
            type_args: HashMap::new(),
            const_args: HashMap::new(),
        },
    )
}

fn unwrap_generic_type(defined: &JsonValue, generics: &Generics) -> Result<JsonValue> {
    let inner = defined
        .get("defined")
        .and_then(JsonValue::as_object)
        .ok_or_else(|| anyhow!("Expected `defined` object"))?;
    let name = inner
        .get("name")
        .and_then(JsonValue::as_str)
        .ok_or_else(|| anyhow!("`defined` missing `name`"))?;
    let generic_type = generics
        .types
        .get(name)
        .ok_or_else(|| anyhow!("Generic type `{name}` not found"))?
        .clone();
    let generic_definitions = generic_type
        .get("generics")
        .and_then(JsonValue::as_array)
        .cloned()
        .unwrap_or_default();
    let generic_args = inner
        .get("generics")
        .and_then(JsonValue::as_array)
        .cloned()
        .unwrap_or_default();

    // Build a *fresh* scope, pre-resolving every arg in the OUTER scope. This
    // breaks the self-shadowing recursion that would otherwise occur whenever
    // a callee re-uses one of its caller's parameter names — e.g. passing
    // `T` from `Outer<T>` into `Inner<T, U>`. If we kept the args as raw
    // Anchor IDL nodes we'd resolve them lazily *in* the inner scope, where
    // `T → {generic: T}` loops forever.
    let mut type_args: HashMap<String, JsonValue> = HashMap::new();
    let mut const_args: HashMap<String, String> = HashMap::new();
    for (i, def) in generic_definitions.iter().enumerate() {
        let def_name = def
            .get("name")
            .and_then(JsonValue::as_str)
            .ok_or_else(|| anyhow!("Generic definition missing `name`"))?
            .to_string();
        let def_kind = def
            .get("kind")
            .and_then(JsonValue::as_str)
            .unwrap_or("type");
        let arg = generic_args
            .get(i)
            .ok_or_else(|| anyhow!("Missing generic argument for `{def_name}`"))?;
        if def_kind == "const" {
            // Common case: `{kind:"const", value:"<literal>"}`.
            if let Some(v) = arg.get("value").and_then(JsonValue::as_str) {
                const_args.insert(def_name, v.to_string());
            } else {
                // Anchor sometimes forwards an outer const generic by emitting
                // `{kind:"type", type:{generic:"N"}}` even when the callee
                // declares the parameter as `const` — the IDL doesn't model
                // const-generic forwarding cleanly. Resolve via the outer
                // scope's `const_args` instead.
                let outer_name = arg
                    .get("type")
                    .and_then(|t| t.get("generic"))
                    .and_then(JsonValue::as_str)
                    .ok_or_else(|| anyhow!("Const generic arg `{def_name}` missing `value`"))?;
                let v = generics.const_args.get(outer_name).ok_or_else(|| {
                    anyhow!(
                        "Const generic arg `{def_name}` forwards unknown outer const \
                         `{outer_name}`"
                    )
                })?;
                const_args.insert(def_name, v.clone());
            }
        } else {
            let arg_type = arg
                .get("type")
                .ok_or_else(|| anyhow!("Type generic arg `{def_name}` missing `type`"))?;
            let resolved = type_node_from_anchor(arg_type, generics)?;
            type_args.insert(def_name, resolved);
        }
    }

    let scoped = Generics {
        types: generics.types.clone(),
        type_args,
        const_args,
    };
    let inner_ty = generic_type
        .get("type")
        .ok_or_else(|| anyhow!("Generic typedef `{name}` missing `type`"))?;
    type_node_from_anchor(inner_ty, &scoped)
}

// ---------------------------------------------------------------------------
// Type nodes — recursively translate Anchor IDL type expressions.
// ---------------------------------------------------------------------------

const NUMBER_LEAVES: &[&str] = &[
    "u8", "u16", "u32", "u64", "u128", "i8", "i16", "i32", "i64", "i128", "f32", "f64", "shortU16",
];

fn type_node_from_anchor(ty: &JsonValue, generics: &Generics) -> Result<JsonValue> {
    // Leaf primitives are encoded as JSON strings.
    if let Some(leaf) = ty.as_str() {
        return Ok(match leaf {
            "bool" => json!({ "kind": "booleanTypeNode", "size": number_node("u8") }),
            "pubkey" => json!({ "kind": "publicKeyTypeNode" }),
            "string" => size_prefix_node(string_node("utf8"), number_node("u32")),
            "bytes" => size_prefix_node(bytes_node(), number_node("u32")),
            n if NUMBER_LEAVES.contains(&n) => number_node(n),
            other => bail!("Unrecognized Anchor IDL leaf type: `{other}`"),
        });
    }
    let obj = ty
        .as_object()
        .ok_or_else(|| anyhow!("Unrecognized Anchor IDL type: {ty}"))?;

    if obj.contains_key("array") {
        let arr = obj["array"]
            .as_array()
            .ok_or_else(|| anyhow!("`array` must be a 2-tuple"))?;
        if arr.len() != 2 {
            bail!("`array` must be a 2-tuple, got {} elements", arr.len());
        }
        let item = type_node_from_anchor(&arr[0], generics)?;
        let size = match &arr[1] {
            JsonValue::Number(n) => n
                .as_u64()
                .ok_or_else(|| anyhow!("Array length must be a non-negative integer"))?,
            JsonValue::Object(o) if o.contains_key("generic") => {
                let gname = o["generic"]
                    .as_str()
                    .ok_or_else(|| anyhow!("`generic` must be a string"))?;
                let v = generics
                    .const_args
                    .get(gname)
                    .ok_or_else(|| anyhow!("Const generic `{gname}` not found"))?;
                v.parse::<u64>()
                    .with_context(|| format!("Const generic `{gname}` value `{v}` is not u64"))?
            }
            other => bail!("Unrecognized array length: {other}"),
        };
        return Ok(json!({
            "kind": "arrayTypeNode",
            "item": item,
            "count": { "kind": "fixedCountNode", "value": size },
        }));
    }

    if let Some(inner) = obj.get("vec") {
        let item = type_node_from_anchor(inner, generics)?;
        return Ok(json!({
            "kind": "arrayTypeNode",
            "item": item,
            "count": { "kind": "prefixedCountNode", "prefix": number_node("u32") },
        }));
    }

    if let Some(defined) = obj.get("defined") {
        // The post-0.30 spec uses an object form `{name, generics?}`. We don't
        // accept the legacy bare-string form here because `anchor idl convert`
        // already normalizes it.
        let def_obj = defined
            .as_object()
            .ok_or_else(|| anyhow!("`defined` must be an object"))?;
        let has_generics = def_obj
            .get("generics")
            .and_then(JsonValue::as_array)
            .is_some_and(|a| !a.is_empty());
        if has_generics {
            return unwrap_generic_type(ty, generics);
        }
        let name = def_obj
            .get("name")
            .and_then(JsonValue::as_str)
            .ok_or_else(|| anyhow!("`defined` missing `name`"))?;
        return Ok(json!({
            "kind": "definedTypeLinkNode",
            "name": camel_case(name),
        }));
    }

    if let Some(generic) = obj.get("generic").and_then(JsonValue::as_str) {
        // Already resolved at substitution time — see `unwrap_generic_type`.
        let resolved = generics
            .type_args
            .get(generic)
            .ok_or_else(|| anyhow!("Type generic `{generic}` not found"))?;
        return Ok(resolved.clone());
    }

    if let Some(inner) = obj.get("option") {
        let item = type_node_from_anchor(inner, generics)?;
        return Ok(json!({
            "kind": "optionTypeNode",
            "fixed": false,
            "item": item,
            "prefix": number_node("u8"),
        }));
    }

    if let Some(inner) = obj.get("coption") {
        let item = type_node_from_anchor(inner, generics)?;
        return Ok(json!({
            "kind": "optionTypeNode",
            "fixed": true,
            "item": item,
            "prefix": number_node("u32"),
        }));
    }

    let kind = obj.get("kind").and_then(JsonValue::as_str);
    if matches!(kind, Some("enum")) {
        let variants = obj
            .get("variants")
            .and_then(JsonValue::as_array)
            .cloned()
            .unwrap_or_default();
        let variant_nodes: Vec<JsonValue> = variants
            .iter()
            .map(|v| enum_variant_from_anchor(v, generics))
            .collect::<Result<_>>()?;
        return Ok(json!({
            "kind": "enumTypeNode",
            "variants": variant_nodes,
            "size": number_node("u8"),
        }));
    }

    // Anchor's type aliases serialize as `{kind: "type", alias: T}` (per the
    // current `anchor-lang-idl-spec`). The TypeScript Codama converter checks
    // for `{kind: "alias", value: T}` instead, so we accept both forms.
    if matches!(kind, Some("type")) {
        if let Some(alias) = obj.get("alias") {
            return type_node_from_anchor(alias, generics);
        }
    }
    if matches!(kind, Some("alias")) {
        if let Some(value) = obj.get("value") {
            return type_node_from_anchor(value, generics);
        }
    }

    if matches!(kind, Some("struct")) {
        let fields = obj
            .get("fields")
            .and_then(JsonValue::as_array)
            .cloned()
            .unwrap_or_default();
        return struct_or_tuple_from_fields(&fields, generics);
    }

    bail!("Unrecognized Anchor IDL type: {ty}")
}

fn struct_or_tuple_from_fields(fields: &[JsonValue], generics: &Generics) -> Result<JsonValue> {
    if fields.is_empty() || is_struct_field_array(fields) {
        let nodes: Vec<JsonValue> = fields
            .iter()
            .map(|f| struct_field_from_anchor(f, generics))
            .collect::<Result<_>>()?;
        return Ok(json!({ "kind": "structTypeNode", "fields": nodes }));
    }
    if is_tuple_field_array(fields) {
        let items: Vec<JsonValue> = fields
            .iter()
            .map(|f| type_node_from_anchor(f, generics))
            .collect::<Result<_>>()?;
        return Ok(json!({ "kind": "tupleTypeNode", "items": items }));
    }
    bail!("Mixed named/positional fields in struct: {:?}", fields)
}

fn is_struct_field(field: &JsonValue) -> bool {
    field
        .as_object()
        .is_some_and(|o| o.contains_key("name") && o.contains_key("type"))
}

fn is_struct_field_array(fields: &[JsonValue]) -> bool {
    fields.iter().all(is_struct_field)
}

fn is_tuple_field_array(fields: &[JsonValue]) -> bool {
    fields.iter().all(|f| !is_struct_field(f))
}

fn struct_field_from_anchor(field: &JsonValue, generics: &Generics) -> Result<JsonValue> {
    let obj = field
        .as_object()
        .ok_or_else(|| anyhow!("Struct field must be an object: {field}"))?;
    let name = obj
        .get("name")
        .and_then(JsonValue::as_str)
        .ok_or_else(|| anyhow!("Struct field missing `name`"))?;
    let ty = obj
        .get("type")
        .ok_or_else(|| anyhow!("Struct field `{name}` missing `type`"))?;
    Ok(json!({
        "kind": "structFieldTypeNode",
        "name": camel_case(name),
        "docs": docs(obj.get("docs")),
        "type": type_node_from_anchor(ty, generics)?,
    }))
}

fn enum_variant_from_anchor(variant: &JsonValue, generics: &Generics) -> Result<JsonValue> {
    let obj = variant
        .as_object()
        .ok_or_else(|| anyhow!("Enum variant must be an object: {variant}"))?;
    let name = obj.get("name").and_then(JsonValue::as_str).unwrap_or("");
    let fields = obj.get("fields").and_then(JsonValue::as_array);
    match fields {
        None => Ok(json!({
            "kind": "enumEmptyVariantTypeNode",
            "name": camel_case(name),
        })),
        Some(fs) if fs.is_empty() => Ok(json!({
            "kind": "enumEmptyVariantTypeNode",
            "name": camel_case(name),
        })),
        Some(fs) if is_struct_field_array(fs) => {
            let nodes: Vec<JsonValue> = fs
                .iter()
                .map(|f| struct_field_from_anchor(f, generics))
                .collect::<Result<_>>()?;
            Ok(json!({
                "kind": "enumStructVariantTypeNode",
                "name": camel_case(name),
                "struct": { "kind": "structTypeNode", "fields": nodes },
            }))
        }
        Some(fs) => {
            let items: Vec<JsonValue> = fs
                .iter()
                .map(|f| type_node_from_anchor(f, generics))
                .collect::<Result<_>>()?;
            Ok(json!({
                "kind": "enumTupleVariantTypeNode",
                "name": camel_case(name),
                "tuple": { "kind": "tupleTypeNode", "items": items },
            }))
        }
    }
}

// ---------------------------------------------------------------------------
// Account/event/error/defined-type nodes.
// ---------------------------------------------------------------------------

fn defined_type_node_from_anchor(ty: &JsonValue, generics: &Generics) -> Result<JsonValue> {
    let name = named(ty).unwrap_or("");
    let inner = ty
        .get("type")
        .cloned()
        .unwrap_or_else(|| json!({ "kind": "struct", "fields": [] }));
    let node = type_node_from_anchor(&inner, generics)?;
    Ok(json!({
        "kind": "definedTypeNode",
        "name": camel_case(name),
        "docs": docs(ty.get("docs")),
        "type": node,
    }))
}

fn account_node_from_anchor(
    acc: &JsonValue,
    types: &[JsonValue],
    generics: &Generics,
) -> Result<JsonValue> {
    let name = named(acc).ok_or_else(|| anyhow!("Account missing `name`"))?;
    let ty_def = types
        .iter()
        .find(|t| named(t) == Some(name))
        .ok_or_else(|| anyhow!("Account type `{name}` not found in `types`"))?;
    let inner = ty_def
        .get("type")
        .ok_or_else(|| anyhow!("Account type `{name}` missing `type`"))?;
    let data = type_node_from_anchor(inner, generics)?;
    let data_obj = data
        .as_object()
        .filter(|o| o.get("kind").and_then(JsonValue::as_str) == Some("structTypeNode"))
        .ok_or_else(|| anyhow!("Account `{name}` data must be a struct"))?;
    let mut fields = data_obj
        .get("fields")
        .and_then(JsonValue::as_array)
        .cloned()
        .unwrap_or_default();
    let disc = discriminator_bytes(acc)?;
    let discriminator_field = json!({
        "kind": "structFieldTypeNode",
        "name": "discriminator",
        "docs": [],
        "type": {
            "kind": "fixedSizeTypeNode",
            "size": disc.len(),
            "type": bytes_node(),
        },
        "defaultValue": discriminator_value(&disc),
        "defaultValueStrategy": "omitted",
    });
    fields.insert(0, discriminator_field);
    Ok(json!({
        "kind": "accountNode",
        "name": camel_case(name),
        "docs": [],
        "data": { "kind": "structTypeNode", "fields": fields },
        "discriminators": [{ "kind": "fieldDiscriminatorNode", "name": "discriminator", "offset": 0 }],
    }))
}

fn event_node_from_anchor(
    ev: &JsonValue,
    types: &[JsonValue],
    generics: &Generics,
) -> Result<JsonValue> {
    let name = named(ev).ok_or_else(|| anyhow!("Event missing `name`"))?;
    let ty_def = types
        .iter()
        .find(|t| named(t) == Some(name))
        .ok_or_else(|| anyhow!("Event type `{name}` not found in `types`"))?;
    let inner = ty_def
        .get("type")
        .ok_or_else(|| anyhow!("Event type `{name}` missing `type`"))?;
    let data = type_node_from_anchor(inner, generics)?;
    let disc = discriminator_bytes(ev)?;
    let constant = json!({
        "kind": "constantValueNode",
        "type": { "kind": "fixedSizeTypeNode", "size": disc.len(), "type": bytes_node() },
        "value": discriminator_value(&disc),
    });
    Ok(json!({
        "kind": "eventNode",
        "name": camel_case(name),
        "docs": [],
        "data": {
            "kind": "hiddenPrefixTypeNode",
            "type": data,
            "prefix": [constant.clone()],
        },
        "discriminators": [{
            "kind": "constantDiscriminatorNode",
            "offset": 0,
            "constant": constant,
        }],
    }))
}

fn error_node_from_anchor(err: &JsonValue) -> JsonValue {
    let name = named(err).unwrap_or("");
    let msg = err
        .get("msg")
        .and_then(JsonValue::as_str)
        .unwrap_or("")
        .to_string();
    let code = err.get("code").and_then(JsonValue::as_i64).unwrap_or(-1);
    json!({
        "kind": "errorNode",
        "name": camel_case(name),
        "code": code,
        "message": msg,
        "docs": [format!("{name}: {msg}")],
    })
}

// ---------------------------------------------------------------------------
// Instruction node + accounts/arguments/PDA seeds.
// ---------------------------------------------------------------------------

fn instruction_node_from_anchor(ix: &JsonValue, generics: &Generics) -> Result<JsonValue> {
    let name = named(ix).ok_or_else(|| anyhow!("Instruction missing `name`"))?;
    let args = ix
        .get("args")
        .and_then(JsonValue::as_array)
        .cloned()
        .unwrap_or_default();
    let mut data_arguments: Vec<JsonValue> = args
        .iter()
        .map(|a| instruction_argument_from_anchor(a, generics))
        .collect::<Result<_>>()?;
    let disc = discriminator_bytes(ix)?;
    let discriminator_arg = json!({
        "kind": "instructionArgumentNode",
        "name": "discriminator",
        "docs": [],
        "type": {
            "kind": "fixedSizeTypeNode",
            "size": disc.len(),
            "type": bytes_node(),
        },
        "defaultValue": discriminator_value(&disc),
        "defaultValueStrategy": "omitted",
    });
    data_arguments.insert(0, discriminator_arg);

    let raw_accounts = ix
        .get("accounts")
        .and_then(JsonValue::as_array)
        .cloned()
        .unwrap_or_default();
    let accounts =
        instruction_account_nodes_from_anchor(&raw_accounts, &data_arguments, None, false)?;

    Ok(json!({
        "kind": "instructionNode",
        "name": camel_case(name),
        "docs": ix.get("docs").cloned().unwrap_or_else(|| json!([])),
        "optionalAccountStrategy": "programId",
        "accounts": accounts,
        "arguments": data_arguments,
        "discriminators": [{ "kind": "fieldDiscriminatorNode", "name": "discriminator", "offset": 0 }],
    }))
}

fn instruction_argument_from_anchor(arg: &JsonValue, generics: &Generics) -> Result<JsonValue> {
    let obj = arg
        .as_object()
        .ok_or_else(|| anyhow!("Instruction argument must be an object: {arg}"))?;
    let name = obj
        .get("name")
        .and_then(JsonValue::as_str)
        .ok_or_else(|| anyhow!("Instruction argument missing `name`"))?;
    let ty = obj
        .get("type")
        .ok_or_else(|| anyhow!("Instruction argument `{name}` missing `type`"))?;
    Ok(json!({
        "kind": "instructionArgumentNode",
        "name": camel_case(name),
        "docs": docs(obj.get("docs")),
        "type": type_node_from_anchor(ty, generics)?,
    }))
}

/// Collect every leaf account name in the (possibly nested) account tree,
/// camelCased, to detect collisions that force prefixing.
fn collect_camel_names(items: &[JsonValue], out: &mut Vec<String>) {
    for item in items {
        let Some(obj) = item.as_object() else {
            continue;
        };
        if let Some(nested) = obj.get("accounts").and_then(JsonValue::as_array) {
            collect_camel_names(nested, out);
        } else if let Some(n) = obj.get("name").and_then(JsonValue::as_str) {
            out.push(camel_case(n));
        }
    }
}

fn has_duplicate_account_names(items: &[JsonValue]) -> bool {
    let mut names = Vec::new();
    collect_camel_names(items, &mut names);
    let mut seen = std::collections::HashSet::new();
    !names.into_iter().all(|n| seen.insert(n))
}

fn instruction_account_nodes_from_anchor(
    items: &[JsonValue],
    instruction_arguments: &[JsonValue],
    prefix: Option<&str>,
    // True when an ancestor required prefixing — propagates into nested groups.
    forced: bool,
) -> Result<Vec<JsonValue>> {
    let should_prefix = forced || prefix.is_some() || has_duplicate_account_names(items);
    let mut out = Vec::new();
    for item in items {
        let obj = match item.as_object() {
            Some(o) => o,
            None => continue,
        };
        if let Some(nested) = obj.get("accounts").and_then(JsonValue::as_array) {
            let group_name = obj.get("name").and_then(JsonValue::as_str).unwrap_or("");
            let new_prefix = if should_prefix {
                Some(match prefix {
                    Some(p) => format!("{p}_{group_name}"),
                    None => group_name.to_string(),
                })
            } else {
                None
            };
            // Once we've decided to prefix at this level, the recursion must
            // also prefix even if its own siblings aren't ambiguous on their
            // own — otherwise the `prefix` we pass would silently be dropped.
            let nested_nodes = instruction_account_nodes_from_anchor(
                nested,
                instruction_arguments,
                new_prefix.as_deref(),
                should_prefix,
            )?;
            out.extend(nested_nodes);
        } else {
            out.push(instruction_account_node_from_anchor(
                item,
                instruction_arguments,
                if should_prefix { prefix } else { None },
            )?);
        }
    }
    Ok(out)
}

fn instruction_account_node_from_anchor(
    item: &JsonValue,
    instruction_arguments: &[JsonValue],
    prefix: Option<&str>,
) -> Result<JsonValue> {
    let obj = item
        .as_object()
        .ok_or_else(|| anyhow!("Account item must be an object: {item}"))?;
    let raw_name = obj.get("name").and_then(JsonValue::as_str).unwrap_or("");
    let name = match prefix {
        Some(p) => format!("{p}_{raw_name}"),
        None => raw_name.to_string(),
    };
    let camel_name = camel_case(&name);
    let is_writable = obj
        .get("writable")
        .and_then(JsonValue::as_bool)
        .unwrap_or(false);
    let is_signer = obj
        .get("signer")
        .and_then(JsonValue::as_bool)
        .unwrap_or(false);
    let is_optional = obj
        .get("optional")
        .and_then(JsonValue::as_bool)
        .unwrap_or(false);
    let docs_v = docs(obj.get("docs"));

    let mut node = Map::new();
    node.insert("kind".into(), json!("instructionAccountNode"));
    node.insert("name".into(), json!(camel_name.clone()));
    node.insert("isWritable".into(), json!(is_writable));
    node.insert("isSigner".into(), json!(is_signer));
    node.insert("isOptional".into(), json!(is_optional));
    node.insert("docs".into(), docs_v);

    if let Some(addr) = obj.get("address").and_then(JsonValue::as_str) {
        node.insert(
            "defaultValue".into(),
            json!({
                "kind": "publicKeyValueNode",
                "publicKey": addr,
                "identifier": camel_name,
            }),
        );
    } else if let Some(pda) = obj.get("pda").and_then(JsonValue::as_object) {
        let seeds = pda
            .get("seeds")
            .and_then(JsonValue::as_array)
            .cloned()
            .unwrap_or_default();
        // Match the Codama TS converter: skip PDA defaults entirely whenever
        // any seed references a nested path (`some.nested.field`). Codama
        // doesn't model nested-path lookups today and silently drops the PDA.
        let nested_path = seeds.iter().any(|s| {
            s.get("path")
                .and_then(JsonValue::as_str)
                .is_some_and(|p| p.contains('.'))
        });
        if !nested_path {
            let mut definitions = Vec::new();
            let mut values = Vec::new();
            for seed in &seeds {
                let (def, val) = pda_seed_node_from_anchor(seed, instruction_arguments, prefix)?;
                definitions.push(def);
                if let Some(v) = val {
                    values.push(v);
                }
            }
            // Resolve `pda.program` if present. A constant base58 program
            // address surfaces as `programId` on the pda link; an account/arg
            // reference surfaces as `programId` on the pdaValueNode.
            let mut program_id: Option<String> = None;
            let mut program_id_value: Option<JsonValue> = None;
            if let Some(prog) = pda.get("program") {
                let (def, val) = pda_seed_node_from_anchor(prog, instruction_arguments, prefix)?;
                if let Some(def_obj) = def.as_object() {
                    if def_obj.get("kind").and_then(JsonValue::as_str)
                        == Some("constantPdaSeedNode")
                    {
                        if let Some(value) = def_obj.get("value").and_then(JsonValue::as_object) {
                            if value.get("kind").and_then(JsonValue::as_str)
                                == Some("bytesValueNode")
                                && value.get("encoding").and_then(JsonValue::as_str)
                                    == Some("base58")
                            {
                                program_id = value
                                    .get("data")
                                    .and_then(JsonValue::as_str)
                                    .map(str::to_string);
                            }
                        }
                    }
                }
                if program_id.is_none() {
                    if let Some(v) = val {
                        if let Some(inner_value) = v.get("value").cloned() {
                            if let Some(k) = inner_value.get("kind").and_then(JsonValue::as_str) {
                                if k == "accountValueNode" || k == "argumentValueNode" {
                                    program_id_value = Some(inner_value);
                                }
                            }
                        }
                    }
                }
            }

            let mut pda_link = Map::new();
            pda_link.insert("kind".into(), json!("pdaNode"));
            pda_link.insert("name".into(), json!(camel_name.clone()));
            pda_link.insert("docs".into(), json!([]));
            if let Some(pid) = program_id {
                pda_link.insert("programId".into(), json!(pid));
            }
            pda_link.insert("seeds".into(), json!(definitions));

            let mut pda_value = Map::new();
            pda_value.insert("kind".into(), json!("pdaValueNode"));
            pda_value.insert("pda".into(), JsonValue::Object(pda_link));
            pda_value.insert("seeds".into(), json!(values));
            if let Some(pidv) = program_id_value {
                pda_value.insert("programId".into(), pidv);
            }
            node.insert("defaultValue".into(), JsonValue::Object(pda_value));
        }
    }

    Ok(JsonValue::Object(node))
}

fn pda_seed_node_from_anchor(
    seed: &JsonValue,
    instruction_arguments: &[JsonValue],
    prefix: Option<&str>,
) -> Result<(JsonValue, Option<JsonValue>)> {
    let obj = seed
        .as_object()
        .ok_or_else(|| anyhow!("PDA seed must be an object: {seed}"))?;
    let kind = obj
        .get("kind")
        .and_then(JsonValue::as_str)
        .ok_or_else(|| anyhow!("PDA seed missing `kind`"))?;
    match kind {
        "const" => {
            let bytes = obj
                .get("value")
                .and_then(JsonValue::as_array)
                .ok_or_else(|| anyhow!("Const seed missing `value` array"))?;
            let raw: Vec<u8> = bytes
                .iter()
                .map(|b| {
                    b.as_u64()
                        .and_then(|n| u8::try_from(n).ok())
                        .ok_or_else(|| anyhow!("Const seed byte must be 0..=255"))
                })
                .collect::<Result<_>>()?;
            let data = bs58::encode(raw).into_string();
            Ok((
                json!({
                    "kind": "constantPdaSeedNode",
                    "type": bytes_node(),
                    "value": {
                        "kind": "bytesValueNode",
                        "encoding": "base58",
                        "data": data,
                    },
                }),
                None,
            ))
        }
        "account" => {
            let path = obj
                .get("path")
                .and_then(JsonValue::as_str)
                .ok_or_else(|| anyhow!("Account seed missing `path`"))?;
            let head = path.split('.').next().unwrap_or("");
            let prefixed = match prefix {
                Some(p) => format!("{p}_{head}"),
                None => head.to_string(),
            };
            let camel_name = camel_case(&prefixed);
            Ok((
                json!({
                    "kind": "variablePdaSeedNode",
                    "name": camel_name.clone(),
                    "docs": [],
                    "type": { "kind": "publicKeyTypeNode" },
                }),
                Some(json!({
                    "kind": "pdaSeedValueNode",
                    "name": camel_name.clone(),
                    "value": { "kind": "accountValueNode", "name": camel_name },
                })),
            ))
        }
        "arg" => {
            let path = obj
                .get("path")
                .and_then(JsonValue::as_str)
                .ok_or_else(|| anyhow!("Arg seed missing `path`"))?;
            let head = path.split('.').next().unwrap_or("");
            let arg_name = camel_case(head);
            let arg_node = instruction_arguments
                .iter()
                .find(|a| a.get("name").and_then(JsonValue::as_str) == Some(arg_name.as_str()))
                .ok_or_else(|| anyhow!("Arg seed `{path}` not found in instruction arguments"))?;
            // Anchor PDA seeds use the raw UTF-8 bytes of a string argument
            // (no Borsh size prefix); detect that pattern and unwrap so the
            // generated codec doesn't write the length on-chain.
            let arg_type = arg_node.get("type").cloned().unwrap_or_else(|| json!("u8"));
            let unwrapped = if is_borsh_string(&arg_type) {
                json!({ "kind": "stringTypeNode", "encoding": "utf8" })
            } else {
                arg_type
            };
            Ok((
                json!({
                    "kind": "variablePdaSeedNode",
                    "name": arg_name.clone(),
                    "docs": [],
                    "type": unwrapped,
                }),
                Some(json!({
                    "kind": "pdaSeedValueNode",
                    "name": arg_name.clone(),
                    "value": { "kind": "argumentValueNode", "name": arg_name },
                })),
            ))
        }
        other => bail!("Unimplemented PDA seed kind: `{other}`"),
    }
}

fn is_borsh_string(ty: &JsonValue) -> bool {
    let Some(obj) = ty.as_object() else {
        return false;
    };
    if obj.get("kind").and_then(JsonValue::as_str) != Some("sizePrefixTypeNode") {
        return false;
    }
    let inner = obj.get("type").and_then(JsonValue::as_object);
    let prefix = obj.get("prefix").and_then(JsonValue::as_object);
    let inner_ok = inner.is_some_and(|o| {
        o.get("kind").and_then(JsonValue::as_str) == Some("stringTypeNode")
            && o.get("encoding").and_then(JsonValue::as_str) == Some("utf8")
    });
    let prefix_ok = prefix.is_some_and(|o| {
        o.get("kind").and_then(JsonValue::as_str) == Some("numberTypeNode")
            && o.get("format").and_then(JsonValue::as_str) == Some("u32")
    });
    inner_ok && prefix_ok
}

// ---------------------------------------------------------------------------
// Small helpers.
// ---------------------------------------------------------------------------

fn discriminator_bytes(node: &JsonValue) -> Result<Vec<u8>> {
    let arr = node
        .get("discriminator")
        .and_then(JsonValue::as_array)
        .ok_or_else(|| anyhow!("Missing `discriminator`"))?;
    arr.iter()
        .map(|b| {
            b.as_u64()
                .and_then(|n| u8::try_from(n).ok())
                .ok_or_else(|| anyhow!("Discriminator byte must be 0..=255"))
        })
        .collect()
}

fn discriminator_value(bytes: &[u8]) -> JsonValue {
    let hex: String = bytes.iter().map(|b| format!("{b:02x}")).collect();
    json!({
        "kind": "bytesValueNode",
        "encoding": "base16",
        "data": hex,
    })
}

fn number_node(format: &str) -> JsonValue {
    json!({ "kind": "numberTypeNode", "format": format, "endian": "le" })
}

fn bytes_node() -> JsonValue {
    json!({ "kind": "bytesTypeNode" })
}

fn string_node(encoding: &str) -> JsonValue {
    json!({ "kind": "stringTypeNode", "encoding": encoding })
}

fn size_prefix_node(ty: JsonValue, prefix: JsonValue) -> JsonValue {
    json!({ "kind": "sizePrefixTypeNode", "type": ty, "prefix": prefix })
}

fn docs(value: Option<&JsonValue>) -> JsonValue {
    match value {
        Some(JsonValue::Array(_)) => value.unwrap().clone(),
        Some(JsonValue::String(s)) => json!([s]),
        _ => json!([]),
    }
}

/// Codama's `camelCase` — split on every non-alphanumeric or before an
/// uppercase letter, capitalize each chunk, join, then lowercase the first
/// character. Mirrors `@codama/nodes/src/shared/stringCases.ts` so identifiers
/// match the JS converter byte-for-byte.
fn camel_case(s: &str) -> String {
    if s.is_empty() {
        return String::new();
    }
    // Insert a space before each ASCII uppercase letter (replicates JS
    // `replace(/([A-Z])/g, ' $1')`).
    let mut spaced = String::with_capacity(s.len() + 4);
    for c in s.chars() {
        if c.is_ascii_uppercase() {
            spaced.push(' ');
        }
        spaced.push(c);
    }
    // Split on runs of non-alphanumeric (matches `/[^a-zA-Z0-9]+/`).
    let words: Vec<String> = spaced
        .split(|c: char| !c.is_ascii_alphanumeric())
        .filter(|w| !w.is_empty())
        .map(capitalize_word)
        .collect();
    let pascal: String = words.join("");
    let mut chars = pascal.chars();
    match chars.next() {
        None => String::new(),
        Some(c) => c.to_ascii_lowercase().to_string() + chars.as_str(),
    }
}

fn capitalize_word(w: &str) -> String {
    let mut iter = w.chars();
    match iter.next() {
        None => String::new(),
        Some(c) => {
            let mut out = String::with_capacity(w.len());
            out.push(c.to_ascii_uppercase());
            for r in iter {
                out.push(r.to_ascii_lowercase());
            }
            out
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn convert_str(input: &str) -> JsonValue {
        let idl: JsonValue = serde_json::from_str(input).unwrap();
        root_node_from_anchor(&idl).unwrap()
    }

    #[test]
    fn camel_case_basic() {
        assert_eq!(camel_case("snake_case_name"), "snakeCaseName");
        assert_eq!(camel_case("kebab-case-name"), "kebabCaseName");
        assert_eq!(camel_case("PascalCaseName"), "pascalCaseName");
        assert_eq!(camel_case("alreadyCamel"), "alreadyCamel");
        assert_eq!(camel_case("u8"), "u8");
        assert_eq!(camel_case(""), "");
        // Matches Codama JS: every capital letter splits a word, then we
        // capitalize each chunk and join, so consecutive caps stay capitalized.
        assert_eq!(camel_case("MyABCThing"), "myABCThing");
    }

    #[test]
    fn empty_idl_yields_root() {
        let idl = json!({
            "address": "11111111111111111111111111111111",
            "metadata": { "name": "demo", "version": "0.1.0", "spec": "0.1.0" },
            "instructions": [],
        });
        let root = root_node_from_anchor(&idl).unwrap();
        assert_eq!(root["kind"], "rootNode");
        assert_eq!(root["standard"], "codama");
        let prog = &root["program"];
        assert_eq!(prog["kind"], "programNode");
        assert_eq!(prog["name"], "demo");
        assert_eq!(prog["origin"], "anchor");
        assert_eq!(prog["publicKey"], "11111111111111111111111111111111");
        assert_eq!(prog["instructions"].as_array().unwrap().len(), 0);
    }

    #[test]
    fn instruction_with_primitives_and_discriminator() {
        let idl = json!({
            "address": "11111111111111111111111111111111",
            "metadata": { "name": "demo", "version": "0.1.0", "spec": "0.1.0" },
            "instructions": [{
                "name": "do_thing",
                "discriminator": [1,2,3,4,5,6,7,8],
                "accounts": [
                    { "name": "payer", "writable": true, "signer": true }
                ],
                "args": [
                    { "name": "amount", "type": "u64" },
                    { "name": "label", "type": "string" },
                    { "name": "data", "type": "bytes" }
                ]
            }],
        });
        let root = root_node_from_anchor(&idl).unwrap();
        let ix = &root["program"]["instructions"][0];
        assert_eq!(ix["name"], "doThing");
        let args = ix["arguments"].as_array().unwrap();
        // discriminator + 3 user args
        assert_eq!(args.len(), 4);
        assert_eq!(args[0]["name"], "discriminator");
        assert_eq!(args[0]["defaultValue"]["data"], "0102030405060708");
        assert_eq!(args[1]["name"], "amount");
        assert_eq!(args[1]["type"]["format"], "u64");
        // string -> sizePrefix(string('utf8'), u32)
        assert_eq!(args[2]["type"]["kind"], "sizePrefixTypeNode");
        assert_eq!(args[2]["type"]["type"]["kind"], "stringTypeNode");
        assert_eq!(args[2]["type"]["prefix"]["format"], "u32");
        // bytes -> sizePrefix(bytes, u32)
        assert_eq!(args[3]["type"]["type"]["kind"], "bytesTypeNode");

        let accounts = ix["accounts"].as_array().unwrap();
        assert_eq!(accounts[0]["name"], "payer");
        assert_eq!(accounts[0]["isWritable"], true);
        assert_eq!(accounts[0]["isSigner"], true);
        assert_eq!(accounts[0]["isOptional"], false);
    }

    #[test]
    fn account_has_discriminator_field_prepended() {
        let idl = json!({
            "address": "11111111111111111111111111111111",
            "metadata": { "name": "demo", "version": "0.1.0", "spec": "0.1.0" },
            "instructions": [],
            "accounts": [
                { "name": "Counter", "discriminator": [9,8,7,6,5,4,3,2] }
            ],
            "types": [
                {
                    "name": "Counter",
                    "type": {
                        "kind": "struct",
                        "fields": [{ "name": "count", "type": "u64" }]
                    }
                }
            ]
        });
        let root = convert_str(&serde_json::to_string(&idl).unwrap());
        let acc = &root["program"]["accounts"][0];
        assert_eq!(acc["kind"], "accountNode");
        assert_eq!(acc["name"], "counter");
        let fields = acc["data"]["fields"].as_array().unwrap();
        assert_eq!(fields.len(), 2);
        assert_eq!(fields[0]["name"], "discriminator");
        assert_eq!(fields[0]["defaultValueStrategy"], "omitted");
        assert_eq!(fields[1]["name"], "count");
        // Backing struct is filtered out of `definedTypes`.
        assert_eq!(root["program"]["definedTypes"].as_array().unwrap().len(), 0);
    }

    #[test]
    fn event_uses_hidden_prefix_and_constant_discriminator() {
        let idl = json!({
            "address": "11111111111111111111111111111111",
            "metadata": { "name": "demo", "version": "0.1.0", "spec": "0.1.0" },
            "instructions": [],
            "events": [{ "name": "Tick", "discriminator": [1,1,1,1,1,1,1,1] }],
            "types": [{
                "name": "Tick",
                "type": { "kind": "struct", "fields": [{ "name": "n", "type": "u32" }] }
            }]
        });
        let root = convert_str(&serde_json::to_string(&idl).unwrap());
        let ev = &root["program"]["events"][0];
        assert_eq!(ev["data"]["kind"], "hiddenPrefixTypeNode");
        assert_eq!(ev["discriminators"][0]["kind"], "constantDiscriminatorNode");
    }

    #[test]
    fn errors_format_docs_as_name_colon_msg() {
        let idl = json!({
            "address": "11111111111111111111111111111111",
            "metadata": { "name": "demo", "version": "0.1.0", "spec": "0.1.0" },
            "instructions": [],
            "errors": [{ "code": 6000, "name": "Boom", "msg": "Kaboom!" }]
        });
        let root = convert_str(&serde_json::to_string(&idl).unwrap());
        let e = &root["program"]["errors"][0];
        assert_eq!(e["code"], 6000);
        assert_eq!(e["name"], "boom");
        assert_eq!(e["docs"][0], "Boom: Kaboom!");
    }

    #[test]
    fn enum_variants_struct_tuple_unit() {
        let idl = json!({
            "address": "11111111111111111111111111111111",
            "metadata": { "name": "demo", "version": "0.1.0", "spec": "0.1.0" },
            "instructions": [],
            "types": [{
                "name": "E",
                "type": {
                    "kind": "enum",
                    "variants": [
                        { "name": "Empty" },
                        { "name": "Tup", "fields": ["u8", "u16"] },
                        { "name": "Stru", "fields": [{ "name": "x", "type": "bool" }] }
                    ]
                }
            }]
        });
        let root = convert_str(&serde_json::to_string(&idl).unwrap());
        let variants = root["program"]["definedTypes"][0]["type"]["variants"]
            .as_array()
            .unwrap();
        assert_eq!(variants[0]["kind"], "enumEmptyVariantTypeNode");
        assert_eq!(variants[1]["kind"], "enumTupleVariantTypeNode");
        assert_eq!(variants[1]["tuple"]["items"][0]["format"], "u8");
        assert_eq!(variants[2]["kind"], "enumStructVariantTypeNode");
    }

    #[test]
    fn vec_array_option_coption() {
        let idl = json!({
            "address": "11111111111111111111111111111111",
            "metadata": { "name": "demo", "version": "0.1.0", "spec": "0.1.0" },
            "instructions": [{
                "name": "f",
                "discriminator": [0,0,0,0,0,0,0,0],
                "accounts": [],
                "args": [
                    { "name": "v", "type": { "vec": "u8" } },
                    { "name": "a", "type": { "array": ["u8", 4] } },
                    { "name": "o", "type": { "option": "u64" } },
                    { "name": "co", "type": { "coption": "u64" } }
                ]
            }]
        });
        let root = convert_str(&serde_json::to_string(&idl).unwrap());
        let args = root["program"]["instructions"][0]["arguments"]
            .as_array()
            .unwrap();
        // [0]=discriminator, [1..]=user args
        assert_eq!(args[1]["type"]["count"]["kind"], "prefixedCountNode");
        assert_eq!(args[2]["type"]["count"]["kind"], "fixedCountNode");
        assert_eq!(args[2]["type"]["count"]["value"], 4);
        assert_eq!(args[3]["type"]["kind"], "optionTypeNode");
        assert_eq!(args[3]["type"]["fixed"], false);
        assert_eq!(args[3]["type"]["prefix"]["format"], "u8");
        assert_eq!(args[4]["type"]["fixed"], true);
        assert_eq!(args[4]["type"]["prefix"]["format"], "u32");
    }

    #[test]
    fn generics_unwrap_value_and_const() {
        let idl = json!({
            "address": "11111111111111111111111111111111",
            "metadata": { "name": "demo", "version": "0.1.0", "spec": "0.1.0" },
            "instructions": [{
                "name": "f",
                "discriminator": [0,0,0,0,0,0,0,0],
                "accounts": [],
                "args": [
                    { "name": "x", "type": {
                        "defined": {
                            "name": "Wrap",
                            "generics": [
                                { "kind": "type", "type": "u64" },
                                { "kind": "const", "value": "3" }
                            ]
                        }
                    }}
                ]
            }],
            "types": [{
                "name": "Wrap",
                "generics": [
                    { "kind": "type", "name": "T" },
                    { "kind": "const", "name": "N", "type": "usize" }
                ],
                "type": {
                    "kind": "struct",
                    "fields": [
                        { "name": "items", "type": { "array": [{ "generic": "T" }, { "generic": "N" }] } }
                    ]
                }
            }]
        });
        let root = convert_str(&serde_json::to_string(&idl).unwrap());
        let arg = &root["program"]["instructions"][0]["arguments"][1];
        // Wrap<u64, 3> -> struct { items: [u64; 3] }
        assert_eq!(arg["type"]["kind"], "structTypeNode");
        let items_field = &arg["type"]["fields"][0];
        assert_eq!(items_field["name"], "items");
        assert_eq!(items_field["type"]["item"]["format"], "u64");
        assert_eq!(items_field["type"]["count"]["value"], 3);
    }

    #[test]
    fn pda_seeds_const_account_arg() {
        let idl = json!({
            "address": "11111111111111111111111111111111",
            "metadata": { "name": "demo", "version": "0.1.0", "spec": "0.1.0" },
            "instructions": [{
                "name": "f",
                "discriminator": [0,0,0,0,0,0,0,0],
                "accounts": [
                    {
                        "name": "vault",
                        "pda": {
                            "seeds": [
                                { "kind": "const", "value": [118, 97, 117, 108, 116] },
                                { "kind": "account", "path": "owner" },
                                { "kind": "arg", "path": "id" }
                            ]
                        }
                    },
                    { "name": "owner", "signer": true }
                ],
                "args": [
                    { "name": "id", "type": "u64" }
                ]
            }]
        });
        let root = convert_str(&serde_json::to_string(&idl).unwrap());
        let acc = &root["program"]["instructions"][0]["accounts"][0];
        assert_eq!(acc["name"], "vault");
        let dv = &acc["defaultValue"];
        assert_eq!(dv["kind"], "pdaValueNode");
        let seeds = dv["pda"]["seeds"].as_array().unwrap();
        assert_eq!(seeds[0]["kind"], "constantPdaSeedNode");
        // "vault" UTF-8 bytes encoded as base58.
        assert_eq!(seeds[0]["value"]["data"], "EMeDBmd");
        assert_eq!(seeds[1]["kind"], "variablePdaSeedNode");
        assert_eq!(seeds[1]["name"], "owner");
        assert_eq!(seeds[2]["name"], "id");
        assert_eq!(seeds[2]["type"]["format"], "u64");
        let values = dv["seeds"].as_array().unwrap();
        assert_eq!(values.len(), 2); // const seed has no value
        assert_eq!(values[0]["value"]["kind"], "accountValueNode");
        assert_eq!(values[1]["value"]["kind"], "argumentValueNode");
    }

    #[test]
    fn pda_arg_string_seed_unwraps_borsh_prefix() {
        let idl = json!({
            "address": "11111111111111111111111111111111",
            "metadata": { "name": "demo", "version": "0.1.0", "spec": "0.1.0" },
            "instructions": [{
                "name": "f",
                "discriminator": [0,0,0,0,0,0,0,0],
                "accounts": [{
                    "name": "vault",
                    "pda": { "seeds": [{ "kind": "arg", "path": "label" }] }
                }],
                "args": [{ "name": "label", "type": "string" }]
            }]
        });
        let root = convert_str(&serde_json::to_string(&idl).unwrap());
        let acc = &root["program"]["instructions"][0]["accounts"][0];
        let seed = &acc["defaultValue"]["pda"]["seeds"][0];
        assert_eq!(seed["type"]["kind"], "stringTypeNode");
        assert_eq!(seed["type"]["encoding"], "utf8");
    }

    #[test]
    fn composite_accounts_get_prefixed_on_collision() {
        let idl = json!({
            "address": "11111111111111111111111111111111",
            "metadata": { "name": "demo", "version": "0.1.0", "spec": "0.1.0" },
            "instructions": [{
                "name": "f",
                "discriminator": [0,0,0,0,0,0,0,0],
                "args": [],
                "accounts": [
                    { "name": "a", "accounts": [
                        { "name": "user", "writable": true }
                    ]},
                    { "name": "b", "accounts": [
                        { "name": "user", "writable": false }
                    ]}
                ]
            }]
        });
        let root = convert_str(&serde_json::to_string(&idl).unwrap());
        let accs = root["program"]["instructions"][0]["accounts"]
            .as_array()
            .unwrap();
        let names: Vec<&str> = accs.iter().map(|a| a["name"].as_str().unwrap()).collect();
        assert_eq!(names, vec!["aUser", "bUser"]);
    }

    #[test]
    fn pda_with_nested_path_drops_default_value() {
        let idl = json!({
            "address": "11111111111111111111111111111111",
            "metadata": { "name": "demo", "version": "0.1.0", "spec": "0.1.0" },
            "instructions": [{
                "name": "f",
                "discriminator": [0,0,0,0,0,0,0,0],
                "accounts": [{
                    "name": "child",
                    "pda": { "seeds": [{ "kind": "account", "path": "parent.field" }] }
                }],
                "args": []
            }]
        });
        let root = convert_str(&serde_json::to_string(&idl).unwrap());
        let acc = &root["program"]["instructions"][0]["accounts"][0];
        assert!(acc.get("defaultValue").is_none());
    }

    #[test]
    fn language_id_and_renderer_package_are_stable() {
        // The script name doubles as the per-language output subdirectory, so
        // changing it would silently move users' generated clients.
        assert_eq!(Language::Js.id(), "js");
        assert_eq!(Language::JsUmi.id(), "js-umi");
        assert_eq!(Language::Rust.id(), "rust");
        assert_eq!(Language::Go.id(), "go");
        assert_eq!(Language::Js.renderer_package(), "@codama/renderers-js");
        assert_eq!(
            Language::JsUmi.renderer_package(),
            "@codama/renderers-js-umi"
        );
        assert_eq!(Language::Rust.renderer_package(), "@codama/renderers-rust");
        assert_eq!(Language::Go.renderer_package(), "@codama/renderers-go");
    }

    #[test]
    fn generate_cli_parses_repeated_and_comma_separated_languages() {
        use clap::Parser;
        // Sanity-check the flag plumbing: `-l go,js -l rust` should yield three
        // distinct languages, in the order they were supplied.
        let parsed = CodamaCommand::try_parse_from([
            "codama", "generate", "-l", "go,js", "-l", "rust", "-p", "out", "idl.json",
        ])
        .expect("flags parse");
        match parsed {
            CodamaCommand::Generate {
                language,
                path,
                idl,
            } => {
                assert_eq!(language, vec![Language::Go, Language::Js, Language::Rust]);
                assert_eq!(path, "out");
                assert_eq!(idl, "idl.json");
            }
            other => panic!("expected Generate, got {other:?}"),
        }
    }

    #[test]
    fn language_from_id_is_inverse_of_id() {
        for lang in [Language::Js, Language::JsUmi, Language::Rust, Language::Go] {
            assert_eq!(Language::from_id(lang.id()), Some(lang));
        }
        assert_eq!(Language::from_id("python"), None);
    }

    #[test]
    fn auto_generate_noops_when_auto_disabled() {
        // `auto = false` (the default) must not spawn Codama even when a
        // language is enabled and an IDL is present — otherwise a workspace
        // that just declared `[clients]` for documentation purposes would
        // start triggering downloads on every `anchor build`.
        use crate::config::{ClientLanguageConfig, ClientsConfig};
        let cfg = ClientsConfig {
            auto: false,
            rust: Some(ClientLanguageConfig::Enabled(true)),
            ..Default::default()
        };
        let tmp = std::env::temp_dir().join("anchor_codama_auto_disabled");
        let _ = fs::remove_dir_all(&tmp);
        fs::create_dir_all(&tmp).unwrap();
        let idl = tmp.join("p.json");
        fs::write(&idl, "{}").unwrap();
        // If Codama were spawned this would fail (no `npx`/`codama` in test
        // env, no real IDL); it returns Ok(()) because we short-circuit.
        auto_generate_for_workspace(&cfg, &tmp, &[idl]).unwrap();
        fs::remove_dir_all(&tmp).ok();
    }

    #[test]
    fn auto_generate_warns_when_no_languages_enabled() {
        use crate::config::ClientsConfig;
        let cfg = ClientsConfig {
            auto: true,
            ..Default::default()
        };
        let tmp = std::env::temp_dir().join("anchor_codama_no_langs");
        let _ = fs::remove_dir_all(&tmp);
        fs::create_dir_all(&tmp).unwrap();
        // `auto = true` but every language is `None` → the function logs a
        // warning and exits cleanly without invoking Codama.
        auto_generate_for_workspace(&cfg, &tmp, &[]).unwrap();
        fs::remove_dir_all(&tmp).ok();
    }

    #[test]
    fn defined_link_is_emitted_for_non_generic_reference() {
        let idl = json!({
            "address": "11111111111111111111111111111111",
            "metadata": { "name": "demo", "version": "0.1.0", "spec": "0.1.0" },
            "instructions": [{
                "name": "f",
                "discriminator": [0,0,0,0,0,0,0,0],
                "accounts": [],
                "args": [
                    { "name": "s", "type": { "defined": { "name": "MyStruct" } } }
                ]
            }],
            "types": [{
                "name": "MyStruct",
                "type": { "kind": "struct", "fields": [] }
            }]
        });
        let root = convert_str(&serde_json::to_string(&idl).unwrap());
        let arg = &root["program"]["instructions"][0]["arguments"][1];
        assert_eq!(arg["type"]["kind"], "definedTypeLinkNode");
        assert_eq!(arg["type"]["name"], "myStruct");
    }
}
