use {
    super::common::*,
    solana_pubkey::Pubkey,
    solana_signer::Signer,
    spl_token_2022_interface::extension::{transfer_hook::TransferHook, ExtensionType},
};

#[test]
fn initializes_and_updates_transfer_hook_program_id() {
    let (mut svm, payer, id) = setup(
        "transfer-hook",
        "token_2022_ext_transfer_hook.so",
        "Bs5CGVSvcNqrTyzZig9fVHcDZhCmvFeserfVFK7BiSjR",
    );
    let mint = Pubkey::new_unique();
    let authority = tests_v2::keypair_for("token-2022-ext-transfer-hook-authority");
    let hook_program = Pubkey::new_unique();
    let updated_hook_program = Pubkey::new_unique();
    seed_mint_with_extensions(&mut svm, mint, &[ExtensionType::TransferHook]);
    svm.airdrop(&authority.pubkey(), 1_000_000_000).unwrap();

    let mut init_data = vec![0];
    init_data.extend_from_slice(&address_bytes(authority.pubkey()));
    init_data.extend_from_slice(&address_bytes(hook_program));
    let ok_init_metas = vec![
        Meta::new(mint, false),
        Meta::new_readonly(token_2022_program_id(), false),
    ];
    let metadata = send(&mut svm, id, init_data.clone(), ok_init_metas, &payer, &[])
        .expect("transfer hook initialize should invoke real Token-2022");
    assert_token_2022_cpi_succeeded(&metadata, "transfer hook initialize");
    assert_transfer_hook(&svm, mint, Some(authority.pubkey()), Some(hook_program));

    mark_mint_initialized(&mut svm, mint, Pubkey::new_unique(), None);
    let mut update_data = vec![1];
    update_data.extend_from_slice(&address_bytes(updated_hook_program));
    let update_metas = vec![
        Meta::new(mint, false),
        Meta::new_readonly(authority.pubkey(), true),
        Meta::new_readonly(token_2022_program_id(), false),
    ];
    let metadata = send(
        &mut svm,
        id,
        update_data,
        update_metas,
        &payer,
        &[&authority],
    )
    .expect("transfer hook update should invoke real Token-2022");
    assert_token_2022_cpi_succeeded(&metadata, "transfer hook update");
    assert_transfer_hook(
        &svm,
        mint,
        Some(authority.pubkey()),
        Some(updated_hook_program),
    );

    let bad_init_metas = vec![
        Meta::new(mint, false),
        Meta::new_readonly(wrong_token_program_id(), false),
    ];
    expect_incorrect_program(
        send(&mut svm, id, init_data, bad_init_metas, &payer, &[]),
        "transfer hook initialize should reject non-Token-2022 program",
    );
}

fn assert_transfer_hook(
    svm: &anchor_v2_testing::litesvm::LiteSVM,
    mint: Pubkey,
    expected_authority: Option<Pubkey>,
    expected_program: Option<Pubkey>,
) {
    let mut data = svm.get_account(&mint).expect("mint exists").data;
    let state = mint_state(&mut data);
    let extension = state
        .get_extension::<TransferHook>()
        .expect("transfer hook extension exists");
    assert_eq!(
        Option::<Pubkey>::from(extension.authority),
        expected_authority
    );
    assert_eq!(
        Option::<Pubkey>::from(extension.program_id),
        expected_program
    );
}
