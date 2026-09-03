//! Focused coverage for developer-facing `#[derive(InitSpace)]` usability:
//! module constants in `#[max_len(...)]`, primitive aliases, Address aliases,
//! arrays of aliases, and nested dynamic collections.

use {
    anchor_lang::solana_program::instruction::AccountMeta,
    litesvm::LiteSVM,
    solana_keypair::Keypair,
    solana_pubkey::Pubkey,
    solana_signer::Signer,
    tests_v2::{build_program, keypair_for, send_instruction},
};

fn program_id() -> Pubkey {
    "9AbShpmjP5WcQLSBW1NQmczpYVmT2CR2FLFoQdxxk47d"
        .parse()
        .unwrap()
}

fn profile_pda() -> Pubkey {
    Pubkey::find_program_address(&[b"profile"], &program_id()).0
}

fn nested_pda() -> Pubkey {
    Pubkey::find_program_address(&[b"nested"], &program_id()).0
}

fn image_pda() -> Pubkey {
    Pubkey::find_program_address(&[b"image"], &program_id()).0
}

fn setup() -> (LiteSVM, Keypair) {
    let test_dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let deploy_dir = test_dir.join("target/deploy");
    build_program(
        test_dir
            .join("programs/init-space-usability")
            .to_str()
            .unwrap(),
        deploy_dir.to_str().unwrap(),
    );

    let mut svm = LiteSVM::new();
    svm.add_program_from_file(program_id(), deploy_dir.join("init_space_usability.so"))
        .expect("load init_space_usability program");
    let payer = keypair_for("init-space-payer");
    svm.airdrop(&payer.pubkey(), 10_000_000_000).unwrap();
    (svm, payer)
}

#[test]
fn init_space_constants_accept_aliases_and_module_max_len() {
    let (mut svm, payer) = setup();
    send_instruction(&mut svm, program_id(), vec![0], vec![], &payer, &[])
        .expect("compile-time InitSpace constants should match expected sizes");
}

#[test]
fn profile_space_allocates_address_alias_and_module_string_bound() {
    let (mut svm, payer) = setup();
    let profile = profile_pda();
    let metas = vec![
        AccountMeta::new(payer.pubkey(), true),
        AccountMeta::new(profile, false),
        AccountMeta::new_readonly(solana_sdk_ids::system_program::ID, false),
    ];
    send_instruction(&mut svm, program_id(), vec![1], metas, &payer, &[])
        .expect("init_profile should allocate using Profile::INIT_SPACE");

    let account = svm.get_account(&profile).expect("profile exists");
    // Profile: owner alias to Address (32) + String max_len(limits::NAME=16)
    // encoded as 4 + 16 = 52, plus 8-byte discriminator.
    assert_eq!(account.data.len(), 8 + 52);
    assert_eq!(&account.data[8..40], payer.pubkey().as_ref());
    let name_len = u32::from_le_bytes(account.data[40..44].try_into().unwrap());
    assert_eq!(name_len, "init-space".len() as u32);
    assert_eq!(&account.data[44..44 + "init-space".len()], b"init-space");
}

#[test]
fn nested_vec_string_space_uses_both_module_constants() {
    let (mut svm, payer) = setup();
    let nested = nested_pda();
    let metas = vec![
        AccountMeta::new(payer.pubkey(), true),
        AccountMeta::new(nested, false),
        AccountMeta::new_readonly(solana_sdk_ids::system_program::ID, false),
    ];
    send_instruction(&mut svm, program_id(), vec![2], metas, &payer, &[])
        .expect("init_nested should allocate using Nested::INIT_SPACE");

    let account = svm.get_account(&nested).expect("nested exists");
    // Nested: Vec<String> max_len(limits::ITEMS=4, limits::NAME=16)
    // = 4 + (4 + 16) * 4 = 84, plus 8-byte discriminator.
    assert_eq!(account.data.len(), 8 + 84);
    let tags_len = u32::from_le_bytes(account.data[8..12].try_into().unwrap());
    assert_eq!(tags_len, 4);
}

#[test]
fn image_space_accepts_primitive_aliases_and_arrays() {
    let (mut svm, payer) = setup();
    let image = image_pda();
    let metas = vec![
        AccountMeta::new(payer.pubkey(), true),
        AccountMeta::new(image, false),
        AccountMeta::new_readonly(solana_sdk_ids::system_program::ID, false),
    ];
    send_instruction(&mut svm, program_id(), vec![3], metas, &payer, &[])
        .expect("init_image should allocate using Image::INIT_SPACE");

    let account = svm.get_account(&image).expect("image exists");
    // Image: Coordinate alias to i32 (4) + Coordinate alias to i32 (4)
    // + [u8; limits::ITEMS=4] = 12, plus 8-byte discriminator.
    assert_eq!(account.data.len(), 8 + 12);
    let width = i32::from_le_bytes(account.data[8..12].try_into().unwrap());
    let height = i32::from_le_bytes(account.data[12..16].try_into().unwrap());
    assert_eq!(width, 640);
    assert_eq!(height, 480);
    assert_eq!(&account.data[16..20], &[1, 2, 3, 4]);
}
