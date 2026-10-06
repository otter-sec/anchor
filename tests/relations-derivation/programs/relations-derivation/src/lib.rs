//! The typescript example serves to show how one would setup an Anchor
//! workspace with TypeScript tests and migrations.

use anchor_lang::prelude::*;

declare_id!("Fg6PaFpoGXkYsidMpWTK6W2BeZ7FEfcYkg476zPFsLnS");

#[program]
pub mod relations_derivation {
    use super::*;

    pub fn init_base(ctx: Context<InitBase>) -> Result<()> {
        ctx.accounts.account.my_account = ctx.accounts.my_account.key();
        ctx.accounts.account.bump = ctx.bumps.account;
        Ok(())
    }

    pub fn test_relation(_ctx: Context<TestRelation>) -> Result<()> {
        Ok(())
    }

    pub fn test_address(_ctx: Context<TestAddress>) -> Result<()> {
        Ok(())
    }
}

#[derive(Accounts)]
pub struct InitBase<'info> {
    /// CHECK: yeah I know
    #[account(mut)]
    my_account: Signer<'info>,
    #[account(
      init,
      payer = my_account,
      seeds = [b"seed"],
      space = 100,
      bump,
    )]
    account: Account<'info, MyAccount>,
    system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct Nested<'info> {
    /// CHECK: yeah I know
    my_account: UncheckedAccount<'info>,
    #[account(
      has_one = my_account,
      seeds = [b"seed"],
      bump = account.bump
    )]
    account: Account<'info, MyAccount>,
}

#[derive(Accounts)]
pub struct TestRelation<'info> {
    /// CHECK: yeah I know
    my_account: UncheckedAccount<'info>,
    #[account(
      has_one = my_account,
      seeds = [b"seed"],
      bump = account.bump
    )]
    account: Account<'info, MyAccount>,
    nested: Nested<'info>,
}

// Test https://github.com/otter-sec/anchor/issues/4143
pub const ADDRESS_WITH_NUMBER_V2: Pubkey = pubkey!("D8cy77BBepLMngZx6ZukaTff5hCt1HrWyKk3Hnd9oitf");

#[derive(Accounts)]
pub struct TestAddress<'info> {
    // Included wit the `address` field in IDL
    // It's actually `static` but it doesn't matter for our purposes
    #[account(address = crate::ID)]
    constant: UncheckedAccount<'info>,
    #[account(address = ADDRESS_WITH_NUMBER_V2)]
    const_with_number: UncheckedAccount<'info>,
    #[account(address = crate::id())]
    const_fn: UncheckedAccount<'info>,

    // Not included with the `address` field in IDL
    #[account(address = my_account.my_account)]
    field: UncheckedAccount<'info>,
    #[account(address = my_account.my_account())]
    method: UncheckedAccount<'info>,

    #[account(seeds = [b"seed"], bump = my_account.bump)]
    my_account: Account<'info, MyAccount>,
}

#[account]
pub struct MyAccount {
    pub my_account: Pubkey,
    pub bump: u8,
}

impl MyAccount {
    pub fn my_account(&self) -> Pubkey {
        self.my_account
    }
}
