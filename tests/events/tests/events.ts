import * as anchor from "@anchor-lang/core";
import { assert } from "chai";

import { Events } from "../target/types/events";

describe("Events", () => {
  // Configure the client to use the local cluster.
  anchor.setProvider(anchor.AnchorProvider.env());
  const program = anchor.workspace.events as anchor.Program<Events>;
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
});
