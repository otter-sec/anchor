import assert from "assert";
import * as anchor from "@anchor-lang/core";
import * as borsh from "borsh";
import { Program } from "@anchor-lang/core";
import { Callee } from "../target/types/callee";
import { Caller } from "../target/types/caller";
import { Malicious } from "../target/types/malicious";
import { ConfirmOptions } from "@solana/web3.js";

const { SystemProgram } = anchor.web3;

describe("CPI return", () => {
  const provider = anchor.AnchorProvider.env();
  anchor.setProvider(provider);

  const callerProgram = anchor.workspace.Caller as Program<Caller>;
  const calleeProgram = anchor.workspace.Callee as Program<Callee>;
  const maliciousProgram = anchor.workspace.Malicious as Program<Malicious>;

  const getReturnLog = (confirmedTransaction) => {
    const prefix = "Program return: ";
    let log = confirmedTransaction.meta.logMessages.find((log) =>
      log.startsWith(prefix)
    );
    log = log.slice(prefix.length);
    const [key, data] = log.split(" ", 2);
    const buffer = Buffer.from(data, "base64");
    return [key, data, buffer];
  };

  const cpiReturn = anchor.web3.Keypair.generate();

  const confirmOptions: ConfirmOptions = {
    commitment: "confirmed",
    preflightCommitment: "confirmed",
    skipPreflight: true,
    maxRetries: 3,
  };

  it("can initialize", async () => {
    await calleeProgram.methods
      .initialize()
      .accounts({
        account: cpiReturn.publicKey,
        user: provider.wallet.publicKey,
        systemProgram: SystemProgram.programId,
      })
      .signers([cpiReturn])
      .rpc();
  });

  it("can return u64 from a cpi", async () => {
    const tx = await callerProgram.methods
      .cpiCallReturnU64()
      .accounts({
        cpiReturn: cpiReturn.publicKey,
        cpiReturnProgram: calleeProgram.programId,
      })
      .rpc(confirmOptions);
    let t = await provider.connection.getTransaction(tx, {
      commitment: "confirmed",
      maxSupportedTransactionVersion: 0,
    });

    const [key, data, buffer] = getReturnLog(t);
    assert.equal(key, calleeProgram.programId);

    // Check for matching log on receive side
    let receiveLog = t.meta.logMessages.find(
      (log) => log == `Program data: ${data}`
    );
    assert(receiveLog !== undefined);

    const reader = new borsh.BinaryReader(buffer);
    assert.equal(reader.readU64().toNumber(), 10);
  });

  it("can make a non-cpi call to a function that returns a u64", async () => {
    const tx = await calleeProgram.methods
      .returnU64()
      .accounts({
        account: cpiReturn.publicKey,
      })
      .rpc(confirmOptions);
    let t = await provider.connection.getTransaction(tx, {
      commitment: "confirmed",
      maxSupportedTransactionVersion: 0,
    });
    const [key, , buffer] = getReturnLog(t);
    assert.equal(key, calleeProgram.programId);
    const reader = new borsh.BinaryReader(buffer);
    assert.equal(reader.readU64().toNumber(), 10);
  });

  it("can return a struct from a cpi", async () => {
    const tx = await callerProgram.methods
      .cpiCallReturnStruct()
      .accounts({
        cpiReturn: cpiReturn.publicKey,
        cpiReturnProgram: calleeProgram.programId,
      })
      .rpc(confirmOptions);
    let t = await provider.connection.getTransaction(tx, {
      commitment: "confirmed",
      maxSupportedTransactionVersion: 0,
    });

    const [key, data, buffer] = getReturnLog(t);
    assert.equal(key, calleeProgram.programId);

    // Check for matching log on receive side
    let receiveLog = t.meta.logMessages.find(
      (log) => log == `Program data: ${data}`
    );
    assert(receiveLog !== undefined);

    // Deserialize the struct and validate
    class Assignable {
      constructor(properties) {
        Object.keys(properties).map((key) => {
          this[key] = properties[key];
        });
      }
    }
    class Data extends Assignable {}
    const schema = new Map([
      [Data, { kind: "struct", fields: [["value", "u64"]] }],
    ]);
    const deserialized = borsh.deserialize(schema, Data, buffer);
    // @ts-ignore
    assert(deserialized.value.toNumber() === 11);
  });

  it("can return a vec from a cpi", async () => {
    const tx = await callerProgram.methods
      .cpiCallReturnVec()
      .accounts({
        cpiReturn: cpiReturn.publicKey,
        cpiReturnProgram: calleeProgram.programId,
      })
      .rpc(confirmOptions);
    let t = await provider.connection.getTransaction(tx, {
      commitment: "confirmed",
      maxSupportedTransactionVersion: 0,
    });

    const [key, data, buffer] = getReturnLog(t);
    assert.equal(key, calleeProgram.programId);

    // Check for matching log on receive side
    let receiveLog = t.meta.logMessages.find(
      (log) => log == `Program data: ${data}`
    );
    assert(receiveLog !== undefined);

    const reader = new borsh.BinaryReader(buffer);
    const array = reader.readArray(() => reader.readU8());
    assert.deepStrictEqual(array, [12, 13, 14, 100]);
  });

  it("sets a return value in idl", async () => {
    // @ts-expect-error
    const returnu64Instruction = calleeProgram._idl.instructions.find(
      (f) => f.name == "returnU64"
    );
    assert.equal(returnu64Instruction.returns, "u64");

    // @ts-expect-error
    const returnStructInstruction = calleeProgram._idl.instructions.find(
      (f) => f.name == "returnStruct"
    );
    assert.deepStrictEqual(returnStructInstruction.returns, {
      defined: { name: "structReturn" },
    });
  });

  it("can return a u64 via view", async () => {
    // @ts-expect-error
    assert(new anchor.BN(99).eq(await callerProgram.views.returnU64()));
    // Via methods API
    assert(
      new anchor.BN(99).eq(await callerProgram.methods.returnU64().view())
    );
  });

  it("can return a struct via view", async () => {
    // @ts-expect-error
    const struct = await callerProgram.views.returnStruct();
    assert(struct.a.eq(new anchor.BN(1)));
    assert(struct.b.eq(new anchor.BN(2)));
    // Via methods API
    const struct2 = await callerProgram.methods.returnStruct().view();
    assert(struct2.a.eq(new anchor.BN(1)));
    assert(struct2.b.eq(new anchor.BN(2)));
  });

  it("can return a vec via view", async () => {
    // @ts-expect-error
    const vec = await callerProgram.views.returnVec();
    assert(vec[0].eq(new anchor.BN(1)));
    assert(vec[1].eq(new anchor.BN(2)));
    assert(vec[2].eq(new anchor.BN(3)));
    // Via methods API
    const vec2 = await callerProgram.methods.returnVec().view();
    assert(vec2[0].eq(new anchor.BN(1)));
    assert(vec2[1].eq(new anchor.BN(2)));
    assert(vec2[2].eq(new anchor.BN(3)));
  });

  it("can return a u64 from an account via view", async () => {
    const value = new anchor.BN(10);
    assert(
      value.eq(
        await calleeProgram.methods
          .returnU64FromAccount()
          .accounts({ account: cpiReturn.publicKey })
          .view()
      )
    );
  });

  it("can't call view on mutable instruction", async () => {
    assert.equal(calleeProgram.views.initialize, undefined);
    try {
      await calleeProgram.methods
        .initialize()
        .accounts({
          account: cpiReturn.publicKey,
          user: provider.wallet.publicKey,
          systemProgram: SystemProgram.programId,
        })
        .signers([cpiReturn])
        .view();
    } catch (e) {
      assert(e.message.includes("Method does not support views"));
    }
  });

  // === VULNERABILITY PoC: Return data spoofing ===

  it("VULNERABILITY: get_unchecked() reads spoofed return data (old behavior)", async () => {
    // This demonstrates what happened BEFORE the fix.
    // get_unchecked() preserves the old behavior for backward compatibility,
    // showing that a malicious program can spoof return data.
    const tx = await callerProgram.methods
      .cpiCallReturnU64Spoofed()
      .accounts({
        authority: provider.wallet.publicKey,
        cpiReturn: cpiReturn.publicKey,
        cpiReturnProgram: calleeProgram.programId,
        maliciousProgram: maliciousProgram.programId,
      })
      .rpc(confirmOptions);

    let t = await provider.connection.getTransaction(tx, {
      commitment: "confirmed",
      maxSupportedTransactionVersion: 0,
    });

    // Find the "Program data:" log emitted by the caller
    const dataPrefix = "Program data: ";
    const dataLogs = t.meta.logMessages.filter((log) =>
      log.startsWith(dataPrefix)
    );
    const lastDataLog = dataLogs[dataLogs.length - 1];
    const b64Data = lastDataLog.slice(dataPrefix.length);
    const buffer = Buffer.from(b64Data, "base64");

    const reader = new borsh.BinaryReader(buffer);
    const spoofedValue = reader.readU64().toNumber();

    // Callee returned 10, but malicious program overwrote with 999.
    // get_unchecked() (old behavior) happily returns the spoofed value.
    assert.notEqual(
      spoofedValue,
      10,
      "Expected spoofed value, not the real callee value"
    );
    assert.equal(
      spoofedValue,
      999,
      "Malicious program successfully spoofed return data"
    );

    console.log(`\n  VULNERABILITY CONFIRMED (get_unchecked / old behavior):`);
    console.log(`    Callee returned: 10`);
    console.log(`    Malicious spoofed: 999`);
    console.log(`    Caller received: ${spoofedValue} (SPOOFED!)\n`);
  });

  it("FIX: get() keeps the original return data after a later CPI from a different program", async () => {
    const tx = await callerProgram.methods
      .cpiCallReturnU64SpoofedPreserved()
      .accounts({
        authority: provider.wallet.publicKey,
        cpiReturn: cpiReturn.publicKey,
        cpiReturnProgram: calleeProgram.programId,
        maliciousProgram: maliciousProgram.programId,
      })
      .rpc(confirmOptions);

    const t = await provider.connection.getTransaction(tx, {
      commitment: "confirmed",
      maxSupportedTransactionVersion: 0,
    });

    const dataPrefix = "Program data: ";
    const dataLogs = t.meta.logMessages.filter((log) =>
      log.startsWith(dataPrefix)
    );
    const lastDataLog = dataLogs[dataLogs.length - 1];
    const b64Data = lastDataLog.slice(dataPrefix.length);
    const buffer = Buffer.from(b64Data, "base64");

    const reader = new borsh.BinaryReader(buffer);
    const value = reader.readU64().toNumber();

    assert.equal(
      value,
      10,
      "Expected get() to return the original callee value, not the later spoofed one"
    );
  });

  it("FIX: get() keeps the original return data after a later CPI to the same program", async () => {
    const tx = await callerProgram.methods
      .cpiCallReturnU64SameProgramSpoofed()
      .accounts({
        cpiReturn: cpiReturn.publicKey,
        cpiReturnProgram: calleeProgram.programId,
      })
      .rpc(confirmOptions);

    const t = await provider.connection.getTransaction(tx, {
      commitment: "confirmed",
      maxSupportedTransactionVersion: 0,
    });

    const dataPrefix = "Program data: ";
    const dataLogs = t.meta.logMessages.filter((log) =>
      log.startsWith(dataPrefix)
    );
    const lastDataLog = dataLogs[dataLogs.length - 1];
    const b64Data = lastDataLog.slice(dataPrefix.length);
    const buffer = Buffer.from(b64Data, "base64");

    const reader = new borsh.BinaryReader(buffer);
    const value = reader.readU64().toNumber();

    assert.equal(
      value,
      10,
      "Expected get() to return the original callee value, not the later spoofed one"
    );
  });
});
