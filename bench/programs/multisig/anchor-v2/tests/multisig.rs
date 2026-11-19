use {
    anchor_lang::solana_program::instruction::{AccountMeta, Instruction},
    anchor_v2_testing::{
        litesvm::{
            types::{FailedTransactionMetadata, TransactionMetadata},
            LiteSVM,
        },
        Keypair, Message, Signer, VersionedMessage, VersionedTransaction,
    },
    multisig_v2::instruction,
};

type TxResult = Result<TransactionMetadata, FailedTransactionMetadata>;

fn setup() -> (LiteSVM, Keypair) {
    let mut svm = anchor_v2_testing::svm();
    let bytes = include_bytes!("../../../../target/deploy/multisig_v2.so");
    svm.add_program(multisig_v2::id(), bytes).unwrap();

    let creator = Keypair::new();
    svm.airdrop(&creator.pubkey(), 10_000_000_000).unwrap();

    (svm, creator)
}

fn send(svm: &mut LiteSVM, ix: Instruction, signers: &[&Keypair]) -> TxResult {
    let blockhash = svm.latest_blockhash();
    let msg = Message::new_with_blockhash(&[ix], Some(&signers[0].pubkey()), &blockhash);
    let tx = VersionedTransaction::try_new(VersionedMessage::Legacy(msg), signers).unwrap();
    svm.send_transaction(tx)
}

fn config_address(creator: &anchor_lang::Address) -> (anchor_lang::Address, u8) {
    multisig_v2::accounts::Create::find_config_address(creator)
}

fn create_multisig(svm: &mut LiteSVM, creator: &Keypair, signers: &[&Keypair], threshold: u8) {
    let mut ix =
        instruction::Create { threshold }.to_instruction(multisig_v2::accounts::CreateResolved {
            creator: creator.pubkey(),
        });
    for s in signers {
        ix.accounts
            .push(AccountMeta::new_readonly(s.pubkey(), true));
    }

    let mut all_signers: Vec<&Keypair> = vec![creator];
    all_signers.extend_from_slice(signers);
    send(svm, ix, &all_signers).expect("create multisig");
}

#[test]
fn test_create() {
    let (mut svm, creator) = setup();
    let signer_one = Keypair::new();
    let signer_two = Keypair::new();

    create_multisig(&mut svm, &creator, &[&signer_one, &signer_two], 2);

    let (config, _) = config_address(&creator.pubkey());
    let account = svm.get_account(&config).expect("config account");
    assert!(account.data.len() > 0);
}

#[test]
fn test_deposit() {
    let (mut svm, creator) = setup();
    let signer_one = Keypair::new();
    create_multisig(&mut svm, &creator, &[&signer_one], 1);

    let (config, _) = config_address(&creator.pubkey());

    let ix = instruction::Deposit { amount: 1_000_000 }.to_instruction(
        multisig_v2::accounts::DepositResolved {
            depositor: creator.pubkey(),
            config,
        },
    );
    let res = send(&mut svm, ix, &[&creator]).expect("deposit");
    println!("deposit CUs: {}", res.compute_units_consumed);
}

#[test]
fn test_set_label() {
    let (mut svm, creator) = setup();
    let signer_one = Keypair::new();
    create_multisig(&mut svm, &creator, &[&signer_one], 1);

    let label_bytes = b"test-label";
    let mut label = [0u8; 32];
    label[..label_bytes.len()].copy_from_slice(label_bytes);

    let (config, _) = config_address(&creator.pubkey());
    let ix = instruction::SetLabel {
        label_len: label_bytes.len() as u8,
        label,
    }
    .to_instruction(multisig_v2::accounts::SetLabelResolved {
        creator: creator.pubkey(),
        config,
    });
    let res = send(&mut svm, ix, &[&creator]).expect("set_label");
    println!("set_label CUs: {}", res.compute_units_consumed);
}

#[test]
fn test_execute_transfer() {
    let (mut svm, creator) = setup();
    let signer_one = Keypair::new();
    let signer_two = Keypair::new();
    create_multisig(&mut svm, &creator, &[&signer_one, &signer_two], 2);

    let (config, _) = config_address(&creator.pubkey());
    let recipient = Keypair::new();
    svm.airdrop(&recipient.pubkey(), 1_000_000).unwrap();

    // Deposit first
    let ix = instruction::Deposit { amount: 2_000_000 }.to_instruction(
        multisig_v2::accounts::DepositResolved {
            depositor: creator.pubkey(),
            config,
        },
    );
    send(&mut svm, ix, &[&creator]).expect("deposit");

    // Execute transfer
    let mut ix = instruction::ExecuteTransfer { amount: 500_000 }.to_instruction(
        multisig_v2::accounts::ExecuteTransferResolved {
            config,
            creator: creator.pubkey(),
            recipient: recipient.pubkey(),
        },
    );
    ix.accounts
        .push(AccountMeta::new_readonly(signer_one.pubkey(), true));
    ix.accounts
        .push(AccountMeta::new_readonly(signer_two.pubkey(), true));
    let res = send(&mut svm, ix, &[&creator, &signer_one, &signer_two]).expect("execute_transfer");
    println!("execute_transfer CUs: {}", res.compute_units_consumed);
}
