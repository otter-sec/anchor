//! Allocator-neutral account metadata used by V2 account builders.

#[cfg(any(target_os = "solana", target_arch = "bpf"))]
use crate::{Address, FixedVec};
#[cfg(not(any(target_os = "solana", target_arch = "bpf")))]
pub use solana_instruction::AccountMeta;

/// Metadata for one account in a V2 instruction.
///
/// On-chain this is an Anchor-owned, allocator-neutral value. Host builds use
/// the canonical SDK type so existing client constructors remain ergonomic.
#[cfg(any(target_os = "solana", target_arch = "bpf"))]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
#[repr(C)]
pub struct AccountMeta {
    pub pubkey: Address,
    pub is_signer: bool,
    pub is_writable: bool,
}

#[cfg(any(target_os = "solana", target_arch = "bpf"))]
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

/// Converts account addresses into a fixed-capacity metadata buffer.
///
/// The capacity is selected by each generated account type, so this public
/// API does not require an allocator or impose a global account limit.
#[cfg(not(any(target_os = "solana", target_arch = "bpf")))]
pub trait ToAccountMetas {
    fn to_account_metas(&self, is_signer: Option<bool>) -> alloc::vec::Vec<AccountMeta>;
}

#[cfg(any(target_os = "solana", target_arch = "bpf"))]
pub trait ToAccountMetas {
    type Output: AsRef<[AccountMeta]>;

    fn to_account_metas(&self, is_signer: Option<bool>) -> Self::Output;
}

#[cfg(any(target_os = "solana", target_arch = "bpf"))]
impl<const N: usize> ToAccountMetas for FixedVec<AccountMeta, N> {
    type Output = FixedVec<AccountMeta, N>;

    fn to_account_metas(&self, is_signer: Option<bool>) -> Self::Output {
        let mut output = FixedVec::new();
        for meta in self {
            let mut meta = meta.clone();
            if let Some(is_signer) = is_signer {
                meta.is_signer = is_signer;
            }
            output.push(meta);
        }
        output
    }
}

#[cfg(any(target_os = "solana", target_arch = "bpf"))]
impl<const N: usize> ToAccountMetas for [AccountMeta; N] {
    type Output = FixedVec<AccountMeta, N>;

    fn to_account_metas(&self, is_signer: Option<bool>) -> Self::Output {
        let mut output = FixedVec::new();
        for meta in self {
            let mut meta = meta.clone();
            if let Some(is_signer) = is_signer {
                meta.is_signer = is_signer;
            }
            output.push(meta);
        }
        output
    }
}
