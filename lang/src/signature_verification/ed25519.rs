use {
    crate::{error::ErrorCode, prelude::*, solana_program::instruction::Instruction},
    bytemuck::pod_read_unaligned,
    solana_ed25519_program::{
        Ed25519SignatureOffsets, PUBKEY_SERIALIZED_SIZE, SIGNATURE_OFFSETS_SERIALIZED_SIZE,
        SIGNATURE_OFFSETS_START, SIGNATURE_SERIALIZED_SIZE,
    },
    solana_instructions_sysvar::load_instruction_at_checked,
    solana_sdk_ids::ed25519_program,
};

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

/// Parses all signature offsets from an Ed25519 instruction.
/// Returns the number of signatures and a vector of offset structures.
fn parse_ed25519_signature_offsets(ix: &Instruction) -> Result<(u8, Vec<Ed25519SignatureOffsets>)> {
    require!(
        ix.data.len() >= SIGNATURE_OFFSETS_START,
        ErrorCode::SignatureVerificationFailed
    );

    let num_signatures = ix.data[0];
    require!(num_signatures > 0, ErrorCode::SignatureVerificationFailed);

    // Calculate minimum required size: header + (offsets per signature)
    let min_size = SIGNATURE_OFFSETS_START
        .checked_add(num_signatures as usize * SIGNATURE_OFFSETS_SERIALIZED_SIZE)
        .ok_or(ErrorCode::SignatureVerificationFailed)?;
    require!(
        ix.data.len() >= min_size,
        ErrorCode::SignatureVerificationFailed
    );

    let mut offsets = Vec::with_capacity(num_signatures as usize);
    let mut offset = SIGNATURE_OFFSETS_START;

    for _ in 0..num_signatures {
        require!(
            offset + SIGNATURE_OFFSETS_SERIALIZED_SIZE <= ix.data.len(),
            ErrorCode::SignatureVerificationFailed
        );

        // Use bytemuck to parse the struct from bytes
        let sig_offsets = pod_read_unaligned::<Ed25519SignatureOffsets>(
            &ix.data[offset..offset + SIGNATURE_OFFSETS_SERIALIZED_SIZE],
        );
        offsets.push(sig_offsets);

        offset += SIGNATURE_OFFSETS_SERIALIZED_SIZE;
    }

    Ok((num_signatures, offsets))
}

/// Verifies an Ed25519 signature instruction by parsing the actual instruction data
/// to extract signature, public key, and message from their actual locations.
/// Supports both formats: [Signature, Pubkey] and [Pubkey, Signature].
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
    require_keys_eq!(
        ix.program_id,
        ed25519_program::id(),
        ErrorCode::Ed25519InvalidProgram
    );
    require_eq!(ix.accounts.len(), 0usize, ErrorCode::InstructionHasAccounts);
    require!(msg.len() <= u16::MAX as usize, ErrorCode::MessageTooLong);

    let (num_signatures, offsets) = parse_ed25519_signature_offsets(ix)?;
    require_eq!(num_signatures, 1u8, ErrorCode::SignatureVerificationFailed);

    let sig_info = &offsets[0];
    require_eq!(
        sig_info.message_data_size as usize,
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
    require!(
        msgs.iter().all(|msg| msg.len() <= u16::MAX as usize),
        ErrorCode::MessageTooLong
    );

    // Verify each signature
    for (i, sig_info) in offsets.iter().enumerate() {
        require_eq!(
            sig_info.message_data_size as usize,
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
    let min_header_size = SIGNATURE_OFFSETS_START
        .checked_add(num_signatures as usize * SIGNATURE_OFFSETS_SERIALIZED_SIZE)
        .ok_or(ErrorCode::SignatureVerificationFailed)?;

    // Validate offsets are reasonable (must be >= min_header_size to avoid reading header)
    require!(
        sig_info.signature_offset as usize >= min_header_size,
        ErrorCode::SignatureVerificationFailed
    );
    require!(
        sig_info.public_key_offset as usize >= min_header_size,
        ErrorCode::SignatureVerificationFailed
    );
    require!(
        sig_info.message_data_offset as usize >= min_header_size,
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
        sig_info.signature_offset,
        sig_info.signature_instruction_index,
        SIGNATURE_SERIALIZED_SIZE,
    )?;
    if actual_sig.as_slice() != sig {
        return Err(ErrorCode::SignatureVerificationFailed.into());
    }

    // Load pubkey from its actual location
    let actual_pubkey = load_data(
        sig_info.public_key_offset,
        sig_info.public_key_instruction_index,
        PUBKEY_SERIALIZED_SIZE,
    )?;
    if actual_pubkey.as_slice() != pubkey {
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

#[cfg(test)]
mod tests {
    use {
        super::*,
        crate::{error::Error, solana_program::pubkey::Pubkey},
    };

    fn build_single_instruction(pubkey: [u8; 32], msg: &[u8], sig: [u8; 64]) -> Instruction {
        let header_size = SIGNATURE_OFFSETS_START + SIGNATURE_OFFSETS_SERIALIZED_SIZE;
        let sig_offset = header_size as u16;
        let pubkey_offset = sig_offset + SIGNATURE_SERIALIZED_SIZE as u16;
        let msg_offset = pubkey_offset + PUBKEY_SERIALIZED_SIZE as u16;

        let mut data = Vec::with_capacity(header_size + sig.len() + pubkey.len() + msg.len());
        data.push(1);
        data.push(0);
        data.extend_from_slice(&sig_offset.to_le_bytes());
        data.extend_from_slice(&u16::MAX.to_le_bytes());
        data.extend_from_slice(&pubkey_offset.to_le_bytes());
        data.extend_from_slice(&u16::MAX.to_le_bytes());
        data.extend_from_slice(&msg_offset.to_le_bytes());
        data.extend_from_slice(&(msg.len() as u16).to_le_bytes());
        data.extend_from_slice(&u16::MAX.to_le_bytes());
        data.extend_from_slice(&sig);
        data.extend_from_slice(&pubkey);
        data.extend_from_slice(msg);

        Instruction {
            program_id: ed25519_program::id(),
            accounts: vec![],
            data,
        }
    }

    fn build_multiple_instruction(
        pubkeys: &[[u8; 32]],
        msgs: &[&[u8]],
        sigs: &[[u8; 64]],
    ) -> Instruction {
        let num_signatures = pubkeys.len();
        let header_size =
            SIGNATURE_OFFSETS_START + num_signatures * SIGNATURE_OFFSETS_SERIALIZED_SIZE;
        let mut data = Vec::with_capacity(
            header_size
                + sigs.iter().map(|sig| sig.len()).sum::<usize>()
                + pubkeys.iter().map(|pubkey| pubkey.len()).sum::<usize>()
                + msgs.iter().map(|msg| msg.len()).sum::<usize>(),
        );

        data.push(num_signatures as u8);
        data.push(0);

        let mut cursor = header_size as u16;
        for ((pubkey, msg), _sig) in pubkeys.iter().zip(msgs.iter()).zip(sigs.iter()) {
            let sig_offset = cursor;
            cursor += SIGNATURE_SERIALIZED_SIZE as u16;

            let pubkey_offset = cursor;
            cursor += PUBKEY_SERIALIZED_SIZE as u16;

            let msg_offset = cursor;
            cursor += msg.len() as u16;

            data.extend_from_slice(&sig_offset.to_le_bytes());
            data.extend_from_slice(&u16::MAX.to_le_bytes());
            data.extend_from_slice(&pubkey_offset.to_le_bytes());
            data.extend_from_slice(&u16::MAX.to_le_bytes());
            data.extend_from_slice(&msg_offset.to_le_bytes());
            data.extend_from_slice(&(msg.len() as u16).to_le_bytes());
            data.extend_from_slice(&u16::MAX.to_le_bytes());

            let _ = pubkey;
        }

        for ((pubkey, msg), sig) in pubkeys.iter().zip(msgs.iter()).zip(sigs.iter()) {
            data.extend_from_slice(sig);
            data.extend_from_slice(pubkey);
            data.extend_from_slice(msg);
        }

        Instruction {
            program_id: ed25519_program::id(),
            accounts: vec![],
            data,
        }
    }

    #[test]
    fn verifies_single_embedded_instruction() {
        let pubkey = [7u8; 32];
        let sig = [9u8; 64];
        let msg = b"verify me";
        let ix = build_single_instruction(pubkey, msg, sig);

        verify_ed25519_ix(&ix, &pubkey, msg, &sig).unwrap();
    }

    #[test]
    fn rejects_non_ed25519_program() {
        let pubkey = [7u8; 32];
        let sig = [9u8; 64];
        let msg = b"verify me";
        let mut ix = build_single_instruction(pubkey, msg, sig);
        ix.program_id = Pubkey::new_from_array([42u8; 32]);

        let err =
            verify_ed25519_ix_with_instruction_index(&ix, None, &pubkey, msg, &sig).unwrap_err();
        let error_code = match err {
            Error::AnchorError(error) => error.error_code_number,
            Error::ProgramError(error) => panic!("unexpected program error: {error:?}"),
        };

        assert_eq!(error_code, ErrorCode::Ed25519InvalidProgram.into());
    }

    #[test]
    fn verifies_multiple_embedded_signatures() {
        let pubkeys = [[1u8; 32], [2u8; 32]];
        let sigs = [[3u8; 64], [4u8; 64]];
        let msg1 = b"first message".as_slice();
        let msg2 = b"second message".as_slice();
        let msgs = [msg1, msg2];
        let ix = build_multiple_instruction(&pubkeys, &msgs, &sigs);

        verify_ed25519_ix_multiple(&ix, None, &pubkeys, &msgs, &sigs).unwrap();
    }
}
