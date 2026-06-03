#![allow(warnings)]

use anchor_lang::prelude::*;

declare_id!("interfaceAccount111111111111111111111111111");

#[program]
pub mod interface_account {
    use super::*;

    pub fn test(ctx: Context<Test>) -> Result<()> {
        Ok(())
    }
}

#[derive(Accounts)]
pub struct Test<'info> {
    pub expected_account: InterfaceAccount<'info, interface::ExpectedAccount>,
}

mod interface {
    #[derive(Clone)]
    pub struct ExpectedAccount(new::ExpectedAccount);

    impl anchor_lang::AccountDeserialize for ExpectedAccount {
        fn try_deserialize(buf: &mut &[u8]) -> anchor_lang::Result<Self> {
            new::ExpectedAccount::try_deserialize(buf).map(Self)
        }

        fn try_deserialize_unchecked(buf: &mut &[u8]) -> anchor_lang::Result<Self> {
            new::ExpectedAccount::try_deserialize_unchecked(buf).map(Self)
        }
    }

    impl anchor_lang::AccountSerialize for ExpectedAccount {}

    impl anchor_lang::Owners for ExpectedAccount {
        fn owners() -> &'static [anchor_lang::prelude::Pubkey] {
            &[old::ID_CONST, new::ID_CONST]
        }
    }

    #[cfg(feature = "idl-build")]
    mod idl_impls {
        use super::ExpectedAccount;

        impl anchor_lang::IdlBuild for ExpectedAccount {}
        impl anchor_lang::Discriminator for ExpectedAccount {
            const DISCRIMINATOR: &'static [u8] = &[];
        }
    }
}
