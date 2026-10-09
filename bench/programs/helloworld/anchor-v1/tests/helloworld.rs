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
    hello_world::{instruction, Counter},
    solana_pubkey::Pubkey,
};

type TxResult = Result<TransactionMetadata, FailedTransactionMetadata>;

fn setup() -> (LiteSVM, Keypair) {
    let mut svm = anchor_v2_testing::svm();
    let bytes = include_bytes!("../../../../target/deploy/hello_world.so");
    svm.add_program(hello_world::id(), bytes).unwrap();

    let payer = Keypair::new();
    svm.airdrop(&payer.pubkey(), 10_000_000_000).unwrap();

    (svm, payer)
}

fn send(svm: &mut LiteSVM, ix: Instruction, signers: &[&Keypair]) -> TxResult {
    let blockhash = svm.latest_blockhash();
    let msg = Message::new_with_blockhash(&[ix], Some(&signers[0].pubkey()), &blockhash);
    let tx = VersionedTransaction::try_new(VersionedMessage::Legacy(msg), signers).unwrap();
    svm.send_transaction(tx)
}

fn counter_address() -> (Pubkey, u8) {
    Pubkey::find_program_address(&[b"counter"], &hello_world::id())
}

#[test]
fn test_init() {
    let (mut svm, payer) = setup();
    let (counter, _) = counter_address();

    let ix = Instruction {
        program_id: hello_world::id(),
        data: instruction::Init {}.data(),
        accounts: hello_world::accounts::Init {
            payer: payer.pubkey(),
            counter,
            system_program: system_program::ID,
        }
        .to_account_metas(None),
    };
    let res = send(&mut svm, ix, &[&payer]).expect("init");
    println!("init CUs: {}", res.compute_units_consumed);

    let account = svm.get_account(&counter).expect("counter account");
    let state = Counter::try_deserialize(&mut account.data.as_slice()).expect("decode counter");
    assert_eq!(state.value, 42);
}
