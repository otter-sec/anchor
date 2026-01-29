use anchor_lang::prelude::*;
use anchor_lang::signature_verification::{
    verify_ed25519_instruction_at_index, verify_ed25519_instruction_at_index_multiple,
    verify_secp256k1_instruction_at_index, verify_secp256k1_instruction_at_index_multiple,
};

declare_id!("9P8zSbNRQkwDrjCmqsHHcU1GTk5npaKYgKHroAkupbLG");

#[program]
pub mod signature_verification_test {
    use super::*;

    pub fn verify_ed25519_signature(
        ctx: Context<VerifyEd25519Signature>,
        message: Vec<u8>,
        signature: [u8; 64],
    ) -> Result<()> {
        verify_ed25519_instruction_at_index(
            &ctx.accounts.ix_sysvar,
            0,
            &ctx.accounts.signer.key().to_bytes(),
            &message,
            &signature,
        )?;

        msg!("Ed25519 signature verified successfully using custom helper!");
        Ok(())
    }

    pub fn verify_secp(
        ctx: Context<VerifySecp256k1Signature>,
        message: Vec<u8>,
        signature: [u8; 64],
        recovery_id: u8,
        eth_address: [u8; 20],
    ) -> Result<()> {
        verify_secp256k1_instruction_at_index(
            &ctx.accounts.ix_sysvar,
            0,
            &eth_address,
            &message,
            &signature,
            recovery_id,
        )?;

        msg!("Secp256k1 signature verified successfully using custom helper!");

        Ok(())
    }

    pub fn verify_ed25519_multiple(
        ctx: Context<VerifyEd25519Multiple>,
        pubkeys: Vec<[u8; 32]>,
        messages: Vec<Vec<u8>>,
        signatures: Vec<[u8; 64]>,
    ) -> Result<()> {
        // Convert messages to slice of slices
        let msg_slices: Vec<&[u8]> = messages.iter().map(|m| m.as_slice()).collect();
        
        verify_ed25519_instruction_at_index_multiple(
            &ctx.accounts.ix_sysvar,
            0,
            &pubkeys,
            &msg_slices,
            &signatures,
        )?;

        msg!("Multiple Ed25519 signatures verified successfully!");
        Ok(())
    }

    pub fn verify_secp_multiple(
        ctx: Context<VerifySecp256k1Multiple>,
        messages: Vec<Vec<u8>>,
        signatures: Vec<[u8; 64]>,
        recovery_ids: Vec<u8>,
        eth_addresses: Vec<[u8; 20]>,
    ) -> Result<()> {
        // Convert messages to slice of slices
        let msg_slices: Vec<&[u8]> = messages.iter().map(|m| m.as_slice()).collect();
        verify_secp256k1_instruction_at_index_multiple(
            &ctx.accounts.ix_sysvar,
            0,
            &eth_addresses,
            &msg_slices,
            &signatures,
            &recovery_ids,
        )?;

        msg!("Multiple Secp256k1 signatures verified successfully!");
        Ok(())
    }
}

#[derive(Accounts)]
pub struct VerifyEd25519Signature<'info> {
    /// CHECK: Signer account
    pub signer: AccountInfo<'info>,
    /// CHECK: Instructions sysvar account
    #[account(address = solana_sdk_ids::sysvar::instructions::ID)]
    pub ix_sysvar: AccountInfo<'info>,
}

#[derive(Accounts)]
pub struct VerifySecp256k1Signature<'info> {
    /// CHECK: Instructions sysvar account
    #[account(address = solana_sdk_ids::sysvar::instructions::ID)]
    pub ix_sysvar: AccountInfo<'info>,
}

#[derive(Accounts)]
pub struct VerifyEd25519Multiple<'info> {
    /// CHECK: Instructions sysvar account
    #[account(address = solana_sdk_ids::sysvar::instructions::ID)]
    pub ix_sysvar: AccountInfo<'info>,
}

#[derive(Accounts)]
pub struct VerifySecp256k1Multiple<'info> {
    /// CHECK: Instructions sysvar account
    #[account(address = solana_sdk_ids::sysvar::instructions::ID)]
    pub ix_sysvar: AccountInfo<'info>,
}
