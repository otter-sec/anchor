#![cfg(feature = "test-sbf")]

use {
    anchor_lang::{
        prelude::Pubkey,
        solana_program::{instruction::Instruction, system_program},
        InstructionData, ToAccountMetas,
    },
    declare_program::external,
    solana_program_test::{tokio, ProgramTest},
    solana_sdk::{
        native_token::LAMPORTS_PER_SOL, signature::Signer, system_instruction,
        transaction::Transaction,
    },
};

#[tokio::test]
async fn proxy() {
    let mut pt = ProgramTest::new("declare_program", declare_program::id(), None);
    pt.add_program("external", external::ID, None);

    let (banks_client, payer, recent_blockhash) = pt.start().await;

    let authority =
        Pubkey::find_program_address(&[declare_program::GLOBAL], &declare_program::ID).0;
    let mut accounts = declare_program::accounts::Proxy {
        program: external::ID,
    }
    .to_account_metas(None);
    accounts.extend(
        external::client::accounts::Init {
            authority,
            my_account: Pubkey::find_program_address(&[authority.as_ref()], &external::ID).0,
            system_program: system_program::ID,
        }
        .to_account_metas(Some(false)), // Forward as remaining accounts but toggle authority not to be a signer
    );

    let data = declare_program::instruction::Proxy {
        data: external::client::args::Init {}.data(),
    }
    .data();
    let transaction = Transaction::new_signed_with_payer(
        &[
            system_instruction::transfer(&payer.pubkey(), &authority, LAMPORTS_PER_SOL),
            Instruction {
                program_id: declare_program::ID,
                accounts,
                data,
            },
        ],
        Some(&payer.pubkey()),
        &[&payer],
        recent_blockhash,
    );

    banks_client.process_transaction(transaction).await.unwrap();
}
