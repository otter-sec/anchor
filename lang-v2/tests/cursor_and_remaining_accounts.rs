//! Tests for `AccountCursor` and `Context::remaining_accounts`.
//!
//! These walks are the hot path between the SBF loader's serialized
//! input buffer and the typed-account machinery. Coverage here pins
//! three things that tests elsewhere don't exercise:
//!
//!   1. `AccountCursor::next` advances the raw pointer past each
//!      account record (header + data + padding + rent-epoch + 8-byte
//!      align) so subsequent reads see the next record, not the tail
//!      of the previous one.
//!   2. Duplicate handling: a dup-record (borrow_state ∈ 0..=254)
//!      yields the earlier `AccountView` from the lookup array — not
//!      an `AccountView` pointing at the dup slot — so every view of an
//!      account shares one runtime borrow state.
//!   3. `Context::remaining_accounts` walks the cursor lazily on first
//!      call, caches the resulting `Vec<AccountView>`, and returns a
//!      fresh clone on each subsequent call without advancing the
//!      cursor or double-populating the cache.
//!
//! Run: `cargo test -p anchor-lang --features testing --test cursor_and_remaining_accounts`

use {
    anchor_lang::{
        cursor::AccountCursor,
        testing::{AccountRecord, SbfInputBuffer},
        AccountViewCompat, Bumps, Context,
    },
    core::mem::MaybeUninit,
    pinocchio::account::AccountView,
    solana_address::Address,
    solana_program_error::ProgramError,
};

// A placeholder header struct that implements `Bumps` so we can construct
// a `Context<DummyHeader>` without needing the full `#[derive(Accounts)]`
// machinery. Empty `Bumps = ()` — no bumps tracked for remaining-only tests.
struct DummyHeader;
impl Bumps for DummyHeader {
    type Bumps = ();
}

fn unique_addr(i: u8) -> [u8; 32] {
    let mut a = [0u8; 32];
    a[0] = i + 1; // avoid [0;32] which collides with the System program id.
    a
}

fn non_dup(i: u8) -> AccountRecord {
    AccountRecord::NonDup {
        address: unique_addr(i),
        owner: [0xAA; 32],
        lamports: 100 + i as u64,
        is_signer: false,
        is_writable: false,
        executable: false,
        data_len: 0,
    }
}

fn non_dup_with_data(i: u8, data_len: usize) -> AccountRecord {
    AccountRecord::NonDup {
        address: unique_addr(i),
        owner: [0xAA; 32],
        lamports: 100,
        is_signer: false,
        is_writable: false,
        executable: false,
        data_len,
    }
}

/// Allocate an uninitialised `[AccountView; 256]` on the heap as the
/// backing store for `AccountCursor`'s lookup table. Callers derive
/// the `*mut AccountView` via `lookup.as_mut_ptr() as *mut _` and keep
/// the `Vec` alive on the test's stack for the duration of the cursor
/// — drop order (cursor before lookup) keeps the pointer valid
/// without a `'static` hop, and avoids Miri leak-check hits.
fn fresh_lookup() -> Vec<MaybeUninit<AccountView>> {
    let mut v: Vec<MaybeUninit<AccountView>> = Vec::with_capacity(256);
    for _ in 0..256 {
        v.push(MaybeUninit::uninit());
    }
    v
}

// -- AccountCursor::next walks each record ---------------------------------

#[test]
fn cursor_next_advances_across_records() {
    let mut sbf = SbfInputBuffer::build(&[non_dup(0), non_dup(1), non_dup(2)]);
    let mut lookup = fresh_lookup();
    let mut cursor =
        unsafe { AccountCursor::new(sbf.as_mut_ptr(), lookup.as_mut_ptr() as *mut AccountView) };

    assert_eq!(cursor.consumed(), 0);
    let v0 = unsafe { cursor.next() };
    assert_eq!(v0.address().to_bytes(), unique_addr(0));
    assert_eq!(cursor.consumed(), 1);
    let v1 = unsafe { cursor.next() };
    assert_eq!(v1.address().to_bytes(), unique_addr(1));
    let v2 = unsafe { cursor.next() };
    assert_eq!(v2.address().to_bytes(), unique_addr(2));
    assert_eq!(cursor.consumed(), 3);
}

#[test]
fn cursor_next_walks_past_variable_data_regions() {
    // Non-zero `data_len` values exercise the `ptr += STATIC + data_len`
    // branch plus the 8-byte alignment fixup. If the alignment math is
    // off the next record's header reads as garbage.
    let records = [
        non_dup_with_data(0, 3),  // unaligned data_len → fixup adds 5 bytes
        non_dup_with_data(1, 17), // unaligned → fixup adds 7 bytes
        non_dup_with_data(2, 8),  // aligned → fixup adds 0 bytes
    ];
    let mut sbf = SbfInputBuffer::build(&records);
    let mut lookup = fresh_lookup();
    let mut cursor =
        unsafe { AccountCursor::new(sbf.as_mut_ptr(), lookup.as_mut_ptr() as *mut AccountView) };

    for i in 0u8..3 {
        let view = unsafe { cursor.next() };
        assert_eq!(
            view.address().to_bytes(),
            unique_addr(i),
            "record {i} address mismatch — alignment-fixup bug?"
        );
        let expected_data_len = match i {
            0 => 3,
            1 => 17,
            _ => 8,
        };
        assert_eq!(view.data_len(), expected_data_len);
    }
}

#[test]
fn cursor_walk_n_returns_all_views_at_once() {
    let mut sbf = SbfInputBuffer::build(&[non_dup(0), non_dup(1), non_dup(2), non_dup(3)]);
    let mut lookup = fresh_lookup();
    let mut cursor =
        unsafe { AccountCursor::new(sbf.as_mut_ptr(), lookup.as_mut_ptr() as *mut AccountView) };

    let views = unsafe { cursor.walk_n(4) };
    assert_eq!(views.len(), 4);
    for (i, v) in views.iter().enumerate() {
        assert_eq!(v.address().to_bytes(), unique_addr(i as u8));
    }
    assert_eq!(cursor.consumed(), 4);
}

// -- Duplicate resolution --------------------------------------------------

#[test]
fn cursor_dup_resolves_to_earlier_view() {
    // Record 2 is a dup of record 0. The cursor must return the
    // `AccountView` stored at `lookup[0]` (same header as record 0),
    // not an AccountView pointing at the dup slot.
    let records = [non_dup(0), non_dup(1), AccountRecord::Dup { index: 0 }];
    let mut sbf = SbfInputBuffer::build(&records);
    let mut lookup = fresh_lookup();
    let mut cursor =
        unsafe { AccountCursor::new(sbf.as_mut_ptr(), lookup.as_mut_ptr() as *mut AccountView) };

    let views = unsafe { cursor.walk_n(3) };

    assert_eq!(views[0].address().to_bytes(), unique_addr(0));
    assert_eq!(views[1].address().to_bytes(), unique_addr(1));
    assert_eq!(views[2].address().to_bytes(), unique_addr(0));
    assert_eq!(views[2].account_ptr(), views[0].account_ptr());
}

#[test]
fn cursor_dup_shares_borrow_state_with_earlier_view() {
    let records = [non_dup(0), AccountRecord::Dup { index: 0 }];
    let mut sbf = SbfInputBuffer::build(&records);
    let mut lookup = fresh_lookup();
    let mut cursor =
        unsafe { AccountCursor::new(sbf.as_mut_ptr(), lookup.as_mut_ptr() as *mut AccountView) };

    let views = unsafe { cursor.walk_n(2) };
    let mut original = views[0];
    let alias = views[1];

    let guard = original.try_borrow_mut().expect("first borrow");
    assert_eq!(alias.try_borrow().err(), Some(ProgramError::AccountBorrowFailed));
    drop(guard);
    assert!(alias.try_borrow().is_ok());
}

// -- Context::remaining_accounts ------------------------------------------

#[test]
fn remaining_accounts_walks_trailing_region() {
    // Full transaction has 5 accounts: 2 declared, 3 trailing.
    let records = [non_dup(0), non_dup(1), non_dup(2), non_dup(3), non_dup(4)];
    let mut sbf = SbfInputBuffer::build(&records);
    let mut lookup = fresh_lookup();
    let mut cursor =
        unsafe { AccountCursor::new(sbf.as_mut_ptr(), lookup.as_mut_ptr() as *mut AccountView) };

    // Simulate the dispatcher consuming the declared (HEADER_SIZE=2) accounts.
    let _ = unsafe { cursor.walk_n(2) };
    assert_eq!(cursor.consumed(), 2);

    let program_id = Address::new_from_array([0x42; 32]);
    let mut ctx: Context<'_, DummyHeader> = Context::new(
        &program_id,
        DummyHeader,
        (),
        &mut cursor,
        /*remaining_num*/ 3,
    );

    let remaining = ctx.remaining_accounts();
    assert_eq!(remaining.len(), 3);
    assert_eq!(remaining[0].address().to_bytes(), unique_addr(2));
    assert_eq!(remaining[1].address().to_bytes(), unique_addr(3));
    assert_eq!(remaining[2].address().to_bytes(), unique_addr(4));
}

#[test]
fn remaining_account_views_expose_compat_helpers() {
    let records = [
        non_dup(0),
        non_dup_with_data(1, 16),
        non_dup_with_data(2, 32),
    ];
    let mut sbf = SbfInputBuffer::build(&records);
    let mut lookup = fresh_lookup();
    let mut cursor =
        unsafe { AccountCursor::new(sbf.as_mut_ptr(), lookup.as_mut_ptr() as *mut AccountView) };

    let _ = unsafe { cursor.walk_n(1) };

    let program_id = Address::new_from_array([0x42; 32]);
    let mut ctx: Context<'_, DummyHeader> = Context::new(
        &program_id,
        DummyHeader,
        (),
        &mut cursor,
        /*remaining_num*/ 2,
    );

    let mut remaining = ctx.remaining_accounts();
    assert_eq!(remaining[0].key().to_bytes(), unique_addr(1));
    assert!(!remaining[0].data_is_empty());
    assert_eq!(remaining[0].try_data_len().unwrap(), 16);
    assert_eq!(remaining[0].try_borrow_data().unwrap().len(), 16);
    assert_eq!(remaining[0].try_borrow_mut_data().unwrap().len(), 16);
}

#[test]
fn remaining_accounts_returns_empty_when_nothing_trails() {
    let mut sbf = SbfInputBuffer::build(&[non_dup(0), non_dup(1)]);
    let mut lookup = fresh_lookup();
    let mut cursor =
        unsafe { AccountCursor::new(sbf.as_mut_ptr(), lookup.as_mut_ptr() as *mut AccountView) };
    let _ = unsafe { cursor.walk_n(2) };

    let program_id = Address::new_from_array([0x42; 32]);
    let mut ctx: Context<'_, DummyHeader> = Context::new(
        &program_id,
        DummyHeader,
        (),
        &mut cursor,
        0,
    );

    assert!(ctx.remaining_accounts().is_empty());
    // Second call on empty — still empty, no cache bookkeeping bug.
    assert!(ctx.remaining_accounts().is_empty());
}

#[test]
fn remaining_accounts_caches_and_does_not_re_walk_cursor() {
    // If the cache were bypassed, the second `remaining_accounts` call
    // would re-enter the cursor past its current position and read
    // garbage (or undefined behaviour) past the input buffer tail.
    let mut sbf = SbfInputBuffer::build(&[non_dup(0), non_dup(1), non_dup(2)]);
    let mut lookup = fresh_lookup();
    let mut cursor =
        unsafe { AccountCursor::new(sbf.as_mut_ptr(), lookup.as_mut_ptr() as *mut AccountView) };
    let _ = unsafe { cursor.walk_n(1) };

    let program_id = Address::new_from_array([0x42; 32]);
    let consumed_before = cursor.consumed();
    let mut ctx: Context<'_, DummyHeader> = Context::new(
        &program_id,
        DummyHeader,
        (),
        &mut cursor,
        /*remaining_num*/ 2,
    );

    let first = ctx.remaining_accounts();
    let second = ctx.remaining_accounts();

    // Structural equality via address, since AccountView is Copy and the
    // cache returns a clone each call.
    assert_eq!(first.len(), 2);
    assert_eq!(second.len(), 2);
    for (a, b) in first.iter().zip(second.iter()) {
        assert_eq!(a.address().to_bytes(), b.address().to_bytes());
    }
    // Cursor must have advanced by exactly `remaining_num` (2), not
    // twice that — verifies caching short-circuits the walk.
    //
    // NB: `consumed()` reads through the `&mut AccountCursor` stored
    // inside `ctx` so we can't call it on the outer `cursor` binding
    // directly; drop `ctx` first to release the borrow.
    drop(ctx);
    assert_eq!(cursor.consumed(), consumed_before + 2);
}

#[test]
fn remaining_alias_shares_borrow_state_with_declared_view() {
    let records = [non_dup(0), AccountRecord::Dup { index: 0 }];
    let mut sbf = SbfInputBuffer::build(&records);
    let mut lookup = fresh_lookup();
    let mut cursor =
        unsafe { AccountCursor::new(sbf.as_mut_ptr(), lookup.as_mut_ptr() as *mut AccountView) };
    let mut declared = unsafe { cursor.walk_n(1) }[0];
    let declared_address = *declared.address();
    let guard = declared.try_borrow_mut().expect("declared borrow");

    let program_id = Address::new_from_array([0x42; 32]);
    let mut ctx: Context<'_, DummyHeader> =
        Context::new(&program_id, DummyHeader, (), &mut cursor, 1);

    let remaining = ctx.remaining_accounts();
    assert_eq!(*remaining[0].address(), declared_address);
    assert_eq!(
        remaining[0].try_borrow().err(),
        Some(ProgramError::AccountBorrowFailed),
        "a remaining alias must observe the declared view's borrow"
    );
    drop(guard);
    assert!(remaining[0].try_borrow().is_ok());
}
