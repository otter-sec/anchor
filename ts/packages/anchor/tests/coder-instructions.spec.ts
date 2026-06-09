import * as assert from "assert";
import { BorshCoder, some } from "../src";
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

  test("Can encode and decode option instruction arguments", () => {
    const idl: Idl = {
      address: "Test111111111111111111111111111111111111111",
      metadata: {
        name: "test",
        version: "0.0.0",
        spec: "0.1.0",
      },
      instructions: [
        {
          name: "optionArg",
          discriminator: [8, 7, 6, 5, 4, 3, 2, 1],
          accounts: [],
          args: [
            {
              name: "arg",
              type: {
                option: "u8",
              },
            },
          ],
        },
      ],
    };

    const coder = new BorshCoder(idl);
    const idlIx = idl.instructions[0];
    const discriminator = idlIx.discriminator;

    const noneValue = coder.instruction.encode(
      idlIx.name,
      toInstruction(idlIx, null)
    );
    const someValue = coder.instruction.encode(
      idlIx.name,
      toInstruction(idlIx, 1)
    );

    assert.deepStrictEqual([...noneValue], [...discriminator, 0]);
    assert.deepStrictEqual([...someValue], [...discriminator, 1, 1]);

    const decodedNone = coder.instruction.decode(noneValue);
    const decodedSome = coder.instruction.decode(someValue);

    assert.deepStrictEqual(decodedNone?.data["arg"], null);
    assert.deepStrictEqual(decodedSome?.data["arg"], 1);
    assert.deepStrictEqual(
      coder.instruction.format(decodedNone!, [])?.args[0].data,
      "null"
    );
    assert.deepStrictEqual(
      coder.instruction.format(decodedSome!, [])?.args[0].data,
      "1"
    );
  });

  test("Can encode nested option instruction arguments", () => {
    const idl: Idl = {
      address: "Test111111111111111111111111111111111111111",
      metadata: {
        name: "test",
        version: "0.0.0",
        spec: "0.1.0",
      },
      instructions: [
        {
          name: "nestedOption",
          discriminator: [1, 2, 3, 4, 5, 6, 7, 8],
          accounts: [],
          args: [
            {
              name: "arg",
              type: {
                option: {
                  option: "u8",
                },
              },
            },
          ],
        },
      ],
    };

    const coder = new BorshCoder(idl);
    const idlIx = idl.instructions[0];
    const discriminator = idlIx.discriminator;

    const noneValue = coder.instruction.encode(
      idlIx.name,
      toInstruction(idlIx, null)
    );
    const someNoneValue = coder.instruction.encode(
      idlIx.name,
      toInstruction(idlIx, some(null))
    );
    const someSomeValue = coder.instruction.encode(
      idlIx.name,
      toInstruction(idlIx, some(1))
    );

    assert.deepStrictEqual([...noneValue], [...discriminator, 0]);
    assert.deepStrictEqual([...someNoneValue], [...discriminator, 1, 0]);
    assert.deepStrictEqual([...someSomeValue], [...discriminator, 1, 1, 1]);

    const decodedNone = coder.instruction.decode(noneValue);
    const decodedSomeNone = coder.instruction.decode(someNoneValue);
    const decodedSomeSome = coder.instruction.decode(someSomeValue);

    // Decoding remains backward-compatible and collapses nested options.
    assert.deepStrictEqual(decodedNone?.data["arg"], null);
    assert.deepStrictEqual(decodedSomeNone?.data["arg"], null);
    assert.deepStrictEqual(decodedSomeSome?.data["arg"], 1);
  });
});
