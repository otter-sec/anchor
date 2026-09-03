pub mod anchor_v1 {
    use {
        crate::bench::{BenchContext, BenchInstruction},
        anchor_lang_v1::{
            solana_program::system_program, InstructionData, ToAccountMetas,
        },
        anyhow::Result,
    };
    
    pub fn build_init_case(ctx: &mut BenchContext) -> Result<BenchInstruction> {
        let program_id = ctx.program_id();
        let (counter_pda, _bump) =
            solana_pubkey::Pubkey::find_program_address(&[b"counter"], &program_id);
    
        let metas = hello_world::accounts::Init {
            payer: ctx.payer_pubkey(),
            counter: counter_pda,
            system_program: system_program::ID,
        }
        .to_account_metas(None);
    
        Ok(BenchInstruction::new(
            hello_world::instruction::Init {}.data(),
            metas,
        ))
    }
}

pub mod anchor_v2 {
    use {
        crate::bench::{BenchContext, BenchInstruction},
        anyhow::Result,
    };

    pub fn build_init_case(ctx: &mut BenchContext) -> Result<BenchInstruction> {
        let ix = hello_world_v2::instruction::Init {}.to_instruction(
            hello_world_v2::accounts::InitResolved { payer: ctx.payer_pubkey() },
        );
        Ok(BenchInstruction::from_instruction(ix))
    }
}

pub mod pinocchio {
    use {
        crate::bench::{BenchContext, BenchInstruction},
        anchor_lang_v1::solana_program::{instruction::AccountMeta, system_program},
        anyhow::Result,
    };
    
    pub fn build_init_case(ctx: &mut BenchContext) -> Result<BenchInstruction> {
        let program_id = ctx.program_id();
        let (counter_pda, _bump) =
            solana_pubkey::Pubkey::find_program_address(&[b"counter"], &program_id);
    
        let metas = vec![
            AccountMeta::new(ctx.payer_pubkey(), true),
            AccountMeta::new(counter_pda, false),
            AccountMeta::new_readonly(system_program::ID, false),
        ];
    
        // Program derives the PDA + bump on-chain (same as v2/quasar/v1/steel),
        // so we pass empty instruction data.
        Ok(BenchInstruction::new(vec![], metas))
    }
}

pub mod quasar {
    use {
        crate::bench::{BenchContext, BenchInstruction},
        anchor_lang_v1::solana_program::{instruction::AccountMeta, system_program},
        anyhow::Result,
    };
    
    pub fn build_init_case(ctx: &mut BenchContext) -> Result<BenchInstruction> {
        let program_id = ctx.program_id();
        let (counter_pda, _bump) =
            solana_pubkey::Pubkey::find_program_address(&[b"counter"], &program_id);
    
        let metas = vec![
            AccountMeta::new(ctx.payer_pubkey(), true),
            AccountMeta::new(counter_pda, false),
            AccountMeta::new_readonly(system_program::ID, false),
        ];
    
        // disc=0 for the `init` instruction, no args.
        Ok(BenchInstruction::new(vec![0u8], metas))
    }
}

pub mod steel {
    use {
        crate::bench::{BenchContext, BenchInstruction},
        anchor_lang_v1::solana_program::{instruction::AccountMeta, system_program},
        anyhow::Result,
    };
    
    pub fn build_init_case(ctx: &mut BenchContext) -> Result<BenchInstruction> {
        let program_id = ctx.program_id();
        let (counter_pda, _bump) =
            solana_pubkey::Pubkey::find_program_address(&[b"counter"], &program_id);
    
        let metas = vec![
            AccountMeta::new(ctx.payer_pubkey(), true),
            AccountMeta::new(counter_pda, false),
            AccountMeta::new_readonly(system_program::ID, false),
        ];
    
        Ok(BenchInstruction::new(vec![], metas))
    }
}
