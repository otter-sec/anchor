use anchor_lang::prelude::*;

declare_id!("Fg6PaFpoGXkYsidMpWTK6W2BeZ7FEfcYkg476zPFsLnS");

#[program]
pub mod callee {
    use super::*;

    #[derive(AnchorSerialize, AnchorDeserialize)]
    pub struct StructReturn {
        pub value: u64,
    }

    pub fn initialize(ctx: Context<Initialize>) -> Result<()> {
        let account = &mut ctx.accounts.account;
        account.value = 10;
        Ok(())
    }

    pub fn return_u64(_ctx: Context<CpiReturn>) -> Result<u64> {
        Ok(10)
    }

    pub fn return_u64_spoofed(_ctx: Context<CpiReturn>) -> Result<u64> {
        Ok(999)
    }

    pub fn return_struct(_ctx: Context<CpiReturn>) -> Result<StructReturn> {
        let s = StructReturn { value: 11 };
        Ok(s)
    }

    pub fn return_vec(_ctx: Context<CpiReturn>) -> Result<Vec<u8>> {
        Ok(vec![12, 13, 14, 100])
    }

    // Used for testing views
    pub fn return_u64_from_account(ctx: Context<CpiReturn>) -> Result<u64> {
        let account = &ctx.accounts.account;
        Ok(account.value)
    }
}

#[derive(Accounts)]
pub struct Initialize<'info> {
    #[account(init, payer = user, space = 8 + 8)]
    pub account: Account<'info, CpiReturnAccount>,
    #[account(mut)]
    pub user: Signer<'info>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct CpiReturn<'info> {
    pub account: Account<'info, CpiReturnAccount>,
}

#[account]
pub struct CpiReturnAccount {
    pub value: u64,
}
