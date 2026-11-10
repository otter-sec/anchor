//! A program that depends on a `cpi` crate which itself depends on a `cpi`
//! crate links two `cpi` dependencies into one SBF binary. Both used to
//! export `__anchor_dispatch`, so `cargo build-sbf` failed on
//! `nested-caller` with a duplicate symbol.

use {
    anchor_lang::{solana_program::instruction::AccountMeta, InstructionData},
    anchor_v2_testing::litesvm::LiteSVM,
    solana_pubkey::Pubkey,
    solana_signer::Signer,
    tests_v2::{build_program, keypair_for, send_instruction},
};

#[test]
fn a_nested_cpi_chain_builds_links_and_runs() {
    let test_dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let deploy_dir = test_dir.join("target/deploy");
    let deploy_str = deploy_dir.to_str().unwrap();

    for program in ["callee", "caller", "nested-caller"] {
        build_program(
            test_dir.join("programs").join(program).to_str().unwrap(),
            deploy_str,
        );
    }

    let callee_id = Pubkey::from(callee::ID.to_bytes());
    let caller_id = Pubkey::from(caller::ID.to_bytes());
    let nested_id = Pubkey::from(nested_caller::ID.to_bytes());

    let mut svm = LiteSVM::new();
    for (id, so) in [
        (callee_id, "callee.so"),
        (caller_id, "caller.so"),
        (nested_id, "nested_caller.so"),
    ] {
        svm.add_program_from_file(id, deploy_dir.join(so))
            .unwrap_or_else(|e| panic!("failed to load {so}: {e:?}"));
    }

    let payer = keypair_for("nested-cpi-payer");
    svm.airdrop(&payer.pubkey(), 1_000_000_000).unwrap();

    let data = nested_caller::instruction::ProxyProxyEmpty {}.data();
    let metas = vec![
        AccountMeta::new_readonly(caller_id, false),
        AccountMeta::new_readonly(callee_id, false),
    ];
    send_instruction(&mut svm, nested_id, data, metas, &payer, &[])
        .expect("nested_caller -> caller -> callee should succeed");
}
