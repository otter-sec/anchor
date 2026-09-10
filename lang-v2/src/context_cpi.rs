extern crate alloc;

use {
    crate::{address_eq, require, CpiHandle, ToCpiAccounts},
    alloc::vec::Vec,
    core::mem::MaybeUninit,
    pinocchio::{
        address::Address,
        instruction::{InstructionAccount, InstructionView},
    },
    solana_instruction::{AccountMeta, Instruction},
    solana_program_error::{ProgramError, ProgramResult},
};

/// Context for cross-program invocations.
///
/// Bundles a typed CPI accounts struct with the target program, optional
/// PDA signer seeds, and optional remaining accounts.
///
/// # Example
///
/// ```ignore
/// let cpi_accounts = callee::cpi::accounts::SetData {
///     data_acc: ctx.accounts.data_acc.cpi_handle_mut(),
///     authority: ctx.accounts.authority.cpi_handle(),
/// };
/// let cpi_ctx = CpiContext::new(ctx.accounts.callee.address(), cpi_accounts);
/// callee::cpi::set_data(cpi_ctx, data)?;
/// ```
pub struct CpiContext<'a, T: ToCpiAccounts<'a>> {
    pub accounts: T,
    pub remaining_accounts: Vec<CpiHandle<'a>>,
    pub program: &'a Address,
    pub signer_seeds: &'a [&'a [&'a [u8]]],
}

impl<'a, T: ToCpiAccounts<'a>> CpiContext<'a, T> {
    #[must_use]
    pub fn new(program: &'a Address, accounts: T) -> Self {
        Self {
            accounts,
            program,
            remaining_accounts: Vec::new(),
            signer_seeds: &[],
        }
    }

    #[must_use]
    pub fn new_with_signer(
        program: &'a Address,
        accounts: T,
        signer_seeds: &'a [&'a [&'a [u8]]],
    ) -> Self {
        Self {
            accounts,
            program,
            remaining_accounts: Vec::new(),
            signer_seeds,
        }
    }

    #[must_use]
    pub fn with_signer(mut self, signer_seeds: &'a [&'a [&'a [u8]]]) -> Self {
        self.signer_seeds = signer_seeds;
        self
    }

    #[must_use]
    pub fn with_remaining_accounts(mut self, ra: Vec<CpiHandle<'a>>) -> Self {
        self.remaining_accounts = ra;
        self
    }

    /// Invoke the CPI with the given instruction data. Collects accounts
    /// from [`ToCpiAccounts`], appends remaining accounts, validates borrow
    /// state, then calls `invoke_signed_unchecked`.
    pub fn invoke(&self, data: &[u8]) -> ProgramResult {
        let mut instruction_accounts = self.accounts.to_instruction_accounts();
        let mut handles = self.accounts.to_cpi_handles();
        let mut optional_sentinel_flags = self.accounts.optional_account_sentinel_flags();

        // Append remaining accounts using the handle's writable/signer flags
        // so PDA remaining accounts marked with `as_signer()` emit signer metas.
        for handle in &self.remaining_accounts {
            instruction_accounts.push(InstructionAccount::new(
                handle.address(),
                handle.is_writable(),
                handle.is_signer(),
            ));
            handles.push(*handle);
            optional_sentinel_flags.push(false);
        }

        crate::program::validate_instruction_accounts(
            &instruction_accounts,
            self.program,
            &handles,
            &optional_sentinel_flags,
            self.signer_seeds.is_empty(),
        )?;

        let instruction = InstructionView {
            program_id: self.program,
            data,
            accounts: &instruction_accounts,
        };

        let _borrow_guards = crate::enter_cpi(&handles);

        // Convert signer seeds to pinocchio Signers.
        // SAFETY: pinocchio::cpi::Seed is repr(C) { *const u8, u64, PhantomData }
        // which has the same layout as &[u8] on SBF. This is verified by the
        // static assertion in cpi.rs.
        let signers: Vec<pinocchio::cpi::Signer> = self
            .signer_seeds
            .iter()
            .map(|seeds| {
                let cpi_seeds: &[pinocchio::cpi::Seed] = unsafe {
                    core::slice::from_raw_parts(
                        seeds.as_ptr() as *const pinocchio::cpi::Seed,
                        seeds.len(),
                    )
                };
                pinocchio::cpi::Signer::from(cpi_seeds)
            })
            .collect();

        // Build CpiAccounts and invoke. Account count matches handles (optional
        // None sentinels have metas but no handles); pinocchio skips program-id
        // placeholder metas that lack a paired CpiAccount.
        let n = handles.len();
        let mut cpi_accounts: Vec<core::mem::MaybeUninit<pinocchio::cpi::CpiAccount>> =
            Vec::with_capacity(n);
        // SAFETY: MaybeUninit does not require initialization.
        unsafe { cpi_accounts.set_len(n) };

        for (handle, slot) in handles.iter().zip(cpi_accounts.iter_mut()) {
            pinocchio::cpi::CpiAccount::init_from_account_view(handle.account_view(), slot);
        }

        // SAFETY:
        // - All CpiAccounts initialized by init_from_account_view above.
        // - CpiHandles hold Rust borrows preventing typed data access.
        // - Mutable handles carry the borrow state established by their
        //   account wrappers, blocking conflicting raw borrows on stale
        //   AccountView copies.
        unsafe {
            pinocchio::cpi::invoke_signed_unchecked(
                &instruction,
                core::slice::from_raw_parts(
                    cpi_accounts.as_ptr() as *const pinocchio::cpi::CpiAccount,
                    n,
                ),
                &signers,
            );
        }

        Ok(())
    }

    /// Invoke a fully built instruction using this context's CPI handles.
    ///
    /// The instruction's program id must match this context's program. Account
    /// metas for the fixed [`ToCpiAccounts`] fields are taken from the
    /// instruction as supplied. Any metas the instruction carries beyond that
    /// prefix are treated as builder-supplied metas for the leading
    /// `remaining_accounts`, in order. `metas` are appended only for the
    /// remaining handles the instruction does not already cover, so metas and
    /// handles stay paired one-to-one.
    ///
    /// Optional `None` sentinel slots must appear in the same positions as in
    /// [`ToCpiAccounts::to_instruction_accounts`]; only those indices may omit
    /// handles for readonly program-id metas.
    pub fn invoke_ix(&self, mut ix: Instruction) -> ProgramResult {
        require!(
            address_eq(self.program, &ix.program_id),
            ProgramError::IncorrectProgramId
        );

        let mut handles = self.accounts.to_cpi_handles();

        let mut optional_sentinel_flags = self.accounts.optional_account_sentinel_flags();
        require!(
            optional_sentinel_flags.len() <= ix.accounts.len(),
            ProgramError::InvalidArgument
        );

        // Metas past the fixed prefix already cover the leading remaining
        // accounts, in order. Append only the rest, keeping metas and handles
        // paired one-to-one.
        let prebuilt_remaining_metas = ix.accounts.len() - optional_sentinel_flags.len();
        require!(
            prebuilt_remaining_metas <= self.remaining_accounts.len(),
            ProgramError::NotEnoughAccountKeys
        );
        for handle in &self.remaining_accounts[prebuilt_remaining_metas..] {
            ix.accounts.push(if handle.is_writable() {
                AccountMeta::new(*handle.address(), handle.is_signer())
            } else {
                AccountMeta::new_readonly(*handle.address(), handle.is_signer())
            });
        }
        handles.extend(self.remaining_accounts.iter().copied());
        // Trailing metas (e.g. remaining accounts already baked into `ix`) are
        // never treated as optional sentinels from this accounts struct.
        optional_sentinel_flags.resize(ix.accounts.len(), false);

        crate::program::invoke_signed_with_optional_sentinels(
            &ix,
            &handles,
            self.signer_seeds,
            &optional_sentinel_flags,
        )
    }
}

#[inline(always)]
fn signer_from_seeds<'a>(seeds: &'a [&'a [u8]]) -> pinocchio::cpi::Signer<'a, 'a> {
    // SAFETY: pinocchio::cpi::Seed is repr(C) { *const u8, u64, PhantomData }
    // which has the same layout as &[u8] on SBF. This is verified by the
    // static assertion in cpi.rs.
    let cpi_seeds: &[pinocchio::cpi::Seed] = unsafe {
        core::slice::from_raw_parts(seeds.as_ptr() as *const pinocchio::cpi::Seed, seeds.len())
    };
    pinocchio::cpi::Signer::from(cpi_seeds)
}

/// Stack-backed fast path for fixed-account CPIs.
///
/// This preserves the same [`CpiHandle`] safety model as [`CpiContext::invoke`]
/// but avoids heap-allocating account metadata and `CpiAccount` buffers for
/// common SPL instructions with a static account list.
#[inline(always)]
pub fn unchecked_invoke_signed_fixed<'a, const N: usize>(
    program: &'a Address,
    data: &[u8],
    instruction_accounts: &[InstructionAccount<'a>; N],
    handles: &[CpiHandle<'a>; N],
    signer_seeds: &'a [&'a [&'a [u8]]],
) {
    let instruction = InstructionView {
        program_id: program,
        data,
        accounts: instruction_accounts,
    };

    let _borrow_guards = crate::enter_cpi(handles);

    let mut cpi_accounts = [const { MaybeUninit::<pinocchio::cpi::CpiAccount>::uninit() }; N];
    for (handle, slot) in handles.iter().zip(cpi_accounts.iter_mut()) {
        pinocchio::cpi::CpiAccount::init_from_account_view(handle.account_view(), slot);
    }
    let cpi_accounts = unsafe {
        core::slice::from_raw_parts(
            cpi_accounts.as_ptr() as *const pinocchio::cpi::CpiAccount,
            N,
        )
    };

    match signer_seeds {
        [] => unsafe {
            pinocchio::cpi::invoke_signed_unchecked(&instruction, cpi_accounts, &[]);
        },
        [seeds] => {
            let signer = signer_from_seeds(seeds);
            unsafe {
                pinocchio::cpi::invoke_signed_unchecked(&instruction, cpi_accounts, &[signer]);
            }
        }
        _ => {
            let signers: Vec<pinocchio::cpi::Signer<'a, 'a>> = signer_seeds
                .iter()
                .map(|seeds| signer_from_seeds(seeds))
                .collect();
            unsafe {
                pinocchio::cpi::invoke_signed_unchecked(&instruction, cpi_accounts, &signers);
            }
        }
    }
}
