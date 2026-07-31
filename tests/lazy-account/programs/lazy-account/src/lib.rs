//! Tests demonstraing the usage of [`LazyAccount`].
//!
//! The tests have been simplified by using a stack heavy account in order to demonstrate the usage
//! and its usefulness without adding excessive amount of accounts.
//!
//! See the individual instructions for more documentation: [`Init`], [`Read`], [`Write`].

use anchor_lang::prelude::*;

declare_id!("LazyAccount11111111111111111111111111111111");

#[program]
pub mod lazy_account {
    use super::*;

    pub fn init(ctx: Context<Init>) -> Result<()> {
        let mut my_account = ctx.accounts.my_account.load_mut()?;
        my_account.authority = ctx.accounts.authority.key();

        for _ in 0..MAX_DATA_LEN {
            my_account.dynamic.push(ctx.accounts.authority.key());
        }

        Ok(())
    }

    pub fn read(ctx: Context<Read>) -> Result<()> {
        // Cached load due to the `has_one` constraint
        let authority = ctx.accounts.my_account.load_authority()?;
        msg!("Authority: {}", authority);
        Ok(())
    }

    pub fn write(ctx: Context<Write>, new_authority: Pubkey) -> Result<()> {
        // Cached load due to the `has_one` constraint
        *ctx.accounts.my_account.load_mut_authority()? = new_authority;
        Ok(())
    }

    pub fn unload_after_account_change(ctx: Context<UnloadAfterAccountChange>) -> Result<()> {
        // Populate the cache before imitating a CPI changing the account data.
        drop(ctx.accounts.my_account.load_authority()?);

        ctx.accounts
            .my_account
            .to_account_info()
            .try_borrow_mut_data()?[0] ^= 0xff;

        ctx.accounts.my_account.unload()?;
        Ok(())
    }
}

#[derive(Accounts)]
pub struct Init<'info> {
    #[account(mut)]
    pub authority: Signer<'info>,
    #[account(
        init,
        payer = authority,
        space = MyAccount::DISCRIMINATOR.len() + MyAccount::INIT_SPACE,
        seeds = [b"my_account"],
        bump
    )]
    pub my_account: LazyAccount<'info, MyAccount>,
    /// Using `Account` instead of `LazyAccount` would either make the instruction fail due to
    /// access violation errors, or worse, it would cause undefined behavior instead.
    ///
    /// Using `Account` with Solana v1.18.17 (`platform-tools` v1.41) results in a stack violation
    /// error (without a compiler error/warning on build).
    #[account(
        init,
        payer = authority,
        space = StackHeavyAccount::DISCRIMINATOR.len() + StackHeavyAccount::INIT_SPACE,
        seeds = [b"stack_heavy_account"],
        bump
    )]
    pub stack_heavy_account: LazyAccount<'info, StackHeavyAccount>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct Read<'info> {
    pub authority: Signer<'info>,
    /// Using `Account` or `Box<Account>` instead of `LazyAccount` would increase the compute
    /// units usage by ~90k units due to the unnecessary deserialization of the unused fields.
    #[account(seeds = [b"my_account"], bump, has_one = authority)]
    pub my_account: LazyAccount<'info, MyAccount>,
    /// This account imitates heavy stack usage in more complex programs
    #[account(seeds = [b"stack_heavy_account"], bump)]
    pub stack_heavy_account: Account<'info, StackHeavyAccount>,
}

#[derive(Accounts)]
pub struct Write<'info> {
    pub authority: Signer<'info>,
    /// Using `Account` instead of `LazyAccount` would either make the instruction fail due to stack
    /// violation errors, or worse, it would cause undefined behavior instead.
    ///
    /// Using `Account` with Solana v1.18.17 (`platform-tools` v1.41) results in undefined behavior
    /// in this instruction, and the authority field gets corrupted when writing.
    #[account(mut, seeds = [b"my_account"], bump, has_one = authority)]
    pub my_account: LazyAccount<'info, MyAccount>,
    /// This account imitates heavy stack usage in more complex programs
    #[account(seeds = [b"stack_heavy_account"], bump)]
    pub stack_heavy_account: Account<'info, StackHeavyAccount>,
}

#[derive(Accounts)]
pub struct UnloadAfterAccountChange<'info> {
    #[account(mut, seeds = [b"my_account"], bump)]
    pub my_account: LazyAccount<'info, MyAccount>,
}

const MAX_DATA_LEN: usize = 256;

#[account]
#[derive(InitSpace)]
pub struct MyAccount {
    pub authority: Pubkey,
    /// Fixed size data
    pub fixed: [Pubkey; 8],
    /// Dynamic sized data also works, unlike `AccountLoader`
    #[max_len(MAX_DATA_LEN)]
    pub dynamic: Vec<Pubkey>,
}

/// Stack heavy filler account that imitates heavy stack usage caused my many accounts
#[account]
#[derive(InitSpace)]
pub struct StackHeavyAccount {
    pub data: [u8; 1600],
}
