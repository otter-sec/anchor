//! The suite launch plan: the ordinary Surfpool fixture arguments for one
//! suite, without the
//! settings a historical fork owns (its boundary, endpoint and frozen clock).
use {
    crate::{
        config::{SurfpoolConfig, TestValidator},
        GeneratedAccount,
    },
    anyhow::{bail, Result},
    base64::{engine::general_purpose::STANDARD, Engine},
    serde_json::{json, Value},
    std::path::Path,
};

pub(super) const SCHEMA: &str = "anchor-historical-plan/v1";

/// Surfpool options a historical fork sets itself or forbids: current-state
/// data sources,
/// block production, runtime features, its endpoint, storage namespace and
/// provider settings.
const FORK_OWNED: &[&str] = &[
    "--after",
    "--rpc-url",
    "-u",
    "--network",
    "-n",
    "--offline",
    "--match-datasource",
    "--block-production-mode",
    "-b",
    "--feature",
    "-f",
    "--disable-feature",
    "--features-all",
    "--skip-signature-verification",
    "--skip-blockhash-check",
    "--host",
    "-o",
    "--port",
    "-p",
    "--ws-port",
    "-w",
    "--studio-port",
    "--surfnet-id",
    "--historical-provider",
    "--historical-config",
    "--key",
    "--historical-key",
    "--key-file",
    "--historical-key-file",
    "--historical-data-dir",
    "--historical-runtime-registry",
    "--historical-runtime-registry-sha256",
];

/// Refuses settings that would silently change a historical fork or be silently
/// ignored by it.
pub(super) fn refuse_conflicts(
    surfpool_config: &Option<SurfpoolConfig>,
    test_validator: &Option<TestValidator>,
) -> Result<()> {
    if let Some(config) = surfpool_config {
        if config.datasource_rpc_url.is_some() {
            bail!(
                "[surfpool] datasource_rpc_url reads current state; historical forks read only \
                 their boundary"
            );
        }
        if config.block_production_mode.is_some() {
            bail!(
                "[surfpool] block_production_mode does not apply: a historical clock advances \
                 only explicitly"
            );
        }
        if config.ws_port.is_some() || config.rpc_port != crate::DEFAULT_RPC_PORT {
            bail!(
                "[surfpool] rpc_port/ws_port do not apply: each historical fork chooses its own \
                 ports"
            );
        }
        for arg in config.extra_args.iter().flatten() {
            let flag = arg.split('=').next().unwrap_or(arg);
            if FORK_OWNED.contains(&flag) {
                bail!(
                    "[surfpool] extra_args `{arg}` conflicts with a historical fork's own setting"
                );
            }
        }
    }
    if let Some(validator) = test_validator
        .as_ref()
        .and_then(|test| test.validator.as_ref())
    {
        if validator.url.is_some() {
            bail!(
                "UNSUPPORTED_HISTORICAL_CONFIG: [test.validator] url is not used by a historical \
                 fork, which reads its boundary through the [historical] provider; remove url for \
                 historical tests"
            );
        }
        let unsupported = [
            ("warp_slot", validator.warp_slot.is_some()),
            ("slots_per_epoch", validator.slots_per_epoch.is_some()),
            ("ticks_per_slot", validator.ticks_per_slot.is_some()),
            ("deactivate_feature", validator.deactivate_feature.is_some()),
            (
                "geyser_plugin_config",
                validator.geyser_plugin_config.is_some(),
            ),
        ];
        if let Some((key, _)) = unsupported.iter().find(|(_, set)| *set) {
            bail!(
                "[test.validator] {key} would change the historical boundary's chain; remove it \
                 for historical tests"
            );
        }
    }
    Ok(())
}

/// The ordinary Surfpool flags with the fork-owned endpoint and clock settings
/// removed. Anchor
/// emits those itself, so their presence is not user intent; user `extra_args`
/// were checked first.
/// The plan's `host` field carries the endpoint host for the helper to pass
/// once.
pub(super) fn fixture_args(normal: &[String]) -> Result<Vec<String>> {
    let mut args = vec!["start".to_string()];
    let mut index = 0;
    while index < normal.len() {
        match normal[index].as_str() {
            "--offline" => index += 1,
            "--block-production-mode" | "--host" | "--port" | "--ws-port" => index += 2,
            "--rpc-url" => bail!("a historical fork cannot use a current-state --rpc-url"),
            arg => {
                args.push(arg.to_string());
                index += 1;
            }
        }
    }
    Ok(args)
}

pub(super) struct Inputs<'a> {
    pub(super) root: &'a Path,
    pub(super) suite: &'a Path,
    pub(super) args: Vec<String>,
    pub(super) provider_config: &'a Path,
    pub(super) wallet: &'a str,
    pub(super) idl_directory: &'a Path,
    pub(super) generated: &'a [GeneratedAccount],
    pub(super) registry: &'a Path,
    pub(super) surfpool_config: &'a Option<SurfpoolConfig>,
    pub(super) shutdown_wait: u64,
}

/// Generated images that Anchor funded as rent-exempt with its default rent. A
/// test helper
/// compares them with the fork's own rent; their bytes are never re-serialized
/// there.
fn rent_checks(generated: &[GeneratedAccount]) -> Result<Vec<Value>> {
    let mut checks = Vec::new();
    for account in generated {
        let image = &account.surfpool_snapshot_value;
        let (Some(lamports), Some(data)) = (image["lamports"].as_u64(), image["data"].as_str())
        else {
            bail!("generated account {} has no snapshot image", account.pubkey);
        };
        let length = STANDARD.decode(data)?.len() as u64;
        if lamports == crate::rent_exempt_minimum(length) {
            checks.push(json!({"pubkey": account.pubkey.to_string(), "dataLength": length, "lamports": lamports}));
        }
    }
    Ok(checks)
}

pub(super) fn document(inputs: Inputs) -> Result<Value> {
    let mut args = inputs.args;
    args.push("--historical-config".into());
    args.push(inputs.provider_config.display().to_string());
    let host = inputs
        .surfpool_config
        .as_ref()
        .map(|config| config.host.clone())
        .unwrap_or_else(|| crate::config::SURFPOOL_HOST.to_string());
    let startup_wait = inputs
        .surfpool_config
        .as_ref()
        .map(|config| config.startup_wait)
        .unwrap_or(crate::config::STARTUP_WAIT);
    Ok(json!({
        "schema": SCHEMA,
        "workspace": inputs.root,
        "suite": inputs.suite,
        "surfpool": {"program": "surfpool", "cwd": inputs.root, "host": host, "args": args},
        "wallet": inputs.wallet,
        "idlDirectory": inputs.idl_directory,
        "rentChecks": rent_checks(inputs.generated)?,
        "registry": inputs.registry,
        "startupWaitMs": startup_wait,
        "shutdownWaitMs": inputs.shutdown_wait,
    }))
}
