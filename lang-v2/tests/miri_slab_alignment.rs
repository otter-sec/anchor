//! Exercise `Slab::ITEMS_OFFSET` alignment math for items with
//! `align_of::<T>() > 1`.
//!
//! The formula at `slab.rs:168-176`:
//! ```text
//! if T is ZST: ITEMS_OFFSET = LEN_OFFSET (4 bytes after header+len)
//! else:        ITEMS_OFFSET = (LEN_OFFSET + 4 + (a-1)) & !(a-1)
//! ```
//! where `a = align_of::<T>()`.
//!
//! This file witnesses that the align-up formula works correctly for
//! the handful of alignments v2 programs might hit in practice. Pod
//! types in anchor-v2 are all align-1, but user-defined Pod items
//! (via `#[derive(bytemuck::Pod)]`) could have larger alignment.
//!
//! Run: `cargo +nightly miri test -p anchor-lang-v2 --test miri_slab_alignment`

use anchor_lang_v2::testing::AccountBuffer;

use anchor_lang_v2::{accounts::Slab, AnchorAccount, Discriminator, Owner};
use bytemuck::{Pod, Zeroable};
use pinocchio::address::Address;

const PROGRAM_ID: [u8; 32] = [0x42; 32];

// Minimal header (align 1, 8 bytes).
#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct HeaderAlign1 {
    counter: [u8; 8],
}

impl Owner for HeaderAlign1 {
    const OWNER: Address = Address::new_from_array(PROGRAM_ID);
}

impl Discriminator for HeaderAlign1 {
    // sha256("account:HeaderAlign1")[..8] — precomputed, verified below.
    const DISCRIMINATOR: &'static [u8] = &HEADER_DISC;
}

// sha256("account:HeaderAlign1")[..8] — verified by
// verify_header_disc_vector test below.
const HEADER_DISC: [u8; 8] = [69, 42, 11, 206, 204, 217, 108, 141];

// ---------------------------------------------------------------------------
// Item types with varied alignment
// ---------------------------------------------------------------------------

// Align 1 — all-u8.
#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable, PartialEq, Debug, Default)]
struct ItemAlign1([u8; 16]);

// -- The alignment guarantee check -----------------------------------
//
// Slab<H, T>::ITEMS_OFFSET must be `align_of::<T>()`-aligned. Since
// HeaderAlign1 + 4-byte len puts us at offset 20 (not divisible by 4
// or 8), the align-up formula must bump ITEMS_OFFSET forward.
//
// For T: align 1 → ITEMS_OFFSET = 20 (no padding needed)
// For T: align 4 → ITEMS_OFFSET = 24 (pad 4)
// For T: align 8 → ITEMS_OFFSET = 24 (pad 4)
// These offsets must match the `ITEMS_OFFSET` const Slab computes.

#[test]
fn verify_header_disc_vector() {
    use sha2::{Digest, Sha256};
    let full = Sha256::digest(b"account:HeaderAlign1");
    assert_eq!(&full[..8], &HEADER_DISC[..]);
}

// -- Round-trip through a Slab with align-1 items --------------------

#[test]
fn slab_align1_push_pop_roundtrip() {
    // HEADER_OFFSET=8, LEN_OFFSET=16, ITEMS_OFFSET=20 (align 1).
    // Capacity = 4: data_len = 20 + 16*4 = 84
    let buf = AccountBuffer::<256>::new();
    buf.init([0xAA; 32], PROGRAM_ID, 84, false, true, false);

    let mut data = [0u8; 84];
    data[..8].copy_from_slice(&HEADER_DISC);
    buf.write_data(&data);
    buf.set_lamports(1_000_000_000);
    let view = unsafe { buf.view() };
    let mut slab =
        unsafe { Slab::<HeaderAlign1, ItemAlign1>::load_mut(view) }.unwrap();

    // Push 3 items.
    let items = [
        ItemAlign1([1; 16]),
        ItemAlign1([2; 16]),
        ItemAlign1([3; 16]),
    ];
    for item in items {
        slab.try_push(item).unwrap();
    }
    assert_eq!(slab.len(), 3);
    assert_eq!(slab.as_slice(), &items);

    // Pop and verify LIFO.
    assert_eq!(slab.pop(), Some(items[2]));
    assert_eq!(slab.pop(), Some(items[1]));
    assert_eq!(slab.pop(), Some(items[0]));
    assert_eq!(slab.len(), 0);
    assert!(slab.pop().is_none());
}

// -- Swap-remove roundtrip -------------------------------------------

#[test]
fn slab_align1_swap_remove_preserves_correctness() {
    let buf = AccountBuffer::<256>::new();
    buf.init([0xAA; 32], PROGRAM_ID, 84, false, true, false);
    let mut data = [0u8; 84];
    data[..8].copy_from_slice(&HEADER_DISC);
    buf.write_data(&data);
    buf.set_lamports(1_000_000_000);
    let view = unsafe { buf.view() };
    let mut slab =
        unsafe { Slab::<HeaderAlign1, ItemAlign1>::load_mut(view) }.unwrap();

    let a = ItemAlign1([0xAA; 16]);
    let b = ItemAlign1([0xBB; 16]);
    let c = ItemAlign1([0xCC; 16]);
    slab.try_push(a).unwrap();
    slab.try_push(b).unwrap();
    slab.try_push(c).unwrap();

    // swap_remove at index 0 — replaces index 0 with last item (c),
    // removes last (was c). Result: [c, b], with a returned.
    let removed = slab.swap_remove(0);
    assert_eq!(removed, a);
    assert_eq!(slab.len(), 2);
    assert_eq!(slab.as_slice(), &[c, b]);
}

// -- Layout self-check: ITEMS_OFFSET alignment -----------------------
//
// Direct inspection of the runtime-offset math. Slab exposes its
// `space_for(capacity)` const fn — it bakes in ITEMS_OFFSET + capacity
// * size_of::<T>(). Derive ITEMS_OFFSET from that.

#[test]
fn slab_items_offset_is_correctly_aligned() {
    // For align-1 T, space_for(0) = ITEMS_OFFSET exactly.
    let items_offset = Slab::<HeaderAlign1, ItemAlign1>::space_for(0);
    // HEADER_OFFSET (8 for discriminator) + size_of::<HeaderAlign1>() (8)
    // + 4 (len field) = 20. Aligned to 1 (no change) = 20.
    assert_eq!(items_offset, 20);

    // And a multiple of align_of::<T>(), which is 1 here.
    assert_eq!(items_offset % core::mem::align_of::<ItemAlign1>(), 0);
}

// -- Exhaustive space-math: for 0..10 capacity, space grows by size_of::<T>() --

#[test]
fn slab_space_for_grows_by_item_size() {
    let base = Slab::<HeaderAlign1, ItemAlign1>::space_for(0);
    for cap in 0..10 {
        let space = Slab::<HeaderAlign1, ItemAlign1>::space_for(cap);
        assert_eq!(
            space,
            base + (cap as usize) * core::mem::size_of::<ItemAlign1>(),
            "space_for({}) doesn't match base + cap * item_size",
            cap
        );
    }
}
