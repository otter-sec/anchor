use super::*;

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
fn empty_or_immutable_genesis_does_not_require_a_wallet() {
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
