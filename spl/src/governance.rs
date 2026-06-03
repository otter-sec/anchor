/// A macro is exposed so that we can embed the program ID.
#[macro_export]
macro_rules! vote_weight_record {
    ($id:expr) => {
        /// Anchor wrapper for the SPL governance program's VoterWeightRecord type.
        #[derive(Clone)]
        pub struct VoterWeightRecord(spl_governance_addin_api::voter_weight::VoterWeightRecord);

        impl anchor_lang::AccountDeserialize for VoterWeightRecord {
            fn try_deserialize(buf: &mut &[u8]) -> anchor_lang::Result<Self> {
                let mut data = buf;
                let vwr: spl_governance_addin_api::voter_weight::VoterWeightRecord =
                    anchor_lang::AnchorDeserialize::deserialize(&mut data)
                        .map_err(|_| anchor_lang::error::ErrorCode::AccountDidNotDeserialize)?;
                if !anchor_lang::solana_program::program_pack::IsInitialized::is_initialized(&vwr) {
                    return Err(anchor_lang::error::ErrorCode::AccountDidNotSerialize.into());
                }
                Ok(VoterWeightRecord(vwr))
            }

            fn try_deserialize_unchecked(buf: &mut &[u8]) -> anchor_lang::Result<Self> {
                let mut data = buf;
                let vwr: spl_governance_addin_api::voter_weight::VoterWeightRecord =
                    anchor_lang::AnchorDeserialize::deserialize(&mut data)
                        .map_err(|_| anchor_lang::error::ErrorCode::AccountDidNotDeserialize)?;
                Ok(VoterWeightRecord(vwr))
            }
        }

        impl anchor_lang::AccountSerialize for VoterWeightRecord {
            fn try_serialize<W: std::io::Write>(&self, writer: &mut W) -> anchor_lang::Result<()> {
                anchor_lang::AnchorSerialize::serialize(&self.0, writer)
                    .map_err(|_| anchor_lang::error::ErrorCode::AccountDidNotSerialize)?;
                Ok(())
            }
        }

        impl anchor_lang::Owner for VoterWeightRecord {
            fn owner() -> Pubkey {
                $id
            }
        }

        impl std::ops::Deref for VoterWeightRecord {
            type Target = spl_governance_addin_api::voter_weight::VoterWeightRecord;

            fn deref(&self) -> &Self::Target {
                &self.0
            }
        }

        impl std::ops::DerefMut for VoterWeightRecord {
            fn deref_mut(&mut self) -> &mut Self::Target {
                &mut self.0
            }
        }

        #[cfg(feature = "idl-build")]
        impl anchor_lang::IdlBuild for VoterWeightRecord {}

        #[cfg(feature = "idl-build")]
        impl anchor_lang::Discriminator for VoterWeightRecord {
            const DISCRIMINATOR: &'static [u8] = &[];
        }
    };
}
