//! Test scaffolding for anchor-v2 unsafe code paths.
//!
//! Construct stack-backed mock `AccountView` values and host-only serialized
//! SBF input buffers. Integration tests use these fixtures to exercise the
//! framework's `unsafe` code paths under Miri without booting an SVM.
//!
//! ## Why a `pub` module rather than `tests/common/`
//!
//! Integration tests (`tests/*.rs`) are separate binaries; a shared
//! `tests/common/` module compiles into each one and triggers per-binary
//! dead-code warnings for items the particular test doesn't use. Lifting
//! the scaffold to `pub mod testing` lets Rust's per-crate dead-code
//! analysis see the union of consumers.
//!
//! ## Long-term
//!
//! These types mock pinocchio's own definitions (`RuntimeAccount`,
//! `AccountView`, `MAX_PERMITTED_DATA_INCREASE`, …). The right long-term
//! home is `pinocchio::testing` so any pinocchio consumer benefits and
//! drift between mock and real types becomes structurally impossible.
//! Tracked separately; replace this module with a re-export when that
//! lands upstream.

pub mod account_buffer;
#[cfg(not(any(target_os = "solana", target_arch = "bpf")))]
pub mod sbf_input_buffer;

pub use account_buffer::{AccountBuffer, MIN_ACCOUNT_BUF};
#[cfg(not(any(target_os = "solana", target_arch = "bpf")))]
pub use sbf_input_buffer::{AccountRecord, SbfInputBuffer};
