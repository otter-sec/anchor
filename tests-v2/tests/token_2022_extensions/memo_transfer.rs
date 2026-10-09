use {
    super::common::*,
    solana_pubkey::Pubkey,
    solana_signer::Signer,
    spl_token_2022_interface::{
        extension::{memo_transfer::MemoTransfer, StateWithExtensionsMut},
        state::Account as Token2022Account,
    },
};

#[test]
fn initializes_and_disables_memo_transfer_requirement() {
    let (mut svm, payer, id) = setup(
        "memo-transfer",
        "token_2022_ext_memo_transfer.so",
        "6wc58Q2xzU5Lw21XrsJXy31LJbcoTcGbEa3knKj7enwM",
    );
    let account = Pubkey::new_unique();
    let mint = Pubkey::new_unique();
    let owner = tests_v2::keypair_for("token-2022-ext-memo-owner");
    seed_initialized_memo_transfer_account(&mut svm, account, mint, owner.pubkey());
    svm.airdrop(&owner.pubkey(), 1_000_000_000).unwrap();

    let enable_metas = vec![
        Meta::new(account, false),
        Meta::new_readonly(owner.pubkey(), true),
        Meta::new_readonly(token_2022_program_id(), false),
    ];
    let metadata = send(&mut svm, id, vec![0], enable_metas, &payer, &[&owner])
        .expect("memo transfer initialize should invoke real Token-2022");
    assert_token_2022_cpi_succeeded(&metadata, "memo transfer initialize");
    assert_memo_required(&svm, account, true);

    let disable_metas = vec![
        Meta::new(account, false),
        Meta::new_readonly(owner.pubkey(), true),
        Meta::new_readonly(token_2022_program_id(), false),
    ];
    let metadata = send(&mut svm, id, vec![1], disable_metas, &payer, &[&owner])
        .expect("memo transfer disable should invoke real Token-2022");
    assert_token_2022_cpi_succeeded(&metadata, "memo transfer disable");
    assert_memo_required(&svm, account, false);

    let bad_metas = vec![
        Meta::new(account, false),
        Meta::new_readonly(owner.pubkey(), true),
        Meta::new_readonly(wrong_token_program_id(), false),
    ];
    expect_incorrect_program(
        send(&mut svm, id, vec![1], bad_metas, &payer, &[&owner]),
        "memo transfer disable should reject non-Token-2022 program",
    );
}

fn assert_memo_required(
    svm: &anchor_v2_testing::litesvm::LiteSVM,
    account: Pubkey,
    expected: bool,
) {
    let mut data = svm.get_account(&account).expect("token exists").data;
    let state =
        StateWithExtensionsMut::<Token2022Account>::unpack(&mut data).expect("unpack token");
    let extension = state
        .get_extension::<MemoTransfer>()
        .expect("memo transfer extension exists");
    assert_eq!(
        bool::from(extension.require_incoming_transfer_memos),
        expected
    );
}
