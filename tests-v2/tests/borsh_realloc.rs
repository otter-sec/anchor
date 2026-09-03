use {
    anchor_lang::{solana_program::instruction::AccountMeta, InstructionData},
    litesvm::LiteSVM,
    solana_pubkey::Pubkey,
    solana_signer::Signer,
    tests_v2::{build_program, keypair_for, send_instruction},
};

fn program_id() -> Pubkey {
    "D51usz545PmMTSqE18F1YSj1RXqvpPhKUUxB6wHPNewT"
        .parse()
        .unwrap()
}

fn setup() -> (LiteSVM, solana_keypair::Keypair) {
    let test_dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let deploy_dir = test_dir.join("target/deploy");
    let deploy_str = deploy_dir.to_str().unwrap();

    build_program(
        test_dir.join("programs/borsh-realloc").to_str().unwrap(),
        deploy_str,
    );

    let mut svm = LiteSVM::new();
    svm.add_program_from_file(program_id(), &deploy_dir.join("borsh_realloc.so"))
        .expect("failed to load borsh-realloc program");

    let payer = keypair_for("borsh-realloc-payer");
    svm.airdrop(&payer.pubkey(), 10_000_000_000).unwrap();

    (svm, payer)
}

fn data_pda() -> Pubkey {
    Pubkey::find_program_address(&[b"data"], &program_id()).0
}

fn init_data(svm: &mut LiteSVM, payer: &solana_keypair::Keypair) -> Pubkey {
    let pda = data_pda();
    let init_data = borsh_realloc::instruction::Initialize {}.data();
    let init_metas = vec![
        AccountMeta::new(payer.pubkey(), true),
        AccountMeta::new(pda, false),
        AccountMeta::new_readonly(solana_sdk_ids::system_program::ID, false),
    ];
    send_instruction(svm, program_id(), init_data, init_metas, payer, &[])
        .expect("initialize should succeed");
    pda
}

/// Read the borsh Vec<u8> items from the account data.
/// Layout: [disc: 8][borsh_vec_len: 4 LE][items: N bytes]
fn read_items(svm: &LiteSVM, pda: &Pubkey) -> Vec<u8> {
    let account = svm.get_account(pda).expect("account should exist");
    let data = &account.data;
    assert!(data.len() >= 12, "account too small");
    let vec_len = u32::from_le_bytes(data[8..12].try_into().unwrap()) as usize;
    assert!(
        data.len() >= 12 + vec_len,
        "account data shorter than borsh vec"
    );
    data[12..12 + vec_len].to_vec()
}

#[test]
fn test_borsh_realloc_grow() {
    let (mut svm, payer) = setup();
    let pda = init_data(&mut svm, &payer);

    let items = read_items(&svm, &pda);
    assert_eq!(items, vec![1, 2, 3], "initial data should be [1,2,3]");

    // 2. Grow: realloc and set items = [1, 2, 3, 4, 5, 6, 7, 8, 9, 10]
    let new_items: Vec<u8> = (1..=10).collect();
    let grow_data = borsh_realloc::instruction::Grow {
        new_items: new_items.clone(),
    }
    .data();
    let grow_metas = vec![
        AccountMeta::new(payer.pubkey(), true),
        AccountMeta::new(pda, false),
        AccountMeta::new_readonly(solana_sdk_ids::system_program::ID, false),
    ];
    send_instruction(&mut svm, program_id(), grow_data, grow_metas, &payer, &[])
        .expect("grow should succeed");

    let items = read_items(&svm, &pda);
    assert_eq!(items, new_items, "data should be [1..=10] after grow");
}

#[test]
fn test_borsh_realloc_shrink() {
    let (mut svm, payer) = setup();
    let pda = init_data(&mut svm, &payer);

    // 2. Grow first so we have room to shrink
    let big_items: Vec<u8> = (1..=10).collect();
    let grow_data = borsh_realloc::instruction::Grow {
        new_items: big_items.clone(),
    }
    .data();
    let grow_metas = vec![
        AccountMeta::new(payer.pubkey(), true),
        AccountMeta::new(pda, false),
        AccountMeta::new_readonly(solana_sdk_ids::system_program::ID, false),
    ];
    send_instruction(&mut svm, program_id(), grow_data, grow_metas, &payer, &[])
        .expect("grow should succeed");

    // 3. Shrink: realloc down and set items = [1, 2]
    let small_items: Vec<u8> = vec![1, 2];
    let shrink_data = borsh_realloc::instruction::Shrink {
        new_items: small_items.clone(),
    }
    .data();
    let shrink_metas = vec![
        AccountMeta::new(payer.pubkey(), true),
        AccountMeta::new(pda, false),
        AccountMeta::new_readonly(solana_sdk_ids::system_program::ID, false),
    ];
    send_instruction(
        &mut svm,
        program_id(),
        shrink_data,
        shrink_metas,
        &payer,
        &[],
    )
    .expect("shrink should succeed");

    let items = read_items(&svm, &pda);
    assert_eq!(items, small_items, "data should be [1,2] after shrink");
}

#[test]
fn test_borsh_realloc_shrink_to_empty() {
    let (mut svm, payer) = setup();
    let pda = init_data(&mut svm, &payer);
    assert_eq!(read_items(&svm, &pda), vec![1, 2, 3]);

    // Shrink to empty vec
    let shrink_data = borsh_realloc::instruction::Shrink { new_items: vec![] }.data();
    send_instruction(
        &mut svm,
        program_id(),
        shrink_data,
        vec![
            AccountMeta::new(payer.pubkey(), true),
            AccountMeta::new(pda, false),
            AccountMeta::new_readonly(solana_sdk_ids::system_program::ID, false),
        ],
        &payer,
        &[],
    )
    .expect("shrink to empty");

    let items = read_items(&svm, &pda);
    assert!(
        items.is_empty(),
        "items should be empty after shrink to zero"
    );

    // Verify account size: disc(8) + vec_len(4) + 0 data = 12
    let account = svm.get_account(&pda).unwrap();
    assert_eq!(account.data.len(), 12, "account should be exactly 12 bytes");
}

#[test]
fn test_borsh_realloc_grow_shrink_grow_roundtrip() {
    let (mut svm, payer) = setup();
    let pda = init_data(&mut svm, &payer);

    // Grow to [1..=20]
    let big: Vec<u8> = (1..=20).collect();
    let grow_data = borsh_realloc::instruction::Grow {
        new_items: big.clone(),
    }
    .data();
    send_instruction(
        &mut svm,
        program_id(),
        grow_data,
        vec![
            AccountMeta::new(payer.pubkey(), true),
            AccountMeta::new(pda, false),
            AccountMeta::new_readonly(solana_sdk_ids::system_program::ID, false),
        ],
        &payer,
        &[],
    )
    .expect("grow to 20");
    assert_eq!(read_items(&svm, &pda), big);

    // Shrink to [10, 20]
    let small: Vec<u8> = vec![10, 20];
    let shrink_data = borsh_realloc::instruction::Shrink {
        new_items: small.clone(),
    }
    .data();
    send_instruction(
        &mut svm,
        program_id(),
        shrink_data,
        vec![
            AccountMeta::new(payer.pubkey(), true),
            AccountMeta::new(pda, false),
            AccountMeta::new_readonly(solana_sdk_ids::system_program::ID, false),
        ],
        &payer,
        &[],
    )
    .expect("shrink to 2");
    assert_eq!(read_items(&svm, &pda), small);

    // Grow again to [100..=110]
    let regrown: Vec<u8> = (100..=110).collect();
    let grow2_data = borsh_realloc::instruction::Grow {
        new_items: regrown.clone(),
    }
    .data();
    send_instruction(
        &mut svm,
        program_id(),
        grow2_data,
        vec![
            AccountMeta::new(payer.pubkey(), true),
            AccountMeta::new(pda, false),
            AccountMeta::new_readonly(solana_sdk_ids::system_program::ID, false),
        ],
        &payer,
        &[],
    )
    .expect("re-grow to 11");
    assert_eq!(
        read_items(&svm, &pda),
        regrown,
        "data should survive grow-shrink-grow roundtrip"
    );
}

#[test]
fn test_borsh_realloc_grow_from_empty() {
    let (mut svm, payer) = setup();
    let pda = init_data(&mut svm, &payer);

    // Shrink to empty
    let shrink_data = borsh_realloc::instruction::Shrink { new_items: vec![] }.data();
    send_instruction(
        &mut svm,
        program_id(),
        shrink_data,
        vec![
            AccountMeta::new(payer.pubkey(), true),
            AccountMeta::new(pda, false),
            AccountMeta::new_readonly(solana_sdk_ids::system_program::ID, false),
        ],
        &payer,
        &[],
    )
    .expect("shrink to empty");
    assert!(read_items(&svm, &pda).is_empty());

    // Grow from empty to [42, 43, 44, 45, 46]
    let items: Vec<u8> = vec![42, 43, 44, 45, 46];
    let grow_data = borsh_realloc::instruction::Grow {
        new_items: items.clone(),
    }
    .data();
    send_instruction(
        &mut svm,
        program_id(),
        grow_data,
        vec![
            AccountMeta::new(payer.pubkey(), true),
            AccountMeta::new(pda, false),
            AccountMeta::new_readonly(solana_sdk_ids::system_program::ID, false),
        ],
        &payer,
        &[],
    )
    .expect("grow from empty");
    assert_eq!(
        read_items(&svm, &pda),
        items,
        "should grow correctly from empty state"
    );
}

#[test]
fn init_rejects_wrong_pda() {
    let (mut svm, payer) = setup();
    let wrong = Pubkey::new_unique();

    let init_data = borsh_realloc::instruction::Initialize {}.data();
    let result = send_instruction(
        &mut svm,
        program_id(),
        init_data,
        vec![
            AccountMeta::new(payer.pubkey(), true),
            AccountMeta::new(wrong, false),
            AccountMeta::new_readonly(solana_sdk_ids::system_program::ID, false),
        ],
        &payer,
        &[],
    );
    assert!(result.is_err(), "init must reject a non-canonical PDA");
    assert!(
        svm.get_account(&wrong).is_none(),
        "wrong PDA must not be created on failed init"
    );
}

#[test]
fn init_rejects_existing_pda() {
    let (mut svm, payer) = setup();
    let pda = init_data(&mut svm, &payer);
    svm.expire_blockhash();

    let init_data = borsh_realloc::instruction::Initialize {}.data();
    let result = send_instruction(
        &mut svm,
        program_id(),
        init_data,
        vec![
            AccountMeta::new(payer.pubkey(), true),
            AccountMeta::new(pda, false),
            AccountMeta::new_readonly(solana_sdk_ids::system_program::ID, false),
        ],
        &payer,
        &[],
    );
    assert!(
        result.is_err(),
        "init must reject an already-initialized PDA"
    );
    assert_eq!(
        read_items(&svm, &pda),
        vec![1, 2, 3],
        "failed duplicate init must leave account data unchanged"
    );
}

#[test]
fn grow_rejects_uninitialized_pda() {
    let (mut svm, payer) = setup();
    let pda = data_pda();

    let grow_data = borsh_realloc::instruction::Grow {
        new_items: vec![9, 9, 9],
    }
    .data();
    let result = send_instruction(
        &mut svm,
        program_id(),
        grow_data,
        vec![
            AccountMeta::new(payer.pubkey(), true),
            AccountMeta::new(pda, false),
            AccountMeta::new_readonly(solana_sdk_ids::system_program::ID, false),
        ],
        &payer,
        &[],
    );
    assert!(result.is_err(), "grow must reject an uninitialized account");
}

#[test]
fn grow_rejects_wrong_pda_and_preserves_data() {
    let (mut svm, payer) = setup();
    let pda = init_data(&mut svm, &payer);
    let wrong = Pubkey::new_unique();

    let grow_data = borsh_realloc::instruction::Grow {
        new_items: vec![4, 5, 6],
    }
    .data();
    let result = send_instruction(
        &mut svm,
        program_id(),
        grow_data,
        vec![
            AccountMeta::new(payer.pubkey(), true),
            AccountMeta::new(wrong, false),
            AccountMeta::new_readonly(solana_sdk_ids::system_program::ID, false),
        ],
        &payer,
        &[],
    );
    assert!(result.is_err(), "grow must reject a non-canonical PDA");
    assert_eq!(
        read_items(&svm, &pda),
        vec![1, 2, 3],
        "failed grow must not mutate the canonical account"
    );
}

#[test]
fn shrink_rejects_wrong_pda_and_preserves_data() {
    let (mut svm, payer) = setup();
    let pda = init_data(&mut svm, &payer);
    let wrong = Pubkey::new_unique();

    let shrink_data = borsh_realloc::instruction::Shrink { new_items: vec![] }.data();
    let result = send_instruction(
        &mut svm,
        program_id(),
        shrink_data,
        vec![
            AccountMeta::new(payer.pubkey(), true),
            AccountMeta::new(wrong, false),
            AccountMeta::new_readonly(solana_sdk_ids::system_program::ID, false),
        ],
        &payer,
        &[],
    );
    assert!(result.is_err(), "shrink must reject a non-canonical PDA");
    assert_eq!(
        read_items(&svm, &pda),
        vec![1, 2, 3],
        "failed shrink must not mutate the canonical account"
    );
}

#[test]
fn shrink_below_discriminator_is_rejected_and_rolls_back() {
    let (mut svm, payer) = setup();
    let pda = init_data(&mut svm, &payer);
    let account_before = svm.get_account(&pda).expect("data exists before");
    let items_before = read_items(&svm, &pda);

    let shrink_data = borsh_realloc::instruction::ShrinkBelowDiscriminator {}.data();
    let result = send_instruction(
        &mut svm,
        program_id(),
        shrink_data,
        vec![
            AccountMeta::new(payer.pubkey(), true),
            AccountMeta::new(pda, false),
            AccountMeta::new_readonly(solana_sdk_ids::system_program::ID, false),
        ],
        &payer,
        &[],
    );

    assert!(
        result.is_err(),
        "realloc below the discriminator length must be rejected"
    );
    let account_after = svm.get_account(&pda).expect("data still exists");
    assert_eq!(
        account_after.data.len(),
        account_before.data.len(),
        "failed realloc must roll back the account size"
    );
    assert_eq!(
        read_items(&svm, &pda),
        items_before,
        "failed realloc must preserve serialized data"
    );
}
