//! On exit, an account whose ownership moved away from the program during the
//! instruction (e.g. reassigned via CPI) is not written: unchanged data is
//! tolerated, an unpersisted modification is an error.

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

fn wrong_owner() -> Error {
    ErrorCode::AccountOwnedByWrongProgram.into()
}

/// Loads `Data` while owned by this program, optionally mutates it in memory,
/// optionally reassigns the account (as a CPI would), then runs `exit`.
fn account_exit(mutate: bool, new_owner: Option<&Pubkey>) -> (Result<()>, Vec<u8>) {
    let key = Pubkey::new_unique();
    let owner = crate::ID;
    let mut lamports = 0;
    let mut data = Data::DISCRIMINATOR.to_vec();
    data.extend_from_slice(&0u64.to_le_bytes());
    let result = {
        let info = AccountInfo::new(&key, false, true, &mut lamports, &mut data, &owner, false);
        let mut account: Account<Data> = Account::try_from_unchecked(&info).unwrap();
        if mutate {
            account.value = 42;
        }
        if let Some(new_owner) = new_owner {
            info.assign(new_owner);
        }
        AccountsExit::exit(&account, &crate::ID)
    };
    (result, data)
}

#[test]
fn account_exit_persists_when_owned() {
    let (result, data) = account_exit(true, None);
    result.unwrap();
    assert_eq!(&data[8..], &42u64.to_le_bytes());
}

#[test]
fn account_exit_tolerates_unchanged_when_owner_changed() {
    let (result, data) = account_exit(false, Some(&Pubkey::new_unique()));
    result.unwrap();
    assert_eq!(&data[8..], &0u64.to_le_bytes());
}

#[test]
fn account_exit_rejects_modified_when_owner_changed() {
    let (result, data) = account_exit(true, Some(&Pubkey::new_unique()));
    assert_eq!(result.unwrap_err(), wrong_owner());
    assert_eq!(&data[8..], &0u64.to_le_bytes());
}

fn account_loader_exit(
    with_discriminator: bool,
    new_owner: Option<&Pubkey>,
) -> (Result<()>, Vec<u8>) {
    account_loader_exit_sized(
        8 + std::mem::size_of::<ZcData>(),
        with_discriminator,
        new_owner,
    )
}

fn account_loader_exit_sized(
    len: usize,
    with_discriminator: bool,
    new_owner: Option<&Pubkey>,
) -> (Result<()>, Vec<u8>) {
    let key = Pubkey::new_unique();
    let owner = crate::ID;
    let mut lamports = 0;
    let mut data = vec![0u8; len];
    if with_discriminator {
        data[..8].copy_from_slice(ZcData::DISCRIMINATOR);
    }
    let result = {
        let info = AccountInfo::new(&key, false, true, &mut lamports, &mut data, &owner, false);
        let loader: AccountLoader<ZcData> =
            AccountLoader::try_from_unchecked(&crate::ID, &info).unwrap();
        if let Some(new_owner) = new_owner {
            info.assign(new_owner);
        }
        AccountsExit::exit(&loader, &crate::ID)
    };
    (result, data)
}

#[test]
fn account_loader_exit_persists_when_owned() {
    let (result, data) = account_loader_exit(false, None);
    result.unwrap();
    assert_eq!(&data[..8], ZcData::DISCRIMINATOR);
}

#[test]
fn account_loader_exit_tolerates_unchanged_when_owner_changed() {
    let (result, data) = account_loader_exit(true, Some(&Pubkey::new_unique()));
    result.unwrap();
    assert_eq!(&data[..8], ZcData::DISCRIMINATOR);
}

#[test]
fn account_loader_exit_rejects_modified_when_owner_changed() {
    let (result, data) = account_loader_exit(false, Some(&Pubkey::new_unique()));
    assert_eq!(result.unwrap_err(), wrong_owner());
    assert_eq!(&data[..8], &[0u8; 8]);
}

#[test]
fn account_loader_exit_rejects_truncated_when_owner_changed() {
    let (result, _) = account_loader_exit_sized(8, true, Some(&Pubkey::new_unique()));
    assert_eq!(
        result.unwrap_err(),
        ErrorCode::AccountDidNotDeserialize.into()
    );
}

/// Migrates in memory, optionally persists with `exit` while still owned,
/// optionally reassigns the account, then runs `exit`.
fn migration_exit(persist_first: bool, new_owner: Option<&Pubkey>) -> (Result<()>, Vec<u8>) {
    let key = Pubkey::new_unique();
    let owner = crate::ID;
    let mut lamports = 0;
    let mut data = Data::DISCRIMINATOR.to_vec();
    data.extend_from_slice(&[0u8; 16]);
    let result = {
        let info = AccountInfo::new(&key, false, true, &mut lamports, &mut data, &owner, false);
        let mut account: Migration<Data, DataV2> = Migration::try_from_unchecked(&info).unwrap();
        account.migrate(DataV2::default()).unwrap();
        if persist_first {
            AccountsExit::exit(&account, &crate::ID).unwrap();
        }
        if let Some(new_owner) = new_owner {
            info.assign(new_owner);
        }
        AccountsExit::exit(&account, &crate::ID)
    };
    (result, data)
}

#[test]
fn migration_exit_persists_when_owned() {
    let (result, data) = migration_exit(false, None);
    result.unwrap();
    assert_eq!(&data[..8], DataV2::DISCRIMINATOR);
}

#[test]
fn migration_exit_tolerates_persisted_when_owner_changed() {
    let (result, data) = migration_exit(true, Some(&Pubkey::new_unique()));
    result.unwrap();
    assert_eq!(&data[..8], DataV2::DISCRIMINATOR);
}

#[test]
fn migration_exit_rejects_unpersisted_when_owner_changed() {
    let (result, data) = migration_exit(false, Some(&Pubkey::new_unique()));
    assert_eq!(result.unwrap_err(), wrong_owner());
    assert_eq!(&data[..8], Data::DISCRIMINATOR);
}

#[cfg(feature = "lazy-account")]
mod lazy_account {
    use {super::*, anchor_lang::accounts::lazy_account::LazyAccount};

    fn lazy_account_exit(mutate: bool, new_owner: Option<&Pubkey>) -> (Result<()>, Vec<u8>) {
        let key = Pubkey::new_unique();
        let owner = crate::ID;
        let mut lamports = 0;
        let mut data = Data::DISCRIMINATOR.to_vec();
        data.extend_from_slice(&0u64.to_le_bytes());
        let result = {
            let info = AccountInfo::new(&key, false, true, &mut lamports, &mut data, &owner, false);
            let account: LazyAccount<Data> = LazyAccount::try_from_unchecked(&info).unwrap();
            if mutate {
                account.load_mut().unwrap().value = 42;
            }
            if let Some(new_owner) = new_owner {
                info.assign(new_owner);
            }
            account.exit(&crate::ID)
        };
        (result, data)
    }

    #[test]
    fn lazy_account_exit_persists_when_owned() {
        let (result, data) = lazy_account_exit(true, None);
        result.unwrap();
        assert_eq!(&data[8..], &42u64.to_le_bytes());
    }

    #[test]
    fn lazy_account_exit_tolerates_unchanged_when_owner_changed() {
        let (result, data) = lazy_account_exit(false, Some(&Pubkey::new_unique()));
        result.unwrap();
        assert_eq!(&data[8..], &0u64.to_le_bytes());
    }

    #[test]
    fn lazy_account_exit_rejects_modified_when_owner_changed() {
        let (result, data) = lazy_account_exit(true, Some(&Pubkey::new_unique()));
        assert_eq!(result.unwrap_err(), wrong_owner());
        assert_eq!(&data[8..], &0u64.to_le_bytes());
    }
}
