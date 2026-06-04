use {
    super::{rpc, ChunkData, FetchTuning},
    anyhow::{anyhow, Result},
    solana_rpc_client::rpc_client::RpcClient,
    solana_signature::Signature,
    solana_transaction_status_client_types::{
        EncodedConfirmedTransactionWithStatusMeta, EncodedTransaction,
        EncodedTransactionWithStatusMeta, UiCompiledInstruction, UiInstruction, UiMessage,
        UiTransaction,
    },
};

const IDL_IX_TAG: [u8; 8] = [0x40, 0xf4, 0xbc, 0x78, 0xa7, 0xe9, 0x69, 0x0a];
const WRITE_VARIANT: u8 = 0x02;
const IDL_HEADER_SIZE: usize = 13;

// Fetches a transaction and extracts any IDL write payload chunks from its instructions.
pub(super) fn extract_chunks_from_transaction(
    client: &RpcClient,
    signature: &Signature,
    tuning: &FetchTuning,
) -> Result<Vec<ChunkData>> {
    let transaction = rpc::fetch_transaction(client, signature, tuning)?;
    let ui_tx = parse_transaction_data(transaction)?;
    extract_chunks_from_message(ui_tx.message)
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
fn extract_chunks_from_message(message: UiMessage) -> Result<Vec<ChunkData>> {
    let chunks = match message {
        UiMessage::Parsed(parsed_msg) => {
            extract_from_parsed_instructions(&parsed_msg.instructions)?
        }
        UiMessage::Raw(raw_msg) => extract_from_raw_instructions(&raw_msg.instructions)?,
    };
    Ok(chunks)
}

// Scans parsed instructions for compiled IDL write payloads.
fn extract_from_parsed_instructions(instructions: &[UiInstruction]) -> Result<Vec<ChunkData>> {
    let chunks = instructions
        .iter()
        .filter_map(|instruction| {
            if let UiInstruction::Compiled(UiCompiledInstruction { data, .. }) = instruction {
                extract_compressed_chunk(data).ok().flatten()
            } else {
                None
            }
        })
        .collect();
    Ok(chunks)
}

// Scans raw compiled instructions for IDL write payloads.
fn extract_from_raw_instructions(instructions: &[UiCompiledInstruction]) -> Result<Vec<ChunkData>> {
    let chunks = instructions
        .iter()
        .filter_map(|instruction| extract_compressed_chunk(&instruction.data).ok().flatten())
        .collect();
    Ok(chunks)
}

// Decodes one instruction's base58 data and returns its compressed IDL payload when present.
fn extract_compressed_chunk(data_str: &str) -> Result<Option<ChunkData>> {
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
