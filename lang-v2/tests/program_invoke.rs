//! Run: `cargo test -p anchor-lang-v2 --features testing --test program_invoke`

use {
    anchor_lang_v2::{
        solana_program::{
            instruction::{AccountMeta, Instruction},
            program,
        },
        testing::{AccountBuffer, MIN_ACCOUNT_BUF},
        Address, CpiContext, CpiHandle, ToCpiAccounts, ToCpiHandle, ToCpiHandleMut,
    },
    solana_program_error::ProgramError,
};

const ID: Address = Address::new_from_array([7; 32]);

#[derive(ToCpiAccounts)]
struct ReadonlyCpi<'a> {
    account: CpiHandle<'a>,
}

#[derive(ToCpiAccounts)]
struct OptionalCpi<'a> {
    account: CpiHandle<'a>,
    optional: Option<CpiHandle<'a>>,
}

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
        program_id: ID,
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
fn checked_invoke_accepts_optional_none_program_id_sentinel() {
    let buffer = account_view([1; 32], false);
    let view = unsafe { buffer.view() };
    let ix = Instruction {
        program_id: ID,
        accounts: vec![
            AccountMeta::new_readonly(*view.address(), false),
            AccountMeta::new_readonly(ID, false),
        ],
        data: vec![],
    };
    let handles = [view.to_cpi_handle()];

    program::invoke(&ix, &handles).unwrap();
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
fn invoke_ix_rejects_readonly_handle_for_writable_meta() {
    let program = ID;
    let buffer = account_view([1; 32], true);
    let view = unsafe { buffer.view() };
    let accounts = ReadonlyCpi {
        account: view.to_cpi_handle(),
    };
    let ix = Instruction {
        program_id: program,
        accounts: vec![AccountMeta::new(*view.address(), false)],
        data: vec![],
    };

    let err = CpiContext::new(&program, accounts)
        .invoke_ix(ix)
        .unwrap_err();

    assert_eq!(err, ProgramError::InvalidArgument);
}

#[test]
fn invoke_ix_accepts_optional_none_program_id_sentinel() {
    let program = ID;
    let buffer = account_view([1; 32], false);
    let view = unsafe { buffer.view() };
    let accounts = OptionalCpi {
        account: view.to_cpi_handle(),
        optional: None,
    };
    let ix = Instruction {
        program_id: program,
        accounts: vec![
            AccountMeta::new_readonly(*view.address(), false),
            AccountMeta::new_readonly(program, false),
        ],
        data: vec![],
    };

    CpiContext::new(&program, accounts).invoke_ix(ix).unwrap();
}

#[test]
fn invoke_ix_rejects_writable_program_id_meta_without_handle() {
    let program = ID;
    let buffer = account_view([1; 32], false);
    let view = unsafe { buffer.view() };
    let accounts = OptionalCpi {
        account: view.to_cpi_handle(),
        optional: None,
    };
    let ix = Instruction {
        program_id: program,
        accounts: vec![
            AccountMeta::new_readonly(*view.address(), false),
            AccountMeta::new(program, false),
        ],
        data: vec![],
    };

    let err = CpiContext::new(&program, accounts)
        .invoke_ix(ix)
        .unwrap_err();

    assert_eq!(err, ProgramError::NotEnoughAccountKeys);
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
