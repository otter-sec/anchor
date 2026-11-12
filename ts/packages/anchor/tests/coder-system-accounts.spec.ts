import * as assert from "assert";
import bs58 from "bs58";
import { NONCE_ACCOUNT_LENGTH } from "@solana/web3.js";
import { SystemAccountsCoder } from "../src/coder/system/accounts";
import { Idl } from "../src";

describe("coder.system.accounts", () => {
  const coder = new SystemAccountsCoder({} as Idl);

  test("memcmp without data only filters by size", () => {
    assert.deepStrictEqual(coder.memcmp("nonce"), {
      dataSize: NONCE_ACCOUNT_LENGTH,
    });
  });

  test("memcmp with data also filters by bytes", () => {
    const data = Buffer.from([1, 2, 3, 4]);
    assert.deepStrictEqual(coder.memcmp("nonce", data), {
      dataSize: NONCE_ACCOUNT_LENGTH,
      offset: 0,
      bytes: bs58.encode(data),
    });
  });
});
