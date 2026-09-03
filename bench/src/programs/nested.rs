pub mod anchor_v1 {
    use {
        crate::bench::{keypair_for_account, BenchContext, BenchInstruction},
        anchor_lang_v1::{
            prelude::*,
            solana_program::system_program,
            InstructionData, ToAccountMetas,
        },
        anyhow::Result,
        solana_signer::Signer,
    };

    fn config_address() -> (Pubkey, u8) {
        Pubkey::find_program_address(&[b"config"], &nested_v1::id())
    }

    fn counter_address() -> (Pubkey, u8) {
        Pubkey::find_program_address(&[b"counter"], &nested_v1::id())
    }

    pub fn build_initialize_case(ctx: &mut BenchContext) -> Result<BenchInstruction> {
        let admin = keypair_for_account("nested-admin");
        ctx.airdrop(&admin.pubkey(), 1_000_000_000)?;

        let (config, _) = config_address();
        let (counter, _) = counter_address();

        let metas = nested_v1::accounts::Initialize {
            admin: admin.pubkey(),
            config,
            counter,
            system_program: system_program::ID,
        }
        .to_account_metas(None);

        Ok(BenchInstruction::new(
            nested_v1::instruction::Initialize {}.data(),
            metas,
        )
        .with_signer(admin))
    }

    pub fn build_increment_case(ctx: &mut BenchContext) -> Result<BenchInstruction> {
        let admin = keypair_for_account("nested-increment-admin");
        ctx.airdrop(&admin.pubkey(), 1_000_000_000)?;
        setup_initialized(ctx, &admin)?;

        let (config, _) = Pubkey::find_program_address(&[b"config"], &nested_v1::id());
        let (counter, _) = Pubkey::find_program_address(&[b"counter"], &nested_v1::id());

        let metas = nested_v1::accounts::Increment {
            admin: admin.pubkey(),
            config,
            counter,
        }
        .to_account_metas(None);

        Ok(BenchInstruction::new(
            nested_v1::instruction::Increment {}.data(),
            metas,
        )
        .with_signer(admin))
    }

    pub fn build_reset_case(ctx: &mut BenchContext) -> Result<BenchInstruction> {
        let admin = keypair_for_account("nested-reset-admin");
        ctx.airdrop(&admin.pubkey(), 1_000_000_000)?;
        setup_initialized(ctx, &admin)?;

        let (config, _) = Pubkey::find_program_address(&[b"config"], &nested_v1::id());
        let (counter, _) = Pubkey::find_program_address(&[b"counter"], &nested_v1::id());

        let metas = nested_v1::accounts::Reset {
            admin: admin.pubkey(),
            config,
            counter,
        }
        .to_account_metas(None);

        Ok(BenchInstruction::new(
            nested_v1::instruction::Reset {}.data(),
            metas,
        )
        .with_signer(admin))
    }

    fn setup_initialized(
        ctx: &mut BenchContext,
        admin: &solana_keypair::Keypair,
    ) -> Result<()> {
        let (config, _) = config_address();
        let (counter, _) = counter_address();

        ctx.execute_with_signers(
            nested_v1::instruction::Initialize {}.data(),
            nested_v1::accounts::Initialize {
                admin: admin.pubkey(),
                config,
                counter,
                system_program: system_program::ID,
            }
            .to_account_metas(None),
            &[admin],
        )?;
        Ok(())
    }
}

pub mod anchor_v2 {
    use {
        crate::bench::{keypair_for_account, BenchContext, BenchInstruction},
        anchor_lang::{
            solana_program::instruction::AccountMeta, InstructionData,
        },
        anyhow::Result,
        solana_pubkey::Pubkey,
        solana_signer::Signer,
    };

    fn config_address() -> (Pubkey, u8) {
        Pubkey::find_program_address(&[b"config"], &nested_v2::id())
    }

    fn counter_address() -> (Pubkey, u8) {
        Pubkey::find_program_address(&[b"counter"], &nested_v2::id())
    }

    pub fn build_initialize_case(ctx: &mut BenchContext) -> Result<BenchInstruction> {
        let admin = keypair_for_account("nested-admin");
        ctx.airdrop(&admin.pubkey(), 1_000_000_000)?;

        let ix = nested_v2::instruction::Initialize {}.to_instruction(
            nested_v2::accounts::InitializeResolved { admin: admin.pubkey() },
        );
        Ok(BenchInstruction::from_instruction(ix).with_signer(admin))
    }

    pub fn build_increment_case(ctx: &mut BenchContext) -> Result<BenchInstruction> {
        let admin = keypair_for_account("nested-increment-admin");
        ctx.airdrop(&admin.pubkey(), 1_000_000_000)?;
        setup_initialized(ctx, &admin)?;

        let (config, _) = config_address();
        let (counter, _) = counter_address();

        // Accounts in order: Nested<AdminConfig>{admin, config}, counter
        let metas = vec![
            AccountMeta::new_readonly(admin.pubkey(), true),
            AccountMeta::new_readonly(config, false),
            AccountMeta::new(counter, false),
        ];
        Ok(BenchInstruction::new(
            nested_v2::instruction::Increment {}.data(),
            metas,
        )
        .with_signer(admin))
    }

    pub fn build_reset_case(ctx: &mut BenchContext) -> Result<BenchInstruction> {
        let admin = keypair_for_account("nested-reset-admin");
        ctx.airdrop(&admin.pubkey(), 1_000_000_000)?;
        setup_initialized(ctx, &admin)?;

        let (config, _) = config_address();
        let (counter, _) = counter_address();

        let metas = vec![
            AccountMeta::new_readonly(admin.pubkey(), true),
            AccountMeta::new_readonly(config, false),
            AccountMeta::new(counter, false),
        ];
        Ok(BenchInstruction::new(
            nested_v2::instruction::Reset {}.data(),
            metas,
        )
        .with_signer(admin))
    }

    fn setup_initialized(
        ctx: &mut BenchContext,
        admin: &solana_keypair::Keypair,
    ) -> Result<()> {
        let ix = nested_v2::instruction::Initialize {}.to_instruction(
            nested_v2::accounts::InitializeResolved { admin: admin.pubkey() },
        );
        ctx.execute_with_signers(ix.data, ix.accounts, &[admin])?;
        Ok(())
    }
}
