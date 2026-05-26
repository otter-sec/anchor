use {
    crate::{
        cursor::{AccountBitvec, AccountCursor},
        AnchorAccount,
    },
    pinocchio::account::AccountView,
    solana_program_error::ProgramError,
};

/// Sequential account loader for `#[derive(Accounts)]`.
///
/// Thin wrapper around an [`AccountCursor`]. The dispatcher bulk-walks the
/// declared account region with [`Self::walk_n`], and specialized init paths
/// can still walk individual accounts through `next_view` / `load_next` /
/// `next_mut`.
///
/// Bounds checking is done before the declared-region walk: the dispatcher has
/// already verified `num_accounts >= T::HEADER_SIZE`.
pub struct AccountLoader<'a> {
    cursor: &'a mut AccountCursor,
}

impl<'a> AccountLoader<'a> {
    #[inline(always)]
    pub fn new(cursor: &'a mut AccountCursor) -> Self {
        Self { cursor }
    }

    #[inline(always)]
    pub fn consumed(&self) -> u8 {
        self.cursor.consumed()
    }

    /// Walk N accounts in bulk, returning a slice of raw `AccountView`s
    /// and the duplicate tracking bitvec.
    /// Cursor math runs in a tight loop before any validation happens.
    ///
    /// # Safety
    ///
    /// Caller must ensure N does not exceed the remaining accounts.
    #[inline(always)]
    pub fn walk_n(&mut self, n: usize) -> (&[AccountView], Option<&AccountBitvec>) {
        unsafe { self.cursor.walk_n(n) }
    }

    /// Walk one account from the cursor and return the raw `AccountView`.
    ///
    /// Used by the init / init_if_needed / zeroed codegen paths where
    /// the derive macro wants to construct + validate the account itself.
    #[inline(always)]
    pub fn next_view(&mut self) -> Result<AccountView, ProgramError> {
        // SAFETY: dispatcher has bounds-checked num_accounts >= HEADER_SIZE.
        Ok(unsafe { self.cursor.next() })
    }

    /// Walk + `T::load()` the next account.
    #[inline(always)]
    pub fn load_next<T: AnchorAccount>(&mut self) -> Result<T, ProgramError> {
        let view = unsafe { self.cursor.next() };
        T::load(view)
    }

    /// Walk + `T::load_mut()` the next account.
    ///
    /// # Safety
    ///
    /// Caller must ensure no other live `&mut` to the same account's data
    /// exists — see [`AnchorAccount::load_mut`] for the full precondition.
    #[inline(always)]
    pub unsafe fn next_mut<T: AnchorAccount>(&mut self) -> Result<T, ProgramError> {
        let view = self.cursor.next();
        T::load_mut(view)
    }
}
