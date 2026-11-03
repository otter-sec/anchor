import * as anchor from "@anchor-lang/core";
import { assert } from "chai";

import { Events } from "../target/types/events";
import { EventsCaller } from "../target/types/events_caller";

describe("Events", () => {
  // Configure the client to use the local cluster.
  anchor.setProvider(anchor.AnchorProvider.env());
  const program = anchor.workspace.events as anchor.Program<Events>;
  const caller = anchor.workspace.eventsCaller as anchor.Program<EventsCaller>;
  const confirmOptions: anchor.web3.ConfirmOptions = {
    commitment: "confirmed",
    preflightCommitment: "confirmed",
    skipPreflight: true,
    maxRetries: 3,
  };

  type Event = anchor.IdlEvents<typeof program["idl"]>;
  const getEvent = async <E extends keyof Event>(
    eventName: E,
    methodName: keyof typeof program["methods"]
  ) => {
    let listenerId: number;
    const event = await new Promise<Event[E]>((res) => {
      listenerId = program.addEventListener(eventName, (event) => {
        res(event);
      });
      program.methods[methodName]().rpc();
    });
    await program.removeEventListener(listenerId);

    return event;
  };

  describe("Normal event", () => {
    it("Single event works", async () => {
      const event = await getEvent("myEvent", "initialize");

      assert.strictEqual(event.data.toNumber(), 5);
      assert.strictEqual(event.label, "hello");
    });

    it("Multiple events work", async () => {
      const eventOne = await getEvent("myEvent", "initialize");
      const eventTwo = await getEvent("myOtherEvent", "testEvent");

      assert.strictEqual(eventOne.data.toNumber(), 5);
      assert.strictEqual(eventOne.label, "hello");

      assert.strictEqual(eventTwo.data.toNumber(), 6);
      assert.strictEqual(eventTwo.label, "bye");
    });
  });

  describe("CPI event", () => {
    const config = {
      commitment: "confirmed",
      preflightCommitment: "confirmed",
      skipPreflight: true,
      maxRetries: 3,
    } as const;

    it("Works without accounts being specified", async () => {
      const tx = await program.methods.testEventCpi().transaction();
      const txHash = await program.provider.sendAndConfirm(
        tx,
        [],
        confirmOptions
      );
      const txResult = await program.provider.connection.getTransaction(
        txHash,
        {
          ...confirmOptions,
          maxSupportedTransactionVersion: 0,
        }
      );

      const ixData = anchor.utils.bytes.bs58.decode(
        txResult.meta.innerInstructions[0].instructions[0].data
      );
      const eventData = anchor.utils.bytes.base64.encode(ixData.slice(8));
      const event = program.coder.events.decode(eventData);

      assert.strictEqual(event.name, "myOtherEvent");
      assert.strictEqual(event.data.label, "cpi");
      assert.strictEqual((event.data.data as anchor.BN).toNumber(), 7);
    });

    it("Throws on unauthorized invocation", async () => {
      const tx = new anchor.web3.Transaction();
      tx.add(
        new anchor.web3.TransactionInstruction({
          programId: program.programId,
          keys: [
            {
              pubkey: anchor.web3.PublicKey.findProgramAddressSync(
                [Buffer.from("__event_authority")],
                program.programId
              )[0],
              isSigner: false,
              isWritable: false,
            },
            {
              pubkey: program.programId,
              isSigner: false,
              isWritable: false,
            },
          ],
          data: Buffer.from([0xe4, 0x45, 0xa5, 0x2e, 0x51, 0xcb, 0x9a, 0x1d]),
        })
      );

      try {
        await program.provider.sendAndConfirm(tx, [], confirmOptions);
      } catch (e) {
        // `sendAndConfirm` only attaches the logs when it can fetch the failed
        // transaction right after confirming it. When it can't, it rethrows
        // the confirmation error, whose message has the transaction status
        // instead, so check the error code there.
        if (e.logs) {
          if (e.logs.some((log) => log.includes("ConstraintSigner"))) return;
        } else {
          const code = anchor.LangErrorCode.ConstraintSigner;
          if (
            e.message?.includes(`"InstructionError":[0,{"Custom":${code}}]`)
          ) {
            return;
          }
        }
        console.log(e);
      }

      throw new Error("Was able to invoke the self-CPI instruction");
    });
  });

  // `events_caller` calls `events` through a CPI, so the event below is
  // logged at invoke depth 2. The caller also logs a valid `MyOtherEvent`
  // of its own before and after the CPI, which must not be attributed to
  // `events`. These tests parse the logs the runtime actually produced.
  describe("Inner instruction event", () => {
    const getLogs = async (txHash: string) => {
      const txResult = await program.provider.connection.getTransaction(
        txHash,
        { commitment: "confirmed", maxSupportedTransactionVersion: 0 }
      );
      return txResult.meta.logMessages;
    };

    const parseLogs = (logs: string[]) => {
      const parser = new anchor.EventParser(program.programId, program.coder);
      return [...parser.parseLogs(logs)];
    };

    it("Is delivered to event listeners", async () => {
      const received: { event: Event["myOtherEvent"]; signature: string }[] =
        [];
      const listenerId = program.addEventListener(
        "myOtherEvent",
        (event, _slot, signature) => received.push({ event, signature })
      );

      const txHash = await caller.methods.emitFromCpi().rpc(confirmOptions);

      // Wait for the event, then a little longer so that a wrongly
      // attributed caller event would also have arrived.
      const deadline = Date.now() + 10_000;
      while (
        !received.some((r) => r.signature === txHash) &&
        Date.now() < deadline
      ) {
        await new Promise((res) => setTimeout(res, 100));
      }
      await new Promise((res) => setTimeout(res, 1000));
      await program.removeEventListener(listenerId);

      const events = received
        .filter((r) => r.signature === txHash)
        .map((r) => r.event);
      assert.strictEqual(events.length, 1);
      assert.strictEqual(events[0].data.toNumber(), 6);
      assert.strictEqual(events[0].label, "bye");
    });

    it("Is parsed from the transaction logs", async () => {
      const txHash = await caller.methods.emitFromCpi().rpc(confirmOptions);
      const logs = await getLogs(txHash);

      // Make sure the logs have the shape this test is about: `events`
      // runs as an inner instruction, and the caller logs its own events
      // on both sides of it.
      assert.include(logs, `Program ${program.programId} invoke [2]`);
      assert.strictEqual(
        logs.filter((log) => log.startsWith("Program data: ")).length,
        3
      );

      const events = parseLogs(logs);
      assert.strictEqual(events.length, 1);
      assert.strictEqual(events[0].name, "myOtherEvent");
      assert.strictEqual(events[0].data.label, "bye");
      assert.strictEqual((events[0].data.data as anchor.BN).toNumber(), 6);
    });

    it("Is parsed in order with top-level events", async () => {
      const emitFromCpi = await caller.methods.emitFromCpi().instruction();
      const initialize = await program.methods.initialize().instruction();
      const tx = new anchor.web3.Transaction().add(
        emitFromCpi,
        initialize,
        emitFromCpi
      );
      const txHash = await program.provider.sendAndConfirm(
        tx,
        [],
        confirmOptions
      );
      const logs = await getLogs(txHash);

      const events = parseLogs(logs);
      assert.deepStrictEqual(
        events.map((e) => [e.name, e.data.label]),
        [
          ["myOtherEvent", "bye"],
          ["myEvent", "hello"],
          ["myOtherEvent", "bye"],
        ]
      );
    });
  });
});
