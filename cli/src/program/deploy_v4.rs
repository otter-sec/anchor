#![cfg(feature = "solana-v4")]

use {
    crate::{
        compat::{
            solana_client, solana_loader_v3_interface, solana_message, solana_packet,
            solana_pubkey, solana_rpc_client, solana_rpc_client_api,
        },
        program::{
            simulate_write_compute_unit_limit, TransactionSimulationResults,
            WRITE_COMPUTE_UNIT_LIMIT,
        },
    },
    anyhow::{anyhow, Result},
    solana_client::send_and_confirm_transactions_in_parallel::{
        send_and_confirm_transactions_in_parallel_v3, SendAndConfirmConfigV3,
    },
    solana_commitment_config::CommitmentConfig,
    solana_instruction::Instruction,
    solana_loader_v3_interface::instruction::{self as loader_v3_instruction},
    solana_message::{Hash, VersionedMessage},
    solana_pubkey::Pubkey,
    solana_rpc_client::rpc_client::RpcClient,
    solana_rpc_client_api::config::RpcSendTransactionConfig,
    solana_signer::Signer,
    std::collections::HashSet,
};

/// Headroom after program data to account for signer(s) and buffer header without simulation.
const LOADED_DATA_SIZE_HEADROOM: u32 = 512;

const MICRO_LAMPORTS_PER_LAMPORT: u64 = 1_000_000;

/// Prepare write messages
pub fn prepare_write_messages(
    program_data: &[u8],
    buffer_pubkey: &Pubkey,
    buffer_authority: &Pubkey,
    fee_payer: &Pubkey,
    blockhash: &Hash,
    priority_fee: Option<u64>,
    existing_buffer_data: Option<&[u8]>,
) -> Vec<VersionedMessage> {
    let mut write_messages = Vec::new();

    // Current BPFLoader accepts 1232 bytes of instruction data
    // construct an instruction with empty bytes to figure how much hwe have left
    let bpf_loader_write_size = bincode::serialize(
        &loader_v3_instruction::UpgradeableLoaderInstruction::Write {
            offset: 0,
            bytes: Vec::new(),
        },
    )
    .unwrap()
    .len();
    let instruction_chunk_size = solana_packet::PACKET_DATA_SIZE - bpf_loader_write_size;
    // with tx v1 we have 4096bytes available in a transaction
    // its enough for 3x instructions with some remaining bytes
    // which may vary depending on number of signers
    let instructions_per_transaction = 3;
    let chunks = program_data.chunks(instruction_chunk_size).zip(0usize..);

    let mut instructions = Vec::new();
    // Priority fee input is in micro-lamports per CU requested
    // calculate the final value as expected by transaction v1
    // (round to 1 lamport if non zero value was requested)
    // https://solana.com/docs/core/transactions/versioned-transactions#resource-limits-in-v1-the-transaction-config
    let compute_unit_limit = WRITE_COMPUTE_UNIT_LIMIT * instructions_per_transaction;
    let priority_fee = if let Some(fee) = priority_fee {
        fee.checked_mul(compute_unit_limit as u64)
            .map(|res| res.div_ceil(MICRO_LAMPORTS_PER_LAMPORT))
            .or(Some(1))
    } else {
        None
    };

    let create_msg = |instructions: Vec<Instruction>| {
        let msg = solana_message::v1::Message::try_compile_with_config(
            fee_payer,
            &instructions,
            *blockhash,
            solana_message::v1::TransactionConfig {
                loaded_accounts_data_size_limit: Some(
                    program_data.len() as u32 + LOADED_DATA_SIZE_HEADROOM,
                ),
                compute_unit_limit: Some(compute_unit_limit),
                priority_fee,
                ..Default::default()
            },
        )
        .unwrap();

        VersionedMessage::V1(msg)
    };

    for (chunk, chunk_idx) in chunks {
        let offset = chunk_idx.saturating_mul(instruction_chunk_size);
        let already_written = match existing_buffer_data {
            Some(existing) => {
                let end = offset.saturating_add(chunk.len());
                end <= existing.len() && &existing[offset..end] == chunk
            }
            None => false,
        };
        if !already_written {
            let ix = loader_v3_instruction::write(
                buffer_pubkey,
                buffer_authority,
                offset as u32,
                chunk.to_vec(),
            );
            instructions.push(ix);

            if instructions.len() == instructions_per_transaction as usize {
                write_messages.push(create_msg(instructions));
                instructions = Vec::new();
            }
        }
    }
    if !instructions.is_empty() {
        write_messages.push(create_msg(instructions));
    }

    write_messages
}

/// Send messages in parallel
pub fn send_messages_in_batches(
    rpc_client: &RpcClient,
    messages: Vec<VersionedMessage>,
    signers: &[&dyn Signer],
    max_sign_attempts: usize,
    _use_rpc: bool,
    commitment: CommitmentConfig,
    send_config: RpcSendTransactionConfig,
) -> Result<()> {
    let mut seen_signers: HashSet<Pubkey> = HashSet::new();
    let deduped_signers = signers
        .iter()
        .filter(|v| seen_signers.insert(v.pubkey()))
        .copied()
        .collect::<Vec<&dyn Signer>>();

    let fut = send_and_confirm_transactions_in_parallel_v3(
        rpc_client.get_inner_client().clone(),
        solana_client::send_and_confirm_transactions_in_parallel::SendTransport::Rpc(
            RpcSendTransactionConfig {
                preflight_commitment: Some(commitment.commitment),
                ..send_config
            },
        ),
        messages,
        &deduped_signers,
        SendAndConfirmConfigV3 {
            max_sign_attempts: std::num::NonZero::new(max_sign_attempts)
                .expect("should be non zero sign attempts"),
            with_spinner: true,
            ..SendAndConfirmConfigV3::default()
        },
    );

    let transaction_errors = tokio::task::block_in_place(|| rpc_client.runtime().block_on(fut))
        .map_err(|err| anyhow!("Data writes to account failed: {}", err))?
        .into_iter()
        .flatten()
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

pub fn simulate_and_update_write_compute_unit_limit(
    rpc_client: &RpcClient,
    mut write_messages: Vec<VersionedMessage>,
    fee_payer_signer: &dyn Signer,
    write_signer: &dyn Signer,
) -> Result<Vec<VersionedMessage>> {
    if write_messages.is_empty() {
        return Ok(write_messages);
    }

    let simulation_result = match simulate_write_compute_unit_limit(
        rpc_client,
        &write_messages[0],
        fee_payer_signer,
        write_signer,
    ) {
        Ok(simulation_result) => simulation_result,
        Err(err) => {
            eprintln!(
                "Note: write transaction simulation failed ({}); keeping placeholder compute unit \
                 limit and loaded data size.",
                err
            );
            return Ok(write_messages);
        }
    };

    for message in &mut write_messages {
        set_compute_unit_limit_ix_data(message, &simulation_result);
    }

    Ok(write_messages)
}

pub fn set_compute_unit_limit_ix_data(
    message: &mut VersionedMessage,
    simulation_result: &TransactionSimulationResults,
) {
    if let VersionedMessage::V1(inner_message) = message {
        inner_message.config.compute_unit_limit = Some(simulation_result.compute_units);
        inner_message.config.loaded_accounts_data_size_limit =
            Some(simulation_result.loaded_data_size);
    }
}
