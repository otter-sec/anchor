use {
    super::{
        parallel::{historical_fetch_worker_count, should_parallelize_historical_fetch},
        rpc::{fetch_signatures_for_address, fetch_transaction},
        MAX_IDL_BUFFER_BYTES,
    },
    crate::fetch::FetchTuning,
    anyhow::{anyhow, bail, Context, Result},
    base64::Engine,
    flate2::read::{GzDecoder, ZlibDecoder},
    solana_pubkey::{pubkey, Pubkey},
    solana_rpc_client::rpc_client::RpcClient,
    solana_rpc_client_api::response::RpcConfirmedTransactionStatusWithSignature,
    solana_signature::Signature,
    solana_transaction_status_client_types::{
        EncodedConfirmedTransactionWithStatusMeta, EncodedTransaction,
        EncodedTransactionWithStatusMeta, UiCompiledInstruction, UiInstruction, UiMessage,
        UiTransaction,
    },
    std::{fmt, io::Read, str::FromStr, thread},
};

const PMP_PROGRAM_ID: Pubkey = pubkey!("ProgM6JCCvbYkfKqJYHePx4xxSUSqJp7rh8Lyv7nk7S");
const IDL_SEED: &str = "idl";
const SEED_SIZE: usize = 16;
const METADATA_SEED_PADDING: usize = 14;

// Buffer accounts include a fixed header before the staged metadata payload bytes begin.
const PMP_BUFFER_HEADER_SIZE: usize = 1 + 32 + 32 + 1 + SEED_SIZE + METADATA_SEED_PADDING;

// Mirrors the Program Metadata instruction discriminators so historical fetch can decode the raw
// compiled instructions it replays from transaction history.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
enum PmpInstruction {
    Write = 0,
    Initialize = 1,
    SetAuthority = 2,
    SetData = 3,
    SetImmutable = 4,
    Trim = 5,
    Close = 6,
    Allocate = 7,
    Extend = 8,
}

impl TryFrom<u8> for PmpInstruction {
    // Converts the first instruction byte into the typed PMP instruction kind used by the replay
    // logic below.
    type Error = PmpFetchError;

    fn try_from(value: u8) -> std::result::Result<Self, Self::Error> {
        match value {
            0 => Ok(Self::Write),
            1 => Ok(Self::Initialize),
            2 => Ok(Self::SetAuthority),
            3 => Ok(Self::SetData),
            4 => Ok(Self::SetImmutable),
            5 => Ok(Self::Trim),
            6 => Ok(Self::Close),
            7 => Ok(Self::Allocate),
            8 => Ok(Self::Extend),
            _ => Err(PmpFetchError::invalid_transaction(format!(
                "unknown PMP instruction {value}"
            ))),
        }
    }
}

// Describes where the metadata payload lives so historical fetch can distinguish direct bytes from
// URL or external-account-backed metadata that it intentionally does not chase.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
enum MetadataDataSource {
    Direct = 0,
    Url = 1,
    External = 2,
}

impl TryFrom<u8> for MetadataDataSource {
    // Decodes the serialized PMP data-source tag from the metadata instruction header.
    type Error = PmpFetchError;

    fn try_from(value: u8) -> std::result::Result<Self, Self::Error> {
        match value {
            0 => Ok(Self::Direct),
            1 => Ok(Self::Url),
            2 => Ok(Self::External),
            _ => Err(PmpFetchError::invalid_transaction(format!(
                "unsupported metadata data source {value}"
            ))),
        }
    }
}

// Encodes the logical content format of the stored metadata so non-JSON payloads can be rejected
// explicitly by IDL historical fetch.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
enum MetadataFormat {
    None = 0,
    Json = 1,
    Yaml = 2,
    Toml = 3,
}

impl TryFrom<u8> for MetadataFormat {
    // Decodes the serialized metadata-format tag from Initialize and SetData instructions.
    type Error = PmpFetchError;

    fn try_from(value: u8) -> std::result::Result<Self, Self::Error> {
        match value {
            0 => Ok(Self::None),
            1 => Ok(Self::Json),
            2 => Ok(Self::Yaml),
            3 => Ok(Self::Toml),
            _ => Err(PmpFetchError::invalid_transaction(format!(
                "unsupported PMP metadata format {value}"
            ))),
        }
    }
}

// Captures how direct PMP metadata bytes were encoded before they were written on-chain.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
enum MetadataEncoding {
    None = 0,
    Utf8 = 1,
    Base58 = 2,
    Base64 = 3,
}

impl TryFrom<u8> for MetadataEncoding {
    // Decodes the serialized metadata-encoding tag from the instruction header.
    type Error = PmpFetchError;

    fn try_from(value: u8) -> std::result::Result<Self, Self::Error> {
        match value {
            0 => Ok(Self::None),
            1 => Ok(Self::Utf8),
            2 => Ok(Self::Base58),
            3 => Ok(Self::Base64),
            _ => Err(PmpFetchError::invalid_transaction(format!(
                "unsupported PMP metadata encoding {value}"
            ))),
        }
    }
}

// Captures whether the direct metadata bytes were compressed before they were encoded on-chain.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
enum MetadataCompression {
    None = 0,
    Gzip = 1,
    Zlib = 2,
}

impl TryFrom<u8> for MetadataCompression {
    // Decodes the serialized compression tag from the instruction header.
    type Error = PmpFetchError;

    fn try_from(value: u8) -> std::result::Result<Self, Self::Error> {
        match value {
            0 => Ok(Self::None),
            1 => Ok(Self::Gzip),
            2 => Ok(Self::Zlib),
            _ => Err(PmpFetchError::invalid_transaction(format!(
                "unsupported PMP metadata compression {value}"
            ))),
        }
    }
}

// Classifies non-fatal PMP historical fetch failures so callers can report skipped history entries
// without losing the recoverable versions from other slots.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PmpHistoryWarningKind {
    UnsupportedDataSource,
    MissingBuffer,
    InvalidBufferInstruction,
    InvalidTransaction,
    RpcError,
}

// Records one warning attached to a specific historical slot for verbose PMP fetch output.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PmpHistoryWarning {
    pub slot: u64,
    pub kind: PmpHistoryWarningKind,
    pub detail: String,
}

// Represents one recovered PMP historical IDL before it is normalized into the shared history
// merge type used by the higher-level fetch orchestration.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PmpHistoricalIdl {
    pub slot: u64,
    pub signature: String,
    pub idl_data: Vec<u8>,
}

// Bundles all recoverable PMP historical versions with any non-fatal warnings encountered during
// transaction replay.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct PmpHistoricalFetch {
    pub idls: Vec<PmpHistoricalIdl>,
    pub warnings: Vec<PmpHistoryWarning>,
}

// Holds the per-update metadata header fields that describe how to decode a direct PMP payload.
#[derive(Clone, Debug, Eq, PartialEq)]
struct PmpMetadataHeader {
    encoding: MetadataEncoding,
    compression: MetadataCompression,
    format: MetadataFormat,
    data_source: MetadataDataSource,
}

// Represents one Write instruction applied to a PMP buffer or staged metadata account.
#[derive(Clone, Debug, Eq, PartialEq)]
struct BufferWrite {
    offset: usize,
    bytes: Vec<u8>,
}

// Captures the byte writes and allocation boundary recovered while replaying a buffer-like account
// up to a target slot.
#[derive(Clone, Debug)]
struct PriorWriteReconstruction {
    writes: Vec<BufferWrite>,
    saw_allocate: bool,
}

// Internal error type that preserves the warning category alongside the human-readable detail.
#[derive(Clone, Debug)]
struct PmpFetchError {
    kind: PmpHistoryWarningKind,
    detail: String,
}

impl PmpFetchError {
    // Builds an error for URL/external data-source shapes that historical fetch intentionally
    // skips instead of resolving indirectly.
    fn unsupported_data_source(detail: impl Into<String>) -> Self {
        Self {
            kind: PmpHistoryWarningKind::UnsupportedDataSource,
            detail: detail.into(),
        }
    }

    // Builds an error for update flows that require a buffer account but do not provide one.
    fn missing_buffer(detail: impl Into<String>) -> Self {
        Self {
            kind: PmpHistoryWarningKind::MissingBuffer,
            detail: detail.into(),
        }
    }

    // Builds an error for malformed or incomplete buffer-replay instructions.
    fn invalid_buffer_instruction(detail: impl Into<String>) -> Self {
        Self {
            kind: PmpHistoryWarningKind::InvalidBufferInstruction,
            detail: detail.into(),
        }
    }

    // Builds an error for malformed transaction data or instruction payloads.
    fn invalid_transaction(detail: impl Into<String>) -> Self {
        Self {
            kind: PmpHistoryWarningKind::InvalidTransaction,
            detail: detail.into(),
        }
    }
}

impl fmt::Display for PmpFetchError {
    // Uses the stored detail directly because warning rendering is handled by higher-level code.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.detail.fmt(f)
    }
}

// Derives the PMP metadata PDA for either the canonical IDL (no authority) or an authority-scoped
// non-canonical IDL.
pub fn pmp_metadata_address(program_id: &Pubkey, authority: Option<&Pubkey>) -> Pubkey {
    let mut padded_seed = [0u8; SEED_SIZE];
    padded_seed[..IDL_SEED.len()].copy_from_slice(IDL_SEED.as_bytes());
    let authority_seed: &[u8] = authority.map(|key| key.as_ref()).unwrap_or(&[]);
    // Canonical PMP IDLs use an empty authority seed; non-canonical IDLs use the authority pubkey
    // as part of the PDA derivation.
    Pubkey::find_program_address(
        &[program_id.as_ref(), authority_seed, &padded_seed],
        &PMP_PROGRAM_ID,
    )
    .0
}

// Replays the transaction history of a PMP metadata account and recovers every historical IDL
// version that can be decoded from it.
pub fn fetch_pmp_historical_idls(
    client: &RpcClient,
    program_id: &Pubkey,
    authority: Option<&Pubkey>,
    signatures: &[RpcConfirmedTransactionStatusWithSignature],
    tuning: &FetchTuning,
) -> PmpHistoricalFetch {
    let mut result = PmpHistoricalFetch::default();
    let metadata_address = pmp_metadata_address(program_id, authority);

    // Unsupported PMP source shapes are recorded as warnings so mixed legacy/PMP history fetches
    // can still return the recoverable versions.
    for sig in signatures {
        let Ok(signature) = Signature::from_str(&sig.signature) else {
            result.warnings.push(PmpHistoryWarning {
                slot: sig.slot,
                kind: PmpHistoryWarningKind::InvalidTransaction,
                detail: format!("invalid signature {}", sig.signature),
            });
            continue;
        };

        match fetch_pmp_idl_from_transaction(
            client,
            &metadata_address,
            &signature,
            sig.slot,
            tuning,
        ) {
            Ok(idls) => result
                .idls
                .extend(idls.into_iter().map(|idl_data| PmpHistoricalIdl {
                    slot: sig.slot,
                    signature: sig.signature.clone(),
                    idl_data,
                })),
            Err(err) => result.warnings.push(PmpHistoryWarning {
                slot: sig.slot,
                kind: err.kind,
                detail: err.detail,
            }),
        }
    }

    result
}

// Extracts every PMP IDL update contained in a single transaction touching the metadata account.
fn fetch_pmp_idl_from_transaction(
    client: &RpcClient,
    metadata_address: &Pubkey,
    signature: &Signature,
    slot: u64,
    tuning: &FetchTuning,
) -> std::result::Result<Vec<Vec<u8>>, PmpFetchError> {
    let transaction =
        fetch_transaction(client, signature, tuning).map_err(|err| PmpFetchError {
            kind: PmpHistoryWarningKind::RpcError,
            detail: err.to_string(),
        })?;
    let ui_tx = parse_transaction_data(transaction)
        .map_err(|err| PmpFetchError::invalid_transaction(err.to_string()))?;
    let account_keys = account_keys(&ui_tx.message)
        .map_err(|err| PmpFetchError::invalid_transaction(err.to_string()))?;
    // Flatten all compiled instructions first so versioned messages and parsed/raw transaction
    // formats feed the same PMP decoder path.
    let instructions = flatten_compiled_instructions(&ui_tx.message, &account_keys);
    let mut recovered = Vec::new();

    for instruction in instructions {
        let Some(program_id) = instruction.program_id(&account_keys) else {
            continue;
        };
        if program_id != PMP_PROGRAM_ID {
            continue;
        }
        if instruction.accounts.first().copied() != Some(*metadata_address) {
            continue;
        }

        match instruction
            .kind()
            .map_err(|err| PmpFetchError::invalid_transaction(err.to_string()))?
        {
            PmpInstruction::Initialize => {
                let (header, inline_bytes) = parse_initialize_data(
                    instruction
                        .data_bytes()
                        .map_err(|err| PmpFetchError::invalid_transaction(err.to_string()))?,
                )?;
                if !inline_bytes.is_empty() {
                    recovered.push(header.decode_direct(inline_bytes)?);
                    continue;
                }

                let reconstructed =
                    reconstruct_prior_writes(client, metadata_address, slot, tuning)?;
                if !reconstructed.saw_allocate {
                    return Err(PmpFetchError::missing_buffer(
                        "metadata account missing prior allocate for PMP initialize",
                    ));
                }
                let staged_bytes = apply_buffer_writes_to_bytes(reconstructed.writes)
                    .map_err(|err| PmpFetchError::invalid_buffer_instruction(err.to_string()))?;
                recovered.push(header.decode_direct(&staged_bytes)?);
            }
            PmpInstruction::SetData => {
                let (header, inline_bytes) = parse_set_data(
                    instruction
                        .data_bytes()
                        .map_err(|err| PmpFetchError::invalid_transaction(err.to_string()))?,
                )?;
                if !inline_bytes.is_empty() {
                    recovered.push(header.decode_direct(inline_bytes)?);
                    continue;
                }

                let buffer_address = instruction.accounts.get(2).copied().ok_or_else(|| {
                    PmpFetchError::missing_buffer("buffer account missing for PMP set-data")
                })?;
                // Some generated clients pad the optional buffer position with the program id when
                // no buffer account is present.
                if buffer_address == PMP_PROGRAM_ID {
                    return Err(PmpFetchError::missing_buffer(
                        "buffer account missing for PMP set-data",
                    ));
                }
                let buffer_bytes = reconstruct_buffer_bytes(client, &buffer_address, slot, tuning)?;
                recovered.push(header.decode_direct(&buffer_bytes)?);
            }
            _ => {}
        }
    }

    Ok(recovered)
}

// Replays a separate PMP buffer account up to a target slot and returns just the staged payload
// bytes stored after its fixed header.
fn reconstruct_buffer_bytes(
    client: &RpcClient,
    buffer_address: &Pubkey,
    before_or_at_slot: u64,
    tuning: &FetchTuning,
) -> std::result::Result<Vec<u8>, PmpFetchError> {
    // Buffer-backed PMP updates replay the buffer account's Write history up to the metadata slot
    // and then strip the fixed account header to recover the staged payload bytes.
    let writes_in_order =
        reconstruct_prior_writes(client, buffer_address, before_or_at_slot, tuning)?.writes;
    apply_buffer_writes_to_bytes(writes_in_order).map_err(|err| {
        PmpFetchError::invalid_buffer_instruction(format!("buffer account {buffer_address} {err}"))
    })
}

// Reconstructs the ordered Write stream for either a dedicated buffer account or a metadata
// account temporarily used as a staging buffer.
fn reconstruct_prior_writes(
    client: &RpcClient,
    account_address: &Pubkey,
    before_or_at_slot: u64,
    tuning: &FetchTuning,
) -> std::result::Result<PriorWriteReconstruction, PmpFetchError> {
    // Reuse the shared account-signature pagination so slot filtering semantics stay aligned
    // across legacy and PMP historical fetch paths.
    let mut eligible = fetch_signatures_for_address(
        client,
        account_address,
        None,
        None,
        Some(before_or_at_slot),
        tuning.max_signatures,
    )
    .map_err(|err| PmpFetchError::invalid_transaction(err.to_string()))?
    .into_iter()
    .map(|status| {
        let signature = Signature::from_str(&status.signature)
            .map_err(|err| PmpFetchError::invalid_transaction(err.to_string()))?;
        Ok((status.slot, signature))
    })
    .collect::<std::result::Result<Vec<_>, PmpFetchError>>()?;

    // Signature pagination is newest-first; replay must be oldest-first so later writes win.
    eligible.reverse();

    fetch_buffer_writes_in_order(client, account_address, &eligible, tuning)
}

// Applies the recovered write spans to a byte vector and strips the fixed PMP buffer header to
// produce the staged metadata payload.
fn apply_buffer_writes_to_bytes(buffer_writes: Vec<BufferWrite>) -> Result<Vec<u8>> {
    let mut highest_end = 0usize;

    // Allocate once to the final observed size, then replay each contiguous write span directly.
    for write in &buffer_writes {
        highest_end = highest_end.max(write.offset + write.bytes.len());
    }

    if highest_end == 0 {
        bail!("did not contain any recoverable PMP buffer write");
    }
    if highest_end > MAX_IDL_BUFFER_BYTES {
        bail!(
            "reconstructed PMP buffer exceeds safety limit of {} bytes",
            MAX_IDL_BUFFER_BYTES
        );
    }

    let mut buffer = vec![0u8; highest_end];
    for write in buffer_writes {
        let end = write.offset + write.bytes.len();
        buffer[write.offset..end].copy_from_slice(&write.bytes);
    }

    // Allocate stamps the buffer discriminator on-chain; reconstruction only sees user Writes,
    // so we just verify there's room for the header and slice the data section out of it.
    let header_end = PMP_BUFFER_HEADER_SIZE;
    if buffer.len() < header_end {
        bail!("does not decode as a PMP buffer");
    }

    Ok(buffer[header_end..].to_vec())
}

// Fetches and replays all relevant buffer-write transactions in chronological order, preserving
// Allocate boundaries and last-write-wins semantics.
fn fetch_buffer_writes_in_order(
    client: &RpcClient,
    buffer_address: &Pubkey,
    signatures: &[(u64, Signature)],
    tuning: &FetchTuning,
) -> std::result::Result<PriorWriteReconstruction, PmpFetchError> {
    if signatures.is_empty() {
        return Ok(PriorWriteReconstruction {
            writes: Vec::new(),
            saw_allocate: false,
        });
    }

    let results = if should_parallelize_historical_fetch(signatures.len(), tuning) {
        fetch_buffer_writes_parallel(client, buffer_address, signatures, tuning)?
    } else {
        signatures
            .iter()
            .map(|(_, signature)| {
                extract_buffer_writes_for_signature(client, buffer_address, signature, tuning)
            })
            .collect::<std::result::Result<Vec<_>, _>>()?
    };

    // The caller already provided chronological signatures, and each chunk preserves input order,
    // so flattening here keeps the final write replay deterministic.
    let mut writes = Vec::new();
    let mut saw_allocate = false;
    for ops in results {
        for op in ops {
            match op {
                BufferReplayOp::Write(write) => writes.push(write),
                BufferReplayOp::Allocate => {
                    saw_allocate = true;
                    writes.clear();
                }
                BufferReplayOp::Extend => {}
                // Replay is based on the written byte ranges rather than the account's allocated
                // capacity, so trimming rent-excess space does not change the reconstructed
                // payload bytes.
                BufferReplayOp::Trim => {}
            }
        }
    }

    Ok(PriorWriteReconstruction {
        writes,
        saw_allocate,
    })
}

// Parallelizes buffer-write transaction fetches when there are enough signatures to justify the
// extra worker threads.
fn fetch_buffer_writes_parallel(
    client: &RpcClient,
    buffer_address: &Pubkey,
    signatures: &[(u64, Signature)],
    tuning: &FetchTuning,
) -> std::result::Result<Vec<Vec<BufferReplayOp>>, PmpFetchError> {
    let worker_count = historical_fetch_worker_count(signatures.len(), tuning);
    if worker_count <= 1 {
        return signatures
            .iter()
            .map(|(_, signature)| {
                extract_buffer_writes_for_signature(client, buffer_address, signature, tuning)
            })
            .collect();
    }

    let chunk_size = signatures.len().div_ceil(worker_count);
    thread::scope(|scope| {
        let mut handles = Vec::new();
        // Split the ordered signature list into stable chunks so the joined results can be
        // concatenated without another sort.
        for chunk in signatures.chunks(chunk_size) {
            handles.push(scope.spawn(move || {
                chunk
                    .iter()
                    .map(|(_, signature)| {
                        extract_buffer_writes_for_signature(
                            client,
                            buffer_address,
                            signature,
                            tuning,
                        )
                    })
                    .collect::<std::result::Result<Vec<_>, _>>()
            }));
        }

        let mut results = Vec::new();
        for handle in handles {
            let chunk_result = handle.join().expect("PMP buffer fetch worker panicked")?;
            results.extend(chunk_result);
        }
        Ok(results)
    })
}

// Extracts only the buffer-replay operations from one transaction for the requested account.
fn extract_buffer_writes_for_signature(
    client: &RpcClient,
    buffer_address: &Pubkey,
    signature: &Signature,
    tuning: &FetchTuning,
) -> std::result::Result<Vec<BufferReplayOp>, PmpFetchError> {
    let transaction =
        fetch_transaction(client, signature, tuning).map_err(|err| PmpFetchError {
            kind: PmpHistoryWarningKind::RpcError,
            detail: err.to_string(),
        })?;
    let ui_tx = parse_transaction_data(transaction)
        .map_err(|err| PmpFetchError::invalid_transaction(err.to_string()))?;
    let account_keys = account_keys(&ui_tx.message)
        .map_err(|err| PmpFetchError::invalid_transaction(err.to_string()))?;
    let instructions = flatten_compiled_instructions(&ui_tx.message, &account_keys);

    instructions
        .into_iter()
        .filter_map(|instruction| {
            let program_id = instruction.program_id(&account_keys)?;
            if program_id != PMP_PROGRAM_ID {
                return None;
            }
            if instruction.accounts.first().copied() != Some(*buffer_address) {
                return None;
            }
            Some(instruction)
        })
        .filter_map(|instruction| {
            // The metadata account itself can host both buffer-style writes (Allocate/Write/Extend
            // /Trim) and the finalising Initialize/SetData calls. Replay only needs the former,
            // so non-buffer ops are silently skipped instead of failing the reconstruction.
            let kind = match instruction.kind() {
                Ok(kind) => kind,
                Err(err) => {
                    return Some(Err(PmpFetchError::invalid_transaction(err.to_string())));
                }
            };
            match kind {
                PmpInstruction::Write => Some(
                    instruction
                        .data_bytes()
                        .map_err(|err| PmpFetchError::invalid_transaction(err.to_string()))
                        .and_then(parse_write_data)
                        .map(BufferReplayOp::Write),
                ),
                PmpInstruction::Allocate => Some(Ok(BufferReplayOp::Allocate)),
                PmpInstruction::Extend => Some(Ok(BufferReplayOp::Extend)),
                PmpInstruction::Trim => Some(Ok(BufferReplayOp::Trim)),
                PmpInstruction::Initialize
                | PmpInstruction::SetData
                | PmpInstruction::SetAuthority
                | PmpInstruction::SetImmutable
                | PmpInstruction::Close => None,
            }
        })
        .collect()
}

// Normalizes the RPC response into the JSON transaction representation expected by the PMP
// instruction decoder.
fn parse_transaction_data(
    transaction: EncodedConfirmedTransactionWithStatusMeta,
) -> Result<UiTransaction> {
    match transaction.transaction {
        EncodedTransactionWithStatusMeta {
            transaction: EncodedTransaction::Json(ui_tx),
            ..
        } => Ok(ui_tx),
        _ => Err(anyhow!("invalid transaction format")),
    }
}

// Converts a raw or parsed message's account-key list into typed pubkeys for instruction decoding.
fn account_keys(message: &UiMessage) -> Result<Vec<Pubkey>> {
    match message {
        UiMessage::Raw(raw) => raw
            .account_keys
            .iter()
            .map(|key| Pubkey::from_str(key))
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(Into::into),
        UiMessage::Parsed(parsed) => parsed
            .account_keys
            .iter()
            .map(|key| Pubkey::from_str(&key.pubkey))
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(Into::into),
    }
}

// Flattens both raw and parsed transaction messages into one compiled-instruction view so the PMP
// decoder can ignore outer message representation differences.
fn flatten_compiled_instructions(
    message: &UiMessage,
    account_keys: &[Pubkey],
) -> Vec<CompiledInstructionView> {
    match message {
        UiMessage::Raw(raw) => raw
            .instructions
            .iter()
            .map(|ix| CompiledInstructionView::from_raw(ix, account_keys))
            .collect(),
        UiMessage::Parsed(parsed) => parsed
            .instructions
            .iter()
            .filter_map(|ix| CompiledInstructionView::from_parsed(ix, account_keys))
            .collect(),
    }
}

// Lightweight instruction view used during historical replay so PMP decoding only deals with typed
// account keys and already-decoded instruction bytes.
#[derive(Clone, Debug)]
struct CompiledInstructionView {
    program_id_index: u8,
    accounts: Vec<Pubkey>,
    data: std::result::Result<Vec<u8>, bs58::decode::Error>,
}

// Enumerates the subset of PMP instructions that matter for reconstructing staged buffer content.
#[derive(Clone, Debug, Eq, PartialEq)]
enum BufferReplayOp {
    Allocate,
    Extend,
    Trim,
    Write(BufferWrite),
}

impl CompiledInstructionView {
    // Builds a typed instruction view from a raw compiled instruction in the RPC response.
    fn from_raw(ix: &UiCompiledInstruction, account_keys: &[Pubkey]) -> Self {
        Self {
            program_id_index: ix.program_id_index,
            accounts: ix
                .accounts
                .iter()
                .filter_map(|index| account_keys.get(*index as usize).copied())
                .collect(),
            data: bs58::decode(&ix.data).into_vec(),
        }
    }

    // Converts parsed-message instructions into the same internal representation used for raw
    // messages, skipping instruction forms that cannot be represented as compiled bytes.
    fn from_parsed(ix: &UiInstruction, account_keys: &[Pubkey]) -> Option<Self> {
        match ix {
            UiInstruction::Compiled(compiled) => Some(Self::from_raw(compiled, account_keys)),
            _ => None,
        }
    }

    // Resolves the instruction's program id from the surrounding message account-key list.
    fn program_id(&self, account_keys: &[Pubkey]) -> Option<Pubkey> {
        account_keys.get(self.program_id_index as usize).copied()
    }

    // Decodes the first instruction byte into the typed PMP instruction kind.
    fn kind(&self) -> Result<PmpInstruction> {
        let data = self.data_bytes()?;
        let value = data
            .first()
            .copied()
            .ok_or_else(|| anyhow!("empty PMP instruction data"))?;
        PmpInstruction::try_from(value).map_err(|err| anyhow!(err.detail))
    }

    // Returns the decoded instruction bytes or a descriptive error if the RPC payload was not
    // valid base58.
    fn data_bytes(&self) -> Result<&[u8]> {
        self.data
            .as_deref()
            .map_err(|err| anyhow!("invalid base58 PMP instruction data: {err}"))
    }
}

// Parses the payload of a PMP Write instruction into an offset plus the bytes to be written.
fn parse_write_data(data: &[u8]) -> std::result::Result<BufferWrite, PmpFetchError> {
    if data.len() < 5 || data[0] != PmpInstruction::Write as u8 {
        return Err(PmpFetchError::invalid_buffer_instruction(
            "invalid PMP buffer write instruction",
        ));
    }
    let offset = u32::from_le_bytes(data[1..5].try_into().map_err(|_| {
        PmpFetchError::invalid_buffer_instruction("invalid PMP buffer write offset")
    })?) as usize;
    Ok(BufferWrite {
        offset,
        bytes: data[5..].to_vec(),
    })
}

// Parses the header and inline payload bytes carried by a PMP Initialize instruction.
fn parse_initialize_data(
    data: &[u8],
) -> std::result::Result<(PmpMetadataHeader, &[u8]), PmpFetchError> {
    if data.len() < 1 + SEED_SIZE + 4 || data[0] != PmpInstruction::Initialize as u8 {
        return Err(PmpFetchError::invalid_transaction(
            "invalid PMP initialize instruction",
        ));
    }
    // Historical fetch only supports the `"idl"` metadata seed; other PMP metadata namespaces are
    // intentionally ignored by this path.
    let seed = parse_seed(&data[1..1 + SEED_SIZE])
        .map_err(|err| PmpFetchError::invalid_transaction(err.to_string()))?;
    if seed != IDL_SEED {
        return Err(PmpFetchError::invalid_transaction(format!(
            "unsupported PMP metadata seed {seed}"
        )));
    }
    let header = PmpMetadataHeader {
        encoding: MetadataEncoding::try_from(data[1 + SEED_SIZE])?,
        compression: MetadataCompression::try_from(data[1 + SEED_SIZE + 1])?,
        format: MetadataFormat::try_from(data[1 + SEED_SIZE + 2])?,
        data_source: MetadataDataSource::try_from(data[1 + SEED_SIZE + 3])?,
    };
    Ok((header, &data[1 + SEED_SIZE + 4..]))
}

// Parses the header and inline payload bytes carried by a PMP SetData instruction.
fn parse_set_data(data: &[u8]) -> std::result::Result<(PmpMetadataHeader, &[u8]), PmpFetchError> {
    if data.len() < 5 || data[0] != PmpInstruction::SetData as u8 {
        return Err(PmpFetchError::invalid_transaction(
            "invalid PMP set-data instruction",
        ));
    }
    Ok((
        PmpMetadataHeader {
            encoding: MetadataEncoding::try_from(data[1])?,
            compression: MetadataCompression::try_from(data[2])?,
            format: MetadataFormat::try_from(data[3])?,
            data_source: MetadataDataSource::try_from(data[4])?,
        },
        &data[5..],
    ))
}

// Decodes the fixed-size seed field used in PMP metadata accounts and Initialize instructions.
fn parse_seed(bytes: &[u8]) -> Result<String> {
    let end = bytes.iter().position(|b| *b == 0).unwrap_or(bytes.len());
    std::str::from_utf8(&bytes[..end])
        .map(|s| s.to_string())
        .context("invalid UTF-8 metadata seed")
}

impl PmpMetadataHeader {
    // Fully decodes a direct PMP payload according to its header fields and verifies that the
    // recovered bytes are valid JSON before returning them as an IDL candidate.
    fn decode_direct(self, bytes: &[u8]) -> std::result::Result<Vec<u8>, PmpFetchError> {
        if self.format != MetadataFormat::Json {
            return Err(PmpFetchError::invalid_transaction(format!(
                "unsupported PMP metadata format {:?}",
                self.format
            )));
        }
        if self.data_source != MetadataDataSource::Direct {
            return Err(PmpFetchError::unsupported_data_source(format!(
                "unsupported metadata data source {:?}",
                self.data_source
            )));
        }

        // Decode and decompress in the same order the JS PMP client applies when writing direct
        // metadata content, then validate that the recovered payload is JSON before returning it.
        let decoded = decode_metadata_data(bytes, self.encoding)?;
        let decompressed = decompress_metadata_data(&decoded, self.compression)?;
        serde_json::from_slice::<serde_json::Value>(&decompressed).map_err(|err| {
            PmpFetchError::invalid_transaction(format!(
                "PMP metadata payload is not valid JSON: {err}"
            ))
        })?;
        Ok(decompressed)
    }
}

// Decodes the direct payload bytes according to the encoding specified in the PMP metadata header.
fn decode_metadata_data(
    bytes: &[u8],
    encoding: MetadataEncoding,
) -> std::result::Result<Vec<u8>, PmpFetchError> {
    match encoding {
        MetadataEncoding::None => {
            let text = std::str::from_utf8(bytes)
                .map_err(|_| PmpFetchError::invalid_transaction("invalid hex metadata payload"))?;
            hex_decode(text).map_err(|err| PmpFetchError::invalid_transaction(err.to_string()))
        }
        MetadataEncoding::Utf8 => Ok(bytes.to_vec()),
        MetadataEncoding::Base58 => bs58::decode(bytes).into_vec().map_err(|err| {
            PmpFetchError::invalid_transaction(format!("invalid base58 metadata payload: {err}"))
        }),
        MetadataEncoding::Base64 => base64::engine::general_purpose::STANDARD
            .decode(bytes)
            .map_err(|err| {
                PmpFetchError::invalid_transaction(format!(
                    "invalid base64 metadata payload: {err}"
                ))
            }),
    }
}

// Decompresses direct payload bytes according to the compression specified in the PMP metadata
// header.
fn decompress_metadata_data(
    bytes: &[u8],
    compression: MetadataCompression,
) -> std::result::Result<Vec<u8>, PmpFetchError> {
    match compression {
        MetadataCompression::None => Ok(bytes.to_vec()),
        MetadataCompression::Gzip => {
            let mut decoder = GzDecoder::new(bytes);
            read_bounded_decompressed(&mut decoder)
        }
        MetadataCompression::Zlib => {
            let mut decoder = ZlibDecoder::new(bytes);
            read_bounded_decompressed(&mut decoder)
        }
    }
}

// Reads the decoder output capped at `MAX_IDL_BUFFER_BYTES`. The decoder is consumed
// through `Read::take` so a bomb cannot expand past the bound, and exceeding it surfaces as
// `invalid_transaction` rather than an unbounded allocation.
fn read_bounded_decompressed<R: Read>(
    decoder: &mut R,
) -> std::result::Result<Vec<u8>, PmpFetchError> {
    let mut out = Vec::new();
    // Read one byte past the cap so that hitting `MAX + 1` definitively signals overflow rather
    // than a payload that legitimately ends on the boundary.
    decoder
        .take(MAX_IDL_BUFFER_BYTES as u64 + 1)
        .read_to_end(&mut out)
        .map_err(|err| PmpFetchError::invalid_transaction(err.to_string()))?;
    if out.len() > MAX_IDL_BUFFER_BYTES {
        return Err(PmpFetchError::invalid_transaction(format!(
            "decompressed PMP metadata exceeds safety limit of {} bytes",
            MAX_IDL_BUFFER_BYTES
        )));
    }
    Ok(out)
}

// Decodes hexadecimal metadata payloads used when PMP encoding is `None`.
fn hex_decode(hex: &str) -> Result<Vec<u8>> {
    if !hex.len().is_multiple_of(2) {
        bail!("hex metadata payload has odd length");
    }
    (0..hex.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&hex[i..i + 2], 16).map_err(anyhow::Error::from))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn canonical_pmp_address_matches_ts_logic() {
        let program = Pubkey::from_str("2uA3amp95zsEHUpo8qnLMhcFAUsiKVEcKHXS1JetFjU5").unwrap();
        assert_eq!(
            pmp_metadata_address(&program, None).to_string(),
            "FquHyG5PSt6GNyzAm7LFupqoKkCbbsLTUVvzdrJmy4MU"
        );
    }

    #[test]
    fn decode_utf8_zlib_json_payload() {
        use {
            flate2::{write::ZlibEncoder, Compression},
            std::io::Write,
        };

        let input = br#"{"name":"example"}"#;
        let mut encoder = ZlibEncoder::new(Vec::new(), Compression::default());
        encoder.write_all(input).unwrap();
        let compressed = encoder.finish().unwrap();

        let decoded = PmpMetadataHeader {
            encoding: MetadataEncoding::Utf8,
            compression: MetadataCompression::Zlib,
            format: MetadataFormat::Json,
            data_source: MetadataDataSource::Direct,
        }
        .decode_direct(&compressed)
        .unwrap();

        assert_eq!(decoded, input);
    }

    #[test]
    fn parse_inline_set_data() {
        let instruction = [
            PmpInstruction::SetData as u8,
            1,
            0,
            MetadataFormat::Json as u8,
            MetadataDataSource::Direct as u8,
            b'{',
            b'}',
        ];
        let (header, payload) = parse_set_data(&instruction).unwrap();
        assert_eq!(
            header,
            PmpMetadataHeader {
                encoding: MetadataEncoding::Utf8,
                compression: MetadataCompression::None,
                format: MetadataFormat::Json,
                data_source: MetadataDataSource::Direct,
            }
        );
        assert_eq!(payload, b"{}");
    }

    #[test]
    fn ordered_buffer_writes_apply_newest_last() {
        let header_len = PMP_BUFFER_HEADER_SIZE;
        let initial = vec![0u8; header_len + 4];

        let out = apply_buffer_writes_to_bytes(vec![
            BufferWrite {
                offset: 0,
                bytes: initial,
            },
            BufferWrite {
                offset: header_len,
                bytes: b"new!".to_vec(),
            },
        ])
        .unwrap();

        assert_eq!(out, b"new!");
    }

    #[test]
    fn parse_initialize_inline_payload_is_raw() {
        let header_len = 1 + SEED_SIZE + 4;
        let mut data = vec![0u8; header_len];
        data[0] = PmpInstruction::Initialize as u8;
        data[1..1 + IDL_SEED.len()].copy_from_slice(IDL_SEED.as_bytes());
        data[1 + SEED_SIZE] = MetadataEncoding::Utf8 as u8;
        data[1 + SEED_SIZE + 1] = MetadataCompression::None as u8;
        data[1 + SEED_SIZE + 2] = MetadataFormat::Json as u8;
        data[1 + SEED_SIZE + 3] = MetadataDataSource::Direct as u8;
        data.extend_from_slice(b"{\"k\":1}");

        let (header, payload) = parse_initialize_data(&data).unwrap();
        assert_eq!(header.format, MetadataFormat::Json);
        assert_eq!(payload, b"{\"k\":1}");
    }

    #[test]
    fn parse_initialize_buffer_path_has_empty_payload() {
        let mut data = vec![0u8; 1 + SEED_SIZE + 4];
        data[0] = PmpInstruction::Initialize as u8;
        data[1..1 + IDL_SEED.len()].copy_from_slice(IDL_SEED.as_bytes());

        let (_, payload) = parse_initialize_data(&data).unwrap();
        assert!(payload.is_empty());
    }
}
