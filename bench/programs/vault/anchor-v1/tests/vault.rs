use {
    anchor_lang::{
        solana_program::{instruction::Instruction, system_program},
        InstructionData, ToAccountMetas,
    },
    anchor_v2_testing::{
        litesvm::{
            types::{FailedTransactionMetadata, TransactionMetadata},
            LiteSVM,
        },
        Keypair, Message, Signer, VersionedMessage, VersionedTransaction,
    },
    solana_account::Account as SolanaAccount,
    solana_pubkey::Pubkey,
    vault_v1::instruction,
};

type TxResult = Result<TransactionMetadata, FailedTransactionMetadata>;

fn setup() -> (LiteSVM, Keypair) {
    let mut svm = anchor_v2_testing::svm();
    let bytes = include_bytes!("../../../../target/deploy/vault_v1.so");
    svm.add_program(vault_v1::id(), bytes).unwrap();

    let user = Keypair::new();
    svm.airdrop(&user.pubkey(), 10_000_000_000).unwrap();

    (svm, user)
}

fn send(svm: &mut LiteSVM, ix: Instruction, signers: &[&Keypair]) -> TxResult {
    let blockhash = svm.latest_blockhash();
    let msg = Message::new_with_blockhash(&[ix], Some(&signers[0].pubkey()), &blockhash);
    let tx = VersionedTransaction::try_new(VersionedMessage::Legacy(msg), signers).unwrap();
    svm.send_transaction(tx)
}

fn vault_address(user: &Pubkey) -> (Pubkey, u8) {
    Pubkey::find_program_address(&[b"vault", user.as_ref()], &vault_v1::id())
}

#[test]
fn test_deposit() {
    let (mut svm, user) = setup();
    let (vault, _) = vault_address(&user.pubkey());

    let ix = Instruction {
        program_id: vault_v1::id(),
        data: instruction::Deposit { amount: 1_000_000 }.data(),
        accounts: vault_v1::accounts::Deposit {
            user: user.pubkey(),
            vault,
            system_program: system_program::ID,
        }
        .to_account_metas(None),
    };
    let res = send(&mut svm, ix, &[&user]).expect("deposit");
    println!("deposit CUs: {}", res.compute_units_consumed);

    assert!(svm.get_account(&vault).unwrap().lamports >= 1_000_000);
}

#[test]
fn test_withdraw() {
    let (mut svm, user) = setup();
    let (vault, _) = vault_address(&user.pubkey());

    svm.set_account(
        vault,
        SolanaAccount {
            lamports: 1_000_000_000,
            data: vec![],
            owner: vault_v1::id(),
            executable: false,
            rent_epoch: 0,
        },
    )
    .unwrap();

    let ix = Instruction {
        program_id: vault_v1::id(),
        data: instruction::Withdraw { amount: 500_000 }.data(),
        accounts: vault_v1::accounts::Withdraw {
            user: user.pubkey(),
            vault,
        }
        .to_account_metas(None),
    };
    let res = send(&mut svm, ix, &[&user]).expect("withdraw");
    println!("withdraw CUs: {}", res.compute_units_consumed);

    assert_eq!(svm.get_account(&vault).unwrap().lamports, 999_500_000);
}
