use {
    crate::{
        context::{Bumps, Context, MutMask},
        cursor::{AccountBitvec, AccountCursor},
        loader::AccountLoader,
    },
    pinocchio::{account::AccountView, address::Address},
    solana_program_error::ProgramError,
};

/// Trait that `#[derive(Accounts)]` implements on account structs.
///
/// `try_accounts` receives a pre-walked `&[AccountView]` slice (from a
/// single `walk_n(HEADER_SIZE)` in [`run_handler`]) rather than the raw
/// cursor.  This lets `Nested<Inner>` fields pass a sub-slice to
/// `Inner::try_accounts` without re-walking the cursor or fighting
/// borrow-checker splits.
///
/// `HEADER_SIZE` is computed recursively at compile time: 1 per direct
/// field, `+ <Inner as TryAccounts>::HEADER_SIZE` per `Nested<Inner>`.
pub trait TryAccounts: Bumps + Sized {
    const HEADER_SIZE: usize;

    /// Bit `i` is set iff account-view index `i` (global, across nested
    /// children) is a non-`Option` mut field without `unsafe(dup)`. The
    /// dispatcher AND's this against the walked `AccountBitvec` once per
    /// `run_handler` call — a single 4-word test replaces N per-field
    /// `get()` checks across the whole struct tree. `Option<T>` mut
    /// fields are excluded here and keep their gated per-field check so
    /// a `None` slot (encoded as `program_id`) stays silent the way it
    /// does today.
    const MUT_MASK: [u64; 4];

    /// True when [`Self::active_mut_mask`] can differ from [`Self::MUT_MASK`].
    /// This lets accounts without optional mutable fields keep the old static
    /// mask path.
    const HAS_DYNAMIC_MUT_MASK: bool;

    /// Runtime mutable-account mask for checks that happen after declared
    /// accounts are loaded. This includes optional mutable accounts only when
    /// they resolved to `Some`.
    fn active_mut_mask(&self) -> [u64; 4];

    /// Parsed instruction args carried alongside validated accounts.
    /// Accounts structs without `#[instruction(...)]` use `()`.
    type IxArgs<'ix>;

    /// `base_offset` is the index of the first view in the global bitvec.
    /// Top-level callers pass 0; `Nested<T>` passes its field's offset so
    /// the inner struct's duplicate-mutable-account checks hit the correct
    /// global bits.
    fn try_accounts<'ix>(
        program_id: &Address,
        views: &[AccountView],
        duplicates: Option<&AccountBitvec>,
        base_offset: usize,
        ix_data: &'ix [u8],
    ) -> Result<(Self, Self::Bumps, Self::IxArgs<'ix>), ProgramError>;

    fn exit_accounts(&mut self) -> Result<(), ProgramError>;
}

/// Run a handler inside a fully-constructed [`Context`].
///
/// Walks all declared accounts in one `walk_n(HEADER_SIZE)` call, then
/// passes the views slice to `T::try_accounts` for per-field loading and
/// constraint checking.  The residual cursor (past the declared accounts)
/// is handed to `Context` for lazy `remaining_accounts()` access.
#[inline(always)]
pub fn run_handler<'a, T: TryAccounts, R>(
    program_id: &'a Address,
    cursor: &'a mut AccountCursor,
    ix_data: &'a [u8],
    num_accounts: usize,
    handler: impl FnOnce(&mut Context<'a, T>, T::IxArgs<'a>) -> Result<R, ProgramError>,
) -> Result<R, ProgramError> {
    if num_accounts < T::HEADER_SIZE {
        return Err(crate::ErrorCode::AccountNotEnoughKeys.into());
    }
    let (ctx_accounts, bumps, ix_args) = {
        let mut loader = AccountLoader::new(cursor);
        let (views, duplicates) = loader.walk_n(T::HEADER_SIZE);
        // Single AND+test across the whole struct tree — replaces the
        // per-mut-field `__duplicates.get()` checks the derive used to
        // emit at each field site. `MUT_MASK == [0; 4]` (no mut fields
        // anywhere) const-folds the intersect away at inline time. The
        // outer `Some` guard short-circuits when the walker didn't need
        // to materialize a bitvec (no account appeared twice), so the
        // no-dup path pays a single Option-tag test.
        if let Some(dups) = duplicates {
            if dups.intersects(&T::MUT_MASK) {
                return Err(crate::ErrorCode::ConstraintDuplicateMutableAccount.into());
            }
        }
        T::try_accounts(program_id, views, duplicates, 0, ix_data)?
    };
    const _: () = assert!(pinocchio::MAX_TX_ACCOUNTS <= u8::MAX as usize);
    let remaining_num = (num_accounts - T::HEADER_SIZE) as u8;
    let mut_mask = if remaining_num != 0 && T::HAS_DYNAMIC_MUT_MASK {
        MutMask::Dynamic(ctx_accounts.active_mut_mask())
    } else {
        MutMask::Static(&T::MUT_MASK)
    };
    let mut ctx = Context::new(
        program_id,
        ctx_accounts,
        bumps,
        cursor,
        remaining_num,
        mut_mask,
    );
    let result = handler(&mut ctx, ix_args)?;
    ctx.accounts.exit_accounts()?;
    Ok(result)
}
