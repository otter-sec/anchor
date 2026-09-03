fn prop_amm_program_id() -> solana_pubkey::Pubkey {
    "55555555555555555555555555555555555555555555"
        .parse()
        .unwrap()
}

/// Deterministic oracle keypair so all case builders hit the same
/// account across a bench run.
fn oracle_keypair() -> solana_keypair::Keypair {
    crate::bench::keypair_for_account("prop-amm-oracle")
}

pub mod anchor_v1 {
    use {
        super::{oracle_keypair, prop_amm_program_id},
        crate::bench::{BenchContext, BenchInstruction},
        anchor_lang_v1::{
            solana_program::system_program, InstructionData, ToAccountMetas,
        },
        anyhow::Result,
        solana_account::Account as SolanaAccount,
        solana_pubkey::Pubkey,
        solana_signer::Signer,
    };

    /// Pre-allocates a fully-initialized Oracle account in LiteSVM so the
    /// update / rotate builders don't each pay for `initialize`. Layout is
    /// `[disc(8) | authority(32) | price(8)]`.
    fn preinit_oracle(ctx: &mut BenchContext, authority: Pubkey, price: u64) -> Result<Pubkey> {
        let mut data = vec![0u8; 48];
        data[0..8].copy_from_slice(
            &<::prop_amm_v1::Oracle as anchor_lang_v1::Discriminator>::DISCRIMINATOR[..8]
                .try_into()
                .unwrap_or([0u8; 8]),
        );
        data[8..40].copy_from_slice(&authority.to_bytes());
        data[40..48].copy_from_slice(&price.to_le_bytes());

        let oracle = oracle_keypair().pubkey();
        ctx.svm_mut()
            .set_account(
                oracle,
                SolanaAccount {
                    lamports: 1_000_000_000,
                    data,
                    owner: prop_amm_program_id(),
                    executable: false,
                    rent_epoch: 0,
                },
            )
            .map_err(|e| anyhow::anyhow!("set_account failed: {e:?}"))?;
        Ok(oracle)
    }

    pub fn build_initialize_case(ctx: &mut BenchContext) -> Result<BenchInstruction> {
        let oracle = oracle_keypair();
        let metas = ::prop_amm_v1::accounts::Initialize {
            payer: ctx.payer_pubkey(),
            oracle: oracle.pubkey(),
            system_program: system_program::ID,
        }
        .to_account_metas(None);
        Ok(BenchInstruction::new(
            ::prop_amm_v1::instruction::Initialize {}.data(),
            metas,
        )
        .with_signer(oracle))
    }

    pub fn build_update_case(ctx: &mut BenchContext) -> Result<BenchInstruction> {
        let authority = crate::bench::keypair_for_account("prop-amm-v1-update-authority");
        let oracle = preinit_oracle(ctx, authority.pubkey(), 0)?;
        ctx.airdrop(&authority.pubkey(), 1_000_000_000)?;

        let metas = ::prop_amm_v1::accounts::Update {
            oracle,
            authority: authority.pubkey(),
        }
        .to_account_metas(None);
        Ok(BenchInstruction::new(
            ::prop_amm_v1::instruction::Update { new_price: 12_345 }.data(),
            metas,
        )
        .with_signer(authority))
    }

    pub fn build_rotate_authority_case(ctx: &mut BenchContext) -> Result<BenchInstruction> {
        let authority = crate::bench::keypair_for_account("prop-amm-v1-rotate-authority");
        let oracle = preinit_oracle(ctx, authority.pubkey(), 0)?;
        let new_authority = crate::bench::keypair_for_account("prop-amm-v1-rotate-new").pubkey();

        let metas = ::prop_amm_v1::accounts::RotateAuthority {
            oracle,
            authority: authority.pubkey(),
        }
        .to_account_metas(None);
        Ok(BenchInstruction::new(
            ::prop_amm_v1::instruction::RotateAuthority { new_authority }.data(),
            metas,
        )
        .with_signer(authority))
    }
}

pub mod anchor_v2 {
    use {
        super::{oracle_keypair, prop_amm_program_id},
        crate::bench::{keypair_for_account, BenchContext, BenchInstruction},
        anchor_lang::{
            solana_program::instruction::AccountMeta, system_program, InstructionData,
        },
        anyhow::Result,
        solana_account::Account as SolanaAccount,
        solana_pubkey::Pubkey,
        solana_signer::Signer,
    };

    /// Reconstructs the keypair matching the on-chain `UPDATE_AUTHORITY`
    /// constant (ed25519 seed `[7u8; 32]`).
    fn update_authority() -> solana_keypair::Keypair {
        solana_keypair::Keypair::new_from_array([7u8; 32])
    }

    /// Pre-allocates a fully-initialized Oracle account directly in LiteSVM so
    /// the update / rotate builders don't each pay for `initialize`. Data is
    /// laid out as `[disc(8) | authority(32) | price(8)]`, matching the POD
    /// layout the asm fast-path writer assumes.
    fn preinit_oracle(ctx: &mut BenchContext, authority: Pubkey, price: u64) -> Result<Pubkey> {
        let mut data = vec![0u8; 48];
        data[0..8].copy_from_slice(
            &<prop_amm_v2::state::Oracle as anchor_lang::Discriminator>::DISCRIMINATOR[..8]
                .try_into()
                .unwrap_or([0u8; 8]),
        );
        data[8..40].copy_from_slice(&authority.to_bytes());
        data[40..48].copy_from_slice(&price.to_le_bytes());

        let oracle = oracle_keypair().pubkey();
        ctx.svm_mut()
            .set_account(
                oracle,
                SolanaAccount {
                    lamports: 1_000_000_000,
                    data,
                    owner: prop_amm_program_id(),
                    executable: false,
                    rent_epoch: 0,
                },
            )
            .map_err(|e| anyhow::anyhow!("set_account failed: {e:?}"))?;
        Ok(oracle)
    }

    pub fn build_initialize_case(ctx: &mut BenchContext) -> Result<BenchInstruction> {
        let oracle = oracle_keypair();
        let metas = vec![
            AccountMeta::new(ctx.payer_pubkey(), true),
            AccountMeta::new(oracle.pubkey(), true),
            AccountMeta::new_readonly(system_program::ID, false),
        ];
        Ok(BenchInstruction::new(
            prop_amm_v2::instruction::Initialize {}.data(),
            metas,
        )
        .with_signer(oracle))
    }

    pub fn build_update_case(ctx: &mut BenchContext) -> Result<BenchInstruction> {
        // The asm path doesn't look at the oracle's authority field — it
        // checks the signer against the hardcoded `UPDATE_AUTHORITY`.
        let authority = update_authority();
        let oracle = preinit_oracle(ctx, authority.pubkey(), 0)?;
        ctx.airdrop(&authority.pubkey(), 1_000_000_000)?;

        let metas = vec![
            AccountMeta::new(oracle, false),
            AccountMeta::new_readonly(authority.pubkey(), true),
        ];
        Ok(BenchInstruction::new(
            prop_amm_v2::instruction::Update { new_price: 12_345 }.data(),
            metas,
        )
        .with_signer(authority))
    }

    pub fn build_rotate_authority_case(ctx: &mut BenchContext) -> Result<BenchInstruction> {
        let authority = keypair_for_account("prop-amm-rotate-authority");
        let oracle = preinit_oracle(ctx, authority.pubkey(), 0)?;
        let new_authority = keypair_for_account("prop-amm-rotate-new").pubkey();

        let metas = vec![
            AccountMeta::new(oracle, false),
            AccountMeta::new_readonly(authority.pubkey(), true),
        ];
        Ok(BenchInstruction::new(
            prop_amm_v2::instruction::RotateAuthority {
                new_authority: new_authority.to_bytes(),
            }
            .data(),
            metas,
        )
        .with_signer(authority))
    }
}
