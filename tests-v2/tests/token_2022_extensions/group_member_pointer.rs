use {
    super::common::*,
    solana_pubkey::Pubkey,
    solana_signer::Signer,
    spl_token_2022_interface::extension::{
        group_member_pointer::GroupMemberPointer, ExtensionType,
    },
};

#[test]
fn initializes_and_updates_group_member_pointer() {
    let (mut svm, payer, id) = setup(
        "group-member-pointer",
        "token_2022_ext_group_member_pointer.so",
        "FCvBGDgHxLFU3rgL8jqp4aGJpjJzyyotDKQdrjLhUner",
    );
    let mint = Pubkey::new_unique();
    let authority = tests_v2::keypair_for("token-2022-ext-group-member-pointer-authority");
    let member = Pubkey::new_unique();
    let updated_member = Pubkey::new_unique();
    seed_mint_with_extensions(&mut svm, mint, &[ExtensionType::GroupMemberPointer]);
    svm.airdrop(&authority.pubkey(), 1_000_000_000).unwrap();

    let mut init_data = vec![0];
    init_data.extend_from_slice(&address_bytes(authority.pubkey()));
    init_data.extend_from_slice(&address_bytes(member));
    let ok_init_metas = vec![
        Meta::new(mint, false),
        Meta::new_readonly(token_2022_program_id(), false),
    ];
    let metadata = send(&mut svm, id, init_data.clone(), ok_init_metas, &payer, &[])
        .expect("group member pointer initialize should invoke real Token-2022");
    assert_token_2022_cpi_succeeded(&metadata, "group member pointer initialize");
    assert_group_member_pointer(&svm, mint, Some(authority.pubkey()), Some(member));

    mark_mint_initialized(&mut svm, mint, Pubkey::new_unique(), None);
    let mut update_data = vec![1];
    update_data.extend_from_slice(&address_bytes(updated_member));
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
    .expect("group member pointer update should invoke real Token-2022");
    assert_token_2022_cpi_succeeded(&metadata, "group member pointer update");
    assert_group_member_pointer(&svm, mint, Some(authority.pubkey()), Some(updated_member));

    let bad_init_metas = vec![
        Meta::new(mint, false),
        Meta::new_readonly(wrong_token_program_id(), false),
    ];
    expect_incorrect_program(
        send(&mut svm, id, init_data, bad_init_metas, &payer, &[]),
        "group member pointer initialize should reject non-Token-2022 program",
    );
}

fn assert_group_member_pointer(
    svm: &anchor_v2_testing::litesvm::LiteSVM,
    mint: Pubkey,
    expected_authority: Option<Pubkey>,
    expected_member: Option<Pubkey>,
) {
    let mut data = svm.get_account(&mint).expect("mint exists").data;
    let state = mint_state(&mut data);
    let extension = state
        .get_extension::<GroupMemberPointer>()
        .expect("group member pointer extension exists");
    assert_eq!(
        Option::<Pubkey>::from(extension.authority),
        expected_authority
    );
    assert_eq!(
        Option::<Pubkey>::from(extension.member_address),
        expected_member
    );
}
