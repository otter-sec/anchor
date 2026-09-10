//! `exit` must not re-serialize an account whose ownership moved away from the program
//! mid-instruction (e.g. reassigned via CPI); such a write fails under direct mapping.

use anchor_lang::{
    accounts::{account_loader::AccountLoader, migration::Migration},
    prelude::*,
};

declare_id!("Fg6PaFpoGXkYsidMpWTK6W2BeZ7FEfcYkg476zPFsLnS");

#[account]
#[derive(Default, Debug)]
pub struct Data {
    pub value: u64,
}

#[account]
#[derive(Default, Debug)]
pub struct DataV2 {
    pub value: u64,
    pub extra: u64,
}

#[account(zero_copy)]
#[derive(Default, Debug)]
pub struct ZcData {
    pub value: u64,
}

/// Deserializes `Data` while owned by this program, mutates it, optionally reassigns the
/// account to `new_owner` (as a CPI would), then runs `exit` and returns the raw data.
fn exit_account(new_owner: Option<&Pubkey>) -> Vec<u8> {
    let key = Pubkey::new_unique();
    let owner = crate::ID;
    let mut lamports = 0;
    let mut data = Data::DISCRIMINATOR.to_vec();
    data.extend_from_slice(&0u64.to_le_bytes());
    {
        let info = AccountInfo::new(&key, false, true, &mut lamports, &mut data, &owner, false);
        let mut account: Account<Data> = Account::try_from_unchecked(&info).unwrap();
        account.value = 42;
        if let Some(new_owner) = new_owner {
            info.assign(new_owner);
        }
        AccountsExit::exit(&account, &crate::ID).unwrap();
    }
    data
}

fn exit_account_loader(new_owner: Option<&Pubkey>) -> Vec<u8> {
    let key = Pubkey::new_unique();
    let owner = crate::ID;
    let mut lamports = 0;
    let mut data = vec![0u8; 8 + std::mem::size_of::<ZcData>()];
    {
        let info = AccountInfo::new(&key, false, true, &mut lamports, &mut data, &owner, false);
        let loader: AccountLoader<ZcData> =
            AccountLoader::try_from_unchecked(&crate::ID, &info).unwrap();
        if let Some(new_owner) = new_owner {
            info.assign(new_owner);
        }
        AccountsExit::exit(&loader, &crate::ID).unwrap();
    }
    data
}

#[test]
fn account_exit_persists_when_owned() {
    let data = exit_account(None);
    assert_eq!(&data[8..], &42u64.to_le_bytes());
}

#[test]
fn account_exit_skips_when_owner_changed() {
    let data = exit_account(Some(&Pubkey::new_unique()));
    assert_eq!(&data[8..], &0u64.to_le_bytes());
}

#[test]
fn account_loader_exit_persists_when_owned() {
    let data = exit_account_loader(None);
    assert_eq!(&data[..8], ZcData::DISCRIMINATOR);
}

#[test]
fn account_loader_exit_skips_when_owner_changed() {
    let data = exit_account_loader(Some(&Pubkey::new_unique()));
    assert_eq!(&data[..8], &[0u8; 8]);
}

/// Runs `exit` while the account data is immutably borrowed: a write attempt fails with
/// `AccountBorrowFailed`, a skipped exit succeeds. Returns whether `exit` succeeded.
fn exit_with_data_borrowed<'info>(
    info: &AccountInfo<'info>,
    exit: impl FnOnce() -> Result<()>,
) -> bool {
    let _guard = info.try_borrow_data().unwrap();
    exit().is_ok()
}

fn migration_exit_succeeds(new_owner: Option<&Pubkey>) -> bool {
    let key = Pubkey::new_unique();
    let owner = crate::ID;
    let mut lamports = 0;
    let mut data = Data::DISCRIMINATOR.to_vec();
    data.extend_from_slice(&0u64.to_le_bytes());
    let info = AccountInfo::new(&key, false, true, &mut lamports, &mut data, &owner, false);
    let mut account: Migration<Data, DataV2> = Migration::try_from_unchecked(&info).unwrap();
    account.migrate(DataV2::default()).unwrap();
    if let Some(new_owner) = new_owner {
        info.assign(new_owner);
    }
    exit_with_data_borrowed(&info, || AccountsExit::exit(&account, &crate::ID))
}

#[test]
fn migration_exit_writes_when_owned() {
    assert!(!migration_exit_succeeds(None));
}

#[test]
fn migration_exit_skips_when_owner_changed() {
    assert!(migration_exit_succeeds(Some(&Pubkey::new_unique())));
}

#[cfg(feature = "lazy-account")]
mod lazy_account {
    use super::*;
    use anchor_lang::accounts::lazy_account::LazyAccount;

    fn lazy_account_exit_succeeds(new_owner: Option<&Pubkey>) -> bool {
        let key = Pubkey::new_unique();
        let owner = crate::ID;
        let mut lamports = 0;
        let mut data = Data::DISCRIMINATOR.to_vec();
        data.extend_from_slice(&0u64.to_le_bytes());
        let info = AccountInfo::new(&key, false, true, &mut lamports, &mut data, &owner, false);
        let account: LazyAccount<Data> = LazyAccount::try_from_unchecked(&info).unwrap();
        if let Some(new_owner) = new_owner {
            info.assign(new_owner);
        }
        exit_with_data_borrowed(&info, || account.exit(&crate::ID))
    }

    #[test]
    fn lazy_account_exit_writes_when_owned() {
        assert!(!lazy_account_exit_succeeds(None));
    }

    #[test]
    fn lazy_account_exit_skips_when_owner_changed() {
        assert!(lazy_account_exit_succeeds(Some(&Pubkey::new_unique())));
    }
}
