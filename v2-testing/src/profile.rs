//! Per-test register-trace capture.
//!
//! Behind the `profile` feature, `svm()` returns a `LiteSVM` with
//! register tracing turned on and a custom [`InvocationInspectCallback`]
//! that routes trace files into a per-test directory keyed on
//! `std::thread::current().name()`. Multi-tx tests (and their CPIs) all
//! land under the same test directory so downstream tooling produces a
//! single flamegraph per test.
//!
//! ## File layout
//!
//! ```text
//! target/anchor-v2-profile/
//! └── <sanitized_test_name>/
//!     ├── 0001__tx1.regs          ← tx 1, top-level invocation
//!     ├── 0001__tx1.insns
//!     ├── 0001__tx1.program_id
//!     ├── 0002__tx1.regs          ← tx 1, CPI into another program
//!     ├── 0002__tx1.insns
//!     ├── 0002__tx1.program_id
//!     ├── 0003__tx2.regs          ← tx 2
//!     └── ...
//! ```
//!
//! - `NNNN` is a monotonic invocation counter (CPIs get their own index).
//! - `txN` identifies the outer transaction — bumped once per
//!   `send_transaction`.
//! - Both counters are per-test, reset across processes.
//!
//! ## Re-run semantics
//!
//! The first invocation that fires for a given `(process, test)` pair
//! wipes the test's directory before writing. This guarantees a test
//! that previously produced more invocations than its current version
//! doesn't leak stale traces into a subsequent run.
//!
//! Override the root directory with the `ANCHOR_PROFILE_DIR` env var.

use {
    litesvm::{InvocationInspectCallback, LiteSVM},
    solana_program_runtime::{
        invoke_context::{Executable, InvokeContext, RegisterTrace},
        solana_sbpf::ebpf,
    },
    solana_transaction::sanitized::SanitizedTransaction,
    solana_transaction_context::{IndexOfAccount, InstructionContext},
    std::{
        collections::HashMap,
        fs::{self, File},
        io::Write,
        path::PathBuf,
        sync::Mutex,
    },
};

/// Reinterpret a slice of POD values as raw bytes. litesvm's own
/// `register_tracing::as_bytes` is `pub(crate)` so we replicate it here.
fn as_bytes<T>(slice: &[T]) -> &[u8] {
    // Safety: T is a fixed-size POD (in practice `[u64; 12]`) — reinterpreting
    // its byte image is a well-defined operation on stable Rust so long as
    // we don't write through it.
    unsafe {
        std::slice::from_raw_parts(slice.as_ptr() as *const u8, std::mem::size_of_val(slice))
    }
}

const DEFAULT_DIR: &str = "target/anchor-v2-profile";

/// Construct a `LiteSVM` wired up for Anchor v2 testing.
///
/// With the `profile` feature, enables register tracing at the VM level
/// and installs [`TestNameCallback`] so each transaction writes an SBF
/// register trace under `target/anchor-v2-profile/<test_name>/`.
/// Without the feature, identical to [`LiteSVM::new()`].
///
/// When `ANCHOR_GDB_SOCKET` is set (by `anchor debugger --gdb`), additionally
/// allocates a free TCP port, writes it into `VM_DEBUG_PORT` so sbpf's
/// gdb-stub picks it up, and announces the port over the Unix socket at
/// `ANCHOR_GDB_SOCKET`. The mutex only covers the tiny window where env is
/// set + LiteSVM constructed (sbpf reads `VM_DEBUG_PORT` in
/// `EbpfVm::new`, which is called inside `LiteSVM::new_debuggable`); once
/// the port is baked into the VM config the guard releases and other
/// threads can build their own VMs in parallel.
pub fn svm() -> LiteSVM {
    #[cfg(feature = "profile")]
    let _gdb_guard = setup_gdb_port();

    // `new_debuggable(true)` is what actually turns on rbpf's
    // instruction tracing — without it, `set_invocation_inspect_callback`
    // fires but `iterate_vm_traces` is empty. litesvm alternatively
    // reads `SBF_TRACE_DIR` at `new()` time, but hard-coding the flag
    // is more robust than depending on ambient env state.
    let mut svm = LiteSVM::new_debuggable(true);
    svm.set_invocation_inspect_callback(TestNameCallback::new());
    svm
}

/// Construct a debuggable `LiteSVM` that writes all captured traces directly
/// into `trace_dir`.
///
/// This is useful for one-off transaction replay tools where there is no Rust
/// test name to key the trace directory.
pub fn svm_with_trace_dir(trace_dir: impl Into<PathBuf>) -> LiteSVM {
    #[cfg(feature = "profile")]
    let _gdb_guard = setup_gdb_port();

    let mut svm = LiteSVM::new_debuggable(true);
    svm.set_invocation_inspect_callback(TestNameCallback::new_with_fixed_dir(trace_dir.into()));
    svm
}

/// Allocates a free TCP port, sets `VM_DEBUG_PORT`, and announces the
/// port to `anchor debugger --gdb` over the Unix socket identified by
/// `ANCHOR_GDB_SOCKET`. Returns a `MutexGuard` that keeps the environment
/// writes from interleaving between threads; drop as soon as the caller
/// has finished constructing its LiteSVM.
///
/// No-op when `ANCHOR_GDB_SOCKET` is unset (the common case — normal
/// `cargo test --features profile` runs without gdb).
#[cfg(feature = "profile")]
fn setup_gdb_port() -> Option<std::sync::MutexGuard<'static, ()>> {
    use std::{
        io::Write,
        net::TcpListener,
        os::unix::net::UnixStream,
        sync::{Mutex, OnceLock},
    };

    let sock_path = std::env::var("ANCHOR_GDB_SOCKET").ok()?;

    // Global lock so two parallel test threads don't clobber each
    // other's `VM_DEBUG_PORT` during the VM-construction window.
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    let guard = LOCK.get_or_init(|| Mutex::new(())).lock().unwrap();

    // Ask the OS for a free port via bind-and-drop. The brief window
    // between drop and sbpf's bind is near-impossible to lose to
    // another process in a localhost-only test run.
    let port = TcpListener::bind("127.0.0.1:0")
        .ok()
        .and_then(|l| l.local_addr().ok())?
        .port();

    // Announce first, then set env — so anchor-debugger is already
    // polling the port by the time sbpf opens its listener.
    if let Ok(mut sock) = UnixStream::connect(&sock_path) {
        let thread_name = std::thread::current()
            .name()
            .unwrap_or("unknown")
            .to_owned();
        let _ = writeln!(sock, "{port}\t{thread_name}");
    }

    std::env::set_var("VM_DEBUG_PORT", port.to_string());

    Some(guard)
}

struct TestState {
    /// Monotonic counter across all invocations (including CPIs) in this test.
    inv_seq: u32,
    /// Monotonic counter across outer transactions in this test. Bumped in
    /// `before_invocation`, which only fires at tx top level.
    tx_seq: u32,
    /// True once this test's directory has been wiped and recreated.
    cleaned: bool,
}

/// Register-tracing callback that keys trace files by test name.
///
/// Implements [`InvocationInspectCallback`]. Use via
/// [`LiteSVM::set_invocation_inspect_callback`].
pub struct TestNameCallback {
    root: PathBuf,
    fixed_dir: Option<PathBuf>,
    state: Mutex<HashMap<String, TestState>>,
}

impl TestNameCallback {
    pub fn new() -> Self {
        let root = std::env::var("ANCHOR_PROFILE_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from(DEFAULT_DIR));
        Self {
            root,
            fixed_dir: None,
            state: Mutex::new(HashMap::new()),
        }
    }

    pub fn new_with_fixed_dir(dir: PathBuf) -> Self {
        Self {
            root: PathBuf::new(),
            fixed_dir: Some(dir),
            state: Mutex::new(HashMap::new()),
        }
    }

    fn test_name(&self) -> String {
        if self.fixed_dir.is_some() {
            "trace".to_owned()
        } else {
            std::thread::current()
                .name()
                .unwrap_or("unknown")
                .replace("::", "__")
        }
    }

    fn test_dir(&self, test: &str) -> PathBuf {
        self.fixed_dir
            .clone()
            .unwrap_or_else(|| self.root.join(test))
    }

    /// Bumps the tx counter (called from `before_invocation`, so only
    /// outer tx entry — CPIs don't go through this path) and returns
    /// the current tx number.
    fn bump_tx_seq(&self, test: &str) -> u32 {
        let mut state = self.state.lock().unwrap();
        let entry = state.entry(test.to_owned()).or_insert(TestState {
            inv_seq: 0,
            tx_seq: 0,
            cleaned: false,
        });
        entry.tx_seq += 1;
        entry.tx_seq
    }

    /// Bumps the invocation counter and returns `(inv_seq, tx_seq,
    /// should_clean)`. `should_clean` is true on the first call for a
    /// test in this process — caller is responsible for wiping the
    /// test's directory before writing.
    fn bump_inv_seq(&self, test: &str) -> (u32, u32, bool) {
        let mut state = self.state.lock().unwrap();
        let entry = state.entry(test.to_owned()).or_insert(TestState {
            inv_seq: 0,
            tx_seq: 0,
            cleaned: false,
        });
        entry.inv_seq += 1;
        let should_clean = !entry.cleaned;
        entry.cleaned = true;
        (entry.inv_seq, entry.tx_seq.max(1), should_clean)
    }
}

impl Default for TestNameCallback {
    fn default() -> Self {
        Self::new()
    }
}

impl InvocationInspectCallback for TestNameCallback {
    // `before_invocation` is a no-op. Bumping `tx_seq` here would
    // double-count transactions that run through the callback but
    // never hit rbpf — e.g. `LiteSVM::airdrop` internally fires a
    // system-program transfer that triggers before/after_invocation
    // but produces no VM trace because the system program is a native
    // builtin. We lazily bump `tx_seq` in `after_invocation` only
    // when `iterate_vm_traces` actually yields records, so `tx1` in
    // filenames == the user's first real tx.
    fn before_invocation(
        &self,
        _: &LiteSVM,
        _: &SanitizedTransaction,
        _: &[IndexOfAccount],
        _: &InvokeContext,
    ) {
    }

    fn after_invocation(
        &self,
        _svm: &LiteSVM,
        invoke_context: &InvokeContext,
        register_tracing_enabled: bool,
    ) {
        if !register_tracing_enabled {
            return;
        }

        let test = self.test_name();
        let dir = self.test_dir(&test);
        let bumped_tx = std::cell::Cell::new(false);

        invoke_context.iterate_vm_traces(
            &|ictx: InstructionContext, exec: &Executable, trace: RegisterTrace| {
                if trace.is_empty() {
                    return;
                }

                if !bumped_tx.get() {
                    bumped_tx.set(true);
                    self.bump_tx_seq(&test);
                }

                let (inv_seq, tx_seq, should_clean) = self.bump_inv_seq(&test);

                // Wipe on the first invocation we write for this test so
                // stale traces from a longer prior run can't leak through.
                // `bump_inv_seq` guarantees `should_clean == true` only once.
                if should_clean {
                    let _ = fs::remove_dir_all(&dir);
                }
                if fs::create_dir_all(&dir).is_err() {
                    return;
                }

                let stem = dir.join(format!("{inv_seq:04}__tx{tx_seq}"));
                let (_, text) = exec.get_text_bytes();

                // .insns — for each traced step, the raw 8-byte instruction
                // at r11 (PC). The renderer needs this to detect syscall
                // CALL_IMMs vs internal calls; without it, find_trace_files
                // silently skips the entire trace.
                if let Ok(mut f) = File::create(stem.with_extension("insns")) {
                    for regs in trace.iter() {
                        let pc = regs[11] as usize;
                        let insn = ebpf::get_insn_unchecked(text, pc).to_array();
                        let _ = f.write_all(&insn);
                    }
                }

                // .regs — raw [u64; 12] register states. PC is r11.
                if let Ok(mut f) = File::create(stem.with_extension("regs")) {
                    for regs in trace.iter() {
                        let _ = f.write_all(as_bytes(regs.as_slice()));
                    }
                }
                // .program_id — base58 program invoked for this trace.
                if let Ok(pid) = ictx.get_program_key() {
                    if let Ok(mut f) = File::create(stem.with_extension("program_id")) {
                        let _ = write!(f, "{}", pid);
                    }
                }
            },
        );
    }
}
