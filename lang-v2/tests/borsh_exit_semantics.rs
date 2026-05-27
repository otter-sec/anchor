//! Tests for `BorshAccount` exit / release / reacquire semantics.
//!
//! 1. `release_borrow` commits `self.data` to the buffer before
//!    dropping the guard, so a CPI invoked between release and
//!    reacquire sees the user's in-memory mutations.
//! 2. `reacquire_borrow_mut` re-runs the load-time invariants
//!    (owner / disc / size) and re-deserializes `self.data` from
//!    the buffer, picking up any CPI-induced changes. A CPI that
//!    reassigned the account or swapped its disc is rejected.
//! 3. `exit` serializes `self.data` through the held mutable guard.
//!    On a closed account (lamports == 0) it is a no-op.
//! 4. `exit`'s stale-size detection fires when an external resize
//!    happened without the user going through release / reacquire.
//!    It does not fire on content-only out-of-band mutation (same
//!    size) — by design, exit treats in-memory `self.data` as
//!    authoritative.

use {
    anchor_lang_v2::{
        prelude::BorshAccount,
        testing::AccountBuffer,
        wincode::{SchemaRead, SchemaWrite},
        AnchorAccount, Discriminator, Owner,
    },
    pinocchio::{account::RuntimeAccount, address::Address},
    solana_program_error::ProgramError,
};

const PROGRAM_ID: [u8; 32] = [0x42; 32];
const FOREIGN_PROGRAM_ID: [u8; 32] = [0x24; 32];

#[derive(SchemaRead, SchemaWrite, Default, Clone, PartialEq, Debug)]
struct Counter {
    value: u64,
}

impl Owner for Counter {
    const OWNER: Address = Address::new_from_array(PROGRAM_ID);
}

impl Discriminator for Counter {
    // sha256("account:Counter")[..8]
    const DISCRIMINATOR: &'static [u8] = &[0xff, 0xb0, 0x04, 0xf5, 0xbc, 0xfd, 0x7c, 0x19];
}

#[derive(SchemaRead, SchemaWrite, Default, Clone, PartialEq, Debug)]
struct ForeignCounter {
    value: u64,
}

impl Owner for ForeignCounter {
    const OWNER: Address = Address::new_from_array(FOREIGN_PROGRAM_ID);
}

impl Discriminator for ForeignCounter {
    const DISCRIMINATOR: &'static [u8] = &[0x23, 0xaa, 0x41, 0x17, 0x83, 0x62, 0xdd, 0x09];
}

#[derive(SchemaRead, SchemaWrite, Default, Clone, PartialEq, Debug)]
struct ShrinkableBytes {
    items: Vec<u8>,
}

impl Owner for ShrinkableBytes {
    const OWNER: Address = Address::new_from_array(PROGRAM_ID);
}

impl Discriminator for ShrinkableBytes {
    const DISCRIMINATOR: &'static [u8] = &[0x5f, 0x81, 0x5c, 0x91, 0xb1, 0x7a, 0x4d, 0xa0];
}

fn counter_disc() -> [u8; 8] {
    [0xff, 0xb0, 0x04, 0xf5, 0xbc, 0xfd, 0x7c, 0x19]
}

fn foreign_counter_disc() -> [u8; 8] {
    [0x23, 0xaa, 0x41, 0x17, 0x83, 0x62, 0xdd, 0x09]
}

fn shrinkable_bytes_disc() -> [u8; 8] {
    [0x5f, 0x81, 0x5c, 0x91, 0xb1, 0x7a, 0x4d, 0xa0]
}

fn setup_counter_buf(buf: &mut AccountBuffer<256>, initial_value: u64) {
    // Layout: disc(8) + wincode(Counter) = disc(8) + u64(8) = 16 bytes
    // (byte-identical to borsh for Counter — single u64, no length prefix).
    let data_len = 16;
    buf.init([0xAA; 32], PROGRAM_ID, data_len, false, true, false);
    let mut data = [0u8; 16];
    data[..8].copy_from_slice(&counter_disc());
    data[8..16].copy_from_slice(&initial_value.to_le_bytes());
    buf.write_data(&data);
    buf.set_lamports(1_000_000_000);
}

fn setup_foreign_counter_buf(buf: &mut AccountBuffer<256>, initial_value: u64) {
    let data_len = 16;
    buf.init([0xAA; 32], FOREIGN_PROGRAM_ID, data_len, false, true, false);
    let mut data = [0u8; 16];
    data[..8].copy_from_slice(&foreign_counter_disc());
    data[8..16].copy_from_slice(&initial_value.to_le_bytes());
    buf.write_data(&data);
    buf.set_lamports(1_000_000_000);
}

fn setup_shrinkable_bytes_buf(buf: &mut AccountBuffer<256>, items: &[u8], reserved_tail: &[u8]) {
    let serialized_len = 4 + items.len();
    let data_len = 8 + serialized_len + reserved_tail.len();
    buf.init([0xAA; 32], PROGRAM_ID, data_len, false, true, false);

    let mut data = Vec::with_capacity(data_len);
    data.extend_from_slice(&shrinkable_bytes_disc());
    data.extend_from_slice(&(items.len() as u32).to_le_bytes());
    data.extend_from_slice(items);
    data.extend_from_slice(reserved_tail);
    buf.write_data(&data);
    buf.set_lamports(1_000_000_000);
}

// Raw data access bypassing the AccountView API (for out-of-band mutation
// and inspection in these tests).
fn set_data_bytes(buf: &mut AccountBuffer<256>, offset: usize, bytes: &[u8]) {
    let header = core::mem::size_of::<RuntimeAccount>();
    let start = header + offset;
    unsafe {
        let base = buf.raw() as *mut u8;
        core::ptr::copy_nonoverlapping(bytes.as_ptr(), base.add(start), bytes.len());
    }
}

fn read_data_bytes(buf: &AccountBuffer<256>, offset: usize, len: usize) -> Vec<u8> {
    let header = core::mem::size_of::<RuntimeAccount>();
    let start = header + offset;
    unsafe {
        let base = buf as *const AccountBuffer<256> as *const u8;
        core::slice::from_raw_parts(base.add(start), len).to_vec()
    }
}

// -- 1. Exit writes self.data back to the account --------------------

#[test]
fn exit_writes_modified_in_memory_state_to_guard() {
    let mut buf = AccountBuffer::<256>::new();
    setup_counter_buf(&mut buf, 42);

    {
        let view = unsafe { buf.view() };
        let mut acct = unsafe { BorshAccount::<Counter>::load_mut(view) }.unwrap();
        assert_eq!(acct.value, 42);
        acct.value = 999;
        acct.exit().unwrap();
    }

    // Read back — the serialized value in the guard should be 999.
    let bytes = read_data_bytes(&buf, 8, 8);
    assert_eq!(u64::from_le_bytes(bytes.try_into().unwrap()), 999);
}

#[test]
fn exit_serializes_mutably_loaded_foreign_owned_account() {
    let mut buf = AccountBuffer::<256>::new();
    setup_foreign_counter_buf(&mut buf, 42);

    {
        let view = unsafe { buf.view() };
        let mut acct = unsafe { BorshAccount::<ForeignCounter>::load_mut(view) }.unwrap();
        assert_eq!(acct.value, 42);
        acct.value = 999;
        acct.exit().unwrap();
    }

    let bytes = read_data_bytes(&buf, 8, 8);
    assert_eq!(u64::from_le_bytes(bytes.try_into().unwrap()), 999);
}

#[test]
fn release_borrow_serializes_mutably_loaded_foreign_owned_account() {
    let mut buf = AccountBuffer::<256>::new();
    setup_foreign_counter_buf(&mut buf, 42);

    {
        let view = unsafe { buf.view() };
        let mut acct = unsafe { BorshAccount::<ForeignCounter>::load_mut(view) }.unwrap();
        assert_eq!(acct.value, 42);
        acct.value = 999;
        acct.release_borrow().unwrap();
    }

    let bytes = read_data_bytes(&buf, 8, 8);
    assert_eq!(u64::from_le_bytes(bytes.try_into().unwrap()), 999);
}

// -- 2. Stale detection: content-only change is NOT detected ---------
//
// If someone mutates guard bytes without changing data_len, the
// "belt-and-braces" heuristic doesn't fire. exit serializes self.data
// (pre-mutation state) and clobbers the external change.

#[test]
// Simulates out-of-band data mutation while `BorshAccount` holds a live
// `RefMut<[u8]>` from `try_borrow_mut`. The real-world scenario (SVM host
// writes during a CPI) happens outside Rust's aliasing model — no Rust
// code performs the concurrent write — so Tree Borrows has no way to
// represent it. The Rust-side simulation is inherently UB and Miri
// correctly rejects it. Covered under regular `cargo test`.
#[cfg_attr(
    miri,
    ignore = "simulates external mutation by violating Rust's exclusive-borrow invariant — \
              inexpressible under Tree Borrows; covered under cargo test"
)]
fn stale_detection_misses_content_only_out_of_band_mutation() {
    let mut buf = AccountBuffer::<256>::new();
    setup_counter_buf(&mut buf, 42);

    let view = unsafe { buf.view() };
    let mut acct = unsafe { BorshAccount::<Counter>::load_mut(view) }.unwrap();
    assert_eq!(acct.value, 42);

    // Out-of-band: somehow the bytes at data[8..16] get mutated while
    // the BorshAccount is still held. (In normal v2 flow, the borrow
    // would prevent this; the scenario here is contrived to check
    // what exit would do if content changed without a size change.)
    set_data_bytes(&mut buf, 8, &555u64.to_le_bytes());

    // self.data still reflects the load-time value, 42.
    assert_eq!(acct.value, 42);

    // No modifications via the API → in-memory self.data stays at 42.
    // exit serializes self.data → guard. The external 555 write is
    // overwritten back to 42.
    acct.exit().unwrap();

    let bytes = read_data_bytes(&buf, 8, 8);
    assert_eq!(
        u64::from_le_bytes(bytes.try_into().unwrap()),
        42,
        "exit overwrites out-of-band content with in-memory self.data; the size-based stale \
         detection does not catch content-only mutations"
    );
}

// -- 3. Regression: reacquire_borrow_mut REFRESHES self.data from the buffer --
//
// Post-fix: reacquire_borrow_mut re-deserializes self.data from the
// fresh guard. CPI-induced changes during the release window are
// preserved in self.data (not silently overwritten by exit()).

#[test]
fn reacquire_refreshes_self_data_from_cpi_changes() {
    let mut buf = AccountBuffer::<256>::new();
    setup_counter_buf(&mut buf, 42);

    let view = unsafe { buf.view() };
    let mut acct = unsafe { BorshAccount::<Counter>::load_mut(view) }.unwrap();

    // Step 1: user modifies self.data.
    acct.value = 100;

    // Step 2: user releases the borrow (e.g., to CPI).
    acct.release_borrow().unwrap();

    // Step 3: simulated CPI mutates the account's data bytes.
    // Writes 777 as the new u64 value.
    set_data_bytes(&mut buf, 8, &777u64.to_le_bytes());

    // Step 4: user reacquires the borrow.
    acct.reacquire_borrow_mut().unwrap();

    // Step 5: reacquire re-deserialized from buffer, so self.data
    // now reflects the CPI's write of 777, not the user's pre-release
    // modification of 100.
    assert_eq!(
        acct.value, 777,
        "post-fix: reacquire_borrow_mut refreshes self.data from the buffer — the CPI's write of \
         777 is reflected, not clobbered"
    );

    // Post-fix: exit() runs on refreshed self.data (777), so the CPI's
    // change persists. If the user wanted to override with 100 instead,
    // they should modify self.value after reacquire.
    acct.exit().unwrap();
    let bytes = read_data_bytes(&buf, 8, 8);
    assert_eq!(
        u64::from_le_bytes(bytes.try_into().unwrap()),
        777,
        "exit serialized refreshed self.data (777) — CPI's write preserved"
    );
}

// -- 3b. Regression: reacquire_borrow_mut REJECTS a changed discriminator --
//
// If the released-window CPI rewrote the discriminator while leaving
// Borsh-compatible payload bytes in place, `reacquire_borrow_mut` must
// not silently succeed — that would leave us holding `BorshAccount<T>`
// over an account that no longer validates as `T`. Post-fix:
// `reacquire_borrow_mut` re-runs the discriminator check.

#[test]
fn reacquire_rejects_when_discriminator_changes_during_release() {
    let mut buf = AccountBuffer::<256>::new();
    setup_counter_buf(&mut buf, 42);

    let view = unsafe { buf.view() };
    let mut acct = unsafe { BorshAccount::<Counter>::load_mut(view) }.unwrap();

    acct.release_borrow().unwrap();

    // Simulated CPI rewrites the discriminator to a different account
    // type's bytes while leaving the u64 payload intact (which would
    // happily Borsh-decode as Counter::value).
    let foreign_disc = [0xDE, 0xAD, 0xBE, 0xEF, 0xDE, 0xAD, 0xBE, 0xEF];
    set_data_bytes(&mut buf, 0, &foreign_disc);

    let result = acct.reacquire_borrow_mut();
    assert_eq!(
        result.err(),
        Some(ProgramError::InvalidAccountData),
        "reacquire_borrow_mut must reject a discriminator that no longer matches T — otherwise \
         the program continues operating on a BorshAccount<T> over an incompatible account."
    );
}

// -- 3c. Regression: reacquire_borrow_mut REJECTS an owner change ----
//
// Even with disc + payload still valid, a released-window CPI could
// transfer the account to a different owner. Without an owner check,
// `reacquire_borrow_mut` would silently accept the now-foreign-owned
// account as `BorshAccount<T>`. Post-fix: `reacquire_borrow_mut` re-runs
// `view.owned_by(&T::OWNER)` against the live header.

#[test]
fn reacquire_rejects_when_owner_changes_during_release() {
    let mut buf = AccountBuffer::<256>::new();
    setup_counter_buf(&mut buf, 42);

    let view = unsafe { buf.view() };
    let mut acct = unsafe { BorshAccount::<Counter>::load_mut(view) }.unwrap();

    acct.release_borrow().unwrap();

    // Simulated CPI transfers ownership to a different program. The
    // discriminator and Borsh payload are untouched — only the owner
    // changes.
    buf.set_owner([0xFE; 32]);

    let result = acct.reacquire_borrow_mut();
    assert_eq!(
        result.err(),
        Some(ProgramError::IllegalOwner),
        "reacquire_borrow_mut must reject when the on-chain owner no longer matches `T::OWNER` — \
         otherwise the program continues holding BorshAccount<T> over a foreign-owned account."
    );
}

// -- 4. Exit on a closed account is a no-op --------------------------
//
// The first line of exit checks `view.lamports() == 0` and bails.
// This prevents serializing back to a closed/about-to-close account.

#[test]
fn exit_on_zero_lamport_account_is_noop() {
    let mut buf = AccountBuffer::<256>::new();
    setup_counter_buf(&mut buf, 42);

    let view = unsafe { buf.view() };
    let mut acct = unsafe { BorshAccount::<Counter>::load_mut(view) }.unwrap();

    // Modify self.data.
    acct.value = 999;

    // Simulate close: set lamports to 0 (bypassing normal close path
    // for test purposes).
    buf.set_lamports(0);

    // exit should be a no-op — closed account's data should not be
    // serialized over.
    acct.exit().unwrap();

    let bytes = read_data_bytes(&buf, 8, 8);
    assert_eq!(
        u64::from_le_bytes(bytes.try_into().unwrap()),
        42,
        "exit on zero-lamport account must not write back — correct"
    );
}

// -- 5. release_borrow commits self.data to the buffer -------------
//
// CPIs invoked between release_borrow and reacquire_borrow_mut must
// observe the user's pre-CPI in-memory mutations. release_borrow
// serializes self.data through the held guard before dropping it.

#[test]
fn release_borrow_commits_in_memory_changes_to_buffer() {
    let mut buf = AccountBuffer::<256>::new();
    setup_counter_buf(&mut buf, 42);

    let view = unsafe { buf.view() };
    let mut acct = unsafe { BorshAccount::<Counter>::load_mut(view) }.unwrap();

    acct.value = 100;
    acct.release_borrow().unwrap();

    let bytes = read_data_bytes(&buf, 8, 8);
    assert_eq!(
        u64::from_le_bytes(bytes.try_into().unwrap()),
        100,
        "release_borrow must serialize self.data so a subsequent CPI sees the in-memory mutations"
    );
}

#[test]
fn exit_zeroes_bytes_between_new_and_old_serialized_lengths() {
    let mut buf = AccountBuffer::<256>::new();
    setup_shrinkable_bytes_buf(&mut buf, &[1, 2, 3], &[0xAA, 0xBB]);

    {
        let view = unsafe { buf.view() };
        let mut acct = unsafe { BorshAccount::<ShrinkableBytes>::load_mut(view) }.unwrap();
        acct.items = vec![1, 2];
        acct.exit().unwrap();
    }

    let data = read_data_bytes(&buf, 0, 17);
    let mut expected = Vec::new();
    expected.extend_from_slice(&shrinkable_bytes_disc());
    expected.extend_from_slice(&2u32.to_le_bytes());
    expected.extend_from_slice(&[1, 2, 0, 0xAA, 0xBB]);
    assert_eq!(
        data, expected,
        "exit must zero the old serialized tail without clearing reserved account capacity"
    );
}

#[test]
fn release_borrow_zeroes_bytes_between_new_and_old_serialized_lengths() {
    let mut buf = AccountBuffer::<256>::new();
    setup_shrinkable_bytes_buf(&mut buf, &[1, 2, 3], &[0xAA, 0xBB]);

    let view = unsafe { buf.view() };
    let mut acct = unsafe { BorshAccount::<ShrinkableBytes>::load_mut(view) }.unwrap();
    acct.items.clear();
    acct.release_borrow().unwrap();

    let data = read_data_bytes(&buf, 0, 17);
    let mut expected = Vec::new();
    expected.extend_from_slice(&shrinkable_bytes_disc());
    expected.extend_from_slice(&0u32.to_le_bytes());
    expected.extend_from_slice(&[0, 0, 0, 0xAA, 0xBB]);
    assert_eq!(
        data, expected,
        "release_borrow must commit a shrink with zero padding before any CPI observes the bytes"
    );
}

#[test]
#[should_panic(expected = "account borrow released (closed)")]
fn deref_mut_panics_after_release_borrow() {
    let mut buf = AccountBuffer::<256>::new();
    setup_counter_buf(&mut buf, 42);

    let view = unsafe { buf.view() };
    let mut acct = unsafe { BorshAccount::<Counter>::load_mut(view) }.unwrap();

    acct.release_borrow().unwrap();
    acct.value = 100;
}

// -- 6. Stale detection DOES fire on size change --------------------
//
// Positive test: the belt-and-braces heuristic works for the case it
// was designed for. If data_len changes between load and exit, the
// heuristic detects it and reacquires.

#[test]
fn stale_detection_fires_on_data_len_change() {
    let mut buf = AccountBuffer::<256>::new();
    setup_counter_buf(&mut buf, 42);

    let view = unsafe { buf.view() };
    let mut acct = unsafe { BorshAccount::<Counter>::load_mut(view) }.unwrap();

    acct.value = 100;

    // Simulate an external realloc that grew data_len from 16 to 32
    // without going through the borrow system.
    buf.set_data_len(32);

    // exit should detect stale (guard.len()=16 != data_len=32),
    // release + reacquire + serialize.
    acct.exit().unwrap();

    // First 16 bytes should reflect the updated state.
    let bytes = read_data_bytes(&buf, 8, 8);
    assert_eq!(u64::from_le_bytes(bytes.try_into().unwrap()), 100);
}

// -- 7. Codec cursor advance contract ---------------------------------
//
// `AnchorAccountSerialize::{serialize, deserialize}` is documented to
// advance the cursor through the bytes it consumes. Wincode's
// `config::deserialize` takes the input by value and would silently leave
// the caller's `&mut &[u8]` unchanged; the impl uses `SchemaRead::get`
// instead so the contract holds. Regression test: encode two values
// back-to-back, decode them in sequence using the codec directly, and
// assert both the values and the post-decode cursor.

#[test]
fn codec_round_trip_advances_cursor() {
    use anchor_lang_v2::accounts::{AnchorAccountSerialize, BorshSerializer};

    let mut buf = [0u8; 32];
    // Serialize: two Counter values back-to-back.
    {
        let mut write: &mut [u8] = &mut buf;
        BorshSerializer::serialize(&Counter { value: 0xAA }, &mut write).unwrap();
        BorshSerializer::serialize(&Counter { value: 0xBB }, &mut write).unwrap();
        // Writer should have advanced 16 bytes (2 × u64).
        assert_eq!(write.len(), 32 - 16);
    }
    // Deserialize: cursor should advance past each value.
    {
        let mut read: &[u8] = &buf[..];
        let a: Counter = BorshSerializer::deserialize(&mut read).unwrap();
        assert_eq!(a.value, 0xAA);
        assert_eq!(
            read.len(),
            32 - 8,
            "deserialize must advance the input cursor"
        );
        let b: Counter = BorshSerializer::deserialize(&mut read).unwrap();
        assert_eq!(b.value, 0xBB);
        assert_eq!(read.len(), 32 - 16);
    }
}

// -- 8. realloc-shrink below discriminator is rejected ----------------
//
// `realloc_account` does not (and cannot, given Slab over external
// header-only types) enforce that `new_space >= DISC_LEN`. The
// BorshAccount-shaped reacquire path must reject it: leaving the buffer
// shorter than the 8-byte discriminator truncates `T::DISCRIMINATOR`
// on chain, bricks the account, and panics exit()'s `guard[DISC_LEN..]`
// slice index. The Solana runtime rolls back the rejected transaction.

#[test]
fn reacquire_guard_only_rejects_buffer_shorter_than_discriminator() {
    let mut buf = AccountBuffer::<256>::new();
    setup_counter_buf(&mut buf, 42);

    let view = unsafe { buf.view() };
    let mut acct = unsafe { BorshAccount::<Counter>::load_mut(view) }.unwrap();

    // Simulate `realloc_account(new_space = 4)`: shrink below DISC_LEN.
    acct.release_borrow().unwrap();
    buf.set_data_len(4);

    let err = acct.reacquire_guard_only().expect_err(
        "reacquire_guard_only must reject a post-resize buffer shorter than DISC_LEN — otherwise \
         the discriminator is truncated on-chain and exit()'s guard[DISC_LEN..] panics",
    );
    assert!(matches!(err, ProgramError::AccountDataTooSmall));
}

// Exit path: an out-of-band shrink below DISC_LEN triggers the stale-
// detection branch in `exit()`, which calls `reacquire_guard_only`. The
// added check propagates the error rather than panicking on the
// subsequent `guard[DISC_LEN..]` slice.

#[test]
fn exit_rejects_external_shrink_below_discriminator() {
    let mut buf = AccountBuffer::<256>::new();
    setup_counter_buf(&mut buf, 42);

    let view = unsafe { buf.view() };
    let mut acct = unsafe { BorshAccount::<Counter>::load_mut(view) }.unwrap();
    acct.value = 100;

    // Simulate an external resize to 4 bytes (below DISC_LEN = 8).
    buf.set_data_len(4);

    let err = acct
        .exit()
        .expect_err("exit must surface the under-DISC_LEN buffer as an error, not panic");
    assert!(matches!(err, ProgramError::AccountDataTooSmall));
}
