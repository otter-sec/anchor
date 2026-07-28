use {
    anchor_lang_v2::{prelude::Address, solana_program::instruction::AccountMeta, InstructionData},
    litesvm::LiteSVM,
    solana_account::Account,
    solana_keypair::Keypair,
    solana_pubkey::Pubkey,
    solana_signer::Signer,
    tests_v2::{build_program, keypair_for, send_instruction},
};

fn program_id() -> Pubkey {
    "Gz3iDiZL332qCU7J2H6yvrXbdeSAeyhUT4Q3ZcLJnb4S"
        .parse()
        .unwrap()
}

fn address(pubkey: Pubkey) -> Address {
    Address::new_from_array(pubkey.to_bytes())
}

fn marker_seeded(marker: Pubkey) -> Pubkey {
    Pubkey::find_program_address(&[b"marker", marker.as_ref()], &program_id()).0
}

fn setup() -> (LiteSVM, Keypair) {
    let test_dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let deploy_dir = test_dir.join("target/deploy");
    build_program(
        test_dir.join("programs/ix-macro").to_str().unwrap(),
        deploy_dir.to_str().unwrap(),
    );

    let mut svm = LiteSVM::new();
    svm.add_program_from_file(program_id(), deploy_dir.join("ix_macro.so"))
        .expect("load ix_macro program");
    let payer = keypair_for("ix-macro-payer");
    svm.airdrop(&payer.pubkey(), 10_000_000_000).unwrap();
    (svm, payer)
}

fn install_account(svm: &mut LiteSVM, key: Pubkey, owner: Pubkey, data_len: usize) {
    svm.set_account(
        key,
        Account {
            lamports: 1_000_000,
            data: vec![0u8; data_len],
            owner,
            executable: false,
            rent_epoch: 0,
        },
    )
    .unwrap();
}

#[test]
fn instruction_args_feed_account_constraints_and_handler() {
    let (mut svm, payer) = setup();
    let marker = Pubkey::new_unique();
    let seeded = marker_seeded(marker);
    let out = Pubkey::new_unique();
    install_account(&mut svm, marker, solana_sdk_ids::system_program::ID, 0);
    install_account(&mut svm, seeded, program_id(), 0);
    install_account(&mut svm, out, program_id(), 64);

    let amount = 0x0102_0304_0506_0708u64;
    let tag = *b"ixok";
    let data = ix_macro::instruction::CheckArgs {
        expected_marker: address(marker),
        amount,
        tag,
    }
    .data();

    send_instruction(
        &mut svm,
        program_id(),
        data,
        vec![
            AccountMeta::new_readonly(marker, false),
            AccountMeta::new_readonly(seeded, false),
            AccountMeta::new(out, false),
        ],
        &payer,
        &[],
    )
    .expect("instruction args should parse for constraints and handler");

    let out_account = svm.get_account(&out).expect("out exists");
    assert_eq!(
        u64::from_le_bytes(out_account.data[..8].try_into().unwrap()),
        amount
    );
    assert_eq!(&out_account.data[8..12], &tag);
    assert_eq!(&out_account.data[12..44], marker.as_ref());
    assert_eq!(
        out_account.data[44],
        Pubkey::find_program_address(&[b"marker", marker.as_ref()], &program_id()).1
    );
}

#[test]
fn instruction_arg_address_constraint_rejects_wrong_account() {
    let (mut svm, payer) = setup();
    let expected_marker = Pubkey::new_unique();
    let actual_marker = Pubkey::new_unique();
    let seeded = marker_seeded(expected_marker);
    let out = Pubkey::new_unique();
    install_account(
        &mut svm,
        actual_marker,
        solana_sdk_ids::system_program::ID,
        0,
    );
    install_account(&mut svm, seeded, program_id(), 0);
    install_account(&mut svm, out, program_id(), 64);

    let data = ix_macro::instruction::CheckArgs {
        expected_marker: address(expected_marker),
        amount: 0x0102_0304_0506_0708,
        tag: *b"ixok",
    }
    .data();

    let err = send_instruction(
        &mut svm,
        program_id(),
        data,
        vec![
            AccountMeta::new_readonly(actual_marker, false),
            AccountMeta::new_readonly(seeded, false),
            AccountMeta::new(out, false),
        ],
        &payer,
        &[],
    )
    .expect_err("address constraint should reject mismatched marker")
    .to_string();
    assert!(err.contains("Custom(2012)"), "unexpected error: {err}");
}
