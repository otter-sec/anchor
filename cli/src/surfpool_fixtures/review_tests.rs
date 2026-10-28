use {
    super::*,
    crate::{
        account_dir_pubkeys,
        config::{AccountDirEntry, AccountEntry, SurfpoolConfig, Validator},
        surfpool_flags, validator_supplied_account_pubkeys,
    },
    base64::{engine::general_purpose::STANDARD, Engine},
    serde_json::json,
    std::fs,
};

#[test]
fn review_reserved_deployment_args_are_rejected_before_launch() {
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
fn review_directory_pubkeys_and_loader_reject_the_same_missing_pubkey() {
    let root = tempfile::tempdir().unwrap();
    let cfg = WithPath::new(Config::default(), root.path().join("Anchor.toml"));
    fs::write(root.path().join("bad.json"), "{}").unwrap();
    let validator = Validator {
        account_dir: Some(vec![AccountDirEntry {
            directory: ".".into(),
        }]),
        ..Default::default()
    };
    let addresses = account_dir_pubkeys(&cfg, &validator).unwrap_err();
    let supplied = validator_supplied_account_pubkeys(&cfg, &validator, &[]).unwrap_err();
    let loaded = compile(
        &cfg,
        &Some(TestValidator {
            validator: Some(validator),
            ..Default::default()
        }),
        &[],
    )
    .unwrap_err();
    for error in [addresses, supplied, loaded] {
        assert!(error.to_string().contains("no pubkey"), "{error:#}");
        assert!(error.to_string().contains("bad.json"), "{error:#}");
    }
}

#[test]
fn review_malformed_fixtures_have_consistent_discovery_and_loading_errors() {
    let root = tempfile::tempdir().unwrap();
    let cfg = WithPath::new(Config::default(), root.path().join("Anchor.toml"));
    let validator = Validator {
        account: Some(vec![AccountEntry {
            address: "-".into(),
            filename: "bad.json".into(),
        }]),
        account_dir: Some(vec![AccountDirEntry {
            directory: ".".into(),
        }]),
        ..Default::default()
    };
    for contents in ["not json", "{}", r#"{"pubkey":"invalid"}"#] {
        fs::write(root.path().join("bad.json"), contents).unwrap();
        let directory = account_dir_pubkeys(&cfg, &validator)
            .unwrap_err()
            .to_string();
        let supplied = validator_supplied_account_pubkeys(&cfg, &validator, &[])
            .unwrap_err()
            .to_string();
        let loaded = compile(
            &cfg,
            &Some(TestValidator {
                validator: Some(validator.clone()),
                ..Default::default()
            }),
            &[],
        )
        .unwrap_err()
        .to_string();
        let normalize = |message: String| message.replace("/./", "/").replace("\\.\\", "\\");
        assert_eq!(normalize(directory.clone()), normalize(supplied));
        assert_eq!(normalize(directory), normalize(loaded));
    }
}

#[test]
fn review_explicit_address_overrides_file_pubkey_for_both_readers() {
    let root = tempfile::tempdir().unwrap();
    let cfg = WithPath::new(Config::default(), root.path().join("Anchor.toml"));
    let address = Pubkey::new_unique();
    fs::write(
        root.path().join("a.json"),
        json!({"pubkey":"invalid", "account":{
        "owner":"11111111111111111111111111111111", "lamports":1,
        "executable":false,"rentEpoch":0,"data":["","base64"]}})
        .to_string(),
    )
    .unwrap();
    let validator = Validator {
        account: Some(vec![AccountEntry {
            address: address.to_string(),
            filename: "a.json".into(),
        }]),
        ..Default::default()
    };
    assert!(validator_supplied_account_pubkeys(&cfg, &validator, &[])
        .unwrap()
        .contains(&address));
    assert!(compile(
        &cfg,
        &Some(TestValidator {
            validator: Some(validator),
            ..Default::default()
        }),
        &[]
    )
    .unwrap()
    .contains_key(&address.to_string()));
}

fn image(data: Value) -> Value {
    json!({"owner":"11111111111111111111111111111111", "lamports":1,
        "executable":false,"rentEpoch":0,"data":data})
}

#[test]
fn review_compressed_fixtures_decode_and_refuse_corruption_or_oversize() {
    let compressed = zstd::stream::encode_all(&[1u8, 2, 3][..], 0).unwrap();
    let fixture = image(json!([STANDARD.encode(&compressed), "base64+zstd"]));
    assert_eq!(super::accounts::image(&fixture).unwrap()["data"], "AQID");
    let corrupt = image(json!([STANDARD.encode(b"invalid zstd"), "base64+zstd"]));
    assert!(super::accounts::image(&corrupt).is_err());
    let truncated = image(json!([
        STANDARD.encode(&compressed[..compressed.len() - 1]),
        "base64+zstd"
    ]));
    assert!(super::accounts::image(&truncated).is_err());
    let bytes = vec![0u8; 10 * 1024 * 1024 + 1];
    let compressed = zstd::stream::encode_all(bytes.as_slice(), 0).unwrap();
    let oversized = image(json!([STANDARD.encode(compressed), "base64+zstd"]));
    assert!(super::accounts::image(&oversized)
        .unwrap_err()
        .to_string()
        .contains("10 MiB"));
}

#[test]
fn review_empty_or_immutable_genesis_does_not_require_a_wallet() {
    use crate::config::GenesisEntry;
    let root = tempfile::tempdir().unwrap();
    let mut config = Config::default();
    config.provider.wallet = root
        .path()
        .join("absent-wallet.json")
        .to_str()
        .unwrap()
        .parse()
        .unwrap();
    let cfg = WithPath::new(config, root.path().join("Anchor.toml"));
    assert!(compile(&cfg, &Some(TestValidator::default()), &[])
        .unwrap()
        .is_empty());
    fs::write(root.path().join("a.so"), b"\x7fELFprogram").unwrap();
    let test = TestValidator {
        genesis: Some(vec![GenesisEntry {
            address: Pubkey::new_unique().to_string(),
            program: "a.so".into(),
            upgradeable: Some(false),
        }]),
        ..Default::default()
    };
    assert_eq!(compile(&cfg, &Some(test), &[]).unwrap().len(), 2);
}
