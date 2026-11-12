use {
    super::common::*, anchor_v2_testing::litesvm::LiteSVM, solana_keypair::Keypair,
    solana_pubkey::Pubkey, solana_signer::Signer,
    spl_token_2022_interface::instruction as token_ix,
};

// The pinned `spl-token-2022-interface` predates `PermissionedBurn`, so the
// test sizes the mint by hand: base mint padded to 165 bytes, one account type
// byte, a 4-byte TLV header and the 32-byte authority.
const PERMISSIONED_BURN_MINT_LEN: usize = 165 + 1 + 4 + 32;
const DECIMALS: u8 = 6;

#[test]
fn burns_only_with_permissioned_burn_authority() {
    let (mut svm, payer, id) = setup(
        "permissioned-burn",
        "token_2022_ext_permissioned_burn.so",
        "4wehnPGjiVvNZNSaT6SUfmU9S5AT6LvwZ9g87DDqyhq1",
    );
    let token_program = token_2022_program_id();
    let mint = Pubkey::new_unique();
    let account = Pubkey::new_unique();
    let mint_authority = tests_v2::keypair_for("token-2022-ext-permissioned-burn-mint-authority");
    let burn_authority = tests_v2::keypair_for("token-2022-ext-permissioned-burn-authority");
    let owner = tests_v2::keypair_for("token-2022-ext-permissioned-burn-owner");
    seed_token_2022_account(&mut svm, mint, vec![0; PERMISSIONED_BURN_MINT_LEN]);

    let mut init_data = vec![0];
    init_data.extend_from_slice(&address_bytes(burn_authority.pubkey()));
    let init_metas = vec![
        Meta::new(mint, false),
        Meta::new_readonly(token_program, false),
    ];
    let metadata = send(&mut svm, id, init_data.clone(), init_metas, &payer, &[])
        .expect("permissioned-burn initialize should invoke real Token-2022");
    assert_token_2022_cpi_succeeded(&metadata, "permissioned-burn initialize");

    let init_mint = token_ix::initialize_mint2(
        &token_program,
        &mint,
        &mint_authority.pubkey(),
        None,
        DECIMALS,
    )
    .unwrap();
    send(
        &mut svm,
        token_program,
        init_mint.data,
        init_mint.accounts,
        &payer,
        &[],
    )
    .expect("initialize the permissioned-burn mint");
    // The extension adds nothing to a token account, so it keeps the base size.
    seed_token_2022_account(&mut svm, account, vec![0; 165]);
    let init_account =
        token_ix::initialize_account3(&token_program, &account, &mint, &owner.pubkey()).unwrap();
    send(
        &mut svm,
        token_program,
        init_account.data,
        init_account.accounts,
        &payer,
        &[],
    )
    .expect("initialize the token account");
    let mint_to = token_ix::mint_to(
        &token_program,
        &mint,
        &account,
        &mint_authority.pubkey(),
        &[],
        1_000,
    )
    .unwrap();
    send(
        &mut svm,
        token_program,
        mint_to.data,
        mint_to.accounts,
        &payer,
        &[&mint_authority],
    )
    .expect("mint to the token account");

    let plain_burn =
        token_ix::burn(&token_program, &account, &mint, &owner.pubkey(), &[], 1).unwrap();
    assert!(
        send(
            &mut svm,
            token_program,
            plain_burn.data,
            plain_burn.accounts,
            &payer,
            &[&owner],
        )
        .is_err(),
        "Token-2022 should reject a plain burn on a permissioned-burn mint"
    );

    let burn_metas = |burn_authority: Pubkey, token_program: Pubkey| {
        vec![
            Meta::new(account, false),
            Meta::new(mint, false),
            Meta::new_readonly(burn_authority, true),
            Meta::new_readonly(owner.pubkey(), true),
            Meta::new_readonly(token_program, false),
        ]
    };

    let mut burn_data = vec![1];
    burn_data.extend_from_slice(&100u64.to_le_bytes());
    let metadata = send(
        &mut svm,
        id,
        burn_data.clone(),
        burn_metas(burn_authority.pubkey(), token_program),
        &payer,
        &[&burn_authority, &owner],
    )
    .expect("permissioned burn should succeed");
    assert_token_2022_cpi_succeeded(&metadata, "permissioned burn");
    assert_balances(&svm, mint, account, 900);

    let mut burn_checked_data = vec![2];
    burn_checked_data.extend_from_slice(&200u64.to_le_bytes());
    burn_checked_data.push(DECIMALS);
    let metadata = send(
        &mut svm,
        id,
        burn_checked_data,
        burn_metas(burn_authority.pubkey(), token_program),
        &payer,
        &[&burn_authority, &owner],
    )
    .expect("permissioned burn checked should succeed");
    assert_token_2022_cpi_succeeded(&metadata, "permissioned burn checked");
    assert_balances(&svm, mint, account, 700);

    let impostor = Keypair::new();
    assert!(
        send(
            &mut svm,
            id,
            burn_data.clone(),
            burn_metas(impostor.pubkey(), token_program),
            &payer,
            &[&impostor, &owner],
        )
        .is_err(),
        "Token-2022 should reject a burn signed by another authority"
    );
    assert_balances(&svm, mint, account, 700);

    expect_incorrect_program(
        send(
            &mut svm,
            id,
            burn_data,
            burn_metas(burn_authority.pubkey(), wrong_token_program_id()),
            &payer,
            &[&burn_authority, &owner],
        ),
        "permissioned burn should reject non-Token-2022 program",
    );
    let bad_init_metas = vec![
        Meta::new(mint, false),
        Meta::new_readonly(wrong_token_program_id(), false),
    ];
    expect_incorrect_program(
        send(&mut svm, id, init_data, bad_init_metas, &payer, &[]),
        "permissioned-burn initialize should reject non-Token-2022 program",
    );
}

fn assert_balances(svm: &LiteSVM, mint: Pubkey, account: Pubkey, expected: u64) {
    let read_u64 = |address: &Pubkey, offset: usize| {
        let data = svm.get_account(address).expect("account exists").data;
        u64::from_le_bytes(data[offset..offset + 8].try_into().unwrap())
    };
    assert_eq!(read_u64(&account, 64), expected, "token account amount");
    assert_eq!(read_u64(&mint, 36), expected, "mint supply");
}
