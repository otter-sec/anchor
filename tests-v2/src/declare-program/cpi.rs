use {
    anchor_lang_v2::{
        solana_program::instruction::AccountMeta, Discriminator, Id, InstructionData,
        ToAccountMetas,
    },
    declare_program_cpi::{
        alt_cpi, cpi_account_type_is_generated, external, external_cpi, hash_cpi, instruction,
    },
    litesvm::LiteSVM,
    sha2::{Digest, Sha256},
    solana_keypair::Keypair,
    solana_pubkey::Pubkey,
    solana_signer::Signer,
    tests_v2::{build_program, keypair_for, send_instruction},
};

fn program_id() -> Pubkey {
    "Externa111111111111111111111111111111111111"
        .parse()
        .unwrap()
}

fn caller_id() -> Pubkey {
    "Dec1areProgram11111111111111111111111111111"
        .parse()
        .unwrap()
}

fn external_cpi_id() -> Pubkey {
    "BF748KR4UhPq7xbhFQYd7yFKmh5UYdqed9GbD6oZvEyu"
        .parse()
        .unwrap()
}

fn alt_cpi_id() -> Pubkey {
    "Con9ukTn9BRPXWcjS2UBbuN3NnCwy1hcaDNZ9Hb8QMNp"
        .parse()
        .unwrap()
}

fn hash_cpi_id() -> Pubkey {
    "FfuEBk58icFrsQX6rPQEKS2bQzvCjRrgErasDce6KsD7"
        .parse()
        .unwrap()
}

fn disc_hash(name: &str) -> [u8; 8] {
    let hash = Sha256::digest(format!("global:{name}").as_bytes());
    hash[..8].try_into().unwrap()
}

fn setup() -> (LiteSVM, Keypair) {
    let test_dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let deploy_dir = test_dir.join("target/deploy");
    let deploy_str = deploy_dir.to_str().unwrap();

    build_program(
        test_dir
            .join("programs/declare-program/cpi/programs/external-cpi")
            .to_str()
            .unwrap(),
        deploy_str,
    );
    build_program(
        test_dir
            .join("programs/declare-program/cpi/programs/alt-cpi")
            .to_str()
            .unwrap(),
        deploy_str,
    );
    build_program(
        test_dir
            .join("programs/declare-program/cpi/programs/hash-cpi")
            .to_str()
            .unwrap(),
        deploy_str,
    );
    build_program(
        test_dir
            .join("programs/declare-program/cpi")
            .to_str()
            .unwrap(),
        deploy_str,
    );

    let mut svm = LiteSVM::new();
    svm.add_program_from_file(external_cpi_id(), &deploy_dir.join("external_cpi.so"))
        .expect("failed to load external_cpi program");
    svm.add_program_from_file(alt_cpi_id(), &deploy_dir.join("alt_cpi.so"))
        .expect("failed to load alt_cpi program");
    svm.add_program_from_file(hash_cpi_id(), &deploy_dir.join("hash_cpi.so"))
        .expect("failed to load hash_cpi program");
    svm.add_program_from_file(caller_id(), &deploy_dir.join("declare_program_cpi.so"))
        .expect("failed to load declare_program program");

    let payer = keypair_for("declare-program-payer");
    svm.airdrop(&payer.pubkey(), 10_000_000_000).unwrap();

    (svm, payer)
}

fn external_store(authority: Pubkey) -> Pubkey {
    Pubkey::find_program_address(&[b"declared-store", authority.as_ref()], &external_cpi_id()).0
}

fn alt_store(authority: Pubkey) -> Pubkey {
    Pubkey::find_program_address(&[b"declared-alt", authority.as_ref()], &alt_cpi_id()).0
}

fn hash_store(authority: Pubkey) -> Pubkey {
    Pubkey::find_program_address(&[b"declared-hash", authority.as_ref()], &hash_cpi_id()).0
}

fn initialize_external_store(svm: &mut LiteSVM, payer: &Keypair, authority: &Keypair) -> Pubkey {
    let data = external_store(authority.pubkey());
    let ix_data = external_cpi::instruction::Initialize {}.data();
    let metas = external_cpi::accounts::Initialize {
        payer: payer.pubkey(),
        data,
        authority: authority.pubkey(),
        system_program: solana_sdk_ids::system_program::ID,
    }
    .to_account_metas(None);

    send_instruction(svm, external_cpi_id(), ix_data, metas, payer, &[authority])
        .expect("external_cpi::initialize should succeed");

    data
}

fn initialize_alt_store(svm: &mut LiteSVM, payer: &Keypair, authority: &Keypair) -> Pubkey {
    let data = alt_store(authority.pubkey());
    let ix_data = alt_cpi::instruction::Initialize {}.data();
    let metas = alt_cpi::accounts::Initialize {
        payer: payer.pubkey(),
        data,
        authority: authority.pubkey(),
        system_program: solana_sdk_ids::system_program::ID,
    }
    .to_account_metas(None);

    send_instruction(svm, alt_cpi_id(), ix_data, metas, payer, &[authority])
        .expect("alt_cpi::initialize should succeed");

    data
}

fn initialize_hash_store(svm: &mut LiteSVM, payer: &Keypair, authority: &Keypair) -> Pubkey {
    let data = hash_store(authority.pubkey());
    let ix_data = hash_cpi::instruction::Initialize {}.data();
    let metas = hash_cpi::accounts::Initialize {
        payer: payer.pubkey(),
        data,
        authority: authority.pubkey(),
        system_program: solana_sdk_ids::system_program::ID,
    }
    .to_account_metas(None);

    send_instruction(svm, hash_cpi_id(), ix_data, metas, payer, &[authority])
        .expect("hash_cpi::initialize should succeed");

    data
}

#[derive(Debug, PartialEq, Eq)]
struct ExternalState {
    value: u64,
    tag: [u8; 3],
    owner: Pubkey,
    count: u16,
    calls: u16,
}

fn external_state(svm: &LiteSVM, data: Pubkey) -> ExternalState {
    let account = svm.get_account(&data).expect("external store should exist");
    let value = u64::from_le_bytes(account.data[8..16].try_into().unwrap());
    let count = u16::from_le_bytes(account.data[16..18].try_into().unwrap());
    let calls = u16::from_le_bytes(account.data[18..20].try_into().unwrap());
    let owner = Pubkey::new_from_array(account.data[53..85].try_into().unwrap());
    let tag = account.data[85..88].try_into().unwrap();
    ExternalState {
        value,
        tag,
        owner,
        count,
        calls,
    }
}

#[derive(Debug, PartialEq, Eq)]
struct AltState {
    value: u64,
    delta: u8,
    calls: u8,
    authority_first_byte: u8,
}

fn alt_state(svm: &LiteSVM, data: Pubkey) -> AltState {
    let account = svm.get_account(&data).expect("alt store should exist");
    AltState {
        value: u64::from_le_bytes(account.data[8..16].try_into().unwrap()),
        delta: account.data[48],
        calls: account.data[49],
        authority_first_byte: account.data[50],
    }
}

#[derive(Debug, PartialEq, Eq)]
struct HashState {
    value: i64,
    delta: i64,
    marker: [u8; 4],
    flag: u8,
    calls: u8,
}

fn hash_state(svm: &LiteSVM, data: Pubkey) -> HashState {
    let account = svm.get_account(&data).expect("hash store should exist");
    HashState {
        value: i64::from_le_bytes(account.data[8..16].try_into().unwrap()),
        delta: i64::from_le_bytes(account.data[48..56].try_into().unwrap()),
        marker: account.data[56..60].try_into().unwrap(),
        flag: account.data[60],
        calls: account.data[61],
    }
}

#[test]
fn declare_program_exports_program_marker_and_id() {
    assert_eq!(external::ID, program_id());
    assert_eq!(external::program::External::id(), program_id());
    assert_eq!(external_cpi::ID, external_cpi_id());
    assert_eq!(external_cpi::program::ExternalCpi::id(), external_cpi_id());
    assert_eq!(alt_cpi::ID, alt_cpi_id());
    assert_eq!(alt_cpi::program::AltCpi::id(), alt_cpi_id());
    assert_eq!(hash_cpi::ID, hash_cpi_id());
    assert_eq!(hash_cpi::program::HashCpi::id(), hash_cpi_id());
}

#[test]
fn declared_instruction_builders_use_external_program_id() {
    let ix =
        external::instruction::Update { value: 42 }.to_instruction(external::accounts::Update {
            authority: Pubkey::new_unique(),
            data: Pubkey::new_unique(),
        });

    assert_eq!(ix.program_id, program_id());

    let ix = external_cpi::instruction::SetValue { value: 7 }.to_instruction(
        external_cpi::accounts::SetValue {
            data: Pubkey::new_unique(),
            authority: Pubkey::new_unique(),
        },
    );
    assert_eq!(ix.program_id, external_cpi_id());

    let ix = alt_cpi::instruction::Bump { delta: 2 }.to_instruction(alt_cpi::accounts::Bump {
        data: Pubkey::new_unique(),
        authority: Pubkey::new_unique(),
    });
    assert_eq!(ix.program_id, alt_cpi_id());

    let ix = hash_cpi::instruction::Apply {
        delta: -3,
        flag: true,
        marker: *b"hash",
    }
    .to_instruction(hash_cpi::accounts::Apply {
        data: Pubkey::new_unique(),
        authority: Pubkey::new_unique(),
    });
    assert_eq!(ix.program_id, hash_cpi_id());
}

#[test]
fn declared_default_discriminator_uses_snake_case_anchor_hash() {
    let ix =
        external::instruction::DefaultDisc {}.to_instruction(external::accounts::DefaultDisc {});

    assert_eq!(
        external::instruction::DefaultDisc::DISCRIMINATOR,
        &disc_hash("default_disc")
    );
    assert_eq!(ix.data, disc_hash("default_disc"));
}

#[test]
fn declared_explicit_discriminators_are_preserved() {
    assert_eq!(external::instruction::Update::DISCRIMINATOR, &[9, 8, 7, 6]);
    assert_eq!(
        external::instruction::DefinedArgs::DISCRIMINATOR,
        &[1, 3, 5, 7, 9, 11, 13, 15]
    );
    assert_eq!(
        external::instruction::BytesAndString::DISCRIMINATOR,
        &[2, 4, 6, 8]
    );
    assert_eq!(
        external::instruction::Composite::DISCRIMINATOR,
        &[44, 45, 46]
    );
    assert_eq!(external_cpi::instruction::Initialize::DISCRIMINATOR, &[0]);
    assert_eq!(external_cpi::instruction::SetValue::DISCRIMINATOR, &[1]);
    assert_eq!(external_cpi::instruction::Composite::DISCRIMINATOR, &[2]);
    assert_eq!(external_cpi::instruction::DefinedArgs::DISCRIMINATOR, &[3]);
    assert_eq!(alt_cpi::instruction::Initialize::DISCRIMINATOR, &[0]);
    assert_eq!(alt_cpi::instruction::Bump::DISCRIMINATOR, &[1]);
}

#[test]
fn declared_default_discriminators_are_used_for_declared_hash_cpi() {
    assert_eq!(
        hash_cpi::instruction::Initialize::DISCRIMINATOR,
        &disc_hash("initialize")
    );
    assert_eq!(
        hash_cpi::instruction::Apply::DISCRIMINATOR,
        &disc_hash("apply")
    );
}

#[test]
fn declared_scalar_and_defined_args_serialize_after_discriminator() {
    let update_data = external::instruction::Update { value: 0x1122_3344 }.data();
    assert_eq!(&update_data[..4], &[9, 8, 7, 6]);
    assert_eq!(&update_data[4..], &0x1122_3344u32.to_le_bytes());

    let owner = Pubkey::new_unique();
    let defined_data = external::instruction::DefinedArgs {
        args: external::MyArgs {
            amount: 0x0102_0304_0506_0708,
            tag: *b"id2",
            owner,
        },
    }
    .data();
    assert_eq!(&defined_data[..8], &[1, 3, 5, 7, 9, 11, 13, 15]);
    assert_eq!(
        &defined_data[8..16],
        &0x0102_0304_0506_0708u64.to_le_bytes()
    );
    assert_eq!(&defined_data[16..19], b"id2");
    assert_eq!(&defined_data[19..51], owner.as_ref());
}

#[test]
fn declared_vec_and_string_args_compile_and_include_discriminator() {
    let data = external::instruction::BytesAndString {
        payload: vec![1, 2, 3],
        label: "anchor".to_string(),
    }
    .data();

    assert_eq!(&data[..4], &[2, 4, 6, 8]);
    assert!(data.len() > 4);
}

#[test]
fn declared_account_metas_preserve_flags_and_composites() {
    let authority = Pubkey::new_unique();
    let vault = Pubkey::new_unique();
    let payer = Pubkey::new_unique();
    let metas = external::accounts::Composite {
        inner: external::__client_accounts_inner::Inner { authority, vault }.into(),
        payer,
    }
    .to_account_metas(None);

    assert_eq!(metas.len(), 3);
    assert_eq!(metas[0].pubkey, authority);
    assert!(metas[0].is_signer);
    assert!(!metas[0].is_writable);
    assert_eq!(metas[1].pubkey, vault);
    assert!(!metas[1].is_signer);
    assert!(metas[1].is_writable);
    assert_eq!(metas[2].pubkey, payer);
    assert!(metas[2].is_signer);
    assert!(metas[2].is_writable);
}

#[test]
fn declared_cpi_idl_account_metas_preserve_flags_and_composites() {
    let payer = Pubkey::new_unique();
    let data = Pubkey::new_unique();
    let authority = Pubkey::new_unique();
    let metas = external_cpi::accounts::Composite {
        inner: external_cpi::__client_accounts_inner::Inner { data, authority }.into(),
        payer,
    }
    .to_account_metas(None);

    assert_eq!(metas.len(), 3);
    assert_eq!(metas[0], AccountMeta::new(data, false));
    assert_eq!(metas[1], AccountMeta::new_readonly(authority, true));
    assert_eq!(metas[2], AccountMeta::new(payer, true));
}

#[test]
fn declared_cpi_account_surface_is_generated() {
    let _ = cpi_account_type_is_generated;
}

#[test]
fn declared_program_cpi_set_value_updates_external_program_state() {
    let (mut svm, payer) = setup();
    let authority = keypair_for("declare-program-authority-set");
    let data = initialize_external_store(&mut svm, &payer, &authority);

    let ix_data = instruction::ProxySetValue { value: 42 }.data();
    let metas = vec![
        AccountMeta::new(data, false),
        AccountMeta::new_readonly(authority.pubkey(), true),
        AccountMeta::new_readonly(external_cpi_id(), false),
    ];

    send_instruction(&mut svm, caller_id(), ix_data, metas, &payer, &[&authority])
        .expect("proxy_set_value should CPI successfully");

    assert_eq!(
        external_state(&svm, data),
        ExternalState {
            value: 43,
            tag: *b"set",
            owner: authority.pubkey(),
            count: 0,
            calls: 1,
        }
    );
}

#[test]
fn declared_program_cpi_rejects_wrong_authority() {
    let (mut svm, payer) = setup();
    let authority = keypair_for("declare-program-authority-owner");
    let impostor = keypair_for("declare-program-authority-impostor");
    let data = initialize_external_store(&mut svm, &payer, &authority);

    let result = send_instruction(
        &mut svm,
        caller_id(),
        instruction::ProxySetValue { value: 99 }.data(),
        vec![
            AccountMeta::new(data, false),
            AccountMeta::new_readonly(impostor.pubkey(), true),
            AccountMeta::new_readonly(external_cpi_id(), false),
        ],
        &payer,
        &[&impostor],
    );

    assert!(result.is_err(), "wrong authority must fail in callee CPI");
    assert_eq!(
        external_state(&svm, data),
        ExternalState {
            value: 0,
            tag: *b"ini",
            owner: Pubkey::default(),
            count: 0,
            calls: 0,
        }
    );
}

#[test]
fn declared_program_cpi_rejects_wrong_program_account() {
    let (mut svm, payer) = setup();
    let authority = keypair_for("declare-program-authority-wrong-program");
    let data = initialize_external_store(&mut svm, &payer, &authority);

    let result = send_instruction(
        &mut svm,
        caller_id(),
        instruction::ProxySetValue { value: 99 }.data(),
        vec![
            AccountMeta::new(data, false),
            AccountMeta::new_readonly(authority.pubkey(), true),
            AccountMeta::new_readonly(alt_cpi_id(), false),
        ],
        &payer,
        &[&authority],
    );

    assert!(
        result.is_err(),
        "CPI through the wrong declared program must fail"
    );
    assert_eq!(
        external_state(&svm, data),
        ExternalState {
            value: 0,
            tag: *b"ini",
            owner: Pubkey::default(),
            count: 0,
            calls: 0,
        }
    );
}

#[test]
fn declared_program_cpi_rejects_swapped_account_order() {
    let (mut svm, payer) = setup();
    let authority = keypair_for("declare-program-authority-swapped");
    let data = initialize_external_store(&mut svm, &payer, &authority);

    let result = send_instruction(
        &mut svm,
        caller_id(),
        instruction::ProxySetValue { value: 99 }.data(),
        vec![
            AccountMeta::new(authority.pubkey(), false),
            AccountMeta::new_readonly(data, true),
            AccountMeta::new_readonly(external_cpi_id(), false),
        ],
        &payer,
        &[&authority],
    );

    assert!(result.is_err(), "swapped account order must fail");
    assert_eq!(
        external_state(&svm, data),
        ExternalState {
            value: 0,
            tag: *b"ini",
            owner: Pubkey::default(),
            count: 0,
            calls: 0,
        }
    );
}

#[test]
fn declared_program_cpi_rejects_duplicate_writable_account_alias() {
    let (mut svm, payer) = setup();
    let authority = keypair_for("declare-program-authority-duplicate-writable");
    let data = initialize_external_store(&mut svm, &payer, &authority);

    let result = send_instruction(
        &mut svm,
        caller_id(),
        instruction::ProxyComposite { count: 4 }.data(),
        vec![
            AccountMeta::new(payer.pubkey(), false),
            AccountMeta::new_readonly(authority.pubkey(), true),
            AccountMeta::new(payer.pubkey(), true),
            AccountMeta::new_readonly(external_cpi_id(), false),
        ],
        &payer,
        &[&authority],
    );

    assert!(result.is_err(), "duplicate writable alias must fail");
    assert_eq!(
        external_state(&svm, data),
        ExternalState {
            value: 0,
            tag: *b"ini",
            owner: Pubkey::default(),
            count: 0,
            calls: 0,
        }
    );
}

#[test]
fn declared_program_cpi_composite_flattens_nested_accounts() {
    let (mut svm, payer) = setup();
    let authority = keypair_for("declare-program-authority-composite");
    let data = initialize_external_store(&mut svm, &payer, &authority);

    send_instruction(
        &mut svm,
        caller_id(),
        instruction::ProxySetValue { value: 5 }.data(),
        vec![
            AccountMeta::new(data, false),
            AccountMeta::new_readonly(authority.pubkey(), true),
            AccountMeta::new_readonly(external_cpi_id(), false),
        ],
        &payer,
        &[&authority],
    )
    .expect("proxy_set_value should seed value");

    let ix_data = instruction::ProxyComposite { count: 4 }.data();
    let metas = vec![
        AccountMeta::new(data, false),
        AccountMeta::new_readonly(authority.pubkey(), true),
        AccountMeta::new(payer.pubkey(), true),
        AccountMeta::new_readonly(external_cpi_id(), false),
    ];

    send_instruction(&mut svm, caller_id(), ix_data, metas, &payer, &[&authority])
        .expect("proxy_composite should CPI successfully");

    assert_eq!(
        external_state(&svm, data),
        ExternalState {
            value: 12,
            tag: *b"cmp",
            owner: authority.pubkey(),
            count: 4,
            calls: 2,
        }
    );
}

#[test]
fn declared_program_cpi_defined_args_round_trip() {
    let (mut svm, payer) = setup();
    let authority = keypair_for("declare-program-authority-defined");
    let data = initialize_external_store(&mut svm, &payer, &authority);

    let ix_data = instruction::ProxyDefinedArgs {
        amount: 123,
        tag: *b"arg",
    }
    .data();
    let metas = vec![
        AccountMeta::new(data, false),
        AccountMeta::new_readonly(authority.pubkey(), true),
        AccountMeta::new_readonly(external_cpi_id(), false),
    ];

    send_instruction(&mut svm, caller_id(), ix_data, metas, &payer, &[&authority])
        .expect("proxy_defined_args should CPI successfully");

    assert_eq!(
        external_state(&svm, data),
        ExternalState {
            value: 124,
            tag: *b"arg",
            owner: authority.pubkey(),
            count: 0,
            calls: 1,
        }
    );
}

#[test]
fn declared_program_cpi_to_second_idl() {
    let (mut svm, payer) = setup();
    let authority = keypair_for("declare-program-authority-alt");
    let data = initialize_alt_store(&mut svm, &payer, &authority);

    let ix_data = instruction::ProxyAltBump { delta: 7 }.data();
    let metas = vec![
        AccountMeta::new(data, false),
        AccountMeta::new_readonly(authority.pubkey(), true),
        AccountMeta::new_readonly(alt_cpi_id(), false),
    ];

    send_instruction(&mut svm, caller_id(), ix_data, metas, &payer, &[&authority])
        .expect("proxy_alt_bump should CPI successfully");

    assert_eq!(
        alt_state(&svm, data),
        AltState {
            value: 18,
            delta: 7,
            calls: 1,
            authority_first_byte: authority.pubkey().as_ref()[0],
        }
    );
}

#[test]
fn declared_program_cpi_to_default_discriminator_program() {
    let (mut svm, payer) = setup();
    let authority = keypair_for("declare-program-authority-hash");
    let data = initialize_hash_store(&mut svm, &payer, &authority);

    let ix_data = instruction::ProxyHashApply {
        delta: -4,
        flag: true,
        marker: *b"hsh!",
    }
    .data();
    let metas = vec![
        AccountMeta::new(data, false),
        AccountMeta::new_readonly(authority.pubkey(), true),
        AccountMeta::new_readonly(hash_cpi_id(), false),
    ];

    send_instruction(&mut svm, caller_id(), ix_data, metas, &payer, &[&authority])
        .expect("proxy_hash_apply should CPI successfully");

    assert_eq!(
        hash_state(&svm, data),
        HashState {
            value: -2,
            delta: -4,
            marker: *b"hsh!",
            flag: 1,
            calls: 1,
        }
    );
}
