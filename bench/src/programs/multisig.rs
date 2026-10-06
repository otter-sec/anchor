pub mod anchor_v1 {
    use {
        crate::bench::{keypair_for_account, BenchContext, BenchInstruction},
        anchor_lang_v1::{
            prelude::*,
            solana_program::{instruction::AccountMeta, system_program},
            InstructionData, ToAccountMetas,
        },
        anyhow::Result,
        solana_keypair::Keypair,
        solana_signer::Signer,
    };
    
    pub fn build_create_case(ctx: &mut BenchContext) -> Result<BenchInstruction> {
        let creator = keypair_for_account("multisig-create-creator");
        let signer_one = keypair_for_account("multisig-create-signer-one");
        let signer_two = keypair_for_account("multisig-create-signer-two");
        let (config, _) = multisig_config_address(&creator.pubkey());
    
        ctx.airdrop(&creator.pubkey(), 1_000_000_000)?;
    
        let mut metas = ::multisig_v1::accounts::Create {
            creator: creator.pubkey(),
            config,
            rent: rent::ID,
            system_program: system_program::ID,
        }
        .to_account_metas(None);
        metas.push(AccountMeta::new_readonly(signer_one.pubkey(), true));
        metas.push(AccountMeta::new_readonly(signer_two.pubkey(), true));
    
        Ok(BenchInstruction::new(
            ::multisig_v1::instruction::Create { threshold: 2 }.data(),
            metas,
        )
        .with_signers(vec![creator, signer_one, signer_two]))
    }
    
    pub fn build_deposit_case(ctx: &mut BenchContext) -> Result<BenchInstruction> {
        let creator = keypair_for_account("multisig-deposit-creator");
        let signer_one = keypair_for_account("multisig-deposit-signer-one");
        let signer_two = keypair_for_account("multisig-deposit-signer-two");
        let (config, _) = multisig_config_address(&creator.pubkey());
        let (vault, _) = multisig_vault_address(&config);
    
        ctx.airdrop(&creator.pubkey(), 1_000_000_000)?;
        setup_multisig(ctx, &creator, &[&signer_one, &signer_two], 2)?;
    
        Ok(BenchInstruction::new(
            ::multisig_v1::instruction::Deposit { amount: 2_000_000 }.data(),
            ::multisig_v1::accounts::Deposit {
                depositor: creator.pubkey(),
                config,
                vault,
                system_program: system_program::ID,
            }
            .to_account_metas(None),
        )
        .with_signer(creator))
    }
    
    pub fn build_set_label_case(ctx: &mut BenchContext) -> Result<BenchInstruction> {
        let creator = keypair_for_account("multisig-set-label-creator");
        let signer_one = keypair_for_account("multisig-set-label-signer-one");
        let signer_two = keypair_for_account("multisig-set-label-signer-two");
        let (config, _) = multisig_config_address(&creator.pubkey());
    
        ctx.airdrop(&creator.pubkey(), 1_000_000_000)?;
        setup_multisig(ctx, &creator, &[&signer_one, &signer_two], 2)?;
    
        Ok(BenchInstruction::new(
            ::multisig_v1::instruction::SetLabel {
                label: "bench-multisig".to_owned(),
            }
            .data(),
            ::multisig_v1::accounts::SetLabel {
                creator: creator.pubkey(),
                config,
                system_program: system_program::ID,
            }
            .to_account_metas(None),
        )
        .with_signer(creator))
    }
    
    pub fn build_execute_transfer_case(ctx: &mut BenchContext) -> Result<BenchInstruction> {
        let creator = keypair_for_account("multisig-execute-transfer-creator");
        let signer_one = keypair_for_account("multisig-execute-transfer-signer-one");
        let signer_two = keypair_for_account("multisig-execute-transfer-signer-two");
        let (config, _) = multisig_config_address(&creator.pubkey());
        let (vault, _) = multisig_vault_address(&config);
        let recipient = keypair_for_account("multisig-execute-transfer-recipient");
    
        ctx.airdrop(&creator.pubkey(), 1_000_000_000)?;
        ctx.airdrop(&recipient.pubkey(), 1_000_000)?;
        setup_multisig(ctx, &creator, &[&signer_one, &signer_two], 2)?;
        ctx.execute_with_signers(
            ::multisig_v1::instruction::Deposit { amount: 2_000_000 }.data(),
            ::multisig_v1::accounts::Deposit {
                depositor: creator.pubkey(),
                config,
                vault,
                system_program: system_program::ID,
            }
            .to_account_metas(None),
            &[&creator],
        )?;
    
        let mut metas = ::multisig_v1::accounts::ExecuteTransfer {
            config,
            creator: creator.pubkey(),
            vault,
            recipient: recipient.pubkey(),
            system_program: system_program::ID,
        }
        .to_account_metas(None);
        metas.push(AccountMeta::new_readonly(signer_one.pubkey(), true));
        metas.push(AccountMeta::new_readonly(signer_two.pubkey(), true));
    
        Ok(BenchInstruction::new(
            ::multisig_v1::instruction::ExecuteTransfer { amount: 500_000 }.data(),
            metas,
        )
        .with_signers(vec![signer_one, signer_two]))
    }
    
    fn setup_multisig(
        ctx: &mut BenchContext,
        creator: &Keypair,
        signers: &[&Keypair],
        threshold: u8,
    ) -> Result<()> {
        let (config, _) = multisig_config_address(&creator.pubkey());
        let mut metas = ::multisig_v1::accounts::Create {
            creator: creator.pubkey(),
            config,
            rent: rent::ID,
            system_program: system_program::ID,
        }
        .to_account_metas(None);
    
        for signer in signers {
            metas.push(AccountMeta::new_readonly(signer.pubkey(), true));
        }
    
        let extra_signers = std::iter::once(creator as &dyn solana_signer::Signer)
            .chain(
                signers
                    .iter()
                    .copied()
                    .map(|signer| signer as &dyn solana_signer::Signer),
            )
            .collect::<Vec<_>>();
    
        ctx.execute_with_signers(
            ::multisig_v1::instruction::Create { threshold }.data(),
            metas,
            &extra_signers,
        )?;
    
        Ok(())
    }
    
    fn multisig_config_address(creator: &Pubkey) -> (Pubkey, u8) {
        Pubkey::find_program_address(&[b"multisig", creator.as_ref()], &::multisig_v1::id())
    }
    
    fn multisig_vault_address(config: &Pubkey) -> (Pubkey, u8) {
        Pubkey::find_program_address(&[b"vault", config.as_ref()], &::multisig_v1::id())
    }
}

pub mod anchor_v2 {
    use {
        crate::bench::{keypair_for_account, BenchContext, BenchInstruction},
        anchor_lang::{AccountMeta, InstructionData, ToAccountMetas},
        anyhow::Result,
        solana_keypair::Keypair,
        solana_signer::Signer,
    };

    fn config_address(creator: &anchor_lang::Address) -> (anchor_lang::Address, u8) {
        multisig_v2::accounts::Create::find_config_address(creator)
    }

    pub fn build_create_case(ctx: &mut BenchContext) -> Result<BenchInstruction> {
        let creator = keypair_for_account("multisig-create-creator");
        let signer_one = keypair_for_account("multisig-create-signer-one");
        let signer_two = keypair_for_account("multisig-create-signer-two");

        ctx.airdrop(&creator.pubkey(), 1_000_000_000)?;

        let mut metas = multisig_v2::accounts::CreateResolved { creator: creator.pubkey() }
            .to_account_metas(None);
        metas.push(AccountMeta::new_readonly(signer_one.pubkey(), true));
        metas.push(AccountMeta::new_readonly(signer_two.pubkey(), true));

        Ok(BenchInstruction::new(
            multisig_v2::instruction::Create { threshold: 2 }.data(),
            metas,
        )
        .with_signers(vec![creator, signer_one, signer_two]))
    }

    pub fn build_deposit_case(ctx: &mut BenchContext) -> Result<BenchInstruction> {
        let creator = keypair_for_account("multisig-deposit-creator");
        let signer_one = keypair_for_account("multisig-deposit-signer-one");
        let signer_two = keypair_for_account("multisig-deposit-signer-two");
        let (config, _) = config_address(&creator.pubkey());

        ctx.airdrop(&creator.pubkey(), 1_000_000_000)?;
        setup_multisig_v2(ctx, &creator, &[&signer_one, &signer_two], 2)?;

        Ok(BenchInstruction::new(
            multisig_v2::instruction::Deposit { amount: 2_000_000 }.data(),
            multisig_v2::accounts::DepositResolved {
                depositor: creator.pubkey(),
                config,
            }
            .to_account_metas(None),
        )
        .with_signer(creator))
    }

    pub fn build_set_label_case(ctx: &mut BenchContext) -> Result<BenchInstruction> {
        let creator = keypair_for_account("multisig-set-label-creator");
        let signer_one = keypair_for_account("multisig-set-label-signer-one");
        let signer_two = keypair_for_account("multisig-set-label-signer-two");

        ctx.airdrop(&creator.pubkey(), 1_000_000_000)?;
        setup_multisig_v2(ctx, &creator, &[&signer_one, &signer_two], 2)?;

        let (config, _) = config_address(&creator.pubkey());
        let label_bytes = b"bench-multisig";
        let mut label = [0u8; 32];
        label[..label_bytes.len()].copy_from_slice(label_bytes);
        Ok(BenchInstruction::new(
            multisig_v2::instruction::SetLabel {
                label_len: label_bytes.len() as u8,
                label,
            }
            .data(),
            multisig_v2::accounts::SetLabelResolved {
                creator: creator.pubkey(),
                config,
            }
            .to_account_metas(None),
        )
        .with_signer(creator))
    }

    pub fn build_execute_transfer_case(ctx: &mut BenchContext) -> Result<BenchInstruction> {
        let creator = keypair_for_account("multisig-execute-transfer-creator");
        let signer_one = keypair_for_account("multisig-execute-transfer-signer-one");
        let signer_two = keypair_for_account("multisig-execute-transfer-signer-two");
        let (config, _) = config_address(&creator.pubkey());
        let recipient = keypair_for_account("multisig-execute-transfer-recipient");

        ctx.airdrop(&creator.pubkey(), 1_000_000_000)?;
        ctx.airdrop(&recipient.pubkey(), 1_000_000)?;
        setup_multisig_v2(ctx, &creator, &[&signer_one, &signer_two], 2)?;
        ctx.execute_with_signers(
            multisig_v2::instruction::Deposit { amount: 2_000_000 }.data(),
            multisig_v2::accounts::DepositResolved {
                depositor: creator.pubkey(),
                config,
            }
            .to_account_metas(None),
            &[&creator],
        )?;

        let mut metas = multisig_v2::accounts::ExecuteTransferResolved {
            config,
            creator: creator.pubkey(),
            recipient: recipient.pubkey(),
        }
        .to_account_metas(None);
        metas.push(AccountMeta::new_readonly(signer_one.pubkey(), true));
        metas.push(AccountMeta::new_readonly(signer_two.pubkey(), true));

        Ok(BenchInstruction::new(
            multisig_v2::instruction::ExecuteTransfer { amount: 500_000 }.data(),
            metas,
        )
        .with_signers(vec![signer_one, signer_two]))
    }

    fn setup_multisig_v2(
        ctx: &mut BenchContext,
        creator: &Keypair,
        signers: &[&Keypair],
        threshold: u8,
    ) -> Result<()> {
        let mut metas = multisig_v2::accounts::CreateResolved { creator: creator.pubkey() }
            .to_account_metas(None);

        for signer in signers {
            metas.push(AccountMeta::new_readonly(signer.pubkey(), true));
        }

        let extra_signers = std::iter::once(creator as &dyn solana_signer::Signer)
            .chain(
                signers
                    .iter()
                    .copied()
                    .map(|signer| signer as &dyn solana_signer::Signer),
            )
            .collect::<Vec<_>>();

        ctx.execute_with_signers(
            multisig_v2::instruction::Create { threshold }.data(),
            metas,
            &extra_signers,
        )?;

        Ok(())
    }
}
