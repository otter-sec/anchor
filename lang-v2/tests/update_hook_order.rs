#![allow(dead_code, deprecated, unexpected_cfgs)]

use {
    anchor_lang::{
        accounts::{BorshAccount, Signer, UncheckedAccount},
        testing::AccountBuffer,
        AccountConstraint, Accounts, AnchorAccount, AnchorDeserialize, AnchorSerialize,
        Discriminator, ErrorCode, Nested, Owner, TryAccounts,
    },
    core::{mem::size_of, ptr},
    pinocchio::address::Address,
    pinocchio::account::{MAX_PERMITTED_DATA_INCREASE, RuntimeAccount},
    solana_program_error::ProgramError,
};

anchor_lang::declare_id!("11111111111111111111111111111111");

const PROGRAM_ID: [u8; 32] = [0x42; 32];
const OLD_AUTHORITY: [u8; 32] = [0x10; 32];
const NEW_AUTHORITY: [u8; 32] = [0x20; 32];

#[derive(AnchorDeserialize, AnchorSerialize, Clone, Copy)]
struct Vault {
    current_authority: Address,
}

impl Owner for Vault {
    const OWNER: Address = Address::new_from_array(PROGRAM_ID);
}

impl Discriminator for Vault {
    const DISCRIMINATOR: &'static [u8] = &[0x41, 0x75, 0x74, 0x68, 0x56, 0x61, 0x75, 0x6c];
}

#[derive(AnchorDeserialize, AnchorSerialize, Clone, Copy)]
struct StepCounter {
    value: u64,
}

impl Owner for StepCounter {
    const OWNER: Address = Address::new_from_array(PROGRAM_ID);
}

impl Discriminator for StepCounter {
    const DISCRIMINATOR: &'static [u8] = &[0x53, 0x74, 0x65, 0x70, 0x43, 0x74, 0x72, 0x21];
}

mod role {
    use super::*;

    pub struct SetAuthorityConstraint;

    impl AccountConstraint<BorshAccount<Vault>> for SetAuthorityConstraint {
        type Value = pinocchio::account::AccountView;

        fn update(
            account: &mut BorshAccount<Vault>,
            new_authority: &Self::Value,
        ) -> Result<(), ProgramError> {
            account.current_authority = *new_authority.address();
            Ok(())
        }
    }
}

mod counter_ns {
    use super::*;

    pub struct IncrementConstraint;

    impl AccountConstraint<BorshAccount<StepCounter>> for IncrementConstraint {
        type Value = u64;

        fn update(
            account: &mut BorshAccount<StepCounter>,
            amount: &Self::Value,
        ) -> Result<(), ProgramError> {
            account.value = account
                .value
                .checked_add(*amount)
                .ok_or(ProgramError::ArithmeticOverflow)?;
            Ok(())
        }
    }
}

#[derive(Accounts)]
struct RotateAuthority {
    #[account(mut, has_one = current_authority, update(role::set_authority = new_authority))]
    vault: BorshAccount<Vault>,
    current_authority: Signer,
    new_authority: UncheckedAccount,
}

#[derive(Accounts)]
struct InnerIncrement {
    #[account(mut, update(counter_ns::increment = 1u64))]
    counter: BorshAccount<StepCounter>,
}

#[derive(Accounts)]
struct OuterNestedIncrement {
    inner: Nested<InnerIncrement>,
}

#[derive(Accounts)]
struct OuterNestedGate {
    inner: Nested<InnerIncrement>,
    #[account(constraint = inner.counter.value == 1u64)]
    witness: UncheckedAccount,
}

#[anchor_lang::program]
mod demo_program {
    use super::*;

    pub fn rotate(ctx: &mut anchor_lang::Context<RotateAuthority>) -> anchor_lang::Result<()> {
        if ctx.accounts.vault.current_authority.to_bytes() != NEW_AUTHORITY {
            return Err(anchor_lang::ErrorCode::ConstraintAddress.into());
        }
        Ok(())
    }
}

fn expect_err<T>(result: Result<T, ProgramError>) -> ProgramError {
    match result {
        Ok(_) => panic!("expected Err, got Ok"),
        Err(err) => err,
    }
}

fn vault_account(authority: [u8; 32]) -> AccountBuffer<128> {
    let buf = AccountBuffer::<128>::new();
    let mut data = [0u8; 40];
    data[..8].copy_from_slice(Vault::DISCRIMINATOR);
    data[8..40].copy_from_slice(&authority);
    buf.init([0xAA; 32], PROGRAM_ID, data.len(), false, true, false);
    buf.write_data(&data);
    buf
}

fn signer_account(address: [u8; 32], signer: bool) -> AccountBuffer<128> {
    let buf = AccountBuffer::<128>::new();
    buf.init(address, PROGRAM_ID, 0, signer, false, false);
    buf
}

fn unchecked_account(address: [u8; 32]) -> AccountBuffer<128> {
    let buf = AccountBuffer::<128>::new();
    buf.init(address, PROGRAM_ID, 0, false, false, false);
    buf
}

fn read_vault_authority(buf: &AccountBuffer<128>) -> [u8; 32] {
    let data = buf.read_data();
    data[8..40].try_into().unwrap()
}

fn counter_account(value: u64) -> AccountBuffer<128> {
    let buf = AccountBuffer::<128>::new();
    let mut data = [0u8; 16];
    data[..8].copy_from_slice(StepCounter::DISCRIMINATOR);
    data[8..16].copy_from_slice(&value.to_le_bytes());
    buf.init([0xAC; 32], PROGRAM_ID, data.len(), false, true, false);
    buf.write_data(&data);
    buf
}

fn read_counter_value(buf: &AccountBuffer<128>) -> u64 {
    let data = buf.read_data();
    u64::from_le_bytes(data[8..16].try_into().unwrap())
}

fn build_dispatch_input<const N: usize>(accounts: &[&AccountBuffer<N>]) -> Vec<u64> {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(&(accounts.len() as u64).to_le_bytes());

    for account in accounts {
        while bytes.len() % 8 != 0 {
            bytes.push(0);
        }

        let raw = account.raw();
        let header = unsafe {
            core::slice::from_raw_parts(raw as *const u8, size_of::<RuntimeAccount>())
        };
        bytes.extend_from_slice(header);
        bytes.extend_from_slice(account.read_data());
        bytes.extend(core::iter::repeat_n(0u8, MAX_PERMITTED_DATA_INCREASE));
        bytes.extend_from_slice(&0u64.to_le_bytes());
    }

    let mut backing = vec![0u64; bytes.len().div_ceil(8)];
    unsafe {
        ptr::copy_nonoverlapping(bytes.as_ptr(), backing.as_mut_ptr() as *mut u8, bytes.len());
    }
    backing
}

fn read_vault_authority_from_dispatch_input(input: &[u64]) -> [u8; 32] {
    unsafe {
        let account = input.as_ptr().cast::<u8>().add(size_of::<u64>()) as *const RuntimeAccount;
        let data_len = (*account).data_len as usize;
        let data = core::slice::from_raw_parts(
            (account as *const u8).add(size_of::<RuntimeAccount>()),
            data_len,
        );
        data[8..40].try_into().unwrap()
    }
}

fn build_rotate_ix_data() -> Vec<u64> {
    let ix = crate::instruction::Rotate {};
    let data = <crate::instruction::Rotate as anchor_lang::InstructionData>::data(&ix);
    let byte_len = size_of::<u64>() + data.len() + 32;
    let mut bytes = Vec::with_capacity(byte_len);
    bytes.extend_from_slice(&(data.len() as u64).to_le_bytes());
    bytes.extend_from_slice(&data);
    bytes.extend_from_slice(&crate::ID.to_bytes());

    let mut backing = vec![0u64; bytes.len().div_ceil(8)];
    unsafe {
        ptr::copy_nonoverlapping(bytes.as_ptr(), backing.as_mut_ptr() as *mut u8, bytes.len());
    }
    backing
}

#[test]
fn try_accounts_checks_authority_before_running_updates() {
    let vault = vault_account(OLD_AUTHORITY);
    let current = signer_account(NEW_AUTHORITY, true);
    let replacement = unchecked_account(NEW_AUTHORITY);
    let views = [unsafe { vault.view() }, unsafe { current.view() }, unsafe {
        replacement.view()
    }];

    let err = expect_err(RotateAuthority::try_accounts(
        &Address::new_from_array(PROGRAM_ID),
        &views,
        None,
        0,
        &[],
    ));

    assert_eq!(err, ErrorCode::ConstraintHasOne.into());
    assert_eq!(read_vault_authority(&vault), OLD_AUTHORITY);
}

#[test]
fn update_accounts_runs_after_validation_and_persists_on_exit() {
    let vault = vault_account(OLD_AUTHORITY);
    let current = signer_account(OLD_AUTHORITY, true);
    let replacement = unchecked_account(NEW_AUTHORITY);
    let views = [unsafe { vault.view() }, unsafe { current.view() }, unsafe {
        replacement.view()
    }];

    let (mut accounts, _, _) = <RotateAuthority as TryAccounts>::validate_accounts(
        &Address::new_from_array(PROGRAM_ID),
        &views,
        None,
        0,
        &[],
    )
    .expect("authority check should pass before updates");

    assert_eq!(accounts.vault.current_authority.to_bytes(), OLD_AUTHORITY);

    accounts.update_accounts().unwrap();
    assert_eq!(accounts.vault.current_authority.to_bytes(), NEW_AUTHORITY);

    accounts.exit_accounts(&[]).unwrap();
    assert_eq!(read_vault_authority(&vault), NEW_AUTHORITY);
}

#[test]
fn try_accounts_still_runs_updates_for_direct_callers() {
    let vault = vault_account(OLD_AUTHORITY);
    let current = signer_account(OLD_AUTHORITY, true);
    let replacement = unchecked_account(NEW_AUTHORITY);
    let views = [unsafe { vault.view() }, unsafe { current.view() }, unsafe {
        replacement.view()
    }];

    let (accounts, _, _) =
        RotateAuthority::try_accounts(&Address::new_from_array(PROGRAM_ID), &views, None, 0, &[])
            .expect("direct callers should still receive updated accounts");

    assert_eq!(accounts.vault.current_authority.to_bytes(), NEW_AUTHORITY);
}

#[test]
fn generated_dispatch_runs_update_phase_before_user_handler() {
    let vault = vault_account(OLD_AUTHORITY);
    let current = signer_account(OLD_AUTHORITY, true);
    let replacement = unchecked_account(NEW_AUTHORITY);
    let mut input = build_dispatch_input(&[&vault, &current, &replacement]);
    let ix_buf = build_rotate_ix_data();

    let result = unsafe {
        crate::__anchor_dispatch(
            input.as_mut_ptr() as *mut u8,
            ix_buf.as_ptr().cast::<u8>().add(8),
        )
    };

    assert_eq!(
        result, 0,
        "generated dispatch must run updates before the user handler observes accounts"
    );
    assert_eq!(
        read_vault_authority_from_dispatch_input(&input),
        NEW_AUTHORITY,
        "generated dispatch must persist update-phase mutations when exit_accounts runs"
    );
}

#[test]
fn nested_validate_accounts_keeps_inner_updates_after_outer_validation() {
    let counter = counter_account(0);
    let witness = unchecked_account([0x33; 32]);
    let views = [unsafe { counter.view() }, unsafe { witness.view() }];

    let err = expect_err(<OuterNestedGate as TryAccounts>::validate_accounts(
        &Address::new_from_array(PROGRAM_ID),
        &views,
        None,
        0,
        &[],
    ));

    assert_eq!(err, ErrorCode::ConstraintRaw.into());
    assert_eq!(
        read_counter_value(&counter),
        0,
        "nested validate_accounts must not run inner update hooks before outer validation"
    );
}

#[test]
fn nested_try_accounts_runs_inner_updates_once() {
    let counter = counter_account(0);
    let views = [unsafe { counter.view() }];

    let (mut accounts, _, _) = OuterNestedIncrement::try_accounts(
        &Address::new_from_array(PROGRAM_ID),
        &views,
        None,
        0,
        &[],
    )
    .expect("nested try_accounts should succeed");

    assert_eq!(
        accounts.inner.counter.value,
        1,
        "nested update hooks should run exactly once"
    );

    accounts.exit_accounts(&[]).unwrap();
    assert_eq!(read_counter_value(&counter), 1);
}
