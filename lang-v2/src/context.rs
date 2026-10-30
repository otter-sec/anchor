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
    /// After `remaining_accounts` is called this holds the range of views
    /// already written to the cursor's lookup table.
    remaining_accounts: RemainingAccounts,

    /// The instruction's account lookup table. Remaining accounts are exposed
    /// as a borrowed slice into this table instead of being copied into a
    /// second fixed-capacity buffer.
    cursor: &'a mut AccountCursor,
}

enum RemainingAccounts {
    Unparsed {
        /// Number of accounts remaining after the initially declared region
        remaining: u8,
    },
    /// Cached range of views in the cursor's lookup table.
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

    /// Returns trailing accounts beyond the declared `T` fields as a borrowed
    /// slice into the instruction's account lookup table. The first call
    /// walks the cursor and caches the resulting range; subsequent calls
    /// replay that range.
    pub fn remaining_accounts(&mut self) -> &'a [AccountView] {
        if let RemainingAccounts::Unparsed { .. } = self.remaining_accounts {
            let (start, len) = self.walk_remaining();
            self.remaining_accounts = RemainingAccounts::Cached { start, len };
        }

        match &self.remaining_accounts {
            RemainingAccounts::Cached { start, len } => {
                // SAFETY: the dispatcher-owned lookup table lives for `'a`,
                // which is also the lifetime of this instruction context.
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
            // region and `remaining_num` is the exact number of accounts
            // to walk.
            unsafe { self.cursor.next() };
        }
        (start, remaining)
    }
}

/// Trait linking an accounts struct to its generated bumps struct.
pub trait Bumps {
    type Bumps;
}
