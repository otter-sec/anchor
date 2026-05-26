extern crate alloc;

use {
    crate::{address_eq, CpiHandle},
    alloc::vec::Vec,
    core::{mem::MaybeUninit, slice::from_raw_parts},
    pinocchio::{
        cpi::{CpiAccount, Seed, Signer},
        instruction::{InstructionAccount, InstructionView},
    },
    solana_instruction::Instruction,
    solana_program_error::{ProgramError, ProgramResult},
};

pub use pinocchio::cpi::{set_return_data, MAX_RETURN_DATA};

/// Get the return data from an invoked program.
///
/// This preserves the v1-shaped `program::get_return_data()` return value while
/// sourcing the data from Pinocchio's `AccountView`-native CPI module.
pub fn get_return_data() -> Option<(crate::Address, Vec<u8>)> {
    pinocchio::cpi::get_return_data().map(|data| (*data.program_id(), data.as_slice().to_vec()))
}

/// Invoke a cross-program instruction using Anchor v2 CPI handles.
///
/// Unlike the legacy `AccountInfo` API, callers pass [`CpiHandle`]s obtained
/// from `cpi_handle()` / `cpi_handle_mut()`, so CPI account lifetimes remain
/// tied to Rust borrows of the caller's typed accounts.
pub fn invoke<'a>(instruction: &Instruction, account_handles: &[CpiHandle<'a>]) -> ProgramResult {
    invoke_signed(instruction, account_handles, &[])
}

/// Invoke a cross-program instruction with PDA signer seeds using Anchor v2
/// CPI handles.
pub fn invoke_signed<'a, 'seeds>(
    instruction: &Instruction,
    account_handles: &[CpiHandle<'a>],
    signer_seeds: &'seeds [&'seeds [&'seeds [u8]]],
) -> ProgramResult {
    validate_handles(instruction, account_handles)?;

    // SAFETY: Validation above proves every instruction account has a matching
    // handle, writable metas use writable handles, and AccountView borrow state
    // permits the CPI.
    unsafe { invoke_signed_unchecked(instruction, account_handles, signer_seeds) }
}

/// Invoke a cross-program instruction without borrow validation.
///
/// # Safety
///
/// The caller must ensure no live Rust references or stale `AccountView` data
/// borrows can be invalidated by the callee. Prefer [`invoke`] unless this is
/// being used through a higher-level API that already enforces those lifetimes.
pub unsafe fn invoke_unchecked<'a>(
    instruction: &Instruction,
    account_handles: &[CpiHandle<'a>],
) -> ProgramResult {
    unsafe { invoke_signed_unchecked(instruction, account_handles, &[]) }
}

/// Invoke a cross-program instruction with PDA signer seeds, without borrow
/// validation.
///
/// # Safety
///
/// The caller must ensure no live Rust references or stale `AccountView` data
/// borrows can be invalidated by the callee. Prefer [`invoke_signed`] unless
/// this is being used through a higher-level API that already enforces those
/// lifetimes.
pub unsafe fn invoke_signed_unchecked<'a, 'seeds>(
    instruction: &Instruction,
    account_handles: &[CpiHandle<'a>],
    signer_seeds: &'seeds [&'seeds [&'seeds [u8]]],
) -> ProgramResult {
    if account_handles.len() < instruction.accounts.len() {
        return Err(ProgramError::NotEnoughAccountKeys);
    }

    let instruction_accounts = instruction_accounts(instruction);
    let instruction_view = InstructionView {
        program_id: &instruction.program_id,
        accounts: &instruction_accounts,
        data: &instruction.data,
    };
    let signers = signers(signer_seeds);
    let cpi_accounts = cpi_accounts(account_handles);

    // SAFETY:
    // - `cpi_accounts` was fully initialized from the provided handles.
    // - This function's caller upholds the unchecked CPI aliasing contract.
    unsafe {
        pinocchio::cpi::invoke_signed_unchecked(
            &instruction_view,
            from_raw_parts(
                cpi_accounts.as_ptr() as *const CpiAccount,
                instruction.accounts.len(),
            ),
            &signers,
        );
    }

    Ok(())
}

fn validate_handles(instruction: &Instruction, account_handles: &[CpiHandle<'_>]) -> ProgramResult {
    if account_handles.len() < instruction.accounts.len() {
        return Err(ProgramError::NotEnoughAccountKeys);
    }

    for (meta, handle) in instruction.accounts.iter().zip(account_handles) {
        if !address_eq(&meta.pubkey, handle.address()) {
            return Err(ProgramError::InvalidArgument);
        }

        if meta.is_writable {
            if !handle.is_writable() {
                return Err(ProgramError::InvalidArgument);
            }
            handle.account_view().check_borrow_mut()?;
        } else {
            handle.account_view().check_borrow()?;
        }
    }

    Ok(())
}

fn instruction_accounts(instruction: &Instruction) -> Vec<InstructionAccount<'_>> {
    instruction
        .accounts
        .iter()
        .map(|meta| InstructionAccount::new(&meta.pubkey, meta.is_writable, meta.is_signer))
        .collect()
}

fn signers<'seeds>(signer_seeds: &'seeds [&'seeds [&'seeds [u8]]]) -> Vec<Signer<'seeds, 'seeds>> {
    signer_seeds
        .iter()
        .map(|seeds| {
            // SAFETY: `Seed` has the same in-memory representation as `&[u8]`;
            // this is the conversion used by `CpiContext::invoke` as well.
            let cpi_seeds: &[Seed] =
                unsafe { from_raw_parts(seeds.as_ptr() as *const Seed, seeds.len()) };
            Signer::from(cpi_seeds)
        })
        .collect()
}

fn cpi_accounts<'a>(account_handles: &[CpiHandle<'a>]) -> Vec<MaybeUninit<CpiAccount<'a>>> {
    let mut accounts = Vec::with_capacity(account_handles.len());
    // SAFETY: `MaybeUninit<CpiAccount>` does not require initialization.
    unsafe { accounts.set_len(account_handles.len()) };

    for (handle, slot) in account_handles.iter().zip(accounts.iter_mut()) {
        CpiAccount::init_from_account_view(handle.account_view(), slot);
    }

    accounts
}
