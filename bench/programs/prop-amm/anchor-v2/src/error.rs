use anchor_lang::prelude::*;

#[error_code]
pub enum OracleError {
    #[msg("signer is not the oracle authority")]
    UnauthorizedAuthority,
}
