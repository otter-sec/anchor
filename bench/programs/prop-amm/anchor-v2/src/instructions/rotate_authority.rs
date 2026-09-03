use {
    crate::{error::OracleError, state::Oracle},
    anchor_lang::prelude::*,
};

#[derive(Accounts)]
pub struct RotateAuthority {
    #[account(mut, has_one = authority @ OracleError::UnauthorizedAuthority)]
    pub oracle: Account<Oracle>,
    pub authority: Signer,
}
