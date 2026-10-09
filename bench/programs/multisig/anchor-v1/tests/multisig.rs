use {
    anchor_lang::{
        solana_program::{
            instruction::{AccountMeta, Instruction},
            rent, system_program,
        },
        InstructionData, ToAccountMetas,
    },
    anchor_v2_testing::{
        litesvm::{
            types::{FailedTransactionMetadata, TransactionMetadata},
            LiteSVM,
        },
        Keypair, Message, Signer, VersionedMessage, VersionedTransaction,
    },
    multisig_v1::instruction,
    solana_pubkey::Pubkey,
};

type TxResult = Result<TransactionMetadata, FailedTransactionMetadata>;

fn setup() -> (LiteSVM, Keypair) {
    let mut svm = anchor_v2_testing::svm();
    let bytes = include_bytes!("../../../../target/deploy/multisig_v1.so");
    svm.add_program(multisig_v1::id(), bytes).unwrap();

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

fn config_address(creator: &Pubkey) -> (Pubkey, u8) {
    Pubkey::find_program_address(&[b"multisig", creator.as_ref()], &multisig_v1::id())
}

fn vault_address(config: &Pubkey) -> (Pubkey, u8) {
    Pubkey::find_program_address(&[b"vault", config.as_ref()], &multisig_v1::id())
}

fn create_multisig(svm: &mut LiteSVM, creator: &Keypair, signers: &[&Keypair], threshold: u8) {
    let (config, _) = config_address(&creator.pubkey());
    let mut metas = multisig_v1::accounts::Create {
        creator: creator.pubkey(),
        config,
        rent: rent::ID,
        system_program: system_program::ID,
    }
    .to_account_metas(None);
    for s in signers {
        metas.push(AccountMeta::new_readonly(s.pubkey(), true));
    }

    let ix = Instruction {
        program_id: multisig_v1::id(),
        data: instruction::Create { threshold }.data(),
        accounts: metas,
    };

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
    assert!(svm.get_account(&config).expect("config").data.len() > 0);
}

#[test]
fn test_deposit() {
    let (mut svm, creator) = setup();
    let signer_one = Keypair::new();
    create_multisig(&mut svm, &creator, &[&signer_one], 1);

    let (config, _) = config_address(&creator.pubkey());
    let (vault, _) = vault_address(&config);

    let ix = Instruction {
        program_id: multisig_v1::id(),
        data: instruction::Deposit { amount: 1_000_000 }.data(),
        accounts: multisig_v1::accounts::Deposit {
            depositor: creator.pubkey(),
            config,
            vault,
            system_program: system_program::ID,
        }
        .to_account_metas(None),
    };
    let res = send(&mut svm, ix, &[&creator]).expect("deposit");
    println!("deposit CUs: {}", res.compute_units_consumed);
}

#[test]
fn test_set_label() {
    let (mut svm, creator) = setup();
    let signer_one = Keypair::new();
    create_multisig(&mut svm, &creator, &[&signer_one], 1);

    let (config, _) = config_address(&creator.pubkey());

    let ix = Instruction {
        program_id: multisig_v1::id(),
        data: instruction::SetLabel {
            label: "test-label".to_owned(),
        }
        .data(),
        accounts: multisig_v1::accounts::SetLabel {
            creator: creator.pubkey(),
            config,
            system_program: system_program::ID,
        }
        .to_account_metas(None),
    };
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
    let (vault, _) = vault_address(&config);
    let recipient = Keypair::new();
    svm.airdrop(&recipient.pubkey(), 1_000_000).unwrap();

    let ix = Instruction {
        program_id: multisig_v1::id(),
        data: instruction::Deposit { amount: 2_000_000 }.data(),
        accounts: multisig_v1::accounts::Deposit {
            depositor: creator.pubkey(),
            config,
            vault,
            system_program: system_program::ID,
        }
        .to_account_metas(None),
    };
    send(&mut svm, ix, &[&creator]).expect("deposit");

    let mut metas = multisig_v1::accounts::ExecuteTransfer {
        config,
        creator: creator.pubkey(),
        vault,
        recipient: recipient.pubkey(),
        system_program: system_program::ID,
    }
    .to_account_metas(None);
    metas.push(AccountMeta::new_readonly(signer_one.pubkey(), true));
    metas.push(AccountMeta::new_readonly(signer_two.pubkey(), true));

    let ix = Instruction {
        program_id: multisig_v1::id(),
        data: instruction::ExecuteTransfer { amount: 500_000 }.data(),
        accounts: metas,
    };
    let res = send(&mut svm, ix, &[&creator, &signer_one, &signer_two]).expect("execute_transfer");
    println!("execute_transfer CUs: {}", res.compute_units_consumed);
}
