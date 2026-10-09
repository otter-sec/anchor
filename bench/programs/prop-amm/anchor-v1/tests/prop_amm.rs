use {
    anchor_lang::{
        solana_program::{instruction::Instruction, system_program},
        AccountDeserialize, InstructionData, ToAccountMetas,
    },
    anchor_v2_testing::{
        litesvm::{
            types::{FailedTransactionMetadata, TransactionMetadata},
            LiteSVM,
        },
        Keypair, Message, Signer, VersionedMessage, VersionedTransaction,
    },
    prop_amm_v1::{instruction, Oracle},
};

type TxResult = Result<TransactionMetadata, FailedTransactionMetadata>;

fn setup() -> (LiteSVM, Keypair, Keypair) {
    let mut svm = anchor_v2_testing::svm();
    let bytes = include_bytes!("../../../../target/deploy/prop_amm_v1.so");
    svm.add_program(prop_amm_v1::id(), bytes).unwrap();

    let payer = Keypair::new();
    let oracle = Keypair::new();
    svm.airdrop(&payer.pubkey(), 1_000_000_000).unwrap();

    (svm, payer, oracle)
}

fn send(svm: &mut LiteSVM, ix: Instruction, signers: &[&Keypair]) -> TxResult {
    let blockhash = svm.latest_blockhash();
    let msg = Message::new_with_blockhash(&[ix], Some(&signers[0].pubkey()), &blockhash);
    let tx = VersionedTransaction::try_new(VersionedMessage::Legacy(msg), signers).unwrap();
    svm.send_transaction(tx)
}

fn read_oracle(svm: &LiteSVM, oracle: &Keypair) -> Oracle {
    let account = svm.get_account(&oracle.pubkey()).expect("oracle account");
    Oracle::try_deserialize(&mut account.data.as_slice()).expect("decode oracle")
}

fn init_oracle(svm: &mut LiteSVM, payer: &Keypair, oracle: &Keypair) {
    let ix = Instruction {
        program_id: prop_amm_v1::id(),
        data: instruction::Initialize {}.data(),
        accounts: prop_amm_v1::accounts::Initialize {
            payer: payer.pubkey(),
            oracle: oracle.pubkey(),
            system_program: system_program::ID,
        }
        .to_account_metas(None),
    };
    send(svm, ix, &[payer, oracle]).expect("initialize");
}

#[test]
fn test_initialize() {
    let (mut svm, payer, oracle) = setup();
    init_oracle(&mut svm, &payer, &oracle);

    let state = read_oracle(&svm, &oracle);
    assert_eq!(state.authority, payer.pubkey());
    assert_eq!(state.price, 0);
}

#[test]
fn test_update() {
    let (mut svm, payer, oracle) = setup();
    init_oracle(&mut svm, &payer, &oracle);

    let ix = Instruction {
        program_id: prop_amm_v1::id(),
        data: instruction::Update { new_price: 1234 }.data(),
        accounts: prop_amm_v1::accounts::Update {
            oracle: oracle.pubkey(),
            authority: payer.pubkey(),
        }
        .to_account_metas(None),
    };
    let res = send(&mut svm, ix, &[&payer]).expect("update");
    println!("update CUs: {}", res.compute_units_consumed);

    assert_eq!(read_oracle(&svm, &oracle).price, 1234);
}

#[test]
fn test_update_rejects_wrong_authority() {
    let (mut svm, payer, oracle) = setup();
    init_oracle(&mut svm, &payer, &oracle);

    let wrong = Keypair::new();
    svm.airdrop(&wrong.pubkey(), 1_000_000_000).unwrap();

    let ix = Instruction {
        program_id: prop_amm_v1::id(),
        data: instruction::Update { new_price: 9999 }.data(),
        accounts: prop_amm_v1::accounts::Update {
            oracle: oracle.pubkey(),
            authority: wrong.pubkey(),
        }
        .to_account_metas(None),
    };
    let res = send(&mut svm, ix, &[&wrong]);
    assert!(res.is_err(), "wrong authority must be rejected");
    assert_eq!(read_oracle(&svm, &oracle).price, 0);
}

#[test]
fn test_rotate_authority() {
    let (mut svm, payer, oracle) = setup();
    init_oracle(&mut svm, &payer, &oracle);

    let new_auth = Keypair::new();
    let ix = Instruction {
        program_id: prop_amm_v1::id(),
        data: instruction::RotateAuthority {
            new_authority: new_auth.pubkey(),
        }
        .data(),
        accounts: prop_amm_v1::accounts::RotateAuthority {
            oracle: oracle.pubkey(),
            authority: payer.pubkey(),
        }
        .to_account_metas(None),
    };
    send(&mut svm, ix, &[&payer]).expect("rotate");

    assert_eq!(read_oracle(&svm, &oracle).authority, new_auth.pubkey());
}
