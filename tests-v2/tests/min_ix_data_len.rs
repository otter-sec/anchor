use {
    anchor_lang::InstructionData,
    litesvm::LiteSVM,
    solana_keypair::Keypair,
    solana_pubkey::Pubkey,
    solana_signer::Signer,
    tests_v2::{build_program, keypair_for, send_instruction},
};

fn program_id() -> Pubkey {
    "5s2e6TBgh2AYCEmW3DZi7WJYtNaLWS7M3e8dnNh4qLVA"
        .parse()
        .unwrap()
}

fn setup() -> (LiteSVM, Keypair) {
    let test_dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let deploy_dir = test_dir.join("target/deploy");
    build_program(
        test_dir.join("programs/min-ix-data-len").to_str().unwrap(),
        deploy_dir.to_str().unwrap(),
    );

    let mut svm = LiteSVM::new();
    svm.add_program_from_file(program_id(), deploy_dir.join("min_ix_data_len.so"))
        .expect("load min_ix_data_len program");
    let payer = keypair_for("min-ix-data-len-payer");
    svm.airdrop(&payer.pubkey(), 10_000_000_000).unwrap();
    (svm, payer)
}

#[test]
fn byte_discriminator_accepts_exact_wincode_payload_length() {
    let (mut svm, payer) = setup();
    let data = min_ix_data_len::instruction::ShortArgs {
        a: 7,
        b: 0x0102_0304_0506_0708,
    }
    .data();

    assert_eq!(data.len(), 10, "1-byte discriminator + 9-byte payload");

    send_instruction(&mut svm, program_id(), data, vec![], &payer, &[])
        .expect("exact wire-length args should dispatch successfully");
}
