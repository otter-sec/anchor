use {
    crate::{state::MultisigConfig, ErrorCode},
    anchor_lang::{prelude::*, system_program},
};

#[derive(Accounts)]
pub struct ExecuteTransfer<'info> {
    #[account(
        has_one = creator,
        seeds = [b"multisig", creator.key().as_ref()],
        bump = config.bump
    )]
    pub config: Account<'info, MultisigConfig>,
    pub creator: UncheckedAccount<'info>,
    #[account(mut, seeds = [b"vault", config.key().as_ref()], bump)]
    pub vault: UncheckedAccount<'info>,
    #[account(mut)]
    pub recipient: UncheckedAccount<'info>,
    pub system_program: Program<'info, System>,
}

impl<'info> ExecuteTransfer<'info> {
    #[inline(always)]
    pub fn verify_and_transfer(
        &self,
        amount: u64,
        bumps: &ExecuteTransferBumps,
        remaining: &[AccountInfo<'info>],
    ) -> Result<()> {
        let stored_signers = self.config.signers();
        let threshold = self.config.threshold;

        let mut approvals = 0u32;
        for signer in stored_signers {
            if remaining
                .iter()
                .any(|account| account.is_signer && account.key == signer)
            {
                approvals += 1;
            }
        }

        require!(
            approvals >= threshold as u32,
            ErrorCode::MissingRequiredSignature
        );

        let config_key = self.config.key();
        let vault_bump = [bumps.vault];
        let signer_seeds: &[&[u8]] = &[b"vault", config_key.as_ref(), &vault_bump];

        system_program::transfer(
            CpiContext::new_with_signer(
                self.system_program.key(),
                system_program::Transfer {
                    from: self.vault.to_account_info(),
                    to: self.recipient.to_account_info(),
                },
                &[signer_seeds],
            ),
            amount,
        )
    }
}
