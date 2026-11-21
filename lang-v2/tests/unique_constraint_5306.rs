//! `#[account(mut, unique)]` — opt-in duplicate rejection (anchor #5306).
//!
//! v2 dropped the derive-level duplicate check (#5091), so wrappers that
//! never take a borrow (`Signer`, `SystemAccount`, `UncheckedAccount`,
//! `Program<T>`) can alias a `mut` slot with no error at all. jamie-osec
//! (2026-11-18) called the automatic blocking of all duplicates a design
//! mistake, but is not opposed to an opt-in `unique` constraint as long as
//! it can be implemented performantly — so this stays opt-in, compares
//! addresses only, and costs nothing for fields without the attribute.
//!
//! An `Option<T>` sibling set to `None` (its view address is the program
//! id) must not read as a collision — two `None`s share the sentinel.

#![allow(dead_code, deprecated, unexpected_cfgs)]

use {
    anchor_lang::{
        accounts::{Signer, UncheckedAccount},
        testing::AccountBuffer,
        Accounts, AnchorAccount, ErrorCode, TryAccounts,
    },
    pinocchio::address::Address,
    solana_program_error::ProgramError,
};

anchor_lang::declare_id!("11111111111111111111111111111111");

const PROGRAM_ID: [u8; 32] = [0x42; 32];
const ALICE: [u8; 32] = [0xAA; 32];
const BOB: [u8; 32] = [0xBB; 32];

#[derive(Accounts)]
struct TwoSigners {
    #[account(mut, unique)]
    first: Signer,
    second: Signer,
}

#[derive(Accounts)]
struct UncheckedPlusSigner {
    #[account(mut, unique)]
    vault: UncheckedAccount,
    authority: Signer,
}

#[derive(Accounts)]
struct UniqueWithOptionalSibling {
    #[account(mut, unique)]
    first: Signer,
    second: Option<Signer>,
}

fn expect_err<T>(result: Result<T, ProgramError>) -> ProgramError {
    match result {
        Ok(_) => panic!("expected Err, got Ok"),
        Err(err) => err,
    }
}

fn signer_buf(address: [u8; 32]) -> AccountBuffer<128> {
    let buf = AccountBuffer::<128>::new();
    buf.init(address, PROGRAM_ID, 0, true, true, false);
    buf
}

fn unchecked_buf(address: [u8; 32]) -> AccountBuffer<128> {
    let buf = AccountBuffer::<128>::new();
    buf.init(address, PROGRAM_ID, 0, false, true, false);
    buf
}

#[test]
fn unique_signer_rejects_same_address() {
    let first = signer_buf(ALICE);
    let second = signer_buf(ALICE);
    let views = [unsafe { first.view() }, unsafe { second.view() }];

    let err = expect_err(TwoSigners::try_accounts(
        &Address::new_from_array(PROGRAM_ID),
        &views,
        &[],
    ));
    assert_eq!(err, ErrorCode::ConstraintUnique.into());
    // Same violation, decoded as the v1 number it reuses.
    assert_eq!(err, ProgramError::Custom(2040));
}

#[test]
fn unique_signer_passes_distinct_addresses() {
    let first = signer_buf(ALICE);
    let second = signer_buf(BOB);
    let views = [unsafe { first.view() }, unsafe { second.view() }];

    TwoSigners::try_accounts(&Address::new_from_array(PROGRAM_ID), &views, &[])
        .expect("distinct addresses must pass the unique check");
}

#[test]
fn unique_unchecked_alias_rejected() {
    // The reporter's shape: a `mut` unchecked account aliased by a signer.
    let vault = unchecked_buf(ALICE);
    let authority = signer_buf(ALICE);
    let views = [unsafe { vault.view() }, unsafe { authority.view() }];

    let err = expect_err(UncheckedPlusSigner::try_accounts(
        &Address::new_from_array(PROGRAM_ID),
        &views,
        &[],
    ));
    assert_eq!(err, ErrorCode::ConstraintUnique.into());
}

#[test]
fn unique_unchecked_passes_distinct_addresses() {
    let vault = unchecked_buf(ALICE);
    let authority = signer_buf(BOB);
    let views = [unsafe { vault.view() }, unsafe { authority.view() }];

    UncheckedPlusSigner::try_accounts(&Address::new_from_array(PROGRAM_ID), &views, &[])
        .expect("distinct addresses must pass the unique check");
}

#[test]
fn unique_optional_sibling_none_is_skipped() {
    // A `None` sibling's view address is the program id — the sentinel
    // must not count as an address to collide with.
    let first = signer_buf(ALICE);
    let second = signer_buf(PROGRAM_ID);
    let views = [unsafe { first.view() }, unsafe { second.view() }];

    UniqueWithOptionalSibling::try_accounts(&Address::new_from_array(PROGRAM_ID), &views, &[])
        .expect("a None sibling must not trip the unique check");
}

#[test]
fn unique_optional_sibling_some_alias_rejected() {
    let first = signer_buf(ALICE);
    let second = signer_buf(ALICE);
    let views = [unsafe { first.view() }, unsafe { second.view() }];

    let err = expect_err(UniqueWithOptionalSibling::try_accounts(
        &Address::new_from_array(PROGRAM_ID),
        &views,
        &[],
    ));
    assert_eq!(err, ErrorCode::ConstraintUnique.into());
}
