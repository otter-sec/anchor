use {
    crate::Pubkey,
    anyhow::{anyhow, Context, Result},
    serde_json::Value,
    std::{
        fs,
        path::{Path, PathBuf},
    },
};

pub(crate) fn account(path: &Path, address: Option<&str>) -> Result<(Pubkey, Value)> {
    let file =
        fs::File::open(path).with_context(|| format!("Open account fixture {}", path.display()))?;
    let value: Value = serde_json::from_reader(file)
        .with_context(|| format!("Parse account fixture {}", path.display()))?;
    let address = match address {
        Some(address) if address != "-" => address,
        _ => value["pubkey"]
            .as_str()
            .ok_or_else(|| anyhow!("Account fixture {} has no pubkey", path.display()))?,
    };
    let address = address
        .parse()
        .with_context(|| format!("Invalid account pubkey in {}", path.display()))?;
    Ok((address, value))
}

pub(crate) fn directory(path: &Path) -> Result<Vec<PathBuf>> {
    let mut files = fs::read_dir(path)
        .with_context(|| format!("Read account directory {}", path.display()))?
        .map(|entry| entry.map(|entry| entry.path()))
        .collect::<std::io::Result<Vec<_>>>()?;
    files.retain(|file| file.extension().and_then(|ext| ext.to_str()) == Some("json"));
    files.sort();
    Ok(files)
}
