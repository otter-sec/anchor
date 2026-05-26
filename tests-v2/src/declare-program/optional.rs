use {
    anchor_lang_v2::{
        solana_program::instruction::AccountMeta, Id, InstructionData, ToAccountMetas,
    },
    declare_program_optional::{instruction, optional_callee},
    litesvm::LiteSVM,
    solana_keypair::Keypair,
    solana_pubkey::Pubkey,
    solana_signer::Signer,
    tests_v2::{build_program, keypair_for, send_instruction},
};

fn caller_id() -> Pubkey {
    "Dec1areProgram11111111111111111111111111111"
        .parse()
        .unwrap()
}

fn optional_callee_id() -> Pubkey {
    "D9t6cEFPTDWmTZfcikokLbnuuyeJT6oXnpEbyXB45LU2"
        .parse()
        .unwrap()
}

fn setup() -> (LiteSVM, Keypair) {
    let test_dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let deploy_dir = test_dir.join("target/deploy");
    let deploy_str = deploy_dir.to_str().unwrap();

    build_program(
        test_dir
            .join("programs/declare-program/optional/programs/optional-callee")
            .to_str()
            .unwrap(),
        deploy_str,
    );
    build_program(
        test_dir
            .join("programs/declare-program/optional")
            .to_str()
            .unwrap(),
        deploy_str,
    );

    let mut svm = LiteSVM::new();
    svm.add_program_from_file(optional_callee_id(), &deploy_dir.join("optional_callee.so"))
        .expect("failed to load optional_callee program");
    svm.add_program_from_file(caller_id(), &deploy_dir.join("declare_program_optional.so"))
        .expect("failed to load declare_program_optional program");

    let payer = keypair_for("declare-program-optional-payer");
    svm.airdrop(&payer.pubkey(), 10_000_000_000).unwrap();

    (svm, payer)
}

fn optional_store(authority: Pubkey) -> Pubkey {
    Pubkey::find_program_address(
        &[b"declared-optional", authority.as_ref()],
        &optional_callee_id(),
    )
    .0
}

fn initialize_optional_store(svm: &mut LiteSVM, payer: &Keypair, authority: &Keypair) -> Pubkey {
    let data = optional_store(authority.pubkey());
    let ix_data = optional_callee::instruction::Initialize {}.data();
    let metas = optional_callee::accounts::Initialize {
        payer: payer.pubkey(),
        data,
        authority: authority.pubkey(),
        system_program: solana_sdk_ids::system_program::ID,
    }
    .to_account_metas(None);

    send_instruction(
        svm,
        optional_callee_id(),
        ix_data,
        metas,
        payer,
        &[authority],
    )
    .expect("optional_callee::initialize should succeed");

    data
}

#[derive(Debug, PartialEq, Eq)]
struct OptionalState {
    value: u64,
    calls: u16,
    saw_marker: u8,
    marker: Pubkey,
}

fn optional_state(svm: &LiteSVM, data: Pubkey) -> OptionalState {
    let account = svm.get_account(&data).expect("optional store should exist");
    OptionalState {
        value: u64::from_le_bytes(account.data[8..16].try_into().unwrap()),
        calls: u16::from_le_bytes(account.data[16..18].try_into().unwrap()),
        saw_marker: account.data[18],
        marker: Pubkey::new_from_array(account.data[52..84].try_into().unwrap()),
    }
}

#[test]
fn declared_optional_exports_declared_program_id() {
    assert_eq!(optional_callee::ID, optional_callee_id());
    assert_eq!(
        optional_callee::program::OptionalCallee::id(),
        optional_callee_id()
    );
}

#[test]
fn declared_optional_client_none_uses_declared_program_id_not_caller_crate_id() {
    assert_ne!(caller_id(), optional_callee_id());

    let data = Pubkey::new_unique();
    let authority = Pubkey::new_unique();
    let metas = optional_callee::accounts::Record {
        data,
        authority,
        maybe_marker: None,
    }
    .to_account_metas(None);

    assert_eq!(metas.len(), 3);
    assert_eq!(metas[0], AccountMeta::new(data, false));
    assert_eq!(metas[1], AccountMeta::new_readonly(authority, true));
    assert_eq!(
        metas[2],
        AccountMeta::new_readonly(optional_callee_id(), false)
    );
}

#[test]
fn declared_optional_cpi_none_executes_with_program_id_sentinel() {
    let (mut svm, payer) = setup();
    let authority = keypair_for("declare-program-optional-authority-none");
    let data = initialize_optional_store(&mut svm, &payer, &authority);

    let ix_data = instruction::ProxyRecord { value: 9 }.data();
    let metas = declare_program_optional::accounts::ProxyRecord {
        data,
        authority: authority.pubkey(),
        maybe_marker: None,
        optional_program: optional_callee_id(),
    }
    .to_account_metas(None);

    send_instruction(&mut svm, caller_id(), ix_data, metas, &payer, &[&authority])
        .expect("proxy_record without optional marker should CPI successfully");

    assert_eq!(
        optional_state(&svm, data),
        OptionalState {
            value: 10,
            calls: 1,
            saw_marker: 0,
            marker: Pubkey::default(),
        }
    );
}

#[test]
fn declared_optional_cpi_some_executes_and_passes_marker() {
    let (mut svm, payer) = setup();
    let authority = keypair_for("declare-program-optional-authority-some");
    let data = initialize_optional_store(&mut svm, &payer, &authority);
    let marker = payer.pubkey();

    let ix_data = instruction::ProxyRecord { value: 11 }.data();
    let metas = declare_program_optional::accounts::ProxyRecord {
        data,
        authority: authority.pubkey(),
        maybe_marker: Some(marker),
        optional_program: optional_callee_id(),
    }
    .to_account_metas(None);

    send_instruction(&mut svm, caller_id(), ix_data, metas, &payer, &[&authority])
        .expect("proxy_record with optional marker should CPI successfully");

    assert_eq!(
        optional_state(&svm, data),
        OptionalState {
            value: 12,
            calls: 1,
            saw_marker: 1,
            marker,
        }
    );
}

#[test]
fn declared_optional_cpi_some_program_id_is_treated_as_none_by_callee() {
    let (mut svm, payer) = setup();
    let authority = keypair_for("declare-program-optional-authority-sentinel-some");
    let data = initialize_optional_store(&mut svm, &payer, &authority);

    let ix_data = instruction::ProxyRecord { value: 13 }.data();
    let metas = declare_program_optional::accounts::ProxyRecord {
        data,
        authority: authority.pubkey(),
        maybe_marker: Some(optional_callee_id()),
        optional_program: optional_callee_id(),
    }
    .to_account_metas(None);

    send_instruction(&mut svm, caller_id(), ix_data, metas, &payer, &[&authority])
        .expect("program-id sentinel in optional CPI slot should execute as None");

    assert_eq!(
        optional_state(&svm, data),
        OptionalState {
            value: 14,
            calls: 1,
            saw_marker: 0,
            marker: Pubkey::default(),
        }
    );
}

#[test]
fn declared_optional_cpi_some_passes_exact_marker_account() {
    let (mut svm, payer) = setup();
    let authority = keypair_for("declare-program-optional-authority-exact-marker");
    let data = initialize_optional_store(&mut svm, &payer, &authority);
    let marker = Pubkey::new_unique();

    let ix_data = instruction::ProxyRecord { value: 15 }.data();
    let metas = declare_program_optional::accounts::ProxyRecord {
        data,
        authority: authority.pubkey(),
        maybe_marker: Some(marker),
        optional_program: optional_callee_id(),
    }
    .to_account_metas(None);

    send_instruction(&mut svm, caller_id(), ix_data, metas, &payer, &[&authority])
        .expect("optional CPI marker should pass through exactly");

    assert_eq!(
        optional_state(&svm, data),
        OptionalState {
            value: 16,
            calls: 1,
            saw_marker: 1,
            marker,
        }
    );
}
