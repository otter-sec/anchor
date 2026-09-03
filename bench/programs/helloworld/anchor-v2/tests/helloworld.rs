use {
    anchor_lang::{accounts::Account, bytemuck, Space},
    anchor_v2_testing::{Keypair, LiteSVM, Message, Signer, VersionedMessage, VersionedTransaction},
    litesvm::types::{FailedTransactionMetadata, TransactionMetadata},
    hello_world_v2::{instruction, Counter},
};

type TxResult = Result<TransactionMetadata, FailedTransactionMetadata>;

fn setup() -> (LiteSVM, Keypair) {
    let mut svm = anchor_v2_testing::svm();
    let bytes = include_bytes!("../../../../target/deploy/hello_world_v2.so");
    svm.add_program(hello_world_v2::id(), bytes).unwrap();

    let payer = Keypair::new();
    svm.airdrop(&payer.pubkey(), 10_000_000_000).unwrap();

    (svm, payer)
}

fn send(svm: &mut LiteSVM, ix: anchor_lang::solana_program::instruction::Instruction, signers: &[&Keypair]) -> TxResult {
    let blockhash = svm.latest_blockhash();
    let msg = Message::new_with_blockhash(&[ix], Some(&signers[0].pubkey()), &blockhash);
    let tx = VersionedTransaction::try_new(VersionedMessage::Legacy(msg), signers).unwrap();
    svm.send_transaction(tx)
}

fn counter_address() -> (anchor_lang::Address, u8) {
    hello_world_v2::accounts::Init::find_counter_address()
}

#[test]
fn test_init() {
    let (mut svm, payer) = setup();
    let (counter_pda, _) = counter_address();

    let ix = instruction::Init {}.to_instruction(
        hello_world_v2::accounts::InitResolved { payer: payer.pubkey() },
    );
    let res = send(&mut svm, ix, &[&payer]).expect("init");
    println!("init CUs: {}", res.compute_units_consumed);

    let account = svm.get_account(&counter_pda).expect("counter account");
    assert_eq!(account.data.len(), <Account<Counter> as Space>::INIT_SPACE);
    let counter: &Counter = bytemuck::from_bytes(&account.data[8..]);
    assert_eq!(counter.value, 42);
}
