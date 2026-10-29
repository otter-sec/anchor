// Copied beside the generated tests; uses the same PR-generated fork helper.
import assert from "node:assert/strict";
import { createServer } from "node:http";
import { networkInterfaces } from "node:os";
import { readFileSync, statSync } from "node:fs";
import { join } from "node:path";
import * as anchor from "@anchor-lang/core";
import { HistoricalFork } from "./historical";
import type { Demo } from "../target/types/demo";
import idl from "../target/idl/demo.json";

describe("historical CI contract", () => {
  it("executes state changes, advances explicitly and cleans up after an error", async () => {
    assert.deepEqual(Object.keys(networkInterfaces()), ["lo"], "Provider networking must be disabled");
    const fork = await HistoricalFork.start({ after: "413023260" });
    const url = fork.url;
    const pid = fork.pid;
    try {
      const slot = await fork.rpc("getSlot", [{ commitment: "processed" }]);
      assert.equal(slot, 413023261);
      await new Promise((resolve) => setTimeout(resolve, 500));
      assert.equal(await fork.rpc("getSlot", [{ commitment: "processed" }]), slot);
      const provider = fork.provider();
      const plan = JSON.parse(readFileSync(process.env.ANCHOR_HISTORICAL_PLAN!, "utf8"));
      const workspaceKey = anchor.web3.Keypair.fromSecretKey(Uint8Array.from(JSON.parse(readFileSync(plan.wallet, "utf8"))));
      assert.notEqual(provider.wallet.publicKey.toBase58(), workspaceKey.publicKey.toBase58());
      assert.equal(fork.provider().wallet.publicKey.toBase58(), provider.wallet.publicKey.toBase58());
      assert.equal(fork.provider(workspaceKey).wallet.publicKey.toBase58(), workspaceKey.publicKey.toBase58());
      for (const directory of [plan.registry, fork.directory]) {
        assert.equal(statSync(directory).mode & 0o777, 0o700);
      }
      for (const file of [process.env.ANCHOR_HISTORICAL_PLAN!, join(fork.directory, "sentinel.snapshot.json"),
        join(fork.directory, "surfpool.log"), join(plan.registry, `${fork.surfnetId}.json`)]) {
        assert.equal(statSync(file).mode & 0o777, 0o600);
      }
      const program = new anchor.Program<Demo>(idl as Demo, provider);
      await fork.setAccount(provider.wallet.publicKey, { lamports: 10_000_000_000 });
      await fork.installProgram(program.programId, "target/deploy/demo.so");
      const [counter] = anchor.web3.PublicKey.findProgramAddressSync(
        [Buffer.from("counter")], program.programId,
      );
      await fork.setAccount(counter);
      await fork.advance(1);
      assert.equal(await fork.rpc("getSlot", [{ commitment: "processed" }]), slot + 1);
      await program.methods.initialize().rpc();
      await program.methods.increment().rpc();
      const account = await program.account.counter.fetch(counter);
      assert.equal(account.count.toString(), "1");
      assert.equal(account.authority.toBase58(), provider.wallet.publicKey.toBase58());
      await assert.rejects(fork.rpc("ci_nonexistent_method"), /-32601/);
      assert.equal(await fork.rpc("getSlot", [{ commitment: "processed" }]), slot + 1);
    } finally {
      await fork.close();
    }
    assert.throws(() => fork.provider(), /NOT_READY/);
    assert.throws(() => process.kill(pid, 0), { code: "ESRCH" });
    await assert.rejects(fetch(url, { signal: AbortSignal.timeout(2000) }));
  });

  it("refuses a replacement listener after its owned fork exits", async () => {
    const fork = await HistoricalFork.start({ after: "413023260" });
    const provider = fork.provider();
    let requests = 0;
    const replacement = createServer(async (request, response) => {
      const chunks: Buffer[] = [];
      for await (const chunk of request) chunks.push(Buffer.from(chunk));
      const rpc = JSON.parse(Buffer.concat(chunks).toString());
      requests++;
      response.writeHead(200, { "content-type": "application/json" });
      response.end(JSON.stringify({ jsonrpc: "2.0", id: rpc.id, result: 999 }));
    });
    try {
      process.kill(fork.pid, "SIGTERM");
      const deadline = Date.now() + 15_000;
      for (;;) {
        try { process.kill(fork.pid, 0); }
        catch (error) {
          if ((error as NodeJS.ErrnoException).code !== "ESRCH") throw error;
          break;
        }
        assert(Date.now() < deadline, "owned fork did not exit");
        await new Promise(resolve => setTimeout(resolve, 25));
      }
      await new Promise<void>((resolve, reject) => {
        replacement.once("error", reject);
        replacement.listen(Number(new URL(fork.url).port), "127.0.0.1", resolve);
      });
      await assert.rejects(fork.rpc("getSlot"), /EXITED/);
      assert.throws(() => fork.provider(), /EXITED/);
      await assert.rejects(provider.connection.getSlot("processed"), /EXITED/);
      assert.equal(requests, 0);
    } finally {
      replacement.closeAllConnections();
      if (replacement.listening) await new Promise<void>((resolve, reject) =>
        replacement.close(error => error ? reject(error) : resolve()));
      await fork.close();
    }
  });
});
