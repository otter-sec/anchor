import * as assert from "assert";
import { BorshCoder } from "../src";
import { Idl, IdlType } from "../src/idl";
import { toInstruction } from "../src/program/common";

describe("coder.instructions", () => {
  test("Can encode and decode type aliased instruction arguments (byte array)", () => {
    const idl: Idl = {
      address: "Test111111111111111111111111111111111111111",
      metadata: {
        name: "test",
        version: "0.0.0",
        spec: "0.1.0",
      },
      instructions: [
        {
          name: "initialize",
          discriminator: [0, 1, 2, 3, 4, 5, 6, 7],
          accounts: [],
          args: [
            {
              name: "arg",
              type: {
                defined: {
                  name: "AliasTest",
                },
              },
            },
          ],
        },
      ],
      types: [
        {
          name: "AliasTest",
          type: {
            kind: "type",
            alias: {
              array: ["u8", 3] as [IdlType, number],
            },
          },
        },
      ],
    };

    const idlIx = idl.instructions[0];
    const expected = [1, 2, 3];

    const coder = new BorshCoder(idl);
    const ix = toInstruction(idlIx, expected);

    const encoded = coder.instruction.encode(idlIx.name, ix);
    const decoded = coder.instruction.decode(encoded);

    assert.deepStrictEqual(decoded?.data[idlIx.args[0].name], expected);
  });

  describe("encode arg validation", () => {
    const idl: Idl = {
      address: "Test111111111111111111111111111111111111111",
      metadata: { name: "test", version: "0.0.0", spec: "0.1.0" },
      instructions: [
        {
          name: "setPassThresholdBps",
          discriminator: [0, 1, 2, 3, 4, 5, 6, 7],
          accounts: [],
          args: [{ name: "passThresholdBps", type: "u16" }],
        },
        {
          name: "noArgs",
          discriminator: [8, 9, 10, 11, 12, 13, 14, 15],
          accounts: [],
          args: [],
        },
      ],
    };
    const coder = new BorshCoder(idl);

    test("throws when caller passes a primitive instead of an object", () => {
      assert.throws(
        () =>
          coder.instruction.encode(
            "setPassThresholdBps",
            1000 as unknown as object
          ),
        /expected an object with fields \{ passThresholdBps \}, got number/
      );
    });

    test("throws when caller passes null", () => {
      assert.throws(
        () =>
          coder.instruction.encode(
            "setPassThresholdBps",
            null as unknown as object
          ),
        /got null/
      );
    });

    test("throws when caller passes an array", () => {
      assert.throws(
        () =>
          coder.instruction.encode("setPassThresholdBps", [
            1000,
          ] as unknown as object),
        /got array/
      );
    });

    test("throws when a required field is missing", () => {
      assert.throws(
        () =>
          coder.instruction.encode("setPassThresholdBps", {
            // typo: missing trailing `s`
            passThresholdBp: 1000,
          } as any),
        /missing field `passThresholdBps`/
      );
    });

    test("encodes successfully when all fields are present", () => {
      const encoded = coder.instruction.encode("setPassThresholdBps", {
        passThresholdBps: 1000,
      });
      const decoded = coder.instruction.decode(encoded);
      assert.deepStrictEqual(decoded?.data, { passThresholdBps: 1000 });
    });

    test("instructions with no args skip validation", () => {
      assert.doesNotThrow(() =>
        coder.instruction.encode("noArgs", {} as object)
      );
    });
  });
});
