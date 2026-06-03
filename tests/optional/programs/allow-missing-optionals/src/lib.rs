//! This tests that the `allow-missing-optionals` feature works

use anchor_lang::prelude::*;

declare_id!("ErjUjtqKE5AGWUsjseSJCVLtddM6rhaMbDqmhzraF9h6");

#[program]
mod allow_missing_optionals {
    use super::*;

    pub fn do_stuff(ctx: Context<DoStuff>) -> Result<()> {
        msg!("Doing stuff...");
        let optional_2 = &mut ctx.accounts.optional_2;
        if let Some(data_account) = optional_2 {
            data_account.data = 42;
        }

        Ok(())
    }
}

#[account]
pub struct DataAccount {
    pub data: u64,
}

impl DataAccount {
    pub const LEN: usize = 8 + 8;
}

#[derive(Accounts)]
pub struct DoStuff<'info> {
    #[account(mut)]
    pub payer: Signer<'info>,
    pub system_program: Option<Program<'info, System>>,
    #[account(init, payer = payer, space = DataAccount::LEN)]
    pub optional_2: Option<Account<'info, DataAccount>>,
}
