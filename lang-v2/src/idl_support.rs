//! Host-only allocation helpers used by generated IDL code.
//!
//! Keeping this namespace outside `lib.rs` makes it clear that these owned
//! collections are part of the host/IDL boundary, not the on-chain runtime.

#[cfg(not(any(target_os = "solana", target_arch = "bpf")))]
#[doc(hidden)]
pub mod __idl {
    pub use alloc::{boxed, collections, format, string, vec};
}

#[cfg(not(any(target_os = "solana", target_arch = "bpf")))]
#[doc(hidden)]
pub type IdlEntries = alloc::vec::Vec<&'static str>;
