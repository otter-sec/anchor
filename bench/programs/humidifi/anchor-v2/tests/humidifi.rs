use {
    anchor_v2_testing::{
        litesvm::{
            types::{FailedTransactionMetadata, TransactionMetadata},
            LiteSVM,
        },
        Keypair, Message, Signer, VersionedMessage, VersionedTransaction,
    },
    solana_account::Account,
    solana_instruction::{AccountMeta, Instruction},
    solana_pubkey::{pubkey, Pubkey},
};

const HUMIDIFI: Pubkey = pubkey!("9H6tua7jkLhdm3w8BvgpTn5LZNU7g4ZynDmCiNN3q6Rp");
const CLOCK_SYSVAR: Pubkey = pubkey!("SysvarC1ock11111111111111111111111111111111");
const CONST_SLOT_BOUND: u64 = 0x6edde0930b59ebea;

fn oracle_account_data(authority_signer: Pubkey) -> Vec<u8> {
    let mut data = vec![0u8; 1728];
    data[512..544].copy_from_slice(&authority_signer.to_bytes());
    data
}

fn setup() -> (LiteSVM, Keypair, Keypair) {
    let mut svm = anchor_v2_testing::svm();
    let bytes = include_bytes!("../humidifi.so");
    svm.add_program(HUMIDIFI, bytes).unwrap();

    let payer = Keypair::new();
    svm.airdrop(&payer.pubkey(), 10_000_000_000).unwrap();

    let oracle = Keypair::new();
    let acct = Account {
        lamports: 100_000_000,
        data: oracle_account_data(payer.pubkey()),
        owner: HUMIDIFI,
        executable: false,
        rent_epoch: 0,
    };
    svm.set_account(oracle.pubkey(), acct).unwrap();

    (svm, payer, oracle)
}

fn send(
    svm: &mut LiteSVM,
    ix: Instruction,
    payer: &Keypair,
) -> Result<TransactionMetadata, FailedTransactionMetadata> {
    let blockhash = svm.latest_blockhash();
    let msg = Message::new_with_blockhash(&[ix], Some(&payer.pubkey()), &blockhash);
    let tx = VersionedTransaction::try_new(VersionedMessage::Legacy(msg), &[payer]).unwrap();
    svm.send_transaction(tx)
}

#[test]
fn oracle_update() {
    let (mut svm, payer, oracle) = setup();

    let mut data = vec![0u8; 65];
    data[24..32].copy_from_slice(&1u64.to_le_bytes());
    data[40..48].copy_from_slice(&(u64::MAX ^ CONST_SLOT_BOUND).to_le_bytes());

    let ix = Instruction {
        program_id: HUMIDIFI,
        accounts: vec![
            AccountMeta::new(oracle.pubkey(), false),
            AccountMeta::new(payer.pubkey(), true),
            AccountMeta::new_readonly(CLOCK_SYSVAR, false),
        ],
        data,
    };

    let meta = send(&mut svm, ix, &payer).expect("oracle_update tx should succeed");
    println!("oracle_update CUs: {}", meta.compute_units_consumed);
}
