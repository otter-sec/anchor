use {
    super::*,
    crate::{account_dir_pubkeys, validator_supplied_account_pubkeys},
};

fn image(data: Value) -> Value {
    json!({"owner":"11111111111111111111111111111111", "lamports":1,
        "executable":false,"rentEpoch":0,"data":data})
}

#[test]
fn directory_pubkeys_and_loader_reject_the_same_missing_pubkey() {
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
fn malformed_fixtures_have_consistent_discovery_and_loading_errors() {
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
fn explicit_address_overrides_file_pubkey_for_both_readers() {
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

#[test]
fn compressed_fixtures_decode_and_refuse_corruption_or_oversize() {
    let compressed = zstd::stream::encode_all(&[1u8, 2, 3][..], 0).unwrap();
    let fixture = image(json!([STANDARD.encode(&compressed), "base64+zstd"]));
    assert_eq!(
        super::super::accounts::image(&fixture).unwrap()["data"],
        "AQID"
    );
    let corrupt = image(json!([STANDARD.encode(b"invalid zstd"), "base64+zstd"]));
    assert!(super::super::accounts::image(&corrupt).is_err());
    let truncated = image(json!([
        STANDARD.encode(&compressed[..compressed.len() - 1]),
        "base64+zstd"
    ]));
    assert!(super::super::accounts::image(&truncated).is_err());
    let bytes = vec![0u8; 10 * 1024 * 1024 + 1];
    let compressed = zstd::stream::encode_all(bytes.as_slice(), 0).unwrap();
    let oversized = image(json!([STANDARD.encode(compressed), "base64+zstd"]));
    assert!(super::super::accounts::image(&oversized)
        .unwrap_err()
        .to_string()
        .contains("10 MiB"));
}
