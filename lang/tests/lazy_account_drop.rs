//! `LazyAccount::unload` must drop the initialized fields instead of leaking them.
//!
//! Run with: `cargo test -p anchor-lang --features lazy-account --test lazy_account_drop`

#![cfg(feature = "lazy-account")]

use {
    anchor_lang::{accounts::lazy_account::LazyAccount, prelude::*},
    std::cell::Cell,
};

declare_id!("Fg6PaFpoGXkYsidMpWTK6W2BeZ7FEfcYkg476zPFsLnS");

thread_local! {
    static DROPS: Cell<usize> = const { Cell::new(0) };
}

fn reset() {
    DROPS.with(|drops| drops.set(0));
}

fn drops() -> usize {
    DROPS.with(Cell::get)
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone)]
pub struct Tracked(Vec<u8>);

impl Drop for Tracked {
    fn drop(&mut self) {
        DROPS.with(|drops| drops.set(drops.get() + 1));
    }
}

#[account]
pub struct TrackedAccount {
    pub a: Tracked,
    pub b: Tracked,
}

/// Builds valid account data. Drops counted while building are reset by the caller.
fn account_data() -> Vec<u8> {
    let account = TrackedAccount {
        a: Tracked(vec![1, 2, 3]),
        b: Tracked(vec![4, 5]),
    };
    let mut data = Vec::new();
    account.try_serialize(&mut data).unwrap();
    data
}

fn with_lazy_account<R>(data: &mut [u8], f: impl FnOnce(&LazyAccount<TrackedAccount>) -> R) -> R {
    let key = Pubkey::new_unique();
    let owner = crate::ID;
    let mut lamports = 0;
    let info = AccountInfo::new(&key, false, true, &mut lamports, data, &owner, false);
    let account = LazyAccount::<TrackedAccount>::try_from(&info).unwrap();
    f(&account)
}

#[test]
fn unload_without_load_drops_nothing() {
    let mut data = account_data();
    reset();
    with_lazy_account(&mut data, |account| {
        account.unload().unwrap();
        account.unload().unwrap();
    });
    assert_eq!(drops(), 0);
}

#[test]
fn partial_load_then_unload_drops_only_loaded_field() {
    let mut data = account_data();
    reset();
    with_lazy_account(&mut data, |account| {
        drop(account.load_a().unwrap());
        account.unload().unwrap();
    });
    assert_eq!(drops(), 1);
}

#[test]
fn full_load_then_unload_drops_whole_account_once() {
    let mut data = account_data();
    reset();
    with_lazy_account(&mut data, |account| {
        drop(account.load().unwrap());
        account.unload().unwrap();
    });
    assert_eq!(drops(), 2);
}

#[test]
fn double_unload_does_not_drop_twice() {
    let mut data = account_data();
    reset();
    with_lazy_account(&mut data, |account| {
        drop(account.load_a().unwrap());
        account.unload().unwrap();
        account.unload().unwrap();
    });
    assert_eq!(drops(), 1);
}

#[test]
fn reload_after_unload_drops_again() {
    let mut data = account_data();
    reset();
    with_lazy_account(&mut data, |account| {
        drop(account.load_a().unwrap());
        account.unload().unwrap();
        drop(account.load_a().unwrap());
        account.unload().unwrap();
    });
    assert_eq!(drops(), 2);
}

#[test]
fn partial_then_full_load_then_unload_does_not_double_drop() {
    let mut data = account_data();
    reset();
    with_lazy_account(&mut data, |account| {
        drop(account.load_a().unwrap());
        drop(account.load().unwrap());
        account.unload().unwrap();
    });
    assert_eq!(drops(), 2);
}

#[test]
fn failed_unload_keeps_initialized_fields() {
    let mut data = account_data();
    reset();
    with_lazy_account(&mut data, |account| {
        drop(account.load_a().unwrap());

        // Corrupt the discriminator so `unload` fails its re-check.
        account.__info.try_borrow_mut_data().unwrap()[0] ^= 0xff;
        assert!(account.unload().is_err());
        assert_eq!(drops(), 0);

        // The cached field is still usable and is dropped by a successful unload.
        drop(account.load_a().unwrap());
        account.__info.try_borrow_mut_data().unwrap()[0] ^= 0xff;
        account.unload().unwrap();
    });
    assert_eq!(drops(), 1);
}
