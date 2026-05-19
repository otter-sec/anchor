use {
    crate::{config::ConfigOverride, get_cluster_and_wallet, AbsolutePath},
    anyhow::{anyhow, Result},
    clap::Parser,
    solana_commitment_config::CommitmentConfig,
    solana_pubkey::Pubkey,
    solana_rpc_client::rpc_client::RpcClient,
    std::{fs::File, io::Write, path::PathBuf},
};

#[derive(Debug, Parser, AbsolutePath)]
pub struct ShowAccountCommand {
    /// Account address to show
    pub account_address: Pubkey,
    /// Display balance in lamports instead of SOL
    #[clap(long)]
    pub lamports: bool,
    /// Write the account data to this file
    #[clap(short = 'o', long)]
    pub output_file: Option<PathBuf>,
    /// Return information in specified output format
    #[clap(long, value_parser = ["json", "json-compact"])]
    pub output: Option<String>,
}

pub fn show_account(cfg_override: &ConfigOverride, cmd: ShowAccountCommand) -> Result<()> {
    // Resolve cluster the same way sibling commands (`balance`, `airdrop`,
    // `epoch`) do: workspace Anchor.toml > Solana CLI config > mainnet-beta,
    // with `--provider.cluster` as the final override.
    let (url, _wallet) = get_cluster_and_wallet(cfg_override)?;

    let rpc_client = RpcClient::new_with_commitment(url, CommitmentConfig::confirmed());

    // Fetch the account
    let account = rpc_client
        .get_account(&cmd.account_address)
        .map_err(|e| anyhow!("Unable to fetch account {}: {}", cmd.account_address, e))?;

    // Handle JSON output
    if let Some(format) = cmd.output {
        use base64::{engine::general_purpose::STANDARD, Engine};

        let json_output = serde_json::json!({
            "pubkey": cmd.account_address.to_string(),
            "account": {
                "lamports": account.lamports,
                "owner": account.owner.to_string(),
                "executable": account.executable,
                "rentEpoch": account.rent_epoch,
                "data": STANDARD.encode(&account.data),
            }
        });

        let output_str = match format.as_str() {
            "json" => serde_json::to_string_pretty(&json_output)?,
            "json-compact" => serde_json::to_string(&json_output)?,
            _ => unreachable!(),
        };

        if let Some(output_file) = cmd.output_file {
            let mut file = File::create(&output_file)?;
            file.write_all(output_str.as_bytes())?;
            println!("Wrote account to {}", output_file.display());
        } else {
            println!("{}", output_str);
        }

        return Ok(());
    }

    // Text output
    println!("Public Key: {}", cmd.account_address);

    if cmd.lamports {
        println!("Balance: {} lamports", account.lamports);
    } else {
        println!("Balance: {} SOL", account.lamports as f64 / 1_000_000_000.0);
    }

    println!("Owner: {}", account.owner);
    println!("Executable: {}", account.executable);
    println!("Rent Epoch: {}", account.rent_epoch);

    // Display account data
    let data_len = account.data.len();
    println!("Length: {} (0x{:x}) bytes", data_len, data_len);

    if !account.data.is_empty() {
        // Write to output file if specified
        if let Some(output_file) = cmd.output_file {
            let mut file = File::create(&output_file)?;
            file.write_all(&account.data)?;
            println!("Wrote account data to {}", output_file.display());
        }

        // Display hex dump
        print_hex_dump(&account.data);
    }

    Ok(())
}

fn print_hex_dump(data: &[u8]) {
    const BYTES_PER_LINE: usize = 16;

    for (i, chunk) in data.chunks(BYTES_PER_LINE).enumerate() {
        let offset = i * BYTES_PER_LINE;

        // Print offset
        print!("{:04x}:  ", offset);

        // Print hex values
        for (j, byte) in chunk.iter().enumerate() {
            if j > 0 && j % 4 == 0 {
                print!(" ");
            }
            print!("{:02x} ", byte);
        }

        // Pad if this is the last line and it's not complete
        if chunk.len() < BYTES_PER_LINE {
            for j in chunk.len()..BYTES_PER_LINE {
                if j > 0 && j % 4 == 0 {
                    print!(" ");
                }
                print!("   ");
            }
        }

        print!("  ");

        // Print ASCII representation
        for byte in chunk {
            let c = *byte as char;
            if c.is_ascii_graphic() || c == ' ' {
                print!("{}", c);
            } else {
                print!(".");
            }
        }

        println!();
    }
}
