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

  describe.each(["sendAndConfirm", "sendAll"] as const)("%s", (method) => {
    it.each([
      "returned error",
      "rejected object",
      "rejected string",
      "transport error",
    ])("handles a %s during confirmation", async (outcome) => {
      const transaction = VersionedTransaction.deserialize(V1_TRANSACTION);
      jest.spyOn(transaction, "serialize").mockReturnValue(new Uint8Array());
      const signature = "mocked-signature";
      const transactionError =
        outcome === "rejected string"
          ? "AccountNotFound"
          : { InstructionError: [0, { Custom: 2002 }] };
      const transportError = new Error("RPC unavailable");
      const confirmTransaction = jest.fn();
      if (outcome === "returned error") {
        confirmTransaction.mockResolvedValue({
          value: { err: transactionError },
        });
      } else {
        confirmTransaction.mockRejectedValue(
          outcome === "transport error" ? transportError : transactionError
        );
      }
      const logs = ["Program failed"];
      const connection = {
        commitment: "processed",
        sendRawTransaction: jest.fn().mockResolvedValue(signature),
        confirmTransaction,
        getTransaction: jest.fn().mockResolvedValue({
          meta: { logMessages: logs },
        }),
      } as unknown as Connection;
      const wallet = {
        publicKey: PublicKey.default,
        signTransaction: jest.fn().mockImplementation(async (tx) => tx),
        signAllTransactions: jest.fn().mockImplementation(async (txs) => txs),
      } as unknown as Wallet;
      const provider = new AnchorProvider(connection, wallet);
      const sent =
        method === "sendAndConfirm"
          ? provider.sendAndConfirm(transaction)
          : provider.sendAll([{ tx: transaction }]);

      if (outcome === "transport error") {
        await expect(sent).rejects.toBe(transportError);
        expect(connection.getTransaction).not.toHaveBeenCalled();
      } else {
        await expect(sent).rejects.toMatchObject({
          message: expect.stringContaining(
            `Raw transaction ${signature} failed`
          ),
          logs,
        });
        expect(connection.getTransaction).toHaveBeenCalledWith(
          expect.any(String),
          {
            commitment: "confirmed",
            maxSupportedTransactionVersion: 1,
          }
        );
      }
    });
  });
});
