#![cfg(feature = "test-sbf")]

use {
    anchor_client::{anchor_lang::Discriminator, Client, Cluster},
    solana_program_test::{tokio, ProgramTest},
    solana_sdk::{
        account::Account, pubkey::Pubkey, signature::Keypair, signer::Signer,
        transaction::Transaction,
    },
    std::rc::Rc,
};

#[tokio::test]
async fn update_foo() {
    let authority = Keypair::new();
    let foo_pubkey = Pubkey::new_unique();
    let foo_account = {
        let mut foo_data = Vec::new();
        foo_data.extend_from_slice(zero_copy::Foo::DISCRIMINATOR);
        foo_data.extend_from_slice(bytemuck::bytes_of(&zero_copy::Foo {
            authority: authority.pubkey(),
            ..zero_copy::Foo::default()
        }));

        Account {
            lamports: 1,
            data: foo_data,
            owner: zero_copy::id(),
            ..Account::default()
        }
    };

    let mut pt = ProgramTest::new("zero_copy", zero_copy::id(), None);
    pt.add_account(foo_pubkey, foo_account);
    pt.set_compute_max_units(4157);
    let (banks_client, payer, recent_blockhash) = pt.start().await;

    let client = Client::new(Cluster::Debug, Rc::new(Keypair::new()));
    let program = client.program(zero_copy::id()).unwrap();
    let update_ix = program
        .request()
        .accounts(zero_copy::accounts::UpdateFoo {
            foo: foo_pubkey,
            authority: authority.pubkey(),
        })
        .args(zero_copy::instruction::UpdateFoo { data: 1u64 })
        .instructions()
        .pop()
        .unwrap();

    let transaction = Transaction::new_signed_with_payer(
        &[update_ix],
        Some(&payer.pubkey()),
        &[&payer, &authority],
        recent_blockhash,
    );

    banks_client.process_transaction(transaction).await.unwrap();
}
