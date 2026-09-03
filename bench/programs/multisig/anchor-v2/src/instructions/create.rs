use anchor_lang::prelude::*;

use crate::{
    errors::MultisigError,
    state::{MultisigConfig, MAX_SIGNERS},
};

#[derive(Accounts)]
pub struct Create {
    #[account(mut)]
    pub creator: Signer,
    #[account(
        init,
        payer = creator,
        seeds = [b"multisig", creator.address().as_ref()],
        bump
    )]
    pub config: Account<MultisigConfig>,
    pub system_program: Program<System>,
}

impl Create {
    #[inline(always)]
    pub fn create_multisig(
        &mut self,
        threshold: u8,
        remaining: &[AccountView],
    ) -> Result<()> {
        if remaining.len() > MAX_SIGNERS {
            return Err(MultisigError::TooManySigners.into());
        }
        if threshold == 0 || threshold as usize > remaining.len() {
            return Err(MultisigError::InvalidThreshold.into());
        }

        let mut signers_tmp = [Address::new_from_array([0u8; 32]); MAX_SIGNERS];
        for (i, account) in remaining.iter().enumerate() {
            if !account.is_signer() {
                return Err(MultisigError::MissingRequiredSignature.into());
            }
            signers_tmp[i] = *account.address();
        }

        self.config.creator = *self.creator.address();
        self.config.threshold = threshold;
        self.config.bump = 0; // caller writes the bump after init
        self.config
            .signers
            .set_from_slice(&signers_tmp[..remaining.len()]);
        self.config.label.clear();

        Ok(())
    }
}
