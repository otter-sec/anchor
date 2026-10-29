use {
    super::*,
    crate::{
        config::{_SurfpoolConfig, SurfpoolConfig, TestValidator, Validator},
        GeneratedAccount, Pubkey,
    },
    std::process::Stdio,
};

#[cfg(unix)]
#[test]
fn suite_identity_directory_is_private_even_with_a_permissive_umask() {
    use std::os::unix::fs::PermissionsExt;
    let root = tempfile::tempdir().unwrap();
    let suite = owned_directory(root.path()).unwrap();
    assert_eq!(
        std::fs::metadata(suite).unwrap().permissions().mode() & 0o777,
        0o700
    );
}

fn surfpool(config: _SurfpoolConfig) -> Option<SurfpoolConfig> {
    Some(config.into())
}

fn strings(values: &[&str]) -> Vec<String> {
    values.iter().map(|value| value.to_string()).collect()
}

#[test]
fn only_opted_in_tests_receive_the_exact_anchor_config_path() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("Anchor.toml");
    let historical = || Config {
        historical: Some(toml::from_str("provider = 'alchemy'").unwrap()),
        ..Config::default()
    };
    assert!(config_path(&WithPath::new(Config::default(), path.clone()))
        .unwrap()
        .is_none());
    assert!(config_path(&WithPath::new(historical(), path.clone())).is_err());
    std::fs::write(&path, "[historical]\nprovider = 'alchemy'\n").unwrap();
    let selected = config_path(&WithPath::new(historical(), path.clone()))
        .unwrap()
        .unwrap();
    assert_eq!(selected, path.canonicalize().unwrap());
}

#[test]
fn fixture_args_keep_fixtures_and_drop_only_fork_owned_endpoint_and_clock_settings() {
    let normal = strings(&[
        "--snapshot",
        "/w/.anchor/generated_accounts/surfpool.snapshot.json",
        "--airdrop",
        "Air1",
        "--host",
        "127.0.0.1",
        "--port",
        "8899",
        "--ws-port",
        "8900",
        "--manifest-file-path",
        "./txtx.yml",
        "--runbook",
        "deployment",
        "--slot-time",
        "400",
        "--offline",
        "--block-production-mode",
        "transaction",
        "--log-level",
        "none",
        "--no-tui",
        "--disable-instruction-profiling",
        "--max-profiles",
        "1",
        "--no-studio",
        "--legacy-anchor-compatibility",
        "--anchor-test-config-path",
        "/w/tests",
    ]);
    assert_eq!(
        plan::fixture_args(&normal).unwrap(),
        strings(&[
            "start",
            "--snapshot",
            "/w/.anchor/generated_accounts/surfpool.snapshot.json",
            "--airdrop",
            "Air1",
            "--manifest-file-path",
            "./txtx.yml",
            "--runbook",
            "deployment",
            "--slot-time",
            "400",
            "--log-level",
            "none",
            "--no-tui",
            "--disable-instruction-profiling",
            "--max-profiles",
            "1",
            "--no-studio",
            "--legacy-anchor-compatibility",
            "--anchor-test-config-path",
            "/w/tests",
        ])
    );
    let no_deploy = strings(&[
        "--offline",
        "--block-production-mode",
        "transaction",
        "--no-deploy",
    ]);
    assert_eq!(
        plan::fixture_args(&no_deploy).unwrap(),
        strings(&["start", "--no-deploy"])
    );
    let current = strings(&["--rpc-url", "https://api.mainnet-beta.solana.com"]);
    assert!(plan::fixture_args(&current)
        .unwrap_err()
        .to_string()
        .contains("--rpc-url"));
}

#[test]
fn conflicting_surfpool_and_validator_settings_are_refused_not_ignored() {
    let refused = |config: Option<SurfpoolConfig>, validator: Option<Validator>| {
        let test_validator = validator.map(|validator| TestValidator {
            validator: Some(validator),
            ..Default::default()
        });
        plan::refuse_conflicts(&config, &test_validator)
            .unwrap_err()
            .to_string()
    };
    assert!(plan::refuse_conflicts(&surfpool(_SurfpoolConfig::default()), &None).is_ok());
    assert!(plan::refuse_conflicts(&None, &None).is_ok());
    for (config, key) in [
        (
            _SurfpoolConfig {
                datasource_rpc_url: Some("http://x".into()),
                ..Default::default()
            },
            "datasource_rpc_url",
        ),
        (
            _SurfpoolConfig {
                block_production_mode: Some("clock".into()),
                ..Default::default()
            },
            "block_production_mode",
        ),
        (
            _SurfpoolConfig {
                ws_port: Some(9000),
                ..Default::default()
            },
            "rpc_port/ws_port",
        ),
        (
            _SurfpoolConfig {
                rpc_port: Some(9001),
                ..Default::default()
            },
            "rpc_port/ws_port",
        ),
    ] {
        assert!(refused(surfpool(config), None).contains(key));
    }
    for arg in [
        "--offline",
        "-u",
        "--after",
        "--port=9000",
        "--surfnet-id",
        "--key-file",
        "--host",
        "--studio-port",
    ] {
        let config = _SurfpoolConfig {
            extra_args: Some(vec![arg.into(), "x".into()]),
            ..Default::default()
        };
        assert!(refused(surfpool(config), None).contains(arg), "{arg}");
    }
    let allowed = _SurfpoolConfig {
        extra_args: Some(strings(&["--log-level", "info"])),
        ..Default::default()
    };
    assert!(plan::refuse_conflicts(&surfpool(allowed), &None).is_ok());
    for (validator, key) in [
        (
            Validator {
                warp_slot: Some(5),
                ..Default::default()
            },
            "warp_slot",
        ),
        (
            Validator {
                slots_per_epoch: Some("32".into()),
                ..Default::default()
            },
            "slots_per_epoch",
        ),
        (
            Validator {
                ticks_per_slot: Some(8),
                ..Default::default()
            },
            "ticks_per_slot",
        ),
        (
            Validator {
                deactivate_feature: Some(vec![]),
                ..Default::default()
            },
            "deactivate_feature",
        ),
        (
            Validator {
                geyser_plugin_config: Some("g.json".into()),
                ..Default::default()
            },
            "geyser_plugin_config",
        ),
    ] {
        assert!(refused(None, Some(validator)).contains(key));
    }
}

#[test]
fn historical_legacy_fixture_configuration_is_supported() {
    let address = Pubkey::new_unique().to_string();
    let test = TestValidator {
        genesis: Some(vec![crate::config::GenesisEntry {
            address: address.clone(),
            program: "fixture.so".into(),
            upgradeable: Some(true),
        }]),
        validator: Some(Validator {
            account: Some(vec![crate::config::AccountEntry {
                address,
                filename: "fixture.json".into(),
            }]),
            account_dir: Some(vec![crate::config::AccountDirEntry {
                directory: "accounts".into(),
            }]),
            ..Default::default()
        }),
        ..Default::default()
    };
    plan::refuse_conflicts(&None, &Some(test)).unwrap();
}

#[test]
fn historical_validator_url_is_refused_not_ignored() {
    let address = Pubkey::new_unique().to_string();
    let validator = |validator: Validator| TestValidator {
        validator: Some(validator),
        ..Default::default()
    };
    let test_validator = validator(Validator {
        url: Some("https://api.mainnet-beta.solana.com".into()),
        ..Default::default()
    });
    let message = plan::refuse_conflicts(&None, &Some(test_validator))
        .unwrap_err()
        .to_string();
    assert!(
        message.starts_with("UNSUPPORTED_HISTORICAL_CONFIG: ")
            && message.contains("[test.validator] url")
            && message.contains("[historical] provider"),
        "{message}"
    );
    // Fixture accounts are local overrides; clone addresses hydrate from the
    // boundary when read, and empty lists declare nothing.
    let supported = validator(Validator {
        account: Some(vec![]),
        clone: Some(vec![crate::config::CloneEntry { address }]),
        fund_accounts: Some(vec![]),
        ..Default::default()
    });
    assert!(plan::refuse_conflicts(&None, &Some(supported)).is_ok());
}

#[test]
fn plan_carries_rent_checks_only_for_images_funded_as_rent_exempt() {
    let image = |owner: &str, lamports: u64, data: &[u8]| GeneratedAccount {
        pubkey: Pubkey::new_unique(),
        file_path: PathBuf::from("unused.json"),
        surfpool_snapshot_value: serde_json::json!({"lamports": lamports, "owner": owner,
            "executable": false, "data": base64::Engine::encode(&base64::engine::general_purpose::STANDARD, data)}),
    };
    let token = "TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA";
    let generated = vec![
        image(token, crate::rent_exempt_minimum(82), &[0; 82]),
        image(token, crate::rent_exempt_minimum(165), &[0; 165]),
        image("11111111111111111111111111111111", 7, &[]),
    ];
    let root = tempfile::tempdir().unwrap();
    let plan = plan::document(plan::Inputs {
        root: root.path(),
        suite: root.path(),
        args: strings(&["start", "--no-deploy"]),
        provider_config: &root.path().join("Anchor.toml"),
        wallet: "/w/id.json",
        idl_directory: &root.path().join("target/idl"),
        generated: &generated,
        registry: &root.path().join("forks"),
        surfpool_config: &None,
        shutdown_wait: 2000,
    })
    .unwrap();
    assert_eq!(plan["schema"], plan::SCHEMA);
    assert_eq!(plan["surfpool"]["host"], crate::config::SURFPOOL_HOST);
    let args: Vec<String> = serde_json::from_value(plan["surfpool"]["args"].clone()).unwrap();
    assert_eq!(
        &args[args.len() - 2..],
        [
            "--historical-config".to_string(),
            root.path().join("Anchor.toml").display().to_string()
        ]
    );
    let checks = plan["rentChecks"].as_array().unwrap();
    assert_eq!(checks.len(), 2);
    assert_eq!(checks[0]["dataLength"], 82);
    assert_eq!(checks[1]["lamports"], crate::rent_exempt_minimum(165));
}

#[test]
fn predeploy_is_refused_only_for_historical_workspaces() {
    let path = PathBuf::from("/w/Anchor.toml");
    let normal = WithPath::new(Config::default(), path.clone());
    assert!(guard_predeploy(&normal, true).is_ok());
    let config = Config {
        historical: Some(toml::from_str("provider = 'alchemy'").unwrap()),
        ..Config::default()
    };
    let historical = WithPath::new(config, path);
    assert!(guard_predeploy(&historical, false).is_ok());
    assert!(guard_predeploy(&historical, true).is_err());
}

fn record(registry: &Path, name: &str, pid: u32, surfnet_id: &str) {
    std::fs::write(
        registry.join(format!("{name}.json")),
        serde_json::json!({"pid": pid, "surfnetId": surfnet_id}).to_string(),
    )
    .unwrap();
}

#[test]
fn reaper_terminates_a_recorded_live_fork_and_fails_the_suite() {
    let root = tempfile::tempdir().unwrap();
    let id = format!("m71-reap-{}", std::process::id());
    let mut fork = std::process::Command::new("python3")
        .args(["-c", "import time; time.sleep(60)", "--surfnet-id", &id])
        .stdout(Stdio::null())
        .spawn()
        .unwrap();
    record(root.path(), "a", fork.id(), &id);
    let plan = SuitePlan {
        path: root.path().join("plan.json"),
        registry: root.path().to_path_buf(),
        shutdown_wait: Duration::from_millis(100),
    };
    let deadline = Instant::now() + Duration::from_secs(5);
    while !is_fork(fork.id() as u64, &id).unwrap() {
        assert!(Instant::now() < deadline, "fork never became visible");
        thread::sleep(Duration::from_millis(20));
    }
    let error = reap(Some(&plan)).unwrap_err().to_string();
    assert!(error.contains("outlived their tests"), "{error}");
    assert!(
        fork.wait().unwrap().code().is_none(),
        "the fork was signalled"
    );
    assert!(std::fs::read_dir(root.path()).unwrap().next().is_none());
}

#[cfg(unix)]
#[test]
fn reaper_uses_full_command_lines_when_columns_is_set() {
    const CHILD: &str = "ANCHOR_REAPER_WIDTH_TEST_CHILD";
    if std::env::var_os(CHILD).is_none() {
        let result = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "historical::tests::reaper_uses_full_command_lines_when_columns_is_set",
                "--nocapture",
            ])
            .env(CHILD, "1")
            .env("COLUMNS", "80")
            .output()
            .unwrap();
        assert!(
            result.status.success(),
            "{}\n{}",
            String::from_utf8_lossy(&result.stdout),
            String::from_utf8_lossy(&result.stderr)
        );
        assert!(String::from_utf8_lossy(&result.stdout).contains("1 passed"));
        return;
    }
    let root = tempfile::tempdir().unwrap();
    let id = format!("anchor-reap-width-{}", std::process::id());
    let mut fork = std::process::Command::new("python3")
        .args([
            "-c",
            "import time; time.sleep(60)",
            &"x".repeat(800),
            "--surfnet-id",
            &id,
        ])
        .stdout(Stdio::null())
        .spawn()
        .unwrap();
    record(root.path(), "wide", fork.id(), &id);
    let plan = SuitePlan {
        path: root.path().join("plan.json"),
        registry: root.path().to_path_buf(),
        shutdown_wait: Duration::from_millis(100),
    };
    let outcome = reap(Some(&plan));
    let still_running = fork.try_wait().unwrap().is_none();
    // The regression must also clean up its synthetic child on the red path.
    if still_running {
        fork.kill().unwrap();
    }
    fork.wait().unwrap();
    let error = outcome.unwrap_err().to_string();
    assert!(error.contains("outlived their tests"), "{error}");
    assert!(!still_running, "the recorded fork must actually be reaped");
}

#[test]
fn reaper_never_signals_a_reused_pid_and_accepts_exited_forks() {
    let root = tempfile::tempdir().unwrap();
    // This test process is alive but was not started with that surfnet id: a
    // reused PID.
    record(
        root.path(),
        "reused",
        std::process::id(),
        "m71-not-this-process",
    );
    let mut exited = std::process::Command::new("true").spawn().unwrap();
    let pid = exited.id();
    exited.wait().unwrap();
    record(root.path(), "exited", pid, "m71-exited");
    let plan = SuitePlan {
        path: root.path().join("plan.json"),
        registry: root.path().to_path_buf(),
        shutdown_wait: Duration::from_millis(100),
    };
    reap(Some(&plan)).unwrap();
    assert!(std::fs::read_dir(root.path()).unwrap().next().is_none());
    std::fs::write(root.path().join("bad.json"), "{\"pid\": 1}").unwrap();
    assert!(reap(Some(&plan)).is_err());
    assert!(
        root.path().join("bad.json").exists(),
        "an unreadable record is kept"
    );
}

#[test]
fn suite_directories_are_new_and_never_reused() {
    let root = tempfile::tempdir().unwrap();
    let first = owned_directory(root.path()).unwrap();
    let second = owned_directory(root.path()).unwrap();
    assert_ne!(first, second);
    assert!(first.is_dir() && second.is_dir());
}
