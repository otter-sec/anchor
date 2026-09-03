use {
    anchor_lang::{
        accounts::Account, bytemuck, programs::System,
        solana_program::instruction::Instruction, Id, InstructionData, Space, ToAccountMetas,
    },
    anchor_v2_testing::{Keypair, LiteSVM, Message, Signer, VersionedMessage, VersionedTransaction},
    litesvm::types::{FailedTransactionMetadata, TransactionMetadata},
    prop_amm_v2::{instruction, state::Oracle, UPDATE_AUTHORITY},
};

type TxResult = Result<TransactionMetadata, FailedTransactionMetadata>;

/// Matches the on-chain `UPDATE_AUTHORITY` constant.
fn update_authority() -> Keypair {
    Keypair::new_from_array([7u8; 32])
}

fn setup() -> (LiteSVM, Keypair, Keypair) {
    let program_id = prop_amm_v2::id();
    let mut svm = anchor_v2_testing::svm();
    let bytes = include_bytes!("../../../../target/deploy/prop_amm_v2.so");
    svm.add_program(program_id, bytes).unwrap();

    let payer = Keypair::new();
    let oracle = Keypair::new();
    svm.airdrop(&payer.pubkey(), 1_000_000_000).unwrap();
    svm.airdrop(&update_authority().pubkey(), 1_000_000_000)
        .unwrap();

    (svm, payer, oracle)
}

fn send(svm: &mut LiteSVM, ix: Instruction, signers: &[&Keypair]) -> TxResult {
    let blockhash = svm.latest_blockhash();
    let msg = Message::new_with_blockhash(&[ix], Some(&signers[0].pubkey()), &blockhash);
    let tx = VersionedTransaction::try_new(VersionedMessage::Legacy(msg), signers).unwrap();
    svm.send_transaction(tx)
}

fn read_oracle(svm: &LiteSVM, oracle_pubkey: &Keypair) -> Oracle {
    let account = svm
        .get_account(&oracle_pubkey.pubkey())
        .expect("oracle account");
    assert_eq!(
        account.data.len(),
        <Account<Oracle> as Space>::INIT_SPACE,
    );
    *bytemuck::from_bytes::<Oracle>(&account.data[8..])
}

fn init_oracle(svm: &mut LiteSVM, payer: &Keypair, oracle: &Keypair) {
    let ix = Instruction::new_with_bytes(
        prop_amm_v2::id(),
        &instruction::Initialize {}.data(),
        prop_amm_v2::accounts::Initialize {
            payer: payer.pubkey(),
            oracle: oracle.pubkey(),
            system_program: System::id(),
        }
        .to_account_metas(None),
    );
    send(svm, ix, &[payer, oracle]).expect("initialize");
}

#[test]
fn test_initialize() {
    let (mut svm, payer, oracle) = setup();
    init_oracle(&mut svm, &payer, &oracle);

    let state = read_oracle(&svm, &oracle);
    assert_eq!(state.authority.to_bytes(), payer.pubkey().to_bytes());
    assert_eq!(state.price, 0);
}

#[test]
fn test_rotate_authority() {
    let (mut svm, payer, oracle) = setup();
    init_oracle(&mut svm, &payer, &oracle);

    let new_auth = Keypair::new();
    let ix = Instruction::new_with_bytes(
        prop_amm_v2::id(),
        &instruction::RotateAuthority {
            new_authority: new_auth.pubkey().to_bytes(),
        }
        .data(),
        prop_amm_v2::accounts::RotateAuthority {
            oracle: oracle.pubkey(),
            authority: payer.pubkey(),
        }
        .to_account_metas(None),
    );
    send(&mut svm, ix, &[&payer]).expect("rotate");

    let state = read_oracle(&svm, &oracle);
    assert_eq!(state.authority.to_bytes(), new_auth.pubkey().to_bytes());
}

#[test]
fn test_update_authorized() {
    let (mut svm, payer, oracle) = setup();
    init_oracle(&mut svm, &payer, &oracle);

    let auth = update_authority();
    assert_eq!(auth.pubkey().to_bytes(), UPDATE_AUTHORITY.to_bytes());

    let ix = Instruction::new_with_bytes(
        prop_amm_v2::id(),
        &instruction::Update { new_price: 1234 }.data(),
        prop_amm_v2::accounts::Update {
            oracle: oracle.pubkey(),
            authority: auth.pubkey(),
        }
        .to_account_metas(None),
    );
    let res = send(&mut svm, ix, &[&auth]).expect("update");
    println!("update CUs: {}", res.compute_units_consumed);

    let state = read_oracle(&svm, &oracle);
    assert_eq!(state.price, 1234);
    assert_eq!(state.authority.to_bytes(), payer.pubkey().to_bytes());
}

#[test]
fn test_update_rejects_wrong_signer() {
    let (mut svm, payer, oracle) = setup();
    init_oracle(&mut svm, &payer, &oracle);

    let ix = Instruction::new_with_bytes(
        prop_amm_v2::id(),
        &instruction::Update { new_price: 9999 }.data(),
        prop_amm_v2::accounts::Update {
            oracle: oracle.pubkey(),
            authority: payer.pubkey(),
        }
        .to_account_metas(None),
    );
    let res = send(&mut svm, ix, &[&payer]);
    assert!(res.is_err(), "wrong signer must be rejected");
    assert_eq!(read_oracle(&svm, &oracle).price, 0);
}

#[test]
fn test_update_rejects_when_not_signer() {
    let (mut svm, _payer, oracle) = setup();
    let payer = update_authority();
    init_oracle(&mut svm, &payer, &oracle);

    let another_payer = Keypair::new();
    svm.airdrop(&another_payer.pubkey(), 1_000_000_000).unwrap();

    let mut metas = prop_amm_v2::accounts::Update {
        oracle: oracle.pubkey(),
        authority: payer.pubkey(),
    }
    .to_account_metas(None);
    // Drop the signer flag so the runtime serializes is_signer = 0 even
    // though the tx is signed by a different keypair — this is what the
    // asm path's is_signer check catches.
    metas[1].is_signer = false;

    let ix = Instruction::new_with_bytes(
        prop_amm_v2::id(),
        &instruction::Update { new_price: 5555 }.data(),
        metas,
    );
    let res = send(&mut svm, ix, &[&another_payer]);
    assert!(res.is_err(), "non-signer authority must be rejected");
    assert_eq!(read_oracle(&svm, &oracle).price, 0);
}
