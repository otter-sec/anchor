//! Build-time support for linking hand-written SBPF assembly into Anchor v2
//! programs via `global_asm!`.
//!
//! Two usage modes:
//!
//! ## Simple mode — port existing assembly projects
//!
//! For projects that already have `.s` files with `.equ` constants (e.g.
//! from an external injection system), just concatenate and link:
//!
//! ```toml
//! # Cargo.toml
//! [build-dependencies]
//! anchor-asm-v2 = { path = "..." }
//! ```
//!
//! ```rust,ignore
//! // build.rs
//! fn main() {
//!     anchor_asm_v2::build("src/asm");
//! }
//! ```
//!
//! ```rust,ignore
//! // lib.rs
//! #![no_std]
//! #![no_main]
//! #![feature(asm_experimental_arch)]
//!
//! anchor_asm_v2::include_asm!();
//!
//! #[panic_handler]
//! fn panic(_: &core::panic::PanicInfo) -> ! { loop {} }
//! ```
//!
//! `build()` walks the assembly directory, expands `.include` directives,
//! and writes `$OUT_DIR/combined.s` plus a Rust wrapper containing
//! `global_asm!` const operands. `include_asm!()` includes that wrapper.
//!
//! ## Full mode — new programs with compile-time constants
//!
//! For new assembly programs, use `asm_program!` from the companion
//! `anchor-asm-v2-macros` crate to define types and generate `const`
//! operands in one block:
//!
//! ```rust,ignore
//! anchor_asm_v2_macros::asm_program! {
//!     #[error_enum(prefix = "E")]
//!     pub enum ErrorCode { InvalidDiscriminant, ... }
//!
//!     #[frame(prefix = "FM")]
//!     #[repr(C)]
//!     pub struct Frame { pub saved_r6: u64, pub bump: u8, ... }
//!
//!     asm { include_str!(concat!(env!("OUT_DIR"), "/combined.s")), }
//! }
//! ```

use anyhow::{anyhow, Context, Result};
use std::path::{Path, PathBuf};

// ---------------------------------------------------------------------------
// Simple mode: build() + include_asm!()
// ---------------------------------------------------------------------------

/// Emit `global_asm!` linking the combined assembly from `build()`.
///
/// Call at crate root scope. Requires `#![feature(asm_experimental_arch)]`.
#[macro_export]
macro_rules! include_asm {
    () => {
        include!(concat!(env!("OUT_DIR"), "/combined.rs"));
    };
}

/// Build-time entry point. Call from `build.rs` with the path to the
/// assembly source directory (relative to the crate root).
///
/// Walks the directory for `.s` files, expands `.include` directives,
/// and writes the concatenated result to `$OUT_DIR/combined.s` plus a Rust
/// wrapper at `$OUT_DIR/combined.rs`.
///
/// If `src/lib.rs` contains `#[repr(C)]` or `#[account]` structs,
/// `.equ` constants for field offsets are prepended automatically. Any Rust
/// modules parsed while generating that preamble are also registered as
/// `cargo:rerun-if-changed` inputs.
pub fn build(asm_dir: &str) {
    let manifest_dir =
        PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR not set"));
    let out_dir = PathBuf::from(std::env::var("OUT_DIR").expect("OUT_DIR not set"));
    let asm_path = manifest_dir.join(asm_dir);
    let lib_rs = manifest_dir.join("src").join("lib.rs");

    let (preamble, operands, preamble_files) = preamble_for_build(&lib_rs);

    let combined = collect_asm(&asm_path);

    let output = if preamble.is_empty() {
        combined
    } else {
        format!("{preamble}\n{combined}")
    };

    std::fs::write(out_dir.join("combined.s"), output).expect("write combined.s");
    std::fs::write(out_dir.join("combined.rs"), render_combined_rs(&operands))
        .expect("write combined.rs");

    println!("cargo:rerun-if-changed={asm_dir}");
    for path in preamble_files {
        println!("cargo:rerun-if-changed={}", path.display());
    }
}

/// Like `build()` but takes absolute paths. Skips the preamble — the
/// caller handles any preprocessing.
pub fn build_to(asm_dir: &Path, output_path: &Path) {
    let combined = collect_asm(asm_dir);
    std::fs::write(output_path, combined).expect("write combined assembly");
    println!("cargo:rerun-if-changed={}", asm_dir.display());
}

// ---------------------------------------------------------------------------
// Assembly collection and .include expansion
// ---------------------------------------------------------------------------

mod preamble;

fn preamble_for_build(lib_rs: &Path) -> (String, Vec<preamble::RustConstOperand>, Vec<PathBuf>) {
    if lib_rs.exists() {
        preamble::generate_with_operands(lib_rs)
    } else {
        (String::new(), Vec::new(), Vec::new())
    }
}

fn render_combined_rs(operands: &[preamble::RustConstOperand]) -> String {
    let mut output = String::from(
        "core::arch::global_asm!(\n    include_str!(concat!(env!(\"OUT_DIR\"), \"/combined.s\")),\n",
    );
    for operand in operands {
        output.push_str(&format!(
            "    {} = const {},\n",
            operand.name, operand.expression
        ));
    }
    output.push_str(");\n");
    output
}

fn collect_asm(dir: &Path) -> String {
    collect_asm_inner(dir).unwrap_or_else(|err| panic!("{err}"))
}

fn collect_asm_inner(dir: &Path) -> Result<String> {
    let mut files: Vec<PathBuf> = Vec::new();
    walk_dir(dir, &mut files);
    files.sort();

    let root_file = find_root_file(dir, &files);

    if let Some(root) = root_file {
        let mut stack = Vec::new();
        expand_includes(&root, dir, &mut stack)
    } else {
        let mut out = String::new();
        for file in &files {
            let content = std::fs::read_to_string(file)
                .with_context(|| format!("read {}", file.display()))?;
            out.push_str(&format!(
                "# --- {} ---\n",
                file.strip_prefix(dir).unwrap_or(file).display()
            ));
            out.push_str(&content);
            out.push('\n');
        }
        Ok(out)
    }
}

/// Find the root assembly file. Priority: dir-name match (e.g.
/// `dropset/dropset.s`) first, then well-known names.
fn find_root_file(dir: &Path, files: &[PathBuf]) -> Option<PathBuf> {
    if let Some(dir_name) = dir.file_name().and_then(|n| n.to_str()) {
        let candidate = dir.join(format!("{dir_name}.s"));
        if files.contains(&candidate) {
            return Some(candidate);
        }
    }

    for name in ["entrypoint.s", "main.s"] {
        let candidate = dir.join(name);
        if files.contains(&candidate) {
            return Some(candidate);
        }
    }

    None
}

fn expand_includes(path: &Path, base_dir: &Path, stack: &mut Vec<PathBuf>) -> Result<String> {
    let canonical = canonicalize_path(path);
    if let Some(pos) = stack.iter().position(|seen_path| *seen_path == canonical) {
        let mut cycle_paths = stack[pos..].to_vec();
        cycle_paths.push(canonical.clone());
        let cycle = cycle_paths
            .iter()
            .map(|entry| display_path(entry, base_dir))
            .collect::<Vec<_>>()
            .join(" -> ");
        return Err(anyhow!("assembly include cycle detected: {cycle}"));
    }

    stack.push(canonical);

    let content =
        std::fs::read_to_string(path).with_context(|| format!("read {}", path.display()))?;

    let mut out = String::new();
    let rel = path.strip_prefix(base_dir).unwrap_or(path);
    out.push_str(&format!("# --- {} ---\n", rel.display()));

    for line in content.lines() {
        let trimmed = line.trim();
        if let Some(rest) = trimmed.strip_prefix(".include") {
            let file = rest.trim().trim_matches('"');
            let include_path = path.parent().unwrap_or(base_dir).join(file);
            if include_path.exists() {
                out.push_str(&expand_includes(&include_path, base_dir, stack)?);
            } else {
                let from_base = base_dir.join(file);
                if from_base.exists() {
                    out.push_str(&expand_includes(&from_base, base_dir, stack)?);
                } else {
                    out.push_str(line);
                    out.push('\n');
                }
            }
        } else {
            out.push_str(line);
            out.push('\n');
        }
    }
    stack.pop();
    Ok(out)
}

fn walk_dir(dir: &Path, out: &mut Vec<PathBuf>) {
    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return,
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            walk_dir(&path, out);
        } else if path.extension().and_then(|e| e.to_str()) == Some("s") {
            out.push(path);
        }
    }
}

fn canonicalize_path(path: &Path) -> PathBuf {
    std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf())
}

fn display_path(path: &Path, base_dir: &Path) -> String {
    path.strip_prefix(base_dir)
        .unwrap_or(path)
        .display()
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        panic,
        time::{SystemTime, UNIX_EPOCH},
    };

    fn temp_test_dir(name: &str) -> PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!(
            "anchor-asm-v2-lib-{name}-{}-{unique}",
            std::process::id()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn panic_message(err: Box<dyn std::any::Any + Send>) -> String {
        match err.downcast::<String>() {
            Ok(msg) => *msg,
            Err(err) => match err.downcast::<&'static str>() {
                Ok(msg) => (*msg).to_string(),
                Err(_) => "non-string panic payload".to_string(),
            },
        }
    }

    #[test]
    fn test_preamble_for_build_tracks_nested_module_files() {
        let dir = temp_test_dir("tracked-preamble");
        let src_dir = dir.join("src");
        std::fs::create_dir_all(&src_dir).unwrap();

        let lib_rs = src_dir.join("lib.rs");
        let state_rs = src_dir.join("state.rs");
        let custom_rs = src_dir.join("custom.rs");

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
            pub struct BuildTracked {
                pub value: u64,
            }
            "#,
        )
        .unwrap();

        let (preamble, operands, tracked_files) = preamble_for_build(&lib_rs);
        assert!(preamble
            .contains(".equ BuildTracked__value, {__anchor_asm_state_child_BuildTracked_value}"));
        assert_eq!(operands.len(), 3);
        let combined_rs = render_combined_rs(&operands);
        assert!(combined_rs.contains("offset_of!(crate::state::child::BuildTracked, value)"));

        let canon = |path: &Path| std::fs::canonicalize(path).unwrap();
        assert!(tracked_files.contains(&canon(&lib_rs)));
        assert!(tracked_files.contains(&canon(&state_rs)));
        assert!(tracked_files.contains(&canon(&custom_rs)));

        std::fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn test_include_cycle_is_rejected() {
        let dir = temp_test_dir("cycle");
        let output = dir.join("combined.s");

        std::fs::write(dir.join("entrypoint.s"), ".include \"a.s\"\n").unwrap();
        std::fs::write(dir.join("a.s"), ".include \"b.s\"\n").unwrap();
        std::fs::write(dir.join("b.s"), ".include \"a.s\"\n").unwrap();

        let err = panic::catch_unwind(|| build_to(&dir, &output))
            .err()
            .expect("cyclic includes should panic");
        let message = panic_message(err);
        assert!(message.contains("assembly include cycle detected"));
        assert!(message.contains("a.s"));
        assert!(message.contains("b.s"));

        std::fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn test_non_cyclic_nested_includes_are_expanded() {
        let dir = temp_test_dir("acyclic");
        let output = dir.join("combined.s");

        std::fs::write(dir.join("entrypoint.s"), ".include \"a.s\"\nentry:\n").unwrap();
        std::fs::write(dir.join("a.s"), ".include \"nested/b.s\"\na:\n").unwrap();
        std::fs::create_dir_all(dir.join("nested")).unwrap();
        std::fs::write(dir.join("nested").join("b.s"), "b:\n").unwrap();

        build_to(&dir, &output);

        let combined = std::fs::read_to_string(&output).unwrap();
        assert!(combined.contains("# --- entrypoint.s ---"));
        assert!(combined.contains("# --- a.s ---"));
        assert!(combined.contains("# --- nested/b.s ---"));
        assert!(combined.contains("entry:"));
        assert!(combined.contains("a:"));
        assert!(combined.contains("b:"));

        std::fs::remove_dir_all(dir).ok();
    }
}
