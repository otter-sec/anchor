use anchor_lang::prelude::*;

declare_id!("66666666666666666666666666666666666666666666");

#[program]
pub mod nested_v2 {
    use super::*;

    /// Create the config and counter accounts.
    #[discrim = 0]
    pub fn initialize(ctx: &mut Context<Initialize>) -> Result<()> {
        ctx.accounts.config.admin = *ctx.accounts.admin.address();
        ctx.accounts.config.bump = ctx.bumps.config;
        ctx.accounts.counter.value = 0;
        ctx.accounts.counter.bump = ctx.bumps.counter;
        Ok(())
    }

    /// Increment the counter. Only the admin (validated via the nested
    /// `AdminConfig` struct) can call this.
    #[discrim = 1]
    pub fn increment(ctx: &mut Context<Increment>) -> Result<()> {
        ctx.accounts.counter.value += 1;
        Ok(())
    }

    /// Reset the counter to zero. Same admin gate, different action —
    /// the `Nested<AdminConfig>` reuses the identical validation.
    #[discrim = 2]
    pub fn reset(ctx: &mut Context<Reset>) -> Result<()> {
        ctx.accounts.counter.value = 0;
        Ok(())
    }
}

// --- Shared admin validation ---
//
// In v1 you'd duplicate these two fields + constraints in every instruction
// that needs admin access. With Nested<T>, define them once.

#[derive(Accounts)]
pub struct AdminConfig {
    pub admin: Signer,
    #[account(has_one = admin, seeds = [b"config"], bump = config.bump)]
    pub config: Account<Config>,
}

// --- Instructions ---

#[derive(Accounts)]
pub struct Initialize {
    #[account(mut)]
    pub admin: Signer,
    #[account(init, payer = admin, seeds = [b"config"], bump)]
    pub config: Account<Config>,
    #[account(init, payer = admin, seeds = [b"counter"], bump)]
    pub counter: Account<Counter>,
    pub system_program: Program<System>,
}

#[derive(Accounts)]
pub struct Increment {
    /// Reuses AdminConfig's signer + has_one check.
    pub admin_config: Nested<AdminConfig>,
    #[account(mut, seeds = [b"counter"], bump = counter.bump)]
    pub counter: Account<Counter>,
}

#[derive(Accounts)]
pub struct Reset {
    /// Same composition — zero duplication.
    pub admin_config: Nested<AdminConfig>,
    #[account(mut, seeds = [b"counter"], bump = counter.bump)]
    pub counter: Account<Counter>,
}

// --- State ---

#[account]
pub struct Config {
    pub admin: Address,
    pub bump: u8,
    pub _pad: [u8; 7],
}

#[account]
pub struct Counter {
    pub value: u64,
    pub bump: u8,
    pub _pad: [u8; 7],
}
