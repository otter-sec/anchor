//! Tests for v1-compatible lamport helper methods.
//!
//! Run: `cargo test -p anchor-lang-v2 --features testing --test lamports`

use {
    anchor_lang_v2::{
        prelude::{BorshAccount, Lamports},
        testing::AccountBuffer,
        wincode::{SchemaRead, SchemaWrite},
        AnchorAccount, Discriminator, Owner,
    },
    pinocchio::address::Address,
    solana_program_error::ProgramError,
};

const PROGRAM_ID: [u8; 32] = [0x42; 32];

#[derive(SchemaRead, SchemaWrite, Default, Clone)]
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

fn setup_counter_buf(buf: &mut AccountBuffer<128>) {
    buf.init([0x44; 32], PROGRAM_ID, 16, false, true, false);
    let mut data = [0u8; 16];
    data[..8].copy_from_slice(Counter::DISCRIMINATOR);
    data[8..16].copy_from_slice(&7u64.to_le_bytes());
    buf.write_data(&data);
}

#[test]
fn account_view_get_add_and_sub_lamports() {
    let buf = AccountBuffer::<128>::new();
    buf.init([0x11; 32], PROGRAM_ID, 0, false, true, false);

    let view = unsafe { buf.view() };
    assert_eq!(view.get_lamports(), 100);

    view.add_lamports(23).unwrap();
    assert_eq!(view.get_lamports(), 123);

    view.sub_lamports(50).unwrap();
    assert_eq!(view.get_lamports(), 73);
}

#[test]
fn borsh_account_get_add_and_sub_lamports() {
    let mut buf = AccountBuffer::<128>::new();
    setup_counter_buf(&mut buf);

    let view = unsafe { buf.view() };
    let account = BorshAccount::<Counter>::load(view).unwrap();
    assert_eq!(account.get_lamports(), 100);

    account.add_lamports(900).unwrap();
    assert_eq!(account.get_lamports(), 1_000);
    assert_eq!(view.get_lamports(), 1_000);

    account.sub_lamports(250).unwrap();
    assert_eq!(account.get_lamports(), 750);
    assert_eq!(view.get_lamports(), 750);
}

#[test]
fn add_lamports_rejects_overflow() {
    let buf = AccountBuffer::<128>::new();
    buf.init([0x22; 32], PROGRAM_ID, 0, false, true, false);
    buf.set_lamports(u64::MAX);

    let view = unsafe { buf.view() };
    assert_eq!(
        view.add_lamports(1).unwrap_err(),
        ProgramError::ArithmeticOverflow
    );
    assert_eq!(view.get_lamports(), u64::MAX);
}

#[test]
fn sub_lamports_rejects_underflow() {
    let buf = AccountBuffer::<128>::new();
    buf.init([0x33; 32], PROGRAM_ID, 0, false, true, false);
    buf.set_lamports(0);

    let view = unsafe { buf.view() };
    assert_eq!(
        view.sub_lamports(1).unwrap_err(),
        ProgramError::ArithmeticOverflow
    );
    assert_eq!(view.get_lamports(), 0);
}
