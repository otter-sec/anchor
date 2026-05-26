//! Miri soundness witness for `Slab<H>` construction + field access.
//!
//! Targets the `header_ptr` provenance claim in `slab.rs`:
//! `Slab::from_ref` derives `header_ptr` via `view.data_ptr().add(HEADER_OFFSET)`
//! then casts to `*mut H`. Under Tree Borrows, this must preserve
//! write provenance for the subsequent `Deref<Target=H>` /
//! `DerefMut<Target=H>` accesses through the cached pointer.
//!
//! Miri will flag if the pointer derivation loses write permission —
//! for example, if `data_ptr()` (returning `*const u8`) were used
//! instead of `data_mut_ptr()`, the cast to `*mut H` would strip the
//! "shared → unique" transition that Tree Borrows requires for mut
//! access.
//!
//! Run: `MIRIFLAGS="-Zmiri-tree-borrows" cargo +nightly miri test -p anchor-lang-v2 --test miri_slab_construction`

use anchor_lang_v2::testing::AccountBuffer;

use anchor_lang_v2::{accounts::Account, AnchorAccount, Discriminator, Owner};
use bytemuck::{Pod, Zeroable};
use pinocchio::address::Address;

// Fixture: a minimal `#[account]`-compatible header. We write the
// trait impls by hand rather than going through the derive because
// the derive pulls in additional machinery we don't need for the
// header_ptr-provenance witness.

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
    // sha256("account:Counter")[..8] — reused vector from disc_vectors.rs
    const DISCRIMINATOR: &'static [u8] = &[0xff, 0xb0, 0x04, 0xf5, 0xbc, 0xfd, 0x7c, 0x19];
}

// Account<T> = Slab<T, HeaderOnly>. The core header-only variant.
type CounterAccount = Account<Counter>;

fn disc() -> [u8; 8] {
    [0xff, 0xb0, 0x04, 0xf5, 0xbc, 0xfd, 0x7c, 0x19]
}

fn setup_counter_buffer() -> AccountBuffer<128> {
    let buf = AccountBuffer::<128>::new();
    // Layout: header + disc (8) + Counter (16) = small.
    let data_len = 8 + core::mem::size_of::<Counter>();
    buf.init(
        [0xAA; 32], // address
        PROGRAM_ID, // owner = Counter::OWNER
        data_len, /*is_signer*/ false, /*is_writable*/ true, /*executable*/ false,
    );
    // Write the discriminator into the first 8 bytes of data.
    let mut data = [0u8; 24];
    data[..8].copy_from_slice(&disc());
    // Counter fields default to zero — no need to write.
    buf.write_data(&data);
    buf
}

// -- Baseline: load succeeds ------------------------------------------

#[test]
fn load_succeeds_with_correct_disc_and_owner() {
    let buf = setup_counter_buffer();
    let view = unsafe { buf.view() };
    let acct = CounterAccount::load(view).unwrap();
    assert_eq!(acct.value, 0);
    assert_eq!(acct.bump, 0);
}

#[test]
fn load_rejects_wrong_owner() {
    let buf = setup_counter_buffer();
    // Corrupt the owner — any value != PROGRAM_ID fails the check.
    buf.init(
        [0xAA; 32], [0xFF; 32], // wrong owner
        24, false, true, false,
    );
    // Re-write data after init (init wiped our data).
    let mut data = [0u8; 24];
    data[..8].copy_from_slice(&disc());
    buf.write_data(&data);

    let view = unsafe { buf.view() };
    assert!(CounterAccount::load(view).is_err());
}

#[test]
fn load_rejects_wrong_disc() {
    let buf = AccountBuffer::<128>::new();
    let data_len = 8 + core::mem::size_of::<Counter>();
    buf.init([0xAA; 32], PROGRAM_ID, data_len, false, true, false);
    // Write wrong discriminator.
    let mut data = [0u8; 24];
    data[..8].copy_from_slice(&[0xDE, 0xAD, 0xBE, 0xEF, 0, 0, 0, 0]);
    buf.write_data(&data);

    let view = unsafe { buf.view() };
    assert!(CounterAccount::load(view).is_err());
}

// -- The header_ptr provenance witness --------------------------------
//
// `load_mut` caches `header_ptr` derived via `data_mut_ptr()`. Then
// `DerefMut<Target=Counter>` writes through that pointer. Under Tree
// Borrows, this must be sound — if `header_ptr` were derived via
// `data_ptr()` (read-only provenance), the `&mut Counter` write would
// be UB.

#[test]
fn load_mut_deref_mut_writes_propagate_to_bytes() {
    let buf = setup_counter_buffer();
    let view = unsafe { buf.view() };

    {
        let mut acct = unsafe { CounterAccount::load_mut(view) }.unwrap();
        acct.value = 0xDEADBEEF;
        acct.bump = 0xAB;
    } // acct drops here

    // Re-read through a fresh load to verify the writes hit the buffer.
    let view2 = unsafe { buf.view() };
    let acct2 = CounterAccount::load(view2).unwrap();
    assert_eq!(acct2.value, 0xDEADBEEF);
    assert_eq!(acct2.bump, 0xAB);
}

// -- Interleaved reads + mut writes -----------------------------------
//
// Drop the mut slab, reload as immutable, and verify the writes stuck.
// Exercises the "release borrow on drop" contract.

#[test]
fn drop_mut_then_load_immutable_sees_writes() {
    let buf = setup_counter_buffer();

    {
        let view = unsafe { buf.view() };
        let mut acct = unsafe { CounterAccount::load_mut(view) }.unwrap();
        acct.value = 99;
    }

    let view = unsafe { buf.view() };
    let acct = CounterAccount::load(view).unwrap();
    assert_eq!(acct.value, 99);
}

// -- Multiple mut-load cycles ----------------------------------------

#[test]
fn multiple_mut_load_cycles_preserve_state() {
    let buf = setup_counter_buffer();

    for i in 0u64..20 {
        let view = unsafe { buf.view() };
        let mut acct = unsafe { CounterAccount::load_mut(view) }.unwrap();
        acct.value = i * 10;
        drop(acct);

        let view_r = unsafe { buf.view() };
        let acct_r = CounterAccount::load(view_r).unwrap();
        assert_eq!(acct_r.value, i * 10);
    }
}
