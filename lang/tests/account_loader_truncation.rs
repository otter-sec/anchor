//! Regression tests proving AccountLoader accessors return structured errors instead of panicking on truncated accounts.

use anchor_lang::{accounts::account_loader::AccountLoader, prelude::*};

declare_id!("Fg6PaFpoGXkYsidMpWTK6W2BeZ7FEfcYkg476zPFsLnS");

#[account(zero_copy)]
#[derive(Default, Debug)]
pub struct ZcStruct {
    pub data: u64,
}

macro_rules! setup_truncated_account {
    ($key:ident, $owner:ident, $lamports:ident, $data:ident, $account_info:ident) => {
        let $key = Pubkey::new_unique();
        let $owner = crate::ID;
        let mut $lamports = 0;
        let mut $data = ZcStruct::DISCRIMINATOR.to_vec();

        #[allow(unused_variables)]
        let $account_info = AccountInfo::new(
            &$key,
            false,
            true,
            &mut $lamports,
            &mut $data,
            &$owner,
            false,
        );
    };
}

#[test]
fn test_load_truncated() {
    setup_truncated_account!(key, owner, lamports, data, account_info);
    let loader: AccountLoader<ZcStruct> = AccountLoader::try_from(&account_info).unwrap();
    assert_eq!(
        loader.load().unwrap_err(),
        ErrorCode::AccountDidNotDeserialize.into()
    );
}

#[test]
fn test_load_mut_truncated() {
    setup_truncated_account!(key, owner, lamports, data, account_info);
    let loader: AccountLoader<ZcStruct> = AccountLoader::try_from(&account_info).unwrap();
    assert_eq!(
        loader.load_mut().unwrap_err(),
        ErrorCode::AccountDidNotDeserialize.into()
    );
}

#[test]
fn test_load_init_truncated() {
    setup_truncated_account!(key, owner, lamports, data, account_info);
    let loader: AccountLoader<ZcStruct> =
        AccountLoader::try_from_unchecked(&crate::ID, &account_info).unwrap();
    assert_eq!(
        loader.load_init().unwrap_err(),
        ErrorCode::AccountDidNotDeserialize.into()
    );
}

#[test]
fn test_load_valid_full_size() {
    let key = Pubkey::new_unique();
    let owner = crate::ID;
    let mut lamports = 0;
    let mut data = vec![0u8; 8 + std::mem::size_of::<ZcStruct>()];
    data[..8].copy_from_slice(ZcStruct::DISCRIMINATOR);

    let account_info = AccountInfo::new(&key, false, true, &mut lamports, &mut data, &owner, false);
    let loader: AccountLoader<ZcStruct> = AccountLoader::try_from(&account_info).unwrap();

    assert!(loader.load().is_ok());
}
