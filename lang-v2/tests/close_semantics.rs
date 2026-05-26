//! Tests for `BorshAccount::close` semantics.
//!
//! ## What the documentation says
//!
//! pinocchio's `AccountView::close` doc (`solana-account-view-2.0.0:325-329`):
//! > "Zero out the the account's data length, lamports and owner fields,
//! >  effectively closing the account. Note: This does not zero the account
//! >  data. The account data will be zeroed by the runtime at the end of the
//! >  instruction where the account was closed or at the next CPI call."
//!
//! `close_unchecked` (lines 372-380) zeros 48 bytes *before* the data region
//! (owner[32] + lamports[8] + data_len[8]) via one `write_bytes` call.
//! The data region itself is untouched.
//!
//! ## Classic Anchor v1 bug class
//!
//! In Anchor v1, `#[account(close)]` originally did not zero the data,
//! leaving the discriminator bytes intact. An attacker could exploit
//! close-then-reinit-in-same-tx to reuse the account with a different type
//! but the same discriminator. v1 fixed this by writing
//! `CLOSED_ACCOUNT_DISCRIMINATOR = [255; 8]` to the first 8 bytes on close.
//!
//! ## What anchor-v2 does
//!
//! `BorshAccount::close` (`accounts/borsh_account.rs:151-163`):
//! 1. Mark the borrow as Released
//! 2. Move lamports to the destination account
//! 3. Set self's lamports to 0
//! 4. Call `pinocchio::AccountView::close()` — zeros the 48-byte header
//!    (owner, lamports, data_len) but not the data
//!
//! **No `CLOSED_ACCOUNT_DISCRIMINATOR` write. No explicit data zero.**
//! v2 relies on:
//!   (a) `data_len = 0` post-close → any subsequent load rejects with
//!       `AccountDataTooSmall`
//!   (b) SVM-runtime zero-on-allocate for the `create_account` reinit path
//!   (c) SVM-runtime end-of-instruction data zero for closed accounts
//!
//! ## What this test verifies
//!
//! 1. After close, does `data_len == 0` (so load rejects)?
//! 2. After close, do data bytes retain the discriminator? (Expected yes
//!    per pinocchio doc — but the framework trusts SVM to handle this.)
//! 3. Can we construct a realistic "attacker resurrects closed account
//!    without going through create_account" scenario that reads the stale
//!    discriminator?

use {
    anchor_lang_v2::{
        accounts::Slab,
        prelude::BorshAccount,
        testing::AccountBuffer,
        wincode::{SchemaRead, SchemaWrite},
        AnchorAccount, Discriminator, Owner,
    },
    bytemuck::{Pod, Zeroable},
    pinocchio::{account::RuntimeAccount, address::Address},
};

const PROGRAM_ID: [u8; 32] = [0x42; 32];

#[derive(SchemaRead, SchemaWrite, Default, Clone)]
struct Vault {
    authority: [u8; 32],
    balance: u64,
}

impl Owner for Vault {
    const OWNER: Address = Address::new_from_array(PROGRAM_ID);
}

impl Discriminator for Vault {
    // sha256("account:Vault")[..8] = d308e82b02987577
    const DISCRIMINATOR: &'static [u8] = &[0xd3, 0x08, 0xe8, 0x2b, 0x02, 0x98, 0x75, 0x77];
}

fn vault_disc() -> [u8; 8] {
    [0xd3, 0x08, 0xe8, 0x2b, 0x02, 0x98, 0x75, 0x77]
}

fn setup_vault_buf(buf: &mut AccountBuffer<256>) {
    let data_len = 8 + 32 + 8;
    buf.init([0xAA; 32], PROGRAM_ID, data_len, false, true, false);
    let mut data = [0u8; 48];
    data[..8].copy_from_slice(&vault_disc());
    // Vault contents: authority at [8..40], balance at [40..48]
    data[8..40].copy_from_slice(&[0xCC; 32]); // some non-zero authority
    data[40..48].copy_from_slice(&999u64.to_le_bytes()); // balance 999
    buf.write_data(&data);
    buf.set_lamports(1_000_000_000); // rent-exempt-ish
}

// Peek at the raw account bytes (post-close). Bypasses AccountView API.
fn raw_data_bytes(buf: &AccountBuffer<256>) -> &[u8] {
    let header_size = core::mem::size_of::<RuntimeAccount>();
    let total_len = 256;
    // Data region is [header_size .. header_size + (original data_len)].
    // After close, header's data_len is 0, but the raw buffer still holds
    // bytes up to where the original data was written.
    let region_size = core::mem::size_of::<[u8; 48]>();
    unsafe {
        core::slice::from_raw_parts(
            (buf as *const AccountBuffer<256> as *const u8).add(header_size),
            region_size.min(total_len - header_size),
        )
    }
}

// -- Baseline: close sets lamports + data_len + owner to zero ---------

#[test]
fn close_zeros_the_48_byte_header() {
    let mut buf = AccountBuffer::<256>::new();
    setup_vault_buf(&mut buf);

    // Destination account to receive lamports.
    let dest_buf = AccountBuffer::<256>::new();
    dest_buf.init([0xDD; 32], PROGRAM_ID, 0, false, true, false);
    dest_buf.set_lamports(100);

    {
        let view = unsafe { buf.view() };
        let dest_view = unsafe { dest_buf.view() };
        let mut vault = unsafe { BorshAccount::<Vault>::load_mut(view) }.unwrap();
        vault.close(dest_view).unwrap();
    }

    // Post-close state via the RuntimeAccount raw header:
    let raw = unsafe { &*(buf.raw() as *const RuntimeAccount) };
    assert_eq!(raw.lamports, 0, "close should zero self lamports");
    assert_eq!(raw.data_len, 0, "close should zero data_len");
    assert_eq!(
        &raw.owner.to_bytes()[..],
        &[0u8; 32][..],
        "close should zero owner"
    );

    // Destination received the lamports:
    let dest_raw = unsafe { &*(dest_buf.raw() as *const RuntimeAccount) };
    assert_eq!(
        dest_raw.lamports,
        100 + 1_000_000_000,
        "destination should have received all self lamports"
    );
}

// -- Regression: close() scrubs the discriminator bytes in raw memory --

#[test]
fn close_scrubs_discriminator_to_closed_sentinel() {
    let mut buf = AccountBuffer::<256>::new();
    setup_vault_buf(&mut buf);

    let dest_buf = AccountBuffer::<256>::new();
    dest_buf.init([0xDD; 32], PROGRAM_ID, 0, false, true, false);

    {
        let view = unsafe { buf.view() };
        let dest_view = unsafe { dest_buf.view() };
        let mut vault = unsafe { BorshAccount::<Vault>::load_mut(view) }.unwrap();
        vault.close(dest_view).unwrap();
    }

    // Framework perspective: data_len=0, nothing to see.
    let raw = unsafe { &*(buf.raw() as *const RuntimeAccount) };
    assert_eq!(raw.data_len, 0);

    // Post-fix: the first 8 bytes of the data region are overwritten
    // with [u8::MAX; 8] — the "closed account" sentinel. Any stale
    // reload attempt sees this sentinel, which doesn't match any
    // legitimate type's discriminator.
    let data = raw_data_bytes(&buf);
    assert_eq!(
        &data[..8],
        &[u8::MAX; 8][..],
        "close should have scrubbed the discriminator to the closed sentinel"
    );
}

// -- Regression: scrub still runs after `release_borrow()` -----------
//
// `release_borrow + close` is a sanctioned call sequence: a handler can
// commit `self.data` and drop the guard pre-CPI (or before letting the
// derive's exit_accounts run close). If the scrub were gated on a live
// `Mutable` guard, this flow would silently leave the discriminator
// intact in the data region and re-open the close-then-resurrect window
// the scrub exists to defend against.

#[test]
fn close_scrubs_discriminator_even_after_release_borrow() {
    let mut buf = AccountBuffer::<256>::new();
    setup_vault_buf(&mut buf);

    let mut dest_buf = AccountBuffer::<256>::new();
    dest_buf.init([0xDD; 32], PROGRAM_ID, 0, false, true, false);

    {
        let view = unsafe { buf.view() };
        let dest_view = unsafe { dest_buf.view() };
        let mut vault = unsafe { BorshAccount::<Vault>::load_mut(view) }.unwrap();

        // Simulate the handler committing state and dropping the guard
        // (e.g. pre-CPI) before close runs in exit_accounts.
        vault.release_borrow().unwrap();
        vault.close(dest_view).unwrap();
    }

    let data = raw_data_bytes(&buf);
    assert_eq!(
        &data[..8],
        &[u8::MAX; 8][..],
        "scrub must run regardless of borrow state — release_borrow + close is legal and must not \
         leave the discriminator intact",
    );
}

// -- Defense: subsequent load rejects because data_len == 0 ----------

#[test]
fn load_after_close_rejects_with_data_too_small() {
    let mut buf = AccountBuffer::<256>::new();
    setup_vault_buf(&mut buf);

    let dest_buf = AccountBuffer::<256>::new();
    dest_buf.init([0xDD; 32], PROGRAM_ID, 0, false, true, false);

    {
        let view = unsafe { buf.view() };
        let dest_view = unsafe { dest_buf.view() };
        let mut vault = unsafe { BorshAccount::<Vault>::load_mut(view) }.unwrap();
        vault.close(dest_view).unwrap();
    }

    // Even though data bytes retain the disc, the framework rejects
    // because `data_len = 0` (< DISC_LEN=8 → AccountDataTooSmall).
    let view = unsafe { buf.view() };
    let result = BorshAccount::<Vault>::load(view);
    assert!(
        result.is_err(),
        "BorshAccount::load must reject a closed account (owner is [0;32] != program_id, AND \
         data_len=0 < DISC_LEN)"
    );
}

// -- Regression: reloading a closed-then-resurrected account rejects --
//
// With the discriminator scrub in place, even if an attacker manages to
// restore data_len + owner + lamports post-close (through any path), the
// scrubbed [u8::MAX; 8] disc in the data region prevents a reload —
// Vault's discriminator doesn't match the sentinel bytes.

#[test]
fn resurrected_account_reload_rejects_after_disc_scrub() {
    let mut buf = AccountBuffer::<256>::new();
    setup_vault_buf(&mut buf);

    let dest_buf = AccountBuffer::<256>::new();
    dest_buf.init([0xDD; 32], PROGRAM_ID, 0, false, true, false);

    {
        let view = unsafe { buf.view() };
        let dest_view = unsafe { dest_buf.view() };
        let mut vault = unsafe { BorshAccount::<Vault>::load_mut(view) }.unwrap();
        vault.close(dest_view).unwrap();
    }

    // Simulate attacker resurrection: restore data_len + owner + lamports.
    unsafe {
        let raw = &mut *(buf.raw() as *mut RuntimeAccount);
        raw.data_len = 48;
        raw.owner = Address::new_from_array(PROGRAM_ID);
        raw.lamports = 1_000_000_000;
    }

    // Post-fix: disc bytes are [u8::MAX; 8], which doesn't match
    // Vault::DISCRIMINATOR. Load must reject with InvalidAccountData.
    let view = unsafe { buf.view() };
    let result = BorshAccount::<Vault>::load(view);
    assert!(
        result.is_err(),
        "Post-fix: reload must reject because scrubbed disc != Vault::DISCRIMINATOR"
    );
}

// -- Behavior claim: the normal init-after-close path is safe --------

#[test]
fn create_account_zeroes_data_on_allocation() {
    // This test documents the SVM invariant that v2's close relies on:
    // when `create_account` is called on a closed account, the SVM
    // runtime zeroes the data region. We can't exercise the real SVM
    // here, but we can model it — and confirm that the resulting
    // state (zeroed data) would correctly reject stale-disc reload.

    let mut buf = AccountBuffer::<256>::new();
    setup_vault_buf(&mut buf);

    // Simulate SVM close-and-create sequence:
    unsafe {
        let raw = &mut *(buf.raw() as *mut RuntimeAccount);
        raw.lamports = 0;
        raw.data_len = 0;
        raw.owner = Address::new_from_array([0u8; 32]);
    }
    // Now simulate create_account: allocates fresh bytes (zeroed),
    // assigns owner, sets lamports. Importantly, the data region is
    // zeroed — this is the SVM-runtime guarantee close relies on.
    unsafe {
        let raw = &mut *(buf.raw() as *mut RuntimeAccount);
        raw.data_len = 48;
        raw.owner = Address::new_from_array(PROGRAM_ID);
        raw.lamports = 1_000_000_000;

        // SVM zeros the data region on allocate. Simulate:
        let data_ptr = (&buf as *const AccountBuffer<256> as *mut u8)
            .add(core::mem::size_of::<RuntimeAccount>());
        core::ptr::write_bytes(data_ptr, 0, 48);
    }

    // After this correct SVM behavior, the first 8 data bytes are
    // zero — no valid discriminator. Load rejects.
    let view = unsafe { buf.view() };
    let result = BorshAccount::<Vault>::load(view);
    assert!(
        result.is_err(),
        "after create_account's SVM-mandated zero-on-allocate, load must reject (data[..8] = [0; \
         8] != Vault::DISCRIMINATOR)"
    );
}

// =====================================================================
// Slab::close — same defenses as BorshAccount::close.
//
// These mirror the BorshAccount tests above but exercise the Slab path:
//   1. discriminator scrub to [u8::MAX; 8]
//   2. is_mutable flip so post-close DerefMut panics
//   3. reload after attacker-resurrection rejects on the scrubbed disc
// =====================================================================

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable, Default)]
struct CounterHeader {
    value: u64,
    bump: u8,
    _pad: [u8; 7],
}

impl Owner for CounterHeader {
    const OWNER: Address = Address::new_from_array(PROGRAM_ID);
}

impl Discriminator for CounterHeader {
    // sha256("account:CounterHeader")[..8] — distinct from Vault's.
    const DISCRIMINATOR: &'static [u8] = &[0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88];
}

fn counter_disc() -> [u8; 8] {
    [0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88]
}

fn setup_counter_buf(buf: &mut AccountBuffer<256>) {
    let data_len = 8 + core::mem::size_of::<CounterHeader>();
    buf.init([0xAA; 32], PROGRAM_ID, data_len, false, true, false);
    let mut data = [0u8; 24];
    data[..8].copy_from_slice(&counter_disc());
    // Non-zero header bytes so a stale read would be visible.
    data[8..16].copy_from_slice(&999u64.to_le_bytes());
    data[16] = 7; // bump
    buf.write_data(&data);
    buf.set_lamports(1_000_000_000);
}

#[test]
fn slab_close_scrubs_discriminator_to_closed_sentinel() {
    let mut buf = AccountBuffer::<256>::new();
    setup_counter_buf(&mut buf);

    let mut dest_buf = AccountBuffer::<256>::new();
    dest_buf.init([0xDD; 32], PROGRAM_ID, 0, false, true, false);

    {
        let view = unsafe { buf.view() };
        let dest_view = unsafe { dest_buf.view() };
        let mut counter = unsafe { Slab::<CounterHeader>::load_mut(view) }.unwrap();
        counter.close(dest_view).unwrap();
    }

    let raw = unsafe { &*(buf.raw() as *const RuntimeAccount) };
    assert_eq!(raw.data_len, 0);

    let data = raw_data_bytes(&buf);
    assert_eq!(
        &data[..8],
        &[u8::MAX; 8][..],
        "Slab::close must scrub the discriminator to the closed sentinel"
    );
}

#[test]
#[should_panic(expected = "Slab<H, T> mutably dereferenced but loaded read-only")]
fn slab_close_flips_is_mutable_so_deref_mut_panics() {
    let mut buf = AccountBuffer::<256>::new();
    setup_counter_buf(&mut buf);

    let mut dest_buf = AccountBuffer::<256>::new();
    dest_buf.init([0xDD; 32], PROGRAM_ID, 0, false, true, false);
    let view = unsafe { buf.view() };
    let dest_view = unsafe { dest_buf.view() };
    let mut counter = unsafe { Slab::<CounterHeader>::load_mut(view) }.unwrap();
    counter.close(dest_view).unwrap();

    // Post-close: is_mutable should be flipped to false. DerefMut must
    // panic instead of silently writing through the cached header_ptr to
    // memory pinocchio is about to mark closed.
    let _ = &mut *counter;
}

#[test]
fn slab_resurrected_account_reload_rejects_after_disc_scrub() {
    let mut buf = AccountBuffer::<256>::new();
    setup_counter_buf(&mut buf);

    let mut dest_buf = AccountBuffer::<256>::new();
    dest_buf.init([0xDD; 32], PROGRAM_ID, 0, false, true, false);

    {
        let view = unsafe { buf.view() };
        let dest_view = unsafe { dest_buf.view() };
        let mut counter = unsafe { Slab::<CounterHeader>::load_mut(view) }.unwrap();
        counter.close(dest_view).unwrap();
    }

    // Attacker resurrection: restore data_len + owner + lamports without
    // re-allocating (so SVM's zero-on-allocate doesn't run).
    unsafe {
        let raw = &mut *(buf.raw() as *mut RuntimeAccount);
        raw.data_len = (8 + core::mem::size_of::<CounterHeader>()) as u64;
        raw.owner = Address::new_from_array(PROGRAM_ID);
        raw.lamports = 1_000_000_000;
    }

    let view = unsafe { buf.view() };
    let result = Slab::<CounterHeader>::load(view);
    assert!(
        result.is_err(),
        "Post-fix: reload must reject because scrubbed disc != CounterHeader::DISCRIMINATOR"
    );
}
