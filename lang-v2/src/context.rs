use {
    crate::cursor::AccountCursor,
    pinocchio::{account::AccountView, address::Address},
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

    /// Holds either a cursor into the serialized input buffer, pointing to the
    /// *start* of the remaining-accounts region (after `try_accounts`
    /// has consumed exactly `T::HEADER_SIZE` declared accounts). Used
    /// by [`Self::remaining_accounts`] for on-demand walking.
    /// After `remaining_accounts` is called this holds a cache of the result.
    remaining_accounts: RemainingAccounts<'a>,
}

enum RemainingAccounts<'a> {
    Unparsed {
        /// Points to the `remaining-accounts` region
        cursor: &'a mut AccountCursor,
        /// Number of accounts remaining after the initially declared region
        remaining: u8,
    },
    /// Cached result of walking `remaining-accounts`.
    Cached(alloc::vec::Vec<AccountView>),
}

impl<'a, T: Bumps> Context<'a, T> {
    #[inline(always)]
    pub fn new(
        program_id: &'a Address,
        accounts: T,
        bumps: T::Bumps,
        cursor: &'a mut AccountCursor,
        remaining_num: u8,
    ) -> Self {
        Self {
            program_id,
            accounts,
            bumps,
            remaining_accounts: RemainingAccounts::Unparsed {
                cursor,
                remaining: remaining_num,
            },
        }
    }

    /// Returns trailing accounts beyond the declared `T` fields as an
    /// owned `Vec<AccountView>`. First call walks the cursor and caches
    /// the views; subsequent calls return a clone of the cache.
    ///
    /// A trailing account can alias a declared account. The views are raw,
    /// so aliasing is only rejected when a conflicting borrow is taken:
    /// loading a view through an account wrapper, borrowing its data, or
    /// passing it to a CPI fails while a declared wrapper holds a
    /// conflicting borrow of the same account.
    pub fn remaining_accounts(&mut self) -> alloc::vec::Vec<AccountView> {
        if let RemainingAccounts::Unparsed { cursor, remaining } = &mut self.remaining_accounts {
            let mut v = alloc::vec::Vec::with_capacity(*remaining as usize);
            for _ in 0..*remaining {
                // SAFETY: cursor is positioned at the start of the remaining
                // region and `remaining` is the exact number of accounts to
                // walk.
                v.push(unsafe { cursor.next() });
            }
            self.remaining_accounts = RemainingAccounts::Cached(v);
        }

        match &self.remaining_accounts {
            RemainingAccounts::Cached(accs) => accs.clone(),
            RemainingAccounts::Unparsed { .. } => unreachable!(),
        }
    }
}

/// Trait linking an accounts struct to its generated bumps struct.
pub trait Bumps {
    type Bumps;
}
