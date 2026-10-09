use {
    super::common::*,
    solana_pubkey::Pubkey,
    solana_signer::Signer,
    spl_token_2022_interface::extension::{group_pointer::GroupPointer, ExtensionType},
};

#[test]
fn initializes_and_updates_group_pointer() {
    let (mut svm, payer, id) = setup(
        "group-pointer",
        "token_2022_ext_group_pointer.so",
        "3aVa6BL8bgD4My8vgUABgjGSBTYpHQ6Ft41wC3H5EQ5f",
    );
    let mint = Pubkey::new_unique();
    let authority = tests_v2::keypair_for("token-2022-ext-group-pointer-authority");
    let group = Pubkey::new_unique();
    let updated_group = Pubkey::new_unique();
    seed_mint_with_extensions(&mut svm, mint, &[ExtensionType::GroupPointer]);
    svm.airdrop(&authority.pubkey(), 1_000_000_000).unwrap();

    let mut init_data = vec![0];
    init_data.extend_from_slice(&address_bytes(authority.pubkey()));
    init_data.extend_from_slice(&address_bytes(group));
    let ok_init_metas = vec![
        Meta::new(mint, false),
        Meta::new_readonly(token_2022_program_id(), false),
    ];
    let metadata = send(&mut svm, id, init_data.clone(), ok_init_metas, &payer, &[])
        .expect("group pointer initialize should invoke real Token-2022");
    assert_token_2022_cpi_succeeded(&metadata, "group pointer initialize");
    assert_group_pointer(&svm, mint, Some(authority.pubkey()), Some(group));

    mark_mint_initialized(&mut svm, mint, Pubkey::new_unique(), None);
    let mut update_data = vec![1];
    update_data.extend_from_slice(&address_bytes(updated_group));
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
    .expect("group pointer update should invoke real Token-2022");
    assert_token_2022_cpi_succeeded(&metadata, "group pointer update");
    assert_group_pointer(&svm, mint, Some(authority.pubkey()), Some(updated_group));

    let bad_init_metas = vec![
        Meta::new(mint, false),
        Meta::new_readonly(wrong_token_program_id(), false),
    ];
    expect_incorrect_program(
        send(&mut svm, id, init_data, bad_init_metas, &payer, &[]),
        "group pointer initialize should reject non-Token-2022 program",
    );
}

fn assert_group_pointer(
    svm: &anchor_v2_testing::litesvm::LiteSVM,
    mint: Pubkey,
    expected_authority: Option<Pubkey>,
    expected_group: Option<Pubkey>,
) {
    let mut data = svm.get_account(&mint).expect("mint exists").data;
    let state = mint_state(&mut data);
    let extension = state
        .get_extension::<GroupPointer>()
        .expect("group pointer extension exists");
    assert_eq!(
        Option::<Pubkey>::from(extension.authority),
        expected_authority
    );
    assert_eq!(
        Option::<Pubkey>::from(extension.group_address),
        expected_group
    );
}
