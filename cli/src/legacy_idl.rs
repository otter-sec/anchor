//! Deprecated: Legacy on-chain IDL account management.
//!
//! This module re-implements the old Anchor IDL instruction protocol for backward
//! compatibility. It will be removed in a future release. Migrate to Program
//! Metadata-based IDL management (`anchor idl` commands).

use {
    crate::{
        abs_path::AbsolutePath,
        cluster_url,
        config::{Config, ConfigOverride, WithPath},
        create_client, prepend_compute_unit_ix, with_workspace,
    },
    anchor_cli_macros::AbsolutePath,
    anchor_lang::{
        idl::{IdlAccount, IdlInstruction, ERASED_AUTHORITY, IDL_IX_TAG},
        AccountDeserialize, AnchorDeserialize, AnchorSerialize, Discriminator,
    },
    anchor_lang_idl::types::Idl,
    anyhow::{anyhow, Result},
    clap::Parser,
    flate2::{read::ZlibDecoder, write::ZlibEncoder, Compression},
    solana_instruction::{AccountMeta, Instruction},
    solana_keypair::Keypair,
    solana_pubkey::Pubkey,
    solana_rpc_client::rpc_client::RpcClient,
    solana_signer::Signer,
    solana_transaction::Transaction,
    std::{fs, io::prelude::*},
};

/// Deprecated: Legacy on-chain IDL subcommands.
///
/// These commands interact with the old Anchor IDL instruction protocol stored
/// on-chain. They will be removed in a future Anchor release.
#[derive(Debug, Parser, AbsolutePath)]
pub enum LegacyIdlCommand {
    /// [DEPRECATED] Close the legacy IDL account and recover rent.
    Close {
        program_id: Pubkey,
        /// The IDL account to close. If none is given, the IDL account derived from program_id is used.
        #[clap(long)]
        idl_address: Option<Pubkey>,
        /// Print the instruction in base64 without executing it.
        /// Useful for multisig execution when the local wallet keypair is not available.
        #[clap(long)]
        print_only: bool,
        #[clap(long)]
        priority_fee: Option<u64>,
    },
    /// [DEPRECATED] Write an IDL into a legacy buffer account. Use with set-buffer to upgrade.
    WriteBuffer {
        program_id: Pubkey,
        #[clap(short, long)]
        filepath: String,
        #[clap(long)]
        priority_fee: Option<u64>,
    },
    /// [DEPRECATED] Set a new IDL buffer for the program.
    SetBuffer {
        program_id: Pubkey,
        /// Address of the buffer account to set as the IDL on the program.
        #[clap(short, long)]
        buffer: Pubkey,
        /// Print the instruction in base64 without executing it.
        #[clap(long)]
        print_only: bool,
        #[clap(long)]
        priority_fee: Option<u64>,
    },
    /// [DEPRECATED] Set a new authority on the legacy IDL account.
    SetAuthority {
        /// The IDL account buffer to set the authority of. If none is given,
        /// the canonical IDL account is used.
        address: Option<Pubkey>,
        /// Program to change the IDL authority.
        #[clap(short, long)]
        program_id: Pubkey,
        /// New authority of the IDL account.
        #[clap(short, long)]
        new_authority: Pubkey,
        /// Print the instruction in base64 without executing it.
        #[clap(long)]
        print_only: bool,
        #[clap(long)]
        priority_fee: Option<u64>,
    },
    /// [DEPRECATED] Remove the ability to modify the legacy IDL account.
    EraseAuthority {
        #[clap(short, long)]
        program_id: Pubkey,
        #[clap(long)]
        priority_fee: Option<u64>,
    },
    /// [DEPRECATED] Output the authority for the legacy IDL account.
    Authority {
        /// The program to view.
        program_id: Pubkey,
    },
    /// [DEPRECATED] Initialize the legacy on-chain IDL account for the first time.
    Init {
        program_id: Pubkey,
        #[clap(short, long)]
        filepath: String,
        #[clap(long)]
        priority_fee: Option<u64>,
    },
    /// [DEPRECATED] Upgrade the legacy IDL (write buffer → set buffer → close buffer).
    Upgrade {
        program_id: Pubkey,
        #[clap(short, long)]
        filepath: String,
        #[clap(long)]
        priority_fee: Option<u64>,
    },
    /// [DEPRECATED] Fetch the IDL from a legacy on-chain IdlAccount.
    Fetch {
        address: Pubkey,
        /// Path to write the fetched IDL. Prints to stdout if omitted.
        #[clap(short, long)]
        out: Option<String>,
    },
}

/// Entry point for all legacy IDL subcommands.
///
/// Prints a deprecation warning on every invocation.
pub fn handle_legacy_idl_command(
    cfg_override: &ConfigOverride,
    subcmd: LegacyIdlCommand,
) -> Result<()> {
    eprintln!(
        "warning: You are using a deprecated legacy IDL command. These commands interact with the \
         old on-chain IDL instruction protocol and will be removed in a future Anchor release. \
         Please migrate to Program Metadata-based IDL management (`anchor idl`)."
    );
    match subcmd {
        LegacyIdlCommand::Close {
            program_id,
            idl_address,
            print_only,
            priority_fee,
        } => {
            let closed_address = idl_close(
                cfg_override,
                program_id,
                idl_address,
                print_only,
                priority_fee,
            )?;
            if !print_only {
                println!("Idl account closed: {closed_address}");
            }
            Ok(())
        }
        LegacyIdlCommand::WriteBuffer {
            program_id,
            filepath,
            priority_fee,
        } => {
            let idl_buffer = idl_write_buffer(cfg_override, program_id, filepath, priority_fee)?;
            println!("Idl buffer created: {idl_buffer}");
            Ok(())
        }
        LegacyIdlCommand::SetBuffer {
            program_id,
            buffer,
            print_only,
            priority_fee,
        } => idl_set_buffer(cfg_override, program_id, buffer, print_only, priority_fee).map(|_| ()),
        LegacyIdlCommand::SetAuthority {
            program_id,
            address,
            new_authority,
            print_only,
            priority_fee,
        } => idl_set_authority(
            cfg_override,
            program_id,
            address,
            new_authority,
            print_only,
            priority_fee,
        ),
        LegacyIdlCommand::EraseAuthority {
            program_id,
            priority_fee,
        } => idl_erase_authority(cfg_override, program_id, priority_fee),
        LegacyIdlCommand::Authority { program_id } => idl_authority(cfg_override, program_id),
        LegacyIdlCommand::Init {
            program_id,
            filepath,
            priority_fee,
        } => idl_init(cfg_override, program_id, filepath, priority_fee),
        LegacyIdlCommand::Upgrade {
            program_id,
            filepath,
            priority_fee,
        } => idl_upgrade(cfg_override, program_id, filepath, priority_fee),
        LegacyIdlCommand::Fetch { address, out } => idl_fetch_cmd(cfg_override, address, out),
    }
}

// ---------------------------------------------------------------------------
// Legacy helper: fetch the IDL JSON from a legacy on-chain IdlAccount.
//
// Intentionally returns `serde_json::Value` rather than `Idl` to also support
// pre-0.30 IDL formats.
// ---------------------------------------------------------------------------
fn fetch_idl(cfg_override: &ConfigOverride, idl_addr: Pubkey) -> Result<serde_json::Value> {
    let url = match crate::config::Config::discover(cfg_override)? {
        Some(cfg) => cluster_url(&cfg, &cfg.test_validator, &cfg.surfpool_config),
        None => {
            if let Some(cluster) = cfg_override.cluster.as_ref() {
                cluster.url().to_string()
            } else {
                crate::config::get_solana_cfg_url()?
            }
        }
    };

    let client = create_client(url);

    let mut account = client.get_account(&idl_addr)?;
    if account.executable {
        let idl_addr = IdlAccount::address(&idl_addr);
        account = client.get_account(&idl_addr)?;
    }

    // Cut off account discriminator.
    let mut d: &[u8] = &account.data[IdlAccount::DISCRIMINATOR.len()..];
    let idl_account: IdlAccount = AnchorDeserialize::deserialize(&mut d)?;

    let compressed_len: usize = idl_account.data_len.try_into().unwrap();
    let compressed_bytes = &account.data[44..44 + compressed_len];
    let mut z = ZlibDecoder::new(compressed_bytes);
    let mut s = Vec::new();
    z.read_to_end(&mut s)?;
    serde_json::from_slice(&s[..]).map_err(Into::into)
}

fn get_idl_account(client: &RpcClient, idl_address: &Pubkey) -> Result<IdlAccount> {
    let account = client.get_account(idl_address)?;
    let mut data: &[u8] = &account.data;
    AccountDeserialize::try_deserialize(&mut data).map_err(|e| anyhow!("{:?}", e))
}

fn idl_close(
    cfg_override: &ConfigOverride,
    program_id: Pubkey,
    idl_address: Option<Pubkey>,
    print_only: bool,
    priority_fee: Option<u64>,
) -> Result<Pubkey> {
    with_workspace(cfg_override, |cfg| {
        let idl_address = idl_address.unwrap_or_else(|| IdlAccount::address(&program_id));
        idl_close_account(cfg, &program_id, idl_address, print_only, priority_fee)?;
        Ok(idl_address)
    })?
}

fn idl_write_buffer(
    cfg_override: &ConfigOverride,
    program_id: Pubkey,
    idl_filepath: String,
    priority_fee: Option<u64>,
) -> Result<Pubkey> {
    with_workspace(cfg_override, |cfg| {
        let idl = fs::read(&idl_filepath)?;
        let idl = anchor_lang_idl::convert::convert_idl(&idl)?;

        let idl_buffer = create_idl_buffer(cfg, &program_id, &idl, priority_fee)?;
        idl_write(cfg, &program_id, &idl, idl_buffer, priority_fee)?;

        Ok(idl_buffer)
    })?
}

fn idl_set_buffer(
    cfg_override: &ConfigOverride,
    program_id: Pubkey,
    buffer: Pubkey,
    print_only: bool,
    priority_fee: Option<u64>,
) -> Result<Pubkey> {
    with_workspace(cfg_override, |cfg| {
        let keypair = cfg.wallet_kp()?;
        let url = cluster_url(cfg, &cfg.test_validator, &cfg.surfpool_config);
        let client = create_client(url);

        let idl_address = IdlAccount::address(&program_id);
        let idl_authority = if print_only {
            get_idl_account(&client, &idl_address)?.authority
        } else {
            keypair.pubkey()
        };

        let ix = {
            let accounts = vec![
                AccountMeta::new(buffer, false),
                AccountMeta::new(idl_address, false),
                AccountMeta::new(idl_authority, true),
            ];
            let mut data = IDL_IX_TAG.to_le_bytes().to_vec();
            data.append(&mut anchor_lang::prelude::borsh::to_vec(
                &IdlInstruction::SetBuffer,
            )?);
            Instruction {
                program_id,
                accounts,
                data,
            }
        };

        if print_only {
            print_idl_instruction("SetBuffer", &ix, &idl_address)?;
        } else {
            let instructions = prepend_compute_unit_ix(
                vec![ix],
                &client,
                priority_fee,
                &[buffer, idl_address, idl_authority],
            );

            let mut latest_hash = client.get_latest_blockhash()?;
            for retries in 0..20 {
                if !client.is_blockhash_valid(&latest_hash, client.commitment())? {
                    latest_hash = client.get_latest_blockhash()?;
                }
                let tx = Transaction::new_signed_with_payer(
                    &instructions,
                    Some(&keypair.pubkey()),
                    &[&keypair],
                    latest_hash,
                );
                match client.send_and_confirm_transaction_with_spinner(&tx) {
                    Ok(_) => break,
                    Err(e) => {
                        if retries == 19 {
                            return Err(anyhow!("Error: {e}. Failed to send transaction."));
                        }
                        println!("Error: {e}. Retrying transaction.");
                    }
                }
            }
        }

        Ok(idl_address)
    })?
}

fn idl_authority(cfg_override: &ConfigOverride, program_id: Pubkey) -> Result<()> {
    with_workspace(cfg_override, |cfg| {
        let url = cluster_url(cfg, &cfg.test_validator, &cfg.surfpool_config);
        let client = create_client(url);
        let idl_address = {
            let account = client.get_account(&program_id)?;
            if account.executable {
                IdlAccount::address(&program_id)
            } else {
                program_id
            }
        };

        let idl_account = get_idl_account(&client, &idl_address)?;
        println!("{:?}", idl_account.authority);
        Ok(())
    })?
}

fn idl_init(
    cfg_override: &ConfigOverride,
    program_id: Pubkey,
    idl_filepath: String,
    priority_fee: Option<u64>,
) -> Result<()> {
    with_workspace(cfg_override, |cfg| {
        let idl = fs::read(&idl_filepath)?;
        let idl = anchor_lang_idl::convert::convert_idl(&idl)?;
        let idl_address = create_idl_account(cfg, &program_id, &idl, priority_fee)?;
        println!("Idl account created: {idl_address}");
        Ok(())
    })?
}

fn idl_upgrade(
    cfg_override: &ConfigOverride,
    program_id: Pubkey,
    idl_filepath: String,
    priority_fee: Option<u64>,
) -> Result<()> {
    // Compose existing helpers — each manages its own workspace/client setup.
    let idl_buffer = idl_write_buffer(cfg_override, program_id, idl_filepath, priority_fee)?;
    idl_set_buffer(cfg_override, program_id, idl_buffer, false, priority_fee)?;
    idl_close(
        cfg_override,
        program_id,
        Some(idl_buffer),
        false,
        priority_fee,
    )?;
    let idl_address = IdlAccount::address(&program_id);
    println!("Idl account upgraded: {idl_address}");
    Ok(())
}

fn idl_fetch_cmd(
    cfg_override: &ConfigOverride,
    address: Pubkey,
    out: Option<String>,
) -> Result<()> {
    let idl = fetch_idl(cfg_override, address)?;
    let idl_json = serde_json::to_string_pretty(&idl)?;
    match out {
        None => println!("{idl_json}"),
        Some(path) => {
            let mut f = fs::File::create(&path)?;
            f.write_all(idl_json.as_bytes())?;
            println!("Wrote IDL to {path}");
        }
    }
    Ok(())
}

fn idl_set_authority(
    cfg_override: &ConfigOverride,
    program_id: Pubkey,
    address: Option<Pubkey>,
    new_authority: Pubkey,
    print_only: bool,
    priority_fee: Option<u64>,
) -> Result<()> {
    with_workspace(cfg_override, |cfg| {
        let idl_address = match address {
            None => IdlAccount::address(&program_id),
            Some(addr) => addr,
        };
        let keypair = cfg.wallet_kp()?;
        let url = cluster_url(cfg, &cfg.test_validator, &cfg.surfpool_config);
        let client = create_client(url);

        let idl_authority = if print_only {
            get_idl_account(&client, &idl_address)?.authority
        } else {
            keypair.pubkey()
        };

        let data = serialize_idl_ix(IdlInstruction::SetAuthority { new_authority })?;

        let accounts = vec![
            AccountMeta::new(idl_address, false),
            AccountMeta::new_readonly(idl_authority, true),
        ];
        let ix = Instruction {
            program_id,
            accounts,
            data,
        };

        if print_only {
            print_idl_instruction("SetAuthority", &ix, &idl_address)?;
        } else {
            let instructions =
                prepend_compute_unit_ix(vec![ix], &client, priority_fee, &[idl_address]);
            let latest_hash = client.get_latest_blockhash()?;
            let tx = Transaction::new_signed_with_payer(
                &instructions,
                Some(&keypair.pubkey()),
                &[&keypair],
                latest_hash,
            );
            client.send_and_confirm_transaction_with_spinner(&tx)?;
            println!("Authority update complete.");
        }

        Ok(())
    })?
}

fn idl_erase_authority(
    cfg_override: &ConfigOverride,
    program_id: Pubkey,
    priority_fee: Option<u64>,
) -> Result<()> {
    println!("Are you sure you want to erase the IDL authority: [y/n]");

    let stdin = std::io::stdin();
    let mut stdin_lines = stdin.lock().lines();
    let input = stdin_lines.next().unwrap().unwrap();
    if input != "y" {
        println!("Not erasing.");
        return Ok(());
    }
    idl_set_authority(
        cfg_override,
        program_id,
        None,
        ERASED_AUTHORITY,
        false,
        priority_fee,
    )
}

fn idl_close_account(
    cfg: &WithPath<Config>,
    program_id: &Pubkey,
    idl_address: Pubkey,
    print_only: bool,
    priority_fee: Option<u64>,
) -> Result<()> {
    let keypair = cfg.wallet_kp()?;
    let url = cluster_url(cfg, &cfg.test_validator, &cfg.surfpool_config);
    let client = create_client(url);

    let idl_authority = if print_only {
        get_idl_account(&client, &idl_address)?.authority
    } else {
        keypair.pubkey()
    };

    let accounts = vec![
        AccountMeta::new(idl_address, false),
        AccountMeta::new_readonly(idl_authority, true),
        AccountMeta::new(keypair.pubkey(), false),
    ];
    let ix = Instruction {
        program_id: *program_id,
        accounts,
        data: serialize_idl_ix(IdlInstruction::Close)?,
    };

    if print_only {
        print_idl_instruction("Close", &ix, &idl_address)?;
    } else {
        let instructions = prepend_compute_unit_ix(
            vec![ix],
            &client,
            priority_fee,
            &[idl_address, keypair.pubkey()],
        );
        let latest_hash = client.get_latest_blockhash()?;
        let tx = Transaction::new_signed_with_payer(
            &instructions,
            Some(&keypair.pubkey()),
            &[&keypair],
            latest_hash,
        );
        client.send_and_confirm_transaction_with_spinner(&tx)?;
    }

    Ok(())
}

// Write the IDL to the account buffer, chopping up the IDL into pieces and
// sending multiple transactions if the IDL doesn't fit into a single transaction.
fn idl_write(
    cfg: &WithPath<Config>,
    program_id: &Pubkey,
    idl: &Idl,
    idl_address: Pubkey,
    priority_fee: Option<u64>,
) -> Result<()> {
    let keypair = cfg.wallet_kp()?;
    let url = cluster_url(cfg, &cfg.test_validator, &cfg.surfpool_config);
    let client = create_client(url);

    let idl_data = {
        let json_bytes = serde_json::to_vec(idl)?;
        let mut e = ZlibEncoder::new(Vec::new(), Compression::default());
        e.write_all(&json_bytes)?;
        e.finish()?
    };

    println!("Idl data length: {:?} bytes", idl_data.len());

    const MAX_WRITE_SIZE: usize = 600;
    let mut offset = 0;
    while offset < idl_data.len() {
        println!("Step {offset}/{} ", idl_data.len());
        let data = {
            let start = offset;
            let end = std::cmp::min(offset + MAX_WRITE_SIZE, idl_data.len());
            serialize_idl_ix(IdlInstruction::Write {
                data: idl_data[start..end].to_vec(),
            })?
        };
        let accounts = vec![
            AccountMeta::new(idl_address, false),
            AccountMeta::new_readonly(keypair.pubkey(), true),
        ];
        let ix = Instruction {
            program_id: *program_id,
            accounts,
            data,
        };
        let instructions = prepend_compute_unit_ix(vec![ix], &client, priority_fee, &[idl_address]);

        let mut latest_hash = client.get_latest_blockhash()?;
        for retries in 0..20 {
            if !client.is_blockhash_valid(&latest_hash, client.commitment())? {
                latest_hash = client.get_latest_blockhash()?;
            }
            let tx = Transaction::new_signed_with_payer(
                &instructions,
                Some(&keypair.pubkey()),
                &[&keypair],
                latest_hash,
            );
            match client.send_and_confirm_transaction_with_spinner(&tx) {
                Ok(_) => break,
                Err(e) => {
                    if retries == 19 {
                        return Err(anyhow!("Error: {e}. Failed to send transaction."));
                    }
                    println!("Error: {e}. Retrying transaction.");
                }
            }
        }
        offset += MAX_WRITE_SIZE;
    }
    Ok(())
}

fn create_idl_account(
    cfg: &WithPath<Config>,
    program_id: &Pubkey,
    idl: &Idl,
    priority_fee: Option<u64>,
) -> Result<Pubkey> {
    let idl_address = IdlAccount::address(program_id);
    let keypair = cfg.wallet_kp()?;
    let url = cluster_url(cfg, &cfg.test_validator, &cfg.surfpool_config);
    let client = create_client(url);
    let idl_data = serialize_idl(idl)?;

    {
        let pda_max_growth = 60_000;
        let idl_header_size = 44;
        let idl_data_len = idl_data.len() as u64;
        if idl_data_len > pda_max_growth {
            return Err(anyhow!(
                "Your IDL is over 60kb and this isn't supported right now"
            ));
        }
        let data_len = (idl_data_len * 2).min(pda_max_growth - idl_header_size);
        let num_additional_instructions = data_len / 10000;
        let mut instructions = Vec::new();
        let data = serialize_idl_ix(IdlInstruction::Create { data_len })?;
        let program_signer = Pubkey::find_program_address(&[], program_id).0;
        let accounts = vec![
            AccountMeta::new_readonly(keypair.pubkey(), true),
            AccountMeta::new(idl_address, false),
            AccountMeta::new_readonly(program_signer, false),
            AccountMeta::new_readonly(solana_sdk_ids::system_program::ID, false),
            AccountMeta::new_readonly(*program_id, false),
        ];
        instructions.push(Instruction {
            program_id: *program_id,
            accounts,
            data,
        });

        for _ in 0..num_additional_instructions {
            let data = serialize_idl_ix(IdlInstruction::Resize { data_len })?;
            instructions.push(Instruction {
                program_id: *program_id,
                accounts: vec![
                    AccountMeta::new(idl_address, false),
                    AccountMeta::new_readonly(keypair.pubkey(), true),
                    AccountMeta::new_readonly(solana_sdk_ids::system_program::ID, false),
                ],
                data,
            });
        }
        instructions = prepend_compute_unit_ix(instructions, &client, priority_fee, &[idl_address]);

        let mut latest_hash = client.get_latest_blockhash()?;
        for retries in 0..20 {
            if !client.is_blockhash_valid(&latest_hash, client.commitment())? {
                latest_hash = client.get_latest_blockhash()?;
            }
            let tx = Transaction::new_signed_with_payer(
                &instructions,
                Some(&keypair.pubkey()),
                &[&keypair],
                latest_hash,
            );
            match client.send_and_confirm_transaction_with_spinner(&tx) {
                Ok(_) => break,
                Err(err) => {
                    if retries == 19 {
                        return Err(anyhow!("Error creating IDL account: {}", err));
                    }
                    println!("Error creating IDL account: {err}. Retrying...");
                }
            }
        }
    }

    idl_write(
        cfg,
        program_id,
        idl,
        IdlAccount::address(program_id),
        priority_fee,
    )?;

    Ok(idl_address)
}

fn create_idl_buffer(
    cfg: &WithPath<Config>,
    program_id: &Pubkey,
    idl: &Idl,
    priority_fee: Option<u64>,
) -> Result<Pubkey> {
    let keypair = cfg.wallet_kp()?;
    let url = cluster_url(cfg, &cfg.test_validator, &cfg.surfpool_config);
    let client = create_client(url);

    let buffer = Keypair::new();

    let create_account_ix = {
        let space = IdlAccount::DISCRIMINATOR.len() + 32 + 4 + serialize_idl(idl)?.len();
        let lamports = client.get_minimum_balance_for_rent_exemption(space)?;
        solana_system_interface::instruction::create_account(
            &keypair.pubkey(),
            &buffer.pubkey(),
            lamports,
            space as u64,
            program_id,
        )
    };

    let create_buffer_ix = {
        let accounts = vec![
            AccountMeta::new(buffer.pubkey(), true),
            AccountMeta::new_readonly(keypair.pubkey(), true),
        ];
        let mut data = IDL_IX_TAG.to_le_bytes().to_vec();
        data.append(&mut anchor_lang::prelude::borsh::to_vec(
            &IdlInstruction::CreateBuffer,
        )?);
        Instruction {
            program_id: *program_id,
            accounts,
            data,
        }
    };

    let instructions = prepend_compute_unit_ix(
        vec![create_account_ix, create_buffer_ix],
        &client,
        priority_fee,
        &[buffer.pubkey()],
    );

    let mut latest_hash = client.get_latest_blockhash()?;
    for retries in 0..20 {
        if !client.is_blockhash_valid(&latest_hash, client.commitment())? {
            latest_hash = client.get_latest_blockhash()?;
        }
        let tx = Transaction::new_signed_with_payer(
            &instructions,
            Some(&keypair.pubkey()),
            &[&keypair, &buffer],
            latest_hash,
        );
        match client.send_and_confirm_transaction_with_spinner(&tx) {
            Ok(_) => break,
            Err(err) => {
                if retries == 19 {
                    return Err(anyhow!("Error creating buffer account: {}", err));
                }
                println!("Error creating buffer account: {err}. Retrying...");
            }
        }
    }

    Ok(buffer.pubkey())
}

// Serialize and zlib-compress the IDL.
fn serialize_idl(idl: &Idl) -> Result<Vec<u8>> {
    let json_bytes = serde_json::to_vec(idl)?;
    let mut e = ZlibEncoder::new(Vec::new(), Compression::default());
    e.write_all(&json_bytes)?;
    e.finish().map_err(Into::into)
}

fn serialize_idl_ix(ix_inner: IdlInstruction) -> Result<Vec<u8>> {
    let mut data = Vec::with_capacity(256);
    data.extend_from_slice(&IDL_IX_TAG.to_le_bytes());
    ix_inner.serialize(&mut data)?;
    Ok(data)
}

/// Print a `base64+borsh` encoded IDL instruction (print-only mode).
fn print_idl_instruction(ix_name: &str, ix: &Instruction, idl_address: &Pubkey) -> Result<()> {
    use base64::{engine::general_purpose::STANDARD, Engine};

    println!("Print only mode. No execution!");
    println!("Instruction: {ix_name}");
    println!("IDL address: {idl_address}");
    println!("Program: {}", ix.program_id);

    // Serialize with `bincode` because `Instruction` does not implement `BorshSerialize`
    let mut serialized_ix = bincode::serialize(ix)?;

    // Remove extra bytes to make the serialized instruction `borsh` compatible.
    // `bincode` uses 8 bytes (LE) for length; `borsh` uses 4 bytes (LE).
    let mut remove_extra_vec_bytes = |index: usize| {
        serialized_ix.drain((index + 4)..(index + 8));
    };

    let accounts_index = std::mem::size_of_val(&ix.program_id);
    remove_extra_vec_bytes(accounts_index);
    let data_index = accounts_index + 4 + std::mem::size_of_val(&*ix.accounts);
    remove_extra_vec_bytes(data_index);

    println!(
        "Base64 encoded instruction: {}",
        STANDARD.encode(serialized_ix)
    );

    Ok(())
}
