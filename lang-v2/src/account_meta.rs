//! Allocator-neutral account metadata used by V2 account builders.

#[cfg(any(target_os = "solana", target_arch = "bpf"))]
use crate::Address;
use crate::Vec;
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

/// Converts account addresses into a metadata buffer.
pub trait ToAccountMetas {
    fn to_account_metas(&self, is_signer: Option<bool>) -> Vec<AccountMeta>;
}

impl ToAccountMetas for [AccountMeta] {
    fn to_account_metas(&self, is_signer: Option<bool>) -> Vec<AccountMeta> {
        self.iter()
            .cloned()
            .map(|mut meta| {
                if let Some(is_signer) = is_signer {
                    meta.is_signer = is_signer;
                }
                meta
            })
            .collect()
    }
}

impl ToAccountMetas for Vec<AccountMeta> {
    fn to_account_metas(&self, is_signer: Option<bool>) -> Vec<AccountMeta> {
        self.as_slice().to_account_metas(is_signer)
    }
}

#[cfg(not(any(target_os = "solana", target_arch = "bpf")))]
impl ToAccountMetas for alloc::vec::Vec<AccountMeta> {
    fn to_account_metas(&self, is_signer: Option<bool>) -> Vec<AccountMeta> {
        self.as_slice().to_account_metas(is_signer)
    }
}

impl<const N: usize> ToAccountMetas for [AccountMeta; N] {
    fn to_account_metas(&self, is_signer: Option<bool>) -> Vec<AccountMeta> {
        self.as_slice().to_account_metas(is_signer)
    }
}
