use {
    anchor_lang::solana_program::instruction::AccountMeta,
    litesvm::LiteSVM,
    solana_keypair::Keypair,
    solana_pubkey::Pubkey,
    solana_signer::Signer,
    tests_v2::{build_program, keypair_for, send_instruction},
};

fn program_id() -> Pubkey {
    "MemoCp1Test11111111111111111111111111111111"
        .parse()
        .unwrap()
}

fn memo_program_id() -> Pubkey {
    "MemoSq4gqABAXKb96qnH8TysNcWxMyWCqXgDLGmfcHr"
        .parse()
        .unwrap()
}

fn setup() -> (LiteSVM, Keypair) {
    let test_dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let deploy_dir = test_dir.join("target/deploy");
    build_program(
        test_dir.join("programs/memo").to_str().unwrap(),
        deploy_dir.to_str().unwrap(),
    );
    build_program(
        test_dir.join("programs/memo-spy").to_str().unwrap(),
        deploy_dir.to_str().unwrap(),
    );

    let mut svm = LiteSVM::new();
    svm.add_program_from_file(program_id(), deploy_dir.join("memo.so"))
        .expect("load memo program");
    // Memo v3 has no vendored fixture; a spy registered at the real Memo
    // program id reproduces its signer check and returns the memo bytes.
    svm.add_program_from_file(memo_program_id(), deploy_dir.join("memo_spy.so"))
        .expect("load memo spy");

    let payer = keypair_for("memo-payer");
    svm.airdrop(&payer.pubkey(), 10_000_000_000).unwrap();
    (svm, payer)
}

/// Memo v3 is a signer list plus opaque bytes, so the caller supplies no
/// accounts of its own. Only the Memo program field is passed through, so
/// these cases forward an empty signer list.
fn sign_memo(svm: &mut LiteSVM, payer: &Keypair, text: &str) -> anyhow::Result<(u32, String)> {
    // discriminator 0, then `text` as a borsh `String`: the fixture decodes
    // its own args and forwards the raw bytes to the Memo program.
    let mut data = vec![0u8];
    data.extend_from_slice(&(text.len() as u32).to_le_bytes());
    data.extend_from_slice(text.as_bytes());

    let metas = vec![AccountMeta::new_readonly(memo_program_id(), false)];
    let meta = send_instruction(svm, program_id(), data, metas, payer, &[])?;
    Ok(observed(&meta))
}

/// The spy returns `<signer count u32 LE><memo bytes>`, so assert on what the
/// callee actually received rather than only on transaction success.
fn observed(meta: &litesvm::types::TransactionMetadata) -> (u32, String) {
    let data = &meta.return_data.data;
    assert!(
        data.len() >= 4,
        "spy must return signer count and memo bytes, got {} bytes",
        data.len()
    );
    let count = u32::from_le_bytes(data[..4].try_into().unwrap());
    (count, String::from_utf8_lossy(&data[4..]).into_owned())
}

/// Same, but with one remaining account that actually signs, so the CPI
/// forwards a non-empty signer list.
fn sign_memo_with_signer(
    svm: &mut LiteSVM,
    payer: &Keypair,
    signer: &Keypair,
    text: &str,
) -> anyhow::Result<(u32, String)> {
    // discriminator 0, then the memo payload as a borsh `String`.
    let mut data = vec![0u8];
    data.extend_from_slice(&(text.len() as u32).to_le_bytes());
    data.extend_from_slice(text.as_bytes());

    let metas = vec![
        AccountMeta::new_readonly(memo_program_id(), false),
        AccountMeta::new_readonly(signer.pubkey(), true),
    ];
    let meta = send_instruction(svm, program_id(), data, metas, payer, &[signer])?;
    Ok(observed(&meta))
}

#[test]
fn memo_without_signers_succeeds() {
    let (mut svm, payer) = setup();
    let (signers, memo) = sign_memo(&mut svm, &payer, "no signers needed")
        .expect("memo with an empty signer list must succeed: Memo v3 accepts zero signers");
    assert_eq!(
        signers, 0,
        "no remaining accounts means an empty signer list"
    );
    assert_eq!(memo, "no signers needed");
}

/// The spy logs the bytes it received, so this pins that `build_memo` forwards
/// the payload verbatim rather than dropping or re-encoding it.
#[test]
fn memo_forwards_the_payload_verbatim() {
    let (mut svm, payer) = setup();
    let (_, memo) = sign_memo(&mut svm, &payer, "verbatim payload").expect("memo should succeed");
    assert_eq!(
        memo, "verbatim payload",
        "memo bytes must reach the callee unmodified"
    );
}

#[test]
fn memo_with_a_signer_succeeds() {
    let (mut svm, payer) = setup();
    let authority = keypair_for("memo-authority");
    svm.airdrop(&authority.pubkey(), 1_000_000_000).unwrap();

    let (signers, memo) =
        sign_memo_with_signer(&mut svm, &payer, &authority, "signed by the authority")
            .expect("memo with one signer must forward a non-empty signer list");
    assert_eq!(
        signers, 1,
        "the one remaining account must be forwarded as a memo signer"
    );
    assert_eq!(memo, "signed by the authority");
}

#[test]
fn memo_forwards_the_text_to_the_memo_program() {
    let (mut svm, payer) = setup();
    let (_, memo) =
        sign_memo(&mut svm, &payer, "hello from anchor v2").expect("memo should forward text");
    assert_eq!(memo, "hello from anchor v2");
}

#[test]
fn memo_with_empty_text_succeeds() {
    let (mut svm, payer) = setup();
    sign_memo(&mut svm, &payer, "").expect("memo accepts an empty payload");
}

#[test]
fn memo_rejects_a_remaining_account_count_mismatch() {
    let (mut svm, payer) = setup();
    let authority = keypair_for("memo-count-authority");
    svm.airdrop(&authority.pubkey(), 1_000_000_000).unwrap();

    // discriminator 1: text: String, count: u8
    let mut data = vec![1u8];
    data.extend_from_slice(&2u32.to_le_bytes());
    data.extend_from_slice(b"hi");
    data.push(2); // expect two remaining accounts, but pass one

    let metas = vec![
        AccountMeta::new_readonly(memo_program_id(), false),
        AccountMeta::new_readonly(authority.pubkey(), true),
    ];
    let failure = send_instruction(&mut svm, program_id(), data, metas, &payer, &[&authority])
        .expect_err("remaining-account count mismatch must fail");

    let rendered = format!("{:?}", failure);
    assert!(
        rendered.contains("InvalidArgument"),
        "expected the program's InvalidArgument, got: {rendered}"
    );
}
