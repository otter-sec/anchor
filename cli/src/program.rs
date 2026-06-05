use {
    crate::{
        config::{Config, Manifest, Program, WithPath},
        ConfigOverride, ProgramCommand,
    },
    anchor_lang_idl::types::Idl,
    anyhow::{anyhow, bail, Result},
    solana_client::send_and_confirm_transactions_in_parallel::{
        send_and_confirm_transactions_in_parallel_blocking_v2, SendAndConfirmConfigV2,
    },
    solana_commitment_config::CommitmentConfig,
    solana_keypair::Keypair,
    solana_loader_v3_interface::{
        instruction as loader_v3_instruction, state::UpgradeableLoaderState,
    },
    solana_message::{Hash, Message},
    solana_packet::PACKET_DATA_SIZE,
    solana_pubkey::Pubkey,
    solana_rpc_client::rpc_client::RpcClient,
    solana_rpc_client_api::config::RpcSendTransactionConfig,
    solana_sdk_ids::bpf_loader_upgradeable as bpf_loader_upgradeable_id,
    solana_signature::Signature,
    solana_signer::{EncodableKey, Signer},
    solana_transaction::Transaction,
    std::{
        fs::{self, File},
        io::Write,
        path::Path,
        sync::Arc,
        thread,
        time::Duration,
    },
};

/// Parse priority fee from solana args
fn parse_priority_fee_from_args(args: &[String]) -> Option<u64> {
    args.windows(2)
        .find(|pair| pair[0] == "--with-compute-unit-price")
        .and_then(|pair| pair[1].parse().ok())
}

/// Discover Solana programs from a non-Anchor Cargo workspace
pub fn discover_solana_programs(program_name: Option<String>) -> Result<Vec<Program>> {
    let current_dir = std::env::current_dir()?;
    let mut program_paths = Vec::new();

    // Check if current directory has Cargo.toml
    let cargo_toml_path = current_dir.join("Cargo.toml");
    if cargo_toml_path.exists() {
        let cargo_content = fs::read_to_string(&cargo_toml_path)?;
        let cargo_toml: toml::Value = toml::from_str(&cargo_content)?;

        // Check if it's a workspace Cargo.toml
        if let Some(workspace) = cargo_toml.get("workspace") {
            // It's a workspace - iterate over members
            if let Some(members) = workspace.get("members").and_then(|m| m.as_array()) {
                for member in members {
                    if let Some(member_path) = member.as_str() {
                        let full_path = current_dir.join(member_path);
                        if full_path.is_dir() && full_path.join("Cargo.toml").exists() {
                            program_paths.push(full_path);
                        }
                    }
                }
            }
        } else if is_solana_program(&current_dir)? {
            // It's a single program Cargo.toml with cdylib - use current directory
            program_paths.push(current_dir.clone());
        }
    }

    // If no programs found yet, fallback to looking in programs/ directory
    if program_paths.is_empty() {
        let programs_dir = current_dir.join("programs");
        if programs_dir.is_dir() {
            for entry in fs::read_dir(programs_dir)? {
                let entry = entry?;
                let path = entry.path();
                if path.is_dir() && path.join("Cargo.toml").exists() {
                    program_paths.push(path);
                }
            }
        }
    }

    // Filter to only Solana programs and build Program structs
    let mut programs = Vec::new();
    for path in program_paths {
        if !is_solana_program(&path)? {
            continue;
        }

        let cargo = Manifest::from_path(path.join("Cargo.toml"))?;
        let lib_name = cargo.lib_name()?;

        // Check if this is the program we're looking for (if name specified)
        if let Some(ref name) = program_name {
            let matches = *name == lib_name || *name == path.file_name().unwrap().to_str().unwrap();
            if !matches {
                continue;
            }
        }

        // Try to read IDL if it exists (will be None for non-Anchor programs)
        let idl_filepath = current_dir
            .join("target")
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

/// Check if a given Cargo project is a Solana program
/// A deployable Solana program must have crate-type = ["cdylib", ...]
fn is_solana_program(path: &Path) -> Result<bool> {
    let cargo_path = path.join("Cargo.toml");
    if !cargo_path.exists() {
        return Ok(false);
    }

    let cargo_content = fs::read_to_string(&cargo_path)?;
    let cargo_toml: toml::Value = toml::from_str(&cargo_content)?;

    // Check if it has cdylib (required for deployable Solana programs)
    // This is the definitive marker - libraries and client tools won't have this
    if let Some(lib) = cargo_toml.get("lib") {
        if let Some(crate_type) = lib.get("crate-type").and_then(|ct| ct.as_array()) {
            if crate_type.iter().any(|ct| ct.as_str() == Some("cdylib")) {
                return Ok(true);
            }
        }
    }

    Ok(false)
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
                 non-Anchor) with programs in the programs/ directory, or provide a program \
                 filepath.",
                name
            ));
        } else {
            return Err(anyhow!(
                "No Solana programs found. Make sure you're in a Solana workspace (Anchor or \
                 non-Anchor) with programs in the programs/ directory, or provide a program \
                 filepath."
            ));
        }
    }

    Ok(programs)
}

/// Public entry point for deploying programs - validates and routes to appropriate handler
#[allow(clippy::too_many_arguments)]
pub fn process_deploy(
    cfg_override: &ConfigOverride,
    program_filepath: Option<String>,
    program_name: Option<String>,
    program_keypair: Option<String>,
    upgrade_authority: Option<String>,
    program_id: Option<Pubkey>,
    buffer: Option<Pubkey>,
    max_len: Option<usize>,
    verifiable: bool,
    no_idl: bool,
    make_final: bool,
    max_sign_attempts: usize,
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
            no_idl,
            make_final,
            max_sign_attempts,
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

        // Delegate to deploy_workspace
        return deploy_workspace(
            cfg_override,
            None, // program_name - deploy all
            program_keypair,
            verifiable,
            no_idl,
            make_final,
            max_sign_attempts,
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
        no_idl,
        make_final,
        max_sign_attempts,
        solana_args,
    )
}

/// Deploy all programs in workspace using native implementation
fn deploy_workspace(
    cfg_override: &ConfigOverride,
    program_name: Option<String>,
    program_keypair: Option<String>,
    verifiable: bool,
    no_idl: bool,
    make_final: bool,
    max_sign_attempts: usize,
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
        let binary_path = program.binary_path(verifiable);

        println!("\nDeploying program: {}", program.lib_name);

        let program_keypair_filepath = match &program_keypair {
            Some(path) => Some(path.clone()),
            None => {
                // Try to find program keypair
                let keypair_path = program
                    .keypair_file()
                    .ok()
                    .map(|kp| kp.path().display().to_string());
                keypair_path
            }
        };

        // Use the native program_deploy implementation
        program_deploy(
            cfg_override,
            Some(binary_path.display().to_string()),
            None, // program_name - not needed since we have filepath
            program_keypair_filepath,
            None, // upgrade_authority - uses wallet
            None, // program_id - derived from keypair
            None, // buffer
            None, // max_len
            no_idl,
            make_final,
            max_sign_attempts,
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
            no_idl,
            make_final,
            max_sign_attempts,
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
            false, // verifiable
            no_idl,
            make_final,
            max_sign_attempts,
            solana_args,
        ),
        ProgramCommand::WriteBuffer {
            program_filepath,
            program_name,
            buffer,
            buffer_authority,
            max_len,
            max_sign_attempts,
        } => program_write_buffer(
            cfg_override,
            program_filepath,
            program_name,
            buffer,
            buffer_authority,
            max_len,
            max_sign_attempts,
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
            solana_args,
        } => program_upgrade(
            cfg_override,
            program_id,
            program_filepath,
            program_name,
            buffer,
            upgrade_authority,
            max_retries,
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
    program_filepath: Option<String>,
    program_name: Option<String>,
    program_keypair: Option<String>,
    upgrade_authority: Option<String>,
    program_id: Option<Pubkey>,
    buffer: Option<Pubkey>,
    max_len: Option<usize>,
    no_idl: bool,
    make_final: bool,
    max_sign_attempts: usize,
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
        let binary_path = program.binary_path(false); // false = not verifiable build

        println!("Deploying program: {}", program.lib_name);

        binary_path.display().to_string()
    };

    // Augment solana_args with recommended defaults (priority fees, max sign attempts, buffer)
    let solana_args = crate::add_recommended_deployment_solana_args(&rpc_client, solana_args)?;

    // Parse priority fee from solana_args
    let priority_fee = parse_priority_fee_from_args(&solana_args);

    // Read program data
    let program_data = fs::read(&program_filepath)
        .map_err(|e| anyhow!("Failed to read program file {}: {}", program_filepath, e))?;

    // Determine program keypair
    let loaded_program_keypair = if let Some(keypair_path) = program_keypair {
        // Load from specified keypair file
        Keypair::read_from_file(&keypair_path).map_err(|e| {
            anyhow!(
                "Failed to read program keypair from {}: {}",
                keypair_path,
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

        let keypair_path = format!("target/deploy/{}-keypair.json", program_name);
        Keypair::read_from_file(&keypair_path).map_err(|e| {
            anyhow!(
                "Failed to read program keypair from {}: {}. Use --program-keypair to specify a \
                 custom location.",
                keypair_path,
                e
            )
        })?
    };

    let program_id = loaded_program_keypair.pubkey();

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

    // Check if program already exists
    let program_account = rpc_client.get_account(&program_id);

    if program_account.is_ok() {
        // Program exists - validate it can be upgraded BEFORE writing buffer
        println!("Program already exists, upgrading...");

        // Verify program can be upgraded before doing expensive buffer write
        verify_program_can_be_upgraded(&rpc_client, &program_id, &upgrade_authority)?;

        // Write to buffer
        let buffer_pubkey = if let Some(buffer) = buffer {
            buffer
        } else {
            let buffer_keypair = Keypair::new();
            write_program_buffer(
                &rpc_client,
                &payer,
                &program_data,
                &upgrade_authority.pubkey(),
                &buffer_keypair,
                max_len,
                max_sign_attempts,
                CommitmentConfig::confirmed(),
                RpcSendTransactionConfig {
                    skip_preflight: false,
                    preflight_commitment: Some(CommitmentConfig::confirmed().commitment),
                    encoding: None,
                    max_retries: None,
                    min_context_slot: None,
                },
            )?
        };

        // Upgrade the program (skip verification - already done above at line 324)
        upgrade_program(
            &rpc_client,
            &payer,
            &program_id,
            &buffer_pubkey,
            &upgrade_authority,
            priority_fee,
            true, // skip_program_verification
        )?;
    } else {
        // New deployment

        let buffer_pubkey = if let Some(buffer) = buffer {
            buffer
        } else {
            let buffer_keypair = Keypair::new();
            write_program_buffer(
                &rpc_client,
                &payer,
                &program_data,
                &upgrade_authority.pubkey(),
                &buffer_keypair,
                max_len,
                max_sign_attempts,
                CommitmentConfig::confirmed(),
                RpcSendTransactionConfig {
                    skip_preflight: false,
                    preflight_commitment: Some(CommitmentConfig::confirmed().commitment),
                    encoding: None,
                    max_retries: None,
                    min_context_slot: None,
                },
            )?
        };

        // Deploy from buffer
        let max_data_len = max_len.unwrap_or(program_data.len());
        deploy_program(
            &rpc_client,
            &payer,
            &buffer_pubkey,
            &loaded_program_keypair,
            &upgrade_authority,
            max_data_len,
            priority_fee,
        )?;
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
        let idl_filepath = format!("target/idl/{}.json", program_name);

        if Path::new(&idl_filepath).exists() {
            // Read and update the IDL with the program address
            let idl_content = fs::read_to_string(&idl_filepath)
                .map_err(|e| anyhow!("Failed to read IDL file {}: {}", idl_filepath, e))?;

            let mut idl: Idl = serde_json::from_str(&idl_content)
                .map_err(|e| anyhow!("Failed to parse IDL file {}: {}", idl_filepath, e))?;

            // Update the IDL with the program address
            idl.address = program_id.to_string();

            // Write the updated IDL back to the file
            let idl_json = serde_json::to_string_pretty(&idl)
                .map_err(|e| anyhow!("Failed to serialize IDL: {}", e))?;
            fs::write(&idl_filepath, idl_json)
                .map_err(|e| anyhow!("Failed to write IDL file {}: {}", idl_filepath, e))?;

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
                idl_filepath
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
fn deploy_program(
    rpc_client: &RpcClient,
    payer: &Keypair,
    buffer: &Pubkey,
    program_keypair: &Keypair,
    upgrade_authority: &Keypair,
    max_data_len: usize,
    priority_fee: Option<u64>,
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

    // Add priority fee if specified
    deploy_ixs = crate::prepend_compute_unit_ix(deploy_ixs, rpc_client, priority_fee);

    let recent_blockhash = rpc_client.get_latest_blockhash()?;
    let deploy_tx = Transaction::new_signed_with_payer(
        &deploy_ixs,
        Some(&payer.pubkey()),
        &[payer, program_keypair, upgrade_authority],
        recent_blockhash,
    );

    rpc_client
        .send_and_confirm_transaction(&deploy_tx)
        .map_err(|e| anyhow!("Failed to deploy program: {}", e))?;

    Ok(())
}

fn upgrade_program(
    rpc_client: &RpcClient,
    payer: &Keypair,
    program_id: &Pubkey,
    buffer: &Pubkey,
    upgrade_authority: &Keypair,
    priority_fee: Option<u64>,
    skip_program_verification: bool,
) -> Result<()> {
    // Verify program can be upgraded (unless caller already verified)
    if !skip_program_verification {
        verify_program_can_be_upgraded(rpc_client, program_id, upgrade_authority)?;
    }

    // Verify the buffer account is valid
    verify_buffer_account(rpc_client, buffer, &upgrade_authority.pubkey())?;

    println!("Sending upgrade transaction...");

    let upgrade_ix = loader_v3_instruction::upgrade(
        program_id,
        buffer,
        &upgrade_authority.pubkey(),
        &payer.pubkey(),
    );

    // Add priority fee if specified
    let upgrade_ixs = crate::prepend_compute_unit_ix(vec![upgrade_ix], rpc_client, priority_fee);

    let recent_blockhash = rpc_client.get_latest_blockhash()?;
    let upgrade_tx = Transaction::new_signed_with_payer(
        &upgrade_ixs,
        Some(&payer.pubkey()),
        &[payer, upgrade_authority],
        recent_blockhash,
    );

    let signature = rpc_client
        .send_and_confirm_transaction(&upgrade_tx)
        .map_err(|e| anyhow!("Failed to upgrade program: {}", e))?;
    println!("Signature: {}", signature);
    Ok(())
}

fn program_write_buffer(
    cfg_override: &ConfigOverride,
    program_filepath: Option<String>,
    program_name: Option<String>,
    _buffer: Option<String>,
    buffer_authority: Option<String>,
    max_len: Option<usize>,
    max_sign_attempts: usize,
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
        let binary_path = program.binary_path(false);

        println!("Writing buffer for program: {}", program.lib_name);

        binary_path.display().to_string()
    };

    // Read program data
    let program_data = fs::read(&program_filepath)
        .map_err(|e| anyhow!("Failed to read program file {}: {}", program_filepath, e))?;

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
        max_len,
        max_sign_attempts,
        CommitmentConfig::confirmed(),
        RpcSendTransactionConfig {
            skip_preflight: false,
            preflight_commitment: Some(CommitmentConfig::confirmed().commitment),
            encoding: None,
            max_retries: None,
            min_context_slot: None,
        },
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
    program_filepath: Option<String>,
    program_name: Option<String>,
    buffer: Option<Pubkey>,
    upgrade_authority: Option<String>,
    max_retries: u32,
    solana_args: Vec<String>,
) -> Result<()> {
    let (rpc_client, config) = get_rpc_client_and_config(cfg_override)?;
    let payer = get_payer_keypair(cfg_override, &config)?;

    // Augment solana_args with recommended defaults if provided
    let solana_args = if !solana_args.is_empty() {
        crate::add_recommended_deployment_solana_args(&rpc_client, solana_args)?
    } else {
        solana_args
    };

    // Parse priority fee from solana_args
    let priority_fee = parse_priority_fee_from_args(&solana_args);

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
        return upgrade_program(
            &rpc_client,
            &payer,
            &program_id,
            &buffer_pubkey,
            &upgrade_authority_keypair,
            priority_fee,
            true, // skip_program_verification - already done above
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
        let binary_path = program.binary_path(false); // false = not verifiable build

        println!("Upgrading program: {}", program.lib_name);

        binary_path.display().to_string()
    };

    let program_data = fs::read(&program_filepath)
        .map_err(|e| anyhow!("Failed to read program file {}: {}", program_filepath, e))?;

    // Retry loop for buffer creation and upgrade
    for retry in 0..(1 + max_retries) {
        if max_retries > 0 {
            println!("\nAttempt {}/{}", retry + 1, max_retries + 1);
        }

        // Create a new buffer for each attempt
        let buffer_keypair = Keypair::new();

        // Write to buffer
        let result = write_program_buffer(
            &rpc_client,
            &payer,
            &program_data,
            &upgrade_authority_keypair.pubkey(),
            &buffer_keypair,
            None, // max_len
            5,    // max_sign_attempts
            CommitmentConfig::confirmed(),
            RpcSendTransactionConfig {
                skip_preflight: false,
                preflight_commitment: Some(CommitmentConfig::confirmed().commitment),
                encoding: None,
                max_retries: None,
                min_context_slot: None,
            },
        );

        let buffer_pubkey = match result {
            Ok(pubkey) => pubkey,
            Err(e) => {
                println!("Buffer write failed: {}", e);
                if retry < max_retries {
                    println!("Retrying {} more time(s)...", max_retries - retry);
                    continue;
                } else {
                    return Err(e);
                }
            }
        };

        // Upgrade the program (skip verification - already done before retry loop)
        let result = upgrade_program(
            &rpc_client,
            &payer,
            &program_id,
            &buffer_pubkey,
            &upgrade_authority_keypair,
            priority_fee,
            true, // skip_program_verification
        );

        match result {
            Ok(_) => {
                if max_retries > 0 {
                    println!("\nUpgrade success");
                }
                return Ok(());
            }
            Err(e) => {
                println!("Upgrade failed: {}", e);
                if retry < max_retries {
                    println!("Retrying {} more time(s)...", max_retries - retry);
                } else {
                    return Err(e);
                }
            }
        }
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

    // Use the checked version which requires upgrade authority signature
    let extend_ix = loader_v3_instruction::extend_program_checked(
        &program_id,
        &upgrade_authority_address,
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

    println!("Program extended succesfully!");
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

#[allow(clippy::too_many_arguments)]
pub fn send_deploy_messages(
    rpc_client: &RpcClient,
    initial_message: Option<Message>,
    write_messages: Vec<Message>,
    final_message: Option<Message>,
    fee_payer_signer: &dyn Signer,
    initial_signer: Option<&dyn Signer>,
    write_signer: Option<&dyn Signer>,
    final_signers: Option<&[&dyn Signer]>,
    max_sign_attempts: usize,
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
            send_messages_in_batches(
                rpc_client,
                &write_messages,
                &[fee_payer_signer, write_signer],
                max_sign_attempts,
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

/// Complete buffer writing implementation
#[allow(clippy::too_many_arguments)]
pub fn write_program_buffer(
    rpc_client: &RpcClient,
    payer: &dyn Signer,
    program_data: &[u8],
    buffer_authority: &Pubkey,
    buffer_keypair: &dyn Signer,
    max_len: Option<usize>,
    max_sign_attempts: usize,
    commitment: CommitmentConfig,
    send_transaction_config: RpcSendTransactionConfig,
) -> Result<Pubkey> {
    let buffer_pubkey = buffer_keypair.pubkey();

    let program_len = program_data.len();
    let buffer_len = max_len.unwrap_or(program_len);

    // Calculate required lamports for buffer
    let buffer_data_len = UpgradeableLoaderState::size_of_buffer(buffer_len);
    let min_balance = rpc_client
        .get_minimum_balance_for_rent_exemption(buffer_data_len)
        .map_err(|e| anyhow!("Failed to get rent exemption: {}", e))?;

    // Get blockhash for all messages
    let blockhash = rpc_client.get_latest_blockhash()?;

    // Create buffer initialization message
    let initial_instructions = loader_v3_instruction::create_buffer(
        &payer.pubkey(),
        &buffer_pubkey,
        buffer_authority,
        min_balance,
        buffer_len,
    )
    .map_err(|e| anyhow!("Failed to create buffer instruction: {}", e))?;

    let initial_message = Some(Message::new_with_blockhash(
        &initial_instructions,
        Some(&payer.pubkey()),
        &blockhash,
    ));

    // Prepare all write messages upfront
    let write_messages = prepare_write_messages(
        program_data,
        &buffer_pubkey,
        buffer_authority,
        &payer.pubkey(),
        &blockhash,
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
        commitment,
        send_transaction_config,
    )?;
    Ok(buffer_pubkey)
}

/// Prepare write messages
fn prepare_write_messages(
    program_data: &[u8],
    buffer_pubkey: &Pubkey,
    buffer_authority: &Pubkey,
    fee_payer: &Pubkey,
    blockhash: &Hash,
) -> Vec<Message> {
    let create_msg = |offset: u32, bytes: Vec<u8>| {
        let instruction =
            loader_v3_instruction::write(buffer_pubkey, buffer_authority, offset, bytes);
        Message::new_with_blockhash(&[instruction], Some(fee_payer), blockhash)
    };

    let mut write_messages = Vec::new();
    let chunk_size = calculate_max_chunk_size(create_msg(0, Vec::new()));

    for (chunk, i) in program_data.chunks(chunk_size).zip(0usize..) {
        let offset = i.saturating_mul(chunk_size);
        write_messages.push(create_msg(offset as u32, chunk.to_vec()));
    }

    write_messages
}

/// Send messages in parallel
fn send_messages_in_batches(
    rpc_client: &RpcClient,
    messages: &[Message],
    signers: &[&dyn Signer],
    max_sign_attempts: usize,
    commitment: CommitmentConfig,
    send_config: RpcSendTransactionConfig,
) -> Result<()> {
    // Use parallel send and confirm function
    // Create a new RpcClient with the same URL and wrap in Arc for parallel processing
    let url = rpc_client.url();
    let new_rpc_client = RpcClient::new_with_commitment(url, commitment);
    let rpc_client_arc = Arc::new(new_rpc_client);

    let transaction_errors = send_and_confirm_transactions_in_parallel_blocking_v2(
        rpc_client_arc,
        None,
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
