//! Run: `cargo test -p anchor-lang-v2 --features testing --test program_invoke`

use {
    anchor_lang_v2::{
        solana_program::{
            instruction::{AccountMeta, Instruction},
            program,
        },
        testing::{AccountBuffer, MIN_ACCOUNT_BUF},
        Address, CpiHandle, ToCpiHandle, ToCpiHandleMut,
    },
    solana_program_error::ProgramError,
};

fn account_view(address: [u8; 32], writable: bool) -> AccountBuffer<{ MIN_ACCOUNT_BUF + 8 }> {
    let buffer = AccountBuffer::new();
    buffer.init(address, [9; 32], 8, false, writable, false);
    buffer
}

fn instruction(account: Address, writable: bool) -> Instruction {
    let meta = if writable {
        AccountMeta::new(account, false)
    } else {
        AccountMeta::new_readonly(account, false)
    };

    Instruction {
        program_id: Address::new_from_array([7; 32]),
        accounts: vec![meta],
        data: vec![1, 2, 3],
    }
}

#[test]
fn checked_invoke_accepts_matching_handles() {
    let buffer = account_view([1; 32], true);
    let mut view = unsafe { buffer.view() };
    let ix = instruction(*view.address(), true);
    let handles = [CpiHandle::writable(&mut view)];

    program::invoke(&ix, &handles).unwrap();
}

#[test]
fn account_view_converts_to_cpi_handles() {
    let buffer = account_view([1; 32], true);
    let mut view = unsafe { buffer.view() };
    let address = *view.address();

    let readonly = view.to_cpi_handle();
    assert_eq!(*readonly.address(), address);
    assert!(!readonly.is_writable());

    let writable = view.to_cpi_handle_mut();
    assert_eq!(*writable.address(), address);
    assert!(writable.is_writable());
}

#[test]
fn checked_invoke_rejects_missing_handle() {
    let ix = instruction(Address::new_from_array([1; 32]), false);

    let err = program::invoke(&ix, &[]).unwrap_err();

    assert_eq!(err, ProgramError::NotEnoughAccountKeys);
}

#[test]
fn checked_invoke_rejects_address_mismatch() {
    let buffer = account_view([1; 32], false);
    let view = unsafe { buffer.view() };
    let ix = instruction(Address::new_from_array([2; 32]), false);
    let handles = [CpiHandle::readonly(&view)];

    let err = program::invoke(&ix, &handles).unwrap_err();

    assert_eq!(err, ProgramError::InvalidArgument);
}

#[test]
fn checked_invoke_rejects_readonly_handle_for_writable_meta() {
    let buffer = account_view([1; 32], true);
    let view = unsafe { buffer.view() };
    let ix = instruction(*view.address(), true);
    let handles = [CpiHandle::readonly(&view)];

    let err = program::invoke(&ix, &handles).unwrap_err();

    assert_eq!(err, ProgramError::InvalidArgument);
}

#[test]
fn checked_invoke_rejects_live_borrow_for_writable_meta() {
    let buffer = account_view([1; 32], true);
    let mut view = unsafe { buffer.view() };
    let borrow_view = view;
    let _borrow = borrow_view.try_borrow().unwrap();
    let ix = instruction(*view.address(), true);
    let handles = [CpiHandle::writable(&mut view)];

    let err = program::invoke(&ix, &handles).unwrap_err();

    assert_eq!(err, ProgramError::AccountBorrowFailed);
}

#[test]
fn unchecked_handle_api_is_available() {
    let ix = Instruction {
        program_id: Address::new_from_array([7; 32]),
        accounts: vec![],
        data: vec![],
    };

    unsafe { program::invoke_unchecked(&ix, &[]) }.unwrap();
}
