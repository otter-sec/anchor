import * as assert from "assert";
import bs58 from "bs58";
import { Idl } from "../src";
import { SystemAccountsCoder } from "../src/coder/system/accounts.js";
import { NONCE_ACCOUNT_LENGTH } from "@solana/web3.js";

describe("coder.system.accounts", () => {
  const coder = new SystemAccountsCoder({} as Idl);

  test("includes appended bytes in nonce memcmp filters", () => {
    const filter = Buffer.from([1, 2, 3]);

    assert.deepEqual(coder.memcmp("nonce", filter), {
      dataSize: NONCE_ACCOUNT_LENGTH,
      offset: 0,
      bytes: bs58.encode(filter),
    });
  });

  test("only filters by nonce account size when no bytes are appended", () => {
    assert.deepEqual(coder.memcmp("nonce"), {
      dataSize: NONCE_ACCOUNT_LENGTH,
    });

    assert.deepEqual(coder.memcmp("nonce", Buffer.alloc(0)), {
      dataSize: NONCE_ACCOUNT_LENGTH,
    });
  });
});
