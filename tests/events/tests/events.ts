import * as anchor from "@anchor-lang/core";
import { assert } from "chai";

import { Events } from "../target/types/events";
import { EventsCaller } from "../target/types/events_caller";

describe("Events", () => {
  // Configure the client to use the local cluster.
  anchor.setProvider(anchor.AnchorProvider.env());
  const program = anchor.workspace.Events as anchor.Program<Events>;
  const eventsCaller = anchor.workspace
    .EventsCaller as anchor.Program<EventsCaller>;
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
        if (e.logs.some((log) => log.includes("ConstraintSigner"))) return;
        console.log(e);
      }

      throw new Error("Was able to invoke the self-CPI instruction");
    });
  });

  // The `events-caller` program CPIs into the `events` program, so the
  // event is emitted from an inner instruction (invoke depth 2). These
  // tests verify the log parsing fix from #4451 against a real
  // deployment instead of synthetic log data (#4656, #4450).
  describe("Inner instruction event", () => {
    it("Is delivered to event listeners", async () => {
      let listenerId: number;
      const eventPromise = new Promise<Event["myOtherEvent"]>((res) => {
        listenerId = program.addEventListener("myOtherEvent", (event) => {
          res(event);
        });
      });
      // Give the log subscription time to become active before sending
      // the transaction, otherwise the only emission can be missed.
      await new Promise((res) => setTimeout(res, 500));
      await eventsCaller.methods
        .cpiEvent()
        .accounts({ eventsProgram: program.programId })
        .rpc(confirmOptions);
      const event = await eventPromise;
      await program.removeEventListener(listenerId);

      assert.strictEqual(event.data.toNumber(), 6);
      assert.strictEqual(event.label, "bye");
    });

    it("Is detected by the event parser in on-chain transaction logs", async () => {
      const txHash = await eventsCaller.methods
        .cpiEvent()
        .accounts({ eventsProgram: program.programId })
        .rpc(confirmOptions);
      const txResult = await program.provider.connection.getTransaction(
        txHash,
        {
          commitment: "confirmed",
          maxSupportedTransactionVersion: 0,
        }
      );

      const parser = new anchor.EventParser(program.programId, program.coder);
      const events = [...parser.parseLogs(txResult.meta.logMessages)];

      assert.strictEqual(events.length, 1);
      assert.strictEqual(events[0].name, "myOtherEvent");
      assert.strictEqual(events[0].data.label, "bye");
      assert.strictEqual((events[0].data.data as anchor.BN).toNumber(), 6);
    });
  });
});
