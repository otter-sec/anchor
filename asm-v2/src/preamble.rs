//! Generates `.equ` assembly constants from `#[account]` structs in lib.rs.
//!
//! For each struct annotated with `#[account]`:
//! - `StructName__SIZE` — `size_of::<Struct>()`
//! - `StructName__DISC_SIZE` — 8 (anchor discriminator)
//! - `StructName__INIT_SPACE` — 8 + size_of (total account allocation)
//! - `StructName__field` — byte offset of each field
//!
//! Offsets and sizes are evaluated by rustc in the program crate through
//! `core::mem::offset_of!` and `core::mem::size_of!` const operands. The build
//! script only discovers eligible structs and emits the public `.equ` names.

use std::{
    collections::HashSet,
    path::{Path, PathBuf},
};
use syn::{punctuated::Punctuated, Meta, Token};

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct RustConstOperand {
    pub(crate) name: String,
    pub(crate) expression: String,
}

/// Parse `lib.rs` and generate `.equ` preamble for all `#[account]` structs.
#[cfg_attr(not(test), allow(dead_code))]
pub fn generate(lib_rs: &Path) -> String {
    generate_with_operands(lib_rs).0
}

/// Like [`generate`], but also returns every Rust source file that was parsed
/// while walking the module tree. Callers can use the returned paths to emit
/// `cargo:rerun-if-changed=` directives.
#[cfg_attr(not(test), allow(dead_code))]
pub(crate) fn generate_tracked(lib_rs: &Path) -> (String, Vec<PathBuf>) {
    let (preamble, _, files) = generate_with_operands(lib_rs);
    (preamble, files)
}

pub(crate) fn generate_with_operands(
    lib_rs: &Path,
) -> (String, Vec<RustConstOperand>, Vec<PathBuf>) {
    let root_dir = lib_rs.parent().unwrap_or_else(|| Path::new("."));
    let mut visited = HashSet::new();
    let mut operands = Vec::new();
    let output = generate_file(lib_rs, root_dir, &mut visited, &[], &mut operands);
    let mut visited_files: Vec<_> = visited.into_iter().collect();
    visited_files.sort();
    (output, operands, visited_files)
}

fn generate_file(
    path: &Path,
    module_dir: &Path,
    visited: &mut HashSet<PathBuf>,
    module_path: &[String],
    operands: &mut Vec<RustConstOperand>,
) -> String {
    let canonical = std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
    if !visited.insert(canonical) {
        return String::new();
    }

    let source =
        std::fs::read_to_string(path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
    let file = match syn::parse_file(&source) {
        Ok(f) => f,
        Err(e) => {
            eprintln!("anchor-asm: warning: can't parse {}: {e}", path.display());
            return String::new();
        }
    };

    let mut out = String::new();
    let source_dir = path.parent().unwrap_or_else(|| Path::new("."));
    visit_items(
        &file.items,
        source_dir,
        module_dir,
        visited,
        module_path,
        operands,
        &mut out,
    );
    out
}

fn visit_items(
    items: &[syn::Item],
    source_dir: &Path,
    module_dir: &Path,
    visited: &mut HashSet<PathBuf>,
    module_path: &[String],
    operands: &mut Vec<RustConstOperand>,
    out: &mut String,
) {
    for item in items {
        match item {
            syn::Item::Struct(s)
                if has_account_attr(s) && cfg_enabled(&s.attrs) == CfgState::Enabled =>
            {
                if let Some(block) = emit_struct(s, module_path, operands) {
                    out.push_str(&block);
                }
            }
            syn::Item::Mod(m) if cfg_enabled(&m.attrs) == CfgState::Enabled => visit_module(
                m,
                source_dir,
                module_dir,
                visited,
                module_path,
                operands,
                out,
            ),
            _ => {}
        }
    }
}

fn visit_module(
    module: &syn::ItemMod,
    source_dir: &Path,
    module_dir: &Path,
    visited: &mut HashSet<PathBuf>,
    module_path: &[String],
    operands: &mut Vec<RustConstOperand>,
    out: &mut String,
) {
    let mut child_module_path = module_path.to_vec();
    child_module_path.push(module.ident.to_string());
    if let Some((_, items)) = &module.content {
        let child_dir = inline_module_dir(source_dir, module_dir, module);
        visit_items(
            items,
            &child_dir,
            &child_dir,
            visited,
            &child_module_path,
            operands,
            out,
        );
    } else if let Some((path, child_dir)) = resolve_module_file(source_dir, module_dir, module) {
        out.push_str(&generate_file(
            &path,
            &child_dir,
            visited,
            &child_module_path,
            operands,
        ));
    }
}

fn inline_module_dir(source_dir: &Path, module_dir: &Path, module: &syn::ItemMod) -> PathBuf {
    module_path_attr(source_dir, module)
        .map(|path| explicit_module_dir(&path))
        .unwrap_or_else(|| module_dir.join(module.ident.to_string()))
}

fn resolve_module_file(
    source_dir: &Path,
    module_dir: &Path,
    module: &syn::ItemMod,
) -> Option<(PathBuf, PathBuf)> {
    if let Some(path) = module_path_attr(source_dir, module) {
        return (path.exists() && path.is_file()).then(|| {
            let child_dir = explicit_module_dir(&path);
            (path, child_dir)
        });
    }

    let module_name = module.ident.to_string();
    let file = module_dir.join(format!("{module_name}.rs"));
    if file.exists() {
        return Some((file, module_dir.join(module_name)));
    }

    let mod_rs = module_dir.join(module_name).join("mod.rs");
    if mod_rs.exists() {
        let child_dir = mod_rs
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .to_path_buf();
        Some((mod_rs, child_dir))
    } else {
        None
    }
}

fn explicit_module_dir(path: &Path) -> PathBuf {
    if path.is_dir() || path.extension().is_none() {
        path.to_path_buf()
    } else {
        path.parent()
            .unwrap_or_else(|| Path::new("."))
            .to_path_buf()
    }
}

fn module_path_attr(source_dir: &Path, module: &syn::ItemMod) -> Option<PathBuf> {
    module
        .attrs
        .iter()
        .find_map(|attr| {
            let Meta::NameValue(nv) = &attr.meta else {
                return None;
            };
            if !nv.path.is_ident("path") {
                return None;
            }
            let syn::Expr::Lit(syn::ExprLit {
                lit: syn::Lit::Str(path),
                ..
            }) = &nv.value
            else {
                return None;
            };
            Some(path.value())
        })
        .map(PathBuf::from)
        .map(|path| {
            if path.is_absolute() {
                path
            } else {
                source_dir.join(path)
            }
        })
}

/// Check if a struct should have assembly constants generated.
/// Matches `#[account]` (anchor v2) or `#[repr(C)]` (plain Pod).
fn has_account_attr(s: &syn::ItemStruct) -> bool {
    let account_attrs: Vec<_> = s
        .attrs
        .iter()
        .filter(|attr| attr.path().is_ident("account"))
        .collect();
    let repr_attrs: Vec<_> = s
        .attrs
        .iter()
        .filter(|attr| attr.path().is_ident("repr"))
        .collect();

    if !account_attrs.is_empty() {
        return account_attrs.len() == 1
            && is_plain_account_attr(account_attrs[0])
            && repr_attrs.is_empty();
    }
    repr_attrs.len() == 1 && is_exact_repr_c(repr_attrs[0])
}

fn is_plain_account_attr(attr: &syn::Attribute) -> bool {
    matches!(&attr.meta, syn::Meta::Path(path) if path.is_ident("account"))
}

fn is_exact_repr_c(attr: &syn::Attribute) -> bool {
    let syn::Meta::List(list) = &attr.meta else {
        return false;
    };
    if !attr.path().is_ident("repr") {
        return false;
    }

    let Ok(args) = list.parse_args_with(
        syn::punctuated::Punctuated::<syn::Meta, syn::Token![,]>::parse_terminated,
    ) else {
        return false;
    };

    args.len() == 1 && matches!(args.first(), Some(syn::Meta::Path(path)) if path.is_ident("C"))
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum CfgState {
    Enabled,
    Disabled,
    Unknown,
}

fn cfg_enabled(attrs: &[syn::Attribute]) -> CfgState {
    attrs
        .iter()
        .filter_map(|attr| {
            if attr.path().is_ident("cfg") {
                Some(eval_cfg_attr(attr))
            } else if attr.path().is_ident("cfg_attr") {
                Some(CfgState::Unknown)
            } else {
                None
            }
        })
        .fold(CfgState::Enabled, combine_all)
}

fn combine_all(left: CfgState, right: CfgState) -> CfgState {
    match (left, right) {
        (CfgState::Disabled, _) | (_, CfgState::Disabled) => CfgState::Disabled,
        (CfgState::Unknown, _) | (_, CfgState::Unknown) => CfgState::Unknown,
        _ => CfgState::Enabled,
    }
}

fn eval_cfg_attr(attr: &syn::Attribute) -> CfgState {
    let Ok(meta) = attr.parse_args::<Meta>() else {
        return CfgState::Unknown;
    };
    eval_cfg_meta(&meta)
}

fn eval_cfg_meta(meta: &Meta) -> CfgState {
    match meta {
        Meta::Path(path) => path
            .get_ident()
            .map(|ident| cfg_flag_is_set(&ident.to_string()))
            .unwrap_or(CfgState::Unknown),
        Meta::NameValue(nv) => {
            let Some(key) = nv.path.get_ident().map(|ident| ident.to_string()) else {
                return CfgState::Unknown;
            };
            let syn::Expr::Lit(syn::ExprLit {
                lit: syn::Lit::Str(value),
                ..
            }) = &nv.value
            else {
                return CfgState::Unknown;
            };
            cfg_key_matches(&key, &value.value())
        }
        Meta::List(list) if list.path.is_ident("all") => parse_cfg_list(list)
            .map(|items| {
                items
                    .into_iter()
                    .map(|item| eval_cfg_meta(&item))
                    .fold(CfgState::Enabled, combine_all)
            })
            .unwrap_or(CfgState::Unknown),
        Meta::List(list) if list.path.is_ident("any") => parse_cfg_list(list)
            .map(|items| combine_any(items.into_iter().map(|item| eval_cfg_meta(&item))))
            .unwrap_or(CfgState::Unknown),
        Meta::List(list) if list.path.is_ident("not") => parse_cfg_list(list)
            .and_then(|items| (items.len() == 1).then(|| eval_cfg_meta(&items[0])))
            .map(|state| match state {
                CfgState::Enabled => CfgState::Disabled,
                CfgState::Disabled => CfgState::Enabled,
                CfgState::Unknown => CfgState::Unknown,
            })
            .unwrap_or(CfgState::Unknown),
        _ => CfgState::Unknown,
    }
}

fn combine_any(states: impl Iterator<Item = CfgState>) -> CfgState {
    let mut saw_unknown = false;
    let mut saw_state = false;
    for state in states {
        saw_state = true;
        match state {
            CfgState::Enabled => return CfgState::Enabled,
            CfgState::Unknown => saw_unknown = true,
            CfgState::Disabled => {}
        }
    }
    if saw_unknown {
        CfgState::Unknown
    } else if saw_state {
        CfgState::Disabled
    } else {
        CfgState::Disabled
    }
}

fn parse_cfg_list(list: &syn::MetaList) -> Option<Vec<Meta>> {
    list.parse_args_with(Punctuated::<Meta, Token![,]>::parse_terminated)
        .ok()
        .map(|items| items.into_iter().collect())
}

fn cfg_flag_is_set(flag: &str) -> CfgState {
    if !is_supported_cfg_key(flag) {
        return CfgState::Unknown;
    }
    let env_key = format!("CARGO_CFG_{}", cfg_env_name(flag));
    if std::env::var_os(&env_key).is_some() {
        CfgState::Enabled
    } else {
        CfgState::Disabled
    }
}

fn cfg_key_matches(key: &str, value: &str) -> CfgState {
    if key == "feature" {
        let feature_key = format!("CARGO_FEATURE_{}", cfg_env_name(value));
        return if std::env::var_os(&feature_key).is_some() {
            CfgState::Enabled
        } else {
            CfgState::Disabled
        };
    }
    if !is_supported_cfg_key(key) {
        return CfgState::Unknown;
    }
    let env_key = format!("CARGO_CFG_{}", cfg_env_name(key));
    let Some(raw) = std::env::var_os(&env_key) else {
        return CfgState::Disabled;
    };
    if raw.to_string_lossy().split(',').any(|entry| entry == value) {
        CfgState::Enabled
    } else {
        CfgState::Disabled
    }
}

fn is_supported_cfg_key(key: &str) -> bool {
    matches!(
        key,
        "debug_assertions"
            | "doc"
            | "doctest"
            | "miri"
            | "panic"
            | "proc_macro"
            | "test"
            | "unix"
            | "windows"
            | "target_arch"
            | "target_endian"
            | "target_env"
            | "target_family"
            | "target_feature"
            | "target_has_atomic"
            | "target_os"
            | "target_pointer_width"
            | "target_vendor"
            | "target_thread_local"
            | "compile_mode"
    )
}

fn cfg_env_name(value: &str) -> String {
    value
        .replace('-', "_")
        .chars()
        .flat_map(|ch| ch.to_uppercase())
        .collect()
}

/// Emit `.equ` constants for a single struct.
fn emit_struct(
    s: &syn::ItemStruct,
    module_path: &[String],
    operands: &mut Vec<RustConstOperand>,
) -> Option<String> {
    let name = &s.ident;
    let fields = match &s.fields {
        syn::Fields::Named(f) => &f.named,
        _ => return None,
    };

    let mut enabled_fields = Vec::new();
    for field in fields {
        match cfg_enabled(&field.attrs) {
            CfgState::Enabled => enabled_fields.push(field),
            CfgState::Disabled => {}
            CfgState::Unknown => return None,
        }
    }

    let type_path = rust_type_path(module_path, name);
    let mut out = String::new();
    out.push_str(&format!("# {name} field offsets and sizes.\n"));
    out.push_str(&format!("# {}\n", "-".repeat(70)));

    for field in enabled_fields {
        let field_name = field.ident.as_ref()?;

        // Skip fields starting with _ (padding).
        let name_str = field_name.to_string();
        if name_str.starts_with('_') {
            continue;
        }

        let operand_name = operand_name(module_path, name, &name_str);
        out.push_str(&format!(".equ {name}__{field_name}, {{{operand_name}}}\n"));
        operands.push(RustConstOperand {
            name: operand_name,
            expression: format!("core::mem::offset_of!({type_path}, {field_name}) as i32"),
        });
    }

    let size_operand = operand_name(module_path, name, "SIZE");
    out.push_str(&format!(".equ {name}__SIZE, {{{size_operand}}}\n"));
    operands.push(RustConstOperand {
        name: size_operand,
        expression: format!("core::mem::size_of::<{type_path}>() as i32"),
    });
    out.push_str(&format!(".equ {name}__DISC_SIZE, 8\n"));
    let init_space_operand = operand_name(module_path, name, "INIT_SPACE");
    out.push_str(&format!(
        ".equ {name}__INIT_SPACE, {{{init_space_operand}}}\n"
    ));
    operands.push(RustConstOperand {
        name: init_space_operand,
        expression: format!("(8 + core::mem::size_of::<{type_path}>()) as i32"),
    });
    out.push_str(&format!("# {}\n\n", "-".repeat(70)));

    Some(out)
}

fn rust_type_path(module_path: &[String], name: &syn::Ident) -> String {
    let mut path = String::from("crate");
    for segment in module_path {
        path.push_str("::");
        path.push_str(segment);
    }
    path.push_str("::");
    path.push_str(&name.to_string());
    path
}

fn operand_name(module_path: &[String], name: &syn::Ident, suffix: &str) -> String {
    let mut parts = vec![String::from("__anchor_asm")];
    parts.extend(module_path.iter().map(|segment| sanitize_ident(segment)));
    parts.push(sanitize_ident(&name.to_string()));
    parts.push(sanitize_ident(suffix));
    parts.join("_")
}

fn sanitize_ident(value: &str) -> String {
    let value = value.strip_prefix("r#").unwrap_or(value);
    let mut result: String = value
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '_' {
                ch
            } else {
                '_'
            }
        })
        .collect();
    if result.is_empty() || result.starts_with(|ch: char| ch.is_ascii_digit()) {
        result.insert(0, '_');
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        sync::{Mutex, OnceLock},
        time::{SystemTime, UNIX_EPOCH},
    };

    fn env_lock() -> std::sync::MutexGuard<'static, ()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(())).lock().unwrap()
    }

    fn temp_test_dir(name: &str) -> PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!(
            "anchor-asm-v2-{name}-{}-{unique}",
            std::process::id()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn with_env_var<F>(key: &str, value: Option<&str>, f: F)
    where
        F: FnOnce(),
    {
        let _guard = env_lock();
        let previous = std::env::var_os(key);
        match value {
            Some(value) => std::env::set_var(key, value),
            None => std::env::remove_var(key),
        }
        f();
        match previous {
            Some(value) => std::env::set_var(key, value),
            None => std::env::remove_var(key),
        }
    }

    fn assert_placeholder(result: &str, struct_name: &str, field_name: &str) {
        let prefix = format!(".equ {struct_name}__{field_name}, {{");
        assert!(
            result.contains(&prefix),
            "missing placeholder {prefix:?}: {result}"
        );
    }

    #[test]
    fn test_simple_struct() {
        let source = r#"
            #[account]
            pub struct Counter {
                pub value: u64,
                pub bump: u8,
                pub _pad: [u8; 7],
            }
        "#;
        let tmp = std::env::temp_dir().join("anchor_asm_test_lib.rs");
        std::fs::write(&tmp, source).unwrap();
        let result = generate(&tmp);
        assert_placeholder(&result, "Counter", "value");
        assert_placeholder(&result, "Counter", "bump");
        assert_placeholder(&result, "Counter", "SIZE");
        assert_placeholder(&result, "Counter", "INIT_SPACE");
        std::fs::remove_file(tmp).ok();
    }

    #[test]
    fn test_address_field() {
        let source = r#"
            #[account]
            pub struct Config {
                pub admin: Address,
                pub bump: u8,
            }
        "#;
        let tmp = std::env::temp_dir().join("anchor_asm_test_addr.rs");
        std::fs::write(&tmp, source).unwrap();
        let result = generate(&tmp);
        // Address is 32 bytes, align 1
        assert_placeholder(&result, "Config", "admin");
        assert_placeholder(&result, "Config", "bump");
        std::fs::remove_file(tmp).ok();
    }

    #[test]
    fn test_account_attr_must_be_plain_or_exact_repr_c() {
        let source = r#"
            #[account]
            pub struct ZeroCopy {
                pub value: u64,
            }

            #[account(borsh)]
            pub struct BorshBacked {
                pub value: u64,
            }

            #[account(zero_copy)]
            pub struct MacroArgs {
                pub value: u64,
            }

            #[account(borsh)]
            #[repr(C)]
            pub struct BorshReprC {
                pub value: u64,
            }

            #[account]
            #[repr(C)]
            pub struct ZeroCopyWithUserReprC {
                pub value: u64,
            }

            #[account]
            #[repr(packed)]
            pub struct ZeroCopyPacked {
                pub value: u64,
            }

            #[account]
            #[repr(align(8))]
            pub struct ZeroCopyAligned {
                pub value: u64,
            }

            #[repr(C)]
            pub struct PlainPod {
                pub value: u64,
            }

            #[repr(packed)]
            pub struct PackedPod {
                pub value: u64,
            }

            #[repr(transparent)]
            pub struct TransparentPod {
                pub value: u64,
            }

            #[repr(C, packed)]
            pub struct MixedReprPod {
                pub value: u64,
            }

            #[repr(C)]
            #[repr(align(8))]
            pub struct SplitAlignedPod {
                pub value: u64,
            }

            #[repr(C)]
            #[repr(packed)]
            pub struct SplitPackedPod {
                pub value: u64,
            }
        "#;
        let tmp = std::env::temp_dir().join("anchor_asm_test_attrs.rs");
        std::fs::write(&tmp, source).unwrap();
        let result = generate(&tmp);
        assert_placeholder(&result, "ZeroCopy", "value");
        assert_placeholder(&result, "PlainPod", "value");
        assert!(!result.contains("BorshBacked__value"));
        assert!(!result.contains("MacroArgs__value"));
        assert!(!result.contains("BorshReprC__value"));
        assert!(!result.contains("ZeroCopyWithUserReprC__value"));
        assert!(!result.contains("ZeroCopyPacked__value"));
        assert!(!result.contains("ZeroCopyAligned__value"));
        assert!(!result.contains("PackedPod__value"));
        assert!(!result.contains("TransparentPod__value"));
        assert!(!result.contains("MixedReprPod__value"));
        assert!(!result.contains("SplitAlignedPod__value"));
        assert!(!result.contains("SplitPackedPod__value"));
        std::fs::remove_file(tmp).ok();
    }

    #[test]
    fn test_generate_respects_cfg_gated_fields() {
        let source = r#"
            #[repr(C)]
            pub struct CfgFieldLayout {
                pub tag: u8,
                #[cfg(feature = "asm_cfg_field")]
                pub gated: u64,
                pub bump: u8,
            }
        "#;
        let tmp = std::env::temp_dir().join("anchor_asm_test_cfg_field.rs");
        std::fs::write(&tmp, source).unwrap();

        with_env_var("CARGO_FEATURE_ASM_CFG_FIELD", None, || {
            let result = generate(&tmp);
            assert_placeholder(&result, "CfgFieldLayout", "tag");
            assert_placeholder(&result, "CfgFieldLayout", "bump");
            assert_placeholder(&result, "CfgFieldLayout", "SIZE");
            assert!(!result.contains("CfgFieldLayout__gated"));
        });

        with_env_var("CARGO_FEATURE_ASM_CFG_FIELD", Some("1"), || {
            let result = generate(&tmp);
            assert_placeholder(&result, "CfgFieldLayout", "tag");
            assert_placeholder(&result, "CfgFieldLayout", "gated");
            assert_placeholder(&result, "CfgFieldLayout", "bump");
            assert_placeholder(&result, "CfgFieldLayout", "SIZE");
        });

        std::fs::remove_file(tmp).ok();
    }

    #[test]
    fn test_generate_drops_struct_with_unknown_cfg() {
        let source = r#"
            #[repr(C)]
            pub struct UnknownCfg {
                pub tag: u8,
                #[cfg(unsupported_predicate = "value")]
                pub gated: u64,
                pub bump: u8,
            }
        "#;
        let tmp = std::env::temp_dir().join("anchor_asm_test_unknown_cfg.rs");
        std::fs::write(&tmp, source).unwrap();
        let result = generate(&tmp);
        assert!(!result.contains("UnknownCfg__"));
        std::fs::remove_file(tmp).ok();
    }

    #[test]
    fn test_generate_recurses_inline_and_file_backed_modules() {
        let dir = temp_test_dir("mods");
        let lib_rs = dir.join("lib.rs");
        let outer_dir = dir.join("outer");
        std::fs::create_dir_all(&outer_dir).unwrap();

        std::fs::write(
            &lib_rs,
            r#"
            mod outer {
                pub mod inline_leaf {
                    #[account]
                    pub struct NestedInline {
                        pub value: u64,
                    }

                    #[account]
                    #[repr(packed)]
                    pub struct NestedPacked {
                        pub value: u64,
                    }
                }

                pub mod file_leaf;
            }

            mod state;
            "#,
        )
        .unwrap();
        std::fs::write(
            outer_dir.join("file_leaf.rs"),
            r#"
            #[repr(C)]
            pub struct NestedFile {
                pub value: u64,
            }

            #[account(borsh)]
            pub struct NestedBorsh {
                pub value: u64,
            }
            "#,
        )
        .unwrap();
        std::fs::write(
            dir.join("state.rs"),
            r#"
            #[account]
            pub struct RootFile {
                pub value: u64,
            }
            "#,
        )
        .unwrap();

        let result = generate(&lib_rs);
        assert_placeholder(&result, "NestedInline", "value");
        assert_placeholder(&result, "NestedFile", "value");
        assert_placeholder(&result, "RootFile", "value");
        assert!(!result.contains("NestedPacked__value"));
        assert!(!result.contains("NestedBorsh__value"));

        std::fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn test_generate_respects_cfg_on_nested_modules() {
        let dir = temp_test_dir("cfg-mod");
        let lib_rs = dir.join("lib.rs");

        std::fs::write(
            &lib_rs,
            r#"
            #[cfg(feature = "asm_cfg_module")]
            mod gated {
                #[account]
                pub struct NestedEnabled {
                    pub value: u64,
                }
            }
            "#,
        )
        .unwrap();

        with_env_var("CARGO_FEATURE_ASM_CFG_MODULE", None, || {
            let result = generate(&lib_rs);
            assert!(!result.contains("NestedEnabled__value"));
        });

        with_env_var("CARGO_FEATURE_ASM_CFG_MODULE", Some("1"), || {
            let result = generate(&lib_rs);
            assert_placeholder(&result, "NestedEnabled", "value");
        });

        std::fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn test_generate_resolves_mod_rs_modules() {
        let dir = temp_test_dir("mod-rs");
        let lib_rs = dir.join("lib.rs");
        let outer_dir = dir.join("outer");
        std::fs::create_dir_all(&outer_dir).unwrap();

        std::fs::write(&lib_rs, "mod outer;\n").unwrap();
        std::fs::write(
            outer_dir.join("mod.rs"),
            r#"
            #[repr(C)]
            pub struct NestedFromModRs {
                pub value: u64,
            }
            "#,
        )
        .unwrap();

        let result = generate(&lib_rs);
        assert_placeholder(&result, "NestedFromModRs", "value");

        std::fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn test_generate_resolves_path_attr_modules() {
        let dir = temp_test_dir("path-attr");
        let lib_rs = dir.join("lib.rs");
        let alt_dir = dir.join("alt");
        std::fs::create_dir_all(&alt_dir).unwrap();

        std::fs::write(&lib_rs, "#[path = \"alt/custom_state.rs\"] mod state;\n").unwrap();
        std::fs::write(
            alt_dir.join("custom_state.rs"),
            r#"
            #[repr(C)]
            pub struct PathAttrState {
                pub value: u64,
            }
            "#,
        )
        .unwrap();

        let result = generate(&lib_rs);
        assert_placeholder(&result, "PathAttrState", "value");

        std::fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn test_generate_resolves_path_attr_relative_to_non_mod_rs_declaring_file() {
        let dir = temp_test_dir("path-attr-non-mod-rs");
        let lib_rs = dir.join("lib.rs");

        std::fs::write(&lib_rs, "mod state;\n").unwrap();
        std::fs::write(
            dir.join("state.rs"),
            r#"
            #[path = "custom.rs"]
            mod child;
            "#,
        )
        .unwrap();
        std::fs::write(
            dir.join("custom.rs"),
            r#"
            #[repr(C)]
            pub struct CustomState {
                pub value: u64,
            }
            "#,
        )
        .unwrap();

        let result = generate(&lib_rs);
        assert_placeholder(&result, "CustomState", "value");

        std::fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn test_generate_resolves_nested_path_attrs_inside_inline_path_modules() {
        let dir = temp_test_dir("inline-path-attr");
        let lib_rs = dir.join("lib.rs");
        let thread_files_dir = dir.join("thread_files");
        std::fs::create_dir_all(&thread_files_dir).unwrap();

        std::fs::write(
            &lib_rs,
            r#"
            #[path = "thread_files"]
            mod thread {
                #[path = "tls.rs"]
                mod local_data;
            }
            "#,
        )
        .unwrap();
        std::fs::write(
            thread_files_dir.join("tls.rs"),
            r#"
            #[repr(C)]
            pub struct InlinePathState {
                pub value: u64,
            }
            "#,
        )
        .unwrap();

        let result = generate(&lib_rs);
        assert_placeholder(&result, "InlinePathState", "value");

        std::fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn test_generate_tracks_nested_module_dependencies() {
        let dir = temp_test_dir("tracked-deps");
        let lib_rs = dir.join("lib.rs");
        let state_rs = dir.join("state.rs");
        let custom_rs = dir.join("custom.rs");

        std::fs::write(&lib_rs, "mod state;\n").unwrap();
        std::fs::write(
            &state_rs,
            r#"
            #[path = "custom.rs"]
            mod child;
            "#,
        )
        .unwrap();
        std::fs::write(
            &custom_rs,
            r#"
            #[repr(C)]
            pub struct NestedTracked {
                pub value: u64,
            }
            "#,
        )
        .unwrap();

        let (result, visited_files) = generate_tracked(&lib_rs);
        assert_placeholder(&result, "NestedTracked", "value");

        let canon = |path: &Path| std::fs::canonicalize(path).unwrap();
        assert!(visited_files.contains(&canon(&lib_rs)));
        assert!(visited_files.contains(&canon(&state_rs)));
        assert!(visited_files.contains(&canon(&custom_rs)));

        std::fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn test_generate_uses_rustc_operands_for_qualified_types() {
        let source = r#"
            mod domain {
                #[repr(C)]
                pub struct Address {
                    pub city: [u8; 64],
                    pub zip: u32,
                }
            }

            #[repr(C)]
            pub struct Registry {
                pub owner: Address,
                pub home: domain::Address,
                pub admin: Address,
            }
        "#;
        let tmp = std::env::temp_dir().join("anchor_asm_test_qualified_type.rs");
        std::fs::write(&tmp, source).unwrap();
        let (result, operands, _) = generate_with_operands(&tmp);
        assert!(result.contains(".equ Registry__admin, {__anchor_asm_Registry_admin}"));
        assert!(result.contains(".equ Registry__SIZE, {__anchor_asm_Registry_SIZE}"));
        assert!(operands.iter().any(|operand| {
            operand.name == "__anchor_asm_Registry_admin"
                && operand.expression == "core::mem::offset_of!(crate::Registry, admin) as i32"
        }));
        assert!(operands.iter().any(|operand| {
            operand.name == "__anchor_asm_Registry_SIZE"
                && operand.expression == "core::mem::size_of::<crate::Registry>() as i32"
        }));
        std::fs::remove_file(tmp).ok();
    }

    #[test]
    fn test_generate_emits_operands_for_generic_repr_c_fields() {
        let source = r#"
            #[repr(C)]
            pub struct Holder {
                pub prefix: u8,
                pub values: PodVec<u16, 1>,
                pub suffix: u8,
            }
        "#;
        let tmp = std::env::temp_dir().join("anchor_asm_test_pod_vec.rs");
        std::fs::write(&tmp, source).unwrap();
        let result = generate(&tmp);
        assert_placeholder(&result, "Holder", "prefix");
        assert_placeholder(&result, "Holder", "values");
        assert_placeholder(&result, "Holder", "suffix");
        assert_placeholder(&result, "Holder", "SIZE");
        std::fs::remove_file(tmp).ok();
    }

    #[test]
    fn test_generate_emits_operands_for_array_like_repr_c_fields() {
        let source = r#"
            #[repr(C)]
            pub struct ByteHolder {
                pub prefix: u8,
                pub values: PodVec<u8, 1>,
                pub suffix: u8,
            }
        "#;
        let tmp = std::env::temp_dir().join("anchor_asm_test_pod_vec_u8.rs");
        std::fs::write(&tmp, source).unwrap();
        let result = generate(&tmp);
        assert_placeholder(&result, "ByteHolder", "prefix");
        assert_placeholder(&result, "ByteHolder", "values");
        assert_placeholder(&result, "ByteHolder", "suffix");
        assert_placeholder(&result, "ByteHolder", "SIZE");
        std::fs::remove_file(tmp).ok();
    }

    #[test]
    fn test_generate_emits_operands_for_u128_fields() {
        let source = r#"
            #[repr(C)]
            pub struct Wide {
                pub tag: u8,
                pub value: u128,
                pub bump: u8,
            }
        "#;
        let tmp = std::env::temp_dir().join("anchor_asm_test_u128.rs");
        std::fs::write(&tmp, source).unwrap();
        let result = generate(&tmp);
        assert_placeholder(&result, "Wide", "tag");
        assert_placeholder(&result, "Wide", "value");
        assert_placeholder(&result, "Wide", "bump");
        assert_placeholder(&result, "Wide", "SIZE");
        std::fs::remove_file(tmp).ok();
    }

    #[test]
    fn test_generate_emits_operands_for_i128_fields() {
        let source = r#"
            #[repr(C)]
            pub struct SignedWide {
                pub tag: u8,
                pub value: i128,
                pub bump: u8,
            }
        "#;
        let tmp = std::env::temp_dir().join("anchor_asm_test_i128.rs");
        std::fs::write(&tmp, source).unwrap();
        let result = generate(&tmp);
        assert_placeholder(&result, "SignedWide", "tag");
        assert_placeholder(&result, "SignedWide", "value");
        assert_placeholder(&result, "SignedWide", "bump");
        assert_placeholder(&result, "SignedWide", "SIZE");
        std::fs::remove_file(tmp).ok();
    }
}
