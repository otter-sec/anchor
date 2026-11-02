use super::*;

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
