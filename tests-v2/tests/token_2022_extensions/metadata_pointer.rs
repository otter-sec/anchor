use {
    super::common::*,
    solana_pubkey::Pubkey,
    solana_signer::Signer,
    spl_token_2022_interface::extension::{metadata_pointer::MetadataPointer, ExtensionType},
};

#[test]
fn initializes_and_updates_metadata_pointer() {
    let (mut svm, payer, id) = setup(
        "metadata-pointer",
        "token_2022_ext_metadata_pointer.so",
        "8PeNs8jhrvR4uDtSSyB2iYcyx5FUQBWhrnHJfwdHwXiS",
    );
    let mint = Pubkey::new_unique();
    let authority = tests_v2::keypair_for("token-2022-ext-metadata-pointer-authority");
    let metadata = Pubkey::new_unique();
    let updated_metadata = Pubkey::new_unique();
    seed_mint_with_extensions(&mut svm, mint, &[ExtensionType::MetadataPointer]);
    svm.airdrop(&authority.pubkey(), 1_000_000_000).unwrap();

    let mut init_data = vec![0];
    init_data.extend_from_slice(&address_bytes(authority.pubkey()));
    init_data.extend_from_slice(&address_bytes(metadata));
    let ok_init_metas = vec![
        Meta::new(mint, false),
        Meta::new_readonly(token_2022_program_id(), false),
    ];
    let result = send(&mut svm, id, init_data.clone(), ok_init_metas, &payer, &[])
        .expect("metadata pointer initialize should invoke real Token-2022");
    assert_token_2022_cpi_succeeded(&result, "metadata pointer initialize");
    assert_metadata_pointer(&svm, mint, Some(authority.pubkey()), Some(metadata));

    mark_mint_initialized(&mut svm, mint, Pubkey::new_unique(), None);
    let mut update_data = vec![1];
    update_data.extend_from_slice(&address_bytes(updated_metadata));
    let update_metas = vec![
        Meta::new(mint, false),
        Meta::new_readonly(authority.pubkey(), true),
        Meta::new_readonly(token_2022_program_id(), false),
    ];
    let result = send(
        &mut svm,
        id,
        update_data,
        update_metas,
        &payer,
        &[&authority],
    )
    .expect("metadata pointer update should invoke real Token-2022");
    assert_token_2022_cpi_succeeded(&result, "metadata pointer update");
    assert_metadata_pointer(&svm, mint, Some(authority.pubkey()), Some(updated_metadata));

    let bad_init_metas = vec![
        Meta::new(mint, false),
        Meta::new_readonly(wrong_token_program_id(), false),
    ];
    expect_incorrect_program(
        send(&mut svm, id, init_data, bad_init_metas, &payer, &[]),
        "metadata pointer initialize should reject non-Token-2022 program",
    );
}

fn assert_metadata_pointer(
    svm: &anchor_v2_testing::litesvm::LiteSVM,
    mint: Pubkey,
    expected_authority: Option<Pubkey>,
    expected_metadata: Option<Pubkey>,
) {
    let mut data = svm.get_account(&mint).expect("mint exists").data;
    let state = mint_state(&mut data);
    let extension = state
        .get_extension::<MetadataPointer>()
        .expect("metadata pointer extension exists");
    assert_eq!(
        Option::<Pubkey>::from(extension.authority),
        expected_authority
    );
    assert_eq!(
        Option::<Pubkey>::from(extension.metadata_address),
        expected_metadata
    );
}
