//! Hooks that `Slab<H, T>` requires from its header type `H`.
//!
//! - [`SlabSchema`] — layout + validation (offset, owner, disc, size).
//!   Blanket impl for `Owner + Discriminator`; SPL types override.
//! - [`SlabInit`] — bytes-level init (create + disc write). Blanket impl
//!   for `Owner + Discriminator`; SPL types override with CPI.

use {
    crate::{require, require_eq, Discriminator, Owner},
    pinocchio::account::AccountView,
    solana_program_error::ProgramError,
};

/// Byte-level schema for a type stored inside a `Slab`: where the header
/// sits in the account buffer (`DATA_OFFSET`) and how to verify the bytes
/// at that offset are a valid `Self` (`validate`).
///
/// Types marked with `#[account]` get the blanket impl below (offset 8 +
/// owner/discriminator check). External types (SPL `Mint` / `TokenAccount`)
/// implement this directly with `DATA_OFFSET = 0` and custom validation.
pub trait SlabSchema {
    /// Byte offset where `Self`'s data starts in the account buffer.
    /// - Anchor native types (`#[account]`): 8 (discriminator length)
    /// - External types (SPL `Mint` / `TokenAccount`): 0
    const DATA_OFFSET: usize;

    /// Minimum account data length that can contain `Self` at `DATA_OFFSET`.
    const MIN_DATA_LEN: usize;

    fn validate(view: &AccountView, data: &[u8]) -> Result<(), ProgramError>;
}

impl<T: Owner + Discriminator> SlabSchema for T {
    const DATA_OFFSET: usize = T::DISCRIMINATOR.len();
    const MIN_DATA_LEN: usize = match Self::DATA_OFFSET.checked_add(core::mem::size_of::<T>()) {
        Some(value) => value,
        None => panic!("slab schema minimum length overflow"),
    };

    #[inline(always)]
    fn validate(view: &AccountView, data: &[u8]) -> Result<(), ProgramError> {
        require!(view.owned_by(&T::OWNER), super::slab::cold_owner_error(view));
        let disc = T::DISCRIMINATOR;
        if data.len() < Self::MIN_DATA_LEN {
            return Err(ProgramError::AccountDataTooSmall);
        }
        require_eq!(&data[..disc.len()], disc, ProgramError::InvalidAccountData);
        Ok(())
    }
}

/// Bytes-writing hook Slab invokes during `init`. The default blanket
/// (`T: Owner + Discriminator`) does `create_account` + write disc; SPL
/// `Mint` / `TokenAccount` override with their own token-program CPIs.
///
/// Not needed for self-contained wrappers — those impl
/// [`AccountInitialize`](crate::AccountInitialize) directly.
pub trait SlabInit {
    type Params<'a>: Default;

    fn create_and_initialize<'a>(
        payer: &AccountView,
        account: &AccountView,
        space: usize,
        params: &Self::Params<'a>,
        signer_seeds: Option<&[&[u8]]>,
    ) -> Result<(), ProgramError>;
}

impl<T: Owner + Discriminator> SlabInit for T {
    type Params<'a> = ();

    #[inline(always)]
    fn create_and_initialize<'a>(
        payer: &AccountView,
        account: &AccountView,
        space: usize,
        _params: &(),
        signer_seeds: Option<&[&[u8]]>,
    ) -> Result<(), ProgramError> {
        let disc = T::DISCRIMINATOR;
        match signer_seeds {
            Some(seeds) => crate::create_account_signed(payer, account, space, &T::OWNER, seeds)?,
            None => crate::create_account(payer, account, space, &T::OWNER)?,
        }
        let mut account_view = *account;
        let data = unsafe { account_view.borrow_unchecked_mut() };
        if data.len() < disc.len() {
            return Err(ProgramError::AccountDataTooSmall);
        }
        data[..disc.len()].copy_from_slice(disc);
        Ok(())
    }
}
