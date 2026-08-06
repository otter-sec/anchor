//! Run: `cargo test -p anchor-lang --features testing --test program_invoke`

use {
    anchor_lang::{
        accounts::Account,
        prelude::BorshAccount,
        solana_program::{
            instruction::{AccountMeta, Instruction},
            program,
        },
        testing::{AccountBuffer, MIN_ACCOUNT_BUF},
        Address, AnchorAccount, AnchorDeserialize, AnchorSerialize, CpiContext, CpiHandle,
        CpiHandleMut, Discriminator, Owner, ToCpiAccounts, ToCpiHandle, ToCpiHandleMut,
    },
    bytemuck::{Pod, Zeroable},
    solana_program_error::ProgramError,
};

const ID: Address = Address::new_from_array([7; 32]);
const PROGRAM_ID: [u8; 32] = [0x42; 32];

#[derive(ToCpiAccounts)]
struct ReadonlyCpi<'a> {
    account: CpiHandle<'a>,
}

#[derive(ToCpiAccounts)]
struct WritableCpi<'a> {
    account: CpiHandleMut<'a>,
}

#[derive(ToCpiAccounts)]
struct OptionalCpi<'a> {
    account: CpiHandle<'a>,
    optional: Option<CpiHandle<'a>>,
}

#[derive(ToCpiAccounts)]
struct ThreeReadonlyCpi<'a> {
    first: CpiHandle<'a>,
    second: CpiHandle<'a>,
    third: CpiHandle<'a>,
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct PodCounter {
    value: u64,
}

impl Owner for PodCounter {
    const OWNER: Address = Address::new_from_array(PROGRAM_ID);
}

impl Discriminator for PodCounter {
    const DISCRIMINATOR: &'static [u8] = &[0x4c, 0xde, 0x7f, 0x28, 0x61, 0x2f, 0x07, 0x73];
}

#[derive(AnchorDeserialize, AnchorSerialize, Default, Clone, PartialEq, Debug)]
struct BorshCounter {
    value: u64,
}

impl Owner for BorshCounter {
    const OWNER: Address = Address::new_from_array(PROGRAM_ID);
}

impl Discriminator for BorshCounter {
    const DISCRIMINATOR: &'static [u8] = &[0xff, 0xb0, 0x04, 0xf5, 0xbc, 0xfd, 0x7c, 0x19];
}

fn account_view(address: [u8; 32], writable: bool) -> AccountBuffer<{ MIN_ACCOUNT_BUF + 8 }> {
    let buffer = AccountBuffer::new();
    buffer.init(address, [9; 32], 8, false, writable, false);
    buffer
}

fn slab_account_view(
    address: [u8; 32],
    writable: bool,
    value: u64,
) -> AccountBuffer<{ MIN_ACCOUNT_BUF + 8 }> {
    let buffer = AccountBuffer::new();
    buffer.init(
        address,
        PROGRAM_ID,
        8 + core::mem::size_of::<PodCounter>(),
        false,
        writable,
        false,
    );
    let mut data = [0u8; 16];
    data[..8].copy_from_slice(PodCounter::DISCRIMINATOR);
    data[8..16].copy_from_slice(&value.to_le_bytes());
    buffer.write_data(&data);
    buffer
}

fn borsh_account_view(address: [u8; 32], writable: bool, value: u64) -> AccountBuffer<256> {
    let buffer = AccountBuffer::new();
    buffer.init(address, PROGRAM_ID, 16, false, writable, false);
    let mut data = [0u8; 16];
    data[..8].copy_from_slice(BorshCounter::DISCRIMINATOR);
    data[8..16].copy_from_slice(&value.to_le_bytes());
    buffer.write_data(&data);
    buffer.set_lamports(1_000_000_000);
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

fn signer_instruction(account: Address, writable: bool) -> Instruction {
    let meta = if writable {
        AccountMeta::new(account, true)
    } else {
        AccountMeta::new_readonly(account, true)
    };

    Instruction {
        program_id: ID,
        accounts: vec![meta],
        data: vec![9, 9, 9],
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
fn cpi_handle_as_signer_overrides_transaction_view() {
    let buffer = account_view([1; 32], false);
    let view = unsafe { buffer.view() };
    assert!(!view.is_signer());

    let handle = CpiHandle::readonly(&view);
    assert!(!handle.is_signer());
    assert!(handle.as_signer().is_signer());

    let mut_buffer = account_view([3; 32], true);
    let mut writable_view = unsafe { mut_buffer.view() };
    let mut_handle = CpiHandleMut::writable(&mut writable_view);
    assert!(!mut_handle.is_signer());
    let mut_signer = mut_handle.as_signer();
    assert!(mut_signer.is_signer());

    let erased: CpiHandle = mut_signer.into();
    assert!(erased.is_signer());
    assert!(erased.is_writable());
    let signer_buffer = AccountBuffer::<{ MIN_ACCOUNT_BUF + 8 }>::new();
    signer_buffer.init([4; 32], [9; 32], 8, true, false, false);
    let signer_view = unsafe { signer_buffer.view() };
    assert!(CpiHandle::readonly(&signer_view).is_signer());
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

    // Index 1 is an intentional Option::None sentinel — not a required account.
    program::invoke_with_optional_sentinels(&ix, &handles, &[false, true]).unwrap();
}

#[test]
fn checked_invoke_rejects_required_program_id_account_without_handle() {
    // Pre-fix: any readonly non-signer meta at the callee program id was
    // treated as an optional None sentinel, so a required program-id account
    // could be skipped when its handle was omitted.
    let other = account_view([1; 32], false);
    let other_view = unsafe { other.view() };
    let ix = Instruction {
        program_id: ID,
        accounts: vec![
            AccountMeta::new_readonly(*other_view.address(), false),
            AccountMeta::new_readonly(ID, false),
        ],
        data: vec![],
    };
    let handles = [other_view.to_cpi_handle()];

    let err = program::invoke(&ix, &handles).unwrap_err();
    assert_eq!(err, ProgramError::NotEnoughAccountKeys);
}

#[test]
fn checked_invoke_accepts_required_program_id_account_with_handle() {
    let other = account_view([1; 32], false);
    let program_account = account_view([7; 32], false);
    let other_view = unsafe { other.view() };
    let program_view = unsafe { program_account.view() };
    let ix = Instruction {
        program_id: ID,
        accounts: vec![
            AccountMeta::new_readonly(*other_view.address(), false),
            AccountMeta::new_readonly(ID, false),
        ],
        data: vec![],
    };
    let handles = [other_view.to_cpi_handle(), program_view.to_cpi_handle()];

    program::invoke(&ix, &handles).unwrap();
}

#[test]
fn checked_invoke_optional_none_before_required_program_id_uses_mask() {
    // Optional None sentinel then a required program-id account. The handle
    // must bind to the required slot, not be stolen by the sentinel.
    let program_account = account_view([7; 32], false);
    let program_view = unsafe { program_account.view() };
    let ix = Instruction {
        program_id: ID,
        accounts: vec![
            AccountMeta::new_readonly(ID, false),
            AccountMeta::new_readonly(ID, false),
        ],
        data: vec![],
    };
    let handles = [program_view.to_cpi_handle()];

    program::invoke_with_optional_sentinels(&ix, &handles, &[true, false]).unwrap();
}

#[test]
fn checked_invoke_accepts_real_program_id_account_before_later_accounts() {
    let first_buffer = account_view([1; 32], false);
    let middle_buffer = account_view([7; 32], false);
    let third_buffer = account_view([2; 32], false);
    let first = unsafe { first_buffer.view() };
    let middle = unsafe { middle_buffer.view() };
    let third = unsafe { third_buffer.view() };
    let ix = Instruction {
        program_id: ID,
        accounts: vec![
            AccountMeta::new_readonly(*first.address(), false),
            AccountMeta::new_readonly(*middle.address(), false),
            AccountMeta::new_readonly(*third.address(), false),
        ],
        data: vec![],
    };
    let handles = [
        first.to_cpi_handle(),
        middle.to_cpi_handle(),
        third.to_cpi_handle(),
    ];

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
fn checked_invoke_rejects_nonsigner_handle_for_signer_meta() {
    let buffer = account_view([1; 32], false);
    let view = unsafe { buffer.view() };
    let ix = signer_instruction(*view.address(), false);
    let handles = [CpiHandle::readonly(&view)];

    let err = program::invoke(&ix, &handles).unwrap_err();

    assert_eq!(err, ProgramError::MissingRequiredSignature);
}

#[test]
fn checked_invoke_signed_allows_signer_meta_without_tx_signer_when_seeds_are_supplied() {
    let buffer = account_view([1; 32], false);
    let view = unsafe { buffer.view() };
    let ix = signer_instruction(*view.address(), false);
    let handles = [CpiHandle::readonly(&view)];

    program::invoke_signed(&ix, &handles, &[&[b"pda", &[7]]]).unwrap();
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
fn invoke_ix_rejects_nonsigner_handle_for_signer_meta_without_seeds() {
    let program = ID;
    let buffer = account_view([1; 32], false);
    let view = unsafe { buffer.view() };
    let accounts = ReadonlyCpi {
        account: view.to_cpi_handle(),
    };
    let ix = signer_instruction(*view.address(), false);

    let err = CpiContext::new(&program, accounts)
        .invoke_ix(ix)
        .unwrap_err();

    assert_eq!(err, ProgramError::MissingRequiredSignature);
}

#[test]
fn invoke_ix_allows_signer_meta_without_tx_signer_when_seeds_are_supplied() {
    let program = ID;
    let buffer = account_view([1; 32], false);
    let view = unsafe { buffer.view() };
    let accounts = ReadonlyCpi {
        account: view.to_cpi_handle(),
    };
    let ix = signer_instruction(*view.address(), false);

    CpiContext::new_with_signer(&program, accounts, &[&[b"pda", &[7]]])
        .invoke_ix(ix)
        .unwrap();
}

#[test]
fn invoke_remaining_pda_as_signer_with_seeds() {
    let program = ID;
    let typed_buffer = account_view([1; 32], false);
    let remaining_buffer = account_view([2; 32], false);
    let typed_view = unsafe { typed_buffer.view() };
    let remaining_view = unsafe { remaining_buffer.view() };
    let accounts = ReadonlyCpi {
        account: typed_view.to_cpi_handle(),
    };

    CpiContext::new_with_signer(&program, accounts, &[&[b"pda", &[7]]])
        .with_remaining_accounts(vec![CpiHandle::readonly(&remaining_view).as_signer()])
        .invoke(&[])
        .unwrap();
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
fn cpi_context_invoke_accepts_real_program_id_account_before_later_accounts() {
    let program = ID;
    let first_buffer = account_view([1; 32], false);
    let middle_buffer = account_view([7; 32], false);
    let third_buffer = account_view([2; 32], false);
    let first = unsafe { first_buffer.view() };
    let middle = unsafe { middle_buffer.view() };
    let third = unsafe { third_buffer.view() };
    let accounts = ThreeReadonlyCpi {
        first: first.to_cpi_handle(),
        second: middle.to_cpi_handle(),
        third: third.to_cpi_handle(),
    };

    CpiContext::new(&program, accounts).invoke(&[]).unwrap();
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
fn cpi_context_invoke_rejects_live_borrow_for_writable_meta() {
    let program = ID;
    let buffer = account_view([1; 32], true);
    let mut view = unsafe { buffer.view() };
    let borrow_view = view;
    let _borrow = borrow_view.try_borrow().unwrap();
    let accounts = WritableCpi {
        account: view.to_cpi_handle_mut(),
    };

    let err = CpiContext::new(&program, accounts)
        .invoke(&[1, 2, 3])
        .unwrap_err();

    assert_eq!(err, ProgramError::AccountBorrowFailed);
}

#[test]
fn cpi_context_invoke_accepts_mutable_slab_handle() {
    let program = ID;
    let buffer = slab_account_view([1; 32], true, 9);
    let view = unsafe { buffer.view() };
    let mut acct = unsafe { Account::<PodCounter>::load_mut(view) }.unwrap();
    let accounts = WritableCpi {
        account: acct.cpi_handle_mut(),
    };

    CpiContext::new(&program, accounts)
        .invoke(&[1, 2, 3])
        .unwrap();
}

#[test]
fn cpi_context_invoke_accepts_readonly_slab_handle_from_mutable_wrapper() {
    let program = ID;
    let buffer = slab_account_view([1; 32], true, 9);
    let view = unsafe { buffer.view() };
    let acct = unsafe { Account::<PodCounter>::load_mut(view) }.unwrap();
    let accounts = ReadonlyCpi {
        account: acct.cpi_handle(),
    };

    CpiContext::new(&program, accounts)
        .invoke(&[1, 2, 3])
        .unwrap();
}

#[test]
fn invoke_ix_rejects_live_borrow_for_writable_meta() {
    let program = ID;
    let buffer = account_view([1; 32], true);
    let mut view = unsafe { buffer.view() };
    let address = *view.address();
    let borrow_view = view;
    let _borrow = borrow_view.try_borrow().unwrap();
    let accounts = WritableCpi {
        account: view.to_cpi_handle_mut(),
    };
    let ix = Instruction {
        program_id: program,
        accounts: vec![AccountMeta::new(address, false)],
        data: vec![],
    };

    let err = CpiContext::new(&program, accounts)
        .invoke_ix(ix)
        .unwrap_err();

    assert_eq!(err, ProgramError::AccountBorrowFailed);
}


#[test]
fn invoke_ix_validates_remaining_account_borrow_state() {
    let program = ID;
    let buffer = account_view([1; 32], true);
    let view = unsafe { buffer.view() };
    let accounts = ReadonlyCpi {
        account: view.to_cpi_handle(),
    };
    let ix = Instruction {
        program_id: program,
        accounts: vec![AccountMeta::new_readonly(*view.address(), false)],
        data: vec![],
    };

    let remaining_buffer = account_view([2; 32], true);
    let mut remaining_view = unsafe { remaining_buffer.view() };
    let borrow_view = remaining_view;
    let _borrow = borrow_view.try_borrow().unwrap();
    let remaining_handle = CpiHandle::writable(&mut remaining_view);

    let err = CpiContext::new(&program, accounts)
        .with_remaining_accounts(vec![remaining_handle])
        .invoke_ix(ix)
        .unwrap_err();

    assert_eq!(err, ProgramError::AccountBorrowFailed);
}


#[test]
fn invoke_ix_accepts_remaining_accounts_alongside_fixed_accounts() {
    let program = ID;
    let buffer = account_view([1; 32], true);
    let view = unsafe { buffer.view() };
    let accounts = ReadonlyCpi {
        account: view.to_cpi_handle(),
    };
    let ix = Instruction {
        program_id: program,
        accounts: vec![AccountMeta::new_readonly(*view.address(), false)],
        data: vec![],
    };

    let remaining_buffer = account_view([2; 32], true);
    let remaining_view = unsafe { remaining_buffer.view() };
    let remaining_handle = remaining_view.to_cpi_handle();

    CpiContext::new(&program, accounts)
        .with_remaining_accounts(vec![remaining_handle])
        .invoke_ix(ix)
        .unwrap();
}

#[test]
fn invoke_ix_accepts_mutable_slab_handle() {
    let program = ID;
    let buffer = slab_account_view([1; 32], true, 9);
    let view = unsafe { buffer.view() };
    let mut acct = unsafe { Account::<PodCounter>::load_mut(view) }.unwrap();
    let address = *acct.address();
    let accounts = WritableCpi {
        account: acct.cpi_handle_mut(),
    };
    let ix = Instruction {
        program_id: program,
        accounts: vec![AccountMeta::new(address, false)],
        data: vec![],
    };

    CpiContext::new(&program, accounts).invoke_ix(ix).unwrap();
}

#[test]
fn invoke_ix_accepts_readonly_slab_handle_from_mutable_wrapper() {
    let program = ID;
    let buffer = slab_account_view([1; 32], true, 9);
    let view = unsafe { buffer.view() };
    let acct = unsafe { Account::<PodCounter>::load_mut(view) }.unwrap();
    let address = *acct.address();
    let accounts = ReadonlyCpi {
        account: acct.cpi_handle(),
    };
    let ix = Instruction {
        program_id: program,
        accounts: vec![AccountMeta::new_readonly(address, false)],
        data: vec![],
    };

    CpiContext::new(&program, accounts).invoke_ix(ix).unwrap();
}

#[test]
fn cpi_context_invoke_accepts_mutable_borsh_handle() {
    let program = ID;
    let buffer = borsh_account_view([1; 32], true, 9);
    let view = unsafe { buffer.view() };
    let mut acct = unsafe { BorshAccount::<BorshCounter>::load_mut(view) }.unwrap();

    {
        let accounts = WritableCpi {
            account: acct.cpi_handle_mut(),
        };
        CpiContext::new(&program, accounts)
            .invoke(&[1, 2, 3])
            .unwrap();
    }

    acct.reacquire_borrow_mut().unwrap();
    acct.value = 11;
    assert_eq!(acct.value, 11);
}

#[test]
fn invoke_ix_accepts_mutable_borsh_handle() {
    let program = ID;
    let buffer = borsh_account_view([1; 32], true, 9);
    let view = unsafe { buffer.view() };
    let mut acct = unsafe { BorshAccount::<BorshCounter>::load_mut(view) }.unwrap();
    let address = *acct.address();

    {
        let accounts = WritableCpi {
            account: acct.cpi_handle_mut(),
        };
        let ix = Instruction {
            program_id: program,
            accounts: vec![AccountMeta::new(address, false)],
            data: vec![],
        };
        CpiContext::new(&program, accounts).invoke_ix(ix).unwrap();
    }

    acct.reacquire_borrow_mut().unwrap();
    acct.value = 11;
    assert_eq!(acct.value, 11);
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
