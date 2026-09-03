use anchor_lang::prelude::*;

#[error_code]
pub enum MultisigError {
    #[msg("Threshold is zero or exceeds signer count")]
    InvalidThreshold,
    #[msg("Signer list exceeds the maximum")]
    TooManySigners,
    #[msg("Required signer did not sign the transaction")]
    MissingRequiredSignature,
    #[msg("Label exceeds maximum length or is not valid UTF-8")]
    LabelTooLong,
    #[msg("Only the original creator can perform this action")]
    UnauthorizedCreator,
}
