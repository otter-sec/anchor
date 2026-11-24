//! Regression tests proving `AccountLoader` re-checks the account owner on
//! every load, the way `Account::reload` does, so an account that its owning
//! program reassigns during a CPI is refused instead of read as the declared
//! type.

use anchor_lang::{accounts::account_loader::AccountLoader, prelude::*};

declare_id!("Fg6PaFpoGXkYsidMpWTK6W2BeZ7FEfcYkg476zPFsLnS");

#[account(zero_copy)]
#[derive(Default, Debug)]
pub struct ZcStruct {
    pub data: u64,
}

fn account_data() -> Vec<u8> {
    let mut data = ZcStruct::DISCRIMINATOR.to_vec();
    data.extend_from_slice(&7u64.to_le_bytes());
    data
}

#[test]
fn load_owner_unchanged_succeeds() {
    let mut data = account_data();
    let mut lamports: u64 = 1;
    let owner: Pubkey = crate::ID;

    let key = Pubkey::new_unique();
    let acc_info = AccountInfo::new(&key, false, true, &mut lamports, &mut data, &owner, false);
    let loader: AccountLoader<ZcStruct> = AccountLoader::try_from(&acc_info).unwrap();

    assert_eq!(loader.load().unwrap().data, 7);
    loader.load_mut().unwrap().data = 8;
    assert_eq!(loader.load().unwrap().data, 8);
}

#[test]
fn load_owner_changed_fails() {
    let mut data = account_data();
    let mut lamports: u64 = 1;
    let mut owner: Pubkey = crate::ID;
    let owner_ptr: *mut Pubkey = &mut owner;

    let key = Pubkey::new_unique();
    let acc_info = AccountInfo::new(&key, false, true, &mut lamports, &mut data, &owner, false);
    let loader: AccountLoader<ZcStruct> = AccountLoader::try_from(&acc_info).unwrap();

    // Simulate a CPI in which the owning program reassigns the account.
    unsafe {
        *owner_ptr = Pubkey::new_unique();
    }

    assert_eq!(
        loader.load().unwrap_err(),
        ErrorCode::AccountOwnedByWrongProgram.into()
    );
}

#[test]
fn load_mut_owner_changed_fails() {
    let mut data = account_data();
    let mut lamports: u64 = 1;
    let mut owner: Pubkey = crate::ID;
    let owner_ptr: *mut Pubkey = &mut owner;

    let key = Pubkey::new_unique();
    let acc_info = AccountInfo::new(&key, false, true, &mut lamports, &mut data, &owner, false);
    let loader: AccountLoader<ZcStruct> = AccountLoader::try_from(&acc_info).unwrap();

    // Simulate a CPI in which the owning program reassigns the account.
    unsafe {
        *owner_ptr = Pubkey::new_unique();
    }

    assert_eq!(
        loader.load_mut().unwrap_err(),
        ErrorCode::AccountOwnedByWrongProgram.into()
    );
}
