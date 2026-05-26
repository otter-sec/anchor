//! Host-side test utilities for Anchor v2 programs.
//!
//! Drop-in replacement for `litesvm::LiteSVM::new()` that — when built
//! with the `profile` feature — records SBF register traces per test
//! under `target/anchor-v2-profile/<test_name>/`.
//!
//! `anchor test --profile` builds your tests with this feature active
//! and post-processes the trace files into flamegraphs.

pub use litesvm::LiteSVM;

// Re-exports so scaffold test files can `use anchor_v2_testing::{Keypair,
// Signer, Message, VersionedMessage, VersionedTransaction}` without each
// scaffold carrying direct deps on the individual `solana-*` crates.
pub use {
    solana_keypair::Keypair,
    solana_message::{Message, VersionedMessage},
    solana_signer::Signer,
    solana_transaction::versioned::VersionedTransaction,
};

#[cfg(feature = "profile")]
mod profile;

#[cfg(feature = "profile")]
pub use profile::{svm, svm_with_trace_dir};

/// When the `profile` feature is off, `svm()` is just `LiteSVM::new()`
/// with zero runtime overhead.
#[cfg(not(feature = "profile"))]
pub fn svm() -> LiteSVM {
    LiteSVM::new()
}

#[cfg(not(feature = "profile"))]
pub fn svm_with_trace_dir(_trace_dir: impl Into<std::path::PathBuf>) -> LiteSVM {
    LiteSVM::new()
}
