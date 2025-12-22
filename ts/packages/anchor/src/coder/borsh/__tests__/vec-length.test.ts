import { Buffer } from "buffer";
import BN from "bn.js";
import { Idl } from "../../../idl.js";
import { BorshAccountsCoder } from "../accounts.js";
import * as borsh from "@coral-xyz/borsh";

describe("Vec with custom length discriminators", () => {
  it("should encode/decode Vec with u8 length", async () => {
    const idl: Idl = {
      address: "11111111111111111111111111111111",
      metadata: {
        name: "test",
        version: "0.1.0",
        spec: "0.1.0",
      },
      instructions: [],
      types: [
        {
          name: "TestAccount",
          serialization: "borshu8",
          type: {
            kind: "struct",
            fields: [
              {
                name: "items",
                type: {
                  vec: "u64",
                },
              },
            ],
          },
        },
      ],
      accounts: [
        {
          name: "TestAccount",
          discriminator: [0, 0, 0, 0, 0, 0, 0, 0],
        },
      ],
    };

    const coder = new BorshAccountsCoder(idl);
    const account = {
      items: [new BN(1), new BN(2), new BN(3)],
    };

    const encoded = await coder.encode("TestAccount", account);
    const decoded = coder.decode("TestAccount", encoded);

    expect(decoded.items.map((x: BN) => x.toNumber())).toEqual([1, 2, 3]);
  });

  it("should encode/decode Vec with u16 length", async () => {
    const idl: Idl = {
      address: "11111111111111111111111111111111",
      metadata: {
        name: "test",
        version: "0.1.0",
        spec: "0.1.0",
      },
      instructions: [],
      types: [
        {
          name: "TestAccount",
          serialization: "borshu16",
          type: {
            kind: "struct",
            fields: [
              {
                name: "prices",
                type: {
                  vec: "u64",
                },
              },
            ],
          },
        },
      ],
      accounts: [
        {
          name: "TestAccount",
          discriminator: [0, 0, 0, 0, 0, 0, 0, 0],
        },
      ],
    };

    const coder = new BorshAccountsCoder(idl);
    const account = {
      prices: [new BN(100), new BN(200), new BN(300)],
    };

    const encoded = await coder.encode("TestAccount", account);
    const decoded = coder.decode("TestAccount", encoded);

    expect(decoded.prices.map((x: BN) => x.toNumber())).toEqual([100, 200, 300]);
  });

  it("should encode/decode Vec with u32 length (default)", async () => {
    const idl: Idl = {
      address: "11111111111111111111111111111111",
      metadata: {
        name: "test",
        version: "0.1.0",
        spec: "0.1.0",
      },
      instructions: [],
      types: [
        {
          name: "TestAccount",
          type: {
            kind: "struct",
            fields: [
              {
                name: "items",
                type: {
                  vec: "u64", // Simple format, defaults to u32
                },
              },
            ],
          },
        },
      ],
      accounts: [
        {
          name: "TestAccount",
          discriminator: [0, 0, 0, 0, 0, 0, 0, 0],
        },
      ],
    };

    const coder = new BorshAccountsCoder(idl);
    const account = {
      items: [new BN(1), new BN(2), new BN(3), new BN(4), new BN(5)],
    };

    const encoded = await coder.encode("TestAccount", account);
    const decoded = coder.decode("TestAccount", encoded);

    expect(decoded.items.map((x: BN) => x.toNumber())).toEqual([1, 2, 3, 4, 5]);
  });

  it("should handle Vec with nested types", async () => {
    const idl: Idl = {
      address: "11111111111111111111111111111111",
      metadata: {
        name: "test",
        version: "0.1.0",
        spec: "0.1.0",
      },
      instructions: [],
      types: [
        {
          name: "PriceFeed",
          type: {
            kind: "struct",
            fields: [
              {
                name: "price",
                type: "u64",
              },
            ],
          },
        },
        {
          name: "TestAccount",
          serialization: "borshu16",
          type: {
            kind: "struct",
            fields: [
              {
                name: "feeds",
                type: {
                  vec: {
                    defined: {
                      name: "PriceFeed",
                    },
                  },
                },
              },
            ],
          },
        },
      ],
      accounts: [
        {
          name: "TestAccount",
          discriminator: [0, 0, 0, 0, 0, 0, 0, 0],
        },
      ],
    };

    const coder = new BorshAccountsCoder(idl);
    const account = {
      feeds: [{ price: new BN(100) }, { price: new BN(200) }],
    };

    const encoded = await coder.encode("TestAccount", account);
    const decoded = coder.decode("TestAccount", encoded);

    expect(decoded.feeds.map((f: any) => ({ price: f.price.toNumber() }))).toEqual([
      { price: 100 },
      { price: 200 },
    ]);
  });

  it("should verify u8 length prefix is used", () => {
    const layout = borsh.vecWithLength(borsh.u64(), "u8");
    const data = [new BN(1), new BN(2), new BN(3)];

    const buffer = Buffer.alloc(1000);
    const encodedLen = layout.encode(data, buffer);
    const encoded = buffer.slice(0, encodedLen);

    // First byte should be the length (3)
    expect(encoded[0]).toBe(3);
    // Next 24 bytes should be the 3 u64 values (8 bytes each)
    expect(encoded.length).toBe(1 + 24); // 1 byte length + 24 bytes data
  });

  it("should verify u16 length prefix is used", () => {
    const layout = borsh.vecWithLength(borsh.u64(), "u16");
    const data = [new BN(1), new BN(2), new BN(3)];

    const buffer = Buffer.alloc(1000);
    const encodedLen = layout.encode(data, buffer);
    const encoded = buffer.slice(0, encodedLen);

    // First 2 bytes should be the length (3) in little-endian
    expect(encoded.readUInt16LE(0)).toBe(3);
    // Next 24 bytes should be the 3 u64 values
    expect(encoded.length).toBe(2 + 24); // 2 bytes length + 24 bytes data
  });

  it("should verify u32 length prefix is used", () => {
    const layout = borsh.vecWithLength(borsh.u64(), "u32");
    const data = [new BN(1), new BN(2), new BN(3)];

    const buffer = Buffer.alloc(1000);
    const encodedLen = layout.encode(data, buffer);
    const encoded = buffer.slice(0, encodedLen);

    // First 4 bytes should be the length (3) in little-endian
    expect(encoded.readUInt32LE(0)).toBe(3);
    // Next 24 bytes should be the 3 u64 values
    expect(encoded.length).toBe(4 + 24); // 4 bytes length + 24 bytes data
  });
});

