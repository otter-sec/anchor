// Test-owned historical Surfpool fork, started from the launch plan `anchor
// test` prepared for
// this suite (ANCHOR_HISTORICAL_PLAN). The boundary is chosen here, in test
// code. Anchor compiled
// the fixtures; Surfpool applies them; SVM Replay executes historical state.
// The clock stays
// frozen until a test advances it explicitly.
"use strict";
const { spawn } = require("node:child_process");
const { randomBytes } = require("node:crypto");
const { closeSync, mkdirSync, openSync, readFileSync, rmSync, writeFileSync } = require("node:fs");
const { createServer } = require("node:net");
const { join } = require("node:path");
const anchor = require("@anchor-lang/core");

const PLAN_SCHEMA = "anchor-historical-plan/v1";
const SYSTEM_PROGRAM = "11111111111111111111111111111111";

class HistoricalForkError extends Error {
  constructor(code, message) {
    super(`${code}: ${message}`);
    this.code = code;
  }
}

function plan() {
  const path = process.env.ANCHOR_HISTORICAL_PLAN;
  if (!path) {
    throw new HistoricalForkError("NO_PLAN", "run these tests with `anchor test` in a workspace with a [historical] section");
  }
  const value = JSON.parse(readFileSync(path, "utf8"));
  if (value.schema !== PLAN_SCHEMA) {
    throw new HistoricalForkError("PLAN_SCHEMA", `${path} is ${value.schema}, expected ${PLAN_SCHEMA}`);
  }
  return value;
}

function freePort(host) {
  return new Promise((resolve, reject) => {
    const server = createServer();
    server.once("error", reject);
    server.listen(0, host, () => {
      const { port } = server.address();
      server.close(() => resolve(port));
    });
  });
}

async function freePortPair(host) {
  for (let attempt = 0; attempt < 20; attempt++) {
    const port = await freePort(host);
    if (port < 65535) {
      const next = await new Promise((resolve) => {
        const server = createServer();
        server.once("error", () => resolve(false));
        server.listen(port + 1, host, () => server.close(() => resolve(true)));
      });
      if (next) return port;
    }
  }
  throw new HistoricalForkError("PORTS", `no free RPC/WebSocket port pair on ${host}`);
}

const sleep = (ms) => new Promise((resolve) => setTimeout(resolve, ms));

class HistoricalFork {
  /**
   * Starts a historical Surfpool fork after `after`, a transaction signature
   * or slot.
   * `dataDir` optionally selects Surfpool's historical cache directory.
   */
  static async start({ after, dataDir, startupTimeoutMs } = {}) {
    if (typeof after !== "string" || after.length === 0) {
      throw new HistoricalForkError("BOUNDARY", "choose `after` (a transaction signature or slot) in test code");
    }
    const launch = plan();
    const host = launch.surfpool.host;
    const surfnetId = `anchor-${process.pid}-${randomBytes(8).toString("hex")}`;
    const directory = join(launch.registry, "..", "forks-data", surfnetId);
    mkdirSync(join(directory, ".."), { recursive: true, mode: 0o700 });
    mkdirSync(directory, { mode: 0o700 });
    // A private image installed by this fork's launch arguments checks startup
    // readiness, not transport authentication for subsequent requests.
    const sentinel = {
      pubkey: anchor.web3.Keypair.generate().publicKey.toBase58(),
      lamports: 2_000_000 + randomBytes(2).readUInt16LE(0),
      data: randomBytes(16).toString("base64"),
    };
    const sentinelFile = join(directory, "sentinel.snapshot.json");
    writeFileSync(sentinelFile, `{"${sentinel.pubkey}":{"lamports":${sentinel.lamports},"owner":"${SYSTEM_PROGRAM}","executable":false,"rentEpoch":18446744073709551615,"data":"${sentinel.data}","parsedData":null}}\n`, { flag: "wx", mode: 0o600 });
    const port = await freePortPair(host);
    // Surfpool always serves its Studio/scenario HTTP endpoint, even with
    // --no-studio: give each
    // fork a private port so concurrent forks never share or expose another
    // fork's server.
    let studioPort = await freePort(host);
    while (studioPort === port || studioPort === port + 1) studioPort = await freePort(host);
    const args = [...launch.surfpool.args, "--after", after, "--host", host, "--port", String(port),
      "--ws-port", String(port + 1), "--studio-port", String(studioPort), "--surfnet-id", surfnetId,
      "--snapshot", sentinelFile];
    if (dataDir) args.push("--historical-data-dir", dataDir);
    const logPath = join(directory, "surfpool.log");
    const log = openSync(logPath, "ax", 0o600);
    const child = spawn(launch.surfpool.program, args, { cwd: launch.surfpool.cwd, stdio: ["ignore", log, log] });
    closeSync(log);
    const exited = new Promise((resolve) => child.once("exit", (code, signal) => resolve({ code, signal })));
    try {
      await new Promise((resolve, reject) => {
        child.once("spawn", resolve);
        child.once("error", reject);
      });
    } catch (error) {
      throw new HistoricalForkError("SPAWN", `cannot start ${launch.surfpool.program}: ${error.code ?? error.message}`);
    }
    const record = join(launch.registry, `${surfnetId}.json`);
    try {
      writeFileSync(record, JSON.stringify({ pid: child.pid, surfnetId, port, studioPort, after }) + "\n", { flag: "wx", mode: 0o600 });
    } catch (error) {
      child.kill("SIGKILL");
      await exited;
      throw new HistoricalForkError("RECORD", `cannot record fork ${surfnetId} for cleanup: ${error.message}`);
    }
    const fork = new HistoricalFork({ launch, child, exited, record, directory, host, port, surfnetId, after, logPath });
    try {
      await fork.#ready(sentinel, startupTimeoutMs ?? launch.startupWaitMs);
    } catch (error) {
      await fork.close();
      throw error;
    }
    return fork;
  }

  #launch; #child; #exited; #record; #logPath; #verified = false; #closed = false; #connections = [];
  #connectionFailures = [];
  #signer = anchor.web3.Keypair.generate();

  constructor({ launch, child, exited, record, directory, host, port, surfnetId, after, logPath }) {
    this.#launch = launch;
    this.#child = child;
    this.#exited = exited;
    this.#record = record;
    this.#logPath = logPath;
    this.directory = directory;
    this.url = `http://${host}:${port}`;
    this.wsUrl = `ws://${host}:${port + 1}`;
    this.surfnetId = surfnetId;
    this.after = after;
    this.pid = child.pid;
    child.once("exit", () => {
      this.#verified = false;
      this.#connectionFailures.push(...this.#stopConnections());
    });
  }

  #assertRunning() {
    if (this.#closed) throw new HistoricalForkError("NOT_READY", "this fork is closed");
    if (this.#child.exitCode !== null || this.#child.signalCode !== null) {
      throw new HistoricalForkError("EXITED", "this fork's surfpool child has exited");
    }
  }

  #assertReady() {
    this.#assertRunning();
    if (!this.#verified) throw new HistoricalForkError("NOT_READY", "this fork is not running");
  }

  async #call(method, params = []) {
    this.#assertRunning();
    const response = await fetch(this.url, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ jsonrpc: "2.0", id: 1, method, params }),
      signal: AbortSignal.timeout(30_000),
    });
    const value = await response.json();
    this.#assertRunning();
    if (value.error) throw new HistoricalForkError("RPC", `${method}: ${JSON.stringify(value.error)}`);
    return value.result;
  }

  /**
   * Surfpool's typed startup failure (`Error: CODE: message`), or a generic exit.
   */
  #exitError(status) {
    let text = "";
    let unreadable = "";
    try { text = readFileSync(this.#logPath, "utf8"); } catch (error) { unreadable = `; its log is unreadable: ${error.message}`; }
    const typed = [...text.matchAll(/^Error: ([A-Z][A-Z0-9_]+): (.*)$/gm)].at(-1);
    if (typed) return new HistoricalForkError(typed[1], `${typed[2]} (see ${this.#logPath})`);
    return new HistoricalForkError("EXITED", `surfpool exited during startup (${JSON.stringify(status)}); see ${this.#logPath}${unreadable}`);
  }

  async #ready(sentinel, timeoutMs) {
    const deadline = Date.now() + timeoutMs;
    let exit;
    this.#exited.then((status) => { exit = status; });
    let identified = false;
    let last = "no response";
    while (Date.now() < deadline) {
      if (exit) throw this.#exitError(exit);
      try {
        if (!identified) {
          const account = (await this.#call("getAccountInfo", [sentinel.pubkey, { encoding: "base64" }])).value;
          identified = account !== null && account.lamports === sentinel.lamports && account.owner === SYSTEM_PROGRAM
            && account.executable === false && account.data[0] === sentinel.data && account.data[1] === "base64";
          if (account !== null && !identified) throw new HistoricalForkError("IDENTITY", "the listener on this port is not this fork");
          last = identified ? "identified" : "sentinel not visible yet";
        }
        if (identified) {
          const info = (await this.#call("surfnet_getSurfnetInfo")).value;
          if (typeof info?.startup?.phase !== "string" || !Array.isArray(info.runbookExecutions)) {
            throw new HistoricalForkError("PROTOCOL", "surfnet_getSurfnetInfo reports no startup phase or runbooks; this Surfpool cannot prove the fork is ready");
          }
          if (info.startup.error) throw new HistoricalForkError("STARTUP", JSON.stringify(info.startup.error));
          const failed = info.runbookExecutions.filter((run) => run.errors?.length);
          if (failed.length) throw new HistoricalForkError("FIXTURES", JSON.stringify(failed));
          last = `startup ${info.startup.phase}, runbooks ${JSON.stringify(info.runbookExecutions.map((run) => run.completedAt))}`;
          if (info.startup.phase === "ready" && info.runbookExecutions.every((run) => run.completedAt != null) && !exit) {
            await this.#checkFixtures();
            this.#verified = true;
            await this.#checkRent();
            return;
          }
        }
      } catch (error) {
        if (error instanceof HistoricalForkError) throw error;
        last = String(error.cause?.code ?? error.message);
      }
      await sleep(100);
    }
    if (exit) throw this.#exitError(exit);
    throw new HistoricalForkError("STARTUP_TIMEOUT", `not ready within ${timeoutMs} ms (last: ${last}); raise [surfpool] startup_wait or see ${this.#logPath}`);
  }

  /**
   * Anchor funded generated mint/token images with its default rent;
   * the boundary decides.
   */
  async #checkRent() {
    for (const check of this.#launch.rentChecks) {
      const minimum = await this.#call("getMinimumBalanceForRentExemption", [check.dataLength]);
      if (check.lamports < minimum) {
        throw new HistoricalForkError("FIXTURE_RENT_MISMATCH", `${check.pubkey} has ${check.lamports} lamports; this boundary requires ${minimum}`);
      }
    }
  }

  async #checkFixtures() {
    const args = this.#launch.surfpool.args;
    // The plan starts with `start`, then Anchor's compiled snapshot if any.
    // Later snapshots belong to extra_args and may contain remote/null entries.
    if (args[1] !== "--snapshot") return;
    {
      const images = JSON.parse(readFileSync(args[2], "utf8"));
      for (const [pubkey, image] of Object.entries(images)) {
        const actual = (await this.#call("getAccountInfo", [pubkey, { encoding: "base64", commitment: "processed" }])).value;
        if (!actual || actual.lamports !== image.lamports || actual.owner !== image.owner
            || actual.executable !== image.executable || actual.data[0] !== image.data) {
          throw new HistoricalForkError("FIXTURE_MISMATCH", `${pubkey} differs from its declared image after startup`);
        }
      }
    }
  }

  /**
   * An RPC call to this verified fork, including explicit `surfnet_*` controls.
   */
  async rpc(method, params = []) {
    this.#assertReady();
    return this.#call(method, params);
  }

  /**
   * An Anchor provider for this fork; defaults to one disposable key per fork.
   * Fund it explicitly with setAccount. With a real funded key at a recent
   * boundary, leaked signed bytes may still be valid on mainnet.
   * A durable nonce can remain valid until its on-chain nonce advances.
   */
  provider(keypair, options = { commitment: "processed", preflightCommitment: "processed" }) {
    this.#assertReady();
    const signer = keypair ?? this.#signer;
    const connection = new anchor.web3.Connection(this.url, {
      commitment: options.commitment,
      wsEndpoint: this.wsUrl,
      fetch: async (input, init) => {
        this.#assertReady();
        const response = await fetch(input, init);
        this.#assertReady();
        return response;
      },
    });
    const socket = connection._rpcWebSocket;
    const connect = socket.connect.bind(socket);
    socket.connect = () => {
      try {
        this.#assertReady();
      } catch (error) {
        // web3 does not await its subscription updater: throwing here would
        // become an unhandled rejection instead of reaching the subscriber.
        if (!(error instanceof HistoricalForkError)) throw error;
        console.error(`historical fork ${this.surfnetId} WebSocket connection refused: ${error.message}`);
        return;
      }
      return connect();
    };
    this.#connections.push(connection);
    return new anchor.AnchorProvider(connection, new anchor.Wallet(signer), options);
  }

  /**
   * Defines an account in this fork only, e.g. a test account or wallet funding.
   */
  async setAccount(pubkey, { lamports = 0, owner = SYSTEM_PROGRAM, data = Buffer.alloc(0), executable = false } = {}) {
    return this.rpc("surfnet_setAccount", [pubkey.toString(), { lamports, owner: owner.toString(), data: Buffer.from(data).toString("hex"), executable, rentEpoch: 0 }]);
  }

  /**
   * Fast ELF setup through Surfpool's program-writing helper: installs an
   * upgradeable program at `programId` in this fork. This is not a loader
   * deployment; use a real deployment to test the loader. The program becomes
   * callable after the next explicit `advance()`.
   */
  async installProgram(programId, soPath, authority) {
    const elf = readFileSync(soPath);
    const id = new anchor.web3.PublicKey(programId.toString());
    const loader = new anchor.web3.PublicKey("BPFLoaderUpgradeab1e11111111111111111111111");
    const [programData] = anchor.web3.PublicKey.findProgramAddressSync([id.toBuffer()], loader);
    await this.setAccount(id);
    await this.setAccount(programData);
    const upgradeAuthority = (authority ?? this.#signer.publicKey).toString();
    for (let offset = 0; offset < elf.length; offset += 4096) {
      await this.rpc("surfnet_writeProgram", [id.toBase58(), elf.subarray(offset, offset + 4096).toString("hex"), offset, upgradeAuthority]);
    }
  }

  /** Explicit local slots; the historical clock never advances on its own. */
  async advance(slots = 1) {
    if (!Number.isSafeInteger(slots) || slots < 1) throw new HistoricalForkError("ADVANCE", "slots must be a positive integer");
    const slot = await this.rpc("getSlot", [{ commitment: "processed" }]);
    const target = slot + slots;
    if (!Number.isSafeInteger(slot) || !Number.isSafeInteger(target)) throw new HistoricalForkError("ADVANCE", `cannot advance slot ${slot} by ${slots}`);
    return this.rpc("surfnet_timeTravel", [{ absoluteSlot: target }]);
  }

  #stopConnections() {
    const failures = [];
    for (const connection of this.#connections) {
      const socket = connection._rpcWebSocket;
      try {
        socket?.setAutoReconnect?.(false);
        clearTimeout(socket?.reconnect_timer_id);
        // Older rpc-websockets cannot close before its first connection.
        // 1001 also avoids web3's explicit-1000 subscription reconnect path.
        if (socket?.socket) socket.close(1001);
      } catch (error) {
        failures.push(error);
      }
    }
    this.#connections = [];
    return failures;
  }

  /**
   * Stops this fork's own child; never signals or calls another listener.
   * Remove event and account listeners first: each `provider()` WebSocket is
   * closed here, and on child exit, with automatic reconnect disabled.
   */
  async close() {
    if (this.#closed) return;
    this.#closed = true;
    this.#connectionFailures.push(...this.#stopConnections());
    if (this.#child.exitCode === null && this.#child.signalCode === null) {
      this.#child.kill("SIGTERM");
      let timer;
      const stopped = await Promise.race([
        this.#exited.then(() => true),
        new Promise((resolve) => { timer = setTimeout(() => resolve(false), Math.max(this.#launch.shutdownWaitMs, 5_000)); }),
      ]);
      clearTimeout(timer); // no timer may outlive close()
      if (!stopped) {
        this.#child.kill("SIGKILL");
        await this.#exited;
      }
    }
    rmSync(this.#record, { force: true });
    if (this.#connectionFailures.length) throw new AggregateError(this.#connectionFailures, "fork stopped, but RPC connection cleanup failed");
  }
}

module.exports = { HistoricalFork, HistoricalForkError };
