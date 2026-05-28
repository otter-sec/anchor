use {
    super::FetchTuning,
    crate::{
        config::{get_solana_cfg_url, Config, ConfigOverride},
        fetch::pmp::pmp_metadata_address,
        get_cluster_and_wallet,
    },
    anyhow::{anyhow, Result},
    solana_commitment_config::CommitmentConfig,
    solana_pubkey::Pubkey,
    solana_rpc_client::rpc_client::{GetConfirmedSignaturesForAddress2Config, RpcClient},
    solana_rpc_client_api::{
        client_error::{reqwest::StatusCode, ErrorKind as RpcClientErrorKind},
        config::RpcTransactionConfig,
        response::RpcConfirmedTransactionStatusWithSignature,
    },
    solana_signature::Signature,
    solana_transaction_status_client_types::{
        EncodedConfirmedTransactionWithStatusMeta, UiTransactionEncoding,
    },
    std::str::FromStr,
};

const IDL_SIGNATURE_PAGE_SIZE: usize = 100;

// Builds the RPC client used for historical IDL fetches from CLI cluster overrides.
pub(super) fn create_rpc_client(cfg_override: &ConfigOverride) -> Result<RpcClient> {
    // Match `anchor idl fetch` cluster resolution so historical fetch talks to the same localnet
    // validator that the rest of the IDL CLI commands use.
    let workspace_cluster = Config::discover(cfg_override)?
        .map(|_| get_cluster_and_wallet(cfg_override))
        .transpose()?;
    let url = match workspace_cluster {
        Some(cluster_url) => cluster_url.0,
        None => {
            if let Some(cluster) = cfg_override.cluster.as_ref() {
                cluster.url().to_string()
            } else {
                get_solana_cfg_url()?
            }
        }
    };
    Ok(crate::create_client(url))
}

// Paginates the IDL account history and applies optional date bounds during collection.
pub(super) fn fetch_idl_signatures(
    client: &RpcClient,
    address: &Pubkey,
    before_timestamp: Option<i64>,
    after_timestamp: Option<i64>,
    target_slot: Option<u64>,
    max_signatures: usize,
) -> Result<Vec<RpcConfirmedTransactionStatusWithSignature>> {
    let program_signer = Pubkey::find_program_address(&[], address).0;
    let idl_account_address = Pubkey::create_with_seed(&program_signer, "anchor:idl", address)
        .map_err(|e| anyhow!("Failed to derive IDL account address: {e}"))?;
    fetch_signatures_for_address(
        client,
        &idl_account_address,
        before_timestamp,
        after_timestamp,
        target_slot,
        max_signatures,
    )
}

// Derives the PMP metadata PDA for the requested authority scope and paginates its history.
pub(super) fn fetch_pmp_idl_signatures(
    client: &RpcClient,
    program_id: &Pubkey,
    authority: Option<&Pubkey>,
    before_timestamp: Option<i64>,
    after_timestamp: Option<i64>,
    target_slot: Option<u64>,
    max_signatures: usize,
) -> Result<Vec<RpcConfirmedTransactionStatusWithSignature>> {
    let metadata_address = pmp_metadata_address(program_id, authority);
    fetch_signatures_for_address(
        client,
        &metadata_address,
        before_timestamp,
        after_timestamp,
        target_slot,
        max_signatures,
    )
}

// Paginates signatures for any account address without applying IDL-account-specific derivation.
//
// `max_signatures` is a hard cap on collected results. Pagination stops once the cap is reached
// and a warning is printed so the caller knows older history may have been truncated. The cap
// must be greater than zero; passing `0` returns an empty result rather than meaning
// "unbounded".
pub(super) fn fetch_signatures_for_address(
    client: &RpcClient,
    address: &Pubkey,
    before_timestamp: Option<i64>,
    after_timestamp: Option<i64>,
    target_slot: Option<u64>,
    max_signatures: usize,
) -> Result<Vec<RpcConfirmedTransactionStatusWithSignature>> {
    let mut signatures = Vec::new();
    let mut cursor: Option<Signature> = None;
    let mut cap_reached = false;

    // Mirror the legacy pagination behavior so date/slot filtering stays consistent across both
    // history sources before they are merged.
    loop {
        let config = GetConfirmedSignaturesForAddress2Config {
            before: cursor,
            until: None,
            limit: Some(IDL_SIGNATURE_PAGE_SIZE),
            // The server defaults `commitment: None` to finalized, which lags fresh
            // localnet transactions; honor the client's configured commitment instead.
            commitment: Some(client.commitment()),
        };
        let page = client.get_signatures_for_address_with_config(address, config)?;

        if page.is_empty() {
            break;
        }

        let next_cursor = page
            .last()
            .and_then(|sig| Signature::from_str(&sig.signature).ok());

        let has_date_filter = before_timestamp.is_some() || after_timestamp.is_some();
        let mut crossed_after_bound = false;
        for sig in page {
            if sig.err.is_some() {
                continue;
            }
            if target_slot.is_some_and(|slot| sig.slot > slot) {
                continue;
            }
            if has_date_filter {
                let Some(bt) = sig.block_time else { continue };
                if before_timestamp.is_some_and(|ts| bt > ts) {
                    continue;
                }
                if after_timestamp.is_some_and(|ts| bt < ts) {
                    crossed_after_bound = true;
                    continue;
                }
            }
            if signatures.len() >= max_signatures {
                cap_reached = true;
                break;
            }
            signatures.push(sig);
        }

        // Stop paginating once the user-configured signature cap has been hit
        if cap_reached {
            break;
        }

        // For slot-based historical fetches we still need to keep paginating after crossing the
        // target slot, because legacy embedded-IDL uploads span multiple older transactions and
        // truncating the history here can leave only partial chunk sessions to decompress.
        if crossed_after_bound {
            break;
        }
        match next_cursor {
            Some(sig) => cursor = Some(sig),
            None => break,
        }
    }

    if cap_reached {
        eprintln!(
            "warning: reached --max-signatures cap of {} for account {}; older history was not \
             scanned. Pass a higher --max-signatures to fetch more.",
            max_signatures, address
        );
    }

    Ok(signatures)
}

// Fetches one transaction with retry/backoff handling for rate-limited RPC responses.
pub(super) fn fetch_transaction(
    client: &RpcClient,
    signature: &Signature,
    tuning: &FetchTuning,
) -> Result<EncodedConfirmedTransactionWithStatusMeta> {
    let config = RpcTransactionConfig {
        encoding: Some(UiTransactionEncoding::Json),
        commitment: Some(CommitmentConfig::confirmed()),
        max_supported_transaction_version: Some(0),
    };

    let mut attempt: u32 = 0;
    loop {
        attempt += 1;
        match client.get_transaction_with_config(signature, config) {
            Ok(tx) => return Ok(tx),
            Err(e) => {
                let retryable = matches!(
                    e.kind(),
                    RpcClientErrorKind::Reqwest(error)
                        if error.status() == Some(StatusCode::TOO_MANY_REQUESTS)
                );
                if !retryable || attempt >= tuning.max_retries {
                    return Err(anyhow!("failed to fetch transaction {signature}: {e}"));
                }
                let shift = (attempt - 1).min(20);
                let backoff = tuning.retry_backoff_ms.saturating_mul(1u64 << shift);
                std::thread::sleep(std::time::Duration::from_millis(backoff));
            }
        }
    }
}
