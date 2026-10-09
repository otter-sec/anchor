import { Connection, PublicKey, VersionedTransaction } from "@solana/web3.js";
import { AnchorProvider, Wallet } from "../src/provider";
import { isVersionedTransaction } from "../src/utils/common";

// A v1 message containing two accounts and one instruction. Version 1
// transactions place their signatures after the message.
const V1_TRANSACTION = new Uint8Array([
  0x81,
  2,
  1,
  1,
  0,
  0,
  0,
  0,
  ...new Array(32).fill(10),
  1,
  2,
  ...new Array(32).fill(11),
  ...new Array(32).fill(12),
  1,
  1,
  3,
  0,
  0,
  1,
  2,
  3,
  ...new Array(128).fill(0),
]);

describe("AnchorProvider transaction-v1 support", () => {
  it("processes a normal web3.js-deserialized version 1 transaction", async () => {
    const transaction = VersionedTransaction.deserialize(V1_TRANSACTION);
    const connection = {
      commitment: "processed",
      simulateTransaction: jest.fn().mockResolvedValue({
        context: { slot: 1 },
        value: { err: null, logs: [] },
      }),
    } as unknown as Connection;
    const wallet = { publicKey: PublicKey.default } as unknown as Wallet;
    const provider = new AnchorProvider(connection, wallet);

    expect(transaction.version).toBe(1);
    expect(transaction.signatures).toHaveLength(2);
    expect(transaction.message.compiledInstructions).toHaveLength(1);
    expect(isVersionedTransaction(transaction)).toBe(true);
    await expect(provider.simulate(transaction)).resolves.toMatchObject({
      err: null,
    });
    expect(connection.simulateTransaction).toHaveBeenCalledWith(transaction, {
      commitment: undefined,
    });
  });

  it("looks up failed version 1 transactions with version 1 support", async () => {
    const transaction = VersionedTransaction.deserialize(V1_TRANSACTION);
    jest.spyOn(transaction, "serialize").mockReturnValue(new Uint8Array());
    const signature = "mocked-signature";
    const connection = {
      commitment: "processed",
      sendRawTransaction: jest.fn().mockResolvedValue(signature),
      confirmTransaction: jest.fn().mockResolvedValue({
        value: { err: { InstructionError: [0, "Custom"] } },
      }),
      getTransaction: jest.fn().mockResolvedValue({
        meta: { logMessages: ["Program failed"] },
      }),
    } as unknown as Connection;
    const wallet = {
      publicKey: PublicKey.default,
      signTransaction: jest.fn().mockImplementation(async (tx) => tx),
    } as unknown as Wallet;
    const provider = new AnchorProvider(connection, wallet);

    await expect(provider.sendAndConfirm(transaction)).rejects.toThrow(
      `Raw transaction ${signature} failed`
    );
    expect(connection.getTransaction).toHaveBeenCalledWith(expect.any(String), {
      commitment: "confirmed",
      maxSupportedTransactionVersion: 1,
    });
  });
});
