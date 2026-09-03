import * as assert from "assert";
import { PublicKey } from "@solana/web3.js";
import { BorshCoder, Idl } from "../src";

describe("coder.types", () => {
  test("Can encode and decode user-defined types", () => {
    const idl: Idl = {
      address: "Test111111111111111111111111111111111111111",
      metadata: {
        name: "basic_0",
        version: "0.0.0",
        spec: "0.1.0",
      },
      instructions: [
        {
          name: "initialize",
          accounts: [],
          args: [],
          discriminator: [],
        },
      ],
      types: [
        {
          name: "MintInfo",
          type: {
            kind: "struct",
            fields: [
              {
                name: "minted",
                type: "bool",
              },
              {
                name: "metadataUrl",
                type: "string",
              },
            ],
          },
        },
      ],
    };
    const coder = new BorshCoder(idl);

    const mintInfo = {
      minted: true,
      metadataUrl: "hello",
    };
    const encoded = coder.types.encode("MintInfo", mintInfo);

    assert.deepEqual(coder.types.decode("MintInfo", encoded), mintInfo);
  });

  test("Can encode and decode 256-bit integers", () => {
    const idl: Idl = {
      address: "Test111111111111111111111111111111111111111",
      metadata: {
        name: "basic_0",
        version: "0.0.0",
        spec: "0.1.0",
      },
      instructions: [
        {
          name: "initialize",
          accounts: [],
          args: [],
          discriminator: [],
        },
      ],
      types: [
        {
          name: "IntegerTest",
          type: {
            kind: "struct",
            fields: [
              {
                name: "unsigned",
                type: "u256",
              },
              {
                name: "signed",
                type: "i256",
              },
            ],
          },
        },
      ],
    };

    const coder = new BorshCoder(idl);

    // Encoding accepts numbers as well as bigints.
    const fromBigInt = coder.types.encode("IntegerTest", {
      unsigned: 2588012355n,
      signed: -93842345n,
    });
    const fromNumber = coder.types.encode("IntegerTest", {
      unsigned: 2588012355,
      signed: -93842345,
    });
    assert.deepStrictEqual([...fromBigInt], [...fromNumber]);

    // Decoding always returns bigints.
    assert.deepStrictEqual(coder.types.decode("IntegerTest", fromBigInt), {
      unsigned: 2588012355n,
      signed: -93842345n,
    });

    // Out of range values throw.
    assert.throws(() =>
      coder.types.encode("IntegerTest", { unsigned: -1n, signed: 0n })
    );
    assert.throws(() =>
      coder.types.encode("IntegerTest", { unsigned: 1n << 256n, signed: 0n })
    );
  });

  test("Can encode and decode 64 and 128-bit integers", () => {
    const idl: Idl = {
      address: "Test111111111111111111111111111111111111111",
      metadata: {
        name: "basic_0",
        version: "0.0.0",
        spec: "0.1.0",
      },
      instructions: [],
      types: [
        {
          name: "IntegerTest",
          type: {
            kind: "struct",
            fields: [
              { name: "u64", type: "u64" },
              { name: "i64", type: "i64" },
              { name: "u128", type: "u128" },
              { name: "i128", type: "i128" },
            ],
          },
        },
      ],
    };

    const coder = new BorshCoder(idl);
    const value = {
      u64: 18446744073709551615n, // u64::MAX
      i64: -9223372036854775808n, // i64::MIN
      u128: 340282366920938463463374607431768211455n, // u128::MAX
      i128: -170141183460469231731687303715884105728n, // i128::MIN
    };

    const encoded = coder.types.encode("IntegerTest", value);
    assert.deepStrictEqual(coder.types.decode("IntegerTest", encoded), value);
  });

  test("Decodes pubkeys as base58 addresses and accepts PublicKey inputs", () => {
    const idl: Idl = {
      address: "Test111111111111111111111111111111111111111",
      metadata: {
        name: "basic_0",
        version: "0.0.0",
        spec: "0.1.0",
      },
      instructions: [],
      types: [
        {
          name: "KeyTest",
          type: {
            kind: "struct",
            fields: [{ name: "key", type: "pubkey" }],
          },
        },
      ],
    };

    const coder = new BorshCoder(idl);
    const key = new PublicKey("J2XMGdW2qQLx7rAdwWtSZpTXDgAQ988BLP9QTgUZvm54");

    const fromPublicKey = coder.types.encode("KeyTest", { key });
    const fromAddress = coder.types.encode("KeyTest", {
      key: key.toBase58(),
    });
    assert.deepStrictEqual([...fromPublicKey], [...key.toBytes()]);
    assert.deepStrictEqual([...fromPublicKey], [...fromAddress]);

    assert.deepStrictEqual(coder.types.decode("KeyTest", fromPublicKey), {
      key: key.toBase58(),
    });
  });

  test("Can encode and decode enums, preserving the single-key object shape", () => {
    const idl: Idl = {
      address: "Test111111111111111111111111111111111111111",
      metadata: {
        name: "basic_0",
        version: "0.0.0",
        spec: "0.1.0",
      },
      instructions: [],
      types: [
        {
          name: "Side",
          type: {
            kind: "enum",
            variants: [
              { name: "unit" },
              {
                name: "named",
                fields: [{ name: "size", type: "u64" }],
              },
              {
                name: "tuple",
                fields: ["string", "u8"],
              },
            ],
          },
        },
      ],
    };

    const coder = new BorshCoder(idl);

    const unit = coder.types.encode("Side", { unit: {} });
    assert.deepStrictEqual([...unit], [0]);
    assert.deepStrictEqual(coder.types.decode("Side", unit), { unit: {} });

    const named = coder.types.encode("Side", { named: { size: 5n } });
    assert.deepStrictEqual([...named], [1, 5, 0, 0, 0, 0, 0, 0, 0]);
    assert.deepStrictEqual(coder.types.decode("Side", named), {
      named: { size: 5n },
    });

    const tuple = coder.types.encode("Side", { tuple: { 0: "ab", 1: 7 } });
    assert.deepStrictEqual([...tuple], [2, 2, 0, 0, 0, 97, 98, 7]);
    assert.deepStrictEqual(coder.types.decode("Side", tuple), {
      tuple: { 0: "ab", 1: 7 },
    });

    assert.throws(() => coder.types.encode("Side", { unknown: {} }));
  });

  test("Can encode and decode coptions, reserving the payload slot for fixed-size None values", () => {
    const idl: Idl = {
      address: "Test111111111111111111111111111111111111111",
      metadata: {
        name: "basic_0",
        version: "0.0.0",
        spec: "0.1.0",
      },
      instructions: [],
      types: [
        {
          name: "COptionTest",
          type: {
            kind: "struct",
            fields: [
              { name: "maybe", type: { coption: "u64" } },
              { name: "after", type: "u8" },
            ],
          },
        },
      ],
    };

    const coder = new BorshCoder(idl);

    // None: u32 tag + zero-filled u64 slot, so `after` stays at a fixed offset.
    const none = coder.types.encode("COptionTest", { maybe: null, after: 7 });
    assert.deepStrictEqual([...none], [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 7]);
    assert.deepStrictEqual(coder.types.decode("COptionTest", none), {
      maybe: null,
      after: 7,
    });

    const some = coder.types.encode("COptionTest", { maybe: 5n, after: 7 });
    assert.deepStrictEqual([...some], [1, 0, 0, 0, 5, 0, 0, 0, 0, 0, 0, 0, 7]);
    assert.deepStrictEqual(coder.types.decode("COptionTest", some), {
      maybe: 5n,
      after: 7,
    });
  });

  test("Can encode and decode bytes and vectors", () => {
    const idl: Idl = {
      address: "Test111111111111111111111111111111111111111",
      metadata: {
        name: "basic_0",
        version: "0.0.0",
        spec: "0.1.0",
      },
      instructions: [],
      types: [
        {
          name: "BytesTest",
          type: {
            kind: "struct",
            fields: [
              { name: "data", type: "bytes" },
              { name: "list", type: { vec: "u16" } },
            ],
          },
        },
      ],
    };

    const coder = new BorshCoder(idl);
    // `bytes` accepts any Uint8Array, including Buffers.
    const encoded = coder.types.encode("BytesTest", {
      data: Buffer.from([1, 2, 3]),
      list: [500, 600],
    });
    assert.deepStrictEqual(
      [...encoded],
      [3, 0, 0, 0, 1, 2, 3, 2, 0, 0, 0, 244, 1, 88, 2]
    );

    const decoded = coder.types.decode("BytesTest", encoded);
    assert.ok(decoded.data instanceof Uint8Array);
    assert.deepStrictEqual([...decoded.data], [1, 2, 3]);
    assert.deepStrictEqual(decoded.list, [500, 600]);
  });
});
