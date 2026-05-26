//! Tests for `AccountCursor`, `AccountBitvec`, and
//! `Context::remaining_accounts`.
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
//!      an `AccountView` pointing at the dup slot.
//!   3. `Context::remaining_accounts` walks the cursor lazily on first
//!      call, caches the resulting `Vec<AccountView>`, and returns a
//!      fresh clone on each subsequent call without advancing the
//!      cursor or double-populating the cache.
//!
//! Run: `cargo test -p anchor-lang-v2 --features testing --test cursor_and_remaining_accounts`

use {
    anchor_lang_v2::{
        cursor::{mut_mask_or_shifted, mut_mask_set_bit, AccountBitvec, AccountCursor},
        testing::{AccountRecord, SbfInputBuffer},
        AccountViewCompat, Bumps, Context, MutMask,
    },
    core::mem::MaybeUninit,
    pinocchio::account::AccountView,
    solana_address::Address,
};

// A placeholder header struct that implements `Bumps` so we can construct
// a `Context<DummyHeader>` without needing the full `#[derive(Accounts)]`
// machinery. Empty `Bumps = ()` — no bumps tracked for remaining-only tests.
struct DummyHeader;
impl Bumps for DummyHeader {
    type Bumps = ();
}

/// No declared mut fields — these tests don't exercise the
/// trailing-dup-vs-declared-mut check (that's covered by
/// `remaining_accounts_mut_check.rs`).
const EMPTY_MUT_MASK: &[u64; 4] = &[0, 0, 0, 0];

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

    let (views, dups) = unsafe { cursor.walk_n(4) };
    assert_eq!(views.len(), 4);
    assert!(
        dups.is_none(),
        "no duplicates present → bitvec stays lazy None"
    );
    for (i, v) in views.iter().enumerate() {
        assert_eq!(v.address().to_bytes(), unique_addr(i as u8));
    }
    assert_eq!(cursor.consumed(), 4);
}

// -- Duplicate resolution --------------------------------------------------

#[test]
fn cursor_dup_resolves_to_earlier_view_and_flags_bitvec() {
    // Record 2 is a dup of record 0. The cursor must return the
    // `AccountView` stored at `lookup[0]` (same address as record 0),
    // not an AccountView pointing at the dup slot.
    let records = [non_dup(0), non_dup(1), AccountRecord::Dup { index: 0 }];
    let mut sbf = SbfInputBuffer::build(&records);
    let mut lookup = fresh_lookup();
    let mut cursor =
        unsafe { AccountCursor::new(sbf.as_mut_ptr(), lookup.as_mut_ptr() as *mut AccountView) };

    let (views, dups) = unsafe { cursor.walk_n(3) };

    assert_eq!(views[0].address().to_bytes(), unique_addr(0));
    assert_eq!(views[1].address().to_bytes(), unique_addr(1));
    // Dup slot — resolved to lookup[0].
    assert_eq!(views[2].address().to_bytes(), unique_addr(0));

    // Bitvec records BOTH the dup position (2) and the earlier instance
    // it points at (0). Used by the dispatcher to reject `mut`+dup combos.
    let bitvec = dups.expect("dup detected → bitvec materialized");
    assert!(bitvec.get(0), "original index must be flagged");
    assert!(!bitvec.get(1), "index 1 is a fresh non-dup");
    assert!(bitvec.get(2), "dup index must be flagged");
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
        MutMask::Static(EMPTY_MUT_MASK),
    );

    let remaining = ctx.remaining_accounts().expect("walk");
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
        MutMask::Static(EMPTY_MUT_MASK),
    );

    let mut remaining = ctx.remaining_accounts().expect("walk");
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
        MutMask::Static(EMPTY_MUT_MASK),
    );

    assert!(ctx.remaining_accounts().expect("walk").is_empty());
    // Second call on empty — still empty, no cache bookkeeping bug.
    assert!(ctx.remaining_accounts().expect("walk").is_empty());
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
        MutMask::Static(EMPTY_MUT_MASK),
    );

    let first = ctx.remaining_accounts().expect("first walk");
    let second = ctx.remaining_accounts().expect("second walk");

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

// -- AccountBitvec + mask helpers -----------------------------------------

#[test]
fn bitvec_intersects_matches_derive_duplicate_mask() {
    // Drive bit population through the cursor — `AccountBitvec::set` is
    // module-private, so the only legitimate way to populate one outside
    // the crate is to run a dup record through the cursor.
    let records = [non_dup(0), non_dup(1), AccountRecord::Dup { index: 0 }];
    let mut sbf = SbfInputBuffer::build(&records);
    let mut lookup = fresh_lookup();
    let mut cursor =
        unsafe { AccountCursor::new(sbf.as_mut_ptr(), lookup.as_mut_ptr() as *mut AccountView) };
    let (_views, dups) = unsafe { cursor.walk_n(3) };
    let bv = dups.expect("dup present");

    // MUT_MASK marking index 0 as a mut field: intersects true.
    let mask_hits_dup = mut_mask_set_bit([0u64; 4], 0);
    assert!(bv.intersects(&mask_hits_dup));

    // MUT_MASK that only flags an unused index (4): intersects false.
    let mask_clean = mut_mask_set_bit([0u64; 4], 4);
    assert!(!bv.intersects(&mask_clean));
}

#[test]
fn bitvec_get_reports_default_as_all_clear() {
    // `Default` zeros the four u64s — get() of any index must be false.
    let bv = AccountBitvec::default();
    for i in [0u8, 1, 31, 63, 64, 127, 128, 191, 192, 255] {
        assert!(!bv.get(i), "bit {i} unexpectedly set in a default bitvec");
    }
}

// -- mut_mask helpers ----------------------------------------------------

#[test]
fn mut_mask_set_bit_sets_the_single_bit_in_the_right_word() {
    // Pick representative bits across all four u64 words.
    for bit in [0usize, 7, 63, 64, 127, 128, 191, 192, 255] {
        let m = mut_mask_set_bit([0u64; 4], bit);
        let word = bit / 64;
        let offset = bit % 64;
        assert_eq!(m[word], 1u64 << offset, "bit {bit} set wrong word/offset");
        for (w, val) in m.iter().enumerate() {
            if w != word {
                assert_eq!(*val, 0, "bit {bit} leaked into word {w}");
            }
        }
    }
}

#[test]
fn mut_mask_set_bit_is_idempotent_or_with_existing_bits() {
    // Calling twice on non-overlapping bits OR-merges them.
    let m = mut_mask_set_bit([0u64; 4], 3);
    let m = mut_mask_set_bit(m, 200);
    assert_eq!(m[0], 1u64 << 3);
    assert_eq!(m[3], 1u64 << (200 - 192));
}

#[test]
fn mut_mask_or_shifted_with_zero_shift_ors_in_place() {
    let parent = [0u64, 0, 0, 0];
    let child = [0xAAu64, 0xBBu64, 0xCCu64, 0xDDu64];
    let merged = mut_mask_or_shifted(parent, child, 0);
    assert_eq!(merged, child);
}

#[test]
fn mut_mask_or_shifted_shifts_by_word_and_bit() {
    // Child has bit 0 set in word 0. Shift by 65 → land in word 1, bit 1.
    let child = mut_mask_set_bit([0u64; 4], 0);
    let merged = mut_mask_or_shifted([0u64; 4], child, 65);
    let expected = mut_mask_set_bit([0u64; 4], 65);
    assert_eq!(merged, expected);
}

#[test]
fn mut_mask_or_shifted_past_end_is_dropped() {
    // Shifting into a word ≥ 4 must silently drop (no panic, no overflow).
    // The const fn is called at compile time and must stay total.
    let child = mut_mask_set_bit([0u64; 4], 0);
    let merged = mut_mask_or_shifted([0u64; 4], child, 256);
    assert_eq!(merged, [0u64; 4], "overflow bits must be dropped");
}
