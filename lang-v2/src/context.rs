use {
    crate::cursor::AccountCursor,
    pinocchio::{account::AccountView, address::Address},
    solana_program_error::ProgramError,
};

/// Instruction-scoped context passed to every handler. Holds the
/// declared accounts, program_id, PDA bumps, and a cursor for lazy
/// `remaining_accounts()` access.
pub struct Context<'a, T: Bumps> {
    /// Program id as a reference — lives for the whole instruction
    /// since it comes from the entrypoint's input buffer.
    pub program_id: &'a Address,

    /// Declared accounts (the `#[derive(Accounts)]` struct).
    pub accounts: T,

    /// Bump seeds found during constraint validation. Provided as a
    /// convenience so handlers don't have to recalculate bump seeds or
    /// pass them in as arguments.
    pub bumps: T::Bumps,

    /// Cursor into the serialized input buffer, positioned at the
    /// *start* of the remaining-accounts region (after `try_accounts`
    /// has consumed exactly `T::HEADER_SIZE` declared accounts). Used
    /// by [`Self::remaining_accounts`] for on-demand walking.
    cursor: &'a mut AccountCursor,

    /// Number of accounts after the declared region. Decremented to
    /// zero once the remaining region is walked.
    remaining_num: u8,

    /// Mutable-account mask covering the declared region (from
    /// `T::MUT_MASK`). Used by [`Self::remaining_accounts`] to
    /// re-check each trailing account against declared mut slots —
    /// without this, a trailing account whose dup index points at a
    /// mut declared account would silently alias it (bug 3: the
    /// `HEADER_SIZE`-only check in `run_handler` can't see dups that
    /// only surface during the trailing walk).
    mut_mask: &'static [u64; 4],

    /// Cached outcome of the remaining-accounts walk. `None` until the
    /// first call to [`Self::remaining_accounts`]; then stores whichever
    /// `Result` that walk produced. Subsequent calls replay the cache
    /// (clone of the vec on success, clone of the error on failure) so
    /// the cursor is advanced exactly once per instruction — retrying
    /// after an error must not call `cursor.next()` again, which would
    /// walk past the remaining region.
    remaining_cache: Option<Result<alloc::vec::Vec<AccountView>, ProgramError>>,
}

impl<'a, T: Bumps> Context<'a, T> {
    #[inline(always)]
    pub fn new(
        program_id: &'a Address,
        accounts: T,
        bumps: T::Bumps,
        cursor: &'a mut AccountCursor,
        remaining_num: u8,
        mut_mask: &'static [u64; 4],
    ) -> Self {
        Self {
            program_id,
            accounts,
            bumps,
            cursor,
            remaining_num,
            mut_mask,
            remaining_cache: None,
        }
    }

    /// Returns trailing accounts beyond the declared `T` fields as an
    /// owned `Vec<AccountView>`. First call walks the cursor and caches
    /// the resulting `Result`; subsequent calls replay the cache (clone
    /// of the vec on success, clone of the error on failure). Caching
    /// the error is important: the walk advances an unsafe cursor, and
    /// a handler that calls this again after an error must not trigger
    /// another `cursor.next()` loop.
    ///
    /// After each cursor advance, re-tests the cursor's duplicate bitvec
    /// against `T::MUT_MASK`. If a trailing account's dup index resolves
    /// to a declared mut slot, returns
    /// `ConstraintDuplicateMutableAccount`. The `HEADER_SIZE`-only check
    /// in `run_handler` only sees duplicates that existed at the end of
    /// the declared walk; trailing-region dups can only be caught here.
    ///
    /// `MUT_MASK` is sized per declared field, so bits set for trailing
    /// indices (past `HEADER_SIZE`) are naturally zero — the intersect
    /// only fires when a trailing slot's bit overlaps with a declared
    /// mut slot's bit, which by construction means the runtime resolved
    /// the trailing slot as a dup of that declared mut account.
    pub fn remaining_accounts(&mut self) -> Result<alloc::vec::Vec<AccountView>, ProgramError> {
        if self.remaining_cache.is_none() {
            self.remaining_cache = Some(self.walk_remaining());
        }
        match self.remaining_cache.as_ref().unwrap() {
            Ok(v) => Ok(v.clone()),
            Err(e) => Err(e.clone()),
        }
    }

    fn walk_remaining(&mut self) -> Result<alloc::vec::Vec<AccountView>, ProgramError> {
        let mut v = alloc::vec::Vec::with_capacity(self.remaining_num as usize);
        for _ in 0..self.remaining_num {
            // SAFETY: cursor is positioned at the start of the remaining
            // region and `remaining_num` is the exact number of accounts
            // to walk. Walking stops on the first mut-alias error so we
            // never advance past the remaining region.
            v.push(unsafe { self.cursor.next() });
            if let Some(dups) = self.cursor.duplicates() {
                if dups.intersects(self.mut_mask) {
                    return Err(crate::ErrorCode::ConstraintDuplicateMutableAccount.into());
                }
            }
        }
        Ok(v)
    }
}

/// Trait linking an accounts struct to its generated bumps struct.
pub trait Bumps {
    type Bumps;
}
