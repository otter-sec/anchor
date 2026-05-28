use anchor_lang_v2::prelude::*;

declare_id!("Fg6PaFpoGXkYsidMpWTK6W2BeZ7FEfcYkg476zPFsLnS");

#[account]
#[repr(C)]
pub struct MyData {
    pub value: u64,
}

#[program]
pub mod pda_payer_test {
    use super::*;

    #[discrim = 0]
    pub fn init_with_fresh_target(ctx: &mut Context<InitWithFreshTarget>) -> Result<()> {
        ctx.accounts.new_account.value = 42;
        Ok(())
    }

    #[discrim = 1]
    pub fn init_with_pda_target(ctx: &mut Context<InitWithPdaTarget>) -> Result<()> {
        ctx.accounts.new_account.value = 7;
        Ok(())
    }
}

#[derive(Accounts)]
pub struct InitWithFreshTarget {
    #[account(mut, seeds = [b"payer"], bump)]
    pub pda_payer: SystemAccount,
    #[account(init, payer = pda_payer, space = 8 + core::mem::size_of::<MyData>())]
    pub new_account: Account<MyData>,
    pub system_program: Program<System>,
}

#[derive(Accounts)]
pub struct InitWithPdaTarget {
    #[account(mut, seeds = [b"payer"], bump)]
    pub pda_payer: SystemAccount,
    #[account(
        init,
        payer = pda_payer,
        space = 8 + core::mem::size_of::<MyData>(),
        seeds = [b"target"],
        bump
    )]
    pub new_account: Account<MyData>,
    pub system_program: Program<System>,
}
