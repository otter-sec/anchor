//! Miri soundness witnesses for `AccountView` aliasing patterns used
//! across `anchor-lang`.
//!
//! Uses the `AccountBuffer` scaffold in `common/` to construct mock
//! `AccountView` instances without running under the SBF loader.
//!
//! Run: `MIRIFLAGS="-Zmiri-tree-borrows" cargo +nightly miri test -p anchor-lang --test miri_account_view_aliasing`
//!
//! These tests address the Class C inventory items that were blocked on
//! AccountView scaffolding (INVENTORY.md §6.12, findings B-NN).

use anchor_lang::testing::AccountBuffer;

// -- Baseline: one view, simple operations ----------------------------

#[test]
fn single_view_read_and_write_lamports() {
    let buf = AccountBuffer::<256>::new();
    buf.init([7; 32], [3; 32], 0, false, true, false);
    let mut view = unsafe { buf.view() };
    assert_eq!(view.lamports(), 100);
    view.set_lamports(500);
    assert_eq!(view.lamports(), 500);
}

// -- AccountView is Copy: two views sharing a raw pointer -----------
//
// pinocchio enables `copy` feature in anchor-lang so `AccountView`
// is `Copy`. Two `AccountView` values can alias the same
// `RuntimeAccount`. This is the foundation of v2's "unchecked CPI"
// claim: the Rust borrow checker doesn't see the aliasing because
// writes go through raw pointers, not through `&mut RuntimeAccount`
// references.

#[test]
fn two_copies_alias_same_runtime_account() {
    let buf = AccountBuffer::<256>::new();
    buf.init([1; 32], [0; 32], 0, false, true, false);
    let mut view_a = unsafe { buf.view() };
    let mut view_b = view_a; // Copy — aliases same raw pointer

    // Write via view_a, read via view_b — must observe the write.
    view_a.set_lamports(999);
    assert_eq!(view_b.lamports(), 999);

    // Write via view_b, read via view_a.
    view_b.set_lamports(42);
    assert_eq!(view_a.lamports(), 42);
}

// -- Interleaved reads + writes through distinct view copies --------
//
// Stress the Tree Borrows model with rapid cycling. If the retag on
// `set_lamports` invalidates view_b's "read permission", this fails.

#[test]
fn interleaved_mut_through_copies_cycles() {
    let buf = AccountBuffer::<256>::new();
    buf.init([2; 32], [0; 32], 0, false, true, false);
    let mut view_a = unsafe { buf.view() };
    let view_b = view_a;
    let mut view_c = view_a;

    for i in 0..50u64 {
        view_a.set_lamports(i);
        assert_eq!(view_b.lamports(), i);
        view_c.set_lamports(i * 2);
        assert_eq!(view_a.lamports(), i * 2);
        assert_eq!(view_b.lamports(), i * 2);
    }
}

// -- Shared-ref reads beside a mutable alias ------------------------
//
// Anchor-v2 Slab holds an AccountView + caches pointers derived from
// it. If a program takes `&slab` (thus `&AccountView`) while another
// code path holds a `&mut AccountView` copy and writes, does Tree
// Borrows reject? With Copy semantics, the `&mut AccountView` refers
// to a local copy — no aliasing violation on the Rust side. Writes
// reach the shared RuntimeAccount through the raw pointer.

#[test]
fn shared_ref_read_while_copy_writes() {
    let buf = AccountBuffer::<256>::new();
    buf.init([5; 32], [0; 32], 0, false, true, false);
    let view_shared = unsafe { buf.view() };
    let mut view_mut_copy = view_shared;

    assert_eq!(view_shared.lamports(), 100);
    view_mut_copy.set_lamports(777);
    assert_eq!(view_shared.lamports(), 777);
    view_mut_copy.set_lamports(888);
    assert_eq!(view_shared.lamports(), 888);
}

// -- `address()` / `owner()` return stable references ---------------
//
// Fields the underlying `RuntimeAccount` has that aren't modified by
// `set_lamports` should remain stable under Tree Borrows through
// concurrent lamport writes via aliased copies.

#[test]
fn address_reference_stable_across_lamport_writes() {
    let buf = AccountBuffer::<256>::new();
    buf.init([0xAB; 32], [0; 32], 0, false, true, false);
    let view_a = unsafe { buf.view() };
    let mut view_b = view_a;

    let addr_before = *view_a.address();
    view_b.set_lamports(1);
    let addr_after = *view_a.address();
    assert_eq!(addr_before, addr_after);
    view_b.set_lamports(2);
    let addr_later = *view_a.address();
    assert_eq!(addr_after, addr_later);
}

// -- Multiple buffers, multiple views: no cross-contamination ------

#[test]
fn distinct_buffers_do_not_alias() {
    let buf1 = AccountBuffer::<256>::new();
    let buf2 = AccountBuffer::<256>::new();
    buf1.init([1; 32], [0; 32], 0, false, true, false);
    buf2.init([2; 32], [0; 32], 0, false, true, false);

    let mut v1 = unsafe { buf1.view() };
    let mut v2 = unsafe { buf2.view() };

    v1.set_lamports(100);
    v2.set_lamports(200);

    assert_eq!(v1.lamports(), 100);
    assert_eq!(v2.lamports(), 200);
    assert_ne!(v1.address(), v2.address());
}

// -- Borrow-state byte behavior --------------------------------------
//
// `NOT_BORROWED` = 255. pinocchio's `try_borrow` / `try_borrow_mut`
// decrement / set this byte as a lock. Verify the byte can be set and
// read directly through the buffer (Miri verifies no UB in the
// pointer path).

#[test]
fn borrow_state_byte_round_trip() {
    let buf = AccountBuffer::<256>::new();
    buf.init([1; 32], [0; 32], 0, false, true, false);
    buf.set_borrow_state(254); // simulate "one immutable borrow outstanding"

    let view = unsafe { buf.view() };
    assert!(view.is_writable());
    // Reading the borrow-state byte itself isn't part of AccountView's
    // public API; the state is consulted by try_borrow/try_borrow_mut
    // methods. Indirect check: the view still exposes its metadata
    // normally while the byte is non-sentinel.
    assert_eq!(view.lamports(), 100);
}
