//! GDB-driven trace capture for `anchor debugger --gdb`.
//!
//! Alternative to the register-tracing-file path. Each test thread's VM
//! blocks on a per-thread TCP port (sbpf's built-in gdb stub, activated
//! via the `debugger` feature on `solana-sbpf`). We drive the stub over
//! the GDB Remote Serial Protocol, single-stepping each invocation while
//! reading the full register set (including the pseudo-register at index
//! 12 that the sbpf target exposes as `InstructionCountRemaining` —
//! compute-unit remaining at each step).
//!
//! Files are written into the same `<profile_dir>/<test>/NNNN__txK.{regs,
//! insns,program_id,cu}` layout `TestNameCallback` produces, so the
//! existing arena + TUI code consumes them unchanged. The only new
//! artifact is `.cu` — 8 bytes per step, the VM's `cu_remaining` value
//! read via the gdb stub's register 12.
//!
//! ## CPI handling
//!
//! Each CPI frame constructs its own `EbpfVm`, which reads `VM_DEBUG_PORT`
//! and binds another listener on the same port (sbpf drops its listener
//! after `accept()`, so re-entrance is fine). The client side has to
//! notice that a step command on the outer connection is taking longer
//! than expected, open a second TCP connection to the same port, and
//! drive the inner session to completion — then the outer step reply
//! arrives, stepping continues. This mirrors the actual sbpf/agave call
//! stack: outer step → CPI syscall → inner VM exec → return → outer
//! step reply.

use {
    anyhow::{anyhow, Context, Result},
    std::{
        io::{BufRead, BufReader, Read, Write},
        net::TcpStream,
        os::unix::net::{UnixListener, UnixStream},
        path::{Path, PathBuf},
        sync::{
            atomic::{AtomicBool, Ordering},
            Arc, Mutex,
        },
        thread,
        time::{Duration, Instant},
    },
};

// sbpf gdb target layout. `g` dumps 12 x u64 = 96 bytes = 192 hex chars:
//   0..9   — Gpr (r0..r9)
//   10     — Sp
//   11     — Pc
// `InstructionCountRemaining` is a PSEUDO register at index 12; gdbstub
// doesn't include pseudo regs in `g` by convention — it requires a
// separate `p 0c` read. We issue that per step.
const REG_COUNT: usize = 12;
const REG_BYTES: usize = REG_COUNT * 8;
const REG_HEX_LEN: usize = REG_BYTES * 2;

/// Env var the gdb driver and the test process use to rendezvous on
/// the announce socket. Single source of truth — both the driver
/// (`std::env::set_var`) and `anchor-v2-testing` (which reads it) must
/// use this name.
pub const SOCKET_ENV: &str = "ANCHOR_GDB_SOCKET";

/// Owns the UDS listener at `<profile_dir>/gdb.sock` and the accept
/// thread that spawns one driver thread per VM announcement. Drop
/// signals stop, joins the accept thread, and removes the socket.
///
/// Decoupled from cargo invocation so both loose mode and Anchor.toml
/// mode can stand up the same listener+driver and just differ in how
/// the test process gets launched.
pub struct GdbDriver {
    profile_dir: PathBuf,
    sock_path: PathBuf,
    stop: Arc<AtomicBool>,
    accept: Option<thread::JoinHandle<Result<()>>>,
    captures: Arc<Mutex<Vec<PendingCapture>>>,
}

impl GdbDriver {
    pub fn sock_path(&self) -> &Path {
        &self.sock_path
    }

    fn shutdown(&mut self) {
        self.stop.store(true, Ordering::Release);
        if let Some(h) = self.accept.take() {
            let _ = h.join();
        }
        if let Ok(captures) = self.captures.lock() {
            if let Err(e) = write_pending_captures(&self.profile_dir, &captures) {
                eprintln!("gdb sidecar write error: {e}");
            }
        }
        let _ = std::fs::remove_file(&self.sock_path);
    }
}

impl Drop for GdbDriver {
    fn drop(&mut self) {
        self.shutdown();
    }
}

/// Bind the UDS listener and spawn the accept thread. Caller is
/// responsible for setting `ANCHOR_GDB_SOCKET` in the env of whatever
/// test process consumes the socket.
pub fn start_gdb_driver(profile_dir: &Path) -> Result<GdbDriver> {
    // Socket lives under the profile dir — same retention semantics as
    // the trace files themselves, cleaned on next debugger run.
    std::fs::create_dir_all(profile_dir).ok();
    let sock_path = profile_dir.join("gdb.sock");
    let _ = std::fs::remove_file(&sock_path); // clear stale
    let listener =
        UnixListener::bind(&sock_path).with_context(|| format!("bind {}", sock_path.display()))?;

    let stop = Arc::new(AtomicBool::new(false));
    let captures = Arc::new(Mutex::new(Vec::new()));

    // Accept thread: pull port announcements off the socket, spawn a
    // driver per announcement.
    let accept_stop = Arc::clone(&stop);
    let profile_dir_for_accept = profile_dir.to_path_buf();
    let captures_for_accept = Arc::clone(&captures);
    let accept_handle = thread::spawn(move || -> Result<()> {
        // Non-blocking accept loop so we can notice when the cargo test
        // process has exited and stop cleanly.
        listener
            .set_nonblocking(true)
            .context("set listener non-blocking")?;
        let mut drivers: Vec<thread::JoinHandle<()>> = Vec::new();
        while !accept_stop.load(Ordering::Acquire) {
            match listener.accept() {
                Ok((conn, _addr)) => {
                    let pd = profile_dir_for_accept.clone();
                    let captures = Arc::clone(&captures_for_accept);
                    drivers.push(thread::spawn(move || {
                        if let Err(e) = handle_announce(conn, &pd, captures) {
                            eprintln!("gdb driver error: {e}");
                        }
                    }));
                    // Reap finished drivers so the vec doesn't grow
                    // unbounded across long test runs.
                    drivers.retain(|h| !h.is_finished());
                }
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    thread::sleep(Duration::from_millis(20));
                }
                Err(e) => return Err(e).context("accept"),
            }
        }
        // Drain remaining drivers — outer-VM trace files must finish
        // writing before shutdown returns.
        for d in drivers {
            let _ = d.join();
        }
        Ok(())
    });

    Ok(GdbDriver {
        profile_dir: profile_dir.to_path_buf(),
        sock_path,
        stop,
        accept: Some(accept_handle),
        captures,
    })
}

/// Loose-mode entry point. Stands up the driver, then runs `cargo
/// test` with `ANCHOR_GDB_SOCKET` and `--test-threads=1`.
///
/// `--test-threads=1` is forced because `VM_DEBUG_PORT` is a
/// process-wide env var and sbpf reads it lazily inside each
/// `EbpfVm::new` — two test threads each running `svm()` with
/// different ports would have the last `set_var` win and both VMs
/// collide on bind. Proper per-thread ports would need sbpf to accept
/// the port through a non-env channel (fork).
#[allow(clippy::too_many_arguments)]
pub fn run_gdb_mode(
    cargo_cwd: &Path,
    current_package: Option<&str>,
    profile_feature: &str,
    profile_dir: &Path,
    test_filter: Option<&str>,
) -> Result<()> {
    let driver = start_gdb_driver(profile_dir)?;

    let mut cmd = std::process::Command::new("cargo");
    cmd.current_dir(cargo_cwd)
        .env(SOCKET_ENV, driver.sock_path())
        .env("ANCHOR_PROFILE_DIR", profile_dir)
        .env("RUST_TEST_THREADS", "1")
        .arg("test")
        .arg("--features")
        .arg(profile_feature);
    if let Some(pkg) = current_package {
        cmd.arg("-p").arg(pkg);
    }
    cmd.arg("--").arg("--test-threads=1");
    if let Some(filter) = test_filter {
        cmd.arg(filter);
    }
    let test_status = cmd.status().context("spawn cargo test")?;

    drop(driver);

    if !test_status.success() {
        return Err(anyhow!("cargo test failed"));
    }
    Ok(())
}

/// One announcement = one outer VM's port. Reads `"<port>\t<test_name>\n"`
/// off the UDS connection, connects to the port, drives the VM to
/// termination while probing for nested CPI listeners on the same port.
fn handle_announce(
    conn: UnixStream,
    profile_dir: &Path,
    captures: Arc<Mutex<Vec<PendingCapture>>>,
) -> Result<()> {
    let mut rdr = BufReader::new(conn);
    let mut line = String::new();
    rdr.read_line(&mut line).context("read announce")?;
    let line = line.trim_end();
    let (port_str, test_name) = line
        .split_once('\t')
        .ok_or_else(|| anyhow!("malformed announce: {line:?}"))?;
    let port: u16 = port_str.parse().context("parse port")?;

    let test_dir = profile_dir.join(sanitize(test_name));
    std::fs::create_dir_all(&test_dir).with_context(|| format!("create {}", test_dir.display()))?;

    // Retry-connect: sbpf may not have bound yet when we get the announce
    // (v2-testing announces before setting env + VM construct). A single
    // announced LiteSVM can execute multiple outer transactions, so keep
    // accepting sequential sessions on the same port until it goes quiet.
    let mut deadline = Duration::from_secs(5);
    while let Some(outer) = wait_for_connect(port, deadline) {
        let capture = drive_session(outer, port, test_name, 0)?;
        captures.lock().unwrap().push(PendingCapture {
            test_name: sanitize(test_name),
            capture,
        });
        deadline = Duration::from_millis(750);
    }
    Ok(())
}

struct PendingCapture {
    test_name: String,
    capture: CapturedInvocation,
}

struct CapturedInvocation {
    regs: Vec<u8>,
    insns: Vec<u8>,
    cu: Vec<u8>,
    children: Vec<CapturedInvocation>,
}

fn write_pending_captures(profile_dir: &Path, captures: &[PendingCapture]) -> Result<()> {
    let mut by_test = std::collections::BTreeMap::<&str, Vec<&CapturedInvocation>>::new();
    for pending in captures {
        by_test
            .entry(&pending.test_name)
            .or_default()
            .push(&pending.capture);
    }

    for (test_name, captures) in by_test {
        let test_dir = profile_dir.join(test_name);
        std::fs::create_dir_all(&test_dir)
            .with_context(|| format!("create {}", test_dir.display()))?;

        let mut flattened = Vec::new();
        for capture in captures {
            flatten_capture_postorder(capture, &mut flattened);
        }

        let mut regular = regular_invocation_stems(&test_dir)?;
        for capture in flattened {
            let step_count = capture.regs.len() / REG_BYTES;
            let regular_idx = regular
                .iter()
                .position(|r| !r.used && r.step_count == step_count)
                .or_else(|| regular.iter().position(|r| !r.used));
            let stem = if let Some(idx) = regular_idx {
                regular[idx].used = true;
                regular[idx].stem.clone()
            } else {
                let fallback = regular.len() + 1;
                test_dir.join(format!("{fallback:04}__tx1"))
            };
            write_capture_sidecars(capture, &stem)?;
        }
    }
    Ok(())
}

fn flatten_capture_postorder<'a>(
    capture: &'a CapturedInvocation,
    out: &mut Vec<&'a CapturedInvocation>,
) {
    for child in &capture.children {
        flatten_capture_postorder(child, out);
    }
    out.push(capture);
}

struct RegularInvocationStem {
    stem: PathBuf,
    step_count: usize,
    used: bool,
}

fn regular_invocation_stems(test_dir: &Path) -> Result<Vec<RegularInvocationStem>> {
    let mut found = Vec::new();
    if !test_dir.exists() {
        return Ok(Vec::new());
    }

    for entry in std::fs::read_dir(test_dir)? {
        let path = entry?.path();
        if path.extension().and_then(|e| e.to_str()) != Some("regs") {
            continue;
        }
        let Some(stem_str) = path.file_stem().and_then(|s| s.to_str()) else {
            continue;
        };
        if stem_str.ends_with(".gdb") {
            continue;
        }
        let Some((inv_part, tx_part)) = stem_str.split_once("__") else {
            continue;
        };
        let Some(tx_digits) = tx_part.strip_prefix("tx") else {
            continue;
        };
        let Ok(inv_seq) = inv_part.parse::<u32>() else {
            continue;
        };
        let Ok(tx_seq) = tx_digits.parse::<u32>() else {
            continue;
        };
        let step_count = std::fs::metadata(&path)
            .map(|m| m.len() as usize / REG_BYTES)
            .unwrap_or(0);
        found.push((
            tx_seq,
            inv_seq,
            RegularInvocationStem {
                stem: path.with_extension(""),
                step_count,
                used: false,
            },
        ));
    }

    found.sort_by_key(|(tx_seq, inv_seq, _)| (*tx_seq, *inv_seq));
    Ok(found.into_iter().map(|(_, _, stem)| stem).collect())
}

fn write_capture_sidecars(capture: &CapturedInvocation, stem: &Path) -> Result<()> {
    std::fs::write(stem.with_extension("gdb.regs"), &capture.regs)?;
    std::fs::write(stem.with_extension("gdb.insns"), &capture.insns)?;
    std::fs::write(stem.with_extension("gdb.cu"), &capture.cu)?;
    Ok(())
}

/// Drives one gdb session (one `EbpfVm::execute_program` invocation) to
/// termination. Recursive: if the session's step stalls (likely a CPI),
/// spawns a helper to connect to the same port and drive the nested
/// session, then resumes outer stepping.
fn drive_session(
    stream: TcpStream,
    port: u16,
    test_name: &str,
    depth: usize,
) -> Result<CapturedInvocation> {
    stream.set_nodelay(true).ok();
    let mut rsp = Rsp::new(stream);
    let started = Instant::now();
    let mut last_progress = started;

    rsp.send("QStartNoAckMode");
    let reply = rsp.recv();
    rsp.no_ack = reply == "OK";

    rsp.send("qSupported:multiprocess+;swbreak+;hwbreak+;vContSupported+");
    let _ = rsp.recv();

    rsp.send("?");
    let mut reply = rsp.recv();

    // Sidecar capture for the canonical invocation files produced by
    // TestNameCallback. We collect in memory so nested BPF CPIs can be
    // written in postorder, matching iterate_vm_traces' file stems.
    let mut capture = CapturedInvocation {
        regs: Vec::new(),
        insns: Vec::new(),
        cu: Vec::new(),
        children: Vec::new(),
    };

    let mut steps: u64 = 0;
    print_progress(test_name, port, depth, steps, started, false);
    loop {
        if reply.starts_with('W') || reply.starts_with('X') {
            break;
        }
        if !reply.starts_with('T') && !reply.starts_with('S') {
            eprintln!("unexpected stop reply: {reply}");
            break;
        }

        rsp.send("g");
        let regs_hex = rsp.recv();
        if regs_hex.is_empty() {
            break;
        }
        // Pseudo-register 12 = InstructionCountRemaining. Not included
        // in `g`'s dump, so read it separately.
        rsp.send("p0c");
        let cu_hex = rsp.recv();

        let (regs, pc, _cu_stub, insn) = decode_regs_hex(&regs_hex)?;
        let cu = decode_u64_hex(&cu_hex).unwrap_or(0);
        capture.regs.extend_from_slice(&regs);
        capture.cu.extend_from_slice(&cu.to_le_bytes());
        let insn_bytes = read_insn_at(&mut rsp, pc)?;
        capture.insns.extend_from_slice(&insn_bytes);
        let _ = insn;

        // Nested CPI probe: try connecting to the same port in a
        // background thread while we await the step reply. If the inner
        // VM has bound the port, our connect succeeds and we recurse.
        let nested_port = port;
        let nested_test_name = test_name.to_owned();
        let probe_cancel = Arc::new(AtomicBool::new(false));
        let probe_cancel_for_thread = Arc::clone(&probe_cancel);
        let probe = thread::spawn(move || -> Result<Option<CapturedInvocation>> {
            if let Some(inner) = probe_for_nested(
                nested_port,
                Duration::from_millis(250),
                &probe_cancel_for_thread,
            ) {
                return drive_session(inner, nested_port, &nested_test_name, depth + 1).map(Some);
            }
            Ok(None)
        });

        rsp.send("s");
        let step_reply = rsp.recv();
        probe_cancel.store(true, Ordering::Release);
        if let Ok(Ok(Some(child))) = probe.join() {
            capture.children.push(child);
        }
        if step_reply.is_empty() {
            // Stream closed — VM exited. Normal termination.
            break;
        }

        reply = step_reply;
        steps += 1;
        if last_progress.elapsed() >= Duration::from_secs(5) {
            print_progress(test_name, port, depth, steps, started, false);
            last_progress = Instant::now();
        }
    }
    print_progress(test_name, port, depth, steps, started, true);

    Ok(capture)
}

fn print_progress(
    test_name: &str,
    port: u16,
    depth: usize,
    steps: u64,
    started: Instant,
    done: bool,
) {
    let elapsed = started.elapsed().as_secs_f64().max(0.001);
    let rate = steps as f64 / elapsed;
    let indent = "  ".repeat(depth);
    let status = if done { "captured" } else { "stepping" };
    eprintln!(
        "{indent}gdb {status} {test_name} :{port} depth={depth} steps={steps} elapsed={:.1}s \
         rate={:.1}/s",
        elapsed, rate,
    );
}

fn wait_for_connect(port: u16, deadline: Duration) -> Option<TcpStream> {
    let start = Instant::now();
    while start.elapsed() < deadline {
        if let Ok(s) = TcpStream::connect_timeout(
            &format!("127.0.0.1:{port}").parse().ok()?,
            Duration::from_millis(50),
        ) {
            return Some(s);
        }
        thread::sleep(Duration::from_millis(5));
    }
    None
}

fn probe_for_nested(port: u16, window: Duration, cancel: &AtomicBool) -> Option<TcpStream> {
    let start = Instant::now();
    while start.elapsed() < window && !cancel.load(Ordering::Acquire) {
        if let Ok(s) = TcpStream::connect(("127.0.0.1", port)) {
            return Some(s);
        }
        thread::yield_now();
    }
    None
}

fn decode_regs_hex(hex: &str) -> Result<([u8; REG_BYTES], u64, u64, [u8; 8])> {
    if hex.len() < REG_HEX_LEN {
        return Err(anyhow!(
            "register hex too short: {} bytes, payload={hex:?}",
            hex.len()
        ));
    }
    let mut regs = [0u8; REG_BYTES];
    for (i, pair) in hex.as_bytes()[..REG_HEX_LEN].chunks_exact(2).enumerate() {
        let s = std::str::from_utf8(pair).unwrap();
        regs[i] = u8::from_str_radix(s, 16).context("invalid hex")?;
    }
    // r11 = pc in sbpf's gdb target (last u64 in `g`'s dump).
    let pc = u64::from_le_bytes(regs[11 * 8..12 * 8].try_into().unwrap());
    let cu = 0; // CU read separately via `p 0c`.
    let insn = [0u8; 8]; // real bytes come from ELF, not gdb
    Ok((regs, pc, cu, insn))
}

fn decode_u64_hex(hex: &str) -> Option<u64> {
    if hex.len() < 16 {
        return None;
    }
    let mut bytes = [0u8; 8];
    for (i, pair) in hex.as_bytes()[..16].chunks_exact(2).enumerate() {
        bytes[i] = u8::from_str_radix(std::str::from_utf8(pair).ok()?, 16).ok()?;
    }
    Some(u64::from_le_bytes(bytes))
}

fn read_insn_at(rsp: &mut Rsp, pc: u64) -> Result<[u8; 8]> {
    // The sbpf gdb stub reports PC as the program virtual byte address.
    // Reading memory at that same address returns the raw instruction bytes.
    rsp.send(&format!("m{pc:x},8"));
    decode_bytes_hex::<8>(&rsp.recv())
}

fn decode_bytes_hex<const N: usize>(hex: &str) -> Result<[u8; N]> {
    if hex.len() < N * 2 {
        return Err(anyhow!(
            "memory hex too short: {} bytes, payload={hex:?}",
            hex.len()
        ));
    }
    let mut out = [0u8; N];
    for (i, pair) in hex.as_bytes()[..N * 2].chunks_exact(2).enumerate() {
        out[i] =
            u8::from_str_radix(std::str::from_utf8(pair).unwrap(), 16).context("invalid hex")?;
    }
    Ok(out)
}

fn sanitize(s: &str) -> String {
    s.chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect()
}

// ---------------------------------------------------------------------------
// Minimal GDB Remote Serial Protocol client. Just enough for our step loop.

struct Rsp {
    stream: TcpStream,
    buf: Vec<u8>,
    no_ack: bool,
}

impl Rsp {
    fn new(stream: TcpStream) -> Self {
        Self {
            stream,
            buf: Vec::with_capacity(512),
            no_ack: false,
        }
    }

    fn send(&mut self, payload: &str) {
        let mut cksum: u32 = 0;
        for b in payload.bytes() {
            cksum = cksum.wrapping_add(b as u32);
        }
        let pkt = format!("${payload}#{:02x}", cksum & 0xff);
        let _ = self.stream.write_all(pkt.as_bytes());
        let _ = self.stream.flush();
        if !self.no_ack {
            let mut one = [0u8; 1];
            let _ = self.stream.read_exact(&mut one);
        }
    }

    fn recv(&mut self) -> String {
        let mut one = [0u8; 1];
        loop {
            if self.stream.read_exact(&mut one).is_err() {
                return String::new();
            }
            if one[0] == b'$' {
                break;
            }
        }
        self.buf.clear();
        loop {
            if self.stream.read_exact(&mut one).is_err() {
                return String::new();
            }
            if one[0] == b'#' {
                break;
            }
            self.buf.push(one[0]);
        }
        let mut cksum = [0u8; 2];
        let _ = self.stream.read_exact(&mut cksum);
        if !self.no_ack {
            let _ = self.stream.write_all(b"+");
            let _ = self.stream.flush();
        }
        // Decode GDB RLE: a `*` followed by a count byte means "repeat the
        // previous char N more times", where N = count_byte - 28. This is
        // what sbpf's stub uses to shrink register dumps full of zeros
        // down from 208 bytes to ~45.
        let decoded = rle_decode(&self.buf);
        String::from_utf8_lossy(&decoded).into_owned()
    }
}

fn rle_decode(src: &[u8]) -> Vec<u8> {
    // GDB RSP RLE: `X*N` where N is an ASCII char, means the total
    // run length of X (including the literal X) is `N - 29 + 1`. So
    // we push `N - 29` additional copies of the preceding char.
    let mut out = Vec::with_capacity(src.len() * 4);
    let mut i = 0;
    while i < src.len() {
        let c = src[i];
        if c == b'*' && i + 1 < src.len() && !out.is_empty() {
            let n = (src[i + 1] as usize).saturating_sub(29);
            let last = *out.last().unwrap();
            for _ in 0..n {
                out.push(last);
            }
            i += 2;
        } else {
            out.push(c);
            i += 1;
        }
    }
    out
}
