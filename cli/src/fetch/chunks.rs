use {
    super::{rpc, ChunkData, FetchTuning},
    crate::compat::{solana_pubkey, solana_rpc_client, solana_transaction_status_client_types},
    anyhow::{anyhow, Result},
    solana_pubkey::Pubkey,
    solana_rpc_client::rpc_client::RpcClient,
    solana_signature::Signature,
    solana_transaction_status_client_types::{
        EncodedConfirmedTransactionWithStatusMeta, EncodedTransaction,
        EncodedTransactionWithStatusMeta, UiCompiledInstruction, UiInstruction, UiMessage,
        UiTransaction,
    },
    std::str::FromStr,
};

const IDL_IX_TAG: [u8; 8] = [0x40, 0xf4, 0xbc, 0x78, 0xa7, 0xe9, 0x69, 0x0a];
const WRITE_VARIANT: u8 = 0x02;
const IDL_HEADER_SIZE: usize = 13;

// Fetches a transaction and extracts any IDL write payload chunks from its instructions.
pub(super) fn extract_chunks_from_transaction(
    client: &RpcClient,
    signature: &Signature,
    tuning: &FetchTuning,
    expected_program_id: &Pubkey,
    expected_idl_address: &Pubkey,
) -> Result<Vec<ChunkData>> {
    let transaction = rpc::fetch_transaction(client, signature, tuning)?;
    let ui_tx = parse_transaction_data(transaction)?;
    extract_chunks_from_message(ui_tx.message, expected_program_id, expected_idl_address)
}

// Narrows the RPC response to the JSON transaction format used by the chunk parser.
fn parse_transaction_data(
    transaction: EncodedConfirmedTransactionWithStatusMeta,
) -> Result<UiTransaction> {
    match transaction.transaction {
        EncodedTransactionWithStatusMeta {
            transaction: EncodedTransaction::Json(ui_tx),
            ..
        } => Ok(ui_tx),
        _ => Err(anyhow!("Invalid transaction format")),
    }
}

// Dispatches chunk extraction across parsed and raw transaction message formats.
fn extract_chunks_from_message(
    message: UiMessage,
    expected_program_id: &Pubkey,
    expected_idl_address: &Pubkey,
) -> Result<Vec<ChunkData>> {
    let chunks = match message {
        UiMessage::Parsed(parsed_msg) => {
            let account_keys = parsed_msg
                .account_keys
                .iter()
                .map(|account| Pubkey::from_str(&account.pubkey))
                .collect::<std::result::Result<Vec<_>, _>>()?;
            extract_from_parsed_instructions(
                &parsed_msg.instructions,
                &account_keys,
                expected_program_id,
                expected_idl_address,
            )?
        }
        UiMessage::Raw(raw_msg) => {
            let account_keys = raw_msg
                .account_keys
                .iter()
                .map(|account| Pubkey::from_str(account))
                .collect::<std::result::Result<Vec<_>, _>>()?;
            extract_from_raw_instructions(
                &raw_msg.instructions,
                &account_keys,
                expected_program_id,
                expected_idl_address,
            )?
        }
    };
    Ok(chunks)
}

// Scans parsed instructions for compiled IDL write payloads.
fn extract_from_parsed_instructions(
    instructions: &[UiInstruction],
    account_keys: &[Pubkey],
    expected_program_id: &Pubkey,
    expected_idl_address: &Pubkey,
) -> Result<Vec<ChunkData>> {
    let chunks = instructions
        .iter()
        .filter_map(|instruction| {
            if let UiInstruction::Compiled(UiCompiledInstruction {
                program_id_index,
                accounts,
                data,
                ..
            }) = instruction
            {
                extract_compressed_chunk(
                    data,
                    *program_id_index,
                    accounts,
                    account_keys,
                    expected_program_id,
                    expected_idl_address,
                )
                .ok()
                .flatten()
            } else {
                None
            }
        })
        .collect();
    Ok(chunks)
}

// Scans raw compiled instructions for IDL write payloads.
fn extract_from_raw_instructions(
    instructions: &[UiCompiledInstruction],
    account_keys: &[Pubkey],
    expected_program_id: &Pubkey,
    expected_idl_address: &Pubkey,
) -> Result<Vec<ChunkData>> {
    let chunks = instructions
        .iter()
        .filter_map(|instruction| {
            extract_compressed_chunk(
                &instruction.data,
                instruction.program_id_index,
                &instruction.accounts,
                account_keys,
                expected_program_id,
                expected_idl_address,
            )
            .ok()
            .flatten()
        })
        .collect();
    Ok(chunks)
}

// Decodes one instruction's base58 data and returns its compressed IDL payload when present.
fn extract_compressed_chunk(
    data_str: &str,
    program_id_index: u8,
    account_indexes: &[u8],
    account_keys: &[Pubkey],
    expected_program_id: &Pubkey,
    expected_idl_address: &Pubkey,
) -> Result<Option<ChunkData>> {
    let Some(first_account_index) = account_indexes.first() else {
        return Ok(None);
    };
    if account_keys.get(program_id_index as usize) != Some(expected_program_id)
        || account_keys.get(*first_account_index as usize) != Some(expected_idl_address)
    {
        return Ok(None);
    }

    let data = bs58::decode(data_str).into_vec()?;

    if !is_valid_idl_write_instruction(&data) {
        return Ok(None);
    }

    let vec_len = extract_payload_length(&data);

    if !has_complete_payload(&data, vec_len) {
        return Ok(None);
    }

    Ok(Some(
        data[IDL_HEADER_SIZE..IDL_HEADER_SIZE + vec_len].to_vec(),
    ))
}

// Checks whether the instruction data matches Anchor's IDL write discriminator and variant.
fn is_valid_idl_write_instruction(data: &[u8]) -> bool {
    if data.len() < IDL_HEADER_SIZE {
        return false;
    }

    if data[0..8] != IDL_IX_TAG {
        return false;
    }

    if data[8] != WRITE_VARIANT {
        return false;
    }

    true
}

// Reads the payload length field from a validated IDL write instruction header.
fn extract_payload_length(data: &[u8]) -> usize {
    u32::from_le_bytes([data[9], data[10], data[11], data[12]]) as usize
}

// Ensures the instruction buffer actually contains the full declared payload bytes.
fn has_complete_payload(data: &[u8], payload_len: usize) -> bool {
    data.len() >= IDL_HEADER_SIZE + payload_len
}

#[cfg(test)]
mod tests {
    use super::*;

    fn instruction(
        data: Vec<u8>,
        program_id_index: u8,
        accounts: Vec<u8>,
    ) -> UiCompiledInstruction {
        UiCompiledInstruction {
            program_id_index,
            accounts,
            data: bs58::encode(data).into_string(),
            stack_height: None,
        }
    }

    fn write_data(payload: &[u8]) -> Vec<u8> {
        let mut data = IDL_IX_TAG.to_vec();
        data.push(WRITE_VARIANT);
        data.extend_from_slice(&(payload.len() as u32).to_le_bytes());
        data.extend_from_slice(payload);
        data
    }

    #[test]
    fn rejects_write_data_with_wrong_program_or_account() {
        let target_program = Pubkey::new_unique();
        let unrelated_program = Pubkey::new_unique();
        let idl_address = Pubkey::new_unique();

        let wrong_program_chunks = extract_from_raw_instructions(
            &[instruction(write_data(b"{}"), 0, vec![1])],
            &[unrelated_program, idl_address],
            &target_program,
            &idl_address,
        )
        .unwrap();
        assert!(wrong_program_chunks.is_empty());

        let unrelated_account = Pubkey::new_unique();

        let wrong_account_chunks = extract_from_raw_instructions(
            &[instruction(write_data(b"{}"), 0, vec![1])],
            &[target_program, unrelated_account, idl_address],
            &target_program,
            &idl_address,
        )
        .unwrap();
        assert!(wrong_account_chunks.is_empty());
    }

    #[test]
    fn accepts_write_data_for_target_program_and_idl_account() {
        let target_program = Pubkey::new_unique();
        let idl_address = Pubkey::new_unique();
        let ix = instruction(write_data(b"{}"), 0, vec![1]);

        let raw_chunks = extract_from_raw_instructions(
            &[ix],
            &[target_program, idl_address],
            &target_program,
            &idl_address,
        )
        .unwrap();
        assert_eq!(raw_chunks, vec![b"{}".to_vec()]);

        let parsed_ix = UiInstruction::Compiled(instruction(write_data(b"{}"), 0, vec![1]));

        let parsed_chunks = extract_from_parsed_instructions(
            &[parsed_ix],
            &[target_program, idl_address],
            &target_program,
            &idl_address,
        )
        .unwrap();

        assert_eq!(parsed_chunks, vec![b"{}".to_vec()]);
    }
}
