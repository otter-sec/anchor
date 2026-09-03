use {
    anchor_lang_v1::{
        prelude::Pubkey,
        solana_program::instruction::{AccountMeta, Instruction},
    },
    anyhow::{anyhow, bail, Context, Result},
    litesvm::{types::TransactionMetadata, LiteSVM},
    solana_keypair::Keypair,
    solana_message::{Message, VersionedMessage},
    solana_signer::Signer,
    solana_transaction::versioned::VersionedTransaction,
    std::{
        collections::BTreeMap,
        fs,
        path::{Path, PathBuf},
        process::Command,
    },
};

// Import history types only when building the binary (main.rs uses these functions).
#[cfg(feature = "bin")]
use crate::history::{BenchmarkResult, ProgramBenchmark, CURRENT_COMMIT};

/// Describes a benchmarked program and the instruction cases it exposes.
#[derive(Clone, Copy)]
pub struct ProgramSuite {
    /// Used both as the results key and as the compiled `.so` basename
    /// (i.e. `target/deploy/{name}.so`).
    pub name: &'static str,
    /// Family name for grouping in comparison tables (e.g. `"hello_world"`
    /// or `"multisig"`). All variants of one workload share a family.
    pub family: &'static str,
    /// Human-readable variant label for comparison output,
    /// e.g. `"anchor v1"`, `"anchor v2"`, `"quasar"`, `"pinocchio"`, `"steel"`.
    pub variant: &'static str,
    /// Path (relative to `bench/`) of the program's Cargo manifest directory,
    /// e.g. `"programs/helloworld"` or `"programs/multisig/anchor-v1"`.
    pub manifest_dir: &'static str,
    pub instructions: &'static [InstructionSuite],
}

/// Describes a single instruction benchmark within a program suite.
///
/// Carries `program_id` + `build` directly so the main driver can call
/// `execute_benchmark` inline without any per-instruction runner shim.
#[derive(Clone, Copy)]
pub struct InstructionSuite {
    pub name: &'static str,
    pub program_id: fn() -> Pubkey,
    pub build: CaseBuilder,
}

/// Builds a ready-to-run benchmark transaction for a program instruction.
pub type CaseBuilder = fn(&mut BenchContext) -> Result<BenchInstruction>;

/// Holds the LiteSVM instance and shared payer used for a single benchmark run.
pub struct BenchContext {
    payer: Keypair,
    program_id: Pubkey,
    svm: LiteSVM,
}

impl BenchContext {
    /// Returns the transaction fee payer keypair's public key.
    /// The payer is deterministically derived from `"bench-payer"` and
    /// pre-funded with 1 SOL.
    pub fn payer_pubkey(&self) -> Pubkey {
        self.payer.pubkey()
    }

    /// Returns the program id this context was built for.
    pub fn program_id(&self) -> Pubkey {
        self.program_id
    }

    /// Returns a reference to the internal LiteSVM instance for advanced use.
    pub fn svm_mut(&mut self) -> &mut LiteSVM {
        &mut self.svm
    }

    /// Runs a benchmark case and returns the CU consumed.
    ///
    /// Higher-level wrapper that takes a closure (not just a fn pointer),
    /// making it easy to build cases that capture configuration state.
    pub fn bench<F>(program_path: &Path, program_id: Pubkey, case: F) -> Result<TransactionMetadata>
    where
        F: FnOnce(&mut BenchContext) -> Result<BenchInstruction>,
    {
        let mut ctx = Self::new(program_path, program_id)?;
        let instruction = case(&mut ctx)?;
        ctx.execute(instruction)
    }
}

/// A labeled benchmark case with a closure-based builder.
///
/// Use `BenchSuite` to build up a set of cases and run them all with
/// consolidated output + comparison tables.
pub struct BenchCase<'a> {
    pub label: &'a str,
    pub builder: Box<dyn Fn(&mut BenchContext) -> Result<BenchInstruction> + 'a>,
}

impl<'a> BenchCase<'a> {
    pub fn new<F>(label: &'a str, builder: F) -> Self
    where
        F: Fn(&mut BenchContext) -> Result<BenchInstruction> + 'a,
    {
        Self { label, builder: Box::new(builder) }
    }
}

/// A suite of benchmark cases runnable against one or more programs.
///
/// Each case produces a single transaction whose CU is measured. Use
/// `run_against` to execute all cases against a program binary and
/// collect the results for printing or comparison.
pub struct BenchSuite<'a> {
    pub name: &'a str,
    pub cases: Vec<BenchCase<'a>>,
}

impl<'a> BenchSuite<'a> {
    pub fn new(name: &'a str) -> Self {
        Self { name, cases: Vec::new() }
    }

    pub fn add<F>(mut self, label: &'a str, builder: F) -> Self
    where
        F: Fn(&mut BenchContext) -> Result<BenchInstruction> + 'a,
    {
        self.cases.push(BenchCase::new(label, builder));
        self
    }

    /// Runs every case in this suite against the given program binary,
    /// returning a vector of (label, Result<CU>) pairs.
    pub fn run_against(
        &self,
        program_path: &Path,
        program_id: Pubkey,
    ) -> Vec<(&str, Result<u64>)> {
        self.cases.iter().map(|case| {
            let result = (|| -> Result<u64> {
                let mut ctx = BenchContext::new(program_path, program_id)?;
                let instruction = (case.builder)(&mut ctx)?;
                let meta = ctx.execute(instruction)?;
                Ok(meta.compute_units_consumed)
            })();
            (case.label, result)
        }).collect()
    }
}

/// Print a side-by-side comparison table of two result sets.
///
/// Rows are keyed by case label; FAILED rows show the error message.
pub fn print_comparison(
    left_name: &str,
    left_results: &[(&str, Result<u64>)],
    right_name: &str,
    right_results: &[(&str, Result<u64>)],
) {
    println!("\n{:<20}  {:>12}  {:>12}  {:>10}  {:>8}", "Instruction", left_name, right_name, "Diff", "Pct");
    println!("{}", "-".repeat(70));

    for (label, left) in left_results {
        let right = right_results
            .iter()
            .find(|(l, _)| l == label)
            .and_then(|(_, r)| r.as_ref().ok().copied());

        match (left.as_ref().ok().copied(), right) {
            (Some(l), Some(r)) => {
                let diff = l as i64 - r as i64;
                let pct = (diff as f64 / r as f64) * 100.0;
                let sign = if diff >= 0 { "+" } else { "" };
                println!("{label:<20}  {l:>12}  {r:>12}  {sign}{diff:>9}  {sign}{pct:>7.1}%");
            }
            (Some(l), None) => {
                println!("{label:<20}  {l:>12}  {:>12}  {:>10}  {:>8}", "FAILED", "-", "-");
            }
            (None, Some(r)) => {
                println!("{label:<20}  {:>12}  {r:>12}  {:>10}  {:>8}", "FAILED", "-", "-");
            }
            (None, None) => {
                println!("{label:<20}  {:>12}  {:>12}  {:>10}  {:>8}", "FAILED", "FAILED", "-", "-");
            }
        }
    }
}

/// Represents one benchmark transaction plus any additional required signers.
pub struct BenchInstruction {
    instruction_data: Vec<u8>,
    account_metas: Vec<AccountMeta>,
    signers: Vec<Keypair>,
}

impl BenchInstruction {
    /// Creates a benchmark instruction from serialized data and account metas.
    pub fn new(instruction_data: Vec<u8>, account_metas: Vec<AccountMeta>) -> Self {
        Self {
            instruction_data,
            account_metas,
            signers: Vec::new(),
        }
    }

    /// Creates a benchmark instruction from a `solana_instruction::Instruction`.
    pub fn from_instruction(ix: Instruction) -> Self {
        Self {
            instruction_data: ix.data,
            account_metas: ix.accounts,
            signers: Vec::new(),
        }
    }

    /// Adds a single extra signer to the benchmark transaction.
    pub fn with_signer(mut self, signer: Keypair) -> Self {
        self.signers.push(signer);
        self
    }

    /// Adds multiple extra signers to the benchmark transaction.
    pub fn with_signers(mut self, signers: Vec<Keypair>) -> Self {
        self.signers.extend(signers);
        self
    }
}

impl BenchContext {
    /// Creates a fresh LiteSVM instance with the target program loaded and a funded payer.
    pub fn new(program_path: &Path, program_id: Pubkey) -> Result<Self> {
        let payer = keypair_for_account("bench-payer");
        let mut svm = LiteSVM::new();

        svm.add_program_from_file(program_id, program_path)
            .with_context(|| format!("failed to load {}", program_path.display()))?;
        svm.airdrop(&payer.pubkey(), 1_000_000_000)
            .map_err(|failure| {
                anyhow!(
                    "failed to fund benchmark payer: {:?}\n{}",
                    failure.err,
                    failure.meta.pretty_logs()
                )
            })?;

        Ok(Self {
            payer,
            program_id,
            svm,
        })
    }

    /// Funds an account inside the benchmark VM before running an instruction.
    pub fn airdrop(&mut self, pubkey: &Pubkey, lamports: u64) -> Result<()> {
        self.svm.airdrop(pubkey, lamports).map_err(|failure| {
            anyhow!(
                "failed to fund benchmark account {}: {:?}\n{}",
                pubkey,
                failure.err,
                failure.meta.pretty_logs()
            )
        })?;
        Ok(())
    }

    /// Executes a benchmark instruction with any signers attached to it.
    pub fn execute(&mut self, instruction: BenchInstruction) -> Result<TransactionMetadata> {
        let signer_refs = instruction
            .signers
            .iter()
            .map(|signer| signer as &dyn solana_signer::Signer)
            .collect::<Vec<_>>();

        self.execute_raw(
            instruction.instruction_data,
            instruction.account_metas,
            &signer_refs,
        )
    }

    /// Executes a benchmark instruction using an explicit signer list.
    pub fn execute_with_signers(
        &mut self,
        instruction_data: Vec<u8>,
        account_metas: Vec<AccountMeta>,
        signers: &[&dyn solana_signer::Signer],
    ) -> Result<TransactionMetadata> {
        self.execute_raw(instruction_data, account_metas, signers)
    }

    /// Constructs and submits the underlying transaction to LiteSVM.
    fn execute_raw(
        &mut self,
        instruction_data: Vec<u8>,
        account_metas: Vec<AccountMeta>,
        signers: &[&dyn solana_signer::Signer],
    ) -> Result<TransactionMetadata> {
        let instruction =
            Instruction::new_with_bytes(self.program_id, &instruction_data, account_metas);

        let blockhash = self.svm.latest_blockhash();
        let message =
            Message::new_with_blockhash(&[instruction], Some(&self.payer.pubkey()), &blockhash);
        let mut all_signers: Vec<&dyn solana_signer::Signer> = vec![&self.payer];
        all_signers.extend_from_slice(signers);
        let transaction =
            VersionedTransaction::try_new(VersionedMessage::Legacy(message), &all_signers)
                .context("failed to create benchmark transaction")?;

        self.svm.send_transaction(transaction).map_err(|failure| {
            anyhow!(
                "benchmark transaction failed: {:?}\n{}",
                failure.err,
                failure.meta.pretty_logs()
            )
        })
    }
}

#[cfg(feature = "bin")]
/// Builds every benchmarked program into `target/deploy` before measurement.
pub fn build_programs(bench_dir: &Path, suites: &[ProgramSuite]) -> Result<()> {
    for suite in suites {
        let manifest_path = format!("{}/Cargo.toml", suite.manifest_dir);
        let deploy_dir = bench_dir.join("../target/deploy");
        // `--tools-version v1.52` is required: older platform-tools ship
        // with Cargo 1.84 which can't parse edition2024 manifests that
        // some transitive crates (e.g. indexmap 2.14+) now use.
        let status = Command::new("cargo")
            .args([
                "build-sbf",
                "--tools-version",
                "v1.52",
                "--manifest-path",
                &manifest_path,
                "--sbf-out-dir",
                &deploy_dir.to_string_lossy(),
            ])
            .current_dir(bench_dir)
            .status()
            .with_context(|| format!("failed to launch cargo build-sbf for {}", suite.name))?;

        if !status.success() {
            bail!(
                "cargo build-sbf failed for {} with status {status}",
                suite.name
            );
        }

    }

    Ok(())
}

/// Loads a program into LiteSVM, prepares a case, and executes the benchmark transaction.
pub fn execute_benchmark(
    program_path: &Path,
    program_id: Pubkey,
    case_builder: CaseBuilder,
) -> Result<TransactionMetadata> {
    let mut ctx = BenchContext::new(program_path, program_id)?;
    let instruction = case_builder(&mut ctx)?;
    ctx.execute(instruction)
}

#[cfg(feature = "bin")]
/// Collects binary size and compute-unit measurements for all configured suites.
pub fn build_results(bench_dir: &Path, suites: &[ProgramSuite]) -> Result<BenchmarkResult> {
    let mut programs = BTreeMap::new();

    for suite in suites {
        let program_path = program_binary_path(bench_dir, suite.name);
        let binary_size_bytes = fs::metadata(&program_path)
            .with_context(|| format!("failed to read metadata for {}", program_path.display()))?
            .len();
        let mut compute_units = BTreeMap::new();

        for instruction in suite.instructions {
            let metadata =
                execute_benchmark(&program_path, (instruction.program_id)(), instruction.build)?;
            compute_units.insert(instruction.name.to_owned(), metadata.compute_units_consumed);
        }

        programs.insert(
            suite.name.to_owned(),
            ProgramBenchmark {
                binary_size_bytes,
                compute_units,
            },
        );
    }

    Ok(BenchmarkResult {
        commit: CURRENT_COMMIT.to_owned(),
        programs,
    })
}

/// Derives a stable keypair from an account label so benchmark inputs are repeatable.
pub fn keypair_for_account(name: &str) -> Keypair {
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

#[cfg(feature = "bin")]
/// Returns the expected on-disk path for a compiled program binary.
/// Programs are built into the workspace root's `target/deploy/`, one level above bench/.
fn program_binary_path(bench_dir: &Path, program_name: &str) -> PathBuf {
    bench_dir
        .join("../target/deploy")
        .join(format!("{program_name}.so"))
}
