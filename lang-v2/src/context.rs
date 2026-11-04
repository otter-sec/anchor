use {
    crate::cursor::AccountCursor,
    pinocchio::{account::AccountView, address::Address},
};

/// Instruction-scoped context passed to every handler. Holds the
/// declared accounts, program_id, PDA bumps, and access to trailing accounts.
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
    /// After `remaining_accounts` is called this holds the range cached in the
    /// cursor's dispatcher-owned lookup storage.
    remaining_accounts: RemainingAccounts,

    /// Cursor for the serialized instruction account region.
    cursor: &'a mut AccountCursor,
}

enum RemainingAccounts {
    Unparsed {
        /// Number of accounts remaining after the initially declared region
        remaining: u8,
    },
    /// Cached range of views in the cursor's lookup storage.
    Cached { start: u8, len: u8 },
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
                remaining: remaining_num,
            },
            cursor,
        }
    }

    /// Returns trailing accounts beyond the declared `T` fields.
    ///
    /// A trailing account can alias a declared account. The views are raw,
    /// so aliasing is only rejected when a conflicting borrow is taken:
    /// loading a view through an account wrapper, borrowing its data, or
    /// passing it to a CPI fails while a declared wrapper holds a
    /// conflicting borrow of the same account.
    /// The returned lifetime is tied to the instruction input, not the
    /// mutable borrow of `Context`, so callers can inspect trailing accounts
    /// and then update their declared accounts in the same handler.
    pub fn remaining_accounts(&mut self) -> &'a [AccountView] {
        if let RemainingAccounts::Unparsed { .. } = self.remaining_accounts {
            let (start, len) = self.walk_remaining();
            self.remaining_accounts = RemainingAccounts::Cached { start, len };
        }

        match &self.remaining_accounts {
            RemainingAccounts::Cached { start, len } => {
                // SAFETY: the dispatcher-owned lookup storage remains valid
                // for the duration of this instruction and all entries in the
                // range were populated by `walk_remaining`.
                unsafe { self.cursor.views(*start, *len) }
            }
            RemainingAccounts::Unparsed { .. } => unreachable!(),
        }
    }

    fn walk_remaining(&mut self) -> (u8, u8) {
        let remaining = match self.remaining_accounts {
            RemainingAccounts::Unparsed { remaining } => remaining,
            RemainingAccounts::Cached { .. } => unreachable!(),
        };
        let start = self.cursor.consumed();
        for _ in 0..remaining {
            // SAFETY: cursor is positioned at the start of the remaining
            // region and `remaining` is the exact number of accounts to walk.
            unsafe { self.cursor.next() };
        }
        (start, remaining)
    }
}

/// Trait linking an accounts struct to its generated bumps struct.
pub trait Bumps {
    type Bumps;
}
