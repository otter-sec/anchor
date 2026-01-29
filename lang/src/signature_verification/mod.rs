use crate::prelude::*;
use crate::solana_program::instruction::Instruction;
use solana_instructions_sysvar::{load_current_index_checked, load_instruction_at_checked};

mod ed25519;
mod secp256k1;

pub use ed25519::{
    verify_ed25519_ix, verify_ed25519_ix_multiple, verify_ed25519_ix_with_instruction_index,
};
pub use secp256k1::{
    verify_secp256k1_ix, verify_secp256k1_ix_multiple, verify_secp256k1_ix_with_instruction_index,
};

/// Load an instruction from the Instructions sysvar at the given index.
pub fn load_instruction(index: usize, ix_sysvar: &AccountInfo<'_>) -> Result<Instruction> {
    let ix = load_instruction_at_checked(index, ix_sysvar)
        .map_err(|_| error!(error::ErrorCode::ConstraintRaw))?;
    Ok(ix)
}

/// Verifies that an Ed25519 signature instruction exists at the given index in the transaction.
/// This is useful when you want to verify that a signature instruction was included in the transaction
/// at a specific position, rather than verifying the currently executing instruction.
pub fn verify_ed25519_instruction_at_index(
    ix_sysvar: &AccountInfo<'_>,
    instruction_index: usize,
    pubkey: &[u8; 32],
    msg: &[u8],
    sig: &[u8; 64],
) -> Result<()> {
    let ix = load_instruction(instruction_index, ix_sysvar)?;
    verify_ed25519_ix_with_instruction_index(&ix, Some(ix_sysvar), pubkey, msg, sig)
}

/// Loads the instruction currently executing in this transaction and verifies it
/// as an Ed25519 signature instruction.
pub fn verify_current_ed25519_instruction(
    ix_sysvar: &AccountInfo<'_>,
    pubkey: &[u8; 32],
    msg: &[u8],
    sig: &[u8; 64],
) -> Result<()> {
    let idx = load_current_index_checked(ix_sysvar)
        .map_err(|_| error!(error::ErrorCode::ConstraintRaw))?;
    verify_ed25519_instruction_at_index(ix_sysvar, idx as usize, pubkey, msg, sig)
}

/// Verifies that an Ed25519 signature instruction with multiple signatures exists at the given index in the transaction.
/// This is useful when you want to verify that a multiple-signature instruction was included in the transaction
/// at a specific position, rather than verifying the currently executing instruction.
pub fn verify_ed25519_instruction_at_index_multiple(
    ix_sysvar: &AccountInfo<'_>,
    instruction_index: usize,
    pubkeys: &[[u8; 32]],
    msgs: &[&[u8]],
    sigs: &[[u8; 64]],
) -> Result<()> {
    let ix = load_instruction(instruction_index, ix_sysvar)?;
    verify_ed25519_ix_multiple(&ix, Some(ix_sysvar), pubkeys, msgs, sigs)
}

/// Loads the instruction currently executing in this transaction and verifies it
/// as an Ed25519 signature instruction with multiple signatures.
pub fn verify_current_ed25519_instruction_multiple(
    ix_sysvar: &AccountInfo<'_>,
    pubkeys: &[[u8; 32]],
    msgs: &[&[u8]],
    sigs: &[[u8; 64]],
) -> Result<()> {
    let idx = load_current_index_checked(ix_sysvar)
        .map_err(|_| error!(error::ErrorCode::ConstraintRaw))?;
    verify_ed25519_instruction_at_index_multiple(ix_sysvar, idx as usize, pubkeys, msgs, sigs)
}

/// Verifies that a Secp256k1 signature instruction exists at the given index in the transaction.
/// This is useful when you want to verify that a signature instruction was included in the transaction
/// at a specific position, rather than verifying the currently executing instruction.
pub fn verify_secp256k1_instruction_at_index(
    ix_sysvar: &AccountInfo<'_>,
    instruction_index: usize,
    eth_address: &[u8; 20],
    msg: &[u8],
    sig: &[u8; 64],
    recovery_id: u8,
) -> Result<()> {
    let ix = load_instruction(instruction_index, ix_sysvar)?;
    verify_secp256k1_ix_with_instruction_index(
        &ix,
        Some(ix_sysvar),
        eth_address,
        msg,
        sig,
        recovery_id,
    )
}

/// Loads the instruction currently executing in this transaction and verifies it
/// as a Secp256k1 signature instruction.
pub fn verify_current_secp256k1_instruction(
    ix_sysvar: &AccountInfo<'_>,
    eth_address: &[u8; 20],
    msg: &[u8],
    sig: &[u8; 64],
    recovery_id: u8,
) -> Result<()> {
    let idx_u16 = load_current_index_checked(ix_sysvar)
        .map_err(|_| error!(error::ErrorCode::ConstraintRaw))?;
    verify_secp256k1_instruction_at_index(
        ix_sysvar,
        idx_u16 as usize,
        eth_address,
        msg,
        sig,
        recovery_id,
    )
}

/// Verifies that a Secp256k1 signature instruction with multiple signatures exists at the given index in the transaction.
/// This is useful when you want to verify that a multiple-signature instruction was included in the transaction
/// at a specific position, rather than verifying the currently executing instruction.
pub fn verify_secp256k1_instruction_at_index_multiple(
    ix_sysvar: &AccountInfo<'_>,
    instruction_index: usize,
    eth_addresses: &[[u8; 20]],
    msgs: &[&[u8]],
    sigs: &[[u8; 64]],
    recovery_ids: &[u8],
) -> Result<()> {
    let ix = load_instruction(instruction_index, ix_sysvar)?;
    verify_secp256k1_ix_multiple(
        &ix,
        Some(ix_sysvar),
        eth_addresses,
        msgs,
        sigs,
        recovery_ids,
    )
}

/// Loads the instruction currently executing in this transaction and verifies it
/// as a Secp256k1 signature instruction with multiple signatures.
pub fn verify_current_secp256k1_instruction_multiple(
    ix_sysvar: &AccountInfo<'_>,
    eth_addresses: &[[u8; 20]],
    msgs: &[&[u8]],
    sigs: &[[u8; 64]],
    recovery_ids: &[u8],
) -> Result<()> {
    let idx_u16 = load_current_index_checked(ix_sysvar)
        .map_err(|_| error!(error::ErrorCode::ConstraintRaw))?;
    verify_secp256k1_instruction_at_index_multiple(
        ix_sysvar,
        idx_u16 as usize,
        eth_addresses,
        msgs,
        sigs,
        recovery_ids,
    )
}
