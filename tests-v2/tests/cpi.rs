use {
    anchor_lang_v2::{
        solana_program::instruction::{AccountMeta, Instruction},
        InstructionData,
    },
    litesvm::{types::TransactionResult, LiteSVM},
    solana_message::{Message, VersionedMessage},
    solana_pubkey::Pubkey,
    solana_signer::Signer,
    solana_transaction::versioned::VersionedTransaction,
    tests_v2::{build_program, keypair_for, send_instruction},
};

fn callee_id() -> Pubkey {
    "4vJ9JU1bJJE96FWSJKvHsmmFADCg4gpZQff4P3bkLKi"
        .parse()
        .unwrap()
}

fn caller_id() -> Pubkey {
    "8qbHbw2BbbTHBW1sbeqakYXVKRQM8Ne7pLK7m6CVfeR"
        .parse()
        .unwrap()
}

fn setup() -> (LiteSVM, solana_keypair::Keypair) {
    let test_dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let deploy_dir = test_dir.join("target/deploy");
    let deploy_str = deploy_dir.to_str().unwrap();

    build_program(
        test_dir.join("programs/callee").to_str().unwrap(),
        deploy_str,
    );
    build_program(
        test_dir.join("programs/caller").to_str().unwrap(),
        deploy_str,
    );

    let mut svm = LiteSVM::new();
    svm.add_program_from_file(callee_id(), &deploy_dir.join("callee.so"))
        .expect("failed to load callee program");
    svm.add_program_from_file(caller_id(), &deploy_dir.join("caller.so"))
        .expect("failed to load caller program");

    let payer = keypair_for("cpi-test-payer");
    svm.airdrop(&payer.pubkey(), 10_000_000_000).unwrap();

    (svm, payer)
}

/// Helper: initialize the callee's data account PDA.
fn init_data_account(
    svm: &mut LiteSVM,
    payer: &solana_keypair::Keypair,
    authority: &solana_keypair::Keypair,
) -> Pubkey {
    let (data_pda, _) = Pubkey::find_program_address(&[b"data"], &callee_id());

    let init_data = callee::instruction::Initialize {}.data();
    let init_metas = vec![
        AccountMeta::new(payer.pubkey(), true),
        AccountMeta::new(data_pda, false),
        AccountMeta::new_readonly(authority.pubkey(), true),
        AccountMeta::new_readonly(solana_sdk_ids::system_program::ID, false),
    ];
    send_instruction(svm, callee_id(), init_data, init_metas, payer, &[authority])
        .expect("callee::initialize should succeed");

    data_pda
}

/// Regression: an Accounts struct with zero fields used to fail E0392
/// in the auto-generated CPI module ("lifetime parameter `'a` is never
/// used"). The `_phantom: PhantomData<&'a ()>` field anchors `'a` on
/// `Self`; this test confirms the resulting `cpi::accounts::Empty::new()`
/// constructor flows through a real CPI call.
#[test]
fn test_cpi_empty_accounts() {
    let (mut svm, payer) = setup();

    let proxy_empty = caller::instruction::ProxyEmpty {}.data();
    let proxy_metas = vec![AccountMeta::new_readonly(callee_id(), false)];
    send_instruction(&mut svm, caller_id(), proxy_empty, proxy_metas, &payer, &[])
        .expect("caller::proxy_empty should succeed");
}

#[test]
fn test_direct_set_data() {
    let (mut svm, payer) = setup();
    let authority = keypair_for("authority");
    svm.airdrop(&authority.pubkey(), 1_000_000_000).unwrap();
    let data_pda = init_data_account(&mut svm, &payer, &authority);

    // Set data directly via callee.
    let value: u64 = 99;
    let set_data = callee::instruction::SetData { value }.data();
    let set_metas = vec![
        AccountMeta::new(data_pda, false),
        AccountMeta::new_readonly(authority.pubkey(), true),
    ];
    send_instruction(
        &mut svm,
        callee_id(),
        set_data,
        set_metas,
        &payer,
        &[&authority],
    )
    .expect("set_data should succeed");

    // Verify.
    let account = svm
        .get_account(&data_pda)
        .expect("data account should exist");
    let stored_value = u64::from_le_bytes(account.data[8..16].try_into().unwrap());
    assert_eq!(stored_value, 99);
}

#[test]
fn test_cpi_set_data() {
    let (mut svm, payer) = setup();
    let authority = keypair_for("authority");
    svm.airdrop(&authority.pubkey(), 1_000_000_000).unwrap();
    let data_pda = init_data_account(&mut svm, &payer, &authority);

    // Call caller::proxy_set_data which CPIs into callee::set_data.
    // The caller passes both a mutable handle (data) and a read-only
    // handle (authority) through the CpiContext.
    let value: u64 = 42;
    let proxy_data = caller::instruction::ProxySetData { value }.data();
    let proxy_metas = vec![
        AccountMeta::new(data_pda, false),
        AccountMeta::new_readonly(authority.pubkey(), true),
        AccountMeta::new_readonly(callee_id(), false),
    ];
    send_instruction(
        &mut svm,
        caller_id(),
        proxy_data,
        proxy_metas,
        &payer,
        &[&authority],
    )
    .expect("caller::proxy_set_data should succeed");

    // Verify the CPI wrote the value.
    let account = svm
        .get_account(&data_pda)
        .expect("data account should exist");
    let stored_value = u64::from_le_bytes(account.data[8..16].try_into().unwrap());
    assert_eq!(stored_value, 42, "CPI should have set value to 42");
}

fn call_raw(
    svm: &mut LiteSVM,
    program: Pubkey,
    data: Vec<u8>,
    metas: Vec<AccountMeta>,
    payer: &solana_keypair::Keypair,
    extra: &[&solana_keypair::Keypair],
) -> TransactionResult {
    let ix = Instruction::new_with_bytes(program, &data, metas);
    let blockhash = svm.latest_blockhash();
    let msg = Message::new_with_blockhash(&[ix], Some(&payer.pubkey()), &blockhash);
    let mut signers: Vec<&dyn solana_signer::Signer> = vec![payer];
    for s in extra {
        signers.push(*s);
    }
    let tx = VersionedTransaction::try_new(VersionedMessage::Legacy(msg), &signers).unwrap();
    svm.send_transaction(tx)
}

#[test]
fn test_cpi_set_data_rejects_wrong_authority() {
    let (mut svm, payer) = setup();
    let authority = keypair_for("authority");
    let impostor = keypair_for("impostor");
    svm.airdrop(&authority.pubkey(), 1_000_000_000).unwrap();
    svm.airdrop(&impostor.pubkey(), 1_000_000_000).unwrap();
    let data_pda = init_data_account(&mut svm, &payer, &authority);

    // Set initial value so we can verify it's unchanged after failure
    let set_data = callee::instruction::SetData { value: 77 }.data();
    send_instruction(
        &mut svm,
        callee_id(),
        set_data,
        vec![
            AccountMeta::new(data_pda, false),
            AccountMeta::new_readonly(authority.pubkey(), true),
        ],
        &payer,
        &[&authority],
    )
    .expect("set initial value");

    // Now try CPI with wrong authority
    let proxy_data = caller::instruction::ProxySetData { value: 999 }.data();
    let proxy_metas = vec![
        AccountMeta::new(data_pda, false),
        AccountMeta::new_readonly(impostor.pubkey(), true),
        AccountMeta::new_readonly(callee_id(), false),
    ];
    let result = call_raw(
        &mut svm,
        caller_id(),
        proxy_data,
        proxy_metas,
        &payer,
        &[&impostor],
    );
    assert!(result.is_err(), "CPI with wrong authority should fail");

    // Verify data is unchanged
    let account = svm.get_account(&data_pda).unwrap();
    let stored = u64::from_le_bytes(account.data[8..16].try_into().unwrap());
    assert_eq!(stored, 77, "value should be unchanged after failed CPI");
}

#[test]
fn test_cpi_rejects_readonly_handle_for_writable_account() {
    let (mut svm, payer) = setup();
    let authority = keypair_for("authority");
    svm.airdrop(&authority.pubkey(), 1_000_000_000).unwrap();
    let data_pda = init_data_account(&mut svm, &payer, &authority);

    let proxy_data = caller::instruction::ProxySetDataReadonlyHandle { value: 999 }.data();
    let proxy_metas = vec![
        AccountMeta::new(data_pda, false),
        AccountMeta::new_readonly(authority.pubkey(), true),
        AccountMeta::new_readonly(callee_id(), false),
    ];
    let result = call_raw(
        &mut svm,
        caller_id(),
        proxy_data,
        proxy_metas,
        &payer,
        &[&authority],
    );
    assert!(
        result.is_err(),
        "writable CPI account built from cpi_handle() should fail validation"
    );

    let account = svm.get_account(&data_pda).unwrap();
    let stored = u64::from_le_bytes(account.data[8..16].try_into().unwrap());
    assert_eq!(stored, 0, "value should be unchanged after rejected CPI");
}

/// Drives the no-extra-args branch of the cpi-wrapper codegen
/// (`callee::cpi::noop`). Also confirms that the `cpi::accounts::SetData`
/// re-export is reachable through more than one wrapper — i.e. the
/// per-Accounts dedupe doesn't accidentally drop the symbol.
#[test]
fn test_cpi_noop_no_args() {
    let (mut svm, payer) = setup();
    let authority = keypair_for("authority");
    svm.airdrop(&authority.pubkey(), 1_000_000_000).unwrap();
    let data_pda = init_data_account(&mut svm, &payer, &authority);

    // Seed a known value, then CPI through `noop`, then re-read.
    let seed = callee::instruction::SetData { value: 7 }.data();
    send_instruction(
        &mut svm,
        callee_id(),
        seed,
        vec![
            AccountMeta::new(data_pda, false),
            AccountMeta::new_readonly(authority.pubkey(), true),
        ],
        &payer,
        &[&authority],
    )
    .expect("seed value");

    let proxy_noop = caller::instruction::ProxyNoop {}.data();
    let proxy_metas = vec![
        AccountMeta::new(data_pda, false),
        AccountMeta::new_readonly(authority.pubkey(), true),
        AccountMeta::new_readonly(callee_id(), false),
    ];
    send_instruction(
        &mut svm,
        caller_id(),
        proxy_noop,
        proxy_metas,
        &payer,
        &[&authority],
    )
    .expect("caller::proxy_noop should succeed");

    // noop must not mutate the account.
    let account = svm.get_account(&data_pda).unwrap();
    let stored = u64::from_le_bytes(account.data[8..16].try_into().unwrap());
    assert_eq!(stored, 7, "noop must leave value untouched");
}

/// Drives every `InstructionAccount` ctor branch (`writable_signer`,
/// `writable`, `readonly_signer`, `readonly`) in the auto-generated
/// `ToCpiAccounts` impl by routing a CPI through `callee::cpi::touch`.
#[test]
fn test_cpi_touch_all_account_flag_combinations() {
    let (mut svm, payer) = setup();
    let authority = keypair_for("authority");
    let spectator = keypair_for("spectator");
    svm.airdrop(&authority.pubkey(), 1_000_000_000).unwrap();
    let data_pda = init_data_account(&mut svm, &payer, &authority);

    // Seed a known starting value so the saturating_add inside `touch`
    // produces a verifiable post-state (no surprise on overflow).
    let seed = callee::instruction::SetData { value: 100 }.data();
    send_instruction(
        &mut svm,
        callee_id(),
        seed,
        vec![
            AccountMeta::new(data_pda, false),
            AccountMeta::new_readonly(authority.pubkey(), true),
        ],
        &payer,
        &[&authority],
    )
    .expect("seed value");

    let proxy_touch = caller::instruction::ProxyTouch { delta: 23 }.data();
    let proxy_metas = vec![
        AccountMeta::new(payer.pubkey(), true),
        AccountMeta::new(data_pda, false),
        AccountMeta::new_readonly(authority.pubkey(), true),
        AccountMeta::new_readonly(spectator.pubkey(), false),
        AccountMeta::new_readonly(callee_id(), false),
    ];
    send_instruction(
        &mut svm,
        caller_id(),
        proxy_touch,
        proxy_metas,
        &payer,
        &[&authority],
    )
    .expect("caller::proxy_touch should succeed");

    let account = svm.get_account(&data_pda).unwrap();
    let stored = u64::from_le_bytes(account.data[8..16].try_into().unwrap());
    assert_eq!(stored, 123, "touch must add delta to the stored value");
}

/// `touch` propagates its `address = data.authority` constraint failure
/// back through CPI when the wrong signer is supplied — the auto-generated
/// `readonly_signer` ctor must mark the account as a signer so the
/// constraint sees the right is_signer flag.
#[test]
fn test_cpi_touch_rejects_wrong_authority() {
    let (mut svm, payer) = setup();
    let authority = keypair_for("authority");
    let impostor = keypair_for("impostor");
    let spectator = keypair_for("spectator");
    svm.airdrop(&authority.pubkey(), 1_000_000_000).unwrap();
    svm.airdrop(&impostor.pubkey(), 1_000_000_000).unwrap();
    let data_pda = init_data_account(&mut svm, &payer, &authority);

    let proxy_touch = caller::instruction::ProxyTouch { delta: 1 }.data();
    let proxy_metas = vec![
        AccountMeta::new(payer.pubkey(), true),
        AccountMeta::new(data_pda, false),
        AccountMeta::new_readonly(impostor.pubkey(), true),
        AccountMeta::new_readonly(spectator.pubkey(), false),
        AccountMeta::new_readonly(callee_id(), false),
    ];
    let result = call_raw(
        &mut svm,
        caller_id(),
        proxy_touch,
        proxy_metas,
        &payer,
        &[&impostor],
    );
    assert!(result.is_err(), "touch CPI with wrong authority must fail");

    // Underlying value must remain at the post-init default (0).
    let account = svm.get_account(&data_pda).unwrap();
    let stored = u64::from_le_bytes(account.data[8..16].try_into().unwrap());
    assert_eq!(stored, 0, "value must be unchanged after failed touch CPI");
}

#[test]
fn test_cpi_set_data_rejects_wrong_program() {
    let (mut svm, payer) = setup();
    let authority = keypair_for("authority");
    svm.airdrop(&authority.pubkey(), 1_000_000_000).unwrap();
    let data_pda = init_data_account(&mut svm, &payer, &authority);

    // Pass system program instead of callee as the CPI target
    let proxy_data = caller::instruction::ProxySetData { value: 42 }.data();
    let proxy_metas = vec![
        AccountMeta::new(data_pda, false),
        AccountMeta::new_readonly(authority.pubkey(), true),
        AccountMeta::new_readonly(solana_sdk_ids::system_program::ID, false),
    ];
    let result = call_raw(
        &mut svm,
        caller_id(),
        proxy_data,
        proxy_metas,
        &payer,
        &[&authority],
    );
    assert!(result.is_err(), "CPI to wrong program should fail");
}
