//! Shared helpers for v2 integration tests.

use {
    anchor_lang::solana_program::instruction::{AccountMeta, Instruction},
    litesvm::LiteSVM,
    solana_keypair::Keypair,
    solana_message::{Message, VersionedMessage},
    solana_signer::Signer,
    solana_transaction::versioned::VersionedTransaction,
    std::{
        collections::HashMap,
        sync::{Mutex, Once, OnceLock},
    },
};

/// Derives a deterministic keypair from a name string.
pub fn keypair_for(name: &str) -> Keypair {
    let mut seed = [0u8; 32];
    for (index, byte) in name.bytes().enumerate() {
        let position = index % seed.len();
        seed[position] = seed[position]
            .wrapping_mul(31)
            .wrapping_add(byte)
            .wrapping_add(index as u8);
    }
    Keypair::new_from_array(seed)
}

/// Build, sign, and send a transaction to the SVM.
pub fn send_instruction(
    svm: &mut LiteSVM,
    program_id: solana_pubkey::Pubkey,
    instruction_data: Vec<u8>,
    account_metas: Vec<AccountMeta>,
    payer: &Keypair,
    extra_signers: &[&Keypair],
) -> anyhow::Result<litesvm::types::TransactionMetadata> {
    let instruction = Instruction::new_with_bytes(program_id, &instruction_data, account_metas);
    let blockhash = svm.latest_blockhash();
    let message = Message::new_with_blockhash(&[instruction], Some(&payer.pubkey()), &blockhash);

    let mut all_signers: Vec<&dyn solana_signer::Signer> = vec![payer];
    for s in extra_signers {
        all_signers.push(*s);
    }
    let transaction =
        VersionedTransaction::try_new(VersionedMessage::Legacy(message), &all_signers)?;

    svm.send_transaction(transaction).map_err(|failure| {
        anyhow::anyhow!(
            "transaction failed: {:?}\n{}",
            failure.err,
            failure.meta.pretty_logs()
        )
    })
}

/// Build the .so for a program by running cargo build-sbf.
///
/// Memoized by `manifest_dir`: within a single test-binary process each
/// program is built at most once, even when multiple `#[test]`s invoke
/// this concurrently from their `setup()`. Parallel callers for the same
/// path block on a per-path `Once` until the first build finishes; the
/// table of `Once`s itself lives behind a short `Mutex` used only for
/// lookup/insert.
pub fn build_program(manifest_dir: &str, sbf_out_dir: &str) {
    static ONCES: OnceLock<Mutex<HashMap<String, &'static Once>>> = OnceLock::new();
    let once: &'static Once = {
        let mut table = ONCES
            .get_or_init(|| Mutex::new(HashMap::new()))
            .lock()
            .unwrap();
        *table
            .entry(manifest_dir.to_string())
            .or_insert_with(|| Box::leak(Box::new(Once::new())))
    };

    once.call_once(|| {
        let mut cmd = std::process::Command::new("cargo");
        if std::env::var_os("SBF_TRACE_DIR").is_some() {
            cmd.env("CARGO_PROFILE_RELEASE_OPT_LEVEL", "1");
        }
        cmd.args([
            "build-sbf",
            "--tools-version",
            "v1.52",
            "--manifest-path",
            &format!("{}/Cargo.toml", manifest_dir),
            "--sbf-out-dir",
            sbf_out_dir,
        ]);
        // Strip coverage instrumentation env vars so they don't leak into the
        // SBF cross-compilation (the Solana toolchain lacks profiler_builtins).
        // Only clear RUSTC_WRAPPER when it's cargo-llvm-cov's wrapper — the
        // anchor RUSTC_WRAPPER (used for DWARF path preservation in coverage
        // and debugger flows, detected via __ANCHOR_RUSTC_WRAPPER) must pass
        // through so it gets to rewrite `-Zremap-cwd-prefix`.
        let llvm_cov_active = std::env::var("CARGO_LLVM_COV").is_ok()
            || std::env::var("__CARGO_LLVM_COV_RUSTC_WRAPPER").is_ok();
        if llvm_cov_active {
            for var in [
                "CARGO_LLVM_COV",
                "CARGO_LLVM_COV_SHOW_ENV",
                "CARGO_LLVM_COV_TARGET_DIR",
                "CARGO_LLVM_COV_BUILD_DIR",
                "LLVM_PROFILE_FILE",
                "RUSTC_WRAPPER",
                "__CARGO_LLVM_COV_RUSTC_WRAPPER",
                "__CARGO_LLVM_COV_RUSTC_WRAPPER_RUSTFLAGS",
                "__CARGO_LLVM_COV_RUSTC_WRAPPER_CRATE_NAMES",
            ] {
                cmd.env_remove(var);
            }
            for (key, _) in std::env::vars() {
                if key.starts_with("CARGO_LLVM_COV") || key.starts_with("__CARGO_LLVM_COV") {
                    cmd.env_remove(&key);
                }
            }
        }
        if let Ok(flags) = std::env::var("RUSTFLAGS") {
            let cleaned: String = flags
                .split(' ')
                .filter(|f| !f.contains("instrument-coverage") && !f.contains("profraw"))
                .collect::<Vec<_>>()
                .join(" ");
            cmd.env("RUSTFLAGS", cleaned);
        }
        if let Ok(flags) = std::env::var("CARGO_ENCODED_RUSTFLAGS") {
            let cleaned: String = flags
                .split('\x1f')
                .filter(|f| !f.contains("instrument-coverage") && !f.contains("profraw"))
                .collect::<Vec<_>>()
                .join("\x1f");
            cmd.env("CARGO_ENCODED_RUSTFLAGS", cleaned);
        }
        let status = cmd
            .status()
            .unwrap_or_else(|e| panic!("failed to run cargo build-sbf for {manifest_dir}: {e}"));

        assert!(
            status.success(),
            "cargo build-sbf failed for {manifest_dir}"
        );
    });
}
