import { Connection, PublicKey, VersionedTransaction } from "@solana/web3.js";
import { AnchorProvider, Wallet } from "../src/provider";
import { isVersionedTransaction } from "../src/utils/common";

describe("AnchorProvider", () => {
  it("processes deserialized version 1 transactions", async () => {
    // A v1 message containing two accounts and one instruction. Version 1
    // transactions place their signatures after the message.
    const serializedTransaction = new Uint8Array([
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
    const transaction = VersionedTransaction.deserialize(serializedTransaction);

    const connection = {
      commitment: "processed",
      getLatestBlockhash: jest.fn().mockResolvedValue({
        blockhash: PublicKey.default.toBase58(),
      }),
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
});
