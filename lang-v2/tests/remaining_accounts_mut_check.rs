//! Bug-3 fix: `Context::remaining_accounts()` re-checks `MUT_MASK`.
//!
//! ## The bug
//!
//! `run_handler` (`lang-v2/src/dispatch.rs`) performs exactly one
//! `duplicates.intersects(&T::MUT_MASK)` test against the bitvec the
//! `HEADER_SIZE` walk produced. `Context::remaining_accounts()` then
//! calls `cursor.next()` for each trailing account. `cursor.next()`
//! does detect dups and flips bits in the cursor's bitvec — but no
//! re-test of `MUT_MASK` happens.
//!
//! Attack: supply `HEADER_SIZE + 1` accounts where the trailing slot
//! aliases a declared mut slot. The handler's
//! `ctx.remaining_accounts()[0]` now points at the same
//! `RuntimeAccount` as `ctx.accounts.<mut_field>` without tripping
//! `ConstraintDuplicateMutableAccount`.
//!
//! ## The fix
//!
//! `Context::remaining_accounts()` re-intersects the cursor's live
//! bitvec against `MUT_MASK` after every `cursor.next()`. Because
//! `MUT_MASK`'s bits only cover declared field indices (0..HEADER_SIZE
//! for top-level structs), a trailing account's own index bit never
//! collides with the mask — the test only fires when the cursor
//! resolved a dup pointing at a declared mut slot.
//!
//! These tests drive `Context` directly (a `Bumps` impl for a
//! stand-in accounts struct plus a fake `mut_mask`) rather than going
//! through `#[derive(Accounts)]` — that keeps the test focused on
//! the `Context::remaining_accounts` control flow and the cursor's
//! bitvec exposure, which are the bits that actually changed.

extern crate alloc;

use {
    anchor_lang_v2::{
        testing::{AccountRecord, SbfInputBuffer},
        AccountCursor, AccountView, Bumps, Context,
    },
    core::mem::MaybeUninit,
    solana_address::Address,
    solana_program_error::ProgramError,
};

/// Trivial accounts struct with no real declared fields — the test
/// constructs `Context` by hand, so the generic bound is all we need.
struct NoAccounts;
impl Bumps for NoAccounts {
    type Bumps = ();
}

const PROGRAM_ID: [u8; 32] = [0x42; 32];
const MAX_ACCOUNTS: usize = 8;

/// `ProgramError::Custom(2005)` — `ErrorCode::ConstraintDuplicateMutableAccount`
/// (`lang-v2/src/lib.rs`).
const DUP_MUT_ERROR: u32 = 2005;

/// Build a fresh non-dup record with a distinct address.
fn fresh(tag: u8) -> AccountRecord {
    AccountRecord::NonDup {
        address: [tag; 32],
        owner: PROGRAM_ID,
        lamports: 100,
        is_signer: false,
        is_writable: true,
        executable: false,
        data_len: 0,
    }
}

/// Run one `Context::remaining_accounts()` invocation over a cursor
/// built from `records`, treating the first `header_size` accounts as
/// the declared region and the remainder as trailing. `mut_mask`
/// bitmap covers the declared region (as `T::MUT_MASK` would).
fn run_with(
    records: &[AccountRecord],
    header_size: usize,
    mut_mask: &'static [u64; 4],
) -> Result<alloc::vec::Vec<AccountView>, ProgramError> {
    let mut buf = SbfInputBuffer::build(records);
    let mut lookup: [MaybeUninit<AccountView>; MAX_ACCOUNTS] =
        [const { MaybeUninit::uninit() }; MAX_ACCOUNTS];

    let program_id = Address::new_from_array(PROGRAM_ID);
    // SAFETY: buf outlives cursor; lookup is aligned + sized for MAX_ACCOUNTS.
    let mut cursor =
        unsafe { AccountCursor::new(buf.as_mut_ptr(), lookup.as_mut_ptr() as *mut AccountView) };

    // Walk the declared region exactly as `run_handler` would. Check
    // `MUT_MASK` once for parity with the dispatcher.
    let (_views, dups) = unsafe { cursor.walk_n(header_size) };
    if let Some(d) = dups {
        if d.intersects(mut_mask) {
            return Err(solana_program_error::ProgramError::Custom(DUP_MUT_ERROR));
        }
    }

    let remaining_num = (records.len() - header_size) as u8;
    let mut ctx: Context<NoAccounts> = Context::new(
        &program_id,
        NoAccounts,
        (),
        &mut cursor,
        remaining_num,
        mut_mask,
    );
    ctx.remaining_accounts()
}

// ---------------------------------------------------------------------------
// Bug reproducer + negative controls
// ---------------------------------------------------------------------------

/// MUT_MASK marking slot 0 as mut. Mirrors what `#[derive(Accounts)]`
/// emits for `{ #[account(mut)] a: UncheckedAccount }`.
const MUT_MASK_SLOT0: &[u64; 4] = &[0b1, 0, 0, 0];

/// MUT_MASK marking slot 1 as mut (slot 0 is non-mut).
const MUT_MASK_SLOT1: &[u64; 4] = &[0b10, 0, 0, 0];

/// Empty MUT_MASK — no declared mut fields.
const MUT_MASK_NONE: &[u64; 4] = &[0, 0, 0, 0];

#[test]
fn trailing_account_aliasing_declared_mut_is_rejected() {
    // Layout: [ mut declared, trailing-dup-of-slot-0 ]
    // Declared region has 1 slot, marked mut. Trailing slot's dup
    // index resolves to slot 0 → alias of a mut declared account.
    let records = [fresh(1), AccountRecord::Dup { index: 0 }];
    let result = run_with(&records, /*header_size*/ 1, MUT_MASK_SLOT0);
    match result {
        Err(ProgramError::Custom(code)) if code == DUP_MUT_ERROR => {}
        other => panic!(
            "expected Err(ConstraintDuplicateMutableAccount), got {:?}",
            other.map(|v| v.len())
        ),
    }
}

#[test]
fn trailing_account_aliasing_non_mut_declared_is_allowed() {
    // Declared region has 2 slots, only slot 1 is mut. Trailing slot
    // dups slot 0 (non-mut) → must be allowed.
    let records = [fresh(1), fresh(2), AccountRecord::Dup { index: 0 }];
    let result = run_with(&records, /*header_size*/ 2, MUT_MASK_SLOT1);
    match result {
        Ok(v) => assert_eq!(v.len(), 1),
        Err(e) => panic!("expected Ok, got Err({:?})", e),
    }
}

#[test]
fn two_trailing_dups_of_each_other_are_allowed() {
    // Declared region has 1 slot, non-mut. Two trailing slots: the
    // second dups the first trailing slot. Neither aliases a
    // declared mut, so the mut-mask check must not fire.
    let records = [fresh(1), fresh(2), AccountRecord::Dup { index: 1 }];
    let result = run_with(&records, /*header_size*/ 1, MUT_MASK_NONE);
    match result {
        Ok(v) => assert_eq!(v.len(), 2),
        Err(e) => panic!("expected Ok, got Err({:?})", e),
    }
}

#[test]
fn fresh_trailing_accounts_are_allowed() {
    // Regression: all-fresh trailing accounts must pass even when
    // slot 0 is declared mut.
    let records = [fresh(1), fresh(2), fresh(3)];
    let result = run_with(&records, /*header_size*/ 1, MUT_MASK_SLOT0);
    match result {
        Ok(v) => assert_eq!(v.len(), 2),
        Err(e) => panic!("expected Ok, got Err({:?})", e),
    }
}

#[test]
fn trailing_account_aliasing_nested_mut_is_rejected() {
    // Simulate `Parent { a: UncheckedAccount, inner: Nested<Inner> }`
    // where `Inner { #[account(mut)] b }`. Parent's `MUT_MASK` has
    // bit 1 set (slot 0 = parent's plain `a`; slot 1 = Inner's mut
    // `b` after the nested-shift). A trailing account aliasing slot 1
    // must still be caught — this is the "Nested children merge into
    // the top-level mask" case.
    let mut mask = [0u64; 4];
    mask[0] = 0b10;
    // Leaked to get 'static; test-only.
    let mask_ref: &'static [u64; 4] = alloc::boxed::Box::leak(alloc::boxed::Box::new(mask));

    let records = [fresh(1), fresh(2), AccountRecord::Dup { index: 1 }];
    let result = run_with(&records, /*header_size*/ 2, mask_ref);
    match result {
        Err(ProgramError::Custom(code)) if code == DUP_MUT_ERROR => {}
        other => panic!(
            "expected Err(ConstraintDuplicateMutableAccount), got {:?}",
            other.map(|v| v.len())
        ),
    }
}

#[test]
fn remaining_accounts_error_is_cached_on_failure() {
    // Before the fix, Err left `remaining_cache` unset, so a repeat
    // call would re-enter the walk loop and call `cursor.next()` again
    // — past the remaining region on an unsafe cursor. After the fix,
    // the error is cached and replayed; the cursor advances exactly
    // once per instruction regardless of how many times the handler
    // calls `remaining_accounts()`.
    let records = [fresh(1), AccountRecord::Dup { index: 0 }];
    let mut buf = SbfInputBuffer::build(&records);
    let mut lookup: [MaybeUninit<AccountView>; MAX_ACCOUNTS] =
        [const { MaybeUninit::uninit() }; MAX_ACCOUNTS];
    let program_id = Address::new_from_array(PROGRAM_ID);
    let mut cursor =
        unsafe { AccountCursor::new(buf.as_mut_ptr(), lookup.as_mut_ptr() as *mut AccountView) };
    let (_views, _dups) = unsafe { cursor.walk_n(1) };
    let mut ctx: Context<NoAccounts> =
        Context::new(&program_id, NoAccounts, (), &mut cursor, 1, MUT_MASK_SLOT0);

    for _ in 0..5 {
        match ctx.remaining_accounts() {
            Err(ProgramError::Custom(code)) => assert_eq!(code, DUP_MUT_ERROR),
            other => panic!(
                "expected cached Err(ConstraintDuplicateMutableAccount), got {:?}",
                other.map(|v| v.len())
            ),
        }
    }
}

#[test]
fn remaining_accounts_result_is_cached_on_success() {
    // Regression on the cache path: two successful calls must return
    // equal-length vecs without re-walking the cursor (second walk
    // would read past the end).
    let records = [fresh(1), fresh(2), fresh(3)];
    let mut buf = SbfInputBuffer::build(&records);
    let mut lookup: [MaybeUninit<AccountView>; MAX_ACCOUNTS] =
        [const { MaybeUninit::uninit() }; MAX_ACCOUNTS];
    let program_id = Address::new_from_array(PROGRAM_ID);
    let mut cursor =
        unsafe { AccountCursor::new(buf.as_mut_ptr(), lookup.as_mut_ptr() as *mut AccountView) };
    let (_views, _dups) = unsafe { cursor.walk_n(1) };
    let mut ctx: Context<NoAccounts> =
        Context::new(&program_id, NoAccounts, (), &mut cursor, 2, MUT_MASK_SLOT0);

    let first = ctx.remaining_accounts().expect("first call");
    let second = ctx.remaining_accounts().expect("second call");
    assert_eq!(first.len(), 2);
    assert_eq!(second.len(), 2);
}
