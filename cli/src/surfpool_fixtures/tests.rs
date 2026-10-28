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

#[test]
fn imports_exact_account_images_and_rejects_duplicates_or_bad_fields() {
    let root = tempfile::tempdir().unwrap();
    let cfg = WithPath::new(Config::default(), root.path().join("Anchor.toml"));
    let address = Pubkey::new_unique();
    let mut fixture = account(address, "base58");
    let file = root.path().join("fixture.json");
    fs::write(&file, fixture.to_string()).unwrap();
    let mut test = TestValidator {
        validator: Some(Validator {
            account: Some(vec![AccountEntry {
                address: address.to_string(),
                filename: "fixture.json".into(),
            }]),
            ..Default::default()
        }),
        ..Default::default()
    };
    let images = compile(&cfg, &Some(test.clone()), &[]).unwrap();
    assert_eq!(
        images[&address.to_string()],
        json!({"lamports":42,
        "owner":"11111111111111111111111111111111", "executable":false,
        "rentEpoch":7,"data":"AQID","parsedData":null})
    );
    test.validator.as_mut().unwrap().account_dir = Some(vec![AccountDirEntry {
        directory: ".".into(),
    }]);
    assert!(compile(&cfg, &Some(test.clone()), &[])
        .unwrap_err()
        .to_string()
        .contains("Duplicate"));
    test.validator.as_mut().unwrap().account_dir = None;
    fixture["account"]["owner"] = json!("invalid");
    fs::write(&file, fixture.to_string()).unwrap();
    assert!(compile(&cfg, &Some(test.clone()), &[]).is_err());
    fs::write(&file, "not json").unwrap();
    assert!(compile(&cfg, &Some(test.clone()), &[]).is_err());
    fs::remove_file(&file).unwrap();
    assert!(compile(&cfg, &Some(test), &[]).is_err());
}

#[test]
fn preserves_suite_relative_fixture_paths() {
    let root = tempfile::tempdir().unwrap();
    let suite = root.path().join("suite");
    fs::create_dir_all(suite.join("accounts")).unwrap();
    let address = Pubkey::new_unique();
    fs::write(
        suite.join("accounts/a.json"),
        account(address, "base64").to_string(),
    )
    .unwrap();
    let toml = format!(
        r#"[scripts]
test='echo test'
[[test.validator.account_dir]]
directory='accounts'
[[test.validator.account]]
address='{}'
filename='accounts/a.json'
"#,
        Pubkey::new_unique()
    );
    fs::write(suite.join("Test.toml"), toml).unwrap();
    let config = crate::config::TestToml::from_path(suite.join("Test.toml")).unwrap();
    let cfg = WithPath::new(Config::default(), root.path().join("Anchor.toml"));
    let images = compile(&cfg, &config.test, &[]).unwrap();
    assert_eq!(images.len(), 2);
    assert_eq!(images[&address.to_string()]["data"], "AQID");
}

#[test]
fn account_dash_uses_file_pubkey_and_directory_requires_one() {
    let root = tempfile::tempdir().unwrap();
    let cfg = WithPath::new(Config::default(), root.path().join("Anchor.toml"));
    let address = Pubkey::new_unique();
    let file = root.path().join("account.json");
    fs::write(&file, account(address, "base64").to_string()).unwrap();
    let test = TestValidator {
        validator: Some(Validator {
            account: Some(vec![AccountEntry {
                address: "-".into(),
                filename: "account.json".into(),
            }]),
            ..Default::default()
        }),
        ..Default::default()
    };
    assert!(compile(&cfg, &Some(test), &[])
        .unwrap()
        .contains_key(&address.to_string()));
    let test = TestValidator {
        validator: Some(Validator {
            account_dir: Some(vec![AccountDirEntry {
                directory: ".".into(),
            }]),
            ..Default::default()
        }),
        ..Default::default()
    };
    let mut value = account(address, "base64");
    value.as_object_mut().unwrap().remove("pubkey");
    fs::write(&file, value.to_string()).unwrap();
    assert!(compile(&cfg, &Some(test.clone()), &[])
        .unwrap_err()
        .to_string()
        .contains("no pubkey"));
    fs::remove_file(&file).unwrap();
    fs::write(root.path().join("README.txt"), "not an account").unwrap();
    assert!(compile(&cfg, &Some(test), &[]).unwrap().is_empty());
}

#[test]
fn upgradeable_genesis_has_exact_linkage_authority_and_rent() {
    let root = tempfile::tempdir().unwrap();
    let wallet = crate::Keypair::new();
    let wallet_path = root.path().join("wallet.json");
    crate::write_keypair_secure(&wallet, &wallet_path).unwrap();
    let mut config = Config::default();
    config.provider.wallet = wallet_path.to_str().unwrap().parse().unwrap();
    let cfg = WithPath::new(config, root.path().join("Anchor.toml"));
    let address = Pubkey::new_unique();
    let elf = b"\x7fELFprogram";
    fs::write(root.path().join("program.so"), elf).unwrap();
    let test = TestValidator {
        genesis: Some(vec![GenesisEntry {
            address: address.to_string(),
            program: "program.so".into(),
            upgradeable: Some(true),
        }]),
        ..Default::default()
    };
    let images = compile(&cfg, &Some(test), &[]).unwrap();
    let loader = bpf_loader_upgradeable::id();
    let (data_address, _) = Pubkey::find_program_address(&[address.as_ref()], &loader);
    let program = &images[&address.to_string()];
    let program_data = &images[&data_address.to_string()];
    let pbytes = STANDARD.decode(program["data"].as_str().unwrap()).unwrap();
    let bytes = STANDARD
        .decode(program_data["data"].as_str().unwrap())
        .unwrap();
    assert_eq!(
        bincode::deserialize::<UpgradeableLoaderState>(&pbytes).unwrap(),
        UpgradeableLoaderState::Program {
            programdata_address: data_address
        }
    );
    assert_eq!(
        bincode::deserialize::<UpgradeableLoaderState>(&bytes).unwrap(),
        UpgradeableLoaderState::ProgramData {
            slot: 0,
            upgrade_authority_address: Some(wallet.pubkey())
        }
    );
    assert_eq!(
        &bytes[UpgradeableLoaderState::size_of_programdata_metadata()..],
        elf
    );
    assert_eq!(program["owner"], loader.to_string());
    assert_eq!(program["executable"], true);
    assert_eq!(program_data["executable"], false);
    assert_eq!(
        program_data["lamports"],
        rent_exempt_minimum(bytes.len() as u64)
    );
}

#[test]
fn skipped_or_changed_snapshot_accounts_fail_verification() {
    use {
        crate::{RpcClient, RpcRequest},
        std::collections::HashMap,
    };
    let root = tempfile::tempdir().unwrap();
    let address = Pubkey::new_unique();
    let expected = account(address, "base64");
    let image = super::accounts::image(&expected).unwrap();
    let path = root.path().join("snapshot.json");
    fs::write(&path, json!({address.to_string():image}).to_string()).unwrap();
    let external = root.path().join("external.json");
    fs::write(
        &external,
        json!({Pubkey::new_unique().to_string():null}).to_string(),
    )
    .unwrap();
    let external_flags = vec![
        "--no-deploy".into(),
        "--snapshot".into(),
        external.display().to_string(),
    ];
    verify(&RpcClient::new_mock("succeeds"), &external_flags).unwrap();
    let flags = vec![
        "--snapshot".into(),
        path.display().to_string(),
        "--snapshot".into(),
        external.display().to_string(),
    ];
    for field in [
        None,
        Some("owner"),
        Some("lamports"),
        Some("data"),
        Some("executable"),
        Some("missing"),
        Some("short-response"),
    ] {
        let mut actual = expected["account"].clone();
        match field {
            Some("owner") => actual["owner"] = json!(Pubkey::new_unique().to_string()),
            Some("lamports") => actual["lamports"] = json!(43),
            Some("data") => actual["data"] = json!(["BAUG", "base64"]),
            Some("executable") => actual["executable"] = json!(true),
            Some("missing") => actual = Value::Null,
            Some("short-response") => {}
            None => {}
            other => panic!("unexpected field {other:?}"),
        }
        let mocks = HashMap::from([(
            RpcRequest::GetMultipleAccounts,
            json!({"context":{"slot":1},"value":if field == Some("short-response") { vec![] } else { vec![actual] }}),
        )]);
        let client = RpcClient::new_mock_with_mocks("succeeds", mocks);
        let result = verify(&client, &flags);
        if field.is_none() {
            result.unwrap();
        } else {
            assert!(result.unwrap_err().to_string().contains("FIXTURE_MISMATCH"));
        }
    }
}
