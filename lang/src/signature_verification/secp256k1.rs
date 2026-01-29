use crate::error::ErrorCode;
use crate::prelude::*;
use crate::solana_program::instruction::Instruction;
use solana_instructions_sysvar::load_instruction_at_checked;
use solana_sdk_ids::secp256k1_program;
use solana_secp256k1_program::{
    SecpSignatureOffsets, HASHED_PUBKEY_SERIALIZED_SIZE, SIGNATURE_OFFSETS_SERIALIZED_SIZE,
    SIGNATURE_SERIALIZED_SIZE,
};

const SECP256K1_HEADER_SIZE: usize = 1; // num_signatures: u8

/// Verifies a Secp256k1 instruction created under the assumption that the
/// signature, address, and message bytes all live inside the same instruction
/// (i.e. the signature ix is placed at index `0`). Prefer
/// [`verify_secp256k1_ix_with_instruction_index`] and pass the actual signature
/// instruction index instead of relying on this default.
pub fn verify_secp256k1_ix(
    ix: &Instruction,
    eth_address: &[u8; 20],
    msg: &[u8],
    sig: &[u8; 64],
    recovery_id: u8,
) -> Result<()> {
    verify_secp256k1_ix_with_instruction_index(ix, None, eth_address, msg, sig, recovery_id)
}

/// Parses all signature offsets from a Secp256k1 instruction.
/// Returns the number of signatures and a vector of offset structures.
fn parse_secp256k1_signature_offsets(ix: &Instruction) -> Result<(u8, Vec<SecpSignatureOffsets>)> {
    require!(
        ix.data.len() >= SECP256K1_HEADER_SIZE,
        ErrorCode::SignatureVerificationFailed
    );

    let num_signatures = ix.data[0];
    require!(num_signatures > 0, ErrorCode::SignatureVerificationFailed);

    // Calculate minimum required size: header + (offsets per signature)
    let min_size = SECP256K1_HEADER_SIZE
        .checked_add(num_signatures as usize * SIGNATURE_OFFSETS_SERIALIZED_SIZE)
        .ok_or(ErrorCode::SignatureVerificationFailed)?;
    require!(
        ix.data.len() >= min_size,
        ErrorCode::SignatureVerificationFailed
    );

    let mut offsets = Vec::with_capacity(num_signatures as usize);
    let mut offset = SECP256K1_HEADER_SIZE;

    for _ in 0..num_signatures {
        require!(
            offset + SIGNATURE_OFFSETS_SERIALIZED_SIZE <= ix.data.len(),
            ErrorCode::SignatureVerificationFailed
        );

        // Manually parse the SDK struct from bytes
        let data_slice = &ix.data[offset..offset + SIGNATURE_OFFSETS_SERIALIZED_SIZE];
        let sig_offsets = SecpSignatureOffsets {
            signature_offset: u16::from_le_bytes([data_slice[0], data_slice[1]]),
            signature_instruction_index: data_slice[2],
            eth_address_offset: u16::from_le_bytes([data_slice[3], data_slice[4]]),
            eth_address_instruction_index: data_slice[5],
            message_data_offset: u16::from_le_bytes([data_slice[6], data_slice[7]]),
            message_data_size: u16::from_le_bytes([data_slice[8], data_slice[9]]),
            message_instruction_index: data_slice[10],
        };
        offsets.push(sig_offsets);

        offset += SIGNATURE_OFFSETS_SERIALIZED_SIZE;
    }

    Ok((num_signatures, offsets))
}

/// Verifies a Secp256k1 signature instruction by parsing the actual instruction data
/// to extract signature, Ethereum address, and message from their actual locations.
///
/// If `ix_sysvar` is provided, the function can load data from external instructions
/// referenced by the signature instruction. If `None`, it only works when all data
/// is embedded in the signature instruction itself (instruction_index == u8::MAX in the header).
///
/// This function verifies a single signature (the first one). For multiple signatures,
/// use [`verify_secp256k1_ix_multiple`].
pub fn verify_secp256k1_ix_with_instruction_index(
    ix: &Instruction,
    ix_sysvar: Option<&AccountInfo>,
    eth_address: &[u8; 20],
    msg: &[u8],
    sig: &[u8; 64],
    recovery_id: u8,
) -> Result<()> {
    require_keys_eq!(
        ix.program_id,
        secp256k1_program::id(),
        ErrorCode::Secp256k1InvalidProgram
    );
    require_eq!(ix.accounts.len(), 0usize, ErrorCode::InstructionHasAccounts);
    require!(recovery_id <= 1, ErrorCode::InvalidRecoveryId);

    let (num_signatures, offsets) = parse_secp256k1_signature_offsets(ix)?;
    require_eq!(num_signatures, 1u8, ErrorCode::SignatureVerificationFailed);

    let sig_info = &offsets[0];
    require_eq!(
        sig_info.message_data_size as usize,
        msg.len(),
        ErrorCode::SignatureVerificationFailed
    );

    verify_secp256k1_signature_at_index(
        ix,
        ix_sysvar,
        sig_info,
        eth_address,
        msg,
        sig,
        recovery_id,
        num_signatures,
    )
}

/// Verifies all Secp256k1 signatures in an instruction against provided arrays.
/// The arrays must have the same length as `num_signatures` in the instruction.
/// Each signature at index `i` will be verified against `eth_addresses[i]`, `msgs[i]`, `sigs[i]`, and `recovery_ids[i]`.
///
/// If `ix_sysvar` is provided, the function can load data from external instructions
/// referenced by the signature instruction. If `None`, it only works when all data
/// is embedded in the signature instruction itself (instruction_index == u8::MAX in the header).
pub fn verify_secp256k1_ix_multiple(
    ix: &Instruction,
    ix_sysvar: Option<&AccountInfo>,
    eth_addresses: &[[u8; 20]],
    msgs: &[&[u8]],
    sigs: &[[u8; 64]],
    recovery_ids: &[u8],
) -> Result<()> {
    require_keys_eq!(
        ix.program_id,
        secp256k1_program::id(),
        ErrorCode::Secp256k1InvalidProgram
    );
    require_eq!(ix.accounts.len(), 0usize, ErrorCode::InstructionHasAccounts);

    let (num_signatures, offsets) = parse_secp256k1_signature_offsets(ix)?;
    require_eq!(
        num_signatures as usize,
        eth_addresses.len(),
        ErrorCode::SignatureVerificationFailed
    );
    require_eq!(
        num_signatures as usize,
        msgs.len(),
        ErrorCode::SignatureVerificationFailed
    );
    require_eq!(
        num_signatures as usize,
        sigs.len(),
        ErrorCode::SignatureVerificationFailed
    );
    require_eq!(
        num_signatures as usize,
        recovery_ids.len(),
        ErrorCode::SignatureVerificationFailed
    );

    // Verify each signature
    for (i, sig_info) in offsets.iter().enumerate() {
        require!(recovery_ids[i] <= 1, ErrorCode::InvalidRecoveryId);
        require_eq!(
            sig_info.message_data_size as usize,
            msgs[i].len(),
            ErrorCode::SignatureVerificationFailed
        );
        verify_secp256k1_signature_at_index(
            ix,
            ix_sysvar,
            sig_info,
            &eth_addresses[i],
            msgs[i],
            &sigs[i],
            recovery_ids[i],
            num_signatures,
        )?;
    }

    Ok(())
}

/// Helper function to verify a single signature at a specific offset index.
#[allow(clippy::too_many_arguments)]
fn verify_secp256k1_signature_at_index(
    ix: &Instruction,
    ix_sysvar: Option<&AccountInfo>,
    sig_info: &SecpSignatureOffsets,
    eth_address: &[u8; 20],
    msg: &[u8],
    sig: &[u8; 64],
    recovery_id: u8,
    num_signatures: u8,
) -> Result<()> {
    require!(msg.len() <= u16::MAX as usize, ErrorCode::MessageTooLong);

    // Calculate minimum header size: header + (offset structures for all signatures)
    let min_header_size = SECP256K1_HEADER_SIZE
        .checked_add(num_signatures as usize * SIGNATURE_OFFSETS_SERIALIZED_SIZE)
        .ok_or(ErrorCode::SignatureVerificationFailed)?;

    // Validate offsets are reasonable (must be >= min_header_size to avoid reading header)
    require!(
        sig_info.signature_offset as usize >= min_header_size,
        ErrorCode::SignatureVerificationFailed
    );
    require!(
        sig_info.eth_address_offset as usize >= min_header_size,
        ErrorCode::SignatureVerificationFailed
    );
    require!(
        sig_info.message_data_offset as usize >= min_header_size,
        ErrorCode::SignatureVerificationFailed
    );

    // Helper to load data from an instruction
    let load_data = |offset: u16, ix_idx: u8, expected_len: usize| -> Result<Vec<u8>> {
        let end_offset = (offset as usize)
            .checked_add(expected_len)
            .ok_or(ErrorCode::SignatureVerificationFailed)?;
        if ix_idx == u8::MAX {
            require!(
                end_offset <= ix.data.len(),
                ErrorCode::SignatureVerificationFailed
            );
            Ok(ix.data[offset as usize..end_offset].to_vec())
        } else {
            // Data is in a different instruction - need sysvar
            let sysvar = ix_sysvar.ok_or(ErrorCode::SignatureVerificationFailed)?;
            let ref_ix = load_instruction_at_checked(ix_idx as usize, sysvar)
                .map_err(|_| ErrorCode::SignatureVerificationFailed)?;
            require!(
                end_offset <= ref_ix.data.len(),
                ErrorCode::SignatureVerificationFailed
            );
            Ok(ref_ix.data[offset as usize..end_offset].to_vec())
        }
    };

    // Load Ethereum address from its actual location
    let actual_eth_address = load_data(
        sig_info.eth_address_offset,
        sig_info.eth_address_instruction_index,
        HASHED_PUBKEY_SERIALIZED_SIZE,
    )?;
    if actual_eth_address.as_slice() != eth_address {
        return Err(ErrorCode::SignatureVerificationFailed.into());
    }

    // Load signature from its actual location
    let actual_sig = load_data(
        sig_info.signature_offset,
        sig_info.signature_instruction_index,
        SIGNATURE_SERIALIZED_SIZE,
    )?;
    if actual_sig.as_slice() != sig {
        return Err(ErrorCode::SignatureVerificationFailed.into());
    }

    // Load recovery id (it's right after the signature)
    // Check for overflow: signature_offset + 64 must not overflow u16
    let recovery_id_offset = sig_info
        .signature_offset
        .checked_add(SIGNATURE_SERIALIZED_SIZE as u16)
        .ok_or(ErrorCode::SignatureVerificationFailed)?;
    let actual_recovery_id = if sig_info.signature_instruction_index == u8::MAX {
        let offset_usize = recovery_id_offset as usize;
        require!(
            offset_usize < ix.data.len(),
            ErrorCode::SignatureVerificationFailed
        );
        ix.data[offset_usize]
    } else {
        let sysvar = ix_sysvar.ok_or(ErrorCode::SignatureVerificationFailed)?;
        let ref_ix =
            load_instruction_at_checked(sig_info.signature_instruction_index as usize, sysvar)
                .map_err(|_| ErrorCode::SignatureVerificationFailed)?;
        let offset_usize = recovery_id_offset as usize;
        require!(
            offset_usize < ref_ix.data.len(),
            ErrorCode::SignatureVerificationFailed
        );
        ref_ix.data[offset_usize]
    };
    if actual_recovery_id != recovery_id {
        return Err(ErrorCode::SignatureVerificationFailed.into());
    }

    // Load message from its actual location
    let actual_msg = load_data(
        sig_info.message_data_offset,
        sig_info.message_instruction_index,
        msg.len(),
    )?;
    if actual_msg.as_slice() != msg {
        return Err(ErrorCode::SignatureVerificationFailed.into());
    }

    Ok(())
}
