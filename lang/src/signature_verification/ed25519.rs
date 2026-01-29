use crate::error::ErrorCode;
use crate::prelude::*;
use crate::solana_program::instruction::Instruction;
use solana_instructions_sysvar::load_instruction_at_checked;
use solana_sdk_ids::ed25519_program;

const ED25519_HEADER_SIZE: usize = 2; // num_signatures: u8, padding: u8
const ED25519_SIGNATURE_OFFSET_SIZE: usize = 14; // 7 u16 fields per signature
const ED25519_SIGNATURE_SIZE: usize = 64;
const ED25519_PUBKEY_SIZE: usize = 32;

/// Verifies an Ed25519 signature instruction assuming the signature, public key,
/// and message bytes are embedded directly inside the instruction data (Solana's
/// default encoding). Prefer [`verify_ed25519_ix_with_instruction_index`] when
/// working with custom instructions that point at external instruction data.
pub fn verify_ed25519_ix(
    ix: &Instruction,
    pubkey: &[u8; 32],
    msg: &[u8],
    sig: &[u8; 64],
) -> Result<()> {
    verify_ed25519_ix_with_instruction_index(ix, None, pubkey, msg, sig)
}

/// Structure representing a single signature's offset information
#[derive(Debug, Clone)]
struct Ed25519SignatureOffsets {
    sig_offset: u16,
    sig_ix_idx: u16,
    pubkey_offset: u16,
    pubkey_ix_idx: u16,
    msg_offset: u16,
    msg_len: u16,
    msg_ix_idx: u16,
}

/// Parses all signature offsets from an Ed25519 instruction.
/// Returns the number of signatures and a vector of offset structures.
fn parse_ed25519_signature_offsets(ix: &Instruction) -> Result<(u8, Vec<Ed25519SignatureOffsets>)> {
    require!(
        ix.data.len() >= ED25519_HEADER_SIZE,
        ErrorCode::SignatureVerificationFailed
    );

    let num_signatures = ix.data[0];
    require!(num_signatures > 0, ErrorCode::SignatureVerificationFailed);

    // Calculate minimum required size: header + (offsets per signature)
    let min_size = ED25519_HEADER_SIZE
        .checked_add(num_signatures as usize * ED25519_SIGNATURE_OFFSET_SIZE)
        .ok_or(ErrorCode::SignatureVerificationFailed)?;
    require!(
        ix.data.len() >= min_size,
        ErrorCode::SignatureVerificationFailed
    );

    let mut offsets = Vec::with_capacity(num_signatures as usize);
    let mut offset = ED25519_HEADER_SIZE;

    for _ in 0..num_signatures {
        require!(
            offset + ED25519_SIGNATURE_OFFSET_SIZE <= ix.data.len(),
            ErrorCode::SignatureVerificationFailed
        );

        let sig_offset = u16::from_le_bytes([ix.data[offset], ix.data[offset + 1]]);
        let sig_ix_idx = u16::from_le_bytes([ix.data[offset + 2], ix.data[offset + 3]]);
        let pubkey_offset = u16::from_le_bytes([ix.data[offset + 4], ix.data[offset + 5]]);
        let pubkey_ix_idx = u16::from_le_bytes([ix.data[offset + 6], ix.data[offset + 7]]);
        let msg_offset = u16::from_le_bytes([ix.data[offset + 8], ix.data[offset + 9]]);
        let msg_len = u16::from_le_bytes([ix.data[offset + 10], ix.data[offset + 11]]);
        let msg_ix_idx = u16::from_le_bytes([ix.data[offset + 12], ix.data[offset + 13]]);

        offsets.push(Ed25519SignatureOffsets {
            sig_offset,
            sig_ix_idx,
            pubkey_offset,
            pubkey_ix_idx,
            msg_offset,
            msg_len,
            msg_ix_idx,
        });

        offset += ED25519_SIGNATURE_OFFSET_SIZE;
    }

    Ok((num_signatures, offsets))
}

/// Verifies an Ed25519 signature instruction by parsing the actual instruction data
/// to extract signature, public key, and message from their actual locations.
/// Supports both formats: [Signature, Pubkey] and [Pubkey, Signature].
///
/// The `instruction_index` parameter is deprecated and ignored. The function now
/// parses the instruction data header to determine where each piece of data is located.
///
/// If `ix_sysvar` is provided, the function can load data from external instructions
/// referenced by the signature instruction. If `None`, it only works when all data
/// is embedded in the signature instruction itself (instruction_index == u16::MAX in the header).
///
/// This function verifies a single signature (the first one). For multiple signatures,
/// use [`verify_ed25519_ix_multiple`].
pub fn verify_ed25519_ix_with_instruction_index(
    ix: &Instruction,
    ix_sysvar: Option<&AccountInfo>,
    pubkey: &[u8; 32],
    msg: &[u8],
    sig: &[u8; 64],
) -> Result<()> {
    let (num_signatures, offsets) = parse_ed25519_signature_offsets(ix)?;
    require_eq!(num_signatures, 1u8, ErrorCode::SignatureVerificationFailed);

    let sig_info = &offsets[0];
    require_eq!(
        sig_info.msg_len as usize,
        msg.len(),
        ErrorCode::SignatureVerificationFailed
    );

    verify_ed25519_signature_at_index(ix, ix_sysvar, sig_info, pubkey, msg, sig, num_signatures)
}

/// Verifies all Ed25519 signatures in an instruction against provided arrays.
/// The arrays must have the same length as `num_signatures` in the instruction.
/// Each signature at index `i` will be verified against `pubkeys[i]`, `msgs[i]`, and `sigs[i]`.
///
/// If `ix_sysvar` is provided, the function can load data from external instructions
/// referenced by the signature instruction. If `None`, it only works when all data
/// is embedded in the signature instruction itself (instruction_index == u16::MAX in the header).
pub fn verify_ed25519_ix_multiple(
    ix: &Instruction,
    ix_sysvar: Option<&AccountInfo>,
    pubkeys: &[[u8; 32]],
    msgs: &[&[u8]],
    sigs: &[[u8; 64]],
) -> Result<()> {
    require_keys_eq!(
        ix.program_id,
        ed25519_program::id(),
        ErrorCode::Ed25519InvalidProgram
    );
    require_eq!(ix.accounts.len(), 0usize, ErrorCode::InstructionHasAccounts);

    let (num_signatures, offsets) = parse_ed25519_signature_offsets(ix)?;
    require_eq!(
        num_signatures as usize,
        pubkeys.len(),
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

    // Verify each signature
    for (i, sig_info) in offsets.iter().enumerate() {
        require_eq!(
            sig_info.msg_len as usize,
            msgs[i].len(),
            ErrorCode::SignatureVerificationFailed
        );
        verify_ed25519_signature_at_index(
            ix,
            ix_sysvar,
            sig_info,
            &pubkeys[i],
            msgs[i],
            &sigs[i],
            num_signatures,
        )?;
    }

    Ok(())
}

/// Helper function to verify a single signature at a specific offset index.
fn verify_ed25519_signature_at_index(
    ix: &Instruction,
    ix_sysvar: Option<&AccountInfo>,
    sig_info: &Ed25519SignatureOffsets,
    pubkey: &[u8; 32],
    msg: &[u8],
    sig: &[u8; 64],
    num_signatures: u8,
) -> Result<()> {
    // Calculate minimum header size: header + (offset structures for all signatures)
    let min_header_size = ED25519_HEADER_SIZE
        .checked_add(num_signatures as usize * ED25519_SIGNATURE_OFFSET_SIZE)
        .ok_or(ErrorCode::SignatureVerificationFailed)?;

    // Validate offsets are reasonable (must be >= min_header_size to avoid reading header)
    require!(
        sig_info.sig_offset as usize >= min_header_size,
        ErrorCode::SignatureVerificationFailed
    );
    require!(
        sig_info.pubkey_offset as usize >= min_header_size,
        ErrorCode::SignatureVerificationFailed
    );
    require!(
        sig_info.msg_offset as usize >= min_header_size,
        ErrorCode::SignatureVerificationFailed
    );

    // Helper to load data from an instruction
    let load_data = |offset: u16, ix_idx: u16, expected_len: usize| -> Result<Vec<u8>> {
        let end_offset = (offset as usize)
            .checked_add(expected_len)
            .ok_or(ErrorCode::SignatureVerificationFailed)?;
        if ix_idx == u16::MAX {
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

    // Load signature from its actual location
    let actual_sig = load_data(
        sig_info.sig_offset,
        sig_info.sig_ix_idx,
        ED25519_SIGNATURE_SIZE,
    )?;
    if actual_sig.as_slice() != sig {
        return Err(ErrorCode::SignatureVerificationFailed.into());
    }

    // Load pubkey from its actual location
    let actual_pubkey = load_data(
        sig_info.pubkey_offset,
        sig_info.pubkey_ix_idx,
        ED25519_PUBKEY_SIZE,
    )?;
    if actual_pubkey.as_slice() != pubkey {
        return Err(ErrorCode::SignatureVerificationFailed.into());
    }

    // Load message from its actual location
    let actual_msg = load_data(sig_info.msg_offset, sig_info.msg_ix_idx, msg.len())?;
    if actual_msg.as_slice() != msg {
        return Err(ErrorCode::SignatureVerificationFailed.into());
    }

    Ok(())
}
