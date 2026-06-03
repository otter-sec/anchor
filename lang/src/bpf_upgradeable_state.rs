use crate::{
    error::ErrorCode,
    solana_program::{
        bpf_loader_upgradeable::UpgradeableLoaderState, program_error::ProgramError, pubkey::Pubkey,
    },
    AccountDeserialize, AccountSerialize, Owner, Result,
};

#[derive(Clone)]
pub struct ProgramData {
    pub slot: u64,
    pub upgrade_authority_address: Option<Pubkey>,
}

impl AccountDeserialize for ProgramData {
    fn try_deserialize(buf: &mut &[u8]) -> Result<Self> {
        ProgramData::try_deserialize_unchecked(buf)
    }

    fn try_deserialize_unchecked(buf: &mut &[u8]) -> Result<Self> {
        let program_state = AccountDeserialize::try_deserialize_unchecked(buf)?;

        match program_state {
            UpgradeableLoaderState::Uninitialized => Err(ErrorCode::AccountNotProgramData.into()),
            UpgradeableLoaderState::Buffer {
                authority_address: _,
            } => Err(ErrorCode::AccountNotProgramData.into()),
            UpgradeableLoaderState::Program {
                programdata_address: _,
            } => Err(ErrorCode::AccountNotProgramData.into()),
            UpgradeableLoaderState::ProgramData {
                slot,
                upgrade_authority_address,
            } => Ok(ProgramData {
                slot,
                upgrade_authority_address,
            }),
        }
    }
}

impl AccountSerialize for ProgramData {
    fn try_serialize<W: std::io::Write>(&self, _writer: &mut W) -> Result<()> {
        // no-op
        Ok(())
    }
}

impl Owner for ProgramData {
    fn owner() -> crate::solana_program::pubkey::Pubkey {
        anchor_lang::solana_program::bpf_loader_upgradeable::ID
    }
}

impl Owner for UpgradeableLoaderState {
    fn owner() -> Pubkey {
        anchor_lang::solana_program::bpf_loader_upgradeable::ID
    }
}

impl AccountSerialize for UpgradeableLoaderState {
    fn try_serialize<W: std::io::Write>(&self, _writer: &mut W) -> Result<()> {
        // no-op
        Ok(())
    }
}

impl AccountDeserialize for UpgradeableLoaderState {
    fn try_deserialize(buf: &mut &[u8]) -> Result<Self> {
        UpgradeableLoaderState::try_deserialize_unchecked(buf)
    }

    fn try_deserialize_unchecked(buf: &mut &[u8]) -> Result<Self> {
        bincode::deserialize(buf).map_err(|_| ProgramError::InvalidAccountData.into())
    }
}

#[cfg(feature = "idl-build")]
mod idl_build {
    use super::*;

    impl crate::IdlBuild for ProgramData {}
    impl crate::Discriminator for ProgramData {
        const DISCRIMINATOR: &'static [u8] = &[];
    }
}
