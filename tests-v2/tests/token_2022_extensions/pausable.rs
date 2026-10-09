use {
    super::common::*,
    solana_pubkey::Pubkey,
    solana_signer::Signer,
    spl_token_2022_interface::extension::{pausable::PausableConfig, ExtensionType},
};

#[test]
fn initializes_pauses_and_resumes_mint() {
    let (mut svm, payer, id) = setup(
        "pausable",
        "token_2022_ext_pausable.so",
        "EoHXMZePT9ShHp5tUxBhZQW4MRm4P4r2ejx7VXMpP2My",
    );
    let mint = Pubkey::new_unique();
    let authority = tests_v2::keypair_for("token-2022-ext-pausable-authority");
    seed_mint_with_extensions(&mut svm, mint, &[ExtensionType::Pausable]);
    svm.airdrop(&authority.pubkey(), 1_000_000_000).unwrap();

    let mut init_data = vec![0];
    init_data.extend_from_slice(&address_bytes(authority.pubkey()));
    let init_metas = vec![
        Meta::new(mint, false),
        Meta::new_readonly(token_2022_program_id(), false),
    ];
    let metadata = send(&mut svm, id, init_data.clone(), init_metas, &payer, &[])
        .expect("pausable initialize should invoke real Token-2022");
    assert_token_2022_cpi_succeeded(&metadata, "pausable initialize");
    assert_pausable(&svm, mint, Some(authority.pubkey()), false);

    mark_mint_initialized(&mut svm, mint, Pubkey::new_unique(), None);
    for (discrim, expected_paused, label) in [(1, true, "pause"), (2, false, "resume")] {
        let metas = vec![
            Meta::new(mint, false),
            Meta::new_readonly(authority.pubkey(), true),
            Meta::new_readonly(token_2022_program_id(), false),
        ];
        let metadata = send(&mut svm, id, vec![discrim], metas, &payer, &[&authority])
            .unwrap_or_else(|error| panic!("pausable {label} should succeed: {error}"));
        assert_token_2022_cpi_succeeded(&metadata, &format!("pausable {label}"));
        assert_pausable(&svm, mint, Some(authority.pubkey()), expected_paused);
    }

    let bad_metas = vec![
        Meta::new(mint, false),
        Meta::new_readonly(wrong_token_program_id(), false),
    ];
    expect_incorrect_program(
        send(&mut svm, id, init_data, bad_metas, &payer, &[]),
        "pausable initialize should reject non-Token-2022 program",
    );
}

fn assert_pausable(
    svm: &anchor_v2_testing::litesvm::LiteSVM,
    mint: Pubkey,
    expected_authority: Option<Pubkey>,
    expected_paused: bool,
) {
    let mut data = svm.get_account(&mint).expect("mint exists").data;
    let state = mint_state(&mut data);
    let extension = state
        .get_extension::<PausableConfig>()
        .expect("pausable config exists");
    assert_eq!(
        Option::<Pubkey>::from(extension.authority),
        expected_authority
    );
    assert_eq!(bool::from(extension.paused), expected_paused);
}
