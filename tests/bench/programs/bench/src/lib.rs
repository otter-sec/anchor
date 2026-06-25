//! This program is used to measure the performance of Anchor programs.
//!
//! If you are making a change to this program, run `anchor run sync`.

use anchor_lang::prelude::*;
use anchor_spl::token_interface::{Mint, TokenAccount, TokenInterface};

declare_id!("Bench11111111111111111111111111111111111111");

#[program]
pub mod bench {
    use super::*;

    pub fn account_info1(_ctx: Context<AccountInfo1>) -> Result<()> {
        Ok(())
    }

    pub fn account_info2(_ctx: Context<AccountInfo2>) -> Result<()> {
        Ok(())
    }

    pub fn account_info4(_ctx: Context<AccountInfo4>) -> Result<()> {
        Ok(())
    }

    pub fn account_info8(_ctx: Context<AccountInfo8>) -> Result<()> {
        Ok(())
    }

    pub fn account_empty_init1(_ctx: Context<AccountEmptyInit1>) -> Result<()> {
        Ok(())
    }

    pub fn account_empty_init2(_ctx: Context<AccountEmptyInit2>) -> Result<()> {
        Ok(())
    }

    pub fn account_empty_init4(_ctx: Context<AccountEmptyInit4>) -> Result<()> {
        Ok(())
    }

    pub fn account_empty_init8(_ctx: Context<AccountEmptyInit8>) -> Result<()> {
        Ok(())
    }

    pub fn account_empty1(_ctx: Context<AccountEmpty1>) -> Result<()> {
        Ok(())
    }

    pub fn account_empty2(_ctx: Context<AccountEmpty2>) -> Result<()> {
        Ok(())
    }

    pub fn account_empty4(_ctx: Context<AccountEmpty4>) -> Result<()> {
        Ok(())
    }

    pub fn account_empty8(_ctx: Context<AccountEmpty8>) -> Result<()> {
        Ok(())
    }

    pub fn account_sized_init1(_ctx: Context<AccountSizedInit1>) -> Result<()> {
        Ok(())
    }

    pub fn account_sized_init2(_ctx: Context<AccountSizedInit2>) -> Result<()> {
        Ok(())
    }

    pub fn account_sized_init4(_ctx: Context<AccountSizedInit4>) -> Result<()> {
        Ok(())
    }

    pub fn account_sized_init8(_ctx: Context<AccountSizedInit8>) -> Result<()> {
        Ok(())
    }

    pub fn account_sized1(_ctx: Context<AccountSized1>) -> Result<()> {
        Ok(())
    }

    pub fn account_sized2(_ctx: Context<AccountSized2>) -> Result<()> {
        Ok(())
    }

    pub fn account_sized4(_ctx: Context<AccountSized4>) -> Result<()> {
        Ok(())
    }

    pub fn account_sized8(_ctx: Context<AccountSized8>) -> Result<()> {
        Ok(())
    }

    pub fn account_unsized_init1(_ctx: Context<AccountUnsizedInit1>) -> Result<()> {
        Ok(())
    }

    pub fn account_unsized_init2(_ctx: Context<AccountUnsizedInit2>) -> Result<()> {
        Ok(())
    }

    pub fn account_unsized_init4(_ctx: Context<AccountUnsizedInit4>) -> Result<()> {
        Ok(())
    }

    pub fn account_unsized_init8(_ctx: Context<AccountUnsizedInit8>) -> Result<()> {
        Ok(())
    }

    pub fn account_unsized1(_ctx: Context<AccountUnsized1>) -> Result<()> {
        Ok(())
    }

    pub fn account_unsized2(_ctx: Context<AccountUnsized2>) -> Result<()> {
        Ok(())
    }

    pub fn account_unsized4(_ctx: Context<AccountUnsized4>) -> Result<()> {
        Ok(())
    }

    pub fn account_unsized8(_ctx: Context<AccountUnsized8>) -> Result<()> {
        Ok(())
    }

    pub fn boxed_account_empty_init1(_ctx: Context<BoxedAccountEmptyInit1>) -> Result<()> {
        Ok(())
    }

    pub fn boxed_account_empty_init2(_ctx: Context<BoxedAccountEmptyInit2>) -> Result<()> {
        Ok(())
    }

    pub fn boxed_account_empty_init4(_ctx: Context<BoxedAccountEmptyInit4>) -> Result<()> {
        Ok(())
    }

    pub fn boxed_account_empty_init8(_ctx: Context<BoxedAccountEmptyInit8>) -> Result<()> {
        Ok(())
    }

    pub fn boxed_account_empty1(_ctx: Context<BoxedAccountEmpty1>) -> Result<()> {
        Ok(())
    }

    pub fn boxed_account_empty2(_ctx: Context<BoxedAccountEmpty2>) -> Result<()> {
        Ok(())
    }

    pub fn boxed_account_empty4(_ctx: Context<BoxedAccountEmpty4>) -> Result<()> {
        Ok(())
    }

    pub fn boxed_account_empty8(_ctx: Context<BoxedAccountEmpty8>) -> Result<()> {
        Ok(())
    }

    pub fn boxed_account_sized_init1(_ctx: Context<BoxedAccountSizedInit1>) -> Result<()> {
        Ok(())
    }

    pub fn boxed_account_sized_init2(_ctx: Context<BoxedAccountSizedInit2>) -> Result<()> {
        Ok(())
    }

    pub fn boxed_account_sized_init4(_ctx: Context<BoxedAccountSizedInit4>) -> Result<()> {
        Ok(())
    }

    pub fn boxed_account_sized_init8(_ctx: Context<BoxedAccountSizedInit8>) -> Result<()> {
        Ok(())
    }

    pub fn boxed_account_sized1(_ctx: Context<BoxedAccountSized1>) -> Result<()> {
        Ok(())
    }

    pub fn boxed_account_sized2(_ctx: Context<BoxedAccountSized2>) -> Result<()> {
        Ok(())
    }

    pub fn boxed_account_sized4(_ctx: Context<BoxedAccountSized4>) -> Result<()> {
        Ok(())
    }

    pub fn boxed_account_sized8(_ctx: Context<BoxedAccountSized8>) -> Result<()> {
        Ok(())
    }

    pub fn boxed_account_unsized_init1(_ctx: Context<BoxedAccountUnsizedInit1>) -> Result<()> {
        Ok(())
    }

    pub fn boxed_account_unsized_init2(_ctx: Context<BoxedAccountUnsizedInit2>) -> Result<()> {
        Ok(())
    }

    pub fn boxed_account_unsized_init4(_ctx: Context<BoxedAccountUnsizedInit4>) -> Result<()> {
        Ok(())
    }

    pub fn boxed_account_unsized_init8(_ctx: Context<BoxedAccountUnsizedInit8>) -> Result<()> {
        Ok(())
    }

    pub fn boxed_account_unsized1(_ctx: Context<BoxedAccountUnsized1>) -> Result<()> {
        Ok(())
    }

    pub fn boxed_account_unsized2(_ctx: Context<BoxedAccountUnsized2>) -> Result<()> {
        Ok(())
    }

    pub fn boxed_account_unsized4(_ctx: Context<BoxedAccountUnsized4>) -> Result<()> {
        Ok(())
    }

    pub fn boxed_account_unsized8(_ctx: Context<BoxedAccountUnsized8>) -> Result<()> {
        Ok(())
    }

    pub fn boxed_interface_account_mint1(_ctx: Context<BoxedInterfaceAccountMint1>) -> Result<()> {
        Ok(())
    }

    pub fn boxed_interface_account_mint2(_ctx: Context<BoxedInterfaceAccountMint2>) -> Result<()> {
        Ok(())
    }

    pub fn boxed_interface_account_mint4(_ctx: Context<BoxedInterfaceAccountMint4>) -> Result<()> {
        Ok(())
    }

    pub fn boxed_interface_account_mint8(_ctx: Context<BoxedInterfaceAccountMint8>) -> Result<()> {
        Ok(())
    }

    pub fn boxed_interface_account_token1(
        _ctx: Context<BoxedInterfaceAccountToken1>,
    ) -> Result<()> {
        Ok(())
    }

    pub fn boxed_interface_account_token2(
        _ctx: Context<BoxedInterfaceAccountToken2>,
    ) -> Result<()> {
        Ok(())
    }

    pub fn boxed_interface_account_token4(
        _ctx: Context<BoxedInterfaceAccountToken4>,
    ) -> Result<()> {
        Ok(())
    }

    pub fn boxed_interface_account_token8(
        _ctx: Context<BoxedInterfaceAccountToken8>,
    ) -> Result<()> {
        Ok(())
    }

    pub fn interface_account_mint1(_ctx: Context<InterfaceAccountMint1>) -> Result<()> {
        Ok(())
    }

    pub fn interface_account_mint2(_ctx: Context<InterfaceAccountMint2>) -> Result<()> {
        Ok(())
    }

    pub fn interface_account_mint4(_ctx: Context<InterfaceAccountMint4>) -> Result<()> {
        Ok(())
    }

    pub fn interface_account_mint8(_ctx: Context<InterfaceAccountMint8>) -> Result<()> {
        Ok(())
    }

    pub fn interface_account_token1(_ctx: Context<InterfaceAccountToken1>) -> Result<()> {
        Ok(())
    }

    pub fn interface_account_token2(_ctx: Context<InterfaceAccountToken2>) -> Result<()> {
        Ok(())
    }

    pub fn interface_account_token4(_ctx: Context<InterfaceAccountToken4>) -> Result<()> {
        Ok(())
    }

    pub fn interface1(_ctx: Context<Interface1>) -> Result<()> {
        Ok(())
    }

    pub fn interface2(_ctx: Context<Interface2>) -> Result<()> {
        Ok(())
    }

    pub fn interface4(_ctx: Context<Interface4>) -> Result<()> {
        Ok(())
    }

    pub fn interface8(_ctx: Context<Interface8>) -> Result<()> {
        Ok(())
    }

    pub fn program1(_ctx: Context<Program1>) -> Result<()> {
        Ok(())
    }

    pub fn program2(_ctx: Context<Program2>) -> Result<()> {
        Ok(())
    }

    pub fn program4(_ctx: Context<Program4>) -> Result<()> {
        Ok(())
    }

    pub fn program8(_ctx: Context<Program8>) -> Result<()> {
        Ok(())
    }

    pub fn signer1(_ctx: Context<Signer1>) -> Result<()> {
        Ok(())
    }

    pub fn signer2(_ctx: Context<Signer2>) -> Result<()> {
        Ok(())
    }

    pub fn signer4(_ctx: Context<Signer4>) -> Result<()> {
        Ok(())
    }

    pub fn signer8(_ctx: Context<Signer8>) -> Result<()> {
        Ok(())
    }

    pub fn system_account1(_ctx: Context<SystemAccount1>) -> Result<()> {
        Ok(())
    }

    pub fn system_account2(_ctx: Context<SystemAccount2>) -> Result<()> {
        Ok(())
    }

    pub fn system_account4(_ctx: Context<SystemAccount4>) -> Result<()> {
        Ok(())
    }

    pub fn system_account8(_ctx: Context<SystemAccount8>) -> Result<()> {
        Ok(())
    }

    pub fn unchecked_account1(_ctx: Context<UncheckedAccount1>) -> Result<()> {
        Ok(())
    }

    pub fn unchecked_account2(_ctx: Context<UncheckedAccount2>) -> Result<()> {
        Ok(())
    }

    pub fn unchecked_account4(_ctx: Context<UncheckedAccount4>) -> Result<()> {
        Ok(())
    }

    pub fn unchecked_account8(_ctx: Context<UncheckedAccount8>) -> Result<()> {
        Ok(())
    }
}

#[account]
pub struct Empty {}

#[account]
pub struct Sized {
    pub field: [u8; 8],
}

#[account]
pub struct Unsized {
    pub field: Vec<u8>,
}

#[derive(Accounts)]
pub struct AccountInfo1<'info> {
    pub account1: AccountInfo<'info>,
}

#[derive(Accounts)]
pub struct AccountInfo2<'info> {
    pub account1: AccountInfo<'info>,
    pub account2: AccountInfo<'info>,
}

#[derive(Accounts)]
pub struct AccountInfo4<'info> {
    pub account1: AccountInfo<'info>,
    pub account2: AccountInfo<'info>,
    pub account3: AccountInfo<'info>,
    pub account4: AccountInfo<'info>,
}

#[derive(Accounts)]
pub struct AccountInfo8<'info> {
    pub account1: AccountInfo<'info>,
    pub account2: AccountInfo<'info>,
    pub account3: AccountInfo<'info>,
    pub account4: AccountInfo<'info>,
    pub account5: AccountInfo<'info>,
    pub account6: AccountInfo<'info>,
    pub account7: AccountInfo<'info>,
    pub account8: AccountInfo<'info>,
}

#[derive(Accounts)]
pub struct AccountEmptyInit1<'info> {
    #[account(mut)]
    pub payer: Signer<'info>,
    pub system_program: Program<'info, System>,
    #[account(init, payer = payer, space = 8)]
    pub account1: Account<'info, Empty>,
}

#[derive(Accounts)]
pub struct AccountEmptyInit2<'info> {
    #[account(mut)]
    pub payer: Signer<'info>,
    pub system_program: Program<'info, System>,
    #[account(init, payer = payer, space = 8)]
    pub account1: Account<'info, Empty>,
    #[account(init, payer = payer, space = 8)]
    pub account2: Account<'info, Empty>,
}

#[derive(Accounts)]
pub struct AccountEmptyInit4<'info> {
    #[account(mut)]
    pub payer: Signer<'info>,
    pub system_program: Program<'info, System>,
    #[account(init, payer = payer, space = 8)]
    pub account1: Account<'info, Empty>,
    #[account(init, payer = payer, space = 8)]
    pub account2: Account<'info, Empty>,
    #[account(init, payer = payer, space = 8)]
    pub account3: Account<'info, Empty>,
    #[account(init, payer = payer, space = 8)]
    pub account4: Account<'info, Empty>,
}

#[derive(Accounts)]
pub struct AccountEmptyInit8<'info> {
    #[account(mut)]
    pub payer: Signer<'info>,
    pub system_program: Program<'info, System>,
    #[account(init, payer = payer, space = 8)]
    pub account1: Account<'info, Empty>,
    #[account(init, payer = payer, space = 8)]
    pub account2: Account<'info, Empty>,
    #[account(init, payer = payer, space = 8)]
    pub account3: Account<'info, Empty>,
    #[account(init, payer = payer, space = 8)]
    pub account4: Account<'info, Empty>,
    #[account(init, payer = payer, space = 8)]
    pub account5: Account<'info, Empty>,
    #[account(init, payer = payer, space = 8)]
    pub account6: Account<'info, Empty>,
    #[account(init, payer = payer, space = 8)]
    pub account7: Account<'info, Empty>,
    #[account(init, payer = payer, space = 8)]
    pub account8: Account<'info, Empty>,
}

#[derive(Accounts)]
pub struct AccountEmpty1<'info> {
    pub account1: Account<'info, Empty>,
}

#[derive(Accounts)]
pub struct AccountEmpty2<'info> {
    pub account1: Account<'info, Empty>,
    pub account2: Account<'info, Empty>,
}

#[derive(Accounts)]
pub struct AccountEmpty4<'info> {
    pub account1: Account<'info, Empty>,
    pub account2: Account<'info, Empty>,
    pub account3: Account<'info, Empty>,
    pub account4: Account<'info, Empty>,
}

#[derive(Accounts)]
pub struct AccountEmpty8<'info> {
    pub account1: Account<'info, Empty>,
    pub account2: Account<'info, Empty>,
    pub account3: Account<'info, Empty>,
    pub account4: Account<'info, Empty>,
    pub account5: Account<'info, Empty>,
    pub account6: Account<'info, Empty>,
    pub account7: Account<'info, Empty>,
    pub account8: Account<'info, Empty>,
}

#[derive(Accounts)]
pub struct AccountSizedInit1<'info> {
    #[account(mut)]
    pub payer: Signer<'info>,
    pub system_program: Program<'info, System>,
    #[account(init, payer = payer, space = 8 + std::mem::size_of::<Sized>())]
    pub account1: Account<'info, Sized>,
}

#[derive(Accounts)]
pub struct AccountSizedInit2<'info> {
    #[account(mut)]
    pub payer: Signer<'info>,
    pub system_program: Program<'info, System>,
    #[account(init, payer = payer, space = 8 + std::mem::size_of::<Sized>())]
    pub account1: Account<'info, Sized>,
    #[account(init, payer = payer, space = 8 + std::mem::size_of::<Sized>())]
    pub account2: Account<'info, Sized>,
}

#[derive(Accounts)]
pub struct AccountSizedInit4<'info> {
    #[account(mut)]
    pub payer: Signer<'info>,
    pub system_program: Program<'info, System>,
    #[account(init, payer = payer, space = 8 + std::mem::size_of::<Sized>())]
    pub account1: Account<'info, Sized>,
    #[account(init, payer = payer, space = 8 + std::mem::size_of::<Sized>())]
    pub account2: Account<'info, Sized>,
    #[account(init, payer = payer, space = 8 + std::mem::size_of::<Sized>())]
    pub account3: Account<'info, Sized>,
    #[account(init, payer = payer, space = 8 + std::mem::size_of::<Sized>())]
    pub account4: Account<'info, Sized>,
}

#[derive(Accounts)]
pub struct AccountSizedInit8<'info> {
    #[account(mut)]
    pub payer: Signer<'info>,
    pub system_program: Program<'info, System>,
    #[account(init, payer = payer, space = 8 + std::mem::size_of::<Sized>())]
    pub account1: Account<'info, Sized>,
    #[account(init, payer = payer, space = 8 + std::mem::size_of::<Sized>())]
    pub account2: Account<'info, Sized>,
    #[account(init, payer = payer, space = 8 + std::mem::size_of::<Sized>())]
    pub account3: Account<'info, Sized>,
    #[account(init, payer = payer, space = 8 + std::mem::size_of::<Sized>())]
    pub account4: Account<'info, Sized>,
    #[account(init, payer = payer, space = 8 + std::mem::size_of::<Sized>())]
    pub account5: Account<'info, Sized>,
    #[account(init, payer = payer, space = 8 + std::mem::size_of::<Sized>())]
    pub account6: Account<'info, Sized>,
    #[account(init, payer = payer, space = 8 + std::mem::size_of::<Sized>())]
    pub account7: Account<'info, Sized>,
    #[account(init, payer = payer, space = 8 + std::mem::size_of::<Sized>())]
    pub account8: Account<'info, Sized>,
}

#[derive(Accounts)]
pub struct AccountSized1<'info> {
    pub account1: Account<'info, Sized>,
}

#[derive(Accounts)]
pub struct AccountSized2<'info> {
    pub account1: Account<'info, Sized>,
    pub account2: Account<'info, Sized>,
}

#[derive(Accounts)]
pub struct AccountSized4<'info> {
    pub account1: Account<'info, Sized>,
    pub account2: Account<'info, Sized>,
    pub account3: Account<'info, Sized>,
    pub account4: Account<'info, Sized>,
}

#[derive(Accounts)]
pub struct AccountSized8<'info> {
    pub account1: Account<'info, Sized>,
    pub account2: Account<'info, Sized>,
    pub account3: Account<'info, Sized>,
    pub account4: Account<'info, Sized>,
    pub account5: Account<'info, Sized>,
    pub account6: Account<'info, Sized>,
    pub account7: Account<'info, Sized>,
    pub account8: Account<'info, Sized>,
}

#[derive(Accounts)]
pub struct AccountUnsizedInit1<'info> {
    #[account(mut)]
    pub payer: Signer<'info>,
    pub system_program: Program<'info, System>,
    #[account(init, payer = payer, space = 8 + std::mem::size_of::<Unsized>())]
    pub account1: Account<'info, Unsized>,
}

#[derive(Accounts)]
pub struct AccountUnsizedInit2<'info> {
    #[account(mut)]
    pub payer: Signer<'info>,
    pub system_program: Program<'info, System>,
    #[account(init, payer = payer, space = 8 + std::mem::size_of::<Unsized>())]
    pub account1: Account<'info, Unsized>,
    #[account(init, payer = payer, space = 8 + std::mem::size_of::<Unsized>())]
    pub account2: Account<'info, Unsized>,
}

#[derive(Accounts)]
pub struct AccountUnsizedInit4<'info> {
    #[account(mut)]
    pub payer: Signer<'info>,
    pub system_program: Program<'info, System>,
    #[account(init, payer = payer, space = 8 + std::mem::size_of::<Unsized>())]
    pub account1: Account<'info, Unsized>,
    #[account(init, payer = payer, space = 8 + std::mem::size_of::<Unsized>())]
    pub account2: Account<'info, Unsized>,
    #[account(init, payer = payer, space = 8 + std::mem::size_of::<Unsized>())]
    pub account3: Account<'info, Unsized>,
    #[account(init, payer = payer, space = 8 + std::mem::size_of::<Unsized>())]
    pub account4: Account<'info, Unsized>,
}

#[derive(Accounts)]
pub struct AccountUnsizedInit8<'info> {
    #[account(mut)]
    pub payer: Signer<'info>,
    pub system_program: Program<'info, System>,
    #[account(init, payer = payer, space = 8 + std::mem::size_of::<Unsized>())]
    pub account1: Account<'info, Unsized>,
    #[account(init, payer = payer, space = 8 + std::mem::size_of::<Unsized>())]
    pub account2: Account<'info, Unsized>,
    #[account(init, payer = payer, space = 8 + std::mem::size_of::<Unsized>())]
    pub account3: Account<'info, Unsized>,
    #[account(init, payer = payer, space = 8 + std::mem::size_of::<Unsized>())]
    pub account4: Account<'info, Unsized>,
    #[account(init, payer = payer, space = 8 + std::mem::size_of::<Unsized>())]
    pub account5: Account<'info, Unsized>,
    #[account(init, payer = payer, space = 8 + std::mem::size_of::<Unsized>())]
    pub account6: Account<'info, Unsized>,
    #[account(init, payer = payer, space = 8 + std::mem::size_of::<Unsized>())]
    pub account7: Account<'info, Unsized>,
    #[account(init, payer = payer, space = 8 + std::mem::size_of::<Unsized>())]
    pub account8: Account<'info, Unsized>,
}

#[derive(Accounts)]
pub struct AccountUnsized1<'info> {
    pub account1: Account<'info, Unsized>,
}

#[derive(Accounts)]
pub struct AccountUnsized2<'info> {
    pub account1: Account<'info, Unsized>,
    pub account2: Account<'info, Unsized>,
}

#[derive(Accounts)]
pub struct AccountUnsized4<'info> {
    pub account1: Account<'info, Unsized>,
    pub account2: Account<'info, Unsized>,
    pub account3: Account<'info, Unsized>,
    pub account4: Account<'info, Unsized>,
}

#[derive(Accounts)]
pub struct AccountUnsized8<'info> {
    pub account1: Account<'info, Unsized>,
    pub account2: Account<'info, Unsized>,
    pub account3: Account<'info, Unsized>,
    pub account4: Account<'info, Unsized>,
    pub account5: Account<'info, Unsized>,
    pub account6: Account<'info, Unsized>,
    pub account7: Account<'info, Unsized>,
    pub account8: Account<'info, Unsized>,
}

#[derive(Accounts)]
pub struct BoxedAccountEmptyInit1<'info> {
    #[account(mut)]
    pub payer: Signer<'info>,
    pub system_program: Program<'info, System>,
    #[account(init, payer = payer, space = 8)]
    pub account1: Box<Account<'info, Empty>>,
}

#[derive(Accounts)]
pub struct BoxedAccountEmptyInit2<'info> {
    #[account(mut)]
    pub payer: Signer<'info>,
    pub system_program: Program<'info, System>,
    #[account(init, payer = payer, space = 8)]
    pub account1: Box<Account<'info, Empty>>,
    #[account(init, payer = payer, space = 8)]
    pub account2: Box<Account<'info, Empty>>,
}

#[derive(Accounts)]
pub struct BoxedAccountEmptyInit4<'info> {
    #[account(mut)]
    pub payer: Signer<'info>,
    pub system_program: Program<'info, System>,
    #[account(init, payer = payer, space = 8)]
    pub account1: Box<Account<'info, Empty>>,
    #[account(init, payer = payer, space = 8)]
    pub account2: Box<Account<'info, Empty>>,
    #[account(init, payer = payer, space = 8)]
    pub account3: Box<Account<'info, Empty>>,
    #[account(init, payer = payer, space = 8)]
    pub account4: Box<Account<'info, Empty>>,
}

#[derive(Accounts)]
pub struct BoxedAccountEmptyInit8<'info> {
    #[account(mut)]
    pub payer: Signer<'info>,
    pub system_program: Program<'info, System>,
    #[account(init, payer = payer, space = 8)]
    pub account1: Box<Account<'info, Empty>>,
    #[account(init, payer = payer, space = 8)]
    pub account2: Box<Account<'info, Empty>>,
    #[account(init, payer = payer, space = 8)]
    pub account3: Box<Account<'info, Empty>>,
    #[account(init, payer = payer, space = 8)]
    pub account4: Box<Account<'info, Empty>>,
    #[account(init, payer = payer, space = 8)]
    pub account5: Box<Account<'info, Empty>>,
    #[account(init, payer = payer, space = 8)]
    pub account6: Box<Account<'info, Empty>>,
    #[account(init, payer = payer, space = 8)]
    pub account7: Box<Account<'info, Empty>>,
    #[account(init, payer = payer, space = 8)]
    pub account8: Box<Account<'info, Empty>>,
}

#[derive(Accounts)]
pub struct BoxedAccountEmpty1<'info> {
    pub account1: Box<Account<'info, Empty>>,
}

#[derive(Accounts)]
pub struct BoxedAccountEmpty2<'info> {
    pub account1: Box<Account<'info, Empty>>,
    pub account2: Box<Account<'info, Empty>>,
}

#[derive(Accounts)]
pub struct BoxedAccountEmpty4<'info> {
    pub account1: Box<Account<'info, Empty>>,
    pub account2: Box<Account<'info, Empty>>,
    pub account3: Box<Account<'info, Empty>>,
    pub account4: Box<Account<'info, Empty>>,
}

#[derive(Accounts)]
pub struct BoxedAccountEmpty8<'info> {
    pub account1: Box<Account<'info, Empty>>,
    pub account2: Box<Account<'info, Empty>>,
    pub account3: Box<Account<'info, Empty>>,
    pub account4: Box<Account<'info, Empty>>,
    pub account5: Box<Account<'info, Empty>>,
    pub account6: Box<Account<'info, Empty>>,
    pub account7: Box<Account<'info, Empty>>,
    pub account8: Box<Account<'info, Empty>>,
}

#[derive(Accounts)]
pub struct BoxedAccountSizedInit1<'info> {
    #[account(mut)]
    pub payer: Signer<'info>,
    pub system_program: Program<'info, System>,
    #[account(init, payer = payer, space = 8 + std::mem::size_of::<Sized>())]
    pub account1: Box<Account<'info, Sized>>,
}

#[derive(Accounts)]
pub struct BoxedAccountSizedInit2<'info> {
    #[account(mut)]
    pub payer: Signer<'info>,
    pub system_program: Program<'info, System>,
    #[account(init, payer = payer, space = 8 + std::mem::size_of::<Sized>())]
    pub account1: Box<Account<'info, Sized>>,
    #[account(init, payer = payer, space = 8 + std::mem::size_of::<Sized>())]
    pub account2: Box<Account<'info, Sized>>,
}

#[derive(Accounts)]
pub struct BoxedAccountSizedInit4<'info> {
    #[account(mut)]
    pub payer: Signer<'info>,
    pub system_program: Program<'info, System>,
    #[account(init, payer = payer, space = 8 + std::mem::size_of::<Sized>())]
    pub account1: Box<Account<'info, Sized>>,
    #[account(init, payer = payer, space = 8 + std::mem::size_of::<Sized>())]
    pub account2: Box<Account<'info, Sized>>,
    #[account(init, payer = payer, space = 8 + std::mem::size_of::<Sized>())]
    pub account3: Box<Account<'info, Sized>>,
    #[account(init, payer = payer, space = 8 + std::mem::size_of::<Sized>())]
    pub account4: Box<Account<'info, Sized>>,
}

#[derive(Accounts)]
pub struct BoxedAccountSizedInit8<'info> {
    #[account(mut)]
    pub payer: Signer<'info>,
    pub system_program: Program<'info, System>,
    #[account(init, payer = payer, space = 8 + std::mem::size_of::<Sized>())]
    pub account1: Box<Account<'info, Sized>>,
    #[account(init, payer = payer, space = 8 + std::mem::size_of::<Sized>())]
    pub account2: Box<Account<'info, Sized>>,
    #[account(init, payer = payer, space = 8 + std::mem::size_of::<Sized>())]
    pub account3: Box<Account<'info, Sized>>,
    #[account(init, payer = payer, space = 8 + std::mem::size_of::<Sized>())]
    pub account4: Box<Account<'info, Sized>>,
    #[account(init, payer = payer, space = 8 + std::mem::size_of::<Sized>())]
    pub account5: Box<Account<'info, Sized>>,
    #[account(init, payer = payer, space = 8 + std::mem::size_of::<Sized>())]
    pub account6: Box<Account<'info, Sized>>,
    #[account(init, payer = payer, space = 8 + std::mem::size_of::<Sized>())]
    pub account7: Box<Account<'info, Sized>>,
    #[account(init, payer = payer, space = 8 + std::mem::size_of::<Sized>())]
    pub account8: Box<Account<'info, Sized>>,
}

#[derive(Accounts)]
pub struct BoxedAccountSized1<'info> {
    pub account1: Box<Account<'info, Sized>>,
}

#[derive(Accounts)]
pub struct BoxedAccountSized2<'info> {
    pub account1: Box<Account<'info, Sized>>,
    pub account2: Box<Account<'info, Sized>>,
}

#[derive(Accounts)]
pub struct BoxedAccountSized4<'info> {
    pub account1: Box<Account<'info, Sized>>,
    pub account2: Box<Account<'info, Sized>>,
    pub account3: Box<Account<'info, Sized>>,
    pub account4: Box<Account<'info, Sized>>,
}

#[derive(Accounts)]
pub struct BoxedAccountSized8<'info> {
    pub account1: Box<Account<'info, Sized>>,
    pub account2: Box<Account<'info, Sized>>,
    pub account3: Box<Account<'info, Sized>>,
    pub account4: Box<Account<'info, Sized>>,
    pub account5: Box<Account<'info, Sized>>,
    pub account6: Box<Account<'info, Sized>>,
    pub account7: Box<Account<'info, Sized>>,
    pub account8: Box<Account<'info, Sized>>,
}

#[derive(Accounts)]
pub struct BoxedAccountUnsizedInit1<'info> {
    #[account(mut)]
    pub payer: Signer<'info>,
    pub system_program: Program<'info, System>,
    #[account(init, payer = payer, space = 8 + std::mem::size_of::<Unsized>())]
    pub account1: Box<Account<'info, Unsized>>,
}

#[derive(Accounts)]
pub struct BoxedAccountUnsizedInit2<'info> {
    #[account(mut)]
    pub payer: Signer<'info>,
    pub system_program: Program<'info, System>,
    #[account(init, payer = payer, space = 8 + std::mem::size_of::<Unsized>())]
    pub account1: Box<Account<'info, Unsized>>,
    #[account(init, payer = payer, space = 8 + std::mem::size_of::<Unsized>())]
    pub account2: Box<Account<'info, Unsized>>,
}

#[derive(Accounts)]
pub struct BoxedAccountUnsizedInit4<'info> {
    #[account(mut)]
    pub payer: Signer<'info>,
    pub system_program: Program<'info, System>,
    #[account(init, payer = payer, space = 8 + std::mem::size_of::<Unsized>())]
    pub account1: Box<Account<'info, Unsized>>,
    #[account(init, payer = payer, space = 8 + std::mem::size_of::<Unsized>())]
    pub account2: Box<Account<'info, Unsized>>,
    #[account(init, payer = payer, space = 8 + std::mem::size_of::<Unsized>())]
    pub account3: Box<Account<'info, Unsized>>,
    #[account(init, payer = payer, space = 8 + std::mem::size_of::<Unsized>())]
    pub account4: Box<Account<'info, Unsized>>,
}

#[derive(Accounts)]
pub struct BoxedAccountUnsizedInit8<'info> {
    #[account(mut)]
    pub payer: Signer<'info>,
    pub system_program: Program<'info, System>,
    #[account(init, payer = payer, space = 8 + std::mem::size_of::<Unsized>())]
    pub account1: Box<Account<'info, Unsized>>,
    #[account(init, payer = payer, space = 8 + std::mem::size_of::<Unsized>())]
    pub account2: Box<Account<'info, Unsized>>,
    #[account(init, payer = payer, space = 8 + std::mem::size_of::<Unsized>())]
    pub account3: Box<Account<'info, Unsized>>,
    #[account(init, payer = payer, space = 8 + std::mem::size_of::<Unsized>())]
    pub account4: Box<Account<'info, Unsized>>,
    #[account(init, payer = payer, space = 8 + std::mem::size_of::<Unsized>())]
    pub account5: Box<Account<'info, Unsized>>,
    #[account(init, payer = payer, space = 8 + std::mem::size_of::<Unsized>())]
    pub account6: Box<Account<'info, Unsized>>,
    #[account(init, payer = payer, space = 8 + std::mem::size_of::<Unsized>())]
    pub account7: Box<Account<'info, Unsized>>,
    #[account(init, payer = payer, space = 8 + std::mem::size_of::<Unsized>())]
    pub account8: Box<Account<'info, Unsized>>,
}

#[derive(Accounts)]
pub struct BoxedAccountUnsized1<'info> {
    pub account1: Box<Account<'info, Unsized>>,
}

#[derive(Accounts)]
pub struct BoxedAccountUnsized2<'info> {
    pub account1: Box<Account<'info, Unsized>>,
    pub account2: Box<Account<'info, Unsized>>,
}

#[derive(Accounts)]
pub struct BoxedAccountUnsized4<'info> {
    pub account1: Box<Account<'info, Unsized>>,
    pub account2: Box<Account<'info, Unsized>>,
    pub account3: Box<Account<'info, Unsized>>,
    pub account4: Box<Account<'info, Unsized>>,
}

#[derive(Accounts)]
pub struct BoxedAccountUnsized8<'info> {
    pub account1: Box<Account<'info, Unsized>>,
    pub account2: Box<Account<'info, Unsized>>,
    pub account3: Box<Account<'info, Unsized>>,
    pub account4: Box<Account<'info, Unsized>>,
    pub account5: Box<Account<'info, Unsized>>,
    pub account6: Box<Account<'info, Unsized>>,
    pub account7: Box<Account<'info, Unsized>>,
    pub account8: Box<Account<'info, Unsized>>,
}

#[derive(Accounts)]
pub struct BoxedInterfaceAccountMint1<'info> {
    pub account1: Box<InterfaceAccount<'info, Mint>>,
}

#[derive(Accounts)]
pub struct BoxedInterfaceAccountMint2<'info> {
    pub account1: Box<InterfaceAccount<'info, Mint>>,
    pub account2: Box<InterfaceAccount<'info, Mint>>,
}

#[derive(Accounts)]
pub struct BoxedInterfaceAccountMint4<'info> {
    pub account1: Box<InterfaceAccount<'info, Mint>>,
    pub account2: Box<InterfaceAccount<'info, Mint>>,
    pub account3: Box<InterfaceAccount<'info, Mint>>,
    pub account4: Box<InterfaceAccount<'info, Mint>>,
}

#[derive(Accounts)]
pub struct BoxedInterfaceAccountMint8<'info> {
    pub account1: Box<InterfaceAccount<'info, Mint>>,
    pub account2: Box<InterfaceAccount<'info, Mint>>,
    pub account3: Box<InterfaceAccount<'info, Mint>>,
    pub account4: Box<InterfaceAccount<'info, Mint>>,
    pub account5: Box<InterfaceAccount<'info, Mint>>,
    pub account6: Box<InterfaceAccount<'info, Mint>>,
    pub account7: Box<InterfaceAccount<'info, Mint>>,
    pub account8: Box<InterfaceAccount<'info, Mint>>,
}

#[derive(Accounts)]
pub struct BoxedInterfaceAccountToken1<'info> {
    pub account1: Box<InterfaceAccount<'info, TokenAccount>>,
}

#[derive(Accounts)]
pub struct BoxedInterfaceAccountToken2<'info> {
    pub account1: Box<InterfaceAccount<'info, TokenAccount>>,
    pub account2: Box<InterfaceAccount<'info, TokenAccount>>,
}

#[derive(Accounts)]
pub struct BoxedInterfaceAccountToken4<'info> {
    pub account1: Box<InterfaceAccount<'info, TokenAccount>>,
    pub account2: Box<InterfaceAccount<'info, TokenAccount>>,
    pub account3: Box<InterfaceAccount<'info, TokenAccount>>,
    pub account4: Box<InterfaceAccount<'info, TokenAccount>>,
}

#[derive(Accounts)]
pub struct BoxedInterfaceAccountToken8<'info> {
    pub account1: Box<InterfaceAccount<'info, TokenAccount>>,
    pub account2: Box<InterfaceAccount<'info, TokenAccount>>,
    pub account3: Box<InterfaceAccount<'info, TokenAccount>>,
    pub account4: Box<InterfaceAccount<'info, TokenAccount>>,
    pub account5: Box<InterfaceAccount<'info, TokenAccount>>,
    pub account6: Box<InterfaceAccount<'info, TokenAccount>>,
    pub account7: Box<InterfaceAccount<'info, TokenAccount>>,
    pub account8: Box<InterfaceAccount<'info, TokenAccount>>,
}

#[derive(Accounts)]
pub struct InterfaceAccountMint1<'info> {
    pub account1: InterfaceAccount<'info, Mint>,
}

#[derive(Accounts)]
pub struct InterfaceAccountMint2<'info> {
    pub account1: InterfaceAccount<'info, Mint>,
    pub account2: InterfaceAccount<'info, Mint>,
}

#[derive(Accounts)]
pub struct InterfaceAccountMint4<'info> {
    pub account1: InterfaceAccount<'info, Mint>,
    pub account2: InterfaceAccount<'info, Mint>,
    pub account3: InterfaceAccount<'info, Mint>,
    pub account4: InterfaceAccount<'info, Mint>,
}

#[derive(Accounts)]
pub struct InterfaceAccountMint8<'info> {
    pub account1: InterfaceAccount<'info, Mint>,
    pub account2: InterfaceAccount<'info, Mint>,
    pub account3: InterfaceAccount<'info, Mint>,
    pub account4: InterfaceAccount<'info, Mint>,
    pub account5: InterfaceAccount<'info, Mint>,
    pub account6: InterfaceAccount<'info, Mint>,
    pub account7: InterfaceAccount<'info, Mint>,
    pub account8: InterfaceAccount<'info, Mint>,
}

#[derive(Accounts)]
pub struct InterfaceAccountToken1<'info> {
    pub account1: InterfaceAccount<'info, TokenAccount>,
}

#[derive(Accounts)]
pub struct InterfaceAccountToken2<'info> {
    pub account1: InterfaceAccount<'info, TokenAccount>,
    pub account2: InterfaceAccount<'info, TokenAccount>,
}

#[derive(Accounts)]
pub struct InterfaceAccountToken4<'info> {
    pub account1: InterfaceAccount<'info, TokenAccount>,
    pub account2: InterfaceAccount<'info, TokenAccount>,
    pub account3: InterfaceAccount<'info, TokenAccount>,
    pub account4: InterfaceAccount<'info, TokenAccount>,
}

#[derive(Accounts)]
pub struct Interface1<'info> {
    pub account1: Interface<'info, TokenInterface>,
}

#[derive(Accounts)]
pub struct Interface2<'info> {
    pub account1: Interface<'info, TokenInterface>,
    pub account2: Interface<'info, TokenInterface>,
}

#[derive(Accounts)]
pub struct Interface4<'info> {
    pub account1: Interface<'info, TokenInterface>,
    pub account2: Interface<'info, TokenInterface>,
    pub account3: Interface<'info, TokenInterface>,
    pub account4: Interface<'info, TokenInterface>,
}

#[derive(Accounts)]
pub struct Interface8<'info> {
    pub account1: Interface<'info, TokenInterface>,
    pub account2: Interface<'info, TokenInterface>,
    pub account3: Interface<'info, TokenInterface>,
    pub account4: Interface<'info, TokenInterface>,
    pub account5: Interface<'info, TokenInterface>,
    pub account6: Interface<'info, TokenInterface>,
    pub account7: Interface<'info, TokenInterface>,
    pub account8: Interface<'info, TokenInterface>,
}

#[derive(Accounts)]
pub struct Program1<'info> {
    pub account1: Program<'info, System>,
}

#[derive(Accounts)]
pub struct Program2<'info> {
    pub account1: Program<'info, System>,
    pub account2: Program<'info, System>,
}

#[derive(Accounts)]
pub struct Program4<'info> {
    pub account1: Program<'info, System>,
    pub account2: Program<'info, System>,
    pub account3: Program<'info, System>,
    pub account4: Program<'info, System>,
}

#[derive(Accounts)]
pub struct Program8<'info> {
    pub account1: Program<'info, System>,
    pub account2: Program<'info, System>,
    pub account3: Program<'info, System>,
    pub account4: Program<'info, System>,
    pub account5: Program<'info, System>,
    pub account6: Program<'info, System>,
    pub account7: Program<'info, System>,
    pub account8: Program<'info, System>,
}

#[derive(Accounts)]
pub struct Signer1<'info> {
    pub account1: Signer<'info>,
}

#[derive(Accounts)]
pub struct Signer2<'info> {
    pub account1: Signer<'info>,
    pub account2: Signer<'info>,
}

#[derive(Accounts)]
pub struct Signer4<'info> {
    pub account1: Signer<'info>,
    pub account2: Signer<'info>,
    pub account3: Signer<'info>,
    pub account4: Signer<'info>,
}

#[derive(Accounts)]
pub struct Signer8<'info> {
    pub account1: Signer<'info>,
    pub account2: Signer<'info>,
    pub account3: Signer<'info>,
    pub account4: Signer<'info>,
    pub account5: Signer<'info>,
    pub account6: Signer<'info>,
    pub account7: Signer<'info>,
    pub account8: Signer<'info>,
}

#[derive(Accounts)]
pub struct SystemAccount1<'info> {
    pub account1: SystemAccount<'info>,
}

#[derive(Accounts)]
pub struct SystemAccount2<'info> {
    pub account1: SystemAccount<'info>,
    pub account2: SystemAccount<'info>,
}

#[derive(Accounts)]
pub struct SystemAccount4<'info> {
    pub account1: SystemAccount<'info>,
    pub account2: SystemAccount<'info>,
    pub account3: SystemAccount<'info>,
    pub account4: SystemAccount<'info>,
}

#[derive(Accounts)]
pub struct SystemAccount8<'info> {
    pub account1: SystemAccount<'info>,
    pub account2: SystemAccount<'info>,
    pub account3: SystemAccount<'info>,
    pub account4: SystemAccount<'info>,
    pub account5: SystemAccount<'info>,
    pub account6: SystemAccount<'info>,
    pub account7: SystemAccount<'info>,
    pub account8: SystemAccount<'info>,
}

#[derive(Accounts)]
pub struct UncheckedAccount1<'info> {
    pub account1: UncheckedAccount<'info>,
}

#[derive(Accounts)]
pub struct UncheckedAccount2<'info> {
    pub account1: UncheckedAccount<'info>,
    pub account2: UncheckedAccount<'info>,
}

#[derive(Accounts)]
pub struct UncheckedAccount4<'info> {
    pub account1: UncheckedAccount<'info>,
    pub account2: UncheckedAccount<'info>,
    pub account3: UncheckedAccount<'info>,
    pub account4: UncheckedAccount<'info>,
}

#[derive(Accounts)]
pub struct UncheckedAccount8<'info> {
    pub account1: UncheckedAccount<'info>,
    pub account2: UncheckedAccount<'info>,
    pub account3: UncheckedAccount<'info>,
    pub account4: UncheckedAccount<'info>,
    pub account5: UncheckedAccount<'info>,
    pub account6: UncheckedAccount<'info>,
    pub account7: UncheckedAccount<'info>,
    pub account8: UncheckedAccount<'info>,
}
