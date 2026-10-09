use {
    super::common::*,
    solana_pubkey::Pubkey,
    solana_signer::Signer,
    spl_token_2022_interface::{
        extension::{
            group_member_pointer::GroupMemberPointer, group_pointer::GroupPointer, ExtensionType,
            StateWithExtensionsMut,
        },
        state::Mint,
    },
    spl_token_group_interface::state::{TokenGroup, TokenGroupMember},
};

#[test]
fn initializes_group_on_the_mint_and_rejects_wrong_program() {
    let (mut svm, payer, id) = setup(
        "token-group",
        "token_2022_ext_token_group.so",
        "BrUvjfkhwnq2oL4uvHCEZ3LDxaXsqbjUCvGUq4LtDHAb",
    );
    let mint = Pubkey::new_unique();
    let mint_authority = tests_v2::keypair_for("token-2022-ext-token-group-mint-authority");
    let group_authority = tests_v2::keypair_for("token-2022-ext-token-group-authority");
    let mut data = initialized_mint_data_with_space(
        mint_authority.pubkey(),
        None,
        &[ExtensionType::GroupPointer],
        256,
    );
    {
        let mut state =
            StateWithExtensionsMut::<Mint>::unpack(&mut data).expect("unpack group mint");
        let extension = state
            .init_extension::<GroupPointer>(false)
            .expect("initialize group pointer");
        extension.authority = Some(group_authority.pubkey()).try_into().unwrap();
        extension.group_address = Some(mint).try_into().unwrap();
    }
    seed_token_2022_account(&mut svm, mint, data);
    svm.airdrop(&mint_authority.pubkey(), 1_000_000_000)
        .unwrap();
    svm.airdrop(&group_authority.pubkey(), 1_000_000_000)
        .unwrap();

    let group_metas = vec![
        Meta::new(mint, false),
        Meta::new_readonly(mint, false),
        Meta::new_readonly(mint_authority.pubkey(), true),
        Meta::new_readonly(token_2022_program_id(), false),
    ];
    let metadata = send(
        &mut svm,
        id,
        vec![0],
        group_metas,
        &payer,
        &[&mint_authority],
    )
    .expect("token group initialize should invoke real Token-2022");
    assert_token_2022_cpi_succeeded(&metadata, "token group initialize");
    assert_group_state(&svm, mint, Some(mint_authority.pubkey()), mint, 0, 10);

    let member_mint = Pubkey::new_unique();
    let member_mint_authority =
        tests_v2::keypair_for("token-2022-ext-token-group-member-mint-authority");
    let mut member_data = initialized_mint_data_with_space(
        member_mint_authority.pubkey(),
        None,
        &[ExtensionType::GroupMemberPointer],
        256,
    );
    {
        let mut state =
            StateWithExtensionsMut::<Mint>::unpack(&mut member_data).expect("unpack member mint");
        let extension = state
            .init_extension::<GroupMemberPointer>(false)
            .expect("initialize group member pointer");
        extension.authority = Some(mint_authority.pubkey()).try_into().unwrap();
        extension.member_address = Some(member_mint).try_into().unwrap();
    }
    seed_token_2022_account(&mut svm, member_mint, member_data);
    svm.airdrop(&member_mint_authority.pubkey(), 1_000_000_000)
        .unwrap();

    let member_metas = vec![
        Meta::new(member_mint, false),
        Meta::new_readonly(member_mint, false),
        Meta::new_readonly(member_mint_authority.pubkey(), true),
        Meta::new(mint, false),
        Meta::new_readonly(mint_authority.pubkey(), true),
        Meta::new_readonly(token_2022_program_id(), false),
    ];
    let metadata = send(
        &mut svm,
        id,
        vec![1],
        member_metas,
        &payer,
        &[&member_mint_authority, &mint_authority],
    )
    .expect("token group member initialize should invoke real Token-2022");
    assert_token_2022_cpi_succeeded(&metadata, "token group member initialize");
    assert_group_state(&svm, mint, Some(mint_authority.pubkey()), mint, 1, 10);
    assert_member_state(&svm, member_mint, member_mint, mint, 1);

    let bad_group_metas = vec![
        Meta::new(mint, false),
        Meta::new_readonly(mint, false),
        Meta::new_readonly(mint_authority.pubkey(), true),
        Meta::new_readonly(wrong_token_program_id(), false),
    ];
    expect_incorrect_program(
        send(
            &mut svm,
            id,
            vec![0],
            bad_group_metas,
            &payer,
            &[&mint_authority],
        ),
        "token group initialize helper should reject non-Token-2022 program",
    );
}

fn assert_group_state(
    svm: &anchor_v2_testing::litesvm::LiteSVM,
    mint: Pubkey,
    expected_update_authority: Option<Pubkey>,
    expected_mint: Pubkey,
    expected_size: u64,
    max_size: u64,
) {
    let mut data = svm.get_account(&mint).expect("mint exists").data;
    let state = mint_state(&mut data);
    let group = state
        .get_extension::<TokenGroup>()
        .expect("token group extension exists");
    assert_eq!(
        Option::<Pubkey>::from(group.update_authority),
        expected_update_authority
    );
    assert_eq!(group.mint.as_ref(), expected_mint.as_ref());
    assert_eq!(u64::from(group.size), expected_size);
    assert_eq!(u64::from(group.max_size), max_size);
}

fn assert_member_state(
    svm: &anchor_v2_testing::litesvm::LiteSVM,
    member: Pubkey,
    expected_mint: Pubkey,
    expected_group: Pubkey,
    expected_member_number: u64,
) {
    let mut data = svm.get_account(&member).expect("member mint exists").data;
    let state = mint_state(&mut data);
    let member = state
        .get_extension::<TokenGroupMember>()
        .expect("token group member extension exists");
    assert_eq!(member.mint.as_ref(), expected_mint.as_ref());
    assert_eq!(member.group.as_ref(), expected_group.as_ref());
    assert_eq!(u64::from(member.member_number), expected_member_number);
}
