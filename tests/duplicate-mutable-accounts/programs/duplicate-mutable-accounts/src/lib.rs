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

    // Should FAIL if the same fresh account is passed for both the `zero` and the `init` field.
    // `init` runs before every other constraint, so it leaves the account program-owned and
    // zero-filled, which is exactly what `zero` accepts.
    pub fn zero_then_init(ctx: Context<ZeroThenInit>) -> Result<()> {
        ctx.accounts.zero_account.count = 1;
        ctx.accounts.init_account.count = 2;
        Ok(())
    }

    // Same rule, opposite declaration order.
    pub fn init_then_zero(ctx: Context<InitThenZero>) -> Result<()> {
        ctx.accounts.zero_account.count = 1;
        ctx.accounts.init_account.count = 2;
        Ok(())
    }

    // `dup` opts an account out of the duplicate-mutable check, but it must not re-open the
    // `zero`/`init` alias: the `zero` uniqueness scan still has to reject this.
    pub fn zero_dup_then_init(ctx: Context<ZeroDupThenInit>) -> Result<()> {
        ctx.accounts.zero_account.count = 1;
        ctx.accounts.init_account.count = 2;
        Ok(())
    }

    // Should FAIL when the `init` and `zero` fields live in two different composite structs.
    pub fn composite_init_and_zero(ctx: Context<CompositeInitAndZero>) -> Result<()> {
        ctx.accounts.init_part.counter.count = 1;
        ctx.accounts.zero_part.counter.count = 2;
        Ok(())
    }

    // Should FAIL when a direct `zero` field aliases an `init` field inside a composite.
    pub fn mixed_zero_and_composite_init(ctx: Context<MixedZeroAndCompositeInit>) -> Result<()> {
        ctx.accounts.zero_account.count = 1;
        ctx.accounts.init_part.counter.count = 2;
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

// `zero` declared before `init`. The same fresh account must not satisfy both.
#[derive(Accounts)]
pub struct ZeroThenInit<'info> {
    #[account(zero)]
    pub zero_account: Account<'info, Counter>,
    #[account(init, payer = payer, space = 8 + 8)]
    pub init_account: Account<'info, Counter>,
    #[account(mut)]
    pub payer: Signer<'info>,
    pub system_program: Program<'info, System>,
}

// `init` declared before `zero`.
#[derive(Accounts)]
pub struct InitThenZero<'info> {
    #[account(init, payer = payer, space = 8 + 8)]
    pub init_account: Account<'info, Counter>,
    #[account(zero)]
    pub zero_account: Account<'info, Counter>,
    #[account(mut)]
    pub payer: Signer<'info>,
    pub system_program: Program<'info, System>,
}

// `dup` skips the duplicate-mutable check, so the `zero` uniqueness scan is the only thing
// standing between this struct and an aliased account.
#[derive(Accounts)]
pub struct ZeroDupThenInit<'info> {
    #[account(zero, dup)]
    pub zero_account: Account<'info, Counter>,
    #[account(init, payer = payer, space = 8 + 8)]
    pub init_account: Account<'info, Counter>,
    #[account(mut)]
    pub payer: Signer<'info>,
    pub system_program: Program<'info, System>,
}

// Composite holding an `init` account.
#[derive(Accounts)]
pub struct InitWrapper<'info> {
    #[account(init, payer = payer, space = 8 + 8)]
    pub counter: Account<'info, Counter>,
    #[account(mut)]
    pub payer: Signer<'info>,
    pub system_program: Program<'info, System>,
}

// Composite holding a `zero` account.
#[derive(Accounts)]
pub struct ZeroWrapper<'info> {
    #[account(zero)]
    pub counter: Account<'info, Counter>,
}

// `init` and `zero` split across two composites.
#[derive(Accounts)]
pub struct CompositeInitAndZero<'info> {
    pub init_part: InitWrapper<'info>,
    pub zero_part: ZeroWrapper<'info>,
}

// Direct `zero` field aliasing an `init` account inside a composite.
#[derive(Accounts)]
pub struct MixedZeroAndCompositeInit<'info> {
    #[account(zero)]
    pub zero_account: Account<'info, Counter>,
    pub init_part: InitWrapper<'info>,
}
