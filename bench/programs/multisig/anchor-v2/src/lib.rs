use anchor_lang_v2::prelude::*;

mod errors;
mod instructions;
mod state;

pub use errors::*;
use instructions::*;
pub use state::*;

declare_id!("44444444444444444444444444444444444444444444");

#[program]
pub mod multisig_v2 {
    use super::*;

    #[discrim = 0]
    pub fn create(ctx: &mut Context<Create>, threshold: u8) -> Result<()> {
        let remaining = ctx.remaining_accounts()?;
        ctx.accounts.create_multisig(threshold, &remaining)?;
        ctx.accounts.config.bump = ctx.bumps.config;
        Ok(())
    }

    #[discrim = 1]
    pub fn deposit(ctx: &mut Context<Deposit>, amount: u64) -> Result<()> {
        ctx.accounts.deposit(amount)
    }

    #[discrim = 2]
    pub fn set_label(ctx: &mut Context<SetLabel>, label_len: u8, label: [u8; 32]) -> Result<()> {
        ctx.accounts.update_label(label_len, label)
    }

    #[discrim = 3]
    pub fn execute_transfer(ctx: &mut Context<ExecuteTransfer>, amount: u64) -> Result<()> {
        let remaining = ctx.remaining_accounts()?;
        ctx.accounts.verify_and_transfer(amount, ctx.bumps.vault, &remaining)
    }
}
