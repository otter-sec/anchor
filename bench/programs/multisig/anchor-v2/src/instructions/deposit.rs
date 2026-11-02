use anchor_lang::prelude::*;

use crate::state::MultisigConfig;

#[derive(Accounts)]
pub struct Deposit {
    #[account(mut)]
    pub depositor: Signer,
    pub config: Account<MultisigConfig>,
    #[account(mut, seeds = [b"vault", config.address().as_ref()], bump)]
    pub vault: UncheckedAccount,
    pub system_program: Program<System>,
}

impl Deposit {
    #[inline(always)]
    pub fn deposit(&self, amount: u64) -> Result<()> {
        pinocchio_system::instructions::Transfer {
            from: self.depositor.account(),
            to: self.vault.account(),
            lamports: amount,
        }
        .invoke()?;
        Ok(())
    }
}
