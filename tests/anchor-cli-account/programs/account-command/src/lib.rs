use anchor_lang::prelude::*;

declare_id!("Fg6PaFpoGXkYsidMpWTK6W2BeZ7FEfcYkg476zPFsLnS");

#[program]
pub mod account_command {
    use super::*;

    pub fn initialize(
        ctx: Context<Initialize>,
        balance: f32,
        amount: u32,
        memo: String,
        values: Vec<u128>,
    ) -> Result<()> {
        let my_account = &mut ctx.accounts.my_account;

        my_account.balance = balance;
        my_account.delegate_pubkey = ctx.accounts.user.key().clone();
        my_account.sub = Sub {
            values,
            state: State::Confirmed { amount, memo },
        };

        Ok(())
    }
}

#[derive(Clone, AnchorDeserialize, AnchorSerialize)]
pub enum State {
    Pending,
    Confirmed { amount: u32, memo: String },
}

#[derive(Clone, AnchorDeserialize, AnchorSerialize)]
pub struct Sub {
    pub values: Vec<u128>,
    pub state: State,
}

#[account]
pub struct MyAccount {
    pub balance: f32,
    pub delegate_pubkey: Pubkey,
    pub sub: Sub,
}

#[derive(Accounts)]
pub struct Initialize<'info> {
    #[account(init, payer = user, space = 8 + 1000)]
    pub my_account: Account<'info, MyAccount>,
    #[account(mut)]
    pub user: Signer<'info>,
    pub system_program: Program<'info, System>,
}
