//! Regression test for #4858: a `#[account(zero)]` field must not alias an
//! `#[account(init)]` field that points to the same account, because both bind
//! the same buffer to a writable, serializing-on-exit typed account and would
//! overwrite each other on exit.
//!
//! The fix makes pure `init` fields participate in the duplicate-mutable-account
//! key set (see `lang/syn/.../duplicate_mutable_account_keys.rs` and
//! `try_accounts.rs`). This test exercises the generated
//! `DuplicateMutableAccountKeys` trait directly: when `init` is included, the
//! shared key is reported twice and the duplicate check rejects the alias.

use anchor_lang::{prelude::*, DuplicateMutableAccountKeys};

declare_id!("Fg6PaFpoGXkYsidMpWTK6W2BeZ7FEfcYkg476zPFsLnS");

#[account]
#[derive(Default, Debug)]
pub struct Data {
    pub val: u64,
}

#[derive(Accounts)]
pub struct ZeroInitAlias<'info> {
    #[account(zero)]
    pub zeroed: Account<'info, Data>,
    #[account(init, payer = payer, space = 8 + core::mem::size_of::<Data>())]
    pub initialized: Account<'info, Data>,
    #[account(mut)]
    pub payer: Signer<'info>,
    pub system_program: Program<'info, System>,
}

#[test]
fn init_and_zero_same_account_are_flagged_as_duplicate() {
    let shared = Pubkey::new_unique();
    let payer_key = Pubkey::new_unique();

    // `zeroed` account (key = shared)
    let z_key = shared;
    let z_owner = crate::ID;
    let mut z_lamports = 1_000_000u64;
    let mut z_data = vec![0u8; 8 + core::mem::size_of::<Data>()];
    z_data[..8].copy_from_slice(Data::DISCRIMINATOR);
    let z_acc = AccountInfo::new(
        &z_key,
        false,
        true,
        &mut z_lamports,
        &mut z_data,
        &z_owner,
        false,
    );
    let zeroed: Account<Data> = Account::try_from(&z_acc).unwrap();

    // `initialized` account (key = shared)
    let i_key = shared;
    let i_owner = crate::ID;
    let mut i_lamports = 1_000_000u64;
    let mut i_data = vec![0u8; 8 + core::mem::size_of::<Data>()];
    i_data[..8].copy_from_slice(Data::DISCRIMINATOR);
    let i_acc = AccountInfo::new(
        &i_key,
        false,
        true,
        &mut i_lamports,
        &mut i_data,
        &i_owner,
        false,
    );
    let initialized: Account<Data> = Account::try_from(&i_acc).unwrap();

    // payer
    let p_key = payer_key;
    let p_owner = crate::ID;
    let mut p_lamports = 1_000_000u64;
    let mut p_data: Vec<u8> = Vec::new();
    let p_acc = AccountInfo::new(
        &p_key,
        true,
        true,
        &mut p_lamports,
        &mut p_data,
        &p_owner,
        false,
    );
    let payer: Signer = Signer::try_from(&p_acc).unwrap();

    // system program
    let s_key = anchor_lang::solana_program::system_program::ID;
    let s_owner = anchor_lang::solana_program::system_program::ID;
    let mut s_lamports = 1_000_000u64;
    let mut s_data: Vec<u8> = Vec::new();
    let s_acc = AccountInfo::new(
        &s_key,
        false,
        false,
        &mut s_lamports,
        &mut s_data,
        &s_owner,
        true,
    );
    let system_program: Program<System> = Program::try_from(&s_acc).unwrap();

    let accs = ZeroInitAlias {
        zeroed,
        initialized,
        payer,
        system_program,
    };

    let keys = accs.duplicate_mutable_account_keys();
    // Both the `zero` field and the `init` field serialize the same buffer on
    // exit, so the shared key must appear twice. Before the fix the `init`
    // field was excluded and the key appeared only once, letting the alias pass.
    let shared_count = keys.iter().filter(|k| **k == shared).count();
    assert_eq!(
        shared_count, 2,
        "init and zero aliasing the same account must be reported as a duplicate (keys={:?})",
        keys
    );
}

#[test]
fn distinct_accounts_are_not_flagged() {
    let a = Pubkey::new_unique();
    let b = Pubkey::new_unique();
    let payer_key = Pubkey::new_unique();

    // `zeroed` account (key = a)
    let z_key = a;
    let z_owner = crate::ID;
    let mut z_lamports = 1_000_000u64;
    let mut z_data = vec![0u8; 8 + core::mem::size_of::<Data>()];
    z_data[..8].copy_from_slice(Data::DISCRIMINATOR);
    let z_acc = AccountInfo::new(
        &z_key,
        false,
        true,
        &mut z_lamports,
        &mut z_data,
        &z_owner,
        false,
    );
    let zeroed: Account<Data> = Account::try_from(&z_acc).unwrap();

    // `initialized` account (key = b)
    let i_key = b;
    let i_owner = crate::ID;
    let mut i_lamports = 1_000_000u64;
    let mut i_data = vec![0u8; 8 + core::mem::size_of::<Data>()];
    i_data[..8].copy_from_slice(Data::DISCRIMINATOR);
    let i_acc = AccountInfo::new(
        &i_key,
        false,
        true,
        &mut i_lamports,
        &mut i_data,
        &i_owner,
        false,
    );
    let initialized: Account<Data> = Account::try_from(&i_acc).unwrap();

    // payer
    let p_key = payer_key;
    let p_owner = crate::ID;
    let mut p_lamports = 1_000_000u64;
    let mut p_data: Vec<u8> = Vec::new();
    let p_acc = AccountInfo::new(
        &p_key,
        true,
        true,
        &mut p_lamports,
        &mut p_data,
        &p_owner,
        false,
    );
    let payer: Signer = Signer::try_from(&p_acc).unwrap();

    // system program
    let s_key = anchor_lang::solana_program::system_program::ID;
    let s_owner = anchor_lang::solana_program::system_program::ID;
    let mut s_lamports = 1_000_000u64;
    let mut s_data: Vec<u8> = Vec::new();
    let s_acc = AccountInfo::new(
        &s_key,
        false,
        false,
        &mut s_lamports,
        &mut s_data,
        &s_owner,
        true,
    );
    let system_program: Program<System> = Program::try_from(&s_acc).unwrap();

    let accs = ZeroInitAlias {
        zeroed,
        initialized,
        payer,
        system_program,
    };

    let keys = accs.duplicate_mutable_account_keys();
    assert_eq!(keys.iter().filter(|k| **k == a).count(), 1);
    assert_eq!(keys.iter().filter(|k| **k == b).count(), 1);
}
