use {
    crate::{
        config::{Config, Program, WithPath},
        target_dir, ConfigOverride, ProgramCommand, DEFAULT_MAX_SIGN_ATTEMPTS,
    },
    anchor_lang_idl::types::Idl,
    anyhow::{anyhow, bail, Result},
    cargo_metadata::{Metadata, MetadataCommand, Package, TargetKind},
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
    solana_keypair::Keypair,
    solana_loader_v3_interface::{
        instruction::{self as loader_v3_instruction, MINIMUM_EXTEND_PROGRAM_BYTES},
        state::UpgradeableLoaderState,
    },
    solana_message::{Hash, Message},
    solana_packet::PACKET_DATA_SIZE,
    solana_pubkey::Pubkey,
    solana_rpc_client::rpc_client::RpcClient,
    solana_rpc_client_api::{
        config::{RpcSendTransactionConfig, RpcSimulateTransactionConfig},
        response::RpcSimulateTransactionResult,
    },
    solana_sdk_ids::{
        bpf_loader_upgradeable as bpf_loader_upgradeable_id,
        compute_budget as compute_budget_program_id,
    },
    solana_signature::Signature,
    solana_signer::{EncodableKey, Signer},
    solana_system_interface::MAX_PERMITTED_DATA_LENGTH,
    solana_transaction::Transaction,
    std::{
        collections::{BTreeMap, HashSet},
        fs::{self, File},
        io::Write,
        path::{Path, PathBuf},
        sync::Arc,
        thread,
        time::Duration,
    },
};

/// Outer retry cap on the full deploy/upgrade cycle; inner per-batch resign is `max_sign_attempts`.
const MAX_DEPLOY_ATTEMPTS: u32 = 3;

/// Tight CU limit per Write tx (~2,670 actual + headroom) so priority-fee-per-CU is competitive.
const WRITE_COMPUTE_UNIT_LIMIT: u32 = 3_000;

/// Max seconds `wait_for_buffer_stable` polls before giving up and using
/// the latest snapshot.
const BUFFER_STABILIZE_MAX_WAIT_SECS: u64 = 60;

/// If `--buffer` is absent, inject a per-program persistent path at
/// `target/deploy/{program_name}-upgrade-buffer.json`. Creates the keypair
/// file on first run; subsequent runs reuse it so a failed deploy/upgrade
/// resumes automatically (the on-chain buffer at that pubkey carries the
/// partial bytes, and `write_program_buffer` diffs against it).
fn ensure_buffer_keypair_arg(mut args: Vec<String>, program_name: &str) -> Result<Vec<String>> {
    if args.iter().any(|a| a == "--buffer") {
        return Ok(args);
    }
    let deploy_dir = target_dir()?.join("deploy");
    if deploy_dir.exists() && !deploy_dir.is_dir() {
        bail!(
            "Cannot create deploy dir at {}: path exists but is not a directory",
            deploy_dir.display()
        );
    }
    std::fs::create_dir_all(&deploy_dir).map_err(|e| {
        anyhow!(
            "Failed to create deploy dir {}: {}",
            deploy_dir.display(),
            e
        )
    })?;
    let path = deploy_dir.join(format!("{program_name}-upgrade-buffer.json"));
    if !path.exists() {
        Keypair::new().write_to_file(&path).map_err(|e| {
            anyhow!(
                "Failed to write buffer keypair to {}: {e:?}",
                path.display()
            )
        })?;
    }
    args.push("--buffer".to_owned());
    args.push(path.to_string_lossy().into_owned());
    Ok(args)
}

/// Parse priority fee from solana args
fn parse_priority_fee_from_args(args: &[String]) -> Option<u64> {
    args.windows(2)
        .find(|pair| pair[0] == "--with-compute-unit-price")
        .and_then(|pair| pair[1].parse().ok())
}

/// Parse `--max-sign-attempts` from solana args, falling back to the default.
fn parse_max_sign_attempts_from_args(args: &[String]) -> usize {
    args.windows(2)
        .find(|pair| pair[0] == "--max-sign-attempts")
        .and_then(|pair| pair[1].parse().ok())
        .unwrap_or(DEFAULT_MAX_SIGN_ATTEMPTS)
}

/// Opt-in: skip RPC preflight on chunked write txs.
/// Default false to match Agave's `solana program deploy` behavior.
fn parse_skip_preflight_from_args(args: &[String]) -> bool {
    args.iter().any(|a| a == "--skip-preflight")
}

/// Parse `--buffer <path>` from solana_args; `ensure_buffer_keypair_arg` injects it when missing.
fn parse_buffer_keypair_path_from_args(args: &[String]) -> Option<PathBuf> {
    args.windows(2)
        .find(|pair| pair[0] == "--buffer")
        .map(|pair| PathBuf::from(&pair[1]))
}

/// Load the persistent buffer keypair from the path in solana_args; `None` if `--buffer` absent.
fn read_buffer_keypair_from_args(args: &[String]) -> Result<Option<Keypair>> {
    let Some(path) = parse_buffer_keypair_path_from_args(args) else {
        return Ok(None);
    };
    let kp = Keypair::read_from_file(&path).map_err(|e| {
        anyhow!(
            "Failed to read buffer keypair from {}: {}",
            path.display(),
            e
        )
    })?;
    Ok(Some(kp))
}

/// Existing on-chain buffer payload + capacity (bytes available for program
/// data, excluding header). Capacity matters for resume: the loader copies the
/// whole buffer body into ProgramData, so stale tail bytes must not remain.
pub struct ExistingBuffer {
    pub data: Vec<u8>,
    pub capacity: usize,
}

/// Fetch buffer body + capacity. `Ok(None)` only when RPC confirms absent; transport errors return `Err`.
fn fetch_buffer_program_data(
    rpc_client: &RpcClient,
    buffer_pubkey: &Pubkey,
    expected_authority: &Pubkey,
) -> Result<Option<ExistingBuffer>> {
    // Match the commitment of our Write txs (`CommitmentConfig::confirmed`).
    let account = rpc_client
        .get_account_with_commitment(buffer_pubkey, CommitmentConfig::confirmed())
        .map_err(|e| anyhow!("Failed to fetch buffer account {}: {}", buffer_pubkey, e))?
        .value;
    let account = match account {
        Some(a) => a,
        None => return Ok(None),
    };
    if account.owner != bpf_loader_upgradeable_id::id() {
        return Err(anyhow!(
            "Account {} exists but is not owned by the BPF Upgradeable Loader",
            buffer_pubkey
        ));
    }
    let state: UpgradeableLoaderState = bincode::deserialize(&account.data)
        .map_err(|e| anyhow!("Failed to deserialize buffer {}: {}", buffer_pubkey, e))?;
    match state {
        UpgradeableLoaderState::Buffer { authority_address } => {
            if authority_address.is_none() {
                return Err(anyhow!("Buffer {} is immutable", buffer_pubkey));
            }
            if authority_address != Some(*expected_authority) {
                return Err(anyhow!(
                    "Buffer {} authority {:?} does not match expected {}",
                    buffer_pubkey,
                    authority_address,
                    expected_authority
                ));
            }
        }
        _ => {
            return Err(anyhow!(
                "Account {} is not a Buffer state account",
                buffer_pubkey
            ));
        }
    }
    let header_size = UpgradeableLoaderState::size_of_buffer_metadata();
    let capacity = account.data.len().saturating_sub(header_size);
    Ok(Some(ExistingBuffer {
        data: account.data[header_size..].to_vec(),
        capacity,
    }))
}

/// Wait until two consecutive buffer fetches match — proves nothing's
/// landing right now. Avoids re-sending chunks that prior in-flight txs
/// are about to write
fn wait_for_buffer_stable(
    rpc_client: &RpcClient,
    buffer_pubkey: &Pubkey,
    expected_authority: &Pubkey,
    max_wait_secs: u64,
) -> Result<Option<ExistingBuffer>> {
    let mut prev = fetch_buffer_program_data(rpc_client, buffer_pubkey, expected_authority)?;
    // Fresh deploy — no buffer means nothing in flight; return immediately.
    if prev.is_none() {
        return Ok(None);
    }
    let start = std::time::Instant::now();
    // First confirmation: back-to-back fetches with no sleep. Stable case
    // (no in-flight activity) finishes in ~2× RPC round-trip (~600ms on
    // devnet) — no wasted seconds when there's nothing to wait for.
    let mut sleep_secs: u64 = 0;
    let mut wait_notice_shown = false;
    loop {
        if sleep_secs > 0 {
            thread::sleep(Duration::from_secs(sleep_secs));
        }
        let current = fetch_buffer_program_data(rpc_client, buffer_pubkey, expected_authority)?;
        let stable = match (prev.as_ref(), current.as_ref()) {
            (Some(p), Some(c)) => p.data == c.data && p.capacity == c.capacity,
            (None, None) => true,
            _ => false,
        };
        if stable {
            return Ok(current);
        }
        if !wait_notice_shown {
            println!(
                "Buffer {} has in-flight writes from a prior run; waiting for state to stabilize \
                 before resume (up to {}s)…",
                buffer_pubkey, max_wait_secs
            );
            wait_notice_shown = true;
        }
        if start.elapsed().as_secs() >= max_wait_secs {
            return Ok(current);
        }
        prev = current;
        // After the back-to-back check confirmed activity, throttle to 3s
        // between polls so we don't hammer the RPC while in-flight txs land.
        sleep_secs = 3;
    }
}

/// Close an undersized buffer we own so the next attempt re-creates it at the correct size.
fn close_buffer_for_resize(
    rpc_client: &RpcClient,
    payer: &Keypair,
    buffer_pubkey: &Pubkey,
    authority: &Keypair,
    priority_fee: Option<u64>,
    skip_preflight: bool,
) -> Result<()> {
    let close_ix = loader_v3_instruction::close_any(
        buffer_pubkey,
        &payer.pubkey(),
        Some(&authority.pubkey()),
        None,
    );
    let mut ixs: Vec<Instruction> = Vec::with_capacity(2);
    if let Some(price) = priority_fee {
        if price > 0 {
            ixs.push(ComputeBudgetInstruction::set_compute_unit_price(price));
        }
    }
    ixs.push(close_ix);

    let blockhash = rpc_client.get_latest_blockhash()?;
    let tx = Transaction::new_signed_with_payer(
        &ixs,
        Some(&payer.pubkey()),
        &[payer, authority],
        blockhash,
    );
    // Pin preflight to confirmed — matches the blockhash commitment; finalized default rejects fresh blockhashes.
    rpc_client
        .send_and_confirm_transaction_with_spinner_and_config(
            &tx,
            CommitmentConfig::confirmed(),
            RpcSendTransactionConfig {
                skip_preflight,
                preflight_commitment: Some(CommitmentConfig::confirmed().commitment),
                ..RpcSendTransactionConfig::default()
            },
        )
        .map_err(|e| anyhow!("Failed to close mis-sized buffer {}: {}", buffer_pubkey, e))?;
    Ok(())
}

fn find_nearest_manifest_path(start_dir: &Path) -> Option<PathBuf> {
    start_dir
        .ancestors()
        .map(|path| path.join("Cargo.toml"))
        .find(|manifest_path| manifest_path.is_file())
}

fn discover_cargo_metadata(start_dir: &Path) -> Result<Option<Metadata>> {
    let Some(manifest_path) = find_nearest_manifest_path(start_dir) else {
        return Ok(None);
    };

    match MetadataCommand::new()
        .manifest_path(manifest_path)
        .no_deps()
        .exec()
    {
        Ok(metadata) => Ok(Some(metadata)),
        Err(cargo_metadata::Error::CargoMetadata { stderr }) => bail!(stderr),
        Err(err) => Err(err.into()),
    }
}

fn package_lib_name(package: &Package) -> Option<String> {
    package
        .targets
        .iter()
        .find(|target| target.kind.iter().any(|kind| kind == &TargetKind::CDyLib))
        .map(|target| target.name.clone())
}

fn discover_solana_programs_from_path(
    current_dir: &Path,
    program_name: Option<String>,
) -> Result<Vec<Program>> {
    let mut candidates = BTreeMap::new();
    let metadata = discover_cargo_metadata(current_dir)?;

    if let Some(metadata) = &metadata {
        let workspace_members = metadata.workspace_members.iter().collect::<HashSet<_>>();

        for package in &metadata.packages {
            if !workspace_members.contains(&package.id) {
                continue;
            }

            let Some(lib_name) = package_lib_name(package) else {
                continue;
            };
            let manifest_path = package.manifest_path.clone().into_std_path_buf();
            let path = manifest_path.parent().unwrap().to_path_buf();
            candidates.insert(path, lib_name);
        }
    }

    let mut programs = Vec::new();
    for (path, lib_name) in candidates {
        if let Some(ref name) = program_name {
            let matches = *name == lib_name || *name == path.file_name().unwrap().to_str().unwrap();
            if !matches {
                continue;
            }
        }

        let idl_filepath = target_dir()?
            .join("idl")
            .join(&lib_name)
            .with_extension("json");
        let idl = fs::read(idl_filepath)
            .ok()
            .and_then(|bytes| serde_json::from_reader(&*bytes).ok());

        programs.push(Program {
            lib_name,
            path: path.canonicalize()?,
            idl,
        });
    }

    Ok(programs)
}

/// Discover Solana programs from a non-Anchor Cargo workspace
pub fn discover_solana_programs(program_name: Option<String>) -> Result<Vec<Program>> {
    discover_solana_programs_from_path(&std::env::current_dir()?, program_name)
}

/// Get programs from workspace (Anchor or non-Anchor)
pub fn get_programs_from_workspace(
    cfg_override: &ConfigOverride,
    program_name: Option<String>,
) -> Result<Vec<Program>> {
    // First try Anchor workspace
    if let Some(cfg) = Config::discover(cfg_override)? {
        return cfg.get_programs(program_name);
    }

    // Fallback to non-Anchor Solana workspace
    let programs = discover_solana_programs(program_name.clone())?;

    if programs.is_empty() {
        if let Some(name) = program_name {
            return Err(anyhow!(
                "Program '{}' not found. Make sure you're in a Solana workspace (Anchor or \
                 non-Anchor), or provide a program filepath.",
                name
            ));
        } else {
            return Err(anyhow!(
                "No Solana programs found. Make sure you're in a Solana workspace (Anchor or \
                 non-Anchor), or provide a program filepath."
            ));
        }
    }

    Ok(programs)
}

/// Public entry point for deploying programs - validates and routes to appropriate handler
#[allow(clippy::too_many_arguments)]
pub fn process_deploy(
    cfg_override: &ConfigOverride,
    program_filepath: Option<PathBuf>,
    program_name: Option<String>,
    program_keypair: Option<PathBuf>,
    upgrade_authority: Option<String>,
    program_id: Option<Pubkey>,
    buffer: Option<Pubkey>,
    max_len: Option<usize>,
    use_rpc: bool,
    verifiable: bool,
    no_idl: bool,
    make_final: bool,
    solana_args: Vec<String>,
) -> Result<()> {
    // If explicit filepath provided, deploy single program
    if program_filepath.is_some() {
        return program_deploy(
            cfg_override,
            program_filepath,
            program_name,
            program_keypair,
            upgrade_authority,
            program_id,
            buffer,
            max_len,
            use_rpc,
            no_idl,
            make_final,
            solana_args,
        );
    }

    // Discover from workspace (Anchor or non-Anchor)
    let programs = get_programs_from_workspace(cfg_override, program_name.clone())?;

    // Multiple programs and no specific program requested -> deploy all
    if programs.len() > 1 && program_name.is_none() {
        // Validate that single-program options aren't used
        if program_id.is_some() {
            return Err(anyhow!(
                "Cannot specify --program-id when deploying multiple programs. Use --program-name \
                 to deploy a specific program."
            ));
        }
        if buffer.is_some() {
            return Err(anyhow!(
                "Cannot specify --buffer when deploying multiple programs. Use --program-name to \
                 deploy a specific program."
            ));
        }
        if upgrade_authority.is_some() {
            return Err(anyhow!(
                "Cannot specify --upgrade-authority when deploying multiple programs. Use \
                 --program-name to deploy a specific program."
            ));
        }
        if max_len.is_some() {
            return Err(anyhow!(
                "Cannot specify --max-len when deploying multiple programs. Use --program-name to \
                 deploy a specific program."
            ));
        }
        if program_keypair.is_some() {
            return Err(anyhow!(
                "Cannot specify --program-keypair when deploying multiple programs. Use \
                 --program-name to deploy a specific program."
            ));
        }

        // Delegate to deploy_workspace
        return deploy_workspace(
            cfg_override,
            None, // program_name - deploy all
            program_keypair,
            use_rpc,
            verifiable,
            no_idl,
            make_final,
            solana_args,
        );
    }

    // Single program or specific program requested -> deploy single
    program_deploy(
        cfg_override,
        program_filepath,
        program_name,
        program_keypair,
        upgrade_authority,
        program_id,
        buffer,
        max_len,
        use_rpc,
        no_idl,
        make_final,
        solana_args,
    )
}

/// Deploy all programs in workspace using native implementation
#[allow(clippy::too_many_arguments)]
fn deploy_workspace(
    cfg_override: &ConfigOverride,
    program_name: Option<String>,
    program_keypair: Option<PathBuf>,
    use_rpc: bool,
    verifiable: bool,
    no_idl: bool,
    make_final: bool,
    solana_args: Vec<String>,
) -> Result<()> {
    // Get programs from workspace (Anchor or non-Anchor)
    let programs = get_programs_from_workspace(cfg_override, program_name.clone())?;

    // For Cargo workspaces, we don't have cluster/wallet in config, so just print basic info
    if let Ok(Some(cfg)) = Config::discover(cfg_override) {
        // Anchor workspace - we have cluster/wallet config
        let url = crate::cluster_url(&cfg, &cfg.test_validator, &cfg.surfpool_config);
        let keypair = cfg.provider.wallet.to_string();
        println!("Deploying cluster: {url}");
        println!("Upgrade authority: {keypair}");
    } else {
        // Cargo workspace - cluster/wallet will come from Solana CLI config or flags
        println!("Deploying programs from Cargo workspace");
    }

    for program in programs {
        let binary_path = program.binary_path(verifiable)?;

        println!("\nDeploying program: {}", program.lib_name);

        let program_keypair_filepath = match &program_keypair {
            Some(path) => Some(path.clone()),
            None => {
                // Try to find program keypair
                let keypair_path = program.keypair_file().ok().map(|kp| kp.path().clone());
                keypair_path
            }
        };

        // Use the native program_deploy implementation
        program_deploy(
            cfg_override,
            Some(binary_path),
            None, // program_name - not needed since we have filepath
            program_keypair_filepath,
            None, // upgrade_authority - uses wallet
            None, // program_id - derived from keypair
            None, // buffer
            None, // max_len
            use_rpc,
            no_idl,
            make_final,
            solana_args.clone(),
        )?;
    }

    println!("\nDeploy success");
    Ok(())
}

// Main entry point for all program commands
pub fn program(cfg_override: &ConfigOverride, cmd: ProgramCommand) -> Result<()> {
    match cmd {
        ProgramCommand::Deploy {
            program_filepath,
            program_name,
            program_keypair,
            upgrade_authority,
            program_id,
            buffer,
            max_len,
            use_rpc,
            no_idl,
            make_final,
            solana_args,
        } => process_deploy(
            cfg_override,
            program_filepath,
            program_name,
            program_keypair,
            upgrade_authority,
            program_id,
            buffer,
            max_len,
            use_rpc,
            false, // verifiable
            no_idl,
            make_final,
            solana_args,
        ),
        ProgramCommand::WriteBuffer {
            program_filepath,
            program_name,
            buffer,
            buffer_authority,
            max_len: _max_len,
        } => program_write_buffer(
            cfg_override,
            program_filepath,
            program_name,
            buffer,
            buffer_authority,
        ),
        ProgramCommand::SetBufferAuthority {
            buffer,
            new_buffer_authority,
        } => program_set_buffer_authority(cfg_override, buffer, new_buffer_authority),
        ProgramCommand::SetUpgradeAuthority {
            program_id,
            new_upgrade_authority,
            new_upgrade_authority_signer,
            skip_new_upgrade_authority_signer_check,
            make_final,
            upgrade_authority,
        } => program_set_upgrade_authority(
            cfg_override,
            program_id,
            new_upgrade_authority,
            new_upgrade_authority_signer,
            skip_new_upgrade_authority_signer_check,
            make_final,
            upgrade_authority,
        ),
        ProgramCommand::Show {
            account,
            get_programs,
            get_buffers,
            all,
        } => program_show(cfg_override, account, get_programs, get_buffers, all),
        ProgramCommand::Upgrade {
            program_id,
            program_filepath,
            program_name,
            buffer,
            upgrade_authority,
            max_retries,
            use_rpc,
            solana_args,
        } => program_upgrade(
            cfg_override,
            program_id,
            program_filepath,
            program_name,
            buffer,
            upgrade_authority,
            max_retries,
            use_rpc,
            solana_args,
        ),
        ProgramCommand::Dump {
            account,
            output_file,
        } => program_dump(cfg_override, account, output_file),
        ProgramCommand::Close {
            account,
            program_name,
            authority,
            recipient,
            bypass_warning,
        } => program_close(
            cfg_override,
            account,
            program_name,
            authority,
            recipient,
            bypass_warning,
        ),
        ProgramCommand::Extend {
            program_id,
            program_name,
            additional_bytes,
        } => program_extend(cfg_override, program_id, program_name, additional_bytes),
    }
}

fn get_rpc_client_and_config(
    cfg_override: &ConfigOverride,
) -> Result<(RpcClient, Option<WithPath<Config>>)> {
    // Try to discover Anchor config first
    let config = Config::discover(cfg_override)?;

    let (url, _wallet_path) = crate::get_cluster_and_wallet(cfg_override)?;
    let rpc_client = RpcClient::new_with_commitment(url, CommitmentConfig::confirmed());

    Ok((rpc_client, config))
}

/// Get payer keypair from either Anchor config or Solana CLI config
fn get_payer_keypair(
    cfg_override: &ConfigOverride,
    config: &Option<WithPath<Config>>,
) -> Result<Keypair> {
    if let Some(cfg) = config {
        cfg.wallet_kp()
    } else {
        // No Anchor config - get wallet from Solana CLI config
        let (_url, wallet_path) = crate::get_cluster_and_wallet(cfg_override)?;
        Keypair::read_from_file(&wallet_path)
            .map_err(|e| anyhow!("Failed to read wallet keypair from {}: {}", wallet_path, e))
    }
}

/// Deploy a single program (either from explicit filepath or workspace) - private implementation
#[allow(clippy::too_many_arguments)]
pub fn program_deploy(
    cfg_override: &ConfigOverride,
    program_filepath: Option<PathBuf>,
    program_name: Option<String>,
    program_keypair: Option<PathBuf>,
    upgrade_authority: Option<String>,
    program_id: Option<Pubkey>,
    buffer: Option<Pubkey>,
    max_len: Option<usize>,
    use_rpc: bool,
    no_idl: bool,
    make_final: bool,
    solana_args: Vec<String>,
) -> Result<()> {
    let (rpc_client, config) = get_rpc_client_and_config(cfg_override)?;
    let payer = get_payer_keypair(cfg_override, &config)?;

    // Determine the program filepath
    let program_filepath = if let Some(filepath) = program_filepath {
        // Explicit filepath provided
        filepath
    } else {
        // Discover from workspace (Anchor or non-Anchor)
        let programs = get_programs_from_workspace(cfg_override, program_name.clone())?;

        let program = &programs[0];
        let binary_path = program.binary_path(false)?; // false = not verifiable build

        println!("Deploying program: {}", program.lib_name);

        binary_path
    };

    // Determine program keypair (loaded before fee discovery so program_id can
    // scope the recent-prioritization-fees query to this program's contention).
    let loaded_program_keypair = if let Some(keypair_path) = program_keypair {
        // Load from specified keypair file
        Keypair::read_from_file(&keypair_path).map_err(|e| {
            anyhow!(
                "Failed to read program keypair from {}: {}",
                keypair_path.display(),
                e
            )
        })?
    } else if let Some(_program_id) = program_id {
        return Err(anyhow!(
            "When --program-id is specified, --program-keypair must also be provided"
        ));
    } else {
        // Auto-detect from target/deploy/{program_name}-keypair.json
        let program_name = Path::new(&program_filepath)
            .file_stem()
            .and_then(|s| s.to_str())
            .ok_or_else(|| anyhow!("Invalid program filepath"))?;

        let keypair_path = target_dir()?
            .join("deploy")
            .join(format!("{program_name}-keypair.json"));
        Keypair::read_from_file(&keypair_path).map_err(|e| {
            anyhow!(
                "Failed to read program keypair from {}: {}. Use --program-keypair to specify a \
                 custom location.",
                keypair_path.display(),
                e
            )
        })?
    };

    let program_id = loaded_program_keypair.pubkey();

    // Inject per-program --buffer keypair so retries
    // within and across runs share the same on-chain buffer.
    let solana_args = if buffer.is_some() {
        solana_args
    } else {
        let program_name_stem = Path::new(&program_filepath)
            .file_stem()
            .and_then(|s| s.to_str())
            .ok_or_else(|| anyhow!("Invalid program filepath"))?;
        ensure_buffer_keypair_arg(solana_args, program_name_stem)?
    };

    // Augment with priority fees + max-sign-attempts;
    // pass program_id so fee query reflects this program's contention.
    let solana_args =
        crate::add_recommended_deployment_solana_args(&rpc_client, solana_args, &[program_id])?;

    // Parse priority fee from solana_args
    let priority_fee = parse_priority_fee_from_args(&solana_args);
    let max_sign_attempts = parse_max_sign_attempts_from_args(&solana_args);
    let skip_preflight = parse_skip_preflight_from_args(&solana_args);

    // Read program data
    let program_data = fs::read(&program_filepath).map_err(|e| {
        anyhow!(
            "Failed to read program file {}: {}",
            program_filepath.display(),
            e
        )
    })?;

    // Determine upgrade authority
    let upgrade_authority = if let Some(auth_path) = upgrade_authority {
        let authority_keypair = Keypair::read_from_file(&auth_path)
            .map_err(|e| anyhow!("Failed to read upgrade authority keypair: {}", e))?;
        println!(
            "Using custom upgrade authority: {}",
            authority_keypair.pubkey()
        );
        authority_keypair
    } else {
        payer.insecure_clone()
    };

    // Check if program already exists → decides deploy vs upgrade path
    let is_upgrade = rpc_client.get_account(&program_id).is_ok();

    if is_upgrade {
        println!("Program already exists, upgrading...");
        // Verify program can be upgraded before doing expensive buffer write
        verify_program_can_be_upgraded(&rpc_client, &program_id, &upgrade_authority)?;
    }

    // Resolve buffer: explicit pubkey from CLI flag (caller manages keypair
    // out-of-band) vs persistent keypair loaded from the path
    // `ensure_buffer_keypair_arg` injected. The persistent path is what
    // enables auto-resume across runs.
    let (buffer_pubkey, buffer_keypair): (Pubkey, Option<Keypair>) = if let Some(b) = buffer {
        (b, None)
    } else {
        let kp = read_buffer_keypair_from_args(&solana_args)?.ok_or_else(|| {
            anyhow!("internal: --buffer not injected by ensure_buffer_keypair_arg")
        })?;
        (kp.pubkey(), Some(kp))
    };

    let max_data_len = max_len.unwrap_or(program_data.len());
    let send_config = RpcSendTransactionConfig {
        skip_preflight,
        preflight_commitment: Some(CommitmentConfig::confirmed().commitment),
        encoding: None,
        max_retries: None,
        min_context_slot: None,
    };

    // Retry the write+commit cycle. Each iteration re-fetches buffer state, so
    // only chunks that didn't land last time are re-sent (diff-only resume).
    let mut last_err: Option<anyhow::Error> = None;
    for attempt in 0..MAX_DEPLOY_ATTEMPTS {
        if attempt > 0 {
            println!(
                "\nDeploy attempt {} of {}",
                attempt + 1,
                MAX_DEPLOY_ATTEMPTS
            );
        }

        let attempt_result: Result<()> = (|| {
            // Fetch existing buffer data — if Some, write_program_buffer skips
            // CreateBuffer and only writes chunks that differ. Waits for the
            // buffer state to stabilize.
            let existing = wait_for_buffer_stable(
                &rpc_client,
                &buffer_pubkey,
                &upgrade_authority.pubkey(),
                BUFFER_STABILIZE_MAX_WAIT_SECS,
            )?;

            // A persistent buffer from a previous run must match the current
            // binary size, not --max-len. The loader copies the whole buffer
            // body into ProgramData; --max-len only controls final ProgramData
            // capacity. If the buffer is too small writes fail, and if it is
            // too large stale tail bytes can be deployed.
            let needed_size = program_data.len();
            let existing_data = match existing {
                Some(buf) if buf.capacity != needed_size => {
                    if buffer_keypair.is_none() {
                        bail!(
                            "Buffer size mismatch: existing buffer {} has {} bytes, program needs \
                             {} bytes",
                            buffer_pubkey,
                            buf.capacity,
                            needed_size
                        );
                    }
                    println!(
                        "Existing buffer {} size mismatch ({} != {} bytes); closing and \
                         recreating.",
                        buffer_pubkey, buf.capacity, needed_size
                    );
                    close_buffer_for_resize(
                        &rpc_client,
                        &payer,
                        &buffer_pubkey,
                        &upgrade_authority,
                        priority_fee,
                        skip_preflight,
                    )?;
                    None
                }
                Some(buf) => Some(buf.data),
                None => None,
            };

            // Need keypair to create a fresh buffer; if it doesn't exist and
            // user gave us only a pubkey, we can't proceed.
            if existing_data.is_none() && buffer_keypair.is_none() {
                bail!(
                    "Buffer {} does not exist on-chain and no keypair available to create it",
                    buffer_pubkey
                );
            }

            if let Some(ref kp) = buffer_keypair {
                // Keep the intermediate buffer at the actual binary length.
                // --max-len is applied only by deploy_with_max_program_len.
                write_program_buffer(
                    &rpc_client,
                    &payer,
                    &program_data,
                    &upgrade_authority.pubkey(),
                    kp,
                    CommitmentConfig::confirmed(),
                    send_config,
                    priority_fee,
                    max_sign_attempts,
                    use_rpc,
                    existing_data,
                )?;
            }

            if is_upgrade {
                upgrade_program(
                    &rpc_client,
                    &payer,
                    &program_id,
                    &buffer_pubkey,
                    program_data.len(),
                    &upgrade_authority,
                    priority_fee,
                    true, // skip_program_verification - done above
                    skip_preflight,
                )?;
            } else {
                deploy_program(
                    &rpc_client,
                    &payer,
                    &buffer_pubkey,
                    &loaded_program_keypair,
                    &upgrade_authority,
                    max_data_len,
                    priority_fee,
                    skip_preflight,
                )?;
            }
            Ok(())
        })();

        match attempt_result {
            Ok(_) => {
                last_err = None;
                break;
            }
            Err(e) => {
                eprintln!("Attempt {} failed: {}", attempt + 1, e);
                last_err = Some(e);
            }
        }
    }

    if let Some(err) = last_err {
        eprintln!("\nDeploy failed after {} attempts.", MAX_DEPLOY_ATTEMPTS);
        eprintln!("Partial buffer: {}", buffer_pubkey);
        if let Some(path) = parse_buffer_keypair_path_from_args(&solana_args) {
            eprintln!("Buffer keypair: {}", path.display());
            eprintln!("Resume:   re-run the same command (buffer auto-loaded)");
            eprintln!(
                "          or anchor program deploy ... --buffer {}",
                buffer_pubkey
            );
            eprintln!("Reclaim:  solana program close {}", buffer_pubkey);
        }
        return Err(err);
    }

    // Print the program ID
    println!("Program ID: {}", program_id);

    // Deploy IDL if not skipped
    if !no_idl {
        // Extract program name from filepath
        let program_name = Path::new(&program_filepath)
            .file_stem()
            .and_then(|s| s.to_str())
            .ok_or_else(|| anyhow!("Invalid program filepath"))?;

        // Look for IDL file in target/idl/{program_name}.json
        let idl_filepath = target_dir()?
            .join("idl")
            .join(program_name)
            .with_extension("json");

        if Path::new(&idl_filepath).exists() {
            // Read and update the IDL with the program address
            let idl_content = fs::read_to_string(&idl_filepath).map_err(|e| {
                anyhow!("Failed to read IDL file {}: {}", idl_filepath.display(), e)
            })?;

            let mut idl: Idl = serde_json::from_str(&idl_content).map_err(|e| {
                anyhow!("Failed to parse IDL file {}: {}", idl_filepath.display(), e)
            })?;

            // Update the IDL with the program address
            idl.address = program_id.to_string();

            // Write the updated IDL back to the file
            let idl_json = serde_json::to_string_pretty(&idl)
                .map_err(|e| anyhow!("Failed to serialize IDL: {}", e))?;
            fs::write(&idl_filepath, idl_json).map_err(|e| {
                anyhow!("Failed to write IDL file {}: {}", idl_filepath.display(), e)
            })?;

            // Wait for the program to be confirmed before initializing IDL to prevent
            // race condition where the program isn't yet available in validator cache
            let max_retries = 5;
            let retry_delay = Duration::from_millis(500);
            let cache_delay = Duration::from_secs(2);

            for attempt in 0..max_retries {
                if let Ok(account) = rpc_client.get_account(&program_id) {
                    if account.executable {
                        thread::sleep(cache_delay);
                        break;
                    }
                }

                if attempt == max_retries - 1 {
                    println!("Failed");
                    return Err(anyhow!(
                        "Timeout waiting for program {} to be confirmed",
                        program_id
                    ));
                }

                thread::sleep(retry_delay);
            }

            // Check if we're on localnet - skip IDL operations on localnet
            let cluster_url = rpc_client.url();
            let is_localnet =
                cluster_url.contains("localhost") || cluster_url.contains("127.0.0.1");

            if is_localnet {
                // IDL deployment is skipped on localnet by default.
                // Use `anchor idl init --allow-localnet` to deploy on localnet.
                println!("Skipping IDL deployment on localnet");
            } else {
                crate::idl_init(
                    Some(program_id),
                    cfg_override,
                    idl_filepath,
                    None,
                    false,
                    false,
                )?;
                println!("✓ Idl metadata created/updated");
            }
        } else {
            println!(
                "Warning: IDL file not found at {}, skipping IDL deployment",
                idl_filepath.display()
            );
        }
    }

    // Make program immutable if --final flag is set
    if make_final {
        println!("\nMaking program immutable...");

        let set_authority_ix = loader_v3_instruction::set_upgrade_authority(
            &program_id,
            &upgrade_authority.pubkey(),
            None, // None = remove upgrade authority = immutable
        );

        let recent_blockhash = rpc_client.get_latest_blockhash()?;
        let tx = Transaction::new_signed_with_payer(
            &[set_authority_ix],
            Some(&payer.pubkey()),
            &[&payer, &upgrade_authority],
            recent_blockhash,
        );

        rpc_client
            .send_and_confirm_transaction(&tx)
            .map_err(|e| anyhow!("Failed to make program immutable: {}", e))?;

        println!("✓ Program is now immutable (cannot be upgraded)");
    }

    Ok(())
}

/// Verify that a buffer account is valid for upgrading
fn verify_buffer_account(
    rpc_client: &RpcClient,
    buffer_pubkey: &Pubkey,
    buffer_authority: &Pubkey,
) -> Result<()> {
    let buffer_account = rpc_client
        .get_account(buffer_pubkey)
        .map_err(|e| anyhow!("Buffer account {} not found: {}", buffer_pubkey, e))?;

    // Check if buffer is owned by BPF Upgradeable Loader
    if buffer_account.owner != bpf_loader_upgradeable_id::id() {
        return Err(anyhow!(
            "Buffer account {} is not owned by the BPF Upgradeable Loader",
            buffer_pubkey
        ));
    }

    // Verify it's actually a Buffer account
    match bincode::deserialize::<UpgradeableLoaderState>(&buffer_account.data) {
        Ok(UpgradeableLoaderState::Buffer { authority_address }) => {
            // Check if buffer is immutable
            if authority_address.is_none() {
                return Err(anyhow!("Buffer {} is immutable", buffer_pubkey));
            }
            // Verify the authority matches
            if authority_address != Some(*buffer_authority) {
                return Err(anyhow!(
                    "Buffer's authority {:?} does not match authority provided {}",
                    authority_address,
                    buffer_authority
                ));
            }
        }
        Ok(_) => {
            return Err(anyhow!("Account {} is not a Buffer account", buffer_pubkey));
        }
        Err(e) => {
            return Err(anyhow!(
                "Failed to deserialize buffer account {}: {}",
                buffer_pubkey,
                e
            ));
        }
    }

    Ok(())
}

/// Verify that a program exists, is upgradeable, and the authority matches
/// This should be called BEFORE doing expensive operations like buffer writes
fn verify_program_can_be_upgraded(
    rpc_client: &RpcClient,
    program_id: &Pubkey,
    upgrade_authority: &Keypair,
) -> Result<()> {
    // Verify the program exists
    let program_account = rpc_client
        .get_account(program_id)
        .map_err(|e| anyhow!("Failed to get program account {}: {}", program_id, e))?;

    if program_account.owner != bpf_loader_upgradeable_id::id() {
        return Err(anyhow!(
            "Program {} is not an upgradeable program",
            program_id
        ));
    }

    // Check if this is a valid program and get the ProgramData address
    let programdata_address =
        match bincode::deserialize::<UpgradeableLoaderState>(&program_account.data) {
            Ok(UpgradeableLoaderState::Program {
                programdata_address,
            }) => programdata_address,
            _ => {
                return Err(anyhow!(
                    "{} is not an upgradeable program account",
                    program_id
                ));
            }
        };

    // Verify the ProgramData account exists and is valid
    let programdata_account = rpc_client.get_account(&programdata_address).map_err(|e| {
        anyhow!(
            "Failed to get ProgramData account: {}. The program may have been closed.",
            e
        )
    })?;

    // Verify it's a valid ProgramData account
    match bincode::deserialize::<UpgradeableLoaderState>(&programdata_account.data) {
        Ok(UpgradeableLoaderState::ProgramData {
            upgrade_authority_address,
            ..
        }) => {
            // Check if the program is immutable
            if upgrade_authority_address.is_none() {
                return Err(anyhow!(
                    "Program {} is immutable and cannot be upgraded",
                    program_id
                ));
            }
            // Verify the authority matches
            if upgrade_authority_address != Some(upgrade_authority.pubkey()) {
                return Err(anyhow!(
                    "Upgrade authority mismatch. Expected {:?}, but ProgramData has {:?}",
                    Some(upgrade_authority.pubkey()),
                    upgrade_authority_address
                ));
            }
        }
        _ => {
            return Err(anyhow!(
                "Program {} has been closed or is in an invalid state",
                program_id
            ));
        }
    }

    Ok(())
}

#[allow(deprecated)]
#[allow(clippy::too_many_arguments)]
fn deploy_program(
    rpc_client: &RpcClient,
    payer: &Keypair,
    buffer: &Pubkey,
    program_keypair: &Keypair,
    upgrade_authority: &Keypair,
    max_data_len: usize,
    priority_fee: Option<u64>,
    skip_preflight: bool,
) -> Result<()> {
    let program_id = program_keypair.pubkey();
    let mut deploy_ixs = loader_v3_instruction::deploy_with_max_program_len(
        &payer.pubkey(),
        &program_id,
        buffer,
        &upgrade_authority.pubkey(),
        rpc_client
            .get_minimum_balance_for_rent_exemption(UpgradeableLoaderState::size_of_program())?,
        max_data_len,
    )
    .map_err(|e| anyhow!("Failed to create deploy instruction: {}", e))?;

    // Add priority fee if specified. Pass write-locked accounts so the RPC
    // fee fallback returns contention-aware data rather than the global median.
    deploy_ixs = crate::prepend_compute_unit_ix(
        deploy_ixs,
        rpc_client,
        priority_fee,
        &[program_id, *buffer],
    );

    let recent_blockhash = rpc_client.get_latest_blockhash()?;
    let deploy_tx = Transaction::new_signed_with_payer(
        &deploy_ixs,
        Some(&payer.pubkey()),
        &[payer, program_keypair, upgrade_authority],
        recent_blockhash,
    );

    // `_with_spinner_and_config` over bare `send_and_confirm_transaction`:
    // honors caller's preflight choice, shows progress, and lets RPC tuning
    // (max_retries, commitment) flow through if added later.
    rpc_client
        .send_and_confirm_transaction_with_spinner_and_config(
            &deploy_tx,
            CommitmentConfig::confirmed(),
            RpcSendTransactionConfig {
                skip_preflight,
                preflight_commitment: Some(CommitmentConfig::confirmed().commitment),
                encoding: None,
                max_retries: None,
                min_context_slot: None,
            },
        )
        .map_err(|e| anyhow!("Failed to deploy program: {}", e))?;

    Ok(())
}

// SIMD-0431: Minimum Extend Program Size
//
// Raise `additional_bytes` to at least MINIMUM_EXTEND_PROGRAM_BYTES (10 KiB),
// unless MAX_PERMITTED_DATA_LENGTH - current_total_len < 10 KiB, in which case
// it may only grow to the remaining free space.
fn adjust_extend_bytes(additional_bytes: u32, current_total_len: usize) -> u32 {
    let headroom =
        u32::try_from((MAX_PERMITTED_DATA_LENGTH as usize).saturating_sub(current_total_len))
            .unwrap_or(u32::MAX);
    additional_bytes.max(MINIMUM_EXTEND_PROGRAM_BYTES.min(headroom))
}

/// Extend programdata in-place if the new buffer exceeds the current allocation.
fn auto_extend_program_data_if_needed(
    rpc_client: &RpcClient,
    payer: &Keypair,
    program_id: &Pubkey,
    program_len: usize,
    upgrade_authority: &Keypair,
    skip_preflight: bool,
) -> Result<()> {
    let programdata_metadata_size = UpgradeableLoaderState::size_of_programdata_metadata();

    // Derive programdata pubkey and fetch its size.
    let (programdata_pubkey, _) =
        Pubkey::find_program_address(&[program_id.as_ref()], &bpf_loader_upgradeable_id::id());
    let programdata_account = rpc_client.get_account(&programdata_pubkey).map_err(|e| {
        anyhow!(
            "Failed to fetch programdata {} for auto-extend check: {}",
            programdata_pubkey,
            e
        )
    })?;
    let programdata_body_len = programdata_account
        .data
        .len()
        .saturating_sub(programdata_metadata_size);
    let required_programdata_len = UpgradeableLoaderState::size_of_programdata(program_len);
    let required_programdata_body_len =
        required_programdata_len.saturating_sub(programdata_metadata_size);

    if required_programdata_body_len <= programdata_body_len {
        return Ok(()); // already large enough
    }

    let additional_bytes = (required_programdata_body_len - programdata_body_len) as u32;
    let additional_bytes = adjust_extend_bytes(additional_bytes, programdata_account.data.len());
    println!(
        "Auto-extending program data by {} bytes ({} → {}) before upgrade…",
        additional_bytes,
        programdata_body_len,
        programdata_body_len + additional_bytes as usize
    );

    let extend_ix =
        loader_v3_instruction::extend_program(program_id, Some(&payer.pubkey()), additional_bytes);
    let recent_blockhash = rpc_client.get_latest_blockhash()?;
    let extend_tx = Transaction::new_signed_with_payer(
        &[extend_ix],
        Some(&payer.pubkey()),
        &[payer, upgrade_authority],
        recent_blockhash,
    );
    rpc_client
        .send_and_confirm_transaction_with_spinner_and_config(
            &extend_tx,
            CommitmentConfig::confirmed(),
            RpcSendTransactionConfig {
                skip_preflight,
                preflight_commitment: Some(CommitmentConfig::confirmed().commitment),
                encoding: None,
                max_retries: None,
                min_context_slot: None,
            },
        )
        .map_err(|e| anyhow!("Auto-extend failed: {}", e))?;

    let extended_slot = rpc_client
        .get_slot()
        .map_err(|e| anyhow!("Failed to fetch slot after auto-extend: {}", e))?;
    for _ in 0..20 {
        let current_slot = rpc_client
            .get_slot()
            .map_err(|e| anyhow!("Failed to wait for next slot after auto-extend: {}", e))?;
        if current_slot > extended_slot {
            return Ok(());
        }
        thread::sleep(Duration::from_millis(400));
    }

    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn upgrade_program(
    rpc_client: &RpcClient,
    payer: &Keypair,
    program_id: &Pubkey,
    buffer: &Pubkey,
    program_len: usize,
    upgrade_authority: &Keypair,
    priority_fee: Option<u64>,
    skip_program_verification: bool,
    skip_preflight: bool,
) -> Result<()> {
    // Verify program can be upgraded (unless caller already verified)
    if !skip_program_verification {
        verify_program_can_be_upgraded(rpc_client, program_id, upgrade_authority)?;
    }

    // Verify the buffer account is valid
    verify_buffer_account(rpc_client, buffer, &upgrade_authority.pubkey())?;

    // Auto-extend programdata if the new buffer's body is larger than the
    // current programdata allocation.
    auto_extend_program_data_if_needed(
        rpc_client,
        payer,
        program_id,
        program_len,
        upgrade_authority,
        skip_preflight,
    )?;

    println!("Sending upgrade transaction...");

    let upgrade_ix = loader_v3_instruction::upgrade(
        program_id,
        buffer,
        &upgrade_authority.pubkey(),
        &payer.pubkey(),
    );

    // Add priority fee if specified
    let upgrade_ixs = crate::prepend_compute_unit_ix(
        vec![upgrade_ix],
        rpc_client,
        priority_fee,
        &[*program_id, *buffer],
    );

    let recent_blockhash = rpc_client.get_latest_blockhash()?;
    let upgrade_tx = Transaction::new_signed_with_payer(
        &upgrade_ixs,
        Some(&payer.pubkey()),
        &[payer, upgrade_authority],
        recent_blockhash,
    );

    let signature = rpc_client
        .send_and_confirm_transaction_with_spinner_and_config(
            &upgrade_tx,
            CommitmentConfig::confirmed(),
            RpcSendTransactionConfig {
                skip_preflight,
                preflight_commitment: Some(CommitmentConfig::confirmed().commitment),
                encoding: None,
                max_retries: None,
                min_context_slot: None,
            },
        )
        .map_err(|e| anyhow!("Failed to upgrade program: {}", e))?;
    println!("Signature: {}", signature);
    Ok(())
}

fn program_write_buffer(
    cfg_override: &ConfigOverride,
    program_filepath: Option<PathBuf>,
    program_name: Option<String>,
    _buffer: Option<String>,
    buffer_authority: Option<String>,
) -> Result<()> {
    let (rpc_client, config) = get_rpc_client_and_config(cfg_override)?;
    let payer = get_payer_keypair(cfg_override, &config)?;

    // Determine the program filepath
    let program_filepath = if let Some(filepath) = program_filepath {
        filepath
    } else {
        // Discover from workspace (Anchor or non-Anchor)
        let programs = get_programs_from_workspace(cfg_override, program_name.clone())?;

        if programs.len() > 1 && program_name.is_none() {
            let program_names: Vec<_> = programs.iter().map(|p| p.lib_name.as_str()).collect();
            return Err(anyhow!(
                "Multiple programs found: {}. Use --program-name to specify which one to write",
                program_names.join(", ")
            ));
        }

        let program = &programs[0];
        let binary_path = program.binary_path(false)?;

        println!("Writing buffer for program: {}", program.lib_name);

        binary_path
    };

    // Read program data
    let program_data = fs::read(&program_filepath).map_err(|e| {
        anyhow!(
            "Failed to read program file {}: {}",
            program_filepath.display(),
            e
        )
    })?;

    // Determine buffer authority
    let buffer_authority_keypair = if let Some(auth_path) = buffer_authority {
        Keypair::read_from_file(&auth_path)
            .map_err(|e| anyhow!("Failed to read buffer authority keypair: {}", e))?
    } else {
        payer.insecure_clone()
    };

    let buffer_keypair = Keypair::new();
    let buffer_pubkey = write_program_buffer(
        &rpc_client,
        &payer,
        &program_data,
        &buffer_authority_keypair.pubkey(),
        &buffer_keypair,
        CommitmentConfig::confirmed(),
        RpcSendTransactionConfig {
            skip_preflight: false,
            preflight_commitment: Some(CommitmentConfig::confirmed().commitment),
            encoding: None,
            max_retries: None,
            min_context_slot: None,
        },
        None,
        DEFAULT_MAX_SIGN_ATTEMPTS,
        false,
        None,
    )?;

    println!("Buffer: {}", buffer_pubkey);
    Ok(())
}

fn program_set_buffer_authority(
    cfg_override: &ConfigOverride,
    buffer: Pubkey,
    new_buffer_authority: Pubkey,
) -> Result<()> {
    let (rpc_client, config) = get_rpc_client_and_config(cfg_override)?;
    let payer = get_payer_keypair(cfg_override, &config)?;

    println!("Setting buffer authority...");
    println!("Buffer: {}", buffer);
    println!("New authority: {}", new_buffer_authority);

    let set_authority_ixs = loader_v3_instruction::set_buffer_authority(
        &buffer,
        &payer.pubkey(),
        &new_buffer_authority,
    );

    let recent_blockhash = rpc_client.get_latest_blockhash()?;
    let tx = Transaction::new_signed_with_payer(
        &[set_authority_ixs],
        Some(&payer.pubkey()),
        &[&payer],
        recent_blockhash,
    );

    rpc_client
        .send_and_confirm_transaction(&tx)
        .map_err(|e| anyhow!("Failed to set buffer authority: {}", e))?;

    println!("Buffer authority updated successfully!");
    Ok(())
}

fn program_set_upgrade_authority(
    cfg_override: &ConfigOverride,
    program_id: Pubkey,
    new_upgrade_authority: Option<Pubkey>,
    new_upgrade_authority_signer: Option<String>,
    skip_new_upgrade_authority_signer_check: bool,
    make_final: bool,
    current_upgrade_authority: Option<String>,
) -> Result<()> {
    let (rpc_client, config) = get_rpc_client_and_config(cfg_override)?;
    let payer = get_payer_keypair(cfg_override, &config)?;

    // Validate that this is a Program account, not ProgramData
    let program_account = rpc_client
        .get_account(&program_id)
        .map_err(|e| anyhow!("Failed to get account {}: {}", program_id, e))?;

    if program_account.owner != bpf_loader_upgradeable_id::id() {
        return Err(anyhow!(
            "Account {} is not owned by the BPF Upgradeable Loader",
            program_id
        ));
    }

    // Ensure this is a Program account, not ProgramData or Buffer
    match bincode::deserialize::<UpgradeableLoaderState>(&program_account.data) {
        Ok(UpgradeableLoaderState::Program { .. }) => {
            // Valid program account
        }
        Ok(UpgradeableLoaderState::ProgramData { .. }) => {
            return Err(anyhow!(
                "Error: {} is a ProgramData account, not a Program account.\n\nTo set the upgrade \
                 authority, you must provide the Program ID, not the ProgramData address.\nUse \
                 'anchor program show {}' to find the associated Program ID.",
                program_id,
                program_id
            ));
        }
        Ok(UpgradeableLoaderState::Buffer { .. }) => {
            return Err(anyhow!(
                "{} is a Buffer account, not a Program account. Use set-buffer-authority for \
                 buffers.",
                program_id
            ));
        }
        _ => {
            return Err(anyhow!("{} is not a valid upgradeable program", program_id));
        }
    }

    println!("Setting upgrade authority...");
    println!("Program ID: {}", program_id);

    if make_final {
        println!("Making program immutable (cannot be upgraded)");
    } else if let Some(new_auth) = new_upgrade_authority {
        println!("New upgrade authority: {}", new_auth);
    } else {
        bail!("Must provide either --new-upgrade-authority or --final");
    }

    // Determine current authority keypair (must be a signer)
    let current_authority_keypair = if let Some(auth_path) = current_upgrade_authority {
        let keypair = Keypair::read_from_file(&auth_path)
            .map_err(|e| anyhow!("Failed to read current upgrade authority keypair: {}", e))?;
        println!("Using custom current authority: {}", keypair.pubkey());
        keypair
    } else {
        payer.insecure_clone()
    };

    // Validate signer requirements and load keypair
    let new_auth_keypair_opt = if let Some(signer_path) = new_upgrade_authority_signer {
        // Signer provided - use checked mode
        let keypair = Keypair::read_from_file(&signer_path)
            .map_err(|e| anyhow!("Failed to read new upgrade authority signer keypair: {}", e))?;

        // Verify the pubkey matches if both are provided
        if let Some(pubkey) = new_upgrade_authority {
            if pubkey != keypair.pubkey() {
                return Err(anyhow!(
                    "New upgrade authority pubkey mismatch: --new-upgrade-authority ({}) doesn't \
                     match --new-upgrade-authority-signer keypair ({})",
                    pubkey,
                    keypair.pubkey()
                ));
            }
        }

        println!("Using CHECKED mode - both current and new authority will sign (recommended)");
        Some(keypair)
    } else if new_upgrade_authority.is_some() && !make_final {
        // No signer provided but new authority specified
        if skip_new_upgrade_authority_signer_check {
            // User explicitly allowed unchecked mode
            println!("WARNING: Using UNCHECKED mode - only current authority will sign");
            println!("         This is less safe! The new authority won't verify ownership.");
            None
        } else {
            // By default, require the signer for safety
            return Err(anyhow!(
                "New upgrade authority signer is required for safety.\nPlease provide \
                 --new-upgrade-authority-signer <KEYPAIR_FILE> (recommended),\nor use \
                 --skip-new-upgrade-authority-signer-check if you're confident the pubkey is \
                 correct."
            ));
        }
    } else {
        // Making program final or no new authority - no signer needed
        None
    };

    // Build instruction based on mode
    let set_authority_ixs = if let Some(ref new_auth_keypair) = new_auth_keypair_opt {
        // Checked mode: both current and new authority sign (safer)
        loader_v3_instruction::set_upgrade_authority_checked(
            &program_id,
            &current_authority_keypair.pubkey(),
            &new_auth_keypair.pubkey(),
        )
    } else {
        // Unchecked mode or final mode: only current authority signs
        loader_v3_instruction::set_upgrade_authority(
            &program_id,
            &current_authority_keypair.pubkey(),
            new_upgrade_authority.as_ref(),
        )
    };

    let recent_blockhash = rpc_client.get_latest_blockhash()?;

    let signature = if let Some(ref new_auth_keypair) = new_auth_keypair_opt {
        // Checked mode with 3 signers
        let tx = Transaction::new_signed_with_payer(
            &[set_authority_ixs],
            Some(&payer.pubkey()),
            &[&payer, &current_authority_keypair, new_auth_keypair],
            recent_blockhash,
        );
        rpc_client
            .send_and_confirm_transaction(&tx)
            .map_err(|e| anyhow!("Failed to set upgrade authority: {}", e))?
    } else {
        // Unchecked mode or final mode with 2 signers
        let tx = Transaction::new_signed_with_payer(
            &[set_authority_ixs],
            Some(&payer.pubkey()),
            &[&payer, &current_authority_keypair],
            recent_blockhash,
        );
        rpc_client
            .send_and_confirm_transaction(&tx)
            .map_err(|e| anyhow!("Failed to set upgrade authority: {}", e))?
    };

    println!();
    println!("Upgrade authority updated successfully!");
    println!("Signature: {}", signature);
    Ok(())
}

fn program_show(
    cfg_override: &ConfigOverride,
    account: Pubkey,
    _get_programs: bool,
    _get_buffers: bool,
    _all: bool,
) -> Result<()> {
    let (rpc_client, _config) = get_rpc_client_and_config(cfg_override)?;

    let account_data = rpc_client
        .get_account(&account)
        .map_err(|e| anyhow!("Failed to get account {}: {}", account, e))?;

    println!("Account: {}", account);
    println!("Owner: {}", account_data.owner);
    println!("Balance: {} lamports", account_data.lamports);
    println!("Data length: {} bytes", account_data.data.len());
    println!("Executable: {}", account_data.executable);

    // Try to parse as upgradeable loader state
    if account_data.owner == bpf_loader_upgradeable_id::id() {
        match bincode::deserialize::<UpgradeableLoaderState>(&account_data.data) {
            Ok(state) => match state {
                UpgradeableLoaderState::Uninitialized => {
                    println!("Type: Uninitialized");
                }
                UpgradeableLoaderState::Buffer { authority_address } => {
                    println!("Type: Buffer");
                    if let Some(authority) = authority_address {
                        println!("Authority: {}", authority);
                    } else {
                        println!("Authority: None (immutable)");
                    }
                }
                UpgradeableLoaderState::Program {
                    programdata_address,
                } => {
                    println!("Type: Program");
                    println!("Program Data Address: {}", programdata_address);

                    // Fetch program data account
                    if let Ok(programdata_account) = rpc_client.get_account(&programdata_address) {
                        if let Ok(UpgradeableLoaderState::ProgramData {
                            slot,
                            upgrade_authority_address,
                        }) = bincode::deserialize::<UpgradeableLoaderState>(
                            &programdata_account.data,
                        ) {
                            println!("Slot: {}", slot);
                            if let Some(authority) = upgrade_authority_address {
                                println!("Upgrade Authority: {}", authority);
                            } else {
                                println!("Upgrade Authority: None (immutable)");
                            }
                        }
                    }
                }
                UpgradeableLoaderState::ProgramData {
                    slot,
                    upgrade_authority_address,
                } => {
                    println!("Type: Program Data");
                    println!("Slot: {}", slot);
                    if let Some(authority) = upgrade_authority_address {
                        println!("Upgrade Authority: {}", authority);
                    } else {
                        println!("Upgrade Authority: None (immutable)");
                    }
                }
            },
            Err(e) => {
                println!("Failed to parse as upgradeable loader state: {}", e);
            }
        }
    }

    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub fn program_upgrade(
    cfg_override: &ConfigOverride,
    program_id: Pubkey,
    program_filepath: Option<PathBuf>,
    program_name: Option<String>,
    buffer: Option<Pubkey>,
    upgrade_authority: Option<String>,
    max_retries: u32,
    use_rpc: bool,
    solana_args: Vec<String>,
) -> Result<()> {
    let (rpc_client, config) = get_rpc_client_and_config(cfg_override)?;
    let payer = get_payer_keypair(cfg_override, &config)?;

    // Augment solana_args with recommended defaults if provided.
    // Pass program_id so recent prio-fee query reflects past upgrade contention
    // for this program (programdata is write-locked during upgrade).
    let solana_args = if !solana_args.is_empty() {
        crate::add_recommended_deployment_solana_args(&rpc_client, solana_args, &[program_id])?
    } else {
        solana_args
    };

    // Parse priority fee from solana_args
    let priority_fee = parse_priority_fee_from_args(&solana_args);
    let max_sign_attempts = parse_max_sign_attempts_from_args(&solana_args);
    let skip_preflight = parse_skip_preflight_from_args(&solana_args);

    // Determine upgrade authority
    let upgrade_authority_keypair = if let Some(auth_path) = upgrade_authority {
        let keypair = Keypair::read_from_file(&auth_path)
            .map_err(|e| anyhow!("Failed to read upgrade authority keypair: {}", e))?;
        println!("Using custom upgrade authority: {}", keypair.pubkey());
        keypair
    } else {
        payer.insecure_clone()
    };

    // Verify the program can be upgraded BEFORE doing expensive operations
    // This prevents wasting time/money on buffer writes if the program is closed or immutable
    verify_program_can_be_upgraded(&rpc_client, &program_id, &upgrade_authority_keypair)?;

    // Case 1: Using existing buffer (no retries needed)
    if let Some(buffer_pubkey) = buffer {
        let buffer_account = rpc_client.get_account(&buffer_pubkey).map_err(|e| {
            anyhow!(
                "Failed to fetch buffer {} for upgrade length check: {}",
                buffer_pubkey,
                e
            )
        })?;
        let buffer_program_len = buffer_account
            .data
            .len()
            .saturating_sub(UpgradeableLoaderState::size_of_buffer_metadata());
        return upgrade_program(
            &rpc_client,
            &payer,
            &program_id,
            &buffer_pubkey,
            buffer_program_len,
            &upgrade_authority_keypair,
            priority_fee,
            true, // skip_program_verification - already done above
            skip_preflight,
        );
    }

    // Case 2: Creating buffer from program file (with retries)
    let program_filepath = if let Some(filepath) = program_filepath {
        // Explicit filepath provided
        filepath
    } else {
        // Discover from workspace (Anchor or non-Anchor)
        let programs = get_programs_from_workspace(cfg_override, program_name.clone())?;

        let program = &programs[0];
        let binary_path = program.binary_path(false)?; // false = not verifiable build

        println!("Upgrading program: {}", program.lib_name);

        binary_path
    };

    // Inject per-program --buffer keypair so retries
    // within and across runs share the same on-chain buffer.
    let program_name_stem = Path::new(&program_filepath)
        .file_stem()
        .and_then(|s| s.to_str())
        .ok_or_else(|| anyhow!("Invalid program filepath"))?;
    let solana_args = ensure_buffer_keypair_arg(solana_args, program_name_stem)?;

    let program_data = fs::read(&program_filepath).map_err(|e| {
        anyhow!(
            "Failed to read program file {}: {}",
            program_filepath.display(),
            e
        )
    })?;

    // Persistent buffer keypair: reuse across retries so partial writes from a
    // failed attempt survive on-chain and the next attempt only re-sends the
    // chunks that didn't land. `ensure_buffer_keypair_arg` guarantees the
    // injection above; absence here is an internal bug.
    let buffer_keypair = read_buffer_keypair_from_args(&solana_args)?
        .ok_or_else(|| anyhow!("internal: --buffer not injected by ensure_buffer_keypair_arg"))?;
    let buffer_pubkey = buffer_keypair.pubkey();

    let send_config = RpcSendTransactionConfig {
        skip_preflight,
        preflight_commitment: Some(CommitmentConfig::confirmed().commitment),
        encoding: None,
        max_retries: None,
        min_context_slot: None,
    };

    // Retry loop for buffer write + upgrade
    let mut last_err: Option<anyhow::Error> = None;
    for retry in 0..(1 + max_retries) {
        if max_retries > 0 {
            println!("\nAttempt {}/{}", retry + 1, max_retries + 1);
        }

        let attempt_result: Result<()> = (|| {
            // Fetch existing buffer state for diff-only resume
            let existing = wait_for_buffer_stable(
                &rpc_client,
                &buffer_pubkey,
                &upgrade_authority_keypair.pubkey(),
                BUFFER_STABILIZE_MAX_WAIT_SECS,
            )?;

            // Same size check as program_deploy — persistent buffer keypair
            // from an earlier run may have been sized for a different binary.
            // Undersize stalls writes; oversize leaves stale tail bytes that
            // the loader's Upgrade ix copies into programdata. Either way we
            // own this keypair (injected by ensure_buffer_keypair_arg), so
            // close it and let the next call to write_program_buffer recreate.
            let needed_size = program_data.len();
            let existing_data = match existing {
                Some(buf) if buf.capacity != needed_size => {
                    println!(
                        "Existing buffer {} size mismatch ({} != {} bytes); closing and \
                         recreating.",
                        buffer_pubkey, buf.capacity, needed_size
                    );
                    close_buffer_for_resize(
                        &rpc_client,
                        &payer,
                        &buffer_pubkey,
                        &upgrade_authority_keypair,
                        priority_fee,
                        skip_preflight,
                    )?;
                    None
                }
                Some(buf) => Some(buf.data),
                None => None,
            };

            write_program_buffer(
                &rpc_client,
                &payer,
                &program_data,
                &upgrade_authority_keypair.pubkey(),
                &buffer_keypair,
                CommitmentConfig::confirmed(),
                send_config,
                priority_fee,
                max_sign_attempts,
                use_rpc,
                existing_data,
            )?;

            upgrade_program(
                &rpc_client,
                &payer,
                &program_id,
                &buffer_pubkey,
                program_data.len(),
                &upgrade_authority_keypair,
                priority_fee,
                true, // skip_program_verification
                skip_preflight,
            )?;
            Ok(())
        })();

        match attempt_result {
            Ok(_) => {
                if max_retries > 0 {
                    println!("\nUpgrade success");
                }
                return Ok(());
            }
            Err(e) => {
                println!("Attempt {} failed: {}", retry + 1, e);
                last_err = Some(e);
                if retry < max_retries {
                    println!("Retrying {} more time(s)...", max_retries - retry);
                }
            }
        }
    }

    if let Some(err) = last_err {
        eprintln!("\nUpgrade failed after {} attempts.", max_retries + 1);
        eprintln!("Partial buffer: {}", buffer_pubkey);
        if let Some(path) = parse_buffer_keypair_path_from_args(&solana_args) {
            eprintln!("Buffer keypair: {}", path.display());
            eprintln!("Resume:   re-run the same command (buffer auto-loaded)");
            eprintln!(
                "          or anchor program upgrade {} <FILE> --buffer {}",
                program_id, buffer_pubkey
            );
            eprintln!("Reclaim:  solana program close {}", buffer_pubkey);
        }
        return Err(err);
    }

    Ok(())
}

fn program_dump(cfg_override: &ConfigOverride, account: Pubkey, output_file: String) -> Result<()> {
    let (rpc_client, _config) = get_rpc_client_and_config(cfg_override)?;

    println!("Fetching program data...");

    let account_data = rpc_client
        .get_account(&account)
        .map_err(|e| anyhow!("Failed to get account {}: {}", account, e))?;

    // Check if this is a program account
    let program_data = if account_data.owner == bpf_loader_upgradeable_id::id() {
        match bincode::deserialize::<UpgradeableLoaderState>(&account_data.data) {
            Ok(UpgradeableLoaderState::Program {
                programdata_address,
            }) => {
                // Fetch the program data account
                let programdata_account = rpc_client
                    .get_account(&programdata_address)
                    .map_err(|e| anyhow!("Failed to get program data account: {}", e))?;

                // Skip the UpgradeableLoaderState header
                let data_offset = UpgradeableLoaderState::size_of_programdata_metadata();
                programdata_account.data[data_offset..].to_vec()
            }
            Ok(UpgradeableLoaderState::Buffer { .. }) => {
                // Buffer account - skip the header
                let data_offset = UpgradeableLoaderState::size_of_buffer_metadata();
                account_data.data[data_offset..].to_vec()
            }
            Ok(UpgradeableLoaderState::ProgramData { .. }) => {
                // Program data account - skip the header
                let data_offset = UpgradeableLoaderState::size_of_programdata_metadata();
                account_data.data[data_offset..].to_vec()
            }
            _ => account_data.data,
        }
    } else {
        // Regular program or other account
        account_data.data
    };

    println!("Writing {} bytes to {}...", program_data.len(), output_file);

    let mut file =
        File::create(&output_file).map_err(|e| anyhow!("Failed to create output file: {}", e))?;

    file.write_all(&program_data)
        .map_err(|e| anyhow!("Failed to write program data: {}", e))?;

    println!("Program dumped to {}", output_file);
    Ok(())
}

fn program_close(
    cfg_override: &ConfigOverride,
    account: Option<Pubkey>,
    program_name: Option<String>,
    authority: Option<String>,
    recipient: Option<Pubkey>,
    bypass_warning: bool,
) -> Result<()> {
    let (rpc_client, config) = get_rpc_client_and_config(cfg_override)?;
    let payer = get_payer_keypair(cfg_override, &config)?;

    // Determine the account to close
    let account = if let Some(acc) = account {
        acc
    } else if let Some(name) = program_name {
        // Discover from workspace (Anchor or non-Anchor)
        let programs = get_programs_from_workspace(cfg_override, Some(name.clone()))?;

        let program = &programs[0];

        // Get the program keypair to derive program ID
        let keypair_path = program.keypair_file()?.path().display().to_string();
        let program_keypair = Keypair::read_from_file(&keypair_path).map_err(|e| {
            anyhow!(
                "Failed to read program keypair from {}: {}",
                keypair_path,
                e
            )
        })?;

        let program_id = program_keypair.pubkey();
        println!("Closing program: {} ({})", program.lib_name, program_id);
        program_id
    } else {
        return Err(anyhow!(
            "Must provide either account address or --program-name"
        ));
    };

    // Fetch the account to determine its type
    let account_data = rpc_client
        .get_account(&account)
        .map_err(|e| anyhow!("Failed to get account {}: {}", account, e))?;

    // Check if this is a BPF Loader Upgradeable account
    if account_data.owner != bpf_loader_upgradeable_id::id() {
        return Err(anyhow!(
            "Account {} is not owned by the BPF Loader Upgradeable program",
            account
        ));
    }

    // Determine which account to actually close
    let (account_to_close, account_type, program_account) =
        match bincode::deserialize::<UpgradeableLoaderState>(&account_data.data) {
            Ok(UpgradeableLoaderState::Program {
                programdata_address,
            }) => (programdata_address, "ProgramData", Some(account)),
            Ok(UpgradeableLoaderState::Buffer { .. }) => (account, "Buffer", None),
            Ok(UpgradeableLoaderState::ProgramData { .. }) => (account, "ProgramData", None),
            _ => {
                return Err(anyhow!(
                    "Account {} is not a Buffer, Program, or ProgramData account",
                    account
                ));
            }
        };

    // Determine authority
    let authority_keypair = if let Some(auth_path) = authority {
        Keypair::read_from_file(&auth_path)
            .map_err(|e| anyhow!("Failed to read authority keypair: {}", e))?
    } else {
        payer.insecure_clone()
    };

    // Determine recipient
    let recipient_pubkey = recipient.unwrap_or_else(|| authority_keypair.pubkey());

    if !bypass_warning {
        println!();
        println!(
            "WARNING: This will close the {} account and reclaim all lamports.",
            account_type
        );

        if account_type == "ProgramData" {
            println!();
            println!(
                "IMPORTANT: Closing the ProgramData account will make the program non-upgradeable"
            );
            println!("and the program will become immutable. This action cannot be undone!");
        }

        println!();
        print!("Continue? (y/n): ");
        std::io::Write::flush(&mut std::io::stdout())?;

        let mut input = String::new();
        std::io::stdin().read_line(&mut input)?;

        if !input.trim().eq_ignore_ascii_case("y") {
            println!("Cancelled");
            return Ok(());
        }
    }

    println!("Closing {} account...", account_type);

    let close_ixs = loader_v3_instruction::close_any(
        &account_to_close,
        &recipient_pubkey,
        Some(&authority_keypair.pubkey()),
        program_account.as_ref(),
    );

    let recent_blockhash = rpc_client.get_latest_blockhash()?;
    let tx = Transaction::new_signed_with_payer(
        &[close_ixs],
        Some(&payer.pubkey()),
        &[&payer, &authority_keypair],
        recent_blockhash,
    );

    rpc_client
        .send_and_confirm_transaction(&tx)
        .map_err(|e| anyhow!("Failed to close account: {}", e))?;

    println!("{} account closed", account_type);
    println!("Reclaimed lamports sent to: {}", recipient_pubkey);
    Ok(())
}

fn program_extend(
    cfg_override: &ConfigOverride,
    program_id: Option<Pubkey>,
    program_name: Option<String>,
    additional_bytes: usize,
) -> Result<()> {
    let (rpc_client, config) = get_rpc_client_and_config(cfg_override)?;
    let payer = get_payer_keypair(cfg_override, &config)?;

    if additional_bytes == 0 {
        return Err(anyhow!("Additional bytes must be greater than zero"));
    }

    // Determine the program ID
    let program_id = if let Some(id) = program_id {
        id
    } else if let Some(name) = program_name {
        // Discover from workspace (Anchor or non-Anchor)
        let programs = get_programs_from_workspace(cfg_override, Some(name.clone()))?;

        let program = &programs[0];

        // Get the program keypair to derive program ID
        let keypair_path = program.keypair_file()?.path().display().to_string();
        let program_keypair = Keypair::read_from_file(&keypair_path).map_err(|e| {
            anyhow!(
                "Failed to read program keypair from {}: {}",
                keypair_path,
                e
            )
        })?;

        let program_id = program_keypair.pubkey();
        println!("Extending program: {} ({})", program.lib_name, program_id);
        program_id
    } else {
        return Err(anyhow!("Must provide either program ID or --program-name"));
    };

    println!("Extending program data...");
    println!("Program ID: {}", program_id);
    println!("Additional bytes: {}", additional_bytes);

    // Get the program account to find the ProgramData address
    let program_account = rpc_client
        .get_account(&program_id)
        .map_err(|e| anyhow!("Failed to get program account {}: {}", program_id, e))?;

    if program_account.owner != bpf_loader_upgradeable_id::id() {
        return Err(anyhow!(
            "Account {} is not an upgradeable program",
            program_id
        ));
    }

    // Get the ProgramData address
    let programdata_address =
        match bincode::deserialize::<UpgradeableLoaderState>(&program_account.data) {
            Ok(UpgradeableLoaderState::Program {
                programdata_address,
            }) => programdata_address,
            _ => {
                return Err(anyhow!(
                    "{} is not an upgradeable program account",
                    program_id
                ));
            }
        };

    // Get the ProgramData account to verify upgrade authority
    let programdata_account = rpc_client
        .get_account(&programdata_address)
        .map_err(|e| anyhow!("Program {} is closed: {}", program_id, e))?;

    // Get the upgrade authority address
    let upgrade_authority_address =
        match bincode::deserialize::<UpgradeableLoaderState>(&programdata_account.data) {
            Ok(UpgradeableLoaderState::ProgramData {
                upgrade_authority_address,
                ..
            }) => upgrade_authority_address,
            _ => {
                return Err(anyhow!("Program {} is closed", program_id));
            }
        };

    let upgrade_authority_address = upgrade_authority_address
        .ok_or_else(|| anyhow!("Program {} is not upgradeable", program_id))?;

    // Verify the payer is the upgrade authority
    if upgrade_authority_address != payer.pubkey() {
        return Err(anyhow!(
            "Upgrade authority mismatch. Expected {}, but ProgramData has {}",
            payer.pubkey(),
            upgrade_authority_address
        ));
    }

    let headroom =
        (MAX_PERMITTED_DATA_LENGTH as usize).saturating_sub(programdata_account.data.len());
    if additional_bytes < MINIMUM_EXTEND_PROGRAM_BYTES as usize && additional_bytes != headroom {
        return Err(if headroom < MINIMUM_EXTEND_PROGRAM_BYTES as usize {
            anyhow!(
                "Program is {headroom} bytes from maximum size, but {additional_bytes} were \
                 requested. Please re-run the command with {headroom} additional bytes."
            )
        } else {
            anyhow!(
                "ExtendProgram requires a minimum of {MINIMUM_EXTEND_PROGRAM_BYTES} additional \
                 bytes, but only {additional_bytes} were requested"
            )
        });
    }

    let extend_ix = loader_v3_instruction::extend_program(
        &program_id,
        Some(&payer.pubkey()),
        additional_bytes as u32,
    );

    let recent_blockhash = rpc_client.get_latest_blockhash()?;
    let tx = Transaction::new_signed_with_payer(
        &[extend_ix],
        Some(&payer.pubkey()),
        &[&payer], // payer is also the upgrade authority
        recent_blockhash,
    );

    rpc_client
        .send_and_confirm_transaction(&tx)
        .map_err(|e| anyhow!("Failed to extend program: {}", e))?;

    println!("Program extended successfully!");
    Ok(())
}

// ========== Agave's core parallel deployment functions ==========

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

fn set_compute_unit_limit_ix_data(message: &mut Message, ix_index: usize, compute_unit_limit: u32) {
    let ix = &mut message.instructions[ix_index];
    let program_id = message.account_keys[ix.program_id_index as usize];
    assert_eq!(program_id, compute_budget_program_id::id());
    ix.data = ComputeBudgetInstruction::set_compute_unit_limit(compute_unit_limit).data;
}

fn simulate_write_compute_unit_limit(
    rpc_client: &RpcClient,
    message: &Message,
    fee_payer_signer: &dyn Signer,
    write_signer: &dyn Signer,
) -> Result<u32> {
    let blockhash = rpc_client.get_latest_blockhash()?;
    let mut message = message.clone();
    message.recent_blockhash = blockhash;
    let mut transaction = Transaction::new_unsigned(message);
    transaction.try_sign(&[fee_payer_signer, write_signer], blockhash)?;

    let RpcSimulateTransactionResult {
        err,
        units_consumed,
        ..
    } = rpc_client
        .simulate_transaction_with_config(
            &transaction,
            RpcSimulateTransactionConfig {
                sig_verify: false,
                replace_recent_blockhash: true,
                commitment: Some(CommitmentConfig::processed()),
                ..RpcSimulateTransactionConfig::default()
            },
        )?
        .value;

    if let Some(err) = err {
        bail!("Write transaction simulation failed: {err:?}");
    }

    let units_consumed = units_consumed
        .ok_or_else(|| anyhow!("Write transaction simulation did not return units_consumed"))?;
    units_consumed
        .try_into()
        .map_err(|_| anyhow!("Simulated write CU limit exceeds u32"))
}

fn simulate_and_update_write_compute_unit_limit(
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
        &write_messages[0],
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
    };

    for message in &mut write_messages {
        set_compute_unit_limit_ix_data(message, compute_unit_limit_ix_index, compute_unit_limit);
    }

    Ok(write_messages)
}

#[allow(clippy::too_many_arguments)]
pub fn send_deploy_messages(
    rpc_client: &RpcClient,
    initial_message: Option<Message>,
    mut write_messages: Vec<Message>,
    final_message: Option<Message>,
    fee_payer_signer: &dyn Signer,
    initial_signer: Option<&dyn Signer>,
    write_signer: Option<&dyn Signer>,
    final_signers: Option<&[&dyn Signer]>,
    max_sign_attempts: usize,
    use_rpc: bool,
    commitment: CommitmentConfig,
    send_transaction_config: RpcSendTransactionConfig,
) -> Result<Option<Signature>> {
    // Handle initial message (e.g., buffer creation)
    if let Some(message) = initial_message {
        if let Some(initial_signer) = initial_signer {
            let mut initial_transaction = Transaction::new_unsigned(message.clone());
            let blockhash = rpc_client.get_latest_blockhash()?;

            // Sign based on number of required signatures
            if message.header.num_required_signatures == 3 {
                initial_transaction.try_sign(
                    &[fee_payer_signer, initial_signer, write_signer.unwrap()],
                    blockhash,
                )?;
            } else if message.header.num_required_signatures == 2 {
                initial_transaction.try_sign(&[fee_payer_signer, initial_signer], blockhash)?;
            } else {
                initial_transaction.try_sign(&[fee_payer_signer], blockhash)?;
            }

            rpc_client
                .send_and_confirm_transaction_with_spinner_and_config(
                    &initial_transaction,
                    commitment,
                    send_transaction_config,
                )
                .map_err(|err| anyhow!("Account allocation failed: {}", err))?;
        } else {
            return Err(anyhow!(
                "Buffer account not created yet, must provide a key pair"
            ));
        }
    }

    if !write_messages.is_empty() {
        if let Some(write_signer) = write_signer {
            write_messages = simulate_and_update_write_compute_unit_limit(
                rpc_client,
                write_messages,
                fee_payer_signer,
                write_signer,
            )?;

            send_messages_in_batches(
                rpc_client,
                &write_messages,
                &[fee_payer_signer, write_signer],
                max_sign_attempts,
                use_rpc,
                commitment,
                send_transaction_config,
            )?;
        }
    }

    if let Some(message) = final_message {
        if let Some(final_signers) = final_signers {
            let mut final_tx = Transaction::new_unsigned(message);
            let blockhash = rpc_client.get_latest_blockhash()?;
            let mut signers = final_signers.to_vec();
            signers.push(fee_payer_signer);
            final_tx.try_sign(&signers, blockhash)?;

            return Ok(Some(
                rpc_client
                    .send_and_confirm_transaction_with_spinner_and_config(
                        &final_tx,
                        commitment,
                        send_transaction_config,
                    )
                    .map_err(|e| anyhow!("Deploying program failed: {}", e))?,
            ));
        }
    }

    Ok(None)
}

/// Complete buffer writing implementation. If `existing_buffer_data` is
/// `Some`, the on-chain buffer already exists (resume case): skip the
/// `CreateBuffer` ix and only send writes for chunks that differ.
#[allow(clippy::too_many_arguments)]
pub fn write_program_buffer(
    rpc_client: &RpcClient,
    payer: &dyn Signer,
    program_data: &[u8],
    buffer_authority: &Pubkey,
    buffer_keypair: &dyn Signer,
    commitment: CommitmentConfig,
    send_transaction_config: RpcSendTransactionConfig,
    priority_fee: Option<u64>,
    max_sign_attempts: usize,
    use_rpc: bool,
    existing_buffer_data: Option<Vec<u8>>,
) -> Result<Pubkey> {
    let buffer_pubkey = buffer_keypair.pubkey();

    let program_len = program_data.len();
    let buffer_len = program_len;

    // Get blockhash for all messages
    let blockhash = rpc_client.get_latest_blockhash()?;

    // Build CreateBuffer ix only if the buffer doesn't already exist on-chain.
    // On resume, we skip this and the loader keeps the existing account.
    let initial_message = if existing_buffer_data.is_none() {
        let buffer_data_len = UpgradeableLoaderState::size_of_buffer(buffer_len);
        let min_balance = rpc_client
            .get_minimum_balance_for_rent_exemption(buffer_data_len)
            .map_err(|e| anyhow!("Failed to get rent exemption: {}", e))?;
        let create_ixs = loader_v3_instruction::create_buffer(
            &payer.pubkey(),
            &buffer_pubkey,
            buffer_authority,
            min_balance,
            buffer_len,
        )
        .map_err(|e| anyhow!("Failed to create buffer instruction: {}", e))?;

        // Carry the same priority fee Write txs use.
        let mut initial_instructions: Vec<Instruction> = Vec::with_capacity(create_ixs.len() + 1);
        if let Some(price) = priority_fee {
            if price > 0 {
                initial_instructions.push(ComputeBudgetInstruction::set_compute_unit_price(price));
            }
        }
        initial_instructions.extend(create_ixs);

        Some(Message::new_with_blockhash(
            &initial_instructions,
            Some(&payer.pubkey()),
            &blockhash,
        ))
    } else {
        None
    };

    // Prepare write messages — skip chunks that already match on-chain bytes
    let write_messages = prepare_write_messages(
        program_data,
        &buffer_pubkey,
        buffer_authority,
        &payer.pubkey(),
        &blockhash,
        priority_fee,
        existing_buffer_data.as_deref(),
    );

    send_deploy_messages(
        rpc_client,
        initial_message,
        write_messages,
        None,
        payer,
        Some(buffer_keypair),
        Some(payer),
        None,
        max_sign_attempts,
        use_rpc,
        commitment,
        send_transaction_config,
    )?;
    Ok(buffer_pubkey)
}

/// Prepare write messages. When `existing_buffer_data` is provided, skip
/// chunks that already match on-chain bytes — letting resume after a failed
/// deploy only re-send the chunks that didn't land.
fn prepare_write_messages(
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

/// Send messages in parallel
fn send_messages_in_batches(
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

#[cfg(test)]
mod tests {
    use {
        super::*,
        std::{collections::BTreeSet, fs, path::Path},
        tempfile::tempdir,
    };

    fn write_file(path: &Path, contents: &str) {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, contents).unwrap();
    }

    fn create_program(root: &Path, name: &str) {
        write_file(
            &root.join("Cargo.toml"),
            &format!(
                r#"[package]
name = "{name}"
version = "0.1.0"
edition = "2021"

[lib]
crate-type = ["cdylib", "lib"]
"#
            ),
        );
        write_file(&root.join("src").join("lib.rs"), "");
    }

    fn create_workspace(root: &Path) {
        write_file(
            &root.join("Cargo.toml"),
            r#"[workspace]
members = ["programs/*"]
resolver = "2"
"#,
        );
        create_program(&root.join("programs").join("foo"), "foo");
        create_program(&root.join("programs").join("bar"), "bar");
    }

    #[test]
    fn find_nearest_manifest_path_finds_closest_cargo_toml() {
        let dir = tempdir().unwrap();
        create_workspace(dir.path());

        let program_dir = dir.path().join("programs").join("foo");
        let nested_dir = program_dir.join("src");

        assert_eq!(
            find_nearest_manifest_path(&nested_dir),
            Some(program_dir.join("Cargo.toml"))
        );
    }

    #[test]
    fn discover_solana_programs_finds_sibling_programs_from_nested_member() {
        let dir = tempdir().unwrap();
        create_workspace(dir.path());

        let root_programs =
            discover_solana_programs_from_path(dir.path(), Some("bar".into())).unwrap();
        let nested_programs = discover_solana_programs_from_path(
            &dir.path().join("programs").join("foo"),
            Some("bar".into()),
        )
        .unwrap();

        assert_eq!(root_programs.len(), 1);
        assert_eq!(nested_programs.len(), 1);
        assert_eq!(root_programs[0].lib_name, "bar");
        assert_eq!(nested_programs[0].lib_name, "bar");
        assert_eq!(root_programs[0].path, nested_programs[0].path);
    }

    #[test]
    fn discover_solana_programs_lists_all_members_from_nested_member() {
        let dir = tempdir().unwrap();
        create_workspace(dir.path());

        let programs =
            discover_solana_programs_from_path(&dir.path().join("programs").join("foo"), None)
                .unwrap();

        let names = programs
            .into_iter()
            .map(|program| program.lib_name)
            .collect::<BTreeSet<_>>();

        assert_eq!(
            names,
            BTreeSet::from(["bar".to_string(), "foo".to_string()])
        );
    }

    #[test]
    fn discover_solana_programs_errors_for_nonmember_current_crate() {
        let dir = tempdir().unwrap();
        create_workspace(dir.path());
        create_program(&dir.path().join("tools").join("baz"), "baz");

        let err = discover_solana_programs_from_path(&dir.path().join("tools").join("baz"), None)
            .unwrap_err()
            .to_string();

        assert!(err.contains("current package believes it's in a workspace when it's not"));
    }

    #[test]
    fn test_adjust_extend_bytes() {
        // Delta below the minimum, with ample headroom: bumped to the minimum.
        assert_eq!(
            adjust_extend_bytes(1, 100_000),
            MINIMUM_EXTEND_PROGRAM_BYTES
        );
        assert_eq!(
            adjust_extend_bytes(MINIMUM_EXTEND_PROGRAM_BYTES - 1, 100_000),
            MINIMUM_EXTEND_PROGRAM_BYTES
        );

        // Delta at or above the minimum: returned untouched.
        assert_eq!(
            adjust_extend_bytes(MINIMUM_EXTEND_PROGRAM_BYTES, 100_000),
            MINIMUM_EXTEND_PROGRAM_BYTES
        );
        assert_eq!(adjust_extend_bytes(50_000, 100_000), 50_000);

        // Within 10 KiB of the max: a small delta grows only to the exact
        // remaining headroom.
        let headroom = 4_096usize;
        let current_total_len = MAX_PERMITTED_DATA_LENGTH as usize - headroom;
        assert_eq!(adjust_extend_bytes(1, current_total_len), headroom as u32);
        // Requesting exactly the headroom stays unchanged.
        assert_eq!(
            adjust_extend_bytes(headroom as u32, current_total_len),
            headroom as u32
        );
        // At the maximum there is zero headroom: never grow.
        assert_eq!(
            adjust_extend_bytes(1, MAX_PERMITTED_DATA_LENGTH as usize),
            1
        );
    }
}
