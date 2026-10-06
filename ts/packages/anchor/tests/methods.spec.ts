import { PublicKey } from "@solana/web3.js";
import {
  AccountRole,
  address,
  decompileTransactionMessage,
  getU64Codec,
  Instruction,
} from "@solana/kit";
import { Idl, Program } from "../src";
import {
  confirmingResponders,
  decodeWireTransaction,
  expectFullySigned,
  latestBlockhashResponse,
  mockProvider,
  randomAddress,
  randomSigner,
  simulationResponse,
  SYSTEM_PROGRAM,
  transferInstruction,
} from "./helpers/mock-provider";

const PROGRAM_ADDRESS = address("Test111111111111111111111111111111111111111");
const DISCRIMINATOR = [175, 175, 109, 31, 13, 152, 155, 237];

const idl: Idl = {
  address: PROGRAM_ADDRESS,
  metadata: { name: "counter", version: "0.1.0", spec: "0.1.0" },
  instructions: [
    {
      name: "initialize",
      discriminator: DISCRIMINATOR,
      accounts: [
        { name: "counter", writable: true, signer: true },
        { name: "authority", writable: true, signer: true },
        { name: "system_program", address: SYSTEM_PROGRAM },
      ],
      args: [{ name: "start", type: "u64" }],
    },
  ],
  events: [{ name: "Initialized", discriminator: [1, 2, 3, 4, 5, 6, 7, 8] }],
  types: [
    {
      name: "Initialized",
      type: { kind: "struct", fields: [{ name: "start", type: "u64" }] },
    },
  ],
};

function expectedData(start: bigint): Uint8Array {
  return new Uint8Array([...DISCRIMINATOR, ...getU64Codec().encode(start)]);
}

describe("Program namespaces", () => {
  describe("methods", () => {
    it("builds a Kit instruction with account roles from the IDL", async () => {
      const { provider, wallet } = mockProvider({});
      const program = new Program(idl, provider);
      const counter = randomSigner();

      // Accounts accept Kit addresses and legacy public keys alike, and the
      // wallet fills in signer accounts that are left out.
      const instruction = await program.methods
        .initialize(42n)
        .accounts({ counter: new PublicKey(counter.address) })
        .instruction();

      expect(instruction).toEqual({
        programAddress: PROGRAM_ADDRESS,
        accounts: [
          { address: counter.address, role: AccountRole.WRITABLE_SIGNER },
          { address: wallet.address, role: AccountRole.WRITABLE_SIGNER },
          { address: SYSTEM_PROGRAM, role: AccountRole.READONLY },
        ],
        data: expectedData(42n),
      } satisfies Instruction);
    });

    it("appends remaining accounts after the IDL accounts", async () => {
      const { provider } = mockProvider({});
      const program = new Program(idl, provider);
      const extra = randomAddress();

      const instruction = await program.methods
        .initialize(1n)
        .accounts({ counter: randomAddress() })
        .remainingAccounts([{ address: extra, role: AccountRole.READONLY }])
        .instruction();

      expect(instruction.accounts).toHaveLength(4);
      expect(instruction.accounts?.[3]).toEqual({
        address: extra,
        role: AccountRole.READONLY,
      });
    });

    it("builds a transaction message with surrounding instructions and signers", async () => {
      const { provider } = mockProvider({});
      const program = new Program(idl, provider);
      const counter = randomSigner();
      const pre = transferInstruction(randomAddress());
      const post = transferInstruction(randomAddress());

      const message = await program.methods
        .initialize(1n)
        .accounts({ counter: counter.address })
        .signers([counter.signer])
        .preInstructions([pre])
        .postInstructions([post])
        .transactionMessage();

      expect(message.version).toBe(0);
      expect(message.instructions.map((ix) => ix.programAddress)).toEqual([
        SYSTEM_PROGRAM,
        PROGRAM_ADDRESS,
        SYSTEM_PROGRAM,
      ]);
      // The counter signer is attached to its account meta, so the message
      // can be signed with Kit's `signTransactionMessageWithSigners`.
      expect(message.instructions[1].accounts?.[0]).toMatchObject({
        address: counter.address,
        signer: counter.signer,
      });
    });

    it("sends the transaction paid for and co-signed by the wallet", async () => {
      const { provider, wallet, requests } = mockProvider(
        confirmingResponders()
      );
      const program = new Program(idl, provider);
      const counter = randomSigner();

      const signature = await program.methods
        .initialize(7n)
        .accounts({ counter: counter.address })
        .signers([counter.signer])
        .rpc();

      expect(typeof signature).toBe("string");
      const sent = requests.find((r) => r.method === "sendTransaction")!;
      const { transaction, message } = decodeWireTransaction(sent);
      // Fee payer first, then the other signer.
      expect(message.staticAccounts.slice(0, 2)).toEqual([
        wallet.address,
        counter.address,
      ]);
      const { instructions } = decompileTransactionMessage(message);
      expect(instructions).toHaveLength(1);
      expect(instructions[0].programAddress).toBe(PROGRAM_ADDRESS);
      expect(instructions[0].data).toEqual(expectedData(7n));
      expectFullySigned(transaction, 2);
    });

    it("simulates and parses events from the logs", async () => {
      const eventData = Buffer.concat([
        Buffer.from([1, 2, 3, 4, 5, 6, 7, 8]),
        Buffer.from(getU64Codec().encode(9n)),
      ]).toString("base64");
      const { provider } = mockProvider({
        getLatestBlockhash: latestBlockhashResponse,
        simulateTransaction: () =>
          simulationResponse({
            logs: [
              `Program ${PROGRAM_ADDRESS} invoke [1]`,
              `Program data: ${eventData}`,
              `Program ${PROGRAM_ADDRESS} success`,
            ],
          }),
      });
      const program = new Program(idl, provider);

      const { events, raw } = await program.methods
        .initialize(9n)
        .accounts({ counter: randomAddress() })
        .simulate();

      expect(raw).toHaveLength(3);
      // Event names are camel-cased along with the rest of the IDL.
      expect(events).toEqual([{ name: "initialized", data: { start: 9n } }]);
    });

    it("prepares the instruction alongside its signers and keys", async () => {
      const { provider, wallet } = mockProvider({});
      const program = new Program(idl, provider);
      const counter = randomSigner();

      const { instruction, signers, addresses } = await program.methods
        .initialize(1n)
        .accounts({ counter: counter.address })
        .signers([counter.signer])
        .prepare();

      expect(instruction.programAddress).toBe(PROGRAM_ADDRESS);
      expect(signers).toEqual([counter.signer]);
      expect(addresses.authority).toBe(wallet.address);
    });
  });

  describe("instruction namespace", () => {
    it("rejects accounts that are neither addresses nor public keys", () => {
      const { provider } = mockProvider({});
      const program = new Program(idl, provider);

      expect(() =>
        program.instruction.initialize(1n, {
          accounts: {
            // A raw string is not an address until validated with `address()`.
            counter: "not-an-address" as any,
            authority: randomAddress(),
            systemProgram: SYSTEM_PROGRAM,
          },
        })
      ).toThrow('Wrong input type for account "counter"');
    });
  });
});
