use {
    super::common::*,
    solana_pubkey::Pubkey,
    solana_signer::Signer,
    spl_token_2022_interface::{
        extension::{
            transfer_fee::{TransferFeeAmount, TransferFeeConfig},
            ExtensionType, StateWithExtensionsMut,
        },
        state::Account as Token2022Account,
    },
};

#[test]
fn initializes_and_updates_transfer_fee_config() {
    let (mut svm, payer, id) = setup(
        "transfer-fee",
        "token_2022_ext_transfer_fee.so",
        "CvCYVXhFDScZ8CNRtm6mSU8AkZrN5tk3NcFF8Q33M45z",
    );
    let mint = Pubkey::new_unique();
    let authority = tests_v2::keypair_for("token-2022-ext-transfer-fee-authority");
    seed_mint_with_extensions(&mut svm, mint, &[ExtensionType::TransferFeeConfig]);
    svm.airdrop(&authority.pubkey(), 1_000_000_000).unwrap();

    let mut init_data = vec![0];
    init_data.extend_from_slice(&address_bytes(authority.pubkey()));
    let ok_metas = vec![
        Meta::new(mint, false),
        Meta::new_readonly(token_2022_program_id(), false),
    ];
    let metadata = send(&mut svm, id, init_data.clone(), ok_metas, &payer, &[])
        .expect("transfer fee initialize should invoke real Token-2022");
    assert_token_2022_cpi_succeeded(&metadata, "transfer fee initialize");
    assert_transfer_fee_config(
        &svm,
        mint,
        Some(authority.pubkey()),
        Some(authority.pubkey()),
        0,
        111,
        42,
    );

    mark_mint_initialized(&mut svm, mint, Pubkey::new_unique(), None);
    let set_metas = vec![
        Meta::new(mint, false),
        Meta::new_readonly(authority.pubkey(), true),
        Meta::new_readonly(token_2022_program_id(), false),
    ];
    let metadata = send(&mut svm, id, vec![1], set_metas, &payer, &[&authority])
        .expect("transfer fee set should invoke real Token-2022");
    assert_token_2022_cpi_succeeded(&metadata, "transfer fee set");
    assert_transfer_fee_config(
        &svm,
        mint,
        Some(authority.pubkey()),
        Some(authority.pubkey()),
        0,
        222,
        84,
    );

    let bad_metas = vec![
        Meta::new(mint, false),
        Meta::new_readonly(wrong_token_program_id(), false),
    ];
    expect_incorrect_program(
        send(&mut svm, id, init_data, bad_metas, &payer, &[]),
        "transfer fee initialize should reject non-Token-2022 program",
    );
}

#[test]
fn transfer_fee_helpers_move_withheld_tokens_through_real_token_2022_state() {
    let (mut svm, payer, id) = setup(
        "transfer-fee",
        "token_2022_ext_transfer_fee.so",
        "CvCYVXhFDScZ8CNRtm6mSU8AkZrN5tk3NcFF8Q33M45z",
    );
    let mint = Pubkey::new_unique();
    let source = Pubkey::new_unique();
    let destination = Pubkey::new_unique();
    let withdraw_destination = Pubkey::new_unique();
    let account_with_withheld_tokens = Pubkey::new_unique();
    let authority = tests_v2::keypair_for("token-2022-ext-transfer-fee-positive-authority");

    seed_mint_with_extensions(&mut svm, mint, &[ExtensionType::TransferFeeConfig]);
    seed_transfer_fee_token_account(&mut svm, source, mint, authority.pubkey(), 1_000, 0);
    seed_transfer_fee_token_account(&mut svm, destination, mint, authority.pubkey(), 0, 0);
    seed_transfer_fee_token_account(
        &mut svm,
        withdraw_destination,
        mint,
        authority.pubkey(),
        100,
        0,
    );
    seed_transfer_fee_token_account(
        &mut svm,
        account_with_withheld_tokens,
        mint,
        authority.pubkey(),
        250,
        9,
    );
    svm.airdrop(&authority.pubkey(), 1_000_000_000).unwrap();

    let mut init_data = vec![0];
    init_data.extend_from_slice(&address_bytes(authority.pubkey()));
    let metadata = send(
        &mut svm,
        id,
        init_data,
        vec![
            Meta::new(mint, false),
            Meta::new_readonly(token_2022_program_id(), false),
        ],
        &payer,
        &[],
    )
    .expect("transfer fee initialize should invoke real Token-2022");
    assert_token_2022_cpi_succeeded(&metadata, "transfer fee initialize");
    mark_mint_initialized(&mut svm, mint, authority.pubkey(), None);
    assert_transfer_fee_config(
        &svm,
        mint,
        Some(authority.pubkey()),
        Some(authority.pubkey()),
        0,
        111,
        42,
    );

    let transfer_metadata = send(
        &mut svm,
        id,
        vec![2],
        vec![
            Meta::new(source, false),
            Meta::new_readonly(mint, false),
            Meta::new(destination, false),
            Meta::new_readonly(authority.pubkey(), true),
            Meta::new_readonly(token_2022_program_id(), false),
        ],
        &payer,
        &[&authority],
    )
    .expect("transfer_checked_with_fee should invoke real Token-2022");
    assert_token_2022_cpi_succeeded(&transfer_metadata, "transfer checked with fee");
    assert_token_amount(&svm, source, 500);
    assert_token_amount(&svm, destination, 494);
    assert_transfer_fee_amount(&svm, destination, 6);

    let harvest_metadata = send(
        &mut svm,
        id,
        vec![3],
        vec![
            Meta::new(mint, false),
            Meta::new(destination, false),
            Meta::new_readonly(token_2022_program_id(), false),
        ],
        &payer,
        &[],
    )
    .expect("harvest_withheld_tokens_to_mint should invoke real Token-2022");
    assert_token_2022_cpi_succeeded(&harvest_metadata, "harvest withheld tokens to mint");
    assert_transfer_fee_amount(&svm, destination, 0);
    assert_transfer_fee_config(
        &svm,
        mint,
        Some(authority.pubkey()),
        Some(authority.pubkey()),
        6,
        111,
        42,
    );

    let withdraw_mint_metadata = send(
        &mut svm,
        id,
        vec![4],
        vec![
            Meta::new(mint, false),
            Meta::new(withdraw_destination, false),
            Meta::new_readonly(authority.pubkey(), true),
            Meta::new_readonly(token_2022_program_id(), false),
        ],
        &payer,
        &[&authority],
    )
    .expect("withdraw_withheld_tokens_from_mint should invoke real Token-2022");
    assert_token_2022_cpi_succeeded(
        &withdraw_mint_metadata,
        "withdraw withheld tokens from mint",
    );
    assert_token_amount(&svm, withdraw_destination, 106);
    assert_transfer_fee_config(
        &svm,
        mint,
        Some(authority.pubkey()),
        Some(authority.pubkey()),
        0,
        111,
        42,
    );

    let withdraw_accounts_metadata = send(
        &mut svm,
        id,
        vec![5],
        vec![
            Meta::new_readonly(mint, false),
            Meta::new(withdraw_destination, false),
            Meta::new_readonly(authority.pubkey(), true),
            Meta::new(account_with_withheld_tokens, false),
            Meta::new_readonly(token_2022_program_id(), false),
        ],
        &payer,
        &[&authority],
    )
    .expect("withdraw_withheld_tokens_from_accounts should invoke real Token-2022");
    assert_token_2022_cpi_succeeded(
        &withdraw_accounts_metadata,
        "withdraw withheld tokens from accounts",
    );
    assert_transfer_fee_amount(&svm, account_with_withheld_tokens, 0);
    assert_token_amount(&svm, withdraw_destination, 115);
}

#[test]
fn transfer_fee_non_initialize_helpers_reject_wrong_program_before_state_changes() {
    let (mut svm, payer, id) = setup(
        "transfer-fee",
        "token_2022_ext_transfer_fee.so",
        "CvCYVXhFDScZ8CNRtm6mSU8AkZrN5tk3NcFF8Q33M45z",
    );
    let mint = Pubkey::new_unique();
    let source = Pubkey::new_unique();
    let destination = Pubkey::new_unique();
    let authority = tests_v2::keypair_for("token-2022-ext-transfer-fee-guard-authority");
    seed_mint_with_extensions(&mut svm, mint, &[ExtensionType::TransferFeeConfig]);
    seed_transfer_fee_token_account(&mut svm, source, mint, authority.pubkey(), 500, 7);
    seed_transfer_fee_token_account(&mut svm, destination, mint, authority.pubkey(), 0, 0);
    svm.airdrop(&authority.pubkey(), 1_000_000_000).unwrap();

    let cases = [
        (
            vec![1],
            vec![
                Meta::new(mint, false),
                Meta::new_readonly(authority.pubkey(), true),
                Meta::new_readonly(wrong_token_program_id(), false),
            ],
        ),
        (
            vec![2],
            vec![
                Meta::new(source, false),
                Meta::new_readonly(mint, false),
                Meta::new(destination, false),
                Meta::new_readonly(authority.pubkey(), true),
                Meta::new_readonly(wrong_token_program_id(), false),
            ],
        ),
        (
            vec![4],
            vec![
                Meta::new(mint, false),
                Meta::new(destination, false),
                Meta::new_readonly(authority.pubkey(), true),
                Meta::new_readonly(wrong_token_program_id(), false),
            ],
        ),
        (
            vec![5],
            vec![
                Meta::new_readonly(mint, false),
                Meta::new(destination, false),
                Meta::new_readonly(authority.pubkey(), true),
                Meta::new(source, false),
                Meta::new_readonly(wrong_token_program_id(), false),
            ],
        ),
    ];

    for (data, metas) in cases {
        expect_incorrect_program(
            send(&mut svm, id, data, metas, &payer, &[&authority]),
            "transfer fee helper should reject non-Token-2022 program",
        );
    }
    assert_transfer_fee_amount(&svm, source, 7);
    assert_transfer_fee_amount(&svm, destination, 0);
}

fn seed_transfer_fee_token_account(
    svm: &mut anchor_v2_testing::litesvm::LiteSVM,
    account: Pubkey,
    mint: Pubkey,
    owner: Pubkey,
    amount: u64,
    withheld_amount: u64,
) {
    let mut data =
        initialized_token_account_data(mint, owner, amount, &[ExtensionType::TransferFeeAmount]);
    {
        let mut state =
            StateWithExtensionsMut::<Token2022Account>::unpack(&mut data).expect("unpack token");
        let extension = state
            .init_extension::<TransferFeeAmount>(false)
            .expect("initialize transfer-fee amount extension");
        extension.withheld_amount = withheld_amount.into();
    }
    seed_token_2022_account(svm, account, data);
}

fn assert_transfer_fee_config(
    svm: &anchor_v2_testing::litesvm::LiteSVM,
    mint: Pubkey,
    expected_config_authority: Option<Pubkey>,
    expected_withdraw_authority: Option<Pubkey>,
    expected_withheld_amount: u64,
    expected_bps: u16,
    expected_max_fee: u64,
) {
    let mut data = svm.get_account(&mint).expect("mint exists").data;
    let state = mint_state(&mut data);
    let extension = state
        .get_extension::<TransferFeeConfig>()
        .expect("transfer fee config exists");
    assert_eq!(
        Option::<Pubkey>::from(extension.transfer_fee_config_authority),
        expected_config_authority
    );
    assert_eq!(
        Option::<Pubkey>::from(extension.withdraw_withheld_authority),
        expected_withdraw_authority
    );
    assert_eq!(
        u64::from(extension.withheld_amount),
        expected_withheld_amount
    );
    assert_eq!(
        u16::from(extension.newer_transfer_fee.transfer_fee_basis_points),
        expected_bps
    );
    assert_eq!(
        u64::from(extension.newer_transfer_fee.maximum_fee),
        expected_max_fee
    );
}

fn assert_token_amount(svm: &anchor_v2_testing::litesvm::LiteSVM, account: Pubkey, expected: u64) {
    let mut data = svm.get_account(&account).expect("token exists").data;
    let state =
        StateWithExtensionsMut::<Token2022Account>::unpack(&mut data).expect("unpack token");
    assert_eq!(state.base.amount, expected);
}

fn assert_transfer_fee_amount(
    svm: &anchor_v2_testing::litesvm::LiteSVM,
    account: Pubkey,
    expected: u64,
) {
    let mut data = svm.get_account(&account).expect("token exists").data;
    let state =
        StateWithExtensionsMut::<Token2022Account>::unpack(&mut data).expect("unpack token");
    let extension = state
        .get_extension::<TransferFeeAmount>()
        .expect("transfer fee amount extension exists");
    assert_eq!(u64::from(extension.withheld_amount), expected);
}
