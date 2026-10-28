use {
    super::FetchTuning,
    crate::{
        compat::{
            solana_pubkey, solana_rpc_client, solana_rpc_client_api,
            solana_transaction_status_client_types,
        },
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

/// Paginates signatures for any account address without applying
/// IDL-account-specific derivation.
///
/// `max_signatures` is a hard cap on collected results. Pagination stops once
/// the cap is reached and a warning is printed so the caller knows older
/// history may have been truncated. If `0` is passed as the cap, no fetch will
/// be done and an empty vector will be immediately returned.
pub(super) fn fetch_signatures_for_address(
    client: &RpcClient,
    address: &Pubkey,
    before_timestamp: Option<i64>,
    after_timestamp: Option<i64>,
    target_slot: Option<u64>,
    max_signatures: usize,
) -> Result<Vec<RpcConfirmedTransactionStatusWithSignature>> {
    if max_signatures == 0 {
        return Ok(Vec::new());
    }
    let mut signatures = Vec::new();
    let mut cursor: Option<Signature> = None;
    let mut cap_reached = false;

    // Mirror the legacy pagination behavior so date/slot filtering stays consistent across both
    // history sources before they are merged.
    loop {
        let config = GetConfirmedSignaturesForAddress2Config {
            before: cursor,
            until: None,
            limit: Some(IDL_SIGNATURE_PAGE_SIZE.min(max_signatures - signatures.len())),
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
            signatures.push(sig);
            if signatures.len() >= max_signatures {
                cap_reached = true;
                break;
            }
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
    // V1 transactions only exist in the `solana-v4` cohort; a `solana-v3` build cannot
    // decode one, so it must not ask the RPC for them.
    let max_supported_transaction_version = if cfg!(feature = "solana-v3") {
        Some(0)
    } else if cfg!(feature = "solana-v4") {
        Some(1)
    } else {
        unreachable!("exactly one solana cohort must be activated")
    };

    let config = RpcTransactionConfig {
        encoding: Some(UiTransactionEncoding::Json),
        commitment: Some(CommitmentConfig::confirmed()),
        max_supported_transaction_version,
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

#[cfg(test)]
mod tests {
    use {
        super::*,
        std::{
            io::{Read, Write},
            net::TcpListener,
            sync::{
                atomic::{AtomicBool, Ordering},
                Arc, Mutex,
            },
            time::{Duration, Instant},
        },
    };

    fn capped_history(maximum: usize) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        let stopped = Arc::new(AtomicBool::new(false));
        let requests = Arc::new(Mutex::new(Vec::new()));
        let server_stopped = stopped.clone();
        let server_requests = requests.clone();
        let signature = Signature::from([7u8; 64]).to_string();
        let returned_signature = signature.clone();
        let server = std::thread::spawn(move || {
            let deadline = Instant::now() + Duration::from_secs(5);
            while !server_stopped.load(Ordering::SeqCst) && Instant::now() < deadline {
                let (mut stream, _) = match listener.accept() {
                    Ok(value) => value,
                    Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                        std::thread::sleep(Duration::from_millis(5));
                        continue;
                    }
                    Err(error) => panic!("accept test RPC: {error}"),
                };
                stream.set_nonblocking(false).unwrap();
                stream
                    .set_read_timeout(Some(Duration::from_secs(2)))
                    .unwrap();
                let mut bytes = Vec::new();
                let (body_start, length) = loop {
                    let mut buffer = [0u8; 4096];
                    let size = stream.read(&mut buffer).unwrap();
                    assert!(size > 0, "test RPC request closed before headers");
                    bytes.extend_from_slice(&buffer[..size]);
                    if let Some(end) = bytes.windows(4).position(|value| value == b"\r\n\r\n") {
                        let headers = std::str::from_utf8(&bytes[..end])
                            .unwrap()
                            .to_ascii_lowercase();
                        let length = headers
                            .lines()
                            .find_map(|line| line.strip_prefix("content-length:"))
                            .unwrap()
                            .trim()
                            .parse::<usize>()
                            .unwrap();
                        break (end + 4, length);
                    }
                };
                while bytes.len() < body_start + length {
                    let mut buffer = [0u8; 4096];
                    let size = stream.read(&mut buffer).unwrap();
                    assert!(size > 0, "test RPC request closed before body");
                    bytes.extend_from_slice(&buffer[..size]);
                }
                let request: serde_json::Value =
                    serde_json::from_slice(&bytes[body_start..body_start + length]).unwrap();
                assert_eq!(request["method"], "getSignaturesForAddress");
                let mut held = server_requests.lock().unwrap();
                held.push(request.clone());
                let response = if held.len() == 1 {
                    serde_json::json!({"jsonrpc":"2.0","id":request["id"],"result":[{"signature":returned_signature,"slot":10,"err":null,"memo":null,"blockTime":1,"confirmationStatus":"finalized"}]})
                } else {
                    serde_json::json!({"jsonrpc":"2.0","id":request["id"],"error":{"code":-32602,"message":"unexpected request beyond signature cap"}})
                }.to_string();
                write!(
                    stream,
                    "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: \
                     {}\r\nConnection: close\r\n\r\n{}",
                    response.len(),
                    response
                )
                .unwrap();
            }
        });
        let client = RpcClient::new_with_timeout(url, Duration::from_secs(2));
        let result =
            fetch_signatures_for_address(&client, &Pubkey::new_unique(), None, None, None, maximum);
        stopped.store(true, Ordering::SeqCst);
        server.join().unwrap();
        let rows = result.unwrap();
        let requests = requests.lock().unwrap();
        if maximum == 0 {
            assert!(rows.is_empty());
            assert!(
                requests.is_empty(),
                "zero cap must not make a provider request"
            );
        } else {
            assert_eq!(rows.len(), 1);
            assert_eq!(rows[0].signature, signature);
            assert_eq!(requests.len(), 1, "stop after collecting the cap");
            assert_eq!(
                requests[0]["params"][1]["limit"], 1,
                "bound the RPC page itself"
            );
        }
    }

    #[test]
    fn history_signature_cap_bounds_the_rpc_page_and_stops_at_the_cap() {
        capped_history(1);
    }

    #[test]
    fn history_signature_zero_cap_never_calls_the_provider() {
        capped_history(0);
    }
}
