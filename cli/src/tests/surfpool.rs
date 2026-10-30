use super::*;

#[test]
fn surfpool_flags_emits_extra_args() {
    let workspace = tempdir().unwrap();
    let cfg = WithPath::new(Config::default(), workspace.path().join("Anchor.toml"));
    let expected = vec![
        "--match-datasource".to_string(),
        "--disable-feature".to_string(),
        "ptokFjwyJtrwCa9Kgo9xoDS59V4QccBGEaRFnRPnSdP".to_string(),
    ];
    let surfpool_config = Some(SurfpoolConfig {
        extra_args: Some(expected.clone()),
        ..Default::default()
    });

    let flags = surfpool_flags(
        &cfg,
        &cfg.test_validator,
        &surfpool_config,
        false,
        true,
        None,
        &[],
    )
    .unwrap();

    assert!(flags
        .windows(expected.len())
        .any(|args| args == expected.as_slice()));
}

#[test]
fn surfpool_flags_do_not_force_runtime_features() {
    let dir = tempdir().unwrap();
    let cfg = WithPath::new(Config::default(), dir.path().join("Anchor.toml"));
    let flags = surfpool_flags(&cfg, &cfg.test_validator, &None, false, false, None, &[]).unwrap();

    assert!(!flags.iter().any(|flag| flag == "--feature"));
}

#[test]
fn surfpool_legacy_fixtures_are_in_the_initial_snapshot() {
    use crate::config::{AccountDirEntry, AccountEntry, GenesisEntry, Validator};
    for kind in ["account", "account_dir", "genesis"] {
        let root = tempdir().unwrap();
        let address = Pubkey::new_unique();
        let image = json!({"pubkey": address.to_string(), "account": {
            "lamports": 1234, "owner": "11111111111111111111111111111111",
            "executable": false, "rentEpoch": 0, "data": ["AQID", "base64"]
        }});
        let accounts = root.path().join("accounts");
        fs::create_dir(&accounts).unwrap();
        fs::write(accounts.join("fixture.json"), image.to_string()).unwrap();
        fs::write(root.path().join("fixture.so"), b"\x7fELFfixture").unwrap();
        let test = match kind {
            "account" => TestValidator {
                validator: Some(Validator {
                    account: Some(vec![AccountEntry {
                        address: address.to_string(),
                        filename: "accounts/fixture.json".into(),
                    }]),
                    ..Default::default()
                }),
                ..Default::default()
            },
            "account_dir" => TestValidator {
                validator: Some(Validator {
                    account_dir: Some(vec![AccountDirEntry {
                        directory: "accounts".into(),
                    }]),
                    ..Default::default()
                }),
                ..Default::default()
            },
            _ => TestValidator {
                genesis: Some(vec![GenesisEntry {
                    address: address.to_string(),
                    program: "fixture.so".into(),
                    upgradeable: Some(false),
                }]),
                ..Default::default()
            },
        };
        let cfg = WithPath::new(
            Config {
                test_validator: Some(test),
                ..Config::default()
            },
            root.path().join("Anchor.toml"),
        );
        let flags =
            surfpool_flags(&cfg, &cfg.test_validator, &None, false, true, None, &[]).unwrap();
        let index = flags
            .iter()
            .position(|arg| arg == "--snapshot")
            .unwrap_or_else(|| panic!("{kind} was silently omitted"));
        let snapshot: JsonValue =
            serde_json::from_reader(File::open(&flags[index + 1]).unwrap()).unwrap();
        let account = &snapshot[address.to_string()];
        assert!(account.is_object(), "{kind}: {snapshot}");
        if kind == "genesis" {
            assert_eq!(account["executable"], true);
            let loader = bpf_loader_upgradeable::id();
            assert_eq!(account["owner"], loader.to_string());
            let (data_address, _) = Pubkey::find_program_address(&[address.as_ref()], &loader);
            let bytes = STANDARD
                .decode(snapshot[data_address.to_string()]["data"].as_str().unwrap())
                .unwrap();
            assert_eq!(
                &bytes[UpgradeableLoaderState::size_of_programdata_metadata()..],
                b"\x7fELFfixture"
            );
        } else {
            assert_eq!(account["lamports"], 1234);
            assert_eq!(account["data"], "AQID");
        }
        // Ordinary auto-deploy is already responsible for these fixtures.
        let automatic =
            surfpool_flags(&cfg, &cfg.test_validator, &None, false, false, None, &[]).unwrap();
        assert!(!automatic.iter().any(|flag| flag == "--snapshot"));
        assert!(automatic
            .iter()
            .any(|flag| flag == "--legacy-anchor-compatibility"));
    }
}

#[test]
fn surfpool_flags_include_snapshot_for_generated_accounts() {
    let workspace = tempdir().unwrap();
    let cfg = WithPath::new(Config::default(), workspace.path().join("Anchor.toml"));
    let test_validator = Some(TestValidator {
        validator: Some(crate::config::Validator {
            bind_address: "127.0.0.1".to_string(),
            ledger: ".anchor/test-ledger".to_string(),
            rpc_port: 18999,
            fund_accounts: Some(vec![crate::config::FundedAccount {
                address: "new".to_string(),
                lamports: Some(2_000_000_000),
            }]),
            ..Default::default()
        }),
        ..Default::default()
    });

    let generated_accounts = generated_validator_accounts(&cfg, &test_validator).unwrap();
    let flags = surfpool_flags(
        &cfg,
        &test_validator,
        &None,
        false,
        false,
        None,
        &generated_accounts,
    )
    .unwrap();

    let snapshot_index = flags.iter().position(|flag| flag == "--snapshot").unwrap();
    let snapshot_path = PathBuf::from(&flags[snapshot_index + 1]);
    let snapshot: JsonValue = serde_json::from_reader(File::open(&snapshot_path).unwrap()).unwrap();

    assert!(snapshot_path.exists());
    assert!(snapshot
        .get(generated_accounts[0].pubkey.to_string())
        .is_some());
}

#[test]
fn reserved_deployment_args_are_rejected_before_launch() {
    let root = tempfile::tempdir().unwrap();
    let cfg = WithPath::new(Config::default(), root.path().join("Anchor.toml"));
    for flag in [
        "--no-deploy",
        "--legacy-anchor-compatibility",
        "--anchor-test-config-path",
    ] {
        for arg in [flag.to_string(), format!("{flag}=value")] {
            for skip_deploy in [false, true] {
                let config = Some(SurfpoolConfig {
                    extra_args: Some(vec![arg.clone()]),
                    ..Default::default()
                });
                let error = surfpool_flags(&cfg, &None, &config, false, skip_deploy, None, &[])
                    .unwrap_err();
                assert!(error.to_string().contains(flag), "{error:#}");
                assert!(error.to_string().contains("--skip-deploy"), "{error:#}");
            }
        }
    }
}

#[test]
fn duplicate_runtime_args_are_rejected_before_launch() {
    let root = tempdir().unwrap();
    let cfg = WithPath::new(Config::default(), root.path().join("Anchor.toml"));
    for arg in [
        "--no-studio",
        "--port=8999",
        "-p8999",
        "--log-level=debug",
        "--offline",
        "--no-tui",
        "--max-profiles=2",
        "--host=localhost",
        "-o127.0.0.1",
        "-ldebug",
        "-c2",
        "--block-production-mode=clock",
        "-bclock",
    ] {
        let config = Some(SurfpoolConfig {
            extra_args: Some(vec![arg.into()]),
            ..Default::default()
        });
        let error = surfpool_flags(&cfg, &None, &config, false, true, None, &[])
            .expect_err("Anchor already supplies this option");
        assert!(error.to_string().contains("extra_args"), "{error:#}");
        assert!(error.to_string().contains("already"), "{error:#}");
    }
}

#[test]
fn repeatable_and_unmanaged_runtime_args_remain_available() {
    let root = tempdir().unwrap();
    let cfg = WithPath::new(Config::default(), root.path().join("Anchor.toml"));
    let extra_args = vec![
        "--airdrop-amount".into(),
        "1".into(),
        "--log-bytes-limit".into(),
        "1".into(),
        "--airdrop".into(),
        Pubkey::new_unique().to_string(),
        "--airdrop".into(),
        Pubkey::new_unique().to_string(),
        "--runbook".into(),
        "custom".into(),
        "--no-studio".into(),
        "--slot-time".into(),
        "400".into(),
    ];
    let config = Some(SurfpoolConfig {
        extra_args: Some(extra_args.clone()),
        ..Default::default()
    });
    let flags = surfpool_flags(&cfg, &None, &config, true, true, None, &[]).unwrap();
    assert!(flags
        .windows(extra_args.len())
        .any(|args| args == extra_args));
    assert_eq!(flags.iter().filter(|arg| *arg == "--no-studio").count(), 1);
    let config = Some(SurfpoolConfig {
        extra_args: Some(vec!["--airdrop-amount".into(), "1".into()]),
        ..Default::default()
    });
    surfpool_flags(&cfg, &None, &config, false, true, None, &[]).unwrap();
}

#[test]
fn configured_runtime_aliases_are_rejected() {
    let root = tempdir().unwrap();
    let cfg = WithPath::new(Config::default(), root.path().join("Anchor.toml"));
    for (flag, value) in [
        ("-p", "8999"),
        ("-l", "debug"),
        ("-b", "clock"),
        ("-t", "400"),
    ] {
        let config = Some(SurfpoolConfig {
            slot_time: Some(400),
            extra_args: Some(vec![flag.into(), value.into()]),
            ..Default::default()
        });
        let error = surfpool_flags(&cfg, &None, &config, false, true, None, &[]).unwrap_err();
        assert!(error.to_string().contains("already supplied"), "{error:#}");
    }
}
