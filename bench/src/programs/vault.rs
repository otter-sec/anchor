//! Bench case builders for the vault family. All five variants share the
//! program id `33333333333333333333333333333333333333333333` so
//! `find_program_address` returns the same PDAs across frameworks, and the
//! CU comparison isolates framework cost.
//!
//! `withdraw` mutates vault lamports via direct arithmetic, which requires
//! the vault PDA to be program-owned. `deposit` via `system::Transfer` leaves
//! it system-owned, so every framework's `withdraw` builder pre-funds the
//! vault as a program-owned account via `set_account` — same pattern as the
//! original quasar harness.

use {
    crate::bench::{keypair_for_account, BenchContext, BenchInstruction},
    anchor_lang_v1::solana_program::{instruction::AccountMeta, system_program},
    anyhow::Result,
    solana_account::Account as SolanaAccount,
    solana_pubkey::Pubkey,
    solana_signer::Signer,
};

fn vault_program_id() -> Pubkey {
    "33333333333333333333333333333333333333333333"
        .parse()
        .unwrap()
}

fn vault_address(user: &Pubkey) -> (Pubkey, u8) {
    Pubkey::find_program_address(&[b"vault", user.as_ref()], &vault_program_id())
}

fn preload_program_owned_vault(ctx: &mut BenchContext, vault: Pubkey) -> Result<()> {
    ctx.svm_mut()
        .set_account(
            vault,
            SolanaAccount {
                lamports: 1_000_000_000,
                data: vec![],
                owner: vault_program_id(),
                executable: false,
                rent_epoch: 0,
            },
        )
        .map_err(|e| anyhow::anyhow!("set_account failed: {e:?}"))
}

fn make_raw_ix_data(disc: u8, amount: u64) -> Vec<u8> {
    let mut data = Vec::with_capacity(9);
    data.push(disc);
    data.extend_from_slice(&amount.to_le_bytes());
    data
}

pub mod anchor_v1 {
    use {
        super::*,
        anchor_lang_v1::{InstructionData, ToAccountMetas},
    };

    pub fn build_deposit_case(ctx: &mut BenchContext) -> Result<BenchInstruction> {
        let user = keypair_for_account("vault-deposit-user");
        let (vault, _) = vault_address(&user.pubkey());
        ctx.airdrop(&user.pubkey(), 1_000_000_000)?;

        let metas = ::vault_v1::accounts::Deposit {
            user: user.pubkey(),
            vault,
            system_program: system_program::ID,
        }
        .to_account_metas(None);

        Ok(BenchInstruction::new(
            ::vault_v1::instruction::Deposit { amount: 1_000_000 }.data(),
            metas,
        )
        .with_signer(user))
    }

    pub fn build_withdraw_case(ctx: &mut BenchContext) -> Result<BenchInstruction> {
        let user = keypair_for_account("vault-withdraw-user");
        let (vault, _) = vault_address(&user.pubkey());
        ctx.airdrop(&user.pubkey(), 1_000_000_000)?;
        preload_program_owned_vault(ctx, vault)?;

        let metas = ::vault_v1::accounts::Withdraw {
            user: user.pubkey(),
            vault,
        }
        .to_account_metas(None);

        Ok(BenchInstruction::new(
            ::vault_v1::instruction::Withdraw { amount: 1_000_000 }.data(),
            metas,
        )
        .with_signer(user))
    }
}

pub mod anchor_v2 {
    use {super::*, anchor_lang::InstructionData};

    pub fn build_deposit_case(ctx: &mut BenchContext) -> Result<BenchInstruction> {
        let user = keypair_for_account("vault-deposit-user");
        let (vault, _) = vault_address(&user.pubkey());
        ctx.airdrop(&user.pubkey(), 1_000_000_000)?;

        let metas = vec![
            AccountMeta::new(user.pubkey(), true),
            AccountMeta::new(vault, false),
            AccountMeta::new_readonly(system_program::ID, false),
        ];

        Ok(BenchInstruction::new(
            vault_v2::instruction::Deposit { amount: 1_000_000 }.data(),
            metas,
        )
        .with_signer(user))
    }

    pub fn build_withdraw_case(ctx: &mut BenchContext) -> Result<BenchInstruction> {
        let user = keypair_for_account("vault-withdraw-user");
        let (vault, _) = vault_address(&user.pubkey());
        ctx.airdrop(&user.pubkey(), 1_000_000_000)?;
        preload_program_owned_vault(ctx, vault)?;

        let metas = vec![
            AccountMeta::new(user.pubkey(), true),
            AccountMeta::new(vault, false),
        ];

        Ok(BenchInstruction::new(
            vault_v2::instruction::Withdraw { amount: 1_000_000 }.data(),
            metas,
        )
        .with_signer(user))
    }
}

pub mod quasar {
    use super::*;

    pub fn build_deposit_case(ctx: &mut BenchContext) -> Result<BenchInstruction> {
        let user = keypair_for_account("vault-deposit-user");
        let (vault, _) = vault_address(&user.pubkey());
        ctx.airdrop(&user.pubkey(), 1_000_000_000)?;

        let metas = vec![
            AccountMeta::new(user.pubkey(), true),
            AccountMeta::new(vault, false),
            AccountMeta::new_readonly(system_program::ID, false),
        ];
        Ok(BenchInstruction::new(make_raw_ix_data(0, 1_000_000), metas)
            .with_signer(user))
    }

    pub fn build_withdraw_case(ctx: &mut BenchContext) -> Result<BenchInstruction> {
        let user = keypair_for_account("vault-withdraw-user");
        let (vault, _) = vault_address(&user.pubkey());
        ctx.airdrop(&user.pubkey(), 1_000_000_000)?;
        preload_program_owned_vault(ctx, vault)?;

        let metas = vec![
            AccountMeta::new(user.pubkey(), true),
            AccountMeta::new(vault, false),
        ];
        Ok(BenchInstruction::new(make_raw_ix_data(1, 1_000_000), metas)
            .with_signer(user))
    }
}

pub mod pinocchio {
    use super::*;

    pub fn build_deposit_case(ctx: &mut BenchContext) -> Result<BenchInstruction> {
        let user = keypair_for_account("vault-deposit-user");
        let (vault, _) = vault_address(&user.pubkey());
        ctx.airdrop(&user.pubkey(), 1_000_000_000)?;

        let metas = vec![
            AccountMeta::new(user.pubkey(), true),
            AccountMeta::new(vault, false),
            AccountMeta::new_readonly(system_program::ID, false),
        ];
        Ok(BenchInstruction::new(make_raw_ix_data(0, 1_000_000), metas)
            .with_signer(user))
    }

    pub fn build_withdraw_case(ctx: &mut BenchContext) -> Result<BenchInstruction> {
        let user = keypair_for_account("vault-withdraw-user");
        let (vault, _) = vault_address(&user.pubkey());
        ctx.airdrop(&user.pubkey(), 1_000_000_000)?;
        preload_program_owned_vault(ctx, vault)?;

        let metas = vec![
            AccountMeta::new(user.pubkey(), true),
            AccountMeta::new(vault, false),
        ];
        Ok(BenchInstruction::new(make_raw_ix_data(1, 1_000_000), metas)
            .with_signer(user))
    }
}

pub mod steel {
    use super::*;

    pub fn build_deposit_case(ctx: &mut BenchContext) -> Result<BenchInstruction> {
        let user = keypair_for_account("vault-deposit-user");
        let (vault, _) = vault_address(&user.pubkey());
        ctx.airdrop(&user.pubkey(), 1_000_000_000)?;

        let metas = vec![
            AccountMeta::new(user.pubkey(), true),
            AccountMeta::new(vault, false),
            AccountMeta::new_readonly(system_program::ID, false),
        ];
        Ok(BenchInstruction::new(make_raw_ix_data(0, 1_000_000), metas)
            .with_signer(user))
    }

    pub fn build_withdraw_case(ctx: &mut BenchContext) -> Result<BenchInstruction> {
        let user = keypair_for_account("vault-withdraw-user");
        let (vault, _) = vault_address(&user.pubkey());
        ctx.airdrop(&user.pubkey(), 1_000_000_000)?;
        preload_program_owned_vault(ctx, vault)?;

        let metas = vec![
            AccountMeta::new(user.pubkey(), true),
            AccountMeta::new(vault, false),
        ];
        Ok(BenchInstruction::new(make_raw_ix_data(1, 1_000_000), metas)
            .with_signer(user))
    }
}
