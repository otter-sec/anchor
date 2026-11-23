#![allow(dead_code)]

//! Integration tests for the opt-in `unique` account constraint.
//!
//! Run: `cargo test -p anchor-lang --features testing --test unique_constraint`

use {
    anchor_lang::{prelude::*, testing::AccountBuffer},
    pinocchio::address::Address,
    solana_program_error::ProgramError,
    std::result::Result,
};

declare_id!("11111111111111111111111111111111");

const PROGRAM_ID: [u8; 32] = [0x42; 32];

fn unchecked(address: [u8; 32]) -> AccountBuffer<128> {
    let buf = AccountBuffer::<128>::new();
    buf.init(address, [0x99; 32], 0, false, false, false);
    buf
}

fn expect_err<T>(result: Result<T, ProgramError>) -> ProgramError {
    match result {
        Ok(_) => panic!("expected Err, got Ok"),
        Err(err) => err,
    }
}

#[derive(Accounts)]
struct UniqueUnchecked {
    #[account(unique)]
    first: UncheckedAccount,
    second: UncheckedAccount,
}

#[derive(Accounts)]
struct NotUniqueUnchecked {
    first: UncheckedAccount,
    second: UncheckedAccount,
}

#[derive(Accounts)]
struct OptionalUnique {
    #[account(unique)]
    maybe: Option<UncheckedAccount>,
    other: UncheckedAccount,
}

#[derive(Accounts)]
struct Inner {
    shared: UncheckedAccount,
}

#[derive(Accounts)]
struct OuterUnique {
    #[account(unique)]
    extra: UncheckedAccount,
    inner: Nested<Inner>,
}

#[derive(Accounts)]
struct UniqueProgramId {
    #[account(unique)]
    first: UncheckedAccount,
    second: UncheckedAccount,
}

#[derive(Accounts)]
struct UniqueProgramIdWithOptionalNone {
    #[account(unique)]
    program_account: UncheckedAccount,
    maybe: Option<UncheckedAccount>,
}

#[derive(Accounts)]
struct UniqueGeneratedNames {
    #[account(unique)]
    __unique_target: UncheckedAccount,
    __unique_slot: UncheckedAccount,
    __unique_other: UncheckedAccount,
}

#[derive(Accounts)]
struct TwoUniqueFields {
    #[account(unique)]
    first: UncheckedAccount,
    #[account(unique)]
    second: UncheckedAccount,
}

#[test]
fn unique_rejects_duplicate_unchecked() {
    let buf = unchecked([0xAB; 32]);
    let view = unsafe { buf.view() };
    let views = [view, view];
    let err = expect_err(UniqueUnchecked::try_accounts(
        &Address::new_from_array(PROGRAM_ID),
        &views,
        &[],
    ));
    assert_eq!(err, ProgramError::Custom(2043));
}

#[test]
fn missing_unique_allows_duplicate_unchecked() {
    let buf = unchecked([0xAB; 32]);
    let view = unsafe { buf.view() };
    let views = [view, view];
    let result =
        NotUniqueUnchecked::try_accounts(&Address::new_from_array(PROGRAM_ID), &views, &[]);
    assert!(result.is_ok());
}

#[test]
fn unique_skips_optional_none_sentinel() {
    let none_buf = unchecked(PROGRAM_ID);
    let other_buf = unchecked([0xCD; 32]);
    let none_view = unsafe { none_buf.view() };
    let other_view = unsafe { other_buf.view() };
    let views = [none_view, other_view];
    let result = OptionalUnique::try_accounts(&Address::new_from_array(PROGRAM_ID), &views, &[]);
    assert!(result.is_ok());
}

#[test]
fn unique_rejects_optional_some_duplicate() {
    let buf = unchecked([0xEF; 32]);
    let view = unsafe { buf.view() };
    let views = [view, view];
    let err = expect_err(OptionalUnique::try_accounts(
        &Address::new_from_array(PROGRAM_ID),
        &views,
        &[],
    ));
    assert_eq!(err, ErrorCode::ConstraintUnique.into());
}

#[test]
fn unique_catches_duplicate_inside_nested() {
    let buf = unchecked([0x11; 32]);
    let view = unsafe { buf.view() };
    let views = [view, view];
    let err = expect_err(OuterUnique::try_accounts(
        &Address::new_from_array(PROGRAM_ID),
        &views,
        &[],
    ));
    assert_eq!(err, ErrorCode::ConstraintUnique.into());
}

#[test]
fn unique_rejects_duplicate_real_program_id_accounts() {
    let first = unchecked(PROGRAM_ID);
    let second = unchecked(PROGRAM_ID);
    let views = [unsafe { first.view() }, unsafe { second.view() }];

    let err = expect_err(UniqueProgramId::try_accounts(
        &Address::new_from_array(PROGRAM_ID),
        &views,
        &[],
    ));
    assert_eq!(err, ErrorCode::ConstraintUnique.into());
}

#[test]
fn unique_skips_optional_none_beside_real_program_id_account() {
    let program_account = unchecked(PROGRAM_ID);
    let none_sentinel = unchecked(PROGRAM_ID);
    let views = [unsafe { program_account.view() }, unsafe {
        none_sentinel.view()
    }];

    let (accounts, _, _) = UniqueProgramIdWithOptionalNone::try_accounts(
        &Address::new_from_array(PROGRAM_ID),
        &views,
        &[],
    )
    .expect("optional None sentinel must not duplicate real program-id account");
    assert!(accounts.maybe.is_none());
}

#[test]
fn unique_locals_do_not_shadow_account_fields() {
    let first = unchecked([0x11; 32]);
    let second = unchecked([0x22; 32]);
    let third = unchecked([0x33; 32]);
    let views = [unsafe { first.view() }, unsafe { second.view() }, unsafe {
        third.view()
    }];

    UniqueGeneratedNames::try_accounts(&Address::new_from_array(PROGRAM_ID), &views, &[])
        .expect("generated locals must not shadow user account fields");
}

#[test]
fn two_unique_fields_still_check_each_other() {
    let first = unchecked([0x11; 32]);
    let second = unchecked([0x11; 32]);
    let views = [unsafe { first.view() }, unsafe { second.view() }];

    let err = expect_err(TwoUniqueFields::try_accounts(
        &Address::new_from_array(PROGRAM_ID),
        &views,
        &[],
    ));
    assert_eq!(err, ErrorCode::ConstraintUnique.into());
}
