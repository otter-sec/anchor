use {
    super::{insert, reader},
    crate::{
        config::{Config, Validator, WithPath},
        resolve_workspace_path, Pubkey,
    },
    anyhow::{anyhow, bail, Context, Result},
    base64::{engine::general_purpose::STANDARD, Engine},
    serde_json::{json, Map, Value},
    std::io::Read,
};

const MAX_ACCOUNT_DATA_LEN: usize = 10 * 1024 * 1024;

pub(super) fn image(value: &Value) -> Result<Value> {
    let account = value.get("account").unwrap_or(value);
    let owner = account["owner"]
        .as_str()
        .ok_or_else(|| anyhow!("missing owner"))?;
    let _: Pubkey = owner.parse().context("invalid account owner")?;
    let lamports = account["lamports"]
        .as_u64()
        .ok_or_else(|| anyhow!("missing/invalid lamports"))?;
    let executable = account["executable"]
        .as_bool()
        .ok_or_else(|| anyhow!("missing executable"))?;
    let rent_epoch = account["rentEpoch"]
        .as_u64()
        .ok_or_else(|| anyhow!("missing/invalid rentEpoch"))?;
    let data = &account["data"];
    let bytes = match data {
        Value::String(text) => STANDARD.decode(text).context("invalid base64 data")?,
        Value::Array(parts) if parts.len() == 2 => {
            let text = parts[0]
                .as_str()
                .ok_or_else(|| anyhow!("invalid account data"))?;
            match parts[1].as_str() {
                Some("base64") => STANDARD.decode(text).context("invalid base64 data")?,
                Some("base58") => bs58::decode(text)
                    .into_vec()
                    .context("invalid base58 data")?,
                Some("base64+zstd") => {
                    let compressed = STANDARD.decode(text).context("invalid base64 data")?;
                    let mut decoder = zstd::stream::read::Decoder::new(compressed.as_slice())
                        .context("invalid zstd data")?;
                    decoder.window_log_max(24).context("invalid zstd window")?;
                    let mut bytes = Vec::new();
                    decoder
                        .take((MAX_ACCOUNT_DATA_LEN + 1) as u64)
                        .read_to_end(&mut bytes)
                        .context("invalid zstd data")?;
                    bytes
                }
                _ => bail!("account fixture data must use base64, base58 or base64+zstd encoding"),
            }
        }
        _ => bail!("account fixture needs binary data, not jsonParsed data"),
    };
    if bytes.len() > MAX_ACCOUNT_DATA_LEN {
        bail!("account fixture data exceeds Solana's 10 MiB account limit");
    }
    Ok(
        json!({"owner":owner, "lamports":lamports, "executable":executable,
        "rentEpoch":rent_epoch, "data":STANDARD.encode(bytes), "parsedData":null}),
    )
}

pub(super) fn load(
    cfg: &WithPath<Config>,
    validator: &Validator,
    images: &mut Map<String, Value>,
) -> Result<()> {
    for account in validator.account.iter().flatten() {
        let path = resolve_workspace_path(cfg, &account.filename)?;
        let (address, value) = reader::account(&path, Some(&account.address))?;
        let loaded =
            image(&value).with_context(|| format!("Account fixture {}", path.display()))?;
        // As with --account ADDRESS FILE, the explicit address is authoritative.
        insert(images, address, loaded)?;
    }
    for entry in validator.account_dir.iter().flatten() {
        let path = resolve_workspace_path(cfg, &entry.directory)?;
        for file in reader::directory(&path)? {
            let (address, value) = reader::account(&file, None)?;
            let loaded =
                image(&value).with_context(|| format!("Account fixture {}", file.display()))?;
            insert(images, address, loaded)?;
        }
    }
    Ok(())
}
