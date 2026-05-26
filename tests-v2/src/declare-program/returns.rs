use {
    anchor_lang_v2::{
        solana_program::instruction::AccountMeta, Discriminator, InstructionData, ToAccountMetas,
    },
    declare_program_returns::{instruction, return_callee},
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

fn callee_id() -> Pubkey {
    "BF748KR4UhPq7xbhFQYd7yFKmh5UYdqed9GbD6oZvEyu"
        .parse()
        .unwrap()
}

fn spoof_id() -> Pubkey {
    "Con9ukTn9BRPXWcjS2UBbuN3NnCwy1hcaDNZ9Hb8QMNp"
        .parse()
        .unwrap()
}

fn setup() -> (LiteSVM, Keypair) {
    let test_dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let deploy_dir = test_dir.join("target/deploy");
    let deploy_str = deploy_dir.to_str().unwrap();

    build_program(
        test_dir
            .join("programs/declare-program/returns/programs/return-spoof")
            .to_str()
            .unwrap(),
        deploy_str,
    );
    build_program(
        test_dir
            .join("programs/declare-program/returns/programs/return-callee")
            .to_str()
            .unwrap(),
        deploy_str,
    );
    build_program(
        test_dir
            .join("programs/declare-program/returns")
            .to_str()
            .unwrap(),
        deploy_str,
    );

    let mut svm = LiteSVM::new();
    svm.add_program_from_file(spoof_id(), &deploy_dir.join("return_spoof.so"))
        .expect("failed to load return_spoof program");
    svm.add_program_from_file(callee_id(), &deploy_dir.join("return_callee.so"))
        .expect("failed to load return_callee program");
    svm.add_program_from_file(caller_id(), &deploy_dir.join("declare_program_returns.so"))
        .expect("failed to load declare_program_returns program");

    let payer = keypair_for("declare-program-returns-payer");
    svm.airdrop(&payer.pubkey(), 10_000_000_000).unwrap();

    (svm, payer)
}

fn callee_store(authority: Pubkey) -> Pubkey {
    Pubkey::find_program_address(&[b"return-store", authority.as_ref()], &callee_id()).0
}

fn proxy_result(authority: Pubkey) -> Pubkey {
    Pubkey::find_program_address(&[b"return-proxy", authority.as_ref()], &caller_id()).0
}

fn initialize_callee_store(svm: &mut LiteSVM, payer: &Keypair, authority: &Keypair) -> Pubkey {
    let data = callee_store(authority.pubkey());
    let ix_data = return_callee::instruction::Initialize {}.data();
    let metas = return_callee::accounts::Initialize {
        payer: payer.pubkey(),
        data,
        authority: authority.pubkey(),
        system_program: solana_sdk_ids::system_program::ID,
    }
    .to_account_metas(None);

    send_instruction(svm, callee_id(), ix_data, metas, payer, &[authority])
        .expect("return_callee::initialize should succeed");

    data
}

fn initialize_proxy_result(svm: &mut LiteSVM, payer: &Keypair, authority: &Keypair) -> Pubkey {
    let result = proxy_result(authority.pubkey());
    let ix_data = instruction::InitializeResult {}.data();
    let metas = vec![
        AccountMeta::new(payer.pubkey(), true),
        AccountMeta::new(result, false),
        AccountMeta::new_readonly(authority.pubkey(), true),
        AccountMeta::new_readonly(solana_sdk_ids::system_program::ID, false),
    ];

    send_instruction(svm, caller_id(), ix_data, metas, payer, &[authority])
        .expect("declare_program_returns::initialize_result should succeed");

    result
}

#[derive(Debug, PartialEq, Eq)]
struct CalleeState {
    last_base: u64,
    last_result: u64,
    calls: u16,
}

fn callee_state(svm: &LiteSVM, data: Pubkey) -> CalleeState {
    let account = svm.get_account(&data).expect("callee store should exist");
    CalleeState {
        last_base: u64::from_le_bytes(account.data[40..48].try_into().unwrap()),
        last_result: u64::from_le_bytes(account.data[48..56].try_into().unwrap()),
        calls: u16::from_le_bytes(account.data[56..58].try_into().unwrap()),
    }
}

#[derive(Debug, PartialEq, Eq)]
struct ProxyState {
    last_return: u64,
    last_observed: u64,
    last_payload_amount: u64,
    last_payload_sample_sum: u64,
    last_payload_label_len: u32,
    calls: u16,
    last_payload_has_authority: u8,
}

fn proxy_state(svm: &LiteSVM, result: Pubkey) -> ProxyState {
    let account = svm.get_account(&result).expect("proxy result should exist");
    ProxyState {
        last_return: u64::from_le_bytes(account.data[40..48].try_into().unwrap()),
        last_observed: u64::from_le_bytes(account.data[48..56].try_into().unwrap()),
        last_payload_amount: u64::from_le_bytes(account.data[56..64].try_into().unwrap()),
        last_payload_sample_sum: u64::from_le_bytes(account.data[64..72].try_into().unwrap()),
        last_payload_label_len: u32::from_le_bytes(account.data[72..76].try_into().unwrap()),
        calls: u16::from_le_bytes(account.data[76..78].try_into().unwrap()),
        last_payload_has_authority: account.data[78],
    }
}

#[test]
fn declared_return_instruction_has_v1_style_return_wrapper() {
    let _wrapper = return_callee::cpi::calculate;
    let _defined_wrapper = return_callee::cpi::describe;
    assert_eq!(return_callee::instruction::Calculate::DISCRIMINATOR, &[1]);
    assert_eq!(return_callee::instruction::Describe::DISCRIMINATOR, &[2]);
}

fn expected_return_payload_bytes(amount: u64, authority: Pubkey) -> Vec<u8> {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(&amount.to_le_bytes());
    bytes.extend_from_slice(&(14u32).to_le_bytes());
    bytes.extend_from_slice(b"return-payload");
    bytes.extend_from_slice(&(2u32).to_le_bytes());
    bytes.extend_from_slice(&4u16.to_le_bytes());
    bytes.extend_from_slice(&1u16.to_le_bytes());
    bytes.push(1);
    bytes.extend_from_slice(authority.as_ref());
    bytes
}

fn expected_empty_return_payload_bytes(amount: u64) -> Vec<u8> {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(&amount.to_le_bytes());
    bytes.extend_from_slice(&0u32.to_le_bytes());
    bytes.extend_from_slice(&0u32.to_le_bytes());
    bytes.push(0);
    bytes
}

fn assert_failed_return_preserves_state(
    svm: &mut LiteSVM,
    payer: &Keypair,
    authority: &Keypair,
    callee_data: Pubkey,
    result: Pubkey,
    ix_data: Vec<u8>,
    metas: Vec<AccountMeta>,
) {
    let send_result = send_instruction(svm, caller_id(), ix_data, metas, payer, &[authority]);
    assert!(send_result.is_err(), "malformed return path must fail");
    assert_eq!(
        callee_state(svm, callee_data),
        CalleeState {
            last_base: 0,
            last_result: 0,
            calls: 0,
        }
    );
    assert_eq!(
        proxy_state(svm, result),
        ProxyState {
            last_return: 0,
            last_observed: 0,
            last_payload_amount: 0,
            last_payload_sample_sum: 0,
            last_payload_label_len: 0,
            calls: 0,
            last_payload_has_authority: 0,
        }
    );
}

#[test]
fn declare_program_cpi_return_round_trips_and_state_changes_persist() {
    let (mut svm, payer) = setup();
    let authority = keypair_for("declare-program-returns-authority");
    let callee_data = initialize_callee_store(&mut svm, &payer, &authority);
    let result = initialize_proxy_result(&mut svm, &payer, &authority);

    let ix_data = instruction::ProxyCalculate { base: 17, bonus: 9 }.data();
    let metas = vec![
        AccountMeta::new(result, false),
        AccountMeta::new(callee_data, false),
        AccountMeta::new_readonly(authority.pubkey(), true),
        AccountMeta::new_readonly(callee_id(), false),
    ];

    send_instruction(&mut svm, caller_id(), ix_data, metas, &payer, &[&authority])
        .expect("proxy_calculate should CPI and read return data");

    assert_eq!(
        callee_state(&svm, callee_data),
        CalleeState {
            last_base: 17,
            last_result: 44,
            calls: 1,
        }
    );
    assert_eq!(
        proxy_state(&svm, result),
        ProxyState {
            last_return: 44,
            last_observed: 45,
            last_payload_amount: 0,
            last_payload_sample_sum: 0,
            last_payload_label_len: 0,
            calls: 1,
            last_payload_has_authority: 0,
        }
    );
}

#[test]
fn declare_program_cpi_return_defined_type_uses_borsh_wire_format() {
    let (mut svm, payer) = setup();
    let authority = keypair_for("declare-program-returns-defined-authority");
    let callee_data = initialize_callee_store(&mut svm, &payer, &authority);
    let result = initialize_proxy_result(&mut svm, &payer, &authority);

    let ix_data = instruction::ProxyDescribe { base: 6, bonus: 4 }.data();
    let metas = vec![
        AccountMeta::new(result, false),
        AccountMeta::new(callee_data, false),
        AccountMeta::new_readonly(authority.pubkey(), true),
        AccountMeta::new_readonly(callee_id(), false),
    ];

    let meta = send_instruction(&mut svm, caller_id(), ix_data, metas, &payer, &[&authority])
        .expect("proxy_describe should CPI and read defined return data");

    assert_eq!(meta.return_data.program_id, callee_id());
    assert_eq!(
        meta.return_data.data,
        expected_return_payload_bytes(23, authority.pubkey())
    );
    assert_eq!(
        callee_state(&svm, callee_data),
        CalleeState {
            last_base: 6,
            last_result: 23,
            calls: 1,
        }
    );
    assert_eq!(
        proxy_state(&svm, result),
        ProxyState {
            last_return: 0,
            last_observed: 0,
            last_payload_amount: 23,
            last_payload_sample_sum: 5,
            last_payload_label_len: 14,
            calls: 1,
            last_payload_has_authority: 1,
        }
    );
}

#[test]
fn declare_program_cpi_return_none_rejects_missing_return_data() {
    let (mut svm, payer) = setup();
    let authority = keypair_for("declare-program-returns-none-authority");
    let callee_data = initialize_callee_store(&mut svm, &payer, &authority);
    let result = initialize_proxy_result(&mut svm, &payer, &authority);

    assert_failed_return_preserves_state(
        &mut svm,
        &payer,
        &authority,
        callee_data,
        result,
        instruction::ProxyNoReturn { base: 33 }.data(),
        vec![
            AccountMeta::new(result, false),
            AccountMeta::new(callee_data, false),
            AccountMeta::new_readonly(authority.pubkey(), true),
            AccountMeta::new_readonly(callee_id(), false),
        ],
    );
}

#[test]
fn declare_program_cpi_return_rejects_truncated_scalar_return_data() {
    let (mut svm, payer) = setup();
    let authority = keypair_for("declare-program-returns-short-authority");
    let callee_data = initialize_callee_store(&mut svm, &payer, &authority);
    let result = initialize_proxy_result(&mut svm, &payer, &authority);

    assert_failed_return_preserves_state(
        &mut svm,
        &payer,
        &authority,
        callee_data,
        result,
        instruction::ProxyShortReturn { base: 33 }.data(),
        vec![
            AccountMeta::new(result, false),
            AccountMeta::new(callee_data, false),
            AccountMeta::new_readonly(authority.pubkey(), true),
            AccountMeta::new_readonly(callee_id(), false),
        ],
    );
}

#[test]
fn declare_program_cpi_return_rejects_wrong_return_program() {
    let (mut svm, payer) = setup();
    let authority = keypair_for("declare-program-returns-spoof-authority");
    let callee_data = initialize_callee_store(&mut svm, &payer, &authority);
    let result = initialize_proxy_result(&mut svm, &payer, &authority);

    assert_failed_return_preserves_state(
        &mut svm,
        &payer,
        &authority,
        callee_data,
        result,
        instruction::ProxySpoofedReturn { base: 33 }.data(),
        vec![
            AccountMeta::new(result, false),
            AccountMeta::new(callee_data, false),
            AccountMeta::new_readonly(authority.pubkey(), true),
            AccountMeta::new_readonly(callee_id(), false),
            AccountMeta::new_readonly(spoof_id(), false),
        ],
    );
}

#[test]
fn declare_program_cpi_return_rejects_malformed_defined_return_data() {
    let (mut svm, payer) = setup();
    let authority = keypair_for("declare-program-returns-malformed-authority");
    let callee_data = initialize_callee_store(&mut svm, &payer, &authority);
    let result = initialize_proxy_result(&mut svm, &payer, &authority);

    assert_failed_return_preserves_state(
        &mut svm,
        &payer,
        &authority,
        callee_data,
        result,
        instruction::ProxyMalformedPayload { base: 33 }.data(),
        vec![
            AccountMeta::new(result, false),
            AccountMeta::new(callee_data, false),
            AccountMeta::new_readonly(authority.pubkey(), true),
            AccountMeta::new_readonly(callee_id(), false),
        ],
    );
}

#[test]
fn declare_program_cpi_return_defined_type_handles_empty_and_none_fields() {
    let (mut svm, payer) = setup();
    let authority = keypair_for("declare-program-returns-empty-authority");
    let callee_data = initialize_callee_store(&mut svm, &payer, &authority);
    let result = initialize_proxy_result(&mut svm, &payer, &authority);

    let ix_data = instruction::ProxyDescribeEmpty { base: 6, bonus: 4 }.data();
    let metas = vec![
        AccountMeta::new(result, false),
        AccountMeta::new(callee_data, false),
        AccountMeta::new_readonly(authority.pubkey(), true),
        AccountMeta::new_readonly(callee_id(), false),
    ];

    let meta = send_instruction(&mut svm, caller_id(), ix_data, metas, &payer, &[&authority])
        .expect("proxy_describe_empty should CPI and read empty return payload");

    assert_eq!(meta.return_data.program_id, callee_id());
    assert_eq!(
        meta.return_data.data,
        expected_empty_return_payload_bytes(35)
    );
    assert_eq!(
        callee_state(&svm, callee_data),
        CalleeState {
            last_base: 6,
            last_result: 35,
            calls: 1,
        }
    );
    assert_eq!(
        proxy_state(&svm, result),
        ProxyState {
            last_return: 0,
            last_observed: 0,
            last_payload_amount: 35,
            last_payload_sample_sum: 0,
            last_payload_label_len: 0,
            calls: 1,
            last_payload_has_authority: 0,
        }
    );
}

#[test]
fn declare_program_cpi_return_rejects_wrong_authority_before_return_data() {
    let (mut svm, payer) = setup();
    let authority = keypair_for("declare-program-returns-owner");
    let impostor = keypair_for("declare-program-returns-impostor");
    let callee_data = initialize_callee_store(&mut svm, &payer, &authority);
    let result = initialize_proxy_result(&mut svm, &payer, &authority);

    let send_result = send_instruction(
        &mut svm,
        caller_id(),
        instruction::ProxyCalculate { base: 17, bonus: 9 }.data(),
        vec![
            AccountMeta::new(result, false),
            AccountMeta::new(callee_data, false),
            AccountMeta::new_readonly(impostor.pubkey(), true),
            AccountMeta::new_readonly(callee_id(), false),
        ],
        &payer,
        &[&impostor],
    );

    assert!(send_result.is_err(), "wrong authority must fail");
    assert_eq!(
        callee_state(&svm, callee_data),
        CalleeState {
            last_base: 0,
            last_result: 0,
            calls: 0,
        }
    );
    assert_eq!(
        proxy_state(&svm, result),
        ProxyState {
            last_return: 0,
            last_observed: 0,
            last_payload_amount: 0,
            last_payload_sample_sum: 0,
            last_payload_label_len: 0,
            calls: 0,
            last_payload_has_authority: 0,
        }
    );
}
