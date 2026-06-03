use anchor_lang::prelude::*;

declare_id!("Fg6PaFpoGXkYsidMpWTK6W2BeZ7FEfcYkg476zPFsLnS");

#[account]
#[derive(Default, Debug)]
struct Dummy {
    val: u64,
}

fn serialize_dummy(val: u64) -> Vec<u8> {
    let mut v: Vec<u8> = Vec::new();
    Dummy { val }.try_serialize(&mut v).unwrap();

    v
}

// For interface_account.
impl anchor_lang::CheckOwner for Dummy {
    fn check_owner(owner: &Pubkey) -> anchor_lang::Result<()> {
        if owner == &<Dummy as Owner>::owner() {
            Ok(())
        } else {
            Err(anchor_lang::error::ErrorCode::AccountOwnedByWrongProgram.into())
        }
    }
}

#[test]
fn reload_owner_unchanged_updates_data() {
    let init: Vec<u8> = serialize_dummy(10);
    let mut data: Vec<u8> = init.clone();
    let mut lamports: u64 = 1;
    let owner: Pubkey = crate::ID;

    let key: Pubkey = Pubkey::new_unique();
    let acc_info: AccountInfo<'_> =
        AccountInfo::new(&key, false, true, &mut lamports, &mut data, &owner, false);

    // Wrap in Account<Dummy>.
    let mut acc: Account<'_, Dummy> = Account::<Dummy>::try_from(&acc_info).unwrap();
    assert_eq!(acc.val, 10);

    // Simulate CPI side-effect by writing via AccountInfo.
    let new_bytes: Vec<u8> = serialize_dummy(42);
    assert_eq!(new_bytes.len(), acc.to_account_info().data_len());

    {
        let mut d: std::cell::RefMut<'_, &mut [u8]> = acc_info.try_borrow_mut_data().unwrap();
        d.copy_from_slice(&new_bytes);
    }

    // reload() should succeed and reflect the new data.
    acc.reload().unwrap();
    assert_eq!(acc.val, 42);
}

#[test]
fn reload_owner_changed_fails() {
    let init: Vec<u8> = serialize_dummy(1);
    let mut data: Vec<u8> = init.clone();
    let mut lamports: u64 = 1;
    let mut owner: Pubkey = crate::ID;
    let owner_ptr: *mut Pubkey = &mut owner;

    let key: Pubkey = Pubkey::new_unique();
    let acc_info: AccountInfo<'_> =
        AccountInfo::new(&key, false, true, &mut lamports, &mut data, &owner, false);

    let mut acc: Account<'_, Dummy> = Account::<Dummy>::try_from(&acc_info).unwrap();

    // Change the referenced owner value (i.e., simulate reassignment after CPI).
    unsafe {
        *owner_ptr = Pubkey::new_unique();
    }

    // reload() must now error with AccountOwnedByWrongProgram
    let err: anchor_lang::error::Error = acc.reload().unwrap_err();
    assert_eq!(
        err,
        anchor_lang::error::ErrorCode::AccountOwnedByWrongProgram.into()
    );
}

#[test]
fn interface_reload_owner_unchanged_updates_data() {
    use anchor_lang::accounts::interface_account::InterfaceAccount;

    let mut data: Vec<u8> = serialize_dummy(5);
    let mut lamports: u64 = 1;
    let owner: Pubkey = crate::ID;
    let key: Pubkey = Pubkey::new_unique();

    let acc_info: AccountInfo<'_> =
        AccountInfo::new(&key, false, true, &mut lamports, &mut data, &owner, false);

    let mut i_face: InterfaceAccount<'_, Dummy> =
        InterfaceAccount::<Dummy>::try_from(&acc_info).unwrap();
    assert_eq!(i_face.val, 5);

    let new_bytes: Vec<u8> = serialize_dummy(6);

    {
        let mut d: std::cell::RefMut<'_, &mut [u8]> = acc_info.try_borrow_mut_data().unwrap();
        d.copy_from_slice(&new_bytes);
    }

    i_face.reload().unwrap();
    assert_eq!(i_face.val, 6);
}

#[test]
fn reload_error_does_not_mutate_cached_state() {
    let mut data: Vec<u8> = serialize_dummy(7);
    let mut lamports: u64 = 1;

    let mut owner: Pubkey = crate::ID;
    let owner_ptr: *mut Pubkey = &mut owner;

    let key: Pubkey = Pubkey::new_unique();
    let acc_info: AccountInfo<'_> =
        AccountInfo::new(&key, false, true, &mut lamports, &mut data, &owner, false);

    let mut acc: Account<'_, Dummy> = Account::<Dummy>::try_from(&acc_info).unwrap();
    assert_eq!(acc.val, 7);

    // Change owner under the hood so reload() errors.
    unsafe {
        *owner_ptr = Pubkey::new_unique();
    }

    let err: anchor_lang::error::Error = acc.reload().unwrap_err();
    assert_eq!(
        err,
        anchor_lang::error::ErrorCode::AccountOwnedByWrongProgram.into()
    );

    // Ensure inner value wasn't changed by a failing reload.
    assert_eq!(acc.val, 7);
}
