mod accounts;
mod parsing;
mod programs;
mod verification;

use {
    super::*,
    crate::{
        bpf_loader_upgradeable,
        config::{AccountDirEntry, AccountEntry, GenesisEntry, Validator},
        rent_exempt_minimum, Signer, UpgradeableLoaderState,
    },
    base64::{engine::general_purpose::STANDARD, Engine},
    serde_json::json,
    std::fs,
};

fn account(address: Pubkey, encoding: &str) -> Value {
    let data = match encoding {
        "base58" => bs58::encode([1, 2, 3]).into_string(),
        _ => STANDARD.encode([1, 2, 3]),
    };
    json!({"pubkey":address.to_string(), "account": {"lamports":42,
        "owner":"11111111111111111111111111111111", "executable":false,
        "rentEpoch":7, "data":[data,encoding]}})
}
