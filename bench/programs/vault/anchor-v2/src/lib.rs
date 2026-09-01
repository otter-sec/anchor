//! Anchor v2 port of the quasar vault bench. Shape matches `examples/vault`.

use anchor_lang::prelude::*;

declare_id!("33333333333333333333333333333333333333333333");

#[program]
pub mod vault_v2 {
    use super::*;

    #[discrim = 0]
    #[inline(always)]
    pub fn deposit(ctx: &mut Context<Deposit>, amount: u64) -> Result<()> {
        pinocchio_system::instructions::Transfer {
            from: ctx.accounts.user.account(),
            to: ctx.accounts.vault.account(),
            lamports: amount,
        }
        .invoke()?;
        Ok(())
    }

    #[discrim = 1]
    #[inline(always)]
    pub fn withdraw(ctx: &mut Context<Withdraw>, amount: u64) -> Result<()> {
        // `AccountView: Copy` — copies still point at the same backing
        // buffer, so `set_lamports(&mut self)` mutates the underlying
        // account through the raw pointer.
        let mut vault = *ctx.accounts.vault.account();
        let mut user = *ctx.accounts.user.account();
        vault.set_lamports(vault.lamports() - amount);
        user.set_lamports(user.lamports() + amount);
        Ok(())
    }
}

#[derive(Accounts)]
pub struct Deposit {
    #[account(mut)]
    pub user: Signer,
    #[account(mut, seeds = [b"vault", user.address().as_ref()], bump)]
    pub vault: UncheckedAccount,
    pub system_program: Program<System>,
}

#[derive(Accounts)]
pub struct Withdraw {
    #[account(mut)]
    pub user: Signer,
    #[account(mut, seeds = [b"vault", user.address().as_ref()], bump)]
    pub vault: UncheckedAccount,
}
