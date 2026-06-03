use anchor_lang::prelude::*;

declare_id!("Fg6PaFpoGXkYsidMpWTK6W2BeZ7FEfcYkg476zPFsLnS");

#[program]
pub mod floats {
    use super::*;

    pub fn create(ctx: Context<Create>, data_f32: f32, data_f64: f64) -> Result<()> {
        let account = &mut ctx.accounts.account;
        let authority = &mut ctx.accounts.authority;

        account.data_f32 = data_f32;
        account.data_f64 = data_f64;
        account.authority = authority.key();

        Ok(())
    }

    pub fn update(ctx: Context<Update>, data_f32: f32, data_f64: f64) -> Result<()> {
        let account = &mut ctx.accounts.account;

        account.data_f32 = data_f32;
        account.data_f64 = data_f64;

        Ok(())
    }
}

#[derive(Accounts)]
pub struct Create<'info> {
    #[account(init, payer = authority, space = 8 + 8 + 4 + 32)]
    pub account: Account<'info, FloatDataAccount>,
    #[account(mut)]
    pub authority: Signer<'info>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct Update<'info> {
    #[account(mut, has_one = authority)]
    pub account: Account<'info, FloatDataAccount>,
    pub authority: Signer<'info>,
}

#[account]
pub struct FloatDataAccount {
    pub data_f64: f64,
    pub data_f32: f32,
    pub authority: Pubkey,
}
