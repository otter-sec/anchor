use {
    anchor_lang::{FixedVec, MAX_RUNTIME_ACCOUNT_METAS},
    pinocchio::address::Address,
    solana_program_error::ProgramError,
    solana_pubkey::Pubkey,
};

#[cfg(any(feature = "guardrails", test))]
use anchor_lang::{programs::Token2022, require_eq, Id};

#[cfg(feature = "guardrails")]
#[inline]
pub(crate) fn validate_token_2022_program(program: &Address) -> Result<(), ProgramError> {
    require_eq!(*program, Token2022::id(), ProgramError::IncorrectProgramId);
    Ok(())
}

#[cfg(not(feature = "guardrails"))]
#[inline]
pub(crate) fn validate_token_2022_program(_program: &Address) -> Result<(), ProgramError> {
    Ok(())
}

pub(crate) fn pubkey_refs<'a>(
    pubkeys: &'a [Pubkey],
) -> Result<FixedVec<&'a Pubkey, MAX_RUNTIME_ACCOUNT_METAS>, ProgramError> {
    let mut refs = FixedVec::new();
    for pubkey in pubkeys {
        refs.try_push(pubkey)
            .map_err(|_| ProgramError::InvalidArgument)?;
    }
    Ok(refs)
}
