use anchor_lang::prelude::*;

declare_id!("66666666666666666666666666666666666666666666");

#[program]
pub mod nested_v1 {
    use super::*;

    pub fn initialize(ctx: Context<Initialize>) -> Result<()> {
        let config = &mut ctx.accounts.config;
        config.admin = ctx.accounts.admin.key();
        config.bump = ctx.bumps.config;

        let counter = &mut ctx.accounts.counter;
        counter.value = 0;
        counter.bump = ctx.bumps.counter;
        Ok(())
    }

    pub fn increment(ctx: Context<Increment>) -> Result<()> {
        ctx.accounts.counter.value += 1;
        Ok(())
    }

    pub fn reset(ctx: Context<Reset>) -> Result<()> {
        ctx.accounts.counter.value = 0;
        Ok(())
    }
}

// --- Instructions ---
//
// This baseline keeps admin + config fields flat in each instruction.
// The v2 example defines AdminConfig once and embeds it directly.
// V1 also supports composite Accounts structs.

#[derive(Accounts)]
pub struct Initialize<'info> {
    #[account(mut)]
    pub admin: Signer<'info>,
    #[account(init, payer = admin, space = 8 + Config::INIT_SPACE, seeds = [b"config"], bump)]
    pub config: Account<'info, Config>,
    #[account(init, payer = admin, space = 8 + Counter::INIT_SPACE, seeds = [b"counter"], bump)]
    pub counter: Account<'info, Counter>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct Increment<'info> {
    // Duplicated admin gate — in v2 this is `admin_config: AdminConfig`
    pub admin: Signer<'info>,
    #[account(has_one = admin, seeds = [b"config"], bump = config.bump)]
    pub config: Account<'info, Config>,
    #[account(mut, seeds = [b"counter"], bump = counter.bump)]
    pub counter: Account<'info, Counter>,
}

#[derive(Accounts)]
pub struct Reset<'info> {
    // Same duplication again
    pub admin: Signer<'info>,
    #[account(has_one = admin, seeds = [b"config"], bump = config.bump)]
    pub config: Account<'info, Config>,
    #[account(mut, seeds = [b"counter"], bump = counter.bump)]
    pub counter: Account<'info, Counter>,
}

// --- State ---

#[account]
#[derive(InitSpace)]
pub struct Config {
    pub admin: Pubkey,
    pub bump: u8,
}

#[account]
#[derive(InitSpace)]
pub struct Counter {
    pub value: u64,
    pub bump: u8,
}
