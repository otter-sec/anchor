//! `anchor debugger` — a foundry-style SBF instruction stepper built on the
//! same per-test register traces that `--profile` consumes for flamegraphs.
//!
//! ## Flow
//!
//! 1. The caller invokes the existing `anchor test --profile` pipeline,
//!    which leaves traces under `target/anchor-v2-profile/<test>/`.
//! 2. [`run`] walks that directory, materializes a [`DebugSession`], and
//!    launches the TUI.
//!
//! Keeping the trace-producing half identical to `--profile` means the
//! debugger and flamegraph don't drift: any improvement to the trace
//! pipeline is picked up by both.

use {
    anyhow::Result,
    std::{
        collections::BTreeMap,
        path::{Path, PathBuf},
    },
};

pub mod arena;
pub mod cargo_deps;
pub mod gdb;
pub mod highlight;
pub mod loose;
pub mod model;
pub mod path_label;
pub mod rustc_wrapper;
pub mod source;
pub mod tui;

/// Default trace directory name (relative to the workspace root). Mirrors
/// `crate::profile::DEFAULT_PROFILE_DIR` so the loose-mode flow agrees
/// with the Anchor.toml-driven flow on where to read/write traces.
pub fn loose_profile_dir_name() -> &'static str {
    crate::profile::DEFAULT_PROFILE_DIR
}

/// Build a [`model::DebugSession`] from the profile trace directory and
/// launch the TUI. Blocks until the user quits.
pub fn run(
    profile_dir: &Path,
    programs: &BTreeMap<String, PathBuf>,
    manifest_dir: Option<&Path>,
    crate_dir: Option<&Path>,
    test_filter: Option<&str>,
) -> Result<()> {
    // Probe the terminal background BEFORE anything that might call into
    // `highlight::ctx()` (arena pre-highlights disasm). The detection
    // round-trips an OSC 11 query on a regular TTY; once the TUI raw-mode
    // takeover starts, the reply might not get back to us cleanly.
    highlight::detect_theme_mode_once();
    let session =
        arena::build_session(profile_dir, programs, manifest_dir, crate_dir, test_filter)?;
    tui::run(session)
}
