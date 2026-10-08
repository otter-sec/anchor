#![cfg(feature = "solana-v3")]

use {
    crate::{
        compat::{
            solana_cli_config, solana_client, solana_loader_v3_interface, solana_message,
            solana_packet, solana_pubkey, solana_rpc_client, solana_rpc_client_api,
            solana_transaction,
        },
        program::{simulate_write_compute_unit_limit, WRITE_COMPUTE_UNIT_LIMIT},
    },
    anyhow::{anyhow, Result},
    solana_cli_config::Config as SolanaCliConfig,
    solana_client::{
        connection_cache::ConnectionCache,
        nonblocking::tpu_client::TpuClient as NonblockingTpuClient,
        send_and_confirm_transactions_in_parallel::{
            send_and_confirm_transactions_in_parallel_blocking_v2, SendAndConfirmConfigV2,
        },
        tpu_client::TpuClientConfig,
    },
    solana_commitment_config::CommitmentConfig,
    solana_compute_budget_interface::ComputeBudgetInstruction,
    solana_instruction::Instruction,
    solana_loader_v3_interface::instruction::{self as loader_v3_instruction},
    solana_message::{Hash, Message, VersionedMessage},
    solana_packet::PACKET_DATA_SIZE,
    solana_pubkey::Pubkey,
    solana_rpc_client::rpc_client::RpcClient,
    solana_rpc_client_api::config::RpcSendTransactionConfig,
    solana_sdk_ids::compute_budget as compute_budget_program_id,
    solana_signature::Signature,
    solana_signer::Signer,
    solana_transaction::Transaction,
    std::sync::Arc,
};

/// Send messages in parallel
pub fn send_messages_in_batches(
    rpc_client: &RpcClient,
    messages: &[Message],
    signers: &[&dyn Signer],
    max_sign_attempts: usize,
    use_rpc: bool,
    commitment: CommitmentConfig,
    send_config: RpcSendTransactionConfig,
) -> Result<()> {
    // Use parallel send and confirm function
    // Create a new RpcClient with the same URL and wrap in Arc for parallel processing
    let url = rpc_client.url();
    let new_rpc_client = RpcClient::new_with_commitment(url.clone(), commitment);
    let rpc_client_arc = Arc::new(new_rpc_client);

    // Construct a TPU client so chunk writes go directly to validator leaders
    // via QUIC, bypassing the RPC node's send path.
    //
    // Failure-tolerant: if TPU construction errors (firewall blocks QUIC,
    // websocket unreachable, etc.) we fall back to `None` and the parallel
    // sender uses the RpcClient — slower but functional.
    let tpu_client = if use_rpc {
        None
    } else {
        let ws_url = SolanaCliConfig::compute_websocket_url(&url);
        if ws_url.is_empty() {
            None
        } else {
            match ConnectionCache::new_quic("anchor_program_deploy_tpu", 1) {
                ConnectionCache::Quic(cache_inner) => {
                    let inner_rpc = rpc_client_arc.get_inner_client().clone();
                    let fut = NonblockingTpuClient::new_with_connection_cache(
                        inner_rpc,
                        &ws_url,
                        TpuClientConfig::default(),
                        cache_inner,
                    );
                    match rpc_client_arc.runtime().block_on(fut) {
                        Ok(client) => Some(client),
                        Err(e) => {
                            eprintln!(
                                "Note: TPU client construction failed ({}); falling back to RPC \
                                 for chunk writes. This is slower but functional.",
                                e
                            );
                            None
                        }
                    }
                }
                ConnectionCache::Udp(_) => None,
            }
        }
    };

    let transaction_errors = send_and_confirm_transactions_in_parallel_blocking_v2(
        rpc_client_arc,
        tpu_client,
        messages,
        signers,
        SendAndConfirmConfigV2 {
            resign_txs_count: Some(max_sign_attempts),
            with_spinner: true,
            rpc_send_transaction_config: send_config,
        },
    )
    .map_err(|err| anyhow!("Data writes to account failed: {}", err))?
    .into_iter()
    .flatten()
    // Drop AlreadyProcessed — tx landed via TPU fanout
    .filter(|e| format!("{:?}", e) != "AlreadyProcessed")
    .collect::<Vec<_>>();

    if !transaction_errors.is_empty() {
        for transaction_error in &transaction_errors {
            eprintln!("{:?}", transaction_error);
        }
        return Err(anyhow!(
            "{} write transactions failed",
            transaction_errors.len()
        ));
    }

    Ok(())
}

/// Prepare write messages. When `existing_buffer_data` is provided, skip
/// chunks that already match on-chain bytes — letting resume after a failed
/// deploy only re-send the chunks that didn't land.
pub fn prepare_write_messages(
    program_data: &[u8],
    buffer_pubkey: &Pubkey,
    buffer_authority: &Pubkey,
    fee_payer: &Pubkey,
    blockhash: &Hash,
    priority_fee: Option<u64>,
    existing_buffer_data: Option<&[u8]>,
) -> Vec<Message> {
    let create_msg = |offset: u32, bytes: Vec<u8>| {
        let mut instructions: Vec<Instruction> = Vec::with_capacity(3);
        if let Some(price) = priority_fee {
            if price > 0 {
                instructions.push(ComputeBudgetInstruction::set_compute_unit_price(price));
            }
        }
        instructions.push(ComputeBudgetInstruction::set_compute_unit_limit(
            WRITE_COMPUTE_UNIT_LIMIT,
        ));
        instructions.push(loader_v3_instruction::write(
            buffer_pubkey,
            buffer_authority,
            offset,
            bytes,
        ));
        Message::new_with_blockhash(&instructions, Some(fee_payer), blockhash)
    };

    let mut write_messages = Vec::new();
    let chunk_size = calculate_max_chunk_size(create_msg(0, Vec::new()));

    for (chunk, i) in program_data.chunks(chunk_size).zip(0usize..) {
        let offset = i.saturating_mul(chunk_size);
        let already_written = match existing_buffer_data {
            Some(existing) => {
                let end = offset.saturating_add(chunk.len());
                end <= existing.len() && &existing[offset..end] == chunk
            }
            None => false,
        };
        if !already_written {
            write_messages.push(create_msg(offset as u32, chunk.to_vec()));
        }
    }

    write_messages
}

pub fn calculate_max_chunk_size(baseline_msg: Message) -> usize {
    let tx_size = bincode::serialized_size(&Transaction {
        signatures: vec![
            Signature::default();
            baseline_msg.header.num_required_signatures as usize
        ],
        message: baseline_msg,
    })
    .unwrap() as usize;
    // add 1 byte buffer to account for shortvec encoding
    PACKET_DATA_SIZE.saturating_sub(tx_size).saturating_sub(1)
}

pub fn set_compute_unit_limit_ix_data(
    message: &mut Message,
    ix_index: usize,
    compute_unit_limit: u32,
) {
    let ix = &mut message.instructions[ix_index];
    let program_id = message.account_keys[ix.program_id_index as usize];
    assert_eq!(program_id, compute_budget_program_id::id());
    ix.data = ComputeBudgetInstruction::set_compute_unit_limit(compute_unit_limit).data;
}

pub fn simulate_and_update_write_compute_unit_limit(
    rpc_client: &RpcClient,
    mut write_messages: Vec<Message>,
    fee_payer_signer: &dyn Signer,
    write_signer: &dyn Signer,
) -> Result<Vec<Message>> {
    if write_messages.is_empty() {
        return Ok(write_messages);
    }

    let compute_unit_limit_ix_index = usize::from(write_messages[0].instructions.len() == 3);
    let compute_unit_limit = match simulate_write_compute_unit_limit(
        rpc_client,
        &VersionedMessage::Legacy(write_messages[0].clone()),
        fee_payer_signer,
        write_signer,
    ) {
        Ok(compute_unit_limit) => compute_unit_limit,
        Err(err) => {
            eprintln!(
                "Note: write transaction simulation failed ({}); keeping placeholder compute unit \
                 limit.",
                err
            );
            return Ok(write_messages);
        }
    }
    .compute_units;

    for message in &mut write_messages {
        set_compute_unit_limit_ix_data(message, compute_unit_limit_ix_index, compute_unit_limit);
    }

    Ok(write_messages)
}
