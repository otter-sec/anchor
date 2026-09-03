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
    spl_token_2022_interface::extension::{
        Extension as SplExtension, ExtensionType as SplExtensionType,
    },
};

// ---------------------------------------------------------------------------
// TLV parser adapter
// ---------------------------------------------------------------------------

/// Trait for fixed-size Token-2022 extension types.
pub trait ExtensionType: Pod + SplExtension {}

impl<T> ExtensionType for T where T: Pod + SplExtension {}

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

impl SplExtension for TransferFeeConfig {
    const TYPE: SplExtensionType = SplExtensionType::TransferFeeConfig;
}

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

impl SplExtension for TransferFeeAmount {
    const TYPE: SplExtensionType = SplExtensionType::TransferFeeAmount;
}

// ---------------------------------------------------------------------------
// MintCloseAuthority (ExtensionType = 3)
// ---------------------------------------------------------------------------

/// Authority that can close the mint.
#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
pub struct MintCloseAuthority {
    pub close_authority: OptionalAddress,
}

impl SplExtension for MintCloseAuthority {
    const TYPE: SplExtensionType = SplExtensionType::MintCloseAuthority;
}

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

impl SplExtension for DefaultAccountState {
    const TYPE: SplExtensionType = SplExtensionType::DefaultAccountState;
}

// ---------------------------------------------------------------------------
// NonTransferable (ExtensionType = 9, mint extension)
// ---------------------------------------------------------------------------

/// Marker extension for non-transferable mints.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct NonTransferable;

unsafe impl Pod for NonTransferable {}
unsafe impl Zeroable for NonTransferable {}

impl SplExtension for NonTransferable {
    const TYPE: SplExtensionType = SplExtensionType::NonTransferable;
}

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

impl SplExtension for CpiGuard {
    const TYPE: SplExtensionType = SplExtensionType::CpiGuard;
}

// ---------------------------------------------------------------------------
// PermanentDelegate (ExtensionType = 12)
// ---------------------------------------------------------------------------

/// Permanent delegate for transferring or burning tokens from any account.
#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
pub struct PermanentDelegate {
    pub delegate: OptionalAddress,
}

impl SplExtension for PermanentDelegate {
    const TYPE: SplExtensionType = SplExtensionType::PermanentDelegate;
}

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

impl SplExtension for TransferHook {
    const TYPE: SplExtensionType = SplExtensionType::TransferHook;
}

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

impl SplExtension for TransferHookAccount {
    const TYPE: SplExtensionType = SplExtensionType::TransferHookAccount;
}

// ---------------------------------------------------------------------------
// NonTransferableAccount (ExtensionType = 13, account extension)
// ---------------------------------------------------------------------------

/// Marker extension for accounts that belong to a non-transferable mint.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct NonTransferableAccount;

unsafe impl Pod for NonTransferableAccount {}
unsafe impl Zeroable for NonTransferableAccount {}

impl SplExtension for NonTransferableAccount {
    const TYPE: SplExtensionType = SplExtensionType::NonTransferableAccount;
}

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

impl SplExtension for MetadataPointer {
    const TYPE: SplExtensionType = SplExtensionType::MetadataPointer;
}

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

impl SplExtension for GroupPointer {
    const TYPE: SplExtensionType = SplExtensionType::GroupPointer;
}

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

impl SplExtension for GroupMemberPointer {
    const TYPE: SplExtensionType = SplExtensionType::GroupMemberPointer;
}

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

impl SplExtension for PausableConfig {
    const TYPE: SplExtensionType = SplExtensionType::Pausable;
}

// ---------------------------------------------------------------------------
// PausableAccount (ExtensionType = 27, account extension)
// ---------------------------------------------------------------------------

/// Marker extension for accounts that belong to a pausable mint.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct PausableAccount;

unsafe impl Pod for PausableAccount {}
unsafe impl Zeroable for PausableAccount {}

impl SplExtension for PausableAccount {
    const TYPE: SplExtensionType = SplExtensionType::PausableAccount;
}
