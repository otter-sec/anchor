use anchor_lang::prelude::*;

use crate::{
    errors::MultisigError,
    state::MultisigConfig,
};

#[derive(Accounts)]
pub struct ExecuteTransfer {
    #[account(
        has_one = creator,
        seeds = [b"multisig", creator.address().as_ref()],
        bump = config.bump
    )]
    pub config: Account<MultisigConfig>,
    pub creator: UncheckedAccount,
    #[account(mut, seeds = [b"vault", config.address().as_ref()], bump)]
    pub vault: UncheckedAccount,
    #[account(mut)]
    pub recipient: UncheckedAccount,
    pub system_program: Program<System>,
}

impl ExecuteTransfer {
    #[inline(always)]
    pub fn verify_and_transfer(
        &self,
        amount: u64,
        vault_bump: u8,
        remaining: &[AccountView],
    ) -> Result<()> {
        let stored_signers = self.config.active_signers();
        let threshold = self.config.threshold;

        let mut approvals = 0u32;
        for account in remaining {
            if !account.is_signer() {
                continue;
            }
            let addr = account.address();
            for stored in stored_signers {
                if addr == stored {
                    approvals = approvals.wrapping_add(1);
                    break;
                }
            }
        }

        if approvals < threshold as u32 {
            return Err(MultisigError::MissingRequiredSignature.into());
        }

        let config_address = self.config.address();
        let vault_bump_bytes = [vault_bump];
        let seeds = [
            pinocchio::cpi::Seed::from(b"vault" as &[u8]),
            pinocchio::cpi::Seed::from(config_address.as_ref()),
            pinocchio::cpi::Seed::from(&vault_bump_bytes as &[u8]),
        ];
        let signer = pinocchio::cpi::Signer::from(&seeds);

        pinocchio_system::instructions::Transfer {
            from: self.vault.account(),
            to: self.recipient.account(),
            lamports: amount,
        }
        .invoke_signed(&[signer])?;

        Ok(())
    }
}
