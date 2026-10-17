//! Borrowed, allocator-neutral account metadata used by V2 account builders.

use crate::Address;

/// Metadata for one account in a V2 instruction.
///
/// This is intentionally owned by `anchor-lang` rather than re-exported from
/// `solana-instruction`. The SDK's `AccountMeta` is a host-side construction
/// type backed by an allocation-heavy `Instruction`; exposing it here would
/// make the V2 trait surface depend on `alloc`.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
#[repr(C)]
pub struct AccountMeta {
    pub pubkey: Address,
    pub is_signer: bool,
    pub is_writable: bool,
}

impl AccountMeta {
    #[inline]
    pub const fn new(pubkey: Address, is_signer: bool) -> Self {
        Self {
            pubkey,
            is_signer,
            is_writable: true,
        }
    }

    #[inline]
    pub const fn new_readonly(pubkey: Address, is_signer: bool) -> Self {
        Self {
            pubkey,
            is_signer,
            is_writable: false,
        }
    }
}
