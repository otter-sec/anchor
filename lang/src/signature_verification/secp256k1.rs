use crate::error::ErrorCode;
use crate::prelude::*;
use crate::solana_program::instruction::Instruction;
use solana_instructions_sysvar::load_instruction_at_checked;
use solana_sdk_ids::secp256k1_program;

const SECP256K1_HEADER_SIZE: usize = 1; // num_signatures: u8
const SECP256K1_SIGNATURE_OFFSET_SIZE: usize = 11; // 7 fields per signature (mixed u16/u8)
const SECP256K1_ETH_ADDRESS_SIZE: usize = 20;
const SECP256K1_SIGNATURE_SIZE: usize = 64;

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

/// Structure representing a single signature's offset information
#[derive(Debug, Clone)]
struct Secp256k1SignatureOffsets {
    sig_offset: u16,
    sig_ix_idx: u8,
    eth_offset: u16,
    eth_ix_idx: u8,
    msg_offset: u16,
    msg_len: u16,
    msg_ix_idx: u8,
}

/// Parses all signature offsets from a Secp256k1 instruction.
/// Returns the number of signatures and a vector of offset structures.
fn parse_secp256k1_signature_offsets(
    ix: &Instruction,
) -> Result<(u8, Vec<Secp256k1SignatureOffsets>)> {
    require!(
        ix.data.len() >= SECP256K1_HEADER_SIZE,
        ErrorCode::SignatureVerificationFailed
    );

    let num_signatures = ix.data[0];
    require!(num_signatures > 0, ErrorCode::SignatureVerificationFailed);

    // Calculate minimum required size: header + (offsets per signature)
    let min_size = SECP256K1_HEADER_SIZE
        .checked_add(num_signatures as usize * SECP256K1_SIGNATURE_OFFSET_SIZE)
        .ok_or(ErrorCode::SignatureVerificationFailed)?;
    require!(
        ix.data.len() >= min_size,
        ErrorCode::SignatureVerificationFailed
    );

    let mut offsets = Vec::with_capacity(num_signatures as usize);
    let mut offset = SECP256K1_HEADER_SIZE;

    for _ in 0..num_signatures {
        require!(
            offset + SECP256K1_SIGNATURE_OFFSET_SIZE <= ix.data.len(),
            ErrorCode::SignatureVerificationFailed
        );

        let sig_offset = u16::from_le_bytes([ix.data[offset], ix.data[offset + 1]]);
        let sig_ix_idx = ix.data[offset + 2];
        let eth_offset = u16::from_le_bytes([ix.data[offset + 3], ix.data[offset + 4]]);
        let eth_ix_idx = ix.data[offset + 5];
        let msg_offset = u16::from_le_bytes([ix.data[offset + 6], ix.data[offset + 7]]);
        let msg_len = u16::from_le_bytes([ix.data[offset + 8], ix.data[offset + 9]]);
        let msg_ix_idx = ix.data[offset + 10];

        offsets.push(Secp256k1SignatureOffsets {
            sig_offset,
            sig_ix_idx,
            eth_offset,
            eth_ix_idx,
            msg_offset,
            msg_len,
            msg_ix_idx,
        });

        offset += SECP256K1_SIGNATURE_OFFSET_SIZE;
    }

    Ok((num_signatures, offsets))
}

/// Verifies a Secp256k1 signature instruction by parsing the actual instruction data
/// to extract signature, Ethereum address, and message from their actual locations.
///
/// The `instruction_index` parameter is deprecated and ignored. The function now
/// parses the instruction data header to determine where each piece of data is located.
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
        sig_info.msg_len as usize,
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
            sig_info.msg_len as usize,
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
    sig_info: &Secp256k1SignatureOffsets,
    eth_address: &[u8; 20],
    msg: &[u8],
    sig: &[u8; 64],
    recovery_id: u8,
    num_signatures: u8,
) -> Result<()> {
    require!(msg.len() <= u16::MAX as usize, ErrorCode::MessageTooLong);

    // Calculate minimum header size: header + (offset structures for all signatures)
    let min_header_size = SECP256K1_HEADER_SIZE
        .checked_add(num_signatures as usize * SECP256K1_SIGNATURE_OFFSET_SIZE)
        .ok_or(ErrorCode::SignatureVerificationFailed)?;

    // Validate offsets are reasonable (must be >= min_header_size to avoid reading header)
    require!(
        sig_info.sig_offset as usize >= min_header_size,
        ErrorCode::SignatureVerificationFailed
    );
    require!(
        sig_info.eth_offset as usize >= min_header_size,
        ErrorCode::SignatureVerificationFailed
    );
    require!(
        sig_info.msg_offset as usize >= min_header_size,
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
        sig_info.eth_offset,
        sig_info.eth_ix_idx,
        SECP256K1_ETH_ADDRESS_SIZE,
    )?;
    if actual_eth_address.as_slice() != eth_address {
        return Err(ErrorCode::SignatureVerificationFailed.into());
    }

    // Load signature from its actual location
    let actual_sig = load_data(
        sig_info.sig_offset,
        sig_info.sig_ix_idx,
        SECP256K1_SIGNATURE_SIZE,
    )?;
    if actual_sig.as_slice() != sig {
        return Err(ErrorCode::SignatureVerificationFailed.into());
    }

    // Load recovery id (it's right after the signature)
    // Check for overflow: sig_offset + 64 must not overflow u16
    let recovery_id_offset = sig_info
        .sig_offset
        .checked_add(SECP256K1_SIGNATURE_SIZE as u16)
        .ok_or(ErrorCode::SignatureVerificationFailed)?;
    let actual_recovery_id = if sig_info.sig_ix_idx == u8::MAX {
        let offset_usize = recovery_id_offset as usize;
        require!(
            offset_usize < ix.data.len(),
            ErrorCode::SignatureVerificationFailed
        );
        ix.data[offset_usize]
    } else {
        let sysvar = ix_sysvar.ok_or(ErrorCode::SignatureVerificationFailed)?;
        let ref_ix = load_instruction_at_checked(sig_info.sig_ix_idx as usize, sysvar)
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
    let actual_msg = load_data(sig_info.msg_offset, sig_info.msg_ix_idx, msg.len())?;
    if actual_msg.as_slice() != msg {
        return Err(ErrorCode::SignatureVerificationFailed.into());
    }

    Ok(())
}
