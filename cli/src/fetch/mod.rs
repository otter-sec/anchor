use {
    crate::config::ConfigOverride,
    anyhow::{anyhow, Result},
    indicatif::ProgressBar,
    solana_pubkey::Pubkey,
    solana_rpc_client::rpc_client::RpcClient,
    solana_rpc_client_api::response::RpcConfirmedTransactionStatusWithSignature,
    solana_signature::Signature,
    std::{path::PathBuf, str::FromStr, thread},
};

mod chunks;
mod decompress;
mod history;
mod legacy;
mod output;
mod parallel;
mod pmp;
mod rpc;
mod sessions;

use self::{
    chunks::extract_chunks_from_transaction,
    history::{merge_historical_idls, HistoricalIdlVersion, IdlHistorySource},
    legacy::fetch_legacy_historical_idls,
    output::{save_historical_idls, write_idl_file},
    parallel::{historical_fetch_worker_count, should_parallelize_historical_fetch},
    pmp::fetch_pmp_historical_idls,
    rpc::{create_rpc_client, fetch_idl_signatures, fetch_pmp_idl_signatures},
};

const DEFAULT_MAX_RETRIES: u32 = 5;
const DEFAULT_RETRY_BACKOFF_MS: u64 = 500;
const PROGRESS_TICK_INTERVAL_MS: u64 = 80;
// Default cap on signatures pulled per history source so pagination cannot accumulate an
// unbounded `Vec` against a program with deep history.
const DEFAULT_MAX_SIGNATURES: usize = 1000;

// Shared safety bound for any historical-IDL buffer the fetcher allocates, whether reassembling
// compressed PMP buffer writes or holding a decompressed legacy/PMP stream. Real anchor IDL JSON
// stays well under this limit; anything larger is rejected as malicious or corrupt.
pub(super) const MAX_IDL_BUFFER_BYTES: usize = 16 * 1024 * 1024;

type ChunkData = Vec<u8>;
type SlotChunk = (u64, String, ChunkData);
type SessionChunks = Vec<SlotChunk>;
type ExtractedIdl = HistoricalIdlVersion;

struct DecompressedSessions {
    extracted_idls: Vec<ExtractedIdl>,
    skipped_sessions: usize,
}

#[derive(Clone, Copy, Debug)]
pub struct FetchTuning {
    pub workers: Option<usize>,
    pub no_parallel: bool,
    pub max_retries: u32,
    pub retry_backoff_ms: u64,
    pub max_signatures: usize,
    pub verbose: bool,
}

impl Default for FetchTuning {
    fn default() -> Self {
        Self {
            workers: None,
            no_parallel: false,
            max_retries: DEFAULT_MAX_RETRIES,
            retry_backoff_ms: DEFAULT_RETRY_BACKOFF_MS,
            max_signatures: DEFAULT_MAX_SIGNATURES,
            verbose: false,
        }
    }
}

pub struct IdlFetcher<'a> {
    client: &'a RpcClient,
    tuning: FetchTuning,
}

impl<'a> IdlFetcher<'a> {
    // Binds a fetch tuning profile to a shared RPC client for one fetch run.
    fn new(client: &'a RpcClient, tuning: FetchTuning) -> Self {
        Self { client, tuning }
    }

    // Rejects slot queries that point past the cluster's current slot.
    fn validate_slot(&self, target_slot: u64) -> Result<()> {
        let current_slot = self.client.get_slot()?;
        if target_slot > current_slot {
            return Err(anyhow::format_err!(
                "Target slot {} is greater than the current slot {}. Cannot fetch IDL from a \
                 future slot.",
                target_slot,
                current_slot
            ));
        }
        Ok(())
    }

    // Collects IDL chunks for a borrowed slice of signatures on the current thread.
    fn collect_chunks(
        &self,
        signatures: &[&RpcConfirmedTransactionStatusWithSignature],
        pb: &ProgressBar,
    ) -> Vec<SlotChunk> {
        signatures
            .iter()
            .filter_map(|sig| {
                pb.inc(1);
                collect_signature_chunks(self.client, sig, &self.tuning, pb)
            })
            .flatten()
            .collect()
    }

    // Chooses sequential or parallel collection for an owned signature page.
    fn collect_chunks_owned(
        &self,
        signatures: &[RpcConfirmedTransactionStatusWithSignature],
        pb: &ProgressBar,
    ) -> Vec<SlotChunk> {
        if should_parallelize_historical_fetch(signatures.len(), &self.tuning) {
            return self.collect_chunks_owned_parallel(signatures, pb);
        }

        let refs: Vec<&RpcConfirmedTransactionStatusWithSignature> = signatures.iter().collect();
        self.collect_chunks(&refs, pb)
    }

    // Splits the signature list across worker threads and merges recovered chunks.
    fn collect_chunks_owned_parallel(
        &self,
        signatures: &[RpcConfirmedTransactionStatusWithSignature],
        pb: &ProgressBar,
    ) -> Vec<SlotChunk> {
        let worker_count = historical_fetch_worker_count(signatures.len(), &self.tuning);
        if worker_count <= 1 {
            let refs: Vec<&RpcConfirmedTransactionStatusWithSignature> =
                signatures.iter().collect();
            return self.collect_chunks(&refs, pb);
        }

        let chunk_size = signatures.len().div_ceil(worker_count);

        thread::scope(|scope| {
            let mut handles = Vec::new();

            for signature_chunk in signatures.chunks(chunk_size) {
                let progress = pb.clone();
                handles.push(scope.spawn(move || {
                    signature_chunk
                        .iter()
                        .filter_map(|sig| {
                            progress.inc(1);
                            collect_signature_chunks(self.client, sig, &self.tuning, &progress)
                        })
                        .flatten()
                        .collect::<Vec<_>>()
                }));
            }

            handles
                .into_iter()
                .flat_map(|handle| handle.join().expect("IDL fetch worker panicked"))
                .collect()
        })
    }
}

// Extracts slot-tagged chunks for one transaction and reports per-signature failures
// through the shared progress bar.
fn collect_signature_chunks(
    client: &RpcClient,
    sig: &RpcConfirmedTransactionStatusWithSignature,
    tuning: &FetchTuning,
    pb: &ProgressBar,
) -> Option<Vec<SlotChunk>> {
    let signature = Signature::from_str(&sig.signature).ok()?;
    let chunks = match extract_chunks_from_transaction(client, &signature, tuning) {
        Ok(chunks) => chunks,
        Err(e) => {
            pb.println(format!("{e}"));
            return None;
        }
    };

    if chunks.is_empty() {
        None
    } else {
        Some(
            chunks
                .into_iter()
                .map(|chunk| (sig.slot, sig.signature.clone(), chunk))
                .collect::<Vec<_>>(),
        )
    }
}

// Parses a CLI date filter into the UTC timestamp used by signature pagination.
fn parse_date_to_timestamp(date_str: &str) -> Result<i64> {
    use chrono::NaiveDate;

    let date = NaiveDate::parse_from_str(date_str, "%Y-%m-%d").map_err(|e| {
        anyhow!(
            "Invalid date format '{}'. Expected YYYY-MM-DD: {}",
            date_str,
            e
        )
    })?;

    let datetime = date
        .and_hms_opt(0, 0, 0)
        .ok_or_else(|| anyhow!("Failed to create datetime from date"))?;

    Ok(datetime.and_utc().timestamp())
}

// Fetches all historical IDL uploads matching the requested slot/date filters.
#[allow(clippy::too_many_arguments)]
pub fn idl_fetch_historical(
    cfg_override: &ConfigOverride,
    address: Pubkey,
    authority: Option<Pubkey>,
    slot: Option<u64>,
    before: Option<String>,
    after: Option<String>,
    out_dir: Option<PathBuf>,
    tuning: FetchTuning,
) -> Result<()> {
    let before_timestamp = before.as_deref().map(parse_date_to_timestamp).transpose()?;
    let after_timestamp = after.as_deref().map(parse_date_to_timestamp).transpose()?;
    if let Some((after_ts, before_ts)) = after_timestamp.zip(before_timestamp) {
        if after_ts > before_ts {
            // `zip` only yields `Some` when both original CLI arguments were present and parsed,
            // so these borrowed flag values are guaranteed to exist here.
            let after_value = after.as_deref().unwrap();
            let before_value = before.as_deref().unwrap();
            return Err(anyhow!(
                "Invalid date range: --after ({}) must be on or before --before ({})",
                after_value,
                before_value,
            ));
        }
    }

    let client = create_rpc_client(cfg_override)?;
    let fetcher = IdlFetcher::new(&client, tuning);

    // Validate the target slot before paginating signatures
    if let Some(target_slot) = slot {
        fetcher.validate_slot(target_slot)?;
    }

    let (filter_before, filter_after) = if slot.is_some() {
        (None, None)
    } else {
        (before_timestamp, after_timestamp)
    };

    let legacy_signatures = if authority.is_none() {
        fetch_idl_signatures(
            &client,
            &address,
            filter_before,
            filter_after,
            slot,
            tuning.max_signatures,
        )?
    } else {
        Vec::new()
    };
    let pmp_signatures = fetch_pmp_idl_signatures(
        &client,
        &address,
        authority.as_ref(),
        filter_before,
        filter_after,
        slot,
        tuning.max_signatures,
    )?;

    if legacy_signatures.is_empty() && pmp_signatures.is_empty() {
        if let Some(authority) = authority {
            println!(
                "No historical IDLs found for authority-scoped metadata account {} on program {}",
                authority, address
            );
        } else {
            println!("The program doesn't have an IDL account");
        }
        return Ok(());
    }
    if tuning.verbose {
        println!(
            "Found {} legacy transactions and {} PMP transactions",
            legacy_signatures.len(),
            pmp_signatures.len()
        );
    }

    // An explicit authority means "only inspect that authority-scoped PMP metadata account".
    // Without an authority filter we merge legacy embedded-IDL history with canonical PMP
    // history so historical fetch stays source-agnostic for the common path.
    let mut historical_idls =
        fetch_legacy_historical_idls(&fetcher, &legacy_signatures, tuning.verbose)?;
    let pmp_idls = fetch_pmp_historical_idls(
        &client,
        &address,
        authority.as_ref(),
        &pmp_signatures,
        &tuning,
    );
    for warning in &pmp_idls.warnings {
        if tuning.verbose
            || matches!(
                warning.kind,
                pmp::PmpHistoryWarningKind::RpcError | pmp::PmpHistoryWarningKind::MissingBuffer
            )
        {
            println!("Skipping PMP slot {}: {}", warning.slot, warning.detail);
        }
    }
    // Merge the two sources together so the rest of the flow can treat them uniformly, preferring PMP
    historical_idls = merge_historical_idls(
        historical_idls,
        pmp_idls
            .idls
            .into_iter()
            .map(|idl| HistoricalIdlVersion {
                slot: idl.slot,
                signature: idl.signature,
                source: IdlHistorySource::Pmp,
                idl_data: idl.idl_data,
            })
            .collect(),
    );

    if historical_idls.is_empty() {
        if let Some(authority) = authority {
            println!(
                "\nNo recoverable historical IDLs found for authority-scoped metadata account {} \
                 on program {}.",
                authority, address
            );
        } else {
            println!("\nNo IDL data could be fetched from historical slots.");
        }
        return Ok(());
    }

    if let Some(target_slot) = slot {
        if let Some(selected) = historical_idls
            .iter()
            .find(|entry| entry.slot <= target_slot)
        {
            return write_idl_file(
                &selected.idl_data,
                &PathBuf::from(format!("idl_{}.json", target_slot)),
                out_dir.as_deref(),
            );
        }
        println!(
            "\nNo IDL upload session or PMP metadata update found at or before slot {}.",
            target_slot
        );
        return Ok(());
    }

    println!(
        "\nSuccessfully extracted {} IDL version(s)",
        historical_idls.len()
    );
    save_historical_idls(&historical_idls, out_dir)
}
