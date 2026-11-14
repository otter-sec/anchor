pub use pinocchio::cpi::{set_return_data, MAX_RETURN_DATA};
use {
    crate::{address_eq, require, Address, CpiHandle, CpiInstruction},
    core::{mem::MaybeUninit, ops::Range, slice::from_raw_parts},
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
    invoke_signed_with_optional_sentinels_with_capacity::<{ pinocchio::MAX_TX_ACCOUNTS }, I>(
        instruction,
        account_handles,
        signer_seeds,
        optional_sentinel_flags,
    )
}

/// Capacity-aware implementation used by [`crate::CpiContext`]. The public
/// slice-based compatibility functions retain their protocol-sized fallback,
/// while generated CPI calls use the capacity selected by their context.
#[doc(hidden)]
pub(crate) fn invoke_signed_with_optional_sentinels_with_capacity<'a, 'seeds, const N: usize, I>(
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
        let cpi_account_count = validate_instruction_accounts::<N>(
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
            invoke_signed_unchecked_with_accounts::<N>(
                program_id,
                data,
                instruction_accounts,
                account_handles,
                signer_seeds,
                cpi_account_count,
            )
        }
    })
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
    unsafe {
        invoke_signed_unchecked_with_optional_sentinels_with_capacity::<
            { pinocchio::MAX_TX_ACCOUNTS },
            I,
        >(
            instruction,
            account_handles,
            signer_seeds,
            optional_sentinel_flags,
        )
    }
}

unsafe fn invoke_signed_unchecked_with_optional_sentinels_with_capacity<
    'a,
    'seeds,
    const N: usize,
    I,
>(
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
        let cpi_account_count = walk_instruction_account_bindings::<N>(
            instruction_accounts,
            program_id,
            account_handles,
            optional_sentinel_flags,
            |_, _| Ok(()),
        )?;
        unsafe {
            invoke_signed_unchecked_with_accounts::<N>(
                program_id,
                data,
                instruction_accounts,
                account_handles,
                signer_seeds,
                cpi_account_count,
            )
        }
    })
}

unsafe fn invoke_signed_unchecked_with_accounts<'a, 'seeds, const N: usize>(
    program_id: &Address,
    data: &[u8],
    instruction_accounts: &[InstructionAccount<'a>],
    account_handles: &[CpiHandle<'a>],
    signer_seeds: &'seeds [&'seeds [&'seeds [u8]]],
    cpi_account_count: usize,
) -> ProgramResult {
    let instruction_view = InstructionView {
        program_id,
        accounts: instruction_accounts,
        data,
    };
    let signers = signers::<N>(signer_seeds)?;
    let _borrow_guards = crate::enter_cpi::<N>(account_handles)?;
    let cpi_accounts = cpi_accounts::<N>(account_handles)?;

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

pub(crate) fn validate_instruction_accounts<'a, const N: usize>(
    instruction_accounts: &[InstructionAccount<'a>],
    program_id: &Address,
    account_handles: &[CpiHandle<'a>],
    optional_sentinel_flags: &[bool],
    enforce_signers: bool,
) -> Result<usize, ProgramError> {
    walk_instruction_account_bindings::<N>(
        instruction_accounts,
        program_id,
        account_handles,
        optional_sentinel_flags,
        |account, handle| {
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

#[inline(never)]
fn walk_instruction_account_bindings<'a, const N: usize>(
    instruction_accounts: &[InstructionAccount<'a>],
    program_id: &Address,
    account_handles: &[CpiHandle<'a>],
    optional_sentinel_flags: &[bool],
    mut visit: impl FnMut(&InstructionAccount<'a>, &CpiHandle<'a>) -> ProgramResult,
) -> Result<usize, ProgramError> {
    require!(account_handles.len() <= N, ProgramError::InvalidArgument);
    // A binding consumes at most one handle per instruction account.
    let handles = &account_handles[..account_handles.len().min(instruction_accounts.len())];
    let mut bindings = [0; N];
    let count = required_cpi_account_count(
        instruction_accounts,
        program_id,
        handles,
        optional_sentinel_flags,
        0..instruction_accounts.len(),
        &mut bindings[..handles.len()],
        true,
    )
    .ok_or_else(|| {
        if account_handles.len()
            < minimum_required_handle_count(
                instruction_accounts,
                program_id,
                optional_sentinel_flags,
            )
        {
            ProgramError::NotEnoughAccountKeys
        } else {
            ProgramError::InvalidArgument
        }
    })?;

    for (&instruction_index, handle) in bindings[..count].iter().zip(handles) {
        visit(&instruction_accounts[instruction_index], handle)?;
    }

    Ok(count)
}

/// Write the skip-first account bindings and return the number consumed.
/// The output doubles as a rolling DP row.
/// Each nontrivial split fixes the binding of the middle handle and halves
/// the handle range. The two smaller problems visit disjoint account ranges.
/// This takes O(accounts * handles) time, O(handles) scratch, and logarithmic
/// call depth. `bindings` has one slot per handle throughout.
#[inline(never)]
fn required_cpi_account_count<'a>(
    instruction_accounts: &[InstructionAccount<'a>],
    program_id: &Address,
    account_handles: &[CpiHandle<'a>],
    optional_sentinel_flags: &[bool],
    instruction_range: Range<usize>,
    bindings: &mut [usize],
    allow_unused_handles: bool,
) -> Option<usize> {
    // If all optional slots can be skipped, this is already the preferred
    // binding. In particular, ordinary CPI takes a single linear walk.
    'skip_all: {
        let mut matched = 0;
        for index in instruction_range.clone() {
            let account = &instruction_accounts[index];
            if is_skippable_optional_sentinel(program_id, account, optional_sentinel_flags, index) {
                continue;
            }
            let Some(handle) = account_handles.get(matched) else {
                break 'skip_all;
            };
            if !instruction_account_matches_handle(account, handle) {
                break 'skip_all;
            }
            bindings[matched] = index;
            matched += 1;
        }
        if allow_unused_handles || matched == account_handles.len() {
            return Some(matched);
        }
    }

    if account_handles.is_empty()
        || !optional_sentinel_flags
            .get(instruction_range.start..)
            .unwrap_or_default()
            .iter()
            .take(instruction_range.len())
            .any(|&optional| optional)
    {
        return None;
    }

    let middle = account_handles.len() / 2;
    let end = instruction_range.end;
    let unreachable = usize::MAX;
    // A reachable suffix carries either the middle handle's account index
    // or `end + consumed` if the suffix does not bind that handle. Both
    // individual slice lengths are bounded by isize::MAX divided by element
    // size, so the endpoint encoding cannot overflow or equal `unreachable`.
    for (index, state) in bindings.iter_mut().enumerate() {
        *state = if allow_unused_handles {
            end + index
        } else {
            unreachable
        };
    }
    let mut after_last_handle = end + account_handles.len();
    for index in instruction_range.clone().rev() {
        let account = &instruction_accounts[index];
        let optional =
            is_skippable_optional_sentinel(program_id, account, optional_sentinel_flags, index);
        // The next column belongs to the previous row until we overwrite it.
        for (column, handle) in account_handles.iter().enumerate() {
            if optional && bindings[column] != unreachable {
                continue;
            }
            let next = bindings
                .get(column + 1)
                .copied()
                .unwrap_or(after_last_handle);
            bindings[column] =
                if next != unreachable && instruction_account_matches_handle(account, handle) {
                    if column == middle {
                        index
                    } else {
                        next
                    }
                } else {
                    unreachable
                };
        }
        if !optional {
            after_last_handle = unreachable;
        }
    }

    let middle_account = bindings[0];
    if middle_account == unreachable {
        return None;
    }
    if middle_account >= end {
        // The preferred binding leaves the middle handle unused. Rebuild
        // using only the consumed prefix, which is at most half the handles.
        let consumed = middle_account - end;
        return required_cpi_account_count(
            instruction_accounts,
            program_id,
            &account_handles[..consumed],
            optional_sentinel_flags,
            instruction_range,
            &mut bindings[..consumed],
            false,
        );
    }

    let (left, right) = bindings.split_at_mut(middle);
    right[0] = middle_account;
    required_cpi_account_count(
        instruction_accounts,
        program_id,
        &account_handles[..middle],
        optional_sentinel_flags,
        instruction_range.start..middle_account,
        left,
        false,
    )?;
    let right_count = required_cpi_account_count(
        instruction_accounts,
        program_id,
        &account_handles[middle + 1..],
        optional_sentinel_flags,
        middle_account + 1..end,
        &mut right[1..],
        allow_unused_handles,
    )?;
    Some(middle + 1 + right_count)
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
