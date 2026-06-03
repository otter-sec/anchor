use anchor_lang::prelude::*;

// Intentionally different program id than the one defined in Anchor.toml.
declare_id!("4D6rvpR7TSPwmFottLGa5gpzMcJ76kN8bimQHV9rogjH");

#[program]
pub mod duplicate_mutable_accounts {
    use super::*;

    pub fn initialize(ctx: Context<Initialize>, initial: u64) -> Result<()> {
        ctx.accounts.data_account.count = initial;
        Ok(())
    }

    // This one should FAIL if the same mutable account is passed twice
    // (Anchor disallows duplicate mutable accounts here).
    pub fn fails_duplicate_mutable(ctx: Context<FailsDuplicateMutable>) -> Result<()> {
        ctx.accounts.account1.count += 1;
        ctx.accounts.account2.count += 1;
        Ok(())
    }

    // This one should SUCCEED even if the same account is passed twice,
    // thanks to the `dup` constraint.
    pub fn allows_duplicate_mutable(ctx: Context<AllowsDuplicateMutable>) -> Result<()> {
        ctx.accounts.account1.count += 1;
        ctx.accounts.account2.count += 1;
        Ok(())
    }

    // Readonly duplicates should always be fine: we just read (no mutation).
    pub fn allows_duplicate_readonly(_ctx: Context<AllowsDuplicateReadonly>) -> Result<()> {
        Ok(())
    }

    // Should FAIL if same mutable account is passed to both composite fields.
    pub fn nested_duplicate(ctx: Context<NestedDuplicate>) -> Result<()> {
        ctx.accounts.wrapper1.counter.count += 1;
        ctx.accounts.wrapper2.counter.count += 1;
        Ok(())
    }

    // Should FAIL if same mutable account is used as a direct field AND inside a composite field.
    pub fn mixed_duplicate(ctx: Context<MixedDuplicate>) -> Result<()> {
        ctx.accounts.account1.count += 1;
        ctx.accounts.wrapper.counter.count += 1;
        Ok(())
    }

    // Test that remaining_accounts are accessible and can be used
    pub fn use_remaining_accounts(ctx: Context<UseRemainingAccounts>) -> Result<()> {
        ctx.accounts.account1.count += 1;

        msg!(
            "Processing {} remaining accounts",
            ctx.remaining_accounts.len()
        );
        for account_info in ctx.remaining_accounts.iter() {
            if account_info.is_writable {
                msg!("Remaining account {} is writable", account_info.key);
            }
        }
        Ok(())
    }

    // Test initializing multiple accounts with the same payer
    pub fn init_multiple_with_same_payer(
        ctx: Context<InitMultipleWithSamePayer>,
        initial1: u64,
        initial2: u64,
    ) -> Result<()> {
        ctx.accounts.data_account1.count = initial1;
        ctx.accounts.data_account2.count = initial2;
        Ok(())
    }

    // Should FAIL if an already-initialized init_if_needed account duplicates
    // another mutable account (double-write on exit).
    pub fn init_if_needed_duplicate_mutable(
        ctx: Context<InitIfNeededDuplicateMutable>,
    ) -> Result<()> {
        ctx.accounts.account_init.count += 1;
        ctx.accounts.account_mut.count += 1;
        Ok(())
    }
}

#[account]
pub struct Counter {
    pub count: u64,
}

#[derive(Accounts)]
pub struct Initialize<'info> {
    #[account(init, payer = user, space = 8 + 8)]
    pub data_account: Account<'info, Counter>,
    #[account(mut)]
    pub user: Signer<'info>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct FailsDuplicateMutable<'info> {
    #[account(mut)]
    pub account1: Account<'info, Counter>,
    #[account(mut)]
    pub account2: Account<'info, Counter>,
}

// Allow the same mutable account to be supplied twice via the `dup` constraint.
#[derive(Accounts)]
pub struct AllowsDuplicateMutable<'info> {
    #[account(mut)]
    pub account1: Account<'info, Counter>,
    #[account(mut, dup)]
    pub account2: Account<'info, Counter>,
}

// Readonly accounts (no `mut`), duplicates allowed by nature.
#[derive(Accounts)]
pub struct AllowsDuplicateReadonly<'info> {
    pub account1: Account<'info, Counter>,
    pub account2: Account<'info, Counter>,
}

// A nested (composite) account struct with a mutable account inside.
#[derive(Accounts)]
pub struct CounterWrapper<'info> {
    #[account(mut)]
    pub counter: Account<'info, Counter>,
}

// Two composite fields
#[derive(Accounts)]
pub struct NestedDuplicate<'info> {
    pub wrapper1: CounterWrapper<'info>,
    pub wrapper2: CounterWrapper<'info>,
}

// Direct field + composite field
#[derive(Accounts)]
pub struct MixedDuplicate<'info> {
    #[account(mut)]
    pub account1: Account<'info, Counter>,
    pub wrapper: CounterWrapper<'info>,
}

// Test using remaining_accounts
#[derive(Accounts)]
pub struct UseRemainingAccounts<'info> {
    #[account(mut)]
    pub account1: Account<'info, Counter>,
}

// Test initializing multiple accounts with the same payer
#[derive(Accounts)]
pub struct InitMultipleWithSamePayer<'info> {
    #[account(init, payer = user, space = 8 + 8)]
    pub data_account1: Account<'info, Counter>,
    #[account(init, payer = user, space = 8 + 8)]
    pub data_account2: Account<'info, Counter>,
    #[account(mut)]
    pub user: Signer<'info>,
    pub system_program: Program<'info, System>,
}

// init_if_needed + a separate mut field pointing at the same account should FAIL
// when the account is already initialized (double-write on exit).
#[derive(Accounts)]
pub struct InitIfNeededDuplicateMutable<'info> {
    #[account(init_if_needed, payer = payer, space = 8 + 8)]
    pub account_init: Account<'info, Counter>,
    #[account(mut)]
    pub account_mut: Account<'info, Counter>,
    #[account(mut)]
    pub payer: Signer<'info>,
    pub system_program: Program<'info, System>,
}
