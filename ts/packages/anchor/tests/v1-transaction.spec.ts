import {
  MessageV1,
  PublicKey,
  SystemProgram,
  Transaction,
} from "@solana/web3.js";
import {
  deserializeTransaction,
  toTransaction,
  V1TransactionConfig,
} from "../src/utils/v1";

// Wire-format Solana transaction-v1 fixture. It includes message-level resource
// limits and a System Program transfer, exercising the transaction shape
// returned through transaction RPC methods.
const V1_TRANSACTION_WIRE_BASE64 =
  "gQEAAB8AAACfhtCBiEx9ZZov6qDFWtAVo79PGysLgizRXWwVsPAKCAEChQ8tbgKkevgk0Jq1ezUthFXDQyqFB+gCFczb6vgtJWYAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAIgTAAAAAAAA4JMEAAAAAQAAgAAAAQIMAAABAgAAAEDiAQAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA==";

describe("transaction-v1 compilation", () => {
  it.each<[string, V1TransactionConfig, number]>([
    ["omitted", {}, 200_000],
    ["undefined", { computeUnitLimit: undefined }, 200_000],
    ["explicit", { computeUnitLimit: 300_000 }, 300_000],
    ["zero", { computeUnitLimit: 0 }, 0],
  ])("serializes the %s compute unit limit", (_, overrides, expectedLimit) => {
    const transactionConfig = Object.freeze({
      priorityFeeLamports: BigInt(5_000),
      loadedAccountsDataSizeLimit: 65_536,
      heapSize: 32_768,
      ...overrides,
    });
    const tx = new Transaction().add(
      SystemProgram.transfer({
        fromPubkey: PublicKey.default,
        toPubkey: PublicKey.default,
        lamports: 1,
      })
    );
    const v1 = toTransaction(tx, {
      payerKey: PublicKey.default,
      recentBlockhash: "GfVcyD5tT6Aua4yzGiVy5gM2y2qXmZRr5GnMaCu2FQVc",
      transactionConfig,
    });
    const decoded = deserializeTransaction(v1.serialize());

    expect((decoded.message as MessageV1).transactionConfig).toEqual({
      priorityFee: 5_000,
      computeUnitLimit: expectedLimit,
      loadedAccountsDataSizeLimit: 65_536,
      heapSize: 32_768,
    });
    expect(transactionConfig.computeUnitLimit).toBe(overrides.computeUnitLimit);
  });
});

describe("transaction-v1 receive", () => {
  it("deserializes a wire transaction with its v1 config", () => {
    const transaction = deserializeTransaction(
      Buffer.from(V1_TRANSACTION_WIRE_BASE64, "base64")
    );

    expect(transaction.version).toBe(1);
    expect((transaction.message as MessageV1).transactionConfig).toEqual({
      priorityFee: 5_000,
      computeUnitLimit: 300_000,
      loadedAccountsDataSizeLimit: 65_536,
      heapSize: 32_768,
    });
    expect(
      transaction.message.staticAccountKeys.map((key) => key.toBase58())
    ).toEqual([
      "9xQeWvG816bUx9EPfEzD1hK8NqfTsE7QkpBfK1J2B5Gq",
      "11111111111111111111111111111111",
    ]);
  });
});
