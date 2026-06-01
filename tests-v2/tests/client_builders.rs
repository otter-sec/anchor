use {
    anchor_lang_v2::{InstructionData, ToAccountMetas},
    client_builders::{accounts, instruction},
    litesvm::{types::TransactionResult, LiteSVM},
    solana_keypair::Keypair,
    solana_message::{Message, VersionedMessage},
    solana_pubkey::Pubkey,
    solana_signer::Signer,
    solana_transaction::versioned::VersionedTransaction,
    tests_v2::{build_program, keypair_for},
};

fn program_id() -> Pubkey {
    "BF748KR4UhPq7xbhFQYd7yFKmh5UYdqed9GbD6oZvEyu"
        .parse()
        .unwrap()
}

fn other_program() -> Pubkey {
    "Gue5TpR6sstSyGhSvmVeH2TeKqBYYqmXpRCacB9jAk8u"
        .parse()
        .unwrap()
}

fn setup() -> (LiteSVM, Keypair, Keypair) {
    let test_dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let deploy_dir = test_dir.join("target/deploy");
    build_program(
        test_dir.join("programs/client-builders").to_str().unwrap(),
        deploy_dir.to_str().unwrap(),
    );

    let mut svm = LiteSVM::new();
    svm.add_program_from_file(program_id(), deploy_dir.join("client_builders.so"))
        .expect("load client_builders program");
    let payer = keypair_for("client-builders-payer");
    let authority = keypair_for("client-builders-authority");
    svm.airdrop(&payer.pubkey(), 10_000_000_000).unwrap();
    svm.airdrop(&authority.pubkey(), 1_000_000_000).unwrap();
    (svm, payer, authority)
}

fn send_ix(
    svm: &mut LiteSVM,
    ix: anchor_lang_v2::solana_program::instruction::Instruction,
    payer: &Keypair,
    extra_signers: &[&Keypair],
) -> TransactionResult {
    let blockhash = svm.latest_blockhash();
    let msg = Message::new_with_blockhash(&[ix], Some(&payer.pubkey()), &blockhash);
    let mut signers: Vec<&dyn solana_signer::Signer> = vec![payer];
    for signer in extra_signers {
        signers.push(*signer);
    }
    let tx = VersionedTransaction::try_new(VersionedMessage::Legacy(msg), &signers).unwrap();
    svm.send_transaction(tx)
}

fn vault_value(svm: &LiteSVM, vault: Pubkey) -> u64 {
    let account = svm.get_account(&vault).expect("vault exists");
    u64::from_le_bytes(account.data[8..16].try_into().unwrap())
}

fn external_pda() -> Pubkey {
    Pubkey::find_program_address(&[b"external"], &other_program()).0
}

#[test]
fn resolved_builder_derives_pda_program_id_and_sends_instruction() {
    let (mut svm, payer, authority) = setup();
    let (vault, bump) = accounts::InitializeVault::find_vault_address(&authority.pubkey());
    let ix = instruction::InitializeVault {}.to_instruction(accounts::InitializeVaultResolved {
        payer: payer.pubkey(),
        authority: authority.pubkey(),
    });

    assert_eq!(ix.program_id, program_id());
    let metas = accounts::InitializeVaultResolved {
        payer: payer.pubkey(),
        authority: authority.pubkey(),
    }
    .to_account_metas(None);
    assert_eq!(metas[1].pubkey, authority.pubkey());
    assert!(metas[0].is_signer);
    assert!(metas[0].is_writable);
    assert!(metas[1].is_signer);
    assert!(!metas[1].is_writable);
    assert_eq!(metas[2].pubkey, vault);
    assert!(!metas[2].is_signer);
    assert!(metas[2].is_writable);
    assert_eq!(metas[3].pubkey, solana_sdk_ids::system_program::ID);

    send_ix(&mut svm, ix, &payer, &[&authority]).expect("initialize via resolved builder");
    let account = svm.get_account(&vault).expect("vault created");
    assert_eq!(account.owner, program_id());
    assert_eq!(account.data[48], bump);
}

#[test]
fn seeds_program_override_drives_client_pda_derivation() {
    let (mut svm, payer, _) = setup();
    let (external_pda, _) = accounts::CheckExternalPda::find_external_pda_address();
    let local_pda = Pubkey::find_program_address(&[b"external"], &program_id()).0;

    assert_eq!(external_pda, self::external_pda());
    assert_ne!(external_pda, local_pda);

    let ix = instruction::CheckExternalPda {}.to_instruction(accounts::CheckExternalPdaResolved {});
    assert_eq!(ix.accounts[0].pubkey, external_pda);

    send_ix(&mut svm, ix, &payer, &[]).expect("external PDA derived with seeds::program");
}

#[test]
fn full_builder_and_instruction_data_update_state() {
    let (mut svm, payer, authority) = setup();
    let (vault, _) = accounts::InitializeVault::find_vault_address(&authority.pubkey());
    let init_ix =
        instruction::InitializeVault {}.to_instruction(accounts::InitializeVaultResolved {
            payer: payer.pubkey(),
            authority: authority.pubkey(),
        });
    send_ix(&mut svm, init_ix, &payer, &[&authority]).expect("initialize");

    let value_ix = instruction::SetValue { value: 77 }.to_instruction(accounts::SetValue {
        vault,
        authority: authority.pubkey(),
    });
    assert_eq!(value_ix.data, instruction::SetValue { value: 77 }.data());
    send_ix(&mut svm, value_ix, &payer, &[&authority]).expect("set value");
    assert_eq!(vault_value(&svm, vault), 77);
}

#[test]
fn fixed_size_instruction_args_work_through_generated_client_path() {
    let (mut svm, payer, authority) = setup();
    let (vault, _) = accounts::InitializeVault::find_vault_address(&authority.pubkey());
    send_ix(
        &mut svm,
        instruction::InitializeVault {}.to_instruction(accounts::InitializeVaultResolved {
            payer: payer.pubkey(),
            authority: authority.pubkey(),
        }),
        &payer,
        &[&authority],
    )
    .expect("initialize");

    let ix = instruction::SetWithDynamicArgs { label: *b"ok" }.to_instruction(accounts::SetValue {
        vault,
        authority: authority.pubkey(),
    });
    send_ix(&mut svm, ix, &payer, &[&authority]).expect("borrowed arg instruction");
    assert_eq!(vault_value(&svm, vault), 202);
}

#[test]
fn optional_none_builder_uses_program_id_sentinel() {
    let accounts = accounts::OptionalBuilderCaseResolved { user_state: None };
    let metas = accounts.to_account_metas(None);
    assert_eq!(metas.len(), 2);
    assert_eq!(metas[0].pubkey, program_id());
    assert!(!metas[0].is_writable);
    assert!(!metas[0].is_signer);
    assert_eq!(metas[1].pubkey, solana_sdk_ids::system_program::ID);
}

#[test]
fn generated_accounts_still_allow_runtime_validation_failures() {
    let (mut svm, payer, authority) = setup();
    let (vault, _) = accounts::InitializeVault::find_vault_address(&authority.pubkey());
    send_ix(
        &mut svm,
        instruction::InitializeVault {}.to_instruction(accounts::InitializeVaultResolved {
            payer: payer.pubkey(),
            authority: authority.pubkey(),
        }),
        &payer,
        &[&authority],
    )
    .expect("initialize");

    let wrong_vault = Pubkey::new_unique();
    assert_ne!(wrong_vault, vault);
    let ix = instruction::SetValue { value: 5 }.to_instruction(accounts::SetValue {
        vault: wrong_vault,
        authority: authority.pubkey(),
    });
    let result = send_ix(&mut svm, ix, &payer, &[&authority]);
    let err = format!("{:?}", result.unwrap_err().err);
    assert!(
        err.contains("InvalidSeeds")
            || err.contains("InvalidAccountData")
            || err.contains("UninitializedAccount")
            || err.contains("AccountDataTooSmall")
            || err.contains("Custom("),
        "wrong required account should fail validation, got: {err}"
    );
}
