use anchor_lang::prelude::*;

declare_id!("9oECKMeeyf1fWNPKzyrB2x1AbLjHDFjs139kEyFwBpoV");

#[program]
pub mod custom_error {
    use super::*;

    pub fn validate_amount(_ctx: Context<ValidateAmount>, amount: u64) -> Result<()> {
        require!(amount >= 10, CustomError::AmountTooSmall);
        require!(amount <= 100, CustomError::AmountTooLarge);

        msg!("Amount validated successfully: {}", amount);
        Ok(())
    }
}

#[derive(Accounts)]
pub struct ValidateAmount {}

#[error_code]
pub enum CustomError {
    #[msg("Amount must be greater than or equal to 10")]
    AmountTooSmall,
    #[msg("Amount must be less than or equal to 100")]
    AmountTooLarge,
}
