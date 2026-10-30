use anchor_lang::Discriminator;

anchor_lang::declare_id!("11111111111111111111111111111111");

mod left {
    use anchor_lang::prelude::*;

    #[account]
    pub struct Config {
        pub value: u64,
    }
}

mod right {
    use anchor_lang::prelude::*;

    #[account]
    pub struct Config {
        pub value: u64,
    }
}

#[test]
fn duplicate_module_accounts_keep_legacy_discriminators_and_are_rejected_by_idl_build() {
    assert_eq!(left::Config::DISCRIMINATOR, right::Config::DISCRIMINATOR);

    #[cfg(feature = "idl-build")]
    {
        let left = <left::Config as anchor_lang::IdlAccountType>::__idl_account_entry().unwrap();
        let right =
            <right::Config as anchor_lang::IdlAccountType>::__idl_account_entry().unwrap();
        let result = std::panic::catch_unwind(|| {
            anchor_lang::idl_build::validate_account_discriminator_entries(&[left, right]);
        });
        assert!(result.is_err(), "duplicate account discriminators must be rejected");
    }
}
