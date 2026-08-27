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

impl anchor_lang::Owners for Dummy {
    fn owners() -> &'static [Pubkey] {
        std::slice::from_ref(&crate::ID)
    }
}

#[test]
fn skip_exit_does_not_persist_in_memory_mutations() {
    let mut data: Vec<u8> = serialize_dummy(10);
    let mut lamports: u64 = 1;
    let owner: Pubkey = crate::ID;
    let key: Pubkey = Pubkey::new_unique();

    let acc_info: AccountInfo<'_> =
        AccountInfo::new(&key, false, true, &mut lamports, &mut data, &owner, false);

    let mut acc: Account<'_, Dummy> = Account::<Dummy>::try_from(&acc_info).unwrap();
    acc.val = 99;
    acc.skip_exit();
    acc.exit(&crate::ID).unwrap();

    assert_eq!(
        acc_info.try_borrow_data().unwrap().as_ref(),
        serialize_dummy(10)
    );
}

#[test]
fn exit_persists_in_memory_mutations_by_default() {
    let mut data: Vec<u8> = serialize_dummy(10);
    let mut lamports: u64 = 1;
    let owner: Pubkey = crate::ID;
    let key: Pubkey = Pubkey::new_unique();

    let acc_info: AccountInfo<'_> =
        AccountInfo::new(&key, false, true, &mut lamports, &mut data, &owner, false);

    let mut acc: Account<'_, Dummy> = Account::<Dummy>::try_from(&acc_info).unwrap();
    acc.val = 99;
    acc.exit(&crate::ID).unwrap();

    assert_eq!(
        acc_info.try_borrow_data().unwrap().as_ref(),
        serialize_dummy(99)
    );
}

#[test]
fn skip_exit_preserves_cpi_side_effects() {
    let mut data: Vec<u8> = serialize_dummy(10);
    let mut lamports: u64 = 1;
    let owner: Pubkey = crate::ID;
    let key: Pubkey = Pubkey::new_unique();

    let acc_info: AccountInfo<'_> =
        AccountInfo::new(&key, false, true, &mut lamports, &mut data, &owner, false);

    let acc: Account<'_, Dummy> = Account::<Dummy>::try_from(&acc_info).unwrap();
    assert_eq!(acc.val, 10);

    // Simulate a CPI that wrote new account data.
    let new_bytes: Vec<u8> = serialize_dummy(42);
    {
        let mut d = acc_info.try_borrow_mut_data().unwrap();
        d.copy_from_slice(&new_bytes);
    }

    acc.skip_exit();
    acc.exit(&crate::ID).unwrap();

    assert_eq!(
        acc_info.try_borrow_data().unwrap().as_ref(),
        serialize_dummy(42)
    );
}

#[test]
fn interface_account_skip_exit_does_not_persist_mutations() {
    use anchor_lang::accounts::interface_account::InterfaceAccount;

    let mut data: Vec<u8> = serialize_dummy(5);
    let mut lamports: u64 = 1;
    let owner: Pubkey = crate::ID;
    let key: Pubkey = Pubkey::new_unique();

    let acc_info: AccountInfo<'_> =
        AccountInfo::new(&key, false, true, &mut lamports, &mut data, &owner, false);

    let mut i_face: InterfaceAccount<'_, Dummy> =
        InterfaceAccount::<Dummy>::try_from(&acc_info).unwrap();
    i_face.val = 6;
    i_face.skip_exit();
    i_face.exit(&crate::ID).unwrap();

    assert_eq!(
        acc_info.try_borrow_data().unwrap().as_ref(),
        serialize_dummy(5)
    );
}
