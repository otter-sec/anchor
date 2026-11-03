//! Regression tests proving AccountLoader accessors return a structured error
//! instead of panicking when a zero-copy account's body is misaligned.
//!
//! A one-byte custom discriminator (e.g. `[1]`) shifts the body off the
//! eight-byte-aligned account data start, so `bytemuck::from_bytes` would
//! panic on any field that needs more than one-byte alignment.

use anchor_lang::{accounts::account_loader::AccountLoader, prelude::*};

declare_id!("Fg6PaFpoGXkYsidMpWTK6W2BeZ7FEfcYkg476zPFsLnS");

#[account(zero_copy, discriminator = [1u8])]
#[derive(Default, Debug)]
pub struct ShortDiscriminator {
    pub value: u64,
}

#[account(zero_copy, discriminator = [1u8])]
#[derive(Default, Debug)]
pub struct ShortDiscriminatorU128 {
    pub value: u128,
}

/// Backing buffer with the account data start eight-byte aligned, like SBF
/// account data, so a one-byte discriminator leaves the body misaligned.
/// Returns the buffer and the offset of the account data within it.
fn aligned_backing(disc: &[u8], body_len: usize) -> (Vec<u8>, usize) {
    let mut backing = vec![0u8; 8 + disc.len() + body_len];
    let base = backing.as_ptr() as usize;
    let off = (8 - base % 8) % 8;
    backing[off..off + disc.len()].copy_from_slice(disc);
    (backing, off)
}

macro_rules! setup_misaligned_account {
    ($ty:ty, $disc:expr, $key:ident, $owner:ident, $lamports:ident, $backing:ident, $data:ident, $account_info:ident) => {
        let $key = Pubkey::new_unique();
        let $owner = crate::ID;
        let mut $lamports = 0;
        let (mut $backing, off) = aligned_backing(&$disc, std::mem::size_of::<$ty>());
        let $data = &mut $backing[off..off + $disc.len() + std::mem::size_of::<$ty>()];
        #[allow(unused_variables)]
        let $account_info =
            AccountInfo::new(&$key, false, true, &mut $lamports, $data, &$owner, false);
    };
}

#[test]
fn test_load_misaligned() {
    setup_misaligned_account!(
        ShortDiscriminator,
        [1u8],
        key,
        owner,
        lamports,
        backing,
        data,
        account_info
    );
    let loader: AccountLoader<ShortDiscriminator> =
        AccountLoader::try_from_unchecked(&crate::ID, &account_info).unwrap();
    assert_eq!(
        loader.load().unwrap_err(),
        ErrorCode::AccountDidNotDeserialize.into()
    );
}

#[test]
fn test_load_mut_misaligned() {
    setup_misaligned_account!(
        ShortDiscriminator,
        [1u8],
        key,
        owner,
        lamports,
        backing,
        data,
        account_info
    );
    let loader: AccountLoader<ShortDiscriminator> =
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
        ShortDiscriminator,
        [0u8],
        key,
        owner,
        lamports,
        backing,
        data,
        account_info
    );
    let loader: AccountLoader<ShortDiscriminator> =
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
fn test_load_aligned_short_discriminator() {
    // Control: same one-byte discriminator, but the account data starts one
    // byte before an eight-byte boundary, so the body itself is aligned and
    // loading still works.
    let key = Pubkey::new_unique();
    let owner = crate::ID;
    let mut lamports = 0;
    let mut backing = vec![0u8; 8 + 1 + std::mem::size_of::<ShortDiscriminator>()];
    let base = backing.as_ptr() as usize;
    let off = (8 - (base + 1) % 8) % 8;
    backing[off] = 1;
    let data = &mut backing[off..off + 1 + std::mem::size_of::<ShortDiscriminator>()];
    let account_info = AccountInfo::new(&key, false, true, &mut lamports, data, &owner, false);
    let loader: AccountLoader<ShortDiscriminator> =
        AccountLoader::try_from_unchecked(&crate::ID, &account_info).unwrap();
    assert!(loader.load().is_ok());
}
