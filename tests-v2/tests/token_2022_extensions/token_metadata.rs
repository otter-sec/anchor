use {
    super::common::*,
    solana_pubkey::Pubkey,
    solana_signer::Signer,
    spl_token_2022_interface::{
        extension::{metadata_pointer::MetadataPointer, ExtensionType, StateWithExtensionsMut},
        state::Mint,
    },
    spl_token_metadata_interface::state::TokenMetadata,
};

#[test]
fn initializes_updates_and_removes_metadata_on_the_mint() {
    let (mut svm, payer, id) = setup(
        "token-metadata",
        "token_2022_ext_token_metadata.so",
        "7j7C1skvNNAm1GPr6icFPbdsare3eYpQGrtuzXJ6jzy6",
    );
    let mint = Pubkey::new_unique();
    let mint_authority = tests_v2::keypair_for("token-2022-ext-token-metadata-mint-authority");
    let update_authority = tests_v2::keypair_for("token-2022-ext-token-metadata-update-authority");
    let new_authority = Pubkey::new_unique();
    let mut mint_data = initialized_mint_data_with_space(
        mint_authority.pubkey(),
        None,
        &[ExtensionType::MetadataPointer],
        512,
    );
    {
        let mut state =
            StateWithExtensionsMut::<Mint>::unpack(&mut mint_data).expect("unpack metadata mint");
        let extension = state
            .init_extension::<MetadataPointer>(false)
            .expect("initialize metadata pointer");
        extension.authority = Some(update_authority.pubkey()).try_into().unwrap();
        extension.metadata_address = Some(mint).try_into().unwrap();
    }
    seed_token_2022_account(&mut svm, mint, mint_data);
    svm.airdrop(&mint_authority.pubkey(), 1_000_000_000)
        .unwrap();
    svm.airdrop(&update_authority.pubkey(), 1_000_000_000)
        .unwrap();

    let init_metas = vec![
        Meta::new(mint, false),
        Meta::new_readonly(update_authority.pubkey(), false),
        Meta::new_readonly(mint, false),
        Meta::new_readonly(mint_authority.pubkey(), true),
        Meta::new_readonly(token_2022_program_id(), false),
    ];
    let metadata = send(
        &mut svm,
        id,
        vec![0],
        init_metas,
        &payer,
        &[&mint_authority],
    )
    .expect("token metadata initialize should invoke real Token-2022");
    assert_token_2022_cpi_succeeded(&metadata, "token metadata initialize");
    assert_metadata_state(&svm, mint, update_authority.pubkey(), mint, "name", "SYM");

    let update_field_metas = vec![
        Meta::new(mint, false),
        Meta::new_readonly(update_authority.pubkey(), true),
        Meta::new_readonly(token_2022_program_id(), false),
    ];
    let metadata = send(
        &mut svm,
        id,
        vec![2],
        update_field_metas,
        &payer,
        &[&update_authority],
    )
    .expect("token metadata update_field should invoke real Token-2022");
    assert_token_2022_cpi_succeeded(&metadata, "token metadata update_field");
    assert_metadata_field(&svm, mint, "field", Some("value"));

    let remove_key_metas = vec![
        Meta::new(mint, false),
        Meta::new_readonly(update_authority.pubkey(), true),
        Meta::new_readonly(token_2022_program_id(), false),
    ];
    let metadata = send(
        &mut svm,
        id,
        vec![3],
        remove_key_metas,
        &payer,
        &[&update_authority],
    )
    .expect("token metadata remove_key should invoke real Token-2022");
    assert_token_2022_cpi_succeeded(&metadata, "token metadata remove_key");
    assert_metadata_field(&svm, mint, "field", None);

    let mut update_authority_data = vec![1];
    update_authority_data.extend_from_slice(&address_bytes(new_authority));
    let update_authority_metas = vec![
        Meta::new(mint, false),
        Meta::new_readonly(update_authority.pubkey(), true),
        Meta::new_readonly(token_2022_program_id(), false),
    ];
    let metadata = send(
        &mut svm,
        id,
        update_authority_data,
        update_authority_metas,
        &payer,
        &[&update_authority],
    )
    .expect("token metadata update_authority should invoke real Token-2022");
    assert_token_2022_cpi_succeeded(&metadata, "token metadata update_authority");
    assert_metadata_state(&svm, mint, new_authority, mint, "name", "SYM");
}

#[test]
fn token_metadata_helpers_reject_wrong_program_before_state_changes() {
    let (mut svm, payer, id) = setup(
        "token-metadata",
        "token_2022_ext_token_metadata.so",
        "7j7C1skvNNAm1GPr6icFPbdsare3eYpQGrtuzXJ6jzy6",
    );
    let mint = Pubkey::new_unique();
    let mint_authority =
        tests_v2::keypair_for("token-2022-ext-token-metadata-guard-mint-authority");
    let update_authority =
        tests_v2::keypair_for("token-2022-ext-token-metadata-guard-update-authority");
    let new_authority = Pubkey::new_unique();
    let mut mint_data = initialized_mint_data_with_space(
        mint_authority.pubkey(),
        None,
        &[ExtensionType::MetadataPointer],
        512,
    );
    {
        let mut state =
            StateWithExtensionsMut::<Mint>::unpack(&mut mint_data).expect("unpack metadata mint");
        let extension = state
            .init_extension::<MetadataPointer>(false)
            .expect("initialize metadata pointer");
        extension.authority = Some(update_authority.pubkey()).try_into().unwrap();
        extension.metadata_address = Some(mint).try_into().unwrap();
    }
    seed_token_2022_account(&mut svm, mint, mint_data);
    svm.airdrop(&mint_authority.pubkey(), 1_000_000_000)
        .unwrap();
    svm.airdrop(&update_authority.pubkey(), 1_000_000_000)
        .unwrap();

    let init_metas = vec![
        Meta::new(mint, false),
        Meta::new_readonly(update_authority.pubkey(), false),
        Meta::new_readonly(mint, false),
        Meta::new_readonly(mint_authority.pubkey(), true),
        Meta::new_readonly(token_2022_program_id(), false),
    ];
    send(
        &mut svm,
        id,
        vec![0],
        init_metas,
        &payer,
        &[&mint_authority],
    )
    .expect("token metadata initialize should invoke real Token-2022");
    assert_metadata_state(&svm, mint, update_authority.pubkey(), mint, "name", "SYM");

    let cases = [
        (
            vec![0],
            vec![
                Meta::new(mint, false),
                Meta::new_readonly(update_authority.pubkey(), false),
                Meta::new_readonly(mint, false),
                Meta::new_readonly(mint_authority.pubkey(), true),
                Meta::new_readonly(wrong_token_program_id(), false),
            ],
            vec![&mint_authority],
        ),
        (
            {
                let mut data = vec![1];
                data.extend_from_slice(&address_bytes(new_authority));
                data
            },
            vec![
                Meta::new(mint, false),
                Meta::new_readonly(update_authority.pubkey(), true),
                Meta::new_readonly(wrong_token_program_id(), false),
            ],
            vec![&update_authority],
        ),
        (
            vec![2],
            vec![
                Meta::new(mint, false),
                Meta::new_readonly(update_authority.pubkey(), true),
                Meta::new_readonly(wrong_token_program_id(), false),
            ],
            vec![&update_authority],
        ),
        (
            vec![3],
            vec![
                Meta::new(mint, false),
                Meta::new_readonly(update_authority.pubkey(), true),
                Meta::new_readonly(wrong_token_program_id(), false),
            ],
            vec![&update_authority],
        ),
    ];

    for (data, metas, signers) in cases {
        expect_incorrect_program(
            send(&mut svm, id, data, metas, &payer, &signers),
            "token metadata helper should reject non-Token-2022 program",
        );
    }
    assert_metadata_state(&svm, mint, update_authority.pubkey(), mint, "name", "SYM");
    assert_metadata_field(&svm, mint, "field", None);
}

fn assert_metadata_state(
    svm: &anchor_v2_testing::litesvm::LiteSVM,
    mint: Pubkey,
    expected_update_authority: Pubkey,
    expected_mint: Pubkey,
    expected_name: &str,
    expected_symbol: &str,
) {
    let mut data = svm.get_account(&mint).expect("mint exists").data;
    let state = mint_state(&mut data);
    let metadata = state
        .get_variable_len_extension::<TokenMetadata>()
        .expect("token metadata extension exists");
    assert_eq!(
        Option::<Pubkey>::from(metadata.update_authority),
        Some(expected_update_authority)
    );
    assert_eq!(metadata.mint, expected_mint);
    assert_eq!(metadata.name, expected_name);
    assert_eq!(metadata.symbol, expected_symbol);
}

fn assert_metadata_field(
    svm: &anchor_v2_testing::litesvm::LiteSVM,
    mint: Pubkey,
    expected_key: &str,
    expected_value: Option<&str>,
) {
    let mut data = svm.get_account(&mint).expect("mint exists").data;
    let state = mint_state(&mut data);
    let metadata = state
        .get_variable_len_extension::<TokenMetadata>()
        .expect("token metadata extension exists");
    let actual = metadata
        .additional_metadata
        .iter()
        .find(|(key, _)| key == expected_key)
        .map(|(_, value)| value.as_str());
    assert_eq!(actual, expected_value);
}
