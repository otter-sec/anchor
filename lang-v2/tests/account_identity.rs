use {
    anchor_lang::{
        accounts::Account, testing::AccountBuffer, AnchorAccount, Discriminator, Owner,
    },
};

anchor_lang::declare_id!("11111111111111111111111111111111");

mod left {
    use anchor_lang::prelude::*;

    #[account(discriminator = [1, 2, 3, 4, 5, 6, 7, 8])]
    pub struct Config {
        pub authority: [u8; 32],
    }
}

mod right {
    use anchor_lang::prelude::*;

    #[account(discriminator = [9, 10, 11, 12, 13, 14, 15, 16])]
    pub struct Config {
        pub balance: [u8; 32],
    }
}

#[test]
fn explicit_discriminators_prevent_same_leaf_cross_loading() {
    assert_ne!(left::Config::DISCRIMINATOR, right::Config::DISCRIMINATOR);
    assert_eq!(left::Config::OWNER, right::Config::OWNER);

    let buf = AccountBuffer::<128>::new();
    buf.init(
        [2; 32],
        ID.to_bytes(),
        8 + core::mem::size_of::<right::Config>(),
        false,
        false,
        false,
    );
    let mut data = [0u8; 40];
    data[..8].copy_from_slice(right::Config::DISCRIMINATOR);
    data[8..].fill(0xaa);
    buf.write_data(&data);

    let view = unsafe { buf.view() };
    assert!(Account::<left::Config>::load(view).is_err());

    #[cfg(feature = "idl-build")]
    {
        let entry = <left::Config as anchor_lang::IdlAccountType>::__idl_account_entry()
            .unwrap();
        assert!(entry.contains("\"discriminator\":[1,2,3,4,5,6,7,8]"));
    }
}
