pub use pinocchio::cpi::{set_return_data, MAX_RETURN_DATA};
use {
    crate::{address_eq, require, Address, CpiHandle, CpiInstruction},
    core::{mem::MaybeUninit, slice::from_raw_parts},
    pinocchio::{
        cpi::{CpiAccount, Seed, Signer},
        instruction::{InstructionAccount, InstructionView},
    },
    solana_program_error::{ProgramError, ProgramResult},
};

/// Get the return data from an invoked program.
///
/// The payload stays in Pinocchio's allocator-neutral return-data buffer.
pub fn get_return_data() -> Option<(crate::Address, pinocchio::cpi::ReturnData)> {
    pinocchio::cpi::get_return_data().map(|data| (*data.program_id(), data))
}

/// Invoke a cross-program instruction using Anchor v2 CPI handles.
///
/// Unlike the legacy `AccountInfo` API, callers pass [`CpiHandle`]s obtained
/// from `cpi_handle()` / `cpi_handle_mut()`, so CPI account lifetimes remain
/// tied to Rust borrows of the caller's typed accounts.
///
/// Optional program-id sentinel metas from `Option::None` CPI slots are not
/// inferred from address alone — use
/// [`invoke_with_optional_sentinels`] / [`invoke_signed_with_optional_sentinels`]
/// (or [`crate::CpiContext`]) so required readonly program-id accounts still
/// require matching handles.
pub fn invoke<'a, I>(instruction: &I, account_handles: &[CpiHandle<'a>]) -> ProgramResult
where
    I: CpiInstruction + ?Sized,
{
    invoke_signed(instruction, account_handles, &[])
}

/// Like [`invoke`], but marks instruction-account indices that are intentional
/// optional `None` program-id sentinels (parallel to
/// [`crate::ToCpiAccountsWithCapacity::optional_account_sentinel_flags_with_capacity`]).
pub fn invoke_with_optional_sentinels<'a, I>(
    instruction: &I,
    account_handles: &[CpiHandle<'a>],
    optional_sentinel_flags: &[bool],
) -> ProgramResult
where
    I: CpiInstruction + ?Sized,
{
    invoke_signed_with_optional_sentinels(
        instruction,
        account_handles,
        &[],
        optional_sentinel_flags,
    )
}

/// Invoke a cross-program instruction with PDA signer seeds using Anchor v2
/// CPI handles.
pub fn invoke_signed<'a, 'seeds, I>(
    instruction: &I,
    account_handles: &[CpiHandle<'a>],
    signer_seeds: &'seeds [&'seeds [&'seeds [u8]]],
) -> ProgramResult
where
    I: CpiInstruction + ?Sized,
{
    invoke_signed_with_optional_sentinels(instruction, account_handles, signer_seeds, &[])
}

/// Like [`invoke_signed`], with an explicit optional-sentinel mask.
pub fn invoke_signed_with_optional_sentinels<'a, 'seeds, I>(
    instruction: &I,
    account_handles: &[CpiHandle<'a>],
    signer_seeds: &'seeds [&'seeds [&'seeds [u8]]],
    optional_sentinel_flags: &[bool],
) -> ProgramResult
where
    I: CpiInstruction + ?Sized,
{
    instruction.with_instruction_accounts(|instruction_accounts| {
        let program_id = instruction.program_id();
        let data = instruction.data();
        validate_instruction_accounts(
            instruction_accounts,
            program_id,
            account_handles,
            optional_sentinel_flags,
            signer_seeds.is_empty(),
        )?;

        // Keep validation and invocation inside the same callback. Some
        // CpiInstruction implementations build a temporary account slice, so
        // fetching it again after validation could observe a different view.
        unsafe {
            invoke_signed_unchecked_with_accounts(
                program_id,
                data,
                instruction_accounts,
                account_handles,
                signer_seeds,
                optional_sentinel_flags,
            )
        }
    })?;

    Ok(())
}

/// Invoke a cross-program instruction without borrow validation.
///
/// # Safety
///
/// The caller must ensure no live Rust references or stale `AccountView` data
/// borrows can be invalidated by the callee. Prefer [`invoke`] unless this is
/// being used through a higher-level API that already enforces those lifetimes.
pub unsafe fn invoke_unchecked<'a, I>(
    instruction: &I,
    account_handles: &[CpiHandle<'a>],
) -> ProgramResult
where
    I: CpiInstruction + ?Sized,
{
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
pub unsafe fn invoke_signed_unchecked<'a, 'seeds, I>(
    instruction: &I,
    account_handles: &[CpiHandle<'a>],
    signer_seeds: &'seeds [&'seeds [&'seeds [u8]]],
) -> ProgramResult
where
    I: CpiInstruction + ?Sized,
{
    unsafe {
        invoke_signed_unchecked_with_optional_sentinels(
            instruction,
            account_handles,
            signer_seeds,
            &[],
        )
    }
}

/// Like [`invoke_signed_unchecked`], with an explicit optional-sentinel mask.
///
/// # Safety
///
/// Same as [`invoke_signed_unchecked`].
pub unsafe fn invoke_signed_unchecked_with_optional_sentinels<'a, 'seeds, I>(
    instruction: &I,
    account_handles: &[CpiHandle<'a>],
    signer_seeds: &'seeds [&'seeds [&'seeds [u8]]],
    optional_sentinel_flags: &[bool],
) -> ProgramResult
where
    I: CpiInstruction + ?Sized,
{
    instruction.with_instruction_accounts(|instruction_accounts| unsafe {
        invoke_signed_unchecked_with_accounts(
            instruction.program_id(),
            instruction.data(),
            instruction_accounts,
            account_handles,
            signer_seeds,
            optional_sentinel_flags,
        )
    })
}

unsafe fn invoke_signed_unchecked_with_accounts<'a, 'seeds>(
    program_id: &Address,
    data: &[u8],
    instruction_accounts: &[InstructionAccount<'a>],
    account_handles: &[CpiHandle<'a>],
    signer_seeds: &'seeds [&'seeds [&'seeds [u8]]],
    optional_sentinel_flags: &[bool],
) -> ProgramResult {
    let cpi_account_count = required_cpi_account_count(
        instruction_accounts,
        program_id,
        account_handles,
        optional_sentinel_flags,
    )?;
    let instruction_view = InstructionView {
        program_id,
        accounts: instruction_accounts,
        data,
    };
    let signers = signers(signer_seeds);
    let _borrow_guards = crate::enter_cpi(account_handles);
    let cpi_accounts = cpi_accounts(account_handles);

    // SAFETY:
    // - `cpi_accounts` was fully initialized from the provided handles.
    // - This function's caller upholds the unchecked CPI aliasing contract.
    unsafe {
        pinocchio::cpi::invoke_signed_unchecked(
            &instruction_view,
            from_raw_parts(
                cpi_accounts.as_ptr() as *const CpiAccount,
                cpi_account_count,
            ),
            &signers,
        );
    }

    Ok(())
}

pub(crate) fn validate_fixed_instruction_accounts<'a, const N: usize>(
    instruction_accounts: &[InstructionAccount<'a>; N],
    handles: &[CpiHandle<'a>; N],
    enforce_signers: bool,
) -> ProgramResult {
    for (account, handle) in instruction_accounts.iter().zip(handles) {
        require!(
            address_eq(account.address, handle.address())
                && (!account.is_writable || handle.is_writable()),
            ProgramError::InvalidArgument
        );

        if handle.requires_borrow_check() {
            if account.is_writable {
                handle.account_view().check_borrow_mut()?;
            } else {
                handle.account_view().check_borrow()?;
            }
        }

        if enforce_signers && account.is_signer {
            require!(
                handle.account_view().is_signer(),
                ProgramError::MissingRequiredSignature
            );
        }
    }

    Ok(())
}

pub(crate) fn validate_instruction_accounts<'a>(
    instruction_accounts: &[InstructionAccount<'a>],
    program_id: &Address,
    account_handles: &[CpiHandle<'a>],
    optional_sentinel_flags: &[bool],
    enforce_signers: bool,
) -> ProgramResult {
    walk_instruction_account_bindings(
        instruction_accounts,
        program_id,
        account_handles,
        optional_sentinel_flags,
        |_, account, binding| {
            let Some(handle_index) = binding else {
                return Ok(());
            };
            let handle = &account_handles[handle_index];

            if account.is_writable {
                if handle.requires_borrow_check() {
                    handle.account_view().check_borrow_mut()?;
                }
            } else if handle.requires_borrow_check() {
                handle.account_view().check_borrow()?;
            }

            if enforce_signers && account.is_signer {
                require!(
                    handle.account_view().is_signer(),
                    ProgramError::MissingRequiredSignature
                );
            }
            Ok(())
        },
    )
}

fn is_optional_instruction_account_sentinel_candidate(
    program_id: &Address,
    account: &InstructionAccount<'_>,
) -> bool {
    !account.is_writable && !account.is_signer && address_eq(account.address, program_id)
}

fn is_skippable_optional_sentinel(
    program_id: &Address,
    account: &InstructionAccount<'_>,
    optional_sentinel_flags: &[bool],
    instruction_index: usize,
) -> bool {
    optional_sentinel_flags
        .get(instruction_index)
        .copied()
        .unwrap_or(false)
        && is_optional_instruction_account_sentinel_candidate(program_id, account)
}

fn required_cpi_account_count<'a>(
    instruction_accounts: &[InstructionAccount<'a>],
    program_id: &Address,
    account_handles: &[CpiHandle<'a>],
    optional_sentinel_flags: &[bool],
) -> Result<usize, ProgramError> {
    let mut count = 0;
    walk_instruction_account_bindings(
        instruction_accounts,
        program_id,
        account_handles,
        optional_sentinel_flags,
        |_, _, binding| {
            count += binding.is_some() as usize;
            Ok(())
        },
    )?;
    Ok(count)
}

fn walk_instruction_account_bindings<'a, F>(
    instruction_accounts: &[InstructionAccount<'a>],
    program_id: &Address,
    account_handles: &[CpiHandle<'a>],
    optional_sentinel_flags: &[bool],
    mut visit: F,
) -> Result<(), ProgramError>
where
    F: FnMut(usize, &InstructionAccount<'a>, Option<usize>) -> Result<(), ProgramError>,
{
    let mut handle_index = 0;

    for (instruction_index, account) in instruction_accounts.iter().enumerate() {
        let is_optional = is_skippable_optional_sentinel(
            program_id,
            account,
            optional_sentinel_flags,
            instruction_index,
        );

        let can_consume = handle_index < account_handles.len()
            && instruction_account_matches_handle(account, &account_handles[handle_index])
            && (!is_optional
                || remaining_handles_can_satisfy_required_accounts(
                    &instruction_accounts[instruction_index + 1..],
                    program_id,
                    optional_sentinel_flags,
                instruction_index + 1,
                    account_handles.len().saturating_sub(handle_index + 1),
                ));
        if can_consume {
            visit(instruction_index, account, Some(handle_index))?;
            handle_index += 1;
            continue;
        }

        if is_optional {
            visit(instruction_index, account, None)?;
            continue;
        }

        return Err(if account_handles.len()
            < minimum_required_handle_count(
                instruction_accounts,
                program_id,
                optional_sentinel_flags,
            ) {
            ProgramError::NotEnoughAccountKeys
        } else {
            ProgramError::InvalidArgument
        });
    }

    Ok(())
}

fn remaining_handles_can_satisfy_required_accounts(
    instruction_accounts: &[InstructionAccount<'_>],
    program_id: &Address,
    optional_sentinel_flags: &[bool],
    instruction_offset: usize,
    handle_count: usize,
) -> bool {
    handle_count
        >= instruction_accounts
            .iter()
            .enumerate()
            .filter(|(index, account)| {
                !is_skippable_optional_sentinel(
                    program_id,
                    account,
                    optional_sentinel_flags,
                    instruction_offset + index,
                )
            })
            .count()
}

fn minimum_required_handle_count(
    instruction_accounts: &[InstructionAccount<'_>],
    program_id: &Address,
    optional_sentinel_flags: &[bool],
) -> usize {
    instruction_accounts
        .iter()
        .enumerate()
        .filter(|(index, account)| {
            !is_skippable_optional_sentinel(program_id, account, optional_sentinel_flags, *index)
        })
        .count()
}

fn instruction_account_matches_handle(
    account: &InstructionAccount<'_>,
    handle: &CpiHandle<'_>,
) -> bool {
    address_eq(account.address, handle.address()) && (!account.is_writable || handle.is_writable())
}

fn signers<'seeds, const N: usize>(
    signer_seeds: &'seeds [&'seeds [&'seeds [u8]]],
) -> Result<crate::SignerBuffer<'seeds, N>, ProgramError> {
    let mut signers = crate::SignerBuffer::new();
    for seeds in signer_seeds {
        // SAFETY: `Seed` has the same in-memory representation as `&[u8]`;
        // this is the conversion used by `CpiContext::invoke` as well.
        let cpi_seeds: &[Seed] =
            unsafe { from_raw_parts(seeds.as_ptr() as *const Seed, seeds.len()) };
        signers
            .try_push(Signer::from(cpi_seeds))
            .map_err(|_| ProgramError::InvalidArgument)?;
    }
    Ok(signers)
}

pub(crate) fn cpi_accounts<'a, const N: usize>(
    account_handles: &[CpiHandle<'a>],
) -> Result<crate::CpiAccountBuffer<'a, N>, ProgramError> {
    let mut accounts = crate::CpiAccountBuffer::new();
    for handle in account_handles {
        let mut slot = MaybeUninit::uninit();
        CpiAccount::init_from_account_view(handle.account_view(), &mut slot);
        accounts
            .try_push(slot)
            .map_err(|_| ProgramError::InvalidArgument)?;
    }

    Ok(accounts)
}
