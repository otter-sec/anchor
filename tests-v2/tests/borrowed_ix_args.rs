use {
    anchor_lang::{Address, InstructionData},
    anchor_v2_testing::litesvm::LiteSVM,
    borrowed_ix_args::{BatchArgs, Entry, WideEntry},
    solana_keypair::Keypair,
    solana_pubkey::Pubkey,
    solana_signer::Signer,
    tests_v2::{build_program, keypair_for, send_instruction},
};

fn program_id() -> Pubkey {
    "3ZQUcsU4hkY7erFj7FMyvmXufuqrcK6AVPWSwz2bDM98"
        .parse()
        .unwrap()
}

fn setup() -> (LiteSVM, Keypair) {
    let test_dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let deploy_dir = test_dir.join("target/deploy");
    build_program(
        test_dir.join("programs/borrowed-ix-args").to_str().unwrap(),
        deploy_dir.to_str().unwrap(),
    );

    let mut svm = LiteSVM::new();
    svm.add_program_from_file(program_id(), deploy_dir.join("borrowed_ix_args.so"))
        .expect("load borrowed_ix_args program");
    let payer = keypair_for("borrowed-ix-args-payer");
    svm.airdrop(&payer.pubkey(), 10_000_000_000).unwrap();
    (svm, payer)
}

fn entries() -> Vec<Entry> {
    (1..=4u8)
        .map(|i| Entry {
            owner: Address::new_from_array([i; 32]),
            amount: (i as u64 * 1_000).to_le_bytes(),
        })
        .collect()
}

/// Mirrors the program's `digest`.
fn expected_digest(a: u32, some_data: &[u8], entries: &[Entry]) -> u64 {
    entries
        .iter()
        .fold(a as u64 + some_data.len() as u64, |acc, entry| {
            acc.wrapping_add(u64::from_le_bytes(entry.amount))
        })
}

fn returned_u64(meta: &anchor_v2_testing::litesvm::types::TransactionMetadata) -> u64 {
    assert_eq!(meta.return_data.program_id, program_id());
    u64::from_le_bytes(meta.return_data.data.as_slice().try_into().unwrap())
}

#[test]
fn borrowed_struct_args_decode_in_place_on_sbf() {
    let (mut svm, payer) = setup();
    let some_data = b"borrowed straight from the input region";
    let entries = entries();
    let data = borrowed_ix_args::instruction::SubmitBatch {
        args: BatchArgs {
            a: 7,
            some_data,
            entries: &entries,
        },
    }
    .data();

    let meta = send_instruction(&mut svm, program_id(), data, vec![], &payer, &[])
        .expect("borrowed struct args should decode in place");
    assert_eq!(returned_u64(&meta), expected_digest(7, some_data, &entries));
}

#[test]
fn borrowed_flat_args_decode_in_place_on_sbf() {
    let (mut svm, payer) = setup();
    let some_data = [0xAB; 3];
    let entries = entries();
    let data = borrowed_ix_args::instruction::SubmitFlat {
        a: 11,
        some_data: &some_data,
        entries: &entries,
    }
    .data();

    let meta = send_instruction(&mut svm, program_id(), data, vec![], &payer, &[])
        .expect("borrowed flat args should decode in place");
    assert_eq!(
        returned_u64(&meta),
        expected_digest(11, &some_data, &entries)
    );
}

#[test]
fn misaligned_wide_struct_slice_is_rejected_on_sbf() {
    let (mut svm, payer) = setup();
    let entries = [WideEntry {
        owner: Address::new_from_array([9; 32]),
        amount: 1,
    }];
    // Elements start at disc (8) + length prefix (4) = 12: not u64-aligned.
    let data = borrowed_ix_args::instruction::SubmitWide { entries: &entries }.data();

    let err = send_instruction(&mut svm, program_id(), data, vec![], &payer, &[])
        .expect_err("misaligned `&[WideEntry]` must not decode")
        .to_string();
    assert!(err.contains("InvalidInstructionData"), "{err}");
}
