//! Miri witnesses for `AccountCursor::next()` — the core dup-index
//! resolution logic that the SBF loader writes into the program's
//! input buffer.
//!
//! Uses `SbfInputBuffer` in `common/` to construct a realistic
//! serialized-input layout (`num_accounts` + per-account records)
//! and walks it via the cursor. Exercises:
//!
//! 1. All-non-dup walk — every account gets a fresh `AccountView`
//! 2. Mixed walk with duplicates — dup indices resolve to earlier views
//! 3. First-account short-circuit — the `consumed == 0` path is
//!    hit when the first record's header has a zero byte (looks like
//!    a dup index but it's the very first record)
//! 4. Consecutive dups — second account dups first, third dups second
//!
//! Run: `MIRIFLAGS="-Zmiri-tree-borrows" cargo +nightly miri test -p anchor-lang --test miri_cursor_walk`
//!
//! Tree Borrows compatible: the cursor's 8-byte-alignment step uses
//! strict-provenance `.addr()` + `.add(delta)` rather than an
//! int-to-ptr mask, so the derivation chain survives Miri's check.

use anchor_lang::testing::{AccountRecord, SbfInputBuffer};

use anchor_lang::cursor::AccountCursor;
use core::mem::MaybeUninit;
use pinocchio::account::AccountView;

// Helper: run the closure with a freshly-initialized cursor + lookup array.
// Using a 256-slot lookup matches the framework's real dispatcher frame.
fn with_cursor<F, R>(input: &mut SbfInputBuffer, f: F) -> R
where
    F: FnOnce(&mut AccountCursor, *mut AccountView) -> R,
{
    let mut lookup: [MaybeUninit<AccountView>; 256] = [const { MaybeUninit::uninit() }; 256];
    let lookup_ptr = lookup.as_mut_ptr() as *mut AccountView;
    let mut cursor = unsafe { AccountCursor::new(input.as_mut_ptr(), lookup_ptr) };
    f(&mut cursor, lookup_ptr)
}

// -- Baseline: three non-dup accounts walk correctly ------------------

#[test]
fn all_non_dup_walk() {
    let mut input = SbfInputBuffer::build(&[
        AccountRecord::NonDup {
            address: [0x11; 32],
            owner: [0xAA; 32],
            lamports: 100,
            is_signer: true,
            is_writable: true,
            executable: false,
            data_len: 0,
        },
        AccountRecord::NonDup {
            address: [0x22; 32],
            owner: [0xBB; 32],
            lamports: 200,
            is_signer: false,
            is_writable: true,
            executable: false,
            data_len: 16,
        },
        AccountRecord::NonDup {
            address: [0x33; 32],
            owner: [0xCC; 32],
            lamports: 300,
            is_signer: false,
            is_writable: false,
            executable: false,
            data_len: 32,
        },
    ]);

    with_cursor(&mut input, |cursor, _lookup| {
        let v0 = unsafe { cursor.next() };
        let v1 = unsafe { cursor.next() };
        let v2 = unsafe { cursor.next() };

        assert_eq!(v0.address().to_bytes(), [0x11; 32]);
        assert_eq!(v1.address().to_bytes(), [0x22; 32]);
        assert_eq!(v2.address().to_bytes(), [0x33; 32]);

        assert_eq!(v0.lamports(), 100);
        assert_eq!(v1.lamports(), 200);
        assert_eq!(v2.lamports(), 300);

        assert!(v0.is_signer());
        assert!(!v1.is_signer());

        assert_eq!(cursor.consumed(), 3);
    });
}

// -- Duplicate: second account is a dup of the first -----------------

#[test]
fn dup_resolves_to_earlier_view() {
    let mut input = SbfInputBuffer::build(&[
        AccountRecord::NonDup {
            address: [0x11; 32],
            owner: [0xAA; 32],
            lamports: 500,
            is_signer: false,
            is_writable: true,
            executable: false,
            data_len: 0,
        },
        AccountRecord::Dup { index: 0 },
        AccountRecord::NonDup {
            address: [0x33; 32],
            owner: [0xCC; 32],
            lamports: 300,
            is_signer: false,
            is_writable: false,
            executable: false,
            data_len: 0,
        },
    ]);

    with_cursor(&mut input, |cursor, _| {
        let v0 = unsafe { cursor.next() };
        let v1 = unsafe { cursor.next() };
        let v2 = unsafe { cursor.next() };

        // v1 should resolve to the same AccountView as v0 (same address,
        // same lamports — actually same raw pointer internally).
        assert_eq!(v1.address().to_bytes(), [0x11; 32]);
        assert_eq!(v1.lamports(), 500);
        assert_eq!(v0.address(), v1.address());

        // v2 is a separate non-dup account.
        assert_eq!(v2.address().to_bytes(), [0x33; 32]);
        assert_ne!(v1.address(), v2.address());
    });
}

// -- Chained dup: third account dups second which dups first ---------
//
// In the SBF wire format, each dup record carries the *original*
// index of the duplicated account (not the most recent reference).
// So [non_dup_A, dup→A, dup→A] is valid; [non_dup_A, dup→A, dup→1]
// (where 1 points to the previous dup slot) is also valid and
// should resolve to A.

#[test]
fn chained_dup_resolves_transitively() {
    let mut input = SbfInputBuffer::build(&[
        AccountRecord::NonDup {
            address: [0x77; 32],
            owner: [0xBB; 32],
            lamports: 42,
            is_signer: true,
            is_writable: true,
            executable: false,
            data_len: 0,
        },
        AccountRecord::Dup { index: 0 }, // dups index 0 directly
        AccountRecord::Dup { index: 1 }, // dups index 1 (which was already dup of 0)
    ]);

    with_cursor(&mut input, |cursor, _| {
        let v0 = unsafe { cursor.next() };
        let v1 = unsafe { cursor.next() };
        let v2 = unsafe { cursor.next() };

        // All three must refer to the same underlying account.
        assert_eq!(v0.address(), v1.address());
        assert_eq!(v1.address(), v2.address());
        assert_eq!(v0.lamports(), 42);
        assert_eq!(v2.lamports(), 42);
    });
}

// -- Lookup cache: `lookup[i]` is populated by the i-th next() call --
//
// This is the framework's claim at `cursor.rs:44-45`:
// "Indexed by `consumed` on write and by the serialized dup index
//  on read". Verify by inspecting lookup array after walks.

#[test]
fn lookup_array_populated_by_consumed_index() {
    let mut input = SbfInputBuffer::build(&[
        AccountRecord::NonDup {
            address: [0x01; 32],
            owner: [0; 32],
            lamports: 10,
            is_signer: false,
            is_writable: true,
            executable: false,
            data_len: 0,
        },
        AccountRecord::NonDup {
            address: [0x02; 32],
            owner: [0; 32],
            lamports: 20,
            is_signer: false,
            is_writable: true,
            executable: false,
            data_len: 0,
        },
    ]);

    with_cursor(&mut input, |cursor, lookup| {
        let _ = unsafe { cursor.next() };
        let _ = unsafe { cursor.next() };

        // After two walks, lookup[0] and lookup[1] are populated.
        let slot0 = unsafe { *lookup.add(0) };
        let slot1 = unsafe { *lookup.add(1) };
        assert_eq!(slot0.address().to_bytes(), [0x01; 32]);
        assert_eq!(slot1.address().to_bytes(), [0x02; 32]);
    });
}

// -- walk_n convenience ----------------------------------------------

#[test]
fn walk_n_returns_slice_of_views() {
    let mut input = SbfInputBuffer::build(&[
        AccountRecord::NonDup {
            address: [0xAA; 32],
            owner: [0; 32],
            lamports: 1,
            is_signer: false,
            is_writable: true,
            executable: false,
            data_len: 0,
        },
        AccountRecord::NonDup {
            address: [0xBB; 32],
            owner: [0; 32],
            lamports: 2,
            is_signer: false,
            is_writable: true,
            executable: false,
            data_len: 0,
        },
        AccountRecord::NonDup {
            address: [0xCC; 32],
            owner: [0; 32],
            lamports: 3,
            is_signer: false,
            is_writable: true,
            executable: false,
            data_len: 0,
        },
    ]);

    with_cursor(&mut input, |cursor, _| {
        let (views, _dup_bitvec) = unsafe { cursor.walk_n(3) };
        assert_eq!(views.len(), 3);
        assert_eq!(views[0].address().to_bytes(), [0xAA; 32]);
        assert_eq!(views[1].address().to_bytes(), [0xBB; 32]);
        assert_eq!(views[2].address().to_bytes(), [0xCC; 32]);
    });
}

// -- Non-zero data_len paths -----------------------------------------
//
// The cursor advances `STATIC_ACCOUNT_DATA + data_len + alignment`
// for non-dup records. Verify the pointer math holds across varied
// data_len values (otherwise subsequent accounts would be read from
// the wrong offset).

#[test]
fn non_dup_with_varied_data_len_advances_correctly() {
    let mut input = SbfInputBuffer::build(&[
        AccountRecord::NonDup {
            address: [0x10; 32],
            owner: [0; 32],
            lamports: 1,
            is_signer: false,
            is_writable: true,
            executable: false,
            data_len: 7, // odd-size triggers alignment padding
        },
        AccountRecord::NonDup {
            address: [0x20; 32],
            owner: [0; 32],
            lamports: 2,
            is_signer: false,
            is_writable: true,
            executable: false,
            data_len: 1000,
        },
        AccountRecord::NonDup {
            address: [0x30; 32],
            owner: [0; 32],
            lamports: 3,
            is_signer: false,
            is_writable: true,
            executable: false,
            data_len: 0,
        },
    ]);

    with_cursor(&mut input, |cursor, _| {
        let v0 = unsafe { cursor.next() };
        let v1 = unsafe { cursor.next() };
        let v2 = unsafe { cursor.next() };

        assert_eq!(v0.address().to_bytes(), [0x10; 32]);
        assert_eq!(v1.address().to_bytes(), [0x20; 32]);
        assert_eq!(v2.address().to_bytes(), [0x30; 32]);
    });
}

// -- First-account short-circuit (consumed == 0) ---------------------
//
// The code at cursor.rs:137 has: `if self.consumed == 0 || borrow_state == NON_DUP_MARKER`.
// If the first record's borrow_state byte were accidentally set to
// something OTHER than NON_DUP_MARKER (which should never happen per
// the SBF ABI — first record is always non-dup), the `consumed == 0`
// short-circuit keeps the walk from reading a phantom dup slot.
//
// Test: manually corrupt the first record's borrow_state byte to 0
// (which would look like "dup of index 0" — itself) and verify the
// walk still treats it as non-dup.

#[test]
fn first_account_short_circuit_overrides_wrong_borrow_state_byte() {
    let mut input = SbfInputBuffer::build(&[AccountRecord::NonDup {
        address: [0xEE; 32],
        owner: [0; 32],
        lamports: 99,
        is_signer: false,
        is_writable: true,
        executable: false,
        data_len: 0,
    }]);

    // Corrupt the borrow_state byte at the first record's offset — if
    // the short-circuit didn't fire, the cursor would try to read
    // lookup[0] which is still uninitialized → UB under Miri.
    let offset = input.record_offsets[0];
    input.bytes_mut()[offset] = 0; // simulate "dup of index 0"

    with_cursor(&mut input, |cursor, _| {
        // Should NOT UB — the `consumed == 0` short-circuit takes the
        // non-dup path regardless of the byte value.
        let v = unsafe { cursor.next() };
        assert_eq!(v.address().to_bytes(), [0xEE; 32]);
        assert_eq!(v.lamports(), 99);
    });
}
