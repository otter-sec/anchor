import { deserializeTransaction } from "../src/utils/v1";

// Wire-format Solana transaction-v1 fixture. It includes message-level resource
// limits and a System Program transfer, exercising the transaction shape
// returned through transaction RPC methods.
const V1_TRANSACTION_WIRE_BASE64 =
  "gQEAAB8AAACfhtCBiEx9ZZov6qDFWtAVo79PGysLgizRXWwVsPAKCAEChQ8tbgKkevgk0Jq1ezUthFXDQyqFB+gCFczb6vgtJWYAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAIgTAAAAAAAA4JMEAAAAAQAAgAAAAQIMAAABAgAAAEDiAQAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA==";

describe("transaction-v1 receive", () => {
  it("deserializes a wire transaction with its v1 config", () => {
    const transaction = deserializeTransaction(
      Buffer.from(V1_TRANSACTION_WIRE_BASE64, "base64")
    );

    expect(transaction.version).toBe(1);
    expect(transaction.message.transactionConfig).toEqual({
      priorityFeeLamports: BigInt(5_000),
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
