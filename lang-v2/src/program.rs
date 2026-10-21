pub use pinocchio::cpi::{set_return_data, MAX_RETURN_DATA};
use {
    crate::{address_eq, require, Address, CpiHandle, CpiInstruction, Vec},
    core::{mem::MaybeUninit, slice::from_raw_parts},
    pinocchio::{
        cpi::{CpiAccount, Seed, Signer},
        instruction::{InstructionAccount, InstructionView},
    },
    solana_program_error::{ProgramError, ProgramResult},
};

/// Get the return data from an invoked program.
///
/// The returned address identifies the program that most recently set the
/// data. Callers should check it against the expected program ID.
pub fn get_return_data() -> Option<(Address, Vec<u8>)> {
    #[cfg(any(target_os = "solana", target_arch = "bpf"))]
    {
        // We *could* call `sol_get_return_data` with 0 length to get the real required
        // size instead of overallocating. However, we'd pay the base syscall cost
        // to save on heap memory which Anchor uses sparingly.
        let mut data = Vec::<u8>::with_capacity(MAX_RETURN_DATA);
        let mut program_id = MaybeUninit::<Address>::uninit();
        // SAFETY: Both pointers are valid writable storage for the syscall's outputs.
        let size = unsafe {
            pinocchio::cpi::sol_get_return_data(
                data.as_mut_ptr(),
                MAX_RETURN_DATA as u64,
                program_id.as_mut_ptr().cast(),
            )
        };
        if size == 0 {
            return None;
        }

        // SAFETY: A nonzero result initializes the program ID and the payload
        // prefix, up to the capacity passed to the syscall.
        // As Pinocchio's bump allocator does not reclaim memory, there is no need
        // to shrink the allocation to fit here.
        unsafe {
            data.set_len(size.min(MAX_RETURN_DATA as u64) as usize);
            Some((program_id.assume_init(), data))
        }
    }

    #[cfg(not(any(target_os = "solana", target_arch = "bpf")))]
    core::hint::black_box(None)
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
/// [`crate::ToCpiAccounts::optional_account_sentinel_flags`]).
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
    let bindings = resolve_instruction_account_bindings(
        instruction_accounts,
        program_id,
        account_handles,
        optional_sentinel_flags,
    )?;

    for (account, binding) in instruction_accounts.iter().zip(bindings) {
        let Some(handle_index) = binding else {
            continue;
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
    }

    Ok(())
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
    Ok(resolve_instruction_account_bindings(
        instruction_accounts,
        program_id,
        account_handles,
        optional_sentinel_flags,
    )?
    .into_iter()
    .flatten()
    .count())
}

fn resolve_instruction_account_bindings<'a>(
    instruction_accounts: &[InstructionAccount<'a>],
    program_id: &Address,
    account_handles: &[CpiHandle<'a>],
    optional_sentinel_flags: &[bool],
) -> Result<Vec<Option<usize>>, ProgramError> {
    let cols = account_handles.len() + 1;
    let rows = instruction_accounts.len() + 1;
    let count = rows
        .checked_mul(cols)
        .ok_or(ProgramError::InvalidArgument)?;
    let mut can_match = Vec::new();
    can_match.try_resize(count, false)?;

    for handle_index in 0..=account_handles.len() {
        can_match[(rows - 1) * cols + handle_index] = true;
    }

    for instruction_index in (0..instruction_accounts.len()).rev() {
        let account = &instruction_accounts[instruction_index];
        for handle_index in (0..=account_handles.len()).rev() {
            let can_consume = handle_index < account_handles.len()
                && instruction_account_matches_handle(account, &account_handles[handle_index])
                && can_match[(instruction_index + 1) * cols + (handle_index + 1)];
            let can_skip = is_skippable_optional_sentinel(
                program_id,
                account,
                optional_sentinel_flags,
                instruction_index,
            ) && can_match[(instruction_index + 1) * cols + handle_index];
            can_match[instruction_index * cols + handle_index] = can_consume || can_skip;
        }
    }

    require!(
        can_match[0],
        if account_handles.len()
            < minimum_required_handle_count(
                instruction_accounts,
                program_id,
                optional_sentinel_flags
            )
        {
            ProgramError::NotEnoughAccountKeys
        } else {
            ProgramError::InvalidArgument
        }
    );

    let mut bindings = Vec::with_capacity(instruction_accounts.len());
    let mut handle_index = 0;

    for (instruction_index, account) in instruction_accounts.iter().enumerate() {
        // Prefer skipping intentional Option::None sentinels so a later
        // required program-id account can still consume a matching handle.
        let can_skip = is_skippable_optional_sentinel(
            program_id,
            account,
            optional_sentinel_flags,
            instruction_index,
        ) && can_match[(instruction_index + 1) * cols + handle_index];
        if can_skip {
            bindings.push(None);
            continue;
        }

        let can_consume = handle_index < account_handles.len()
            && instruction_account_matches_handle(account, &account_handles[handle_index])
            && can_match[(instruction_index + 1) * cols + (handle_index + 1)];
        if can_consume {
            bindings.push(Some(handle_index));
            handle_index += 1;
            continue;
        }

        debug_assert!(
            false,
            "validated instruction-account matching must reconstruct"
        );
        bindings.push(None);
    }

    Ok(bindings)
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

pub(crate) fn cpi_accounts<'a>(
    account_handles: &'a [CpiHandle<'a>],
) -> Vec<MaybeUninit<CpiAccount<'a>>> {
    account_handles
        .iter()
        .map(|handle| {
            let mut account = MaybeUninit::uninit();
            CpiAccount::init_from_account_view(handle.account_view(), &mut account);
            account
        })
        .collect()
}
