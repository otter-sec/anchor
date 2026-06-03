use anchor_lang::prelude::*;

declare_id!("BZoppwWi6jMnydnUBEJzotgEXHwLr3b3NramJgZtWeF2");

#[program]
pub mod init_if_needed {
    use super::*;

    // _val only used to make tx different so that it doesn't result
    // in dup tx error
    pub fn initialize(ctx: Context<Initialize>, _val: u8) -> Result<()> {
        ctx.accounts.acc.val = 1000;
        Ok(())
    }

    pub fn second_initialize(ctx: Context<SecondInitialize>, _val: u8) -> Result<()> {
        ctx.accounts.acc.other_val = 2000;
        Ok(())
    }

    pub fn close(ctx: Context<Close>) -> Result<()> {
        ctx.accounts.acc.val = 5000;
        Ok(())
    }
}

#[account]
pub struct MyData {
    pub val: u64,
}

#[account]
pub struct OtherData {
    pub other_val: u64,
}

#[derive(Accounts)]
pub struct Initialize<'info> {
    #[account(init_if_needed, payer = payer, space = 8 + 8)]
    pub acc: Account<'info, MyData>,
    #[account(mut)]
    pub payer: Signer<'info>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct SecondInitialize<'info> {
    #[account(init, payer = payer, space = 8 + 8)]
    pub acc: Account<'info, OtherData>,
    #[account(mut)]
    pub payer: Signer<'info>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct Close<'info> {
    #[account(mut, close = receiver)]
    pub acc: Account<'info, MyData>,
    /// CHECK: ignore
    #[account(mut)]
    pub receiver: UncheckedAccount<'info>,
}
