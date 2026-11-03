//! Regression tests proving AccountLoader accessors return a structured error
//! instead of panicking when a zero-copy account's body is misaligned.
//!
//! Two layers guard the body alignment:
//! - A compile-time check in `AccountLoader::check_alignment` rejects
//!   account types whose discriminator can never leave the body aligned
//!   (discriminator length not a multiple of the body type's alignment,
//!   for types aligned to eight bytes or less). A one-byte discriminator
//!   with a `u64` body — the case in #5246 — is now a build error, so it
//!   has no runtime test here.
//! - The runtime check still guards misalignment that only shows up at
//!   runtime, e.g. account data that itself starts off an eight-byte
//!   boundary. These tests use a standard eight-byte discriminator and
//!   shift the account data start by one byte instead. The `u128` case
//!   keeps a one-byte discriminator: its sixteen-byte alignment can't be
//!   proven from an eight-byte aligned start either way, so the static
//!   check stays quiet and the runtime check does the work.

use anchor_lang::{accounts::account_loader::AccountLoader, prelude::*};

declare_id!("Fg6PaFpoGXkYsidMpWTK6W2BeZ7FEfcYkg476zPFsLnS");

#[account(zero_copy, discriminator = [1u8, 1u8, 1u8, 1u8, 1u8, 1u8, 1u8, 1u8])]
#[derive(Default, Debug)]
pub struct StandardDiscriminator {
    pub value: u64,
}

#[account(zero_copy, discriminator = [1u8])]
#[derive(Default, Debug)]
pub struct ShortDiscriminatorU128 {
    pub value: u128,
}

/// Backing buffer with the account data start at `start_mod` mod 8, so a
/// one-byte shift leaves the body misaligned even with a standard
/// eight-byte discriminator. Returns the buffer and the offset of the
/// account data within it.
fn backing_with_start_mod(disc: &[u8], body_len: usize, start_mod: usize) -> (Vec<u8>, usize) {
    let mut backing = vec![0u8; 8 + disc.len() + body_len];
    let base = backing.as_ptr() as usize;
    let off = (start_mod + 8 - base % 8) % 8;
    backing[off..off + disc.len()].copy_from_slice(disc);
    (backing, off)
}

macro_rules! setup_misaligned_account {
    ($ty:ty, $disc:expr, $start_mod:expr, $key:ident, $owner:ident, $lamports:ident, $backing:ident, $data:ident, $account_info:ident) => {
        let $key = Pubkey::new_unique();
        let $owner = crate::ID;
        let mut $lamports = 0;
        let (mut $backing, off) =
            backing_with_start_mod(&$disc, std::mem::size_of::<$ty>(), $start_mod);
        let $data = &mut $backing[off..off + $disc.len() + std::mem::size_of::<$ty>()];
        #[allow(unused_variables)]
        let $account_info =
            AccountInfo::new(&$key, false, true, &mut $lamports, $data, &$owner, false);
    };
}

#[test]
fn test_load_misaligned() {
    setup_misaligned_account!(
        StandardDiscriminator,
        [1u8; 8],
        1,
        key,
        owner,
        lamports,
        backing,
        data,
        account_info
    );
    let loader: AccountLoader<StandardDiscriminator> =
        AccountLoader::try_from_unchecked(&crate::ID, &account_info).unwrap();
    assert_eq!(
        loader.load().unwrap_err(),
        ErrorCode::AccountDidNotDeserialize.into()
    );
}

#[test]
fn test_load_mut_misaligned() {
    setup_misaligned_account!(
        StandardDiscriminator,
        [1u8; 8],
        1,
        key,
        owner,
        lamports,
        backing,
        data,
        account_info
    );
    let loader: AccountLoader<StandardDiscriminator> =
        AccountLoader::try_from_unchecked(&crate::ID, &account_info).unwrap();
    assert_eq!(
        loader.load_mut().unwrap_err(),
        ErrorCode::AccountDidNotDeserialize.into()
    );
}

#[test]
fn test_load_init_misaligned() {
    // Initializing, so the discriminator bytes are still zero.
    setup_misaligned_account!(
        StandardDiscriminator,
        [0u8; 8],
        1,
        key,
        owner,
        lamports,
        backing,
        data,
        account_info
    );
    let loader: AccountLoader<StandardDiscriminator> =
        AccountLoader::try_from_unchecked(&crate::ID, &account_info).unwrap();
    assert_eq!(
        loader.load_init().unwrap_err(),
        ErrorCode::AccountDidNotDeserialize.into()
    );
}

#[test]
fn test_load_misaligned_u128() {
    setup_misaligned_account!(
        ShortDiscriminatorU128,
        [1u8],
        0,
        key,
        owner,
        lamports,
        backing,
        data,
        account_info
    );
    let loader: AccountLoader<ShortDiscriminatorU128> =
        AccountLoader::try_from_unchecked(&crate::ID, &account_info).unwrap();
    assert_eq!(
        loader.load().unwrap_err(),
        ErrorCode::AccountDidNotDeserialize.into()
    );
}

#[test]
fn test_load_aligned() {
    // Control: same eight-byte discriminator with the account data start on
    // an eight-byte boundary, so the body is aligned and loading works.
    setup_misaligned_account!(
        StandardDiscriminator,
        [1u8; 8],
        0,
        key,
        owner,
        lamports,
        backing,
        data,
        account_info
    );
    let loader: AccountLoader<StandardDiscriminator> =
        AccountLoader::try_from_unchecked(&crate::ID, &account_info).unwrap();
    assert!(loader.load().is_ok());
}
