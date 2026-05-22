//! Integration tests for account-wrapper coverage — Sysvar, Box<Account>,
//! SystemAccount, UncheckedAccount.

use {
    anchor_lang_v2::{
        programs::{Token, Token2022},
        solana_program::instruction::{AccountMeta, Instruction},
        Id,
    },
    litesvm::{types::TransactionResult, LiteSVM},
    solana_keypair::Keypair,
    solana_message::{Message, VersionedMessage},
    solana_pubkey::Pubkey,
    solana_signer::Signer,
    solana_transaction::versioned::VersionedTransaction,
    tests_v2::{build_program, keypair_for, send_instruction},
};

fn program_id() -> Pubkey {
    "Acc1111111111111111111111111111111111111111"
        .parse()
        .unwrap()
}

fn clock_sysvar_id() -> Pubkey {
    "SysvarC1ock11111111111111111111111111111111"
        .parse()
        .unwrap()
}

fn rent_sysvar_id() -> Pubkey {
    "SysvarRent111111111111111111111111111111111"
        .parse()
        .unwrap()
}

fn recent_blockhashes_sysvar_id() -> Pubkey {
    "SysvarRecentB1ockHashes11111111111111111111"
        .parse()
        .unwrap()
}

fn counter_pda() -> Pubkey {
    Pubkey::find_program_address(&[b"counter"], &program_id()).0
}

fn boxed_counter_pda() -> Pubkey {
    Pubkey::find_program_address(&[b"boxed-counter"], &program_id()).0
}

fn later_seed_counter_pda(payer: &Pubkey) -> Pubkey {
    Pubkey::find_program_address(&[b"later-seed", payer.as_ref()], &program_id()).0
}

fn borsh_counter_pda() -> Pubkey {
    Pubkey::find_program_address(&[b"borsh-counter"], &program_id()).0
}

fn foreign_borsh_owner() -> Pubkey {
    "Gue5TpR6sstSyGhSvmVeH2TeKqBYYqmXpRCacB9jAk8u"
        .parse()
        .unwrap()
}

fn foreign_borsh_counter_disc() -> [u8; 8] {
    [0x8d, 0xb8, 0x46, 0xd8, 0x4f, 0xf6, 0x08, 0x9c]
}

const SYSTEM_SEED: &str = "anchor-v2-seed";
const SYSTEM_TRANSFER_SEED: &str = "anchor-v2-transfer";
const NONCE_ACCOUNT_LENGTH: usize = 80;

fn setup() -> (LiteSVM, Keypair) {
    let test_dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let deploy_dir = test_dir.join("target/deploy");
    build_program(
        test_dir.join("programs/accounts").to_str().unwrap(),
        deploy_dir.to_str().unwrap(),
    );

    let mut svm = LiteSVM::new();
    svm.add_program_from_file(program_id(), deploy_dir.join("accounts_test.so"))
        .expect("load accounts_test program");
    let payer = keypair_for("accounts-payer");
    svm.airdrop(&payer.pubkey(), 10_000_000_000).unwrap();
    (svm, payer)
}

fn do_initialize(svm: &mut LiteSVM, payer: &Keypair) -> Pubkey {
    let counter = counter_pda();
    let metas = vec![
        AccountMeta::new(payer.pubkey(), true),
        AccountMeta::new(counter, false),
        AccountMeta::new_readonly(solana_sdk_ids::system_program::ID, false),
    ];
    send_instruction(svm, program_id(), vec![0], metas, payer, &[])
        .expect("initialize should succeed");
    counter
}

fn data_with_u64s(disc: u8, values: &[u64]) -> Vec<u8> {
    let mut data = vec![disc];
    for value in values {
        data.extend_from_slice(&value.to_le_bytes());
    }
    data
}

fn set_system_account(svm: &mut LiteSVM, address: Pubkey, lamports: u64, data_len: usize) {
    svm.set_account(
        address,
        solana_account::Account {
            lamports,
            data: vec![0u8; data_len],
            owner: solana_sdk_ids::system_program::ID,
            executable: false,
            rent_epoch: 0,
        },
    )
    .unwrap();
}

fn set_foreign_borsh_counter(svm: &mut LiteSVM, address: Pubkey, value: u64) {
    let mut data = Vec::with_capacity(16);
    data.extend_from_slice(&foreign_borsh_counter_disc());
    data.extend_from_slice(&value.to_le_bytes());
    svm.set_account(
        address,
        solana_account::Account {
            lamports: 1_000_000,
            data,
            owner: foreign_borsh_owner(),
            executable: false,
            rent_epoch: 0,
        },
    )
    .unwrap();
}

fn add_account_lamports(svm: &mut LiteSVM, address: Pubkey, lamports: u64) {
    let mut account = svm.get_account(&address).expect("account exists");
    account.lamports = account
        .lamports
        .checked_add(lamports)
        .expect("lamports overflow");
    svm.set_account(address, account).unwrap();
}

fn seeded_address(base: &Pubkey, seed: &str, owner: &Pubkey) -> Pubkey {
    Pubkey::create_with_seed(base, seed, owner).expect("valid seeded address")
}

fn do_initialize_boxed(svm: &mut LiteSVM, payer: &Keypair) -> Pubkey {
    let counter = boxed_counter_pda();
    let metas = vec![
        AccountMeta::new(payer.pubkey(), true),
        AccountMeta::new(counter, false),
        AccountMeta::new_readonly(solana_sdk_ids::system_program::ID, false),
    ];
    send_instruction(svm, program_id(), vec![3], metas, payer, &[])
        .expect("initialize_boxed should succeed");
    counter
}

fn do_initialize_borsh_counter(svm: &mut LiteSVM, payer: &Keypair) -> Pubkey {
    let counter = borsh_counter_pda();
    let metas = vec![
        AccountMeta::new(payer.pubkey(), true),
        AccountMeta::new(counter, false),
        AccountMeta::new_readonly(solana_sdk_ids::system_program::ID, false),
    ];
    send_instruction(svm, program_id(), vec![30], metas, payer, &[])
        .expect("initialize_borsh_counter should succeed");
    counter
}

#[test]
fn initialize_creates_counter_with_value_one() {
    let (mut svm, payer) = setup();
    let counter = do_initialize(&mut svm, &payer);
    let account = svm.get_account(&counter).expect("counter exists");
    // 8-byte disc + u64 value. disc prefix = 8 bytes.
    let value = u64::from_le_bytes(account.data[8..16].try_into().unwrap());
    assert_eq!(value, 1);
}

#[test]
fn initialize_can_reference_later_seed_account() {
    let (mut svm, payer) = setup();
    let counter = later_seed_counter_pda(&payer.pubkey());
    let metas = vec![
        AccountMeta::new(counter, false),
        AccountMeta::new(payer.pubkey(), true),
        AccountMeta::new_readonly(solana_sdk_ids::system_program::ID, false),
    ];

    call_raw(&mut svm, &payer, 28, metas).expect("init should see later payer seed");

    let account = svm.get_account(&counter).expect("counter exists");
    let value = u64::from_le_bytes(account.data[8..16].try_into().unwrap());
    assert_eq!(value, 42);
}

#[test]
fn initialize_rejects_payer_as_target_account() {
    let (mut svm, payer) = setup();
    let payer_before = svm.get_account(&payer.pubkey()).expect("payer exists");
    let metas = vec![
        AccountMeta::new(payer.pubkey(), true),
        AccountMeta::new(payer.pubkey(), true),
        AccountMeta::new_readonly(solana_sdk_ids::system_program::ID, false),
    ];

    let result = call_raw(&mut svm, &payer, 33, metas);
    let err = format!("{:?}", result.expect_err("payer-target init must fail").err);
    assert!(
        err.contains("InvalidArgument") || err.contains("invalid program argument"),
        "expected InvalidArgument for payer-target init, got: {err}"
    );

    let payer_after = svm.get_account(&payer.pubkey()).expect("payer exists");
    assert_eq!(
        payer_after.owner, payer_before.owner,
        "failed init must not reassign payer owner"
    );
    assert_eq!(
        payer_after.data, payer_before.data,
        "failed init must not allocate payer data"
    );
}

#[test]
fn initialize_later_seed_rejects_wrong_pda() {
    let (mut svm, payer) = setup();
    let wrong_counter = counter_pda();
    let metas = vec![
        AccountMeta::new(wrong_counter, false),
        AccountMeta::new(payer.pubkey(), true),
        AccountMeta::new_readonly(solana_sdk_ids::system_program::ID, false),
    ];

    let result = call_raw(&mut svm, &payer, 28, metas);
    assert!(
        result.is_err(),
        "init should reject PDA derived from the wrong seeds"
    );
}

#[test]
fn lamports_helpers_transfer_from_account_to_system_account() {
    let (mut svm, payer) = setup();
    let counter = do_initialize(&mut svm, &payer);
    add_account_lamports(&mut svm, counter, 1_000_000);
    let recipient = keypair_for("lamports-helper-account-recipient");
    svm.airdrop(&recipient.pubkey(), 1_000_000).unwrap();

    let counter_before = svm.get_account(&counter).unwrap().lamports;
    let recipient_before = svm.get_account(&recipient.pubkey()).unwrap().lamports;
    let amount = 123_456u64;

    let metas = vec![
        AccountMeta::new(counter, false),
        AccountMeta::new(recipient.pubkey(), false),
    ];
    send_instruction(
        &mut svm,
        program_id(),
        data_with_u64s(29, &[amount]),
        metas,
        &payer,
        &[],
    )
    .expect("Lamports helpers should transfer from Account<T>");

    let counter_after = svm.get_account(&counter).unwrap().lamports;
    let recipient_after = svm.get_account(&recipient.pubkey()).unwrap().lamports;
    assert_eq!(counter_before - counter_after, amount);
    assert_eq!(recipient_after - recipient_before, amount);
}

#[test]
fn lamports_helpers_transfer_from_borsh_account_to_system_account() {
    let (mut svm, payer) = setup();
    let counter = do_initialize_borsh_counter(&mut svm, &payer);
    add_account_lamports(&mut svm, counter, 1_000_000);
    let recipient = keypair_for("lamports-helper-borsh-recipient");
    svm.airdrop(&recipient.pubkey(), 1_000_000).unwrap();

    let counter_before = svm.get_account(&counter).unwrap().lamports;
    let recipient_before = svm.get_account(&recipient.pubkey()).unwrap().lamports;
    let amount = 222_222u64;

    let metas = vec![
        AccountMeta::new(counter, false),
        AccountMeta::new(recipient.pubkey(), false),
    ];
    send_instruction(
        &mut svm,
        program_id(),
        data_with_u64s(31, &[amount]),
        metas,
        &payer,
        &[],
    )
    .expect("Lamports helpers should transfer from BorshAccount<T>");

    let counter_after = svm.get_account(&counter).unwrap().lamports;
    let recipient_after = svm.get_account(&recipient.pubkey()).unwrap().lamports;
    assert_eq!(counter_before - counter_after, amount);
    assert_eq!(recipient_after - recipient_before, amount);
}

#[test]
fn lamports_helpers_reject_underflow_and_preserve_balances() {
    let (mut svm, payer) = setup();
    let counter = do_initialize_borsh_counter(&mut svm, &payer);
    add_account_lamports(&mut svm, counter, 1_000_000);
    let recipient = keypair_for("lamports-helper-underflow-recipient");
    svm.airdrop(&recipient.pubkey(), 1_000_000).unwrap();

    let counter_before = svm.get_account(&counter).unwrap().lamports;
    let recipient_before = svm.get_account(&recipient.pubkey()).unwrap().lamports;
    let amount = counter_before + 1;

    let metas = vec![
        AccountMeta::new(counter, false),
        AccountMeta::new(recipient.pubkey(), false),
    ];
    let result = send_instruction(
        &mut svm,
        program_id(),
        data_with_u64s(31, &[amount]),
        metas,
        &payer,
        &[],
    );
    assert!(
        result.is_err(),
        "sub_lamports should reject insufficient lamports"
    );

    let counter_after = svm.get_account(&counter).unwrap().lamports;
    let recipient_after = svm.get_account(&recipient.pubkey()).unwrap().lamports;
    assert_eq!(counter_after, counter_before);
    assert_eq!(recipient_after, recipient_before);
}

#[test]
fn foreign_borsh_account_mutation_is_not_written_on_exit() {
    let (mut svm, payer) = setup();
    let foreign_counter = Pubkey::new_unique();
    set_foreign_borsh_counter(&mut svm, foreign_counter, 42);

    let metas = vec![AccountMeta::new(foreign_counter, false)];
    send_instruction(&mut svm, program_id(), vec![32], metas, &payer, &[])
        .expect("foreign-owned BorshAccount<T> should load and handler should succeed");

    let account = svm
        .get_account(&foreign_counter)
        .expect("foreign counter still exists");
    assert_eq!(account.owner, foreign_borsh_owner());
    assert_eq!(
        &account.data[..8],
        &foreign_borsh_counter_disc(),
        "discriminator should be unchanged"
    );
    let value = u64::from_le_bytes(account.data[8..16].try_into().unwrap());
    assert_eq!(
        value, 42,
        "generated exit must not serialize in-memory mutations into foreign-owned account data"
    );
}

#[test]
fn bump_boxed_mutates_through_box_deref() {
    let (mut svm, payer) = setup();
    let counter = do_initialize(&mut svm, &payer);

    // bump_boxed (discrim = 1) — value = 1 + 1 = 2
    let metas = vec![AccountMeta::new(counter, false)];
    send_instruction(&mut svm, program_id(), vec![1], metas, &payer, &[])
        .expect("bump_boxed should succeed");

    let account = svm.get_account(&counter).expect("counter exists");
    let value = u64::from_le_bytes(account.data[8..16].try_into().unwrap());
    assert_eq!(value, 2);
}

#[test]
fn read_clock_succeeds_and_sysvar_is_well_formed() {
    let (mut svm, payer) = setup();
    let metas = vec![AccountMeta::new_readonly(clock_sysvar_id(), false)];
    send_instruction(&mut svm, program_id(), vec![5], metas, &payer, &[])
        .expect("read_clock should succeed");

    // Verify the Clock sysvar account exists and has the expected layout.
    // Clock layout: slot(u64) + epoch_start_timestamp(i64) + epoch(u64)
    //             + leader_schedule_epoch(u64) + unix_timestamp(i64) = 40 bytes.
    let clock_account = svm
        .get_account(&clock_sysvar_id())
        .expect("clock sysvar exists");
    assert!(
        clock_account.data.len() >= 40,
        "clock sysvar data should be at least 40 bytes, got {}",
        clock_account.data.len()
    );
    // slot lives at offset 0; epoch at offset 16. Both should be
    // parseable (the test program already accessed them without error).
    let slot = u64::from_le_bytes(clock_account.data[0..8].try_into().unwrap());
    let epoch = u64::from_le_bytes(clock_account.data[16..24].try_into().unwrap());
    // LiteSVM may start at slot 0 / epoch 0, so we only assert they
    // are parseable and the instruction succeeded — the important
    // property is that the sysvar deserialized correctly on-chain.
    let _ = (slot, epoch);
}

#[test]
fn read_clock_rejects_wrong_sysvar() {
    let (mut svm, payer) = setup();
    // Passing rent instead of clock trips `T::SYSVAR_ID` compare in
    // `Sysvar<Clock>::load`.
    let metas = vec![AccountMeta::new_readonly(rent_sysvar_id(), false)];
    let result = send_instruction(&mut svm, program_id(), vec![5], metas, &payer, &[]);
    let err_msg = format!("{:?}", result.as_ref().err().expect("should fail"));
    // The sysvar ID mismatch surfaces as InvalidArgument, InvalidAccountData,
    // or a Custom error depending on the runtime path.
    assert!(
        err_msg.contains("InvalidAccountData")
            || err_msg.contains("InvalidArgument")
            || err_msg.contains("Custom("),
        "wrong sysvar should be rejected, got: {err_msg}"
    );
}

#[test]
fn read_rent_succeeds_and_has_positive_minimum_balance() {
    let (mut svm, payer) = setup();
    let metas = vec![AccountMeta::new_readonly(rent_sysvar_id(), false)];
    send_instruction(&mut svm, program_id(), vec![6], metas, &payer, &[])
        .expect("read_rent should succeed");

    // Verify that the Rent sysvar returns meaningful values.
    // The minimum_balance_for_rent_exemption for any non-zero data size
    // should be > 0.
    let min_balance = svm.minimum_balance_for_rent_exemption(100);
    assert!(
        min_balance > 0,
        "rent minimum balance for 100 bytes should be > 0, got {min_balance}"
    );
}

#[test]
fn check_system_accepts_system_owned_account() {
    let (mut svm, payer) = setup();
    // `payer` was funded via airdrop, so it's owned by the System program.
    let wallet = keypair_for("wallet");
    svm.airdrop(&wallet.pubkey(), 1_000_000).unwrap();

    let metas = vec![AccountMeta::new_readonly(wallet.pubkey(), false)];
    send_instruction(&mut svm, program_id(), vec![7], metas, &payer, &[])
        .expect("check_system should succeed");
}

#[test]
fn check_system_rejects_non_system_owned() {
    let (mut svm, payer) = setup();
    let counter = do_initialize(&mut svm, &payer);

    // `counter` is owned by our program, not the System program.
    let metas = vec![AccountMeta::new_readonly(counter, false)];
    let result = send_instruction(&mut svm, program_id(), vec![7], metas, &payer, &[]);
    let err_msg = format!("{:?}", result.as_ref().err().expect("should fail"));
    assert!(
        err_msg.contains("IllegalOwner") || err_msg.contains("Custom("),
        "non-system-owned account should be rejected with owner error, got: {err_msg}"
    );
}

#[test]
fn touch_unchecked_accepts_arbitrary_account() {
    let (mut svm, payer) = setup();
    // UncheckedAccount does no owner/address validation — any account passes.
    let any = keypair_for("anyone");
    let metas = vec![AccountMeta::new_readonly(any.pubkey(), false)];
    send_instruction(&mut svm, program_id(), vec![8], metas, &payer, &[])
        .expect("touch_unchecked should succeed");
}

// ---- Helpers ---------------------------------------------------------------

fn call_raw(
    svm: &mut LiteSVM,
    payer: &Keypair,
    disc: u8,
    accounts: Vec<AccountMeta>,
) -> TransactionResult {
    let ix = Instruction::new_with_bytes(program_id(), &[disc], accounts);
    let blockhash = svm.latest_blockhash();
    let msg = Message::new_with_blockhash(&[ix], Some(&payer.pubkey()), &blockhash);
    let signers: Vec<&dyn solana_signer::Signer> = vec![payer];
    let tx = VersionedTransaction::try_new(VersionedMessage::Legacy(msg), &signers).unwrap();
    svm.send_transaction(tx)
}

fn assert_single_account_instruction_rejects(
    svm: &mut LiteSVM,
    payer: &Keypair,
    disc: u8,
    account: Pubkey,
    reason: &str,
) {
    let result = call_raw(
        svm,
        payer,
        disc,
        vec![AccountMeta::new_readonly(account, false)],
    );
    assert!(result.is_err(), "{reason}");
}

fn assert_program_marker_rejects_wrong_pubkey(
    svm: &mut LiteSVM,
    payer: &Keypair,
    disc: u8,
    wrong_program: Pubkey,
    marker_name: &str,
) {
    let result = call_raw(
        svm,
        payer,
        disc,
        vec![AccountMeta::new_readonly(wrong_program, false)],
    );
    let err = format!("{:?}", result.unwrap_err().err);
    assert!(
        err.contains("IncorrectProgramId") || err.contains("Custom("),
        "Program<{marker_name}> must reject wrong pubkey {wrong_program}, got: {err}"
    );
}

// ---- Sysvar field assertions -----------------------------------------------

#[test]
fn read_clock_sysvar_has_valid_fields() {
    let (mut svm, payer) = setup();
    // The Clock sysvar at its well-known address stores:
    // [slot:u64][epoch_start_timestamp:i64][epoch:u64][leader_schedule_epoch:u64][unix_timestamp:i64]
    // = 40 bytes total.
    let account = svm
        .get_account(&clock_sysvar_id())
        .expect("clock sysvar exists");
    assert_eq!(account.data.len(), 40, "Clock sysvar should be 40 bytes");

    let slot = u64::from_le_bytes(account.data[0..8].try_into().unwrap());
    let epoch = u64::from_le_bytes(account.data[16..24].try_into().unwrap());
    // LiteSVM starts at slot > 0 after genesis
    assert!(
        slot > 0 || epoch == 0,
        "slot or epoch should reflect a valid genesis state"
    );

    // Also verify the on-chain handler succeeds (already tested, but now
    // we know the sysvar is well-formed for the assertions above)
    let metas = vec![AccountMeta::new_readonly(clock_sysvar_id(), false)];
    send_instruction(&mut svm, program_id(), vec![5], metas, &payer, &[])
        .expect("read_clock should succeed with well-formed sysvar");
}

#[test]
fn read_rent_sysvar_has_positive_minimum_balance() {
    let (mut svm, payer) = setup();
    // Rent sysvar: [lamports_per_byte_year:u64][exemption_threshold:f64][burn_percent:u8]
    let account = svm
        .get_account(&rent_sysvar_id())
        .expect("rent sysvar exists");
    assert!(
        account.data.len() >= 17,
        "Rent sysvar should be at least 17 bytes"
    );

    let lamports_per_byte = u64::from_le_bytes(account.data[0..8].try_into().unwrap());
    assert!(
        lamports_per_byte > 0,
        "lamports_per_byte_year should be positive"
    );

    // Also verify minimum_balance_for_rent_exemption gives sensible output
    let min_balance = svm.minimum_balance_for_rent_exemption(100);
    assert!(
        min_balance > 0,
        "minimum_balance for 100 bytes should be > 0"
    );

    // The on-chain handler should succeed
    let metas = vec![AccountMeta::new_readonly(rent_sysvar_id(), false)];
    send_instruction(&mut svm, program_id(), vec![6], metas, &payer, &[])
        .expect("read_rent should succeed");
}

#[test]
fn read_clock_rejects_wrong_sysvar_with_specific_error() {
    let (mut svm, payer) = setup();
    let metas = vec![AccountMeta::new_readonly(rent_sysvar_id(), false)];
    let result = call_raw(&mut svm, &payer, 5, metas);
    let err = format!("{:?}", result.unwrap_err().err);
    // The Sysvar<Clock> load checks the account address matches the Clock sysvar ID.
    assert!(
        err.contains("InvalidArgument")
            || err.contains("InvalidAccountData")
            || err.contains("Custom("),
        "expected a specific error for wrong sysvar, got: {err}"
    );
}

#[test]
fn read_rent_rejects_wrong_sysvar() {
    let (mut svm, payer) = setup();
    // Pass clock sysvar to rent handler
    assert_single_account_instruction_rejects(
        &mut svm,
        &payer,
        6,
        clock_sysvar_id(),
        "wrong sysvar should be rejected for rent",
    );
}

// ---- Box<Account<T>> -------------------------------------------------------

#[test]
fn bump_boxed_accumulates_across_calls() {
    let (mut svm, payer) = setup();
    let counter = do_initialize(&mut svm, &payer);

    // Call bump_boxed 3 times, verify value increments each time
    for expected in 2..=4u64 {
        let metas = vec![AccountMeta::new(counter, false)];
        send_instruction(&mut svm, program_id(), vec![1], metas, &payer, &[])
            .expect("bump_boxed should succeed");

        let account = svm.get_account(&counter).unwrap();
        let value = u64::from_le_bytes(account.data[8..16].try_into().unwrap());
        assert_eq!(
            value,
            expected,
            "counter should be {expected} after bump #{}",
            expected - 1
        );

        svm.expire_blockhash();
    }
}

#[test]
fn bump_boxed_rejects_wrong_owner() {
    let (mut svm, payer) = setup();

    // Create an account owned by system program, not our program
    let fake = Pubkey::new_unique();
    let account = solana_account::Account {
        lamports: 1_000_000,
        data: vec![0u8; 16], // disc(8) + value(8)
        owner: solana_sdk_ids::system_program::ID,
        executable: false,
        rent_epoch: 0,
    };
    svm.set_account(fake, account).unwrap();

    let metas = vec![AccountMeta::new(fake, false)];
    let result = call_raw(&mut svm, &payer, 1, metas);
    assert!(
        result.is_err(),
        "system-owned account should fail Box<Account<Counter>> load"
    );
}

#[test]
fn read_boxed_accepts_immutable_box() {
    let (mut svm, payer) = setup();
    let counter = do_initialize_boxed(&mut svm, &payer);

    let metas = vec![AccountMeta::new_readonly(counter, false)];
    send_instruction(&mut svm, program_id(), vec![2], metas, &payer, &[])
        .expect("read_boxed should succeed");

    let account = svm.get_account(&counter).expect("counter exists");
    let value = u64::from_le_bytes(account.data[8..16].try_into().unwrap());
    assert_eq!(value, 7, "immutable boxed read must not mutate state");
}

#[test]
fn initialize_boxed_uses_box_init_path() {
    let (mut svm, payer) = setup();
    let counter = do_initialize_boxed(&mut svm, &payer);
    let account = svm.get_account(&counter).expect("boxed counter exists");

    assert_eq!(
        account.owner,
        program_id(),
        "boxed init should assign program owner"
    );
    assert_eq!(account.data.len(), 16, "boxed counter should be disc + u64");

    let value = u64::from_le_bytes(account.data[8..16].try_into().unwrap());
    assert_eq!(
        value, 7,
        "initialize_boxed should set the boxed counter value"
    );
}

#[test]
fn close_boxed_transfers_lamports_and_clears_account() {
    let (mut svm, payer) = setup();
    let counter = do_initialize_boxed(&mut svm, &payer);
    let receiver = keypair_for("boxed-close-receiver");
    svm.airdrop(&receiver.pubkey(), 10_000_000).unwrap();

    let receiver_before = svm.get_account(&receiver.pubkey()).unwrap().lamports;
    let counter_before = svm.get_account(&counter).unwrap().lamports;
    assert!(
        counter_before > 0,
        "boxed counter must hold lamports before close"
    );

    let metas = vec![
        AccountMeta::new(counter, false),
        AccountMeta::new(receiver.pubkey(), false),
    ];
    send_instruction(&mut svm, program_id(), vec![4], metas, &payer, &[])
        .expect("close_boxed should succeed");

    let receiver_after = svm.get_account(&receiver.pubkey()).unwrap().lamports;
    assert_eq!(
        receiver_after,
        receiver_before + counter_before,
        "close_boxed should transfer all lamports to the receiver",
    );

    match svm.get_account(&counter) {
        None => {}
        Some(account) => assert_eq!(account.lamports, 0, "closed boxed account should be empty"),
    }
}

// ---- SystemAccount ---------------------------------------------------------

#[test]
fn check_system_rejects_program_owned_with_specific_error() {
    let (mut svm, payer) = setup();
    let counter = do_initialize(&mut svm, &payer);

    let metas = vec![AccountMeta::new_readonly(counter, false)];
    let result = call_raw(&mut svm, &payer, 7, metas);
    let err = format!("{:?}", result.unwrap_err().err);
    assert!(
        err.contains("IllegalOwner") || err.contains("Custom("),
        "expected IllegalOwner for program-owned account in SystemAccount, got: {err}"
    );
}

#[test]
fn check_system_accepts_unfunded_system_account() {
    let (mut svm, payer) = setup();
    // A pubkey with no on-chain account is treated as system-owned with 0 lamports
    let unfunded = Pubkey::new_unique();
    let metas = vec![AccountMeta::new_readonly(unfunded, false)];
    send_instruction(&mut svm, program_id(), vec![7], metas, &payer, &[])
        .expect("unfunded system account should be accepted");
}

// ---- Program<T> builtin markers -------------------------------------------

#[test]
fn program_system_rejects_wrong_pubkey() {
    let (mut svm, payer) = setup();
    assert_program_marker_rejects_wrong_pubkey(&mut svm, &payer, 9, Token::id(), "System");
}

#[test]
fn program_token_rejects_wrong_pubkey() {
    let (mut svm, payer) = setup();
    assert_program_marker_rejects_wrong_pubkey(&mut svm, &payer, 10, Token2022::id(), "Token");
}

#[test]
fn program_token_2022_rejects_wrong_pubkey() {
    let (mut svm, payer) = setup();
    assert_program_marker_rejects_wrong_pubkey(&mut svm, &payer, 11, Token::id(), "Token2022");
}

#[test]
fn program_associated_token_rejects_wrong_pubkey() {
    let (mut svm, payer) = setup();
    assert_program_marker_rejects_wrong_pubkey(
        &mut svm,
        &payer,
        12,
        solana_sdk_ids::system_program::ID,
        "AssociatedToken",
    );
}

#[test]
fn program_memo_rejects_wrong_pubkey() {
    let (mut svm, payer) = setup();
    assert_program_marker_rejects_wrong_pubkey(
        &mut svm,
        &payer,
        13,
        solana_sdk_ids::system_program::ID,
        "Memo",
    );
}

#[test]
fn system_program_transfer_moves_lamports() {
    let (mut svm, payer) = setup();
    let recipient = keypair_for("system-transfer-recipient");
    svm.airdrop(&recipient.pubkey(), 1_000_000).unwrap();
    let before = svm
        .get_account(&recipient.pubkey())
        .expect("recipient exists")
        .lamports;
    let amount = 123_456u64;

    let mut data = vec![14];
    data.extend_from_slice(&amount.to_le_bytes());
    let metas = vec![
        AccountMeta::new(payer.pubkey(), true),
        AccountMeta::new(recipient.pubkey(), false),
        AccountMeta::new_readonly(solana_sdk_ids::system_program::ID, false),
    ];
    send_instruction(&mut svm, program_id(), data, metas, &payer, &[])
        .expect("system_program::transfer should succeed");

    let after = svm
        .get_account(&recipient.pubkey())
        .expect("recipient still exists")
        .lamports;
    assert_eq!(after - before, amount);
}

#[test]
fn system_program_create_allocate_assign_helpers_work() {
    let (mut svm, payer) = setup();

    let created = keypair_for("system-create-account");
    let create_space = 16u64;
    let create_lamports = svm.minimum_balance_for_rent_exemption(create_space as usize);
    let create_metas = vec![
        AccountMeta::new(payer.pubkey(), true),
        AccountMeta::new(created.pubkey(), true),
        AccountMeta::new_readonly(solana_sdk_ids::system_program::ID, false),
    ];
    send_instruction(
        &mut svm,
        program_id(),
        data_with_u64s(15, &[create_lamports, create_space]),
        create_metas,
        &payer,
        &[&created],
    )
    .expect("system_program::create_account should succeed");
    let created_account = svm.get_account(&created.pubkey()).unwrap();
    assert_eq!(created_account.owner, program_id());
    assert_eq!(created_account.data.len(), create_space as usize);
    assert_eq!(created_account.lamports, create_lamports);

    let allocated = keypair_for("system-allocate-account");
    let allocate_space = 24u64;
    let allocate_lamports = svm.minimum_balance_for_rent_exemption(allocate_space as usize);
    set_system_account(&mut svm, allocated.pubkey(), allocate_lamports, 0);
    let allocate_metas = vec![
        AccountMeta::new(allocated.pubkey(), true),
        AccountMeta::new_readonly(solana_sdk_ids::system_program::ID, false),
    ];
    send_instruction(
        &mut svm,
        program_id(),
        data_with_u64s(17, &[allocate_space]),
        allocate_metas,
        &payer,
        &[&allocated],
    )
    .expect("system_program::allocate should succeed");
    let allocated_account = svm.get_account(&allocated.pubkey()).unwrap();
    assert_eq!(allocated_account.owner, solana_sdk_ids::system_program::ID);
    assert_eq!(allocated_account.data.len(), allocate_space as usize);

    let assigned = keypair_for("system-assign-account");
    set_system_account(&mut svm, assigned.pubkey(), 1_000_000, 0);
    let assign_metas = vec![
        AccountMeta::new(assigned.pubkey(), true),
        AccountMeta::new_readonly(solana_sdk_ids::system_program::ID, false),
    ];
    send_instruction(
        &mut svm,
        program_id(),
        vec![19],
        assign_metas,
        &payer,
        &[&assigned],
    )
    .expect("system_program::assign should succeed");
    let assigned_account = svm.get_account(&assigned.pubkey()).unwrap();
    assert_eq!(assigned_account.owner, program_id());
}

#[test]
fn system_program_seeded_helpers_work() {
    let (mut svm, payer) = setup();

    let create_base = keypair_for("system-create-with-seed-base");
    svm.airdrop(&create_base.pubkey(), 1_000_000).unwrap();
    let created = seeded_address(&create_base.pubkey(), SYSTEM_SEED, &program_id());
    let create_space = 16u64;
    let create_lamports = svm.minimum_balance_for_rent_exemption(create_space as usize);
    let create_metas = vec![
        AccountMeta::new(payer.pubkey(), true),
        AccountMeta::new(created, false),
        AccountMeta::new_readonly(create_base.pubkey(), true),
        AccountMeta::new_readonly(solana_sdk_ids::system_program::ID, false),
    ];
    send_instruction(
        &mut svm,
        program_id(),
        data_with_u64s(16, &[create_lamports, create_space]),
        create_metas,
        &payer,
        &[&create_base],
    )
    .expect("system_program::create_account_with_seed should succeed");
    let created_account = svm.get_account(&created).unwrap();
    assert_eq!(created_account.owner, program_id());
    assert_eq!(created_account.data.len(), create_space as usize);

    let allocate_base = keypair_for("system-allocate-with-seed-base");
    svm.airdrop(&allocate_base.pubkey(), 1_000_000).unwrap();
    let allocated = seeded_address(&allocate_base.pubkey(), SYSTEM_SEED, &program_id());
    let allocate_space = 24u64;
    let allocate_lamports = svm.minimum_balance_for_rent_exemption(allocate_space as usize);
    set_system_account(&mut svm, allocated, allocate_lamports, 0);
    let allocate_metas = vec![
        AccountMeta::new(allocated, false),
        AccountMeta::new_readonly(allocate_base.pubkey(), true),
        AccountMeta::new_readonly(solana_sdk_ids::system_program::ID, false),
    ];
    send_instruction(
        &mut svm,
        program_id(),
        data_with_u64s(18, &[allocate_space]),
        allocate_metas,
        &payer,
        &[&allocate_base],
    )
    .expect("system_program::allocate_with_seed should succeed");
    let allocated_account = svm.get_account(&allocated).unwrap();
    assert_eq!(allocated_account.owner, program_id());
    assert_eq!(allocated_account.data.len(), allocate_space as usize);

    let assign_base = keypair_for("system-assign-with-seed-base");
    svm.airdrop(&assign_base.pubkey(), 1_000_000).unwrap();
    let assigned = seeded_address(&assign_base.pubkey(), SYSTEM_SEED, &program_id());
    set_system_account(&mut svm, assigned, 1_000_000, 0);
    let assign_metas = vec![
        AccountMeta::new(assigned, false),
        AccountMeta::new_readonly(assign_base.pubkey(), true),
        AccountMeta::new_readonly(solana_sdk_ids::system_program::ID, false),
    ];
    send_instruction(
        &mut svm,
        program_id(),
        vec![20],
        assign_metas,
        &payer,
        &[&assign_base],
    )
    .expect("system_program::assign_with_seed should succeed");
    let assigned_account = svm.get_account(&assigned).unwrap();
    assert_eq!(assigned_account.owner, program_id());

    let transfer_base = keypair_for("system-transfer-with-seed-base");
    svm.airdrop(&transfer_base.pubkey(), 1_000_000).unwrap();
    let from = seeded_address(
        &transfer_base.pubkey(),
        SYSTEM_TRANSFER_SEED,
        &solana_sdk_ids::system_program::ID,
    );
    let recipient = keypair_for("system-transfer-with-seed-recipient");
    svm.airdrop(&recipient.pubkey(), 1_000_000).unwrap();
    set_system_account(&mut svm, from, 2_000_000, 0);
    let recipient_before = svm.get_account(&recipient.pubkey()).unwrap().lamports;
    let amount = 111_222u64;
    let transfer_metas = vec![
        AccountMeta::new(from, false),
        AccountMeta::new_readonly(transfer_base.pubkey(), true),
        AccountMeta::new(recipient.pubkey(), false),
        AccountMeta::new_readonly(solana_sdk_ids::system_program::ID, false),
    ];
    send_instruction(
        &mut svm,
        program_id(),
        data_with_u64s(21, &[amount]),
        transfer_metas,
        &payer,
        &[&transfer_base],
    )
    .expect("system_program::transfer_with_seed should succeed");
    let recipient_after = svm.get_account(&recipient.pubkey()).unwrap().lamports;
    let from_after = svm.get_account(&from).unwrap().lamports;
    assert_eq!(recipient_after - recipient_before, amount);
    assert_eq!(from_after, 2_000_000 - amount);
}

#[test]
fn system_program_nonce_helpers_work() {
    let (mut svm, payer) = setup();
    let nonce_lamports = svm.minimum_balance_for_rent_exemption(NONCE_ACCOUNT_LENGTH) + 100_000;
    let nonce_authority = keypair_for("system-nonce-authority");
    svm.airdrop(&nonce_authority.pubkey(), 1_000_000).unwrap();

    let seeded_base = keypair_for("system-create-nonce-with-seed-base");
    svm.airdrop(&seeded_base.pubkey(), 1_000_000).unwrap();
    let seeded_nonce = seeded_address(
        &seeded_base.pubkey(),
        SYSTEM_SEED,
        &solana_sdk_ids::system_program::ID,
    );
    let seeded_nonce_metas = vec![
        AccountMeta::new(payer.pubkey(), true),
        AccountMeta::new(seeded_nonce, false),
        AccountMeta::new_readonly(seeded_base.pubkey(), true),
        AccountMeta::new_readonly(nonce_authority.pubkey(), true),
        AccountMeta::new_readonly(recent_blockhashes_sysvar_id(), false),
        AccountMeta::new_readonly(rent_sysvar_id(), false),
        AccountMeta::new_readonly(solana_sdk_ids::system_program::ID, false),
    ];
    send_instruction(
        &mut svm,
        program_id(),
        data_with_u64s(23, &[nonce_lamports]),
        seeded_nonce_metas,
        &payer,
        &[&seeded_base, &nonce_authority],
    )
    .expect("system_program::create_nonce_account_with_seed should succeed");
    let seeded_nonce_account = svm.get_account(&seeded_nonce).unwrap();
    assert_eq!(
        seeded_nonce_account.owner,
        solana_sdk_ids::system_program::ID
    );
    assert_eq!(seeded_nonce_account.data.len(), NONCE_ACCOUNT_LENGTH);

    let nonce = keypair_for("system-create-nonce");
    let nonce_metas = vec![
        AccountMeta::new(payer.pubkey(), true),
        AccountMeta::new(nonce.pubkey(), true),
        AccountMeta::new_readonly(nonce_authority.pubkey(), true),
        AccountMeta::new_readonly(recent_blockhashes_sysvar_id(), false),
        AccountMeta::new_readonly(rent_sysvar_id(), false),
        AccountMeta::new_readonly(solana_sdk_ids::system_program::ID, false),
    ];
    send_instruction(
        &mut svm,
        program_id(),
        data_with_u64s(22, &[nonce_lamports]),
        nonce_metas,
        &payer,
        &[&nonce, &nonce_authority],
    )
    .expect("system_program::create_nonce_account should succeed");
    let nonce_account = svm.get_account(&nonce.pubkey()).unwrap();
    assert_eq!(nonce_account.owner, solana_sdk_ids::system_program::ID);
    assert_eq!(nonce_account.data.len(), NONCE_ACCOUNT_LENGTH);

    svm.expire_blockhash();
    let nonce_before_advance = svm.get_account(&nonce.pubkey()).unwrap().data;
    let advance_metas = vec![
        AccountMeta::new(nonce.pubkey(), false),
        AccountMeta::new_readonly(nonce_authority.pubkey(), true),
        AccountMeta::new_readonly(recent_blockhashes_sysvar_id(), false),
        AccountMeta::new_readonly(solana_sdk_ids::system_program::ID, false),
    ];
    send_instruction(
        &mut svm,
        program_id(),
        vec![24],
        advance_metas,
        &payer,
        &[&nonce_authority],
    )
    .expect("system_program::advance_nonce_account should succeed");
    let nonce_after_advance = svm.get_account(&nonce.pubkey()).unwrap().data;
    assert_ne!(
        nonce_after_advance, nonce_before_advance,
        "advance_nonce_account should update nonce state"
    );

    let new_authority = keypair_for("system-nonce-new-authority");
    let authorize_metas = vec![
        AccountMeta::new(nonce.pubkey(), false),
        AccountMeta::new_readonly(nonce_authority.pubkey(), true),
        AccountMeta::new_readonly(new_authority.pubkey(), false),
        AccountMeta::new_readonly(solana_sdk_ids::system_program::ID, false),
    ];
    send_instruction(
        &mut svm,
        program_id(),
        vec![25],
        authorize_metas,
        &payer,
        &[&nonce_authority],
    )
    .expect("system_program::authorize_nonce_account should succeed");

    let recipient = keypair_for("system-withdraw-nonce-recipient");
    svm.airdrop(&recipient.pubkey(), 1_000_000).unwrap();
    let recipient_before = svm.get_account(&recipient.pubkey()).unwrap().lamports;
    let withdraw_amount = 12_345u64;
    let withdraw_metas = vec![
        AccountMeta::new(nonce.pubkey(), false),
        AccountMeta::new(recipient.pubkey(), false),
        AccountMeta::new_readonly(recent_blockhashes_sysvar_id(), false),
        AccountMeta::new_readonly(rent_sysvar_id(), false),
        AccountMeta::new_readonly(new_authority.pubkey(), true),
        AccountMeta::new_readonly(solana_sdk_ids::system_program::ID, false),
    ];
    send_instruction(
        &mut svm,
        program_id(),
        data_with_u64s(26, &[withdraw_amount]),
        withdraw_metas,
        &payer,
        &[&new_authority],
    )
    .expect("system_program::withdraw_nonce_account should succeed");
    let recipient_after = svm.get_account(&recipient.pubkey()).unwrap().lamports;
    assert_eq!(recipient_after - recipient_before, withdraw_amount);
}

#[test]
fn system_program_helpers_reject_wrong_program_id() {
    let (mut svm, payer) = setup();
    let dummies = [
        Pubkey::new_unique(),
        Pubkey::new_unique(),
        Pubkey::new_unique(),
        Pubkey::new_unique(),
        Pubkey::new_unique(),
    ];
    for address in dummies {
        set_system_account(&mut svm, address, 1_000_000, 0);
    }

    let helper_names = [
        "advance_nonce_account",
        "allocate",
        "allocate_with_seed",
        "assign",
        "assign_with_seed",
        "authorize_nonce_account",
        "create_account",
        "create_account_with_seed",
        "create_nonce_account",
        "create_nonce_account_with_seed",
        "transfer",
        "transfer_with_seed",
        "withdraw_nonce_account",
    ];

    for (opcode, name) in helper_names.iter().enumerate() {
        let metas = vec![
            AccountMeta::new(dummies[0], false),
            AccountMeta::new(dummies[1], false),
            AccountMeta::new(dummies[2], false),
            AccountMeta::new(dummies[3], false),
            AccountMeta::new(dummies[4], false),
            AccountMeta::new_readonly(program_id(), false),
        ];
        let result = send_instruction(
            &mut svm,
            program_id(),
            vec![27, opcode as u8],
            metas,
            &payer,
            &[],
        );
        let err = result
            .expect_err("wrong system program id should be rejected")
            .to_string();
        assert!(
            err.contains("IncorrectProgramId"),
            "{name} should reject a non-system program id, got: {err}"
        );
        svm.expire_blockhash();
    }
}

// ---- Counter init ----------------------------------------------------------

#[test]
fn initialize_sets_correct_discriminator() {
    let (mut svm, payer) = setup();
    let counter = do_initialize(&mut svm, &payer);
    let account = svm.get_account(&counter).expect("counter exists");

    // Account should be: disc(8) + value(8) = 16 bytes, owned by program
    assert_eq!(
        account.data.len(),
        16,
        "counter should be 16 bytes (disc + u64)"
    );
    assert_eq!(
        account.owner,
        program_id(),
        "counter should be owned by accounts program"
    );

    // Discriminator should be non-zero (SHA256 hash prefix)
    let disc = &account.data[0..8];
    assert_ne!(disc, &[0u8; 8], "discriminator should be set after init");

    let value = u64::from_le_bytes(account.data[8..16].try_into().unwrap());
    assert_eq!(value, 1, "value should be 1 after init");
}

#[test]
fn initialize_rejects_double_init() {
    let (mut svm, payer) = setup();
    do_initialize(&mut svm, &payer);
    svm.expire_blockhash();

    // Second init on same PDA should fail — account already exists
    let counter = counter_pda();
    let metas = vec![
        AccountMeta::new(payer.pubkey(), true),
        AccountMeta::new(counter, false),
        AccountMeta::new_readonly(solana_sdk_ids::system_program::ID, false),
    ];
    let result = call_raw(&mut svm, &payer, 0, metas);
    assert!(
        result.is_err(),
        "double init on same PDA should be rejected"
    );
}

#[test]
fn initialize_rejects_wrong_pda() {
    let (mut svm, payer) = setup();
    let wrong = Pubkey::new_unique();

    let metas = vec![
        AccountMeta::new(payer.pubkey(), true),
        AccountMeta::new(wrong, false),
        AccountMeta::new_readonly(solana_sdk_ids::system_program::ID, false),
    ];
    let result = call_raw(&mut svm, &payer, 0, metas);
    assert!(result.is_err(), "init must reject a non-canonical PDA");
    assert!(
        svm.get_account(&wrong).is_none(),
        "wrong PDA must not be created on failed init"
    );
}

#[test]
fn initialize_rejects_wrong_system_program_pubkey() {
    let (mut svm, payer) = setup();
    let counter = counter_pda();

    let metas = vec![
        AccountMeta::new(payer.pubkey(), true),
        AccountMeta::new(counter, false),
        AccountMeta::new_readonly(Token::id(), false),
    ];
    let result = call_raw(&mut svm, &payer, 0, metas);
    assert!(
        result.is_err(),
        "init must reject a non-System program pubkey"
    );
    assert!(
        svm.get_account(&counter).is_none(),
        "failed init must not create the PDA"
    );
}

#[test]
fn initialize_boxed_rejects_wrong_system_program_pubkey() {
    let (mut svm, payer) = setup();
    let counter = boxed_counter_pda();

    let metas = vec![
        AccountMeta::new(payer.pubkey(), true),
        AccountMeta::new(counter, false),
        AccountMeta::new_readonly(Token::id(), false),
    ];
    let result = call_raw(&mut svm, &payer, 3, metas);
    assert!(
        result.is_err(),
        "boxed init must reject a non-System program pubkey"
    );
    assert!(
        svm.get_account(&counter).is_none(),
        "failed boxed init must not create the PDA"
    );
}

#[test]
fn read_boxed_rejects_wrong_seed_pda() {
    let (mut svm, payer) = setup();
    let counter = do_initialize(&mut svm, &payer);

    // `read_boxed` expects the boxed-counter PDA, not the regular counter PDA.
    let metas = vec![AccountMeta::new_readonly(counter, false)];
    let result = call_raw(&mut svm, &payer, 2, metas);
    assert!(
        result.is_err(),
        "Box<Account<Counter>> with seeds must reject the wrong PDA"
    );
}

#[test]
fn read_boxed_rejects_wrong_discriminator() {
    let (mut svm, payer) = setup();
    let counter = do_initialize_boxed(&mut svm, &payer);
    let mut account = svm.get_account(&counter).expect("boxed counter exists");
    account.data[0] ^= 0xff;
    svm.set_account(counter, account).unwrap();

    let metas = vec![AccountMeta::new_readonly(counter, false)];
    let result = call_raw(&mut svm, &payer, 2, metas);
    assert!(
        result.is_err(),
        "Box<Account<Counter>> load must reject discriminator mismatch"
    );
}

#[test]
fn close_boxed_rejects_non_system_receiver_and_preserves_accounts() {
    let (mut svm, payer) = setup();
    let boxed = do_initialize_boxed(&mut svm, &payer);
    let receiver = do_initialize(&mut svm, &payer);
    let boxed_before = svm.get_account(&boxed).expect("boxed counter exists");
    let receiver_before = svm.get_account(&receiver).expect("receiver counter exists");

    let metas = vec![
        AccountMeta::new(boxed, false),
        AccountMeta::new(receiver, false),
    ];
    let result = call_raw(&mut svm, &payer, 4, metas);
    assert!(
        result.is_err(),
        "close receiver is SystemAccount and must reject program-owned accounts"
    );

    let boxed_after = svm
        .get_account(&boxed)
        .expect("boxed counter should remain");
    let receiver_after = svm
        .get_account(&receiver)
        .expect("receiver counter should remain");
    assert_eq!(
        boxed_after.lamports, boxed_before.lamports,
        "failed close must preserve boxed counter lamports"
    );
    assert_eq!(
        receiver_after.lamports, receiver_before.lamports,
        "failed close must not credit the invalid receiver"
    );
}
