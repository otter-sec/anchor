import * as borsh from "@anchor-lang/borsh";
import { BorshCoder, Idl } from "../src";
import { IdlType } from "../src/idl";

const metadata = {
  name: "malformed_lengths",
  version: "0.0.0",
  spec: "0.1.0",
};

function u32Le(value: number): Buffer {
  const buffer = Buffer.alloc(4);
  buffer.writeUInt32LE(value, 0);
  return buffer;
}

describe("coder malformed lengths", () => {
  test("Rejects malformed vec lengths in account data", () => {
    const discriminator = Buffer.from([0, 1, 2, 3, 4, 5, 6, 7]);
    const idl: Idl = {
      address: "Test111111111111111111111111111111111111111",
      metadata,
      instructions: [
        {
          name: "initialize",
          discriminator: [],
          accounts: [],
          args: [],
        },
      ],
      accounts: [
        {
          name: "VectorAccount",
          discriminator: Array.from(discriminator),
        },
      ],
      types: [
        {
          name: "VectorAccount",
          type: {
            kind: "struct",
            fields: [
              {
                name: "items",
                type: {
                  vec: "u8",
                },
              },
            ],
          },
        },
      ],
    };
    const coder = new BorshCoder(idl);
    const malformed = Buffer.concat([discriminator, u32Le(16)]);

    expect(() => coder.accounts.decode("VectorAccount", malformed)).toThrow(
      /remaining bytes/
    );
  });

  test("Rejects malformed string lengths in type decoding", () => {
    const idl: Idl = {
      address: "Test111111111111111111111111111111111111111",
      metadata,
      instructions: [
        {
          name: "initialize",
          discriminator: [],
          accounts: [],
          args: [],
        },
      ],
      types: [
        {
          name: "UnsafeString",
          type: {
            kind: "struct",
            fields: [
              {
                name: "value",
                type: "string",
              },
            ],
          },
        },
      ],
    };
    const coder = new BorshCoder(idl);

    expect(() => coder.types.decode("UnsafeString", u32Le(64))).toThrow(
      /remaining bytes/
    );
  });

  test("Rejects malformed nested dynamic lengths", () => {
    const idl: Idl = {
      address: "Test111111111111111111111111111111111111111",
      metadata,
      instructions: [
        {
          name: "initialize",
          discriminator: [],
          accounts: [],
          args: [],
        },
      ],
      types: [
        {
          name: "NestedStrings",
          type: {
            kind: "struct",
            fields: [
              {
                name: "items",
                type: {
                  vec: "string",
                },
              },
            ],
          },
        },
      ],
    };
    const coder = new BorshCoder(idl);
    const malformed = Buffer.concat([u32Le(1), u32Le(64)]);

    expect(() => coder.types.decode("NestedStrings", malformed)).toThrow(
      /remaining bytes/
    );
  });

  test("Rejects malformed bytes in instruction decoding", () => {
    const discriminator = Buffer.from([8, 7, 6, 5, 4, 3, 2, 1]);
    const idl: Idl = {
      address: "Test111111111111111111111111111111111111111",
      metadata,
      instructions: [
        {
          name: "setData",
          discriminator: Array.from(discriminator),
          accounts: [],
          args: [
            {
              name: "data",
              type: "bytes",
            },
          ],
        },
      ],
    };
    const coder = new BorshCoder(idl);
    const malformed = Buffer.concat([discriminator, u32Le(64)]);

    expect(() => coder.instruction.decode(malformed)).toThrow(
      /remaining bytes/
    );
  });

  test("Rejects malformed string lengths in event decoding", () => {
    const discriminator = Buffer.from([9, 8, 7, 6, 5, 4, 3, 2]);
    const idl: Idl = {
      address: "Test111111111111111111111111111111111111111",
      metadata,
      instructions: [
        {
          name: "initialize",
          discriminator: [],
          accounts: [],
          args: [],
        },
      ],
      events: [
        {
          name: "UnsafeEvent",
          discriminator: Array.from(discriminator),
        },
      ],
      types: [
        {
          name: "UnsafeEvent",
          type: {
            kind: "struct",
            fields: [
              {
                name: "message",
                type: "string",
              },
            ],
          },
        },
      ],
    };
    const coder = new BorshCoder(idl);
    const malformed = Buffer.concat([discriminator, u32Le(64)]).toString(
      "base64"
    );

    expect(() => coder.events.decode(malformed)).toThrow(/remaining bytes/);
  });

  test("Rejects malformed fixed arrays coming from IDL", () => {
    const idl: Idl = {
      address: "Test111111111111111111111111111111111111111",
      metadata,
      instructions: [
        {
          name: "initialize",
          discriminator: [],
          accounts: [],
          args: [],
        },
      ],
      types: [
        {
          name: "UnsafeArray",
          type: {
            kind: "type",
            alias: {
              array: ["u8", 16] as [IdlType, number],
            },
          },
        },
      ],
    };
    const coder = new BorshCoder(idl);

    expect(() => coder.types.decode("UnsafeArray", Buffer.alloc(0))).toThrow(
      /remaining bytes/
    );
  });

  test("Rejects malformed map lengths in borsh layouts", () => {
    const layout = borsh.map(borsh.str("key"), borsh.u8("value"), "entries");

    expect(() => layout.decode(u32Le(2))).toThrow(/remaining bytes/);
  });
});
