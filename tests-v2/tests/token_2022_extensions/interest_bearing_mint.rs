use {
    super::common::*,
    solana_pubkey::Pubkey,
    solana_signer::Signer,
    spl_token_2022_interface::extension::{
        interest_bearing_mint::InterestBearingConfig, ExtensionType,
    },
};

#[test]
fn initializes_and_updates_interest_rate() {
    let (mut svm, payer, id) = setup(
        "interest-bearing-mint",
        "token_2022_ext_interest_bearing_mint.so",
        "4Yv95TS6s4kME8qgqLpjkekknuXJnksuCnVbc37Pdp6j",
    );
    let mint = Pubkey::new_unique();
    let authority = tests_v2::keypair_for("token-2022-ext-interest-authority");
    seed_mint_with_extensions(&mut svm, mint, &[ExtensionType::InterestBearingConfig]);
    svm.airdrop(&authority.pubkey(), 1_000_000_000).unwrap();

    let mut init_data = vec![0];
    init_data.extend_from_slice(&address_bytes(authority.pubkey()));
    let ok_metas = vec![
        Meta::new(mint, false),
        Meta::new_readonly(token_2022_program_id(), false),
    ];
    let metadata = send(&mut svm, id, init_data.clone(), ok_metas, &payer, &[])
        .expect("interest-bearing initialize should invoke real Token-2022");
    assert_token_2022_cpi_succeeded(&metadata, "interest-bearing initialize");
    assert_interest_config(&svm, mint, Some(authority.pubkey()), 125);

    mark_mint_initialized(&mut svm, mint, Pubkey::new_unique(), None);
    let update_metas = vec![
        Meta::new(mint, false),
        Meta::new_readonly(authority.pubkey(), true),
        Meta::new_readonly(token_2022_program_id(), false),
    ];
    let metadata = send(&mut svm, id, vec![1], update_metas, &payer, &[&authority])
        .expect("interest-bearing update should invoke real Token-2022");
    assert_token_2022_cpi_succeeded(&metadata, "interest-bearing update");
    assert_interest_config(&svm, mint, Some(authority.pubkey()), -125);

    let bad_metas = vec![
        Meta::new(mint, false),
        Meta::new_readonly(wrong_token_program_id(), false),
    ];
    expect_incorrect_program(
        send(&mut svm, id, init_data, bad_metas, &payer, &[]),
        "interest-bearing initialize should reject non-Token-2022 program",
    );
}

fn assert_interest_config(
    svm: &anchor_v2_testing::litesvm::LiteSVM,
    mint: Pubkey,
    expected_authority: Option<Pubkey>,
    expected_rate: i16,
) {
    let mut data = svm.get_account(&mint).expect("mint exists").data;
    let state = mint_state(&mut data);
    let extension = state
        .get_extension::<InterestBearingConfig>()
        .expect("interest-bearing config exists");
    assert_eq!(
        Option::<Pubkey>::from(extension.rate_authority),
        expected_authority
    );
    assert_eq!(i16::from(extension.current_rate), expected_rate);
}
