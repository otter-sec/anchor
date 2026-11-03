//! Token-2022 extension data types and TLV parsing.
//!
//! Provides zero-copy access to extension data stored in Token-2022 accounts.
//! The TLV (Type-Length-Value) format is parsed by `spl-token-2022-interface`
//! so Anchor v2 keeps the same account-type and extension-length semantics as
//! SPL Token-2022.
//!
//! # Usage
//!
//! ```ignore
//! use anchor_spl::{
//!     extensions::{MetadataPointer, TransferFeeConfig},
//!     token_interface::TokenInterfaceAccountExtensions,
//! };
//!
//! // Read extension from an InterfaceAccount<Mint>
//! let fee_config: &TransferFeeConfig = mint.get_extension()?;
//! let metadata_ptr: &MetadataPointer = mint.get_extension()?;
//! ```

use {
    bytemuck::{Pod, Zeroable},
    solana_address::Address,
};

// ---------------------------------------------------------------------------
// TLV parser adapter
// ---------------------------------------------------------------------------

/// Trait for fixed-size Token-2022 extension types.
pub trait ExtensionType: Pod {
    /// Token-2022 wire tag for this extension.
    #[doc(hidden)]
    const TYPE: u16;

    /// Whether this extension belongs to a mint rather than a token account.
    #[doc(hidden)]
    const IS_MINT_EXTENSION: bool;
}

macro_rules! extension_type {
    ($type:ty, $tag:expr, mint) => {
        impl ExtensionType for $type {
            const TYPE: u16 = $tag;
            const IS_MINT_EXTENSION: bool = true;
        }
    };
    ($type:ty, $tag:expr, account) => {
        impl ExtensionType for $type {
            const TYPE: u16 = $tag;
            const IS_MINT_EXTENSION: bool = false;
        }
    };
}

// ---------------------------------------------------------------------------
// Extension struct definitions
//
// Layouts match spl-token-2022-interface exactly. All fields are alignment-1
// (Pod-safe). OptionalNonZeroPubkey is represented as Address — all-zeros
// means None.
// ---------------------------------------------------------------------------

/// Optional authority/address field. All-zeros represents "not set".
pub type OptionalAddress = Address;

/// Check if an optional address is set (non-zero).
#[inline(always)]
pub fn is_some_address(addr: &Address) -> bool {
    let ptr = addr.as_ref().as_ptr();
    // SAFETY: Address is exactly 32 initialized bytes. `read_unaligned`
    // permits reading u64 words from the byte-aligned address buffer.
    unsafe {
        (ptr as *const u64).read_unaligned()
            | (ptr.add(8) as *const u64).read_unaligned()
            | (ptr.add(16) as *const u64).read_unaligned()
            | (ptr.add(24) as *const u64).read_unaligned()
            != 0
    }
}

/// Return the address as an Option — None if all zeros.
#[inline(always)]
pub fn optional_address(addr: &Address) -> Option<&Address> {
    if is_some_address(addr) {
        Some(addr)
    } else {
        None
    }
}

// ---------------------------------------------------------------------------
// TransferFeeConfig (ExtensionType = 1, mint extension)
// ---------------------------------------------------------------------------

/// Per-epoch transfer fee parameters.
#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
pub struct TransferFee {
    /// First epoch where this fee takes effect.
    pub epoch: [u8; 8],
    /// Maximum fee assessed on transfers (token amount).
    pub maximum_fee: [u8; 8],
    /// Fee in basis points of the transfer amount (0.01% increments).
    pub transfer_fee_basis_points: [u8; 2],
}

impl TransferFee {
    pub fn epoch(&self) -> u64 {
        u64::from_le_bytes(self.epoch)
    }
    pub fn maximum_fee(&self) -> u64 {
        u64::from_le_bytes(self.maximum_fee)
    }
    pub fn basis_points(&self) -> u16 {
        u16::from_le_bytes(self.transfer_fee_basis_points)
    }
}

/// Transfer fee configuration on the mint.
#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
pub struct TransferFeeConfig {
    pub transfer_fee_config_authority: OptionalAddress,
    pub withdraw_withheld_authority: OptionalAddress,
    pub withheld_amount: [u8; 8],
    pub older_transfer_fee: TransferFee,
    pub newer_transfer_fee: TransferFee,
}

impl TransferFeeConfig {
    pub fn withheld_amount(&self) -> u64 {
        u64::from_le_bytes(self.withheld_amount)
    }
}

extension_type!(TransferFeeConfig, 1, mint);

// ---------------------------------------------------------------------------
// TransferFeeAmount (ExtensionType = 2, account extension)
// ---------------------------------------------------------------------------

/// Withheld fee on a token account.
#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
pub struct TransferFeeAmount {
    pub withheld_amount: [u8; 8],
}

impl TransferFeeAmount {
    pub fn withheld_amount(&self) -> u64 {
        u64::from_le_bytes(self.withheld_amount)
    }
}

extension_type!(TransferFeeAmount, 2, account);

// ---------------------------------------------------------------------------
// MintCloseAuthority (ExtensionType = 3)
// ---------------------------------------------------------------------------

/// Authority that can close the mint.
#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
pub struct MintCloseAuthority {
    pub close_authority: OptionalAddress,
}

extension_type!(MintCloseAuthority, 3, mint);

// ---------------------------------------------------------------------------
// DefaultAccountState (ExtensionType = 6)
// ---------------------------------------------------------------------------

/// Default state for new token accounts created from this mint.
#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
pub struct DefaultAccountState {
    /// 1 = Initialized, 2 = Frozen
    pub state: u8,
}

extension_type!(DefaultAccountState, 6, mint);

// ---------------------------------------------------------------------------
// NonTransferable (ExtensionType = 9, mint extension)
// ---------------------------------------------------------------------------

/// Marker extension for non-transferable mints.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct NonTransferable;

unsafe impl Pod for NonTransferable {}
unsafe impl Zeroable for NonTransferable {}

extension_type!(NonTransferable, 9, mint);

// ---------------------------------------------------------------------------
// CpiGuard (ExtensionType = 11, account extension)
// ---------------------------------------------------------------------------

/// CPI guard state on a token account.
#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
pub struct CpiGuard {
    /// Non-zero when privileged token operations are blocked through CPI.
    pub lock_cpi: u8,
}

impl CpiGuard {
    pub fn is_enabled(&self) -> bool {
        self.lock_cpi != 0
    }
}

extension_type!(CpiGuard, 11, account);

// ---------------------------------------------------------------------------
// PermanentDelegate (ExtensionType = 12)
// ---------------------------------------------------------------------------

/// Permanent delegate for transferring or burning tokens from any account.
#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
pub struct PermanentDelegate {
    pub delegate: OptionalAddress,
}

extension_type!(PermanentDelegate, 12, mint);

// ---------------------------------------------------------------------------
// TransferHook (ExtensionType = 14, mint extension)
// ---------------------------------------------------------------------------

/// Transfer hook configuration on the mint.
#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
pub struct TransferHook {
    pub authority: OptionalAddress,
    pub program_id: OptionalAddress,
}

extension_type!(TransferHook, 14, mint);

// ---------------------------------------------------------------------------
// TransferHookAccount (ExtensionType = 15, account extension)
// ---------------------------------------------------------------------------

/// Transfer hook state on a token account.
#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
pub struct TransferHookAccount {
    /// Whether the account is currently being transferred.
    pub transferring: u8,
}

extension_type!(TransferHookAccount, 15, account);

// ---------------------------------------------------------------------------
// NonTransferableAccount (ExtensionType = 13, account extension)
// ---------------------------------------------------------------------------

/// Marker extension for accounts that belong to a non-transferable mint.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct NonTransferableAccount;

unsafe impl Pod for NonTransferableAccount {}
unsafe impl Zeroable for NonTransferableAccount {}

extension_type!(NonTransferableAccount, 13, account);

// ---------------------------------------------------------------------------
// MetadataPointer (ExtensionType = 18, mint extension)
// ---------------------------------------------------------------------------

/// Points to the account holding token metadata.
#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
pub struct MetadataPointer {
    pub authority: OptionalAddress,
    pub metadata_address: OptionalAddress,
}

extension_type!(MetadataPointer, 18, mint);

// ---------------------------------------------------------------------------
// GroupPointer (ExtensionType = 20, mint extension)
// ---------------------------------------------------------------------------

/// Points to the account holding group configuration.
#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
pub struct GroupPointer {
    pub authority: OptionalAddress,
    pub group_address: OptionalAddress,
}

extension_type!(GroupPointer, 20, mint);

// ---------------------------------------------------------------------------
// GroupMemberPointer (ExtensionType = 22, mint extension)
// ---------------------------------------------------------------------------

/// Points to the account holding group member configuration.
#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
pub struct GroupMemberPointer {
    pub authority: OptionalAddress,
    pub member_address: OptionalAddress,
}

extension_type!(GroupMemberPointer, 22, mint);

// ---------------------------------------------------------------------------
// PausableConfig (ExtensionType = 26, mint extension)
// ---------------------------------------------------------------------------

/// Pausable mint configuration.
#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
pub struct PausableConfig {
    pub authority: OptionalAddress,
    /// Non-zero when minting, burning, and transfers are paused.
    pub paused: u8,
}

impl PausableConfig {
    pub fn is_paused(&self) -> bool {
        self.paused != 0
    }
}

extension_type!(PausableConfig, 26, mint);

// ---------------------------------------------------------------------------
// PausableAccount (ExtensionType = 27, account extension)
// ---------------------------------------------------------------------------

/// Marker extension for accounts that belong to a pausable mint.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct PausableAccount;

unsafe impl Pod for PausableAccount {}
unsafe impl Zeroable for PausableAccount {}

extension_type!(PausableAccount, 27, account);

#[cfg(test)]
mod tests {
    use {super::*, spl_token_2022_interface::extension::ExtensionType as CanonicalExtensionType};

    fn assert_extension<T: ExtensionType>(canonical: CanonicalExtensionType, is_mint: bool) {
        assert_eq!(T::TYPE, canonical as u16);
        assert_eq!(T::IS_MINT_EXTENSION, is_mint);
    }

    #[test]
    fn local_extension_tags_match_token_2022_wire_values() {
        assert_extension::<TransferFeeConfig>(CanonicalExtensionType::TransferFeeConfig, true);
        assert_extension::<TransferFeeAmount>(CanonicalExtensionType::TransferFeeAmount, false);
        assert_extension::<MintCloseAuthority>(CanonicalExtensionType::MintCloseAuthority, true);
        assert_extension::<DefaultAccountState>(CanonicalExtensionType::DefaultAccountState, true);
        assert_extension::<NonTransferable>(CanonicalExtensionType::NonTransferable, true);
        assert_extension::<CpiGuard>(CanonicalExtensionType::CpiGuard, false);
        assert_extension::<PermanentDelegate>(CanonicalExtensionType::PermanentDelegate, true);
        assert_extension::<NonTransferableAccount>(
            CanonicalExtensionType::NonTransferableAccount,
            false,
        );
        assert_extension::<TransferHook>(CanonicalExtensionType::TransferHook, true);
        assert_extension::<TransferHookAccount>(CanonicalExtensionType::TransferHookAccount, false);
        assert_extension::<MetadataPointer>(CanonicalExtensionType::MetadataPointer, true);
        assert_extension::<GroupPointer>(CanonicalExtensionType::GroupPointer, true);
        assert_extension::<GroupMemberPointer>(CanonicalExtensionType::GroupMemberPointer, true);
        assert_extension::<PausableConfig>(CanonicalExtensionType::Pausable, true);
        assert_extension::<PausableAccount>(CanonicalExtensionType::PausableAccount, false);
    }
}

// ---------------------------------------------------------------------------
// Constraint markers for `#[account(extensions::*)]`
// -------------------------------------------
pub struct MetadataPointerAuthorityConstraint;
pub struct MetadataPointerMetadataAddressConstraint;
pub struct GroupPointerAuthorityConstraint;
pub struct GroupPointerGroupAddressConstraint;
pub struct GroupMemberPointerAuthorityConstraint;
pub struct GroupMemberPointerMemberAddressConstraint;
pub struct CloseAuthorityAuthorityConstraint;
pub struct TransferHookAuthorityConstraint;
pub struct TransferHookProgramIdConstraint;
pub struct PermanentDelegateDelegateConstraint;
