use {
    super::common::*,
    solana_pubkey::Pubkey,
    solana_signer::Signer,
    spl_token_2022_interface::{
        extension::{default_account_state::DefaultAccountState, ExtensionType},
        state::AccountState,
    },
};

#[test]
fn initializes_and_updates_default_account_state() {
    let (mut svm, payer, id) = setup(
        "default-account-state",
        "token_2022_ext_default_account_state.so",
        "Fetkn8caf7wN24u751NWUYhtXXGuCPrqTLyDtqU25EY8",
    );
    let mint = Pubkey::new_unique();
    let freeze_authority = tests_v2::keypair_for("token-2022-ext-default-state-authority");
    seed_mint_with_extensions(&mut svm, mint, &[ExtensionType::DefaultAccountState]);
    svm.airdrop(&freeze_authority.pubkey(), 1_000_000_000)
        .unwrap();

    let init_metas = vec![
        Meta::new(mint, false),
        Meta::new_readonly(token_2022_program_id(), false),
    ];
    let metadata = send(&mut svm, id, vec![0], init_metas, &payer, &[])
        .expect("default-account-state initialize should invoke real Token-2022");
    assert_token_2022_cpi_succeeded(&metadata, "default-account-state initialize");
    assert_default_state(&svm, mint, AccountState::Frozen);

    mark_mint_initialized(
        &mut svm,
        mint,
        Pubkey::new_unique(),
        Some(freeze_authority.pubkey()),
    );

    let update_metas = vec![
        Meta::new(mint, false),
        Meta::new_readonly(freeze_authority.pubkey(), true),
        Meta::new_readonly(token_2022_program_id(), false),
    ];
    let metadata = send(
        &mut svm,
        id,
        vec![1],
        update_metas,
        &payer,
        &[&freeze_authority],
    )
    .expect("default-account-state update should invoke real Token-2022");
    assert_token_2022_cpi_succeeded(&metadata, "default-account-state update");
    assert_default_state(&svm, mint, AccountState::Initialized);

    let bad_update_metas = vec![
        Meta::new(mint, false),
        Meta::new_readonly(freeze_authority.pubkey(), true),
        Meta::new_readonly(wrong_token_program_id(), false),
    ];
    expect_incorrect_program(
        send(
            &mut svm,
            id,
            vec![1],
            bad_update_metas,
            &payer,
            &[&freeze_authority],
        ),
        "default-account-state update should reject non-Token-2022 program",
    );
}

fn assert_default_state(
    svm: &anchor_v2_testing::litesvm::LiteSVM,
    mint: Pubkey,
    expected: AccountState,
) {
    let mut data = svm.get_account(&mint).expect("mint exists").data;
    let state = mint_state(&mut data);
    let extension = state
        .get_extension::<DefaultAccountState>()
        .expect("default-account-state extension exists");
    assert_eq!(extension.state, expected as u8);
}
