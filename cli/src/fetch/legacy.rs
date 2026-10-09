use {
    super::{
        decompress::decompress_all_streams,
        history::{HistoricalIdlVersion, IdlHistorySource},
        sessions::group_chunks_into_sessions,
        DecompressedSessions, IdlFetcher, RpcConfirmedTransactionStatusWithSignature,
        SessionChunks, SlotChunk, PROGRESS_TICK_INTERVAL_MS,
    },
    anyhow::{anyhow, Result},
    indicatif::{ProgressBar, ProgressStyle},
    std::time::Duration,
};

// Concatenates the ordered chunk payloads that make up one legacy embedded-IDL upload session.
fn combine_chunks(chunks: &[SlotChunk]) -> Vec<u8> {
    chunks
        .iter()
        .flat_map(|(_, _, chunk)| chunk.iter())
        .copied()
        .collect()
}

// Pulls all legacy chunk writes out of the provided signatures and orders them by slot so session
// grouping can rebuild each historical upload in chronological order.
fn collect_and_process_chunks(
    fetcher: &IdlFetcher,
    signatures: &[RpcConfirmedTransactionStatusWithSignature],
    pb: &ProgressBar,
) -> Vec<SlotChunk> {
    let mut all_chunks = fetcher.collect_chunks_owned(signatures, pb);
    all_chunks.sort_by(|a, b| a.0.cmp(&b.0).then_with(|| a.1.cmp(&b.1)));
    all_chunks
}

// Decompresses each reconstructed legacy session and attaches the slot/signature metadata that the
// merged historical timeline needs later on.
fn decompress_sessions(
    sessions: &[SessionChunks],
    pb: &ProgressBar,
) -> Result<DecompressedSessions> {
    let mut failed = 0usize;
    let mut extracted = Vec::new();

    for session in sessions {
        pb.inc(1);
        let combined_data = combine_chunks(session);
        let streams = decompress_all_streams(&combined_data);
        if streams.is_empty() {
            failed += 1;
            continue;
        }

        let (session_slot, session_sig) = session
            .first()
            .map(|(slot, signature, _)| (*slot, signature.clone()))
            .ok_or_else(|| {
                anyhow!("could not reconstruct an IDL upload from the fetched transactions")
            })?;

        // A single legacy upload session can contain multiple complete IDL streams, so emit one
        // historical version per recovered stream while keeping the session slot/signature.
        extracted.extend(streams.into_iter().map(|idl_data| HistoricalIdlVersion {
            slot: session_slot,
            signature: session_sig.clone(),
            source: IdlHistorySource::Legacy,
            idl_data,
        }));
    }

    Ok(DecompressedSessions {
        extracted_idls: extracted,
        skipped_sessions: failed,
    })
}

// Reconstructs all recoverable historical IDLs from the legacy embedded-IDL account path while
// preserving the existing chunk/session/zlib behavior behind one helper.
pub(super) fn fetch_legacy_historical_idls(
    fetcher: &IdlFetcher,
    signatures: &[RpcConfirmedTransactionStatusWithSignature],
    verbose: bool,
) -> Result<Vec<HistoricalIdlVersion>> {
    if signatures.is_empty() {
        return Ok(Vec::new());
    }

    if verbose {
        println!(
            "Processing {} transactions on the legacy IDL account...",
            signatures.len()
        );
    }

    // Preserve the existing legacy reconstruction flow and convert its output into the shared
    // history representation used by the merge step.
    let pb = ProgressBar::new(signatures.len() as u64);
    pb.enable_steady_tick(Duration::from_millis(PROGRESS_TICK_INTERVAL_MS));
    pb.set_style(
        ProgressStyle::default_bar()
            .template(
                "{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len} \
                 transactions ({eta})",
            )
            .unwrap()
            .progress_chars("#>-"),
    );
    pb.set_message("Extracting legacy IDL chunks from transactions...");

    let all_chunks = collect_and_process_chunks(fetcher, signatures, &pb);

    pb.finish_with_message("Legacy transaction processing complete");

    if all_chunks.is_empty() {
        return Ok(Vec::new());
    }

    if verbose {
        println!(
            "Grouping {} legacy chunks into sessions...",
            all_chunks.len()
        );
    }
    let sessions = group_chunks_into_sessions(&all_chunks);
    let decompress_pb = ProgressBar::new(sessions.len() as u64);
    decompress_pb.enable_steady_tick(Duration::from_millis(PROGRESS_TICK_INTERVAL_MS));
    decompress_pb.set_style(
        ProgressStyle::default_bar()
            .template(
                "{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len} sessions",
            )
            .unwrap()
            .progress_chars("#>-"),
    );
    decompress_pb.set_message("Decompressing reconstructed legacy upload sessions...");
    let decompressed = decompress_sessions(&sessions, &decompress_pb)?;
    decompress_pb.finish_with_message("Legacy decompression complete");

    if decompressed.skipped_sessions > 0 {
        println!(
            "Skipped {}/{} legacy session(s): no valid zlib streams found (partial uploads)",
            decompressed.skipped_sessions,
            sessions.len()
        );
    }

    Ok(decompressed.extracted_idls)
}
