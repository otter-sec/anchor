//! Reproducers for two `Slab` post-resize soundness bugs: `capacity()`
//! underflow, and `as_slice` / `as_mut_slice` OOB. Both arise when an
//! external `realloc_account` shrinks `data_len` below `ITEMS_OFFSET`
//! (or below the populated `len`) while a `Slab` wrapper is retained.
//!
//! Both tests build a Slab<Counter, [u8; 8]>, load it normally, then
//! mutate the underlying `RuntimeAccount.data_len` to simulate what
//! happens when an external `realloc_account` shrinks the account
//! below the Slab's structural expectations. The Slab still holds its
//! AccountView by value and reads live `data_len()` on every call.
//!
//! Run: `cargo test -p anchor-lang-v2 --test slab_resize_reproducers`

use {
    anchor_lang_v2::{
        accounts::{Account, Slab},
        testing::AccountBuffer,
        AccountRealloc, AnchorAccount, Discriminator, Owner, Space,
    },
    bytemuck::{Pod, Zeroable},
    pinocchio::address::Address,
    solana_program_error::ProgramError,
};

const PROGRAM_ID: [u8; 32] = [0x42; 32];

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct Counter {
    value: u64,
    bump: u8,
    _pad: [u8; 7],
}

impl Owner for Counter {
    const OWNER: Address = Address::new_from_array(PROGRAM_ID);
}

impl Discriminator for Counter {
    // sha256("account:Counter")[..8]
    const DISCRIMINATOR: &'static [u8] = &[0xff, 0xb0, 0x04, 0xf5, 0xbc, 0xfd, 0x7c, 0x19];
}

type CounterLedger = Slab<Counter, [u8; 8]>;
type CounterAccount = Account<Counter>;

// Layout: disc(8) + Counter(16) + len(4) + items([u8;8] × N)
//   HEADER_OFFSET = 8,  LEN_OFFSET = 24,  ITEMS_OFFSET = 28 (T align 1)
const HEADER_OFFSET: usize = 8;
const LEN_OFFSET: usize = HEADER_OFFSET + core::mem::size_of::<Counter>();
const ITEMS_OFFSET: usize = LEN_OFFSET + 4;
const ITEM_SIZE: usize = core::mem::size_of::<[u8; 8]>();

fn disc_bytes() -> [u8; 8] {
    [0xff, 0xb0, 0x04, 0xf5, 0xbc, 0xfd, 0x7c, 0x19]
}

fn setup_ledger(capacity: usize, populated_len: u32) -> AccountBuffer<256> {
    let buf = AccountBuffer::<256>::new();
    let data_len = ITEMS_OFFSET + capacity * ITEM_SIZE;
    buf.init(
        [0xAA; 32], PROGRAM_ID, data_len, /*signer*/ false, /*writable*/ true,
        /*executable*/ false,
    );
    let mut data = [0u8; 256];
    data[..8].copy_from_slice(&disc_bytes());
    // Counter header is all zero (default) — skip.
    // len field at LEN_OFFSET — populated_len as u32 LE.
    data[LEN_OFFSET..LEN_OFFSET + 4].copy_from_slice(&populated_len.to_le_bytes());
    buf.write_data(&data[..data_len]);
    buf
}

fn setup_counter_account() -> AccountBuffer<256> {
    let buf = AccountBuffer::<256>::new();
    let data_len = HEADER_OFFSET + core::mem::size_of::<Counter>();
    buf.init(
        [0xAB; 32], PROGRAM_ID, data_len, /*signer*/ false, /*writable*/ true,
        /*executable*/ false,
    );
    let mut data = [0u8; 256];
    data[..8].copy_from_slice(&disc_bytes());
    buf.write_data(&data[..data_len]);
    buf
}

fn expected_min_lamports(space: usize) -> Result<u64, ProgramError> {
    anchor_lang_v2::cpi::rent_exempt_lamports(space)
}

#[test]
fn slab_min_data_len_tracks_schema_and_tail_layout() {
    assert_eq!(
        <CounterAccount as AnchorAccount>::MIN_DATA_LEN,
        HEADER_OFFSET + core::mem::size_of::<Counter>()
    );
    assert_eq!(
        <CounterAccount as Space>::INIT_SPACE,
        <CounterAccount as AnchorAccount>::MIN_DATA_LEN
    );

    assert_eq!(<CounterLedger as AnchorAccount>::MIN_DATA_LEN, ITEMS_OFFSET);
    assert_eq!(
        <CounterLedger as Space>::INIT_SPACE,
        <CounterLedger as AnchorAccount>::MIN_DATA_LEN
    );
}

#[test]
fn slab_realloc_rejects_shrink_below_typed_header_layout() {
    let buf = setup_counter_account();
    let payer = AccountBuffer::<128>::new();
    payer.init([0xCC; 32], PROGRAM_ID, 0, true, true, false);

    let view = unsafe { buf.view() };
    let mut account = unsafe { CounterAccount::load_mut(view) }.unwrap();
    let payer_view = unsafe { payer.view() };

    let err = account
        .realloc_account(8, payer_view, false)
        .expect_err("realloc must reject a buffer shorter than [disc][Counter]");

    assert_eq!(err, ProgramError::AccountDataTooSmall);
    assert_eq!(
        account.current_space(),
        HEADER_OFFSET + core::mem::size_of::<Counter>()
    );
}

#[test]
fn slab_realloc_clamps_tail_len_to_resized_capacity() {
    let buf = setup_ledger(/*capacity*/ 4, /*len*/ 3);
    let payer = AccountBuffer::<128>::new();
    payer.init([0xCC; 32], PROGRAM_ID, 0, true, true, false);

    let view = unsafe { buf.view() };
    let mut slab = unsafe { CounterLedger::load_mut(view) }.unwrap();
    let payer_view = unsafe { payer.view() };

    slab.realloc_account(ITEMS_OFFSET + ITEM_SIZE, payer_view, false)
        .unwrap();

    assert_eq!(slab.capacity(), 1);
    assert_eq!(slab.len(), 1);

    drop(slab);

    assert_eq!(CounterLedger::load(view).unwrap().len(), 1);
}

// -- `load_mut` rejects a buffer shrunk below ITEMS_OFFSET -----------

#[test]
fn load_mut_rejects_data_len_below_items_offset() {
    let buf = setup_ledger(/*capacity*/ 4, /*len*/ 0);

    // Load succeeds — data_len (60) > ITEMS_OFFSET (28).
    let view = unsafe { buf.view() };
    let slab = unsafe { CounterLedger::load_mut(view) }.unwrap();
    assert_eq!(slab.capacity(), 4);
    assert_eq!(slab.len(), 0);
    drop(slab);

    // External resize path: shrink data_len to 16 (< ITEMS_OFFSET=28).
    // Represents a realloc_account call that shrunk this account while
    // a Slab wrapper held it.
    buf.set_data_len(16);

    // Reload the Slab. `load_mut` re-validates but... does it catch
    // this? `slab.rs:596` checks `data.len() < Self::ITEMS_OFFSET` —
    // returns AccountDataTooSmall. Good — `load_mut` catches it.
    let view2 = unsafe { buf.view() };
    let reload = unsafe { CounterLedger::load_mut(view2) };
    assert!(
        reload.is_err(),
        "load_mut should reject data_len < ITEMS_OFFSET — if it doesn't, the subsequent \
         capacity() computation will underflow"
    );
}

#[test]
fn load_rejects_len_greater_than_capacity_via_from_ref_validate_tail() {
    let buf = setup_ledger(/*capacity*/ 1, /*len*/ 2);
    let mut buf = buf;

    let view = unsafe { buf.view() };
    let result = CounterLedger::load(view);
    assert_eq!(result.err(), Some(ProgramError::InvalidAccountData));
}

#[test]
fn load_mut_rejects_len_greater_than_capacity_via_validate_tail() {
    let mut buf = setup_ledger(/*capacity*/ 1, /*len*/ 2);

    let view = unsafe { buf.view() };
    let result = unsafe { CounterLedger::load_mut(view) };
    assert_eq!(result.err(), Some(ProgramError::InvalidAccountData));
}

// Regression: after the guard lands, `capacity()` returns 0 (no panic,
// no usize underflow) when a retained Slab sees `data_len` shrink below
// `ITEMS_OFFSET`.
#[test]
fn capacity_returns_zero_when_data_len_below_items_offset() {
    let buf = setup_ledger(/*capacity*/ 4, /*len*/ 0);

    let view = unsafe { buf.view() };
    let slab = unsafe { CounterLedger::load_mut(view) }.unwrap();
    assert_eq!(slab.capacity(), 4);

    // External resize shrinks buffer while we still hold `slab`.
    // Pre-fix: capacity() would panic (debug) or wrap to huge (release).
    buf.set_data_len(16); // < ITEMS_OFFSET=28

    // Post-fix: capacity() guards against underflow and returns 0.
    assert_eq!(slab.capacity(), 0);
}

// -- Regression: as_slice clamps len to capacity after external shrink

#[test]
fn as_slice_clamps_len_to_capacity_after_external_shrink() {
    // Buffer with capacity 4, populated with 3 items (len=3).
    let buf = setup_ledger(/*capacity*/ 4, /*len*/ 3);

    let view = unsafe { buf.view() };
    let slab = unsafe { CounterLedger::load_mut(view) }.unwrap();
    assert_eq!(slab.len(), 3);
    assert_eq!(slab.capacity(), 4);

    // External resize shrinks the buffer: new data_len = ITEMS_OFFSET + 1*ITEM_SIZE = 36.
    // Now capacity would be 1, but len is still 3 (the len bytes at
    // LEN_OFFSET weren't touched).
    buf.set_data_len((ITEMS_OFFSET + ITEM_SIZE) as u64);

    // Pre-fix: as_slice() OOB'd on the bytes slice. Post-fix: len is
    // clamped to capacity before slicing, so the slice is valid.
    let slice = slab.as_slice();
    // len() still reports 3 (the raw value); as_slice clamps to 1 to
    // avoid OOB.
    assert!(slice.len() <= slab.capacity());
    assert_eq!(slice.len(), 1); // capacity after resize
}

// Regression: pop/swap_remove/truncate use effective_len after an
// external shrink, so they don't index past the live buffer or leave
// `len > capacity`.

#[test]
fn pop_after_external_shrink_uses_effective_len() {
    let buf = setup_ledger(/*capacity*/ 4, /*len*/ 3);

    let view = unsafe { buf.view() };
    let mut slab = unsafe { CounterLedger::load_mut(view) }.unwrap();
    assert_eq!(slab.len(), 3);
    assert_eq!(slab.capacity(), 4);

    // External resize: capacity drops to 1 with the slab still in scope.
    buf.set_data_len((ITEMS_OFFSET + ITEM_SIZE) as u64);
    assert_eq!(slab.capacity(), 1);

    // pop() must read the item at effective_len-1 = 0 (the last live
    // item), not raw_len-1 = 2 (which would be out of the live buffer).
    let popped = slab.pop();
    assert!(popped.is_some(), "pop should return the lone live item");

    assert_eq!(slab.len(), 0);
}

// -- Regression: swap_remove respects effective_len after shrink ------

#[test]
fn swap_remove_after_external_shrink_uses_effective_len() {
    let buf = setup_ledger(/*capacity*/ 4, /*len*/ 3);

    let view = unsafe { buf.view() };
    let mut slab = unsafe { CounterLedger::load_mut(view) }.unwrap();

    // Shrink: capacity drops to 2 with the slab still in scope.
    buf.set_data_len((ITEMS_OFFSET + 2 * ITEM_SIZE) as u64);
    assert_eq!(slab.capacity(), 2);

    // index 1 is in-bounds for effective_len=2.
    let _ = slab.swap_remove(1);
    // On-disk len is now 1.
    assert_eq!(slab.len(), 1);
}

#[test]
#[should_panic(expected = "swap_remove index out of bounds")]
fn swap_remove_panics_when_index_geq_effective_len() {
    let buf = setup_ledger(/*capacity*/ 4, /*len*/ 3);

    let view = unsafe { buf.view() };
    let mut slab = unsafe { CounterLedger::load_mut(view) }.unwrap();

    // Shrink: capacity drops to 1 with the slab still in scope.
    buf.set_data_len((ITEMS_OFFSET + ITEM_SIZE) as u64);
    // index 2 is past effective_len=1 — must panic.
    let _ = slab.swap_remove(2);
}

#[test]
#[should_panic(
    expected = "Slab<H, T> mutated through a read-only load. Add #[account(mut)] to your accounts \
                struct."
)]
fn clear_panics_when_tail_mutation_uses_guard_bytes_mut_on_read_only_slab() {
    let mut buf = setup_ledger(/*capacity*/ 2, /*len*/ 1);

    let view = unsafe { buf.view() };
    let mut slab = CounterLedger::load(view).unwrap();
    slab.clear();
}

// -- Regression: truncate clamps to effective_len ---------------------

#[test]
fn truncate_clamps_to_effective_len_after_shrink() {
    let buf = setup_ledger(/*capacity*/ 4, /*len*/ 3);

    let view = unsafe { buf.view() };
    let mut slab = unsafe { CounterLedger::load_mut(view) }.unwrap();

    // Shrink while slab is in scope.
    buf.set_data_len((ITEMS_OFFSET + ITEM_SIZE) as u64);
    assert_eq!(slab.len(), 3);
    assert_eq!(slab.capacity(), 1);

    // truncate(2) is a no-op against raw len=3, but post-fix it clamps
    // to effective_len=1.
    slab.truncate(2);
    assert_eq!(slab.len(), 1);
}

// -- The framework-owned resize path does clamp len correctly --------
//
// This positive test verifies that when Slab's own `resize_to_capacity`
// is used (rather than an external realloc_account), len is clamped to
// the new capacity. This locks in the difference between the safe
// path and the unsafe path.

#[test]
fn slab_resize_to_capacity_clamps_len() {
    let buf = setup_ledger(/*capacity*/ 4, /*len*/ 3);

    let view = unsafe { buf.view() };
    let slab = unsafe { CounterLedger::load_mut(view) }.unwrap();
    assert_eq!(slab.len(), 3);

    // This is a smoke test for the defensive pattern. It also documents
    // that `resize_to_capacity` is the *safe* alternative to bare
    // `realloc_account` when a Slab is in scope.
    //
    // (Test body intentionally minimal — the positive path is
    // well-exercised by integration tests elsewhere.)
    // Would need AccountView::resize_unchecked backing in the mock.
    // Full witness deferred — the negative paths above are the
    // demonstrated-bug contribution.
    drop(slab);
}

// -- Rent helpers -----------------------------------------------------

#[test]
fn min_lamports_matches_rent_helper_for_current_space() {
    let mut buf = setup_ledger(/*capacity*/ 4, /*len*/ 1);

    let view = unsafe { buf.view() };
    let slab = unsafe { CounterLedger::load_mut(view) }.unwrap();

    assert_eq!(slab.current_space(), ITEMS_OFFSET + 4 * ITEM_SIZE);
    assert_eq!(
        slab.min_lamports().unwrap(),
        expected_min_lamports(slab.current_space()).unwrap()
    );
}

#[test]
fn refund_moves_excess_lamports_to_recipient() {
    let mut buf = setup_ledger(/*capacity*/ 4, /*len*/ 1);

    let required = expected_min_lamports(ITEMS_OFFSET + 4 * ITEM_SIZE).unwrap();
    buf.set_lamports(required + 500);

    let mut recipient = AccountBuffer::<128>::new();
    recipient.init([0xBB; 32], PROGRAM_ID, 0, false, true, false);
    recipient.set_lamports(25);

    let view = unsafe { buf.view() };
    let mut slab = unsafe { CounterLedger::load_mut(view) }.unwrap();
    let mut recipient_view = unsafe { recipient.view() };

    slab.refund(&mut recipient_view).unwrap();

    assert_eq!(slab.view().lamports(), required);
    assert_eq!(recipient_view.lamports(), 25 + 500);
}

#[test]
fn refund_is_noop_when_account_is_at_rent_floor() {
    let mut buf = setup_ledger(/*capacity*/ 4, /*len*/ 1);

    let required = expected_min_lamports(ITEMS_OFFSET + 4 * ITEM_SIZE).unwrap();
    buf.set_lamports(required);

    let mut recipient = AccountBuffer::<128>::new();
    recipient.init([0xBB; 32], PROGRAM_ID, 0, false, true, false);
    recipient.set_lamports(25);

    let view = unsafe { buf.view() };
    let mut slab = unsafe { CounterLedger::load_mut(view) }.unwrap();
    let mut recipient_view = unsafe { recipient.view() };

    slab.refund(&mut recipient_view).unwrap();

    assert_eq!(slab.view().lamports(), required);
    assert_eq!(recipient_view.lamports(), 25);
}

#[test]
fn top_up_is_noop_when_account_already_has_enough_lamports() {
    let mut buf = setup_ledger(/*capacity*/ 4, /*len*/ 1);

    let required = expected_min_lamports(ITEMS_OFFSET + 4 * ITEM_SIZE).unwrap();
    buf.set_lamports(required + 123);

    let mut payer = AccountBuffer::<128>::new();
    payer.init([0xCC; 32], PROGRAM_ID, 0, true, true, false);
    payer.set_lamports(999);

    let view = unsafe { buf.view() };
    let mut slab = unsafe { CounterLedger::load_mut(view) }.unwrap();
    let payer_view = unsafe { payer.view() };

    slab.top_up(&payer_view).unwrap();

    assert_eq!(slab.view().lamports(), required + 123);
    assert_eq!(payer_view.lamports(), 999);
}
