import * as assert from "assert";
import BN from "bn.js";
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

  test("encodes undefined option instruction arguments as null", () => {
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
              name: "required",
              type: "bool",
            },
            {
              name: "someArg",
              type: {
                option: "u64",
              },
            },
            {
              name: "someArg2",
              type: {
                option: "u64",
              },
            },
          ],
        },
      ],
      types: [],
    };

    const idlIx = idl.instructions[0];
    const coder = new BorshCoder(idl);
    const ix = toInstruction(idlIx, true, undefined, new BN(0x3030));

    const encoded = coder.instruction.encode(idlIx.name, ix);
    const decoded = coder.instruction.decode(encoded);

    assert.deepStrictEqual(
      [...encoded],
      [
        ...idlIx.discriminator,
        1, // required bool
        0, // someArg: None
        1, // someArg2: Some
        0x30,
        0x30,
        0,
        0,
        0,
        0,
        0,
        0,
      ]
    );
    assert.strictEqual(decoded?.data["someArg"], null);
    assert.ok(decoded?.data["someArg2"].eq(new BN(0x3030)));
  });

  test("allows missing option instruction arguments", () => {
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
              name: "required",
              type: "bool",
            },
            {
              name: "someArg",
              type: {
                option: "u64",
              },
            },
          ],
        },
      ],
      types: [],
    };

    const coder = new BorshCoder(idl);
    const encoded = coder.instruction.encode("initialize", { required: true });
    const decoded = coder.instruction.decode(encoded);

    assert.deepStrictEqual(
      [...encoded],
      [
        ...idl.instructions[0].discriminator,
        1, // required bool
        0, // someArg: None
      ]
    );
    assert.strictEqual(decoded?.data["someArg"], null);
  });

  test("encodes undefined aliased option instruction arguments as null", () => {
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
              name: "required",
              type: "bool",
            },
            {
              name: "someArg",
              type: {
                defined: {
                  name: "MaybeU64",
                },
              },
            },
            {
              name: "someArg2",
              type: {
                defined: {
                  name: "MaybeU64",
                },
              },
            },
          ],
        },
      ],
      types: [
        {
          name: "MaybeU64",
          type: {
            kind: "type",
            alias: {
              option: "u64",
            },
          },
        },
      ],
    };

    const idlIx = idl.instructions[0];
    const coder = new BorshCoder(idl);
    const ix = toInstruction(idlIx, true, undefined, new BN(0x3030));

    const encoded = coder.instruction.encode(idlIx.name, ix);
    const decoded = coder.instruction.decode(encoded);

    assert.deepStrictEqual(
      [...encoded],
      [
        ...idlIx.discriminator,
        1, // required bool
        0, // someArg: None
        1, // someArg2: Some
        0x30,
        0x30,
        0,
        0,
        0,
        0,
        0,
        0,
      ]
    );
    assert.strictEqual(decoded?.data["someArg"], null);
    assert.ok(decoded?.data["someArg2"].eq(new BN(0x3030)));
  });

  test("allows missing aliased option instruction arguments", () => {
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
              name: "required",
              type: "bool",
            },
            {
              name: "someArg",
              type: {
                defined: {
                  name: "MaybeU64",
                },
              },
            },
          ],
        },
      ],
      types: [
        {
          name: "MaybeU64",
          type: {
            kind: "type",
            alias: {
              option: "u64",
            },
          },
        },
      ],
    };

    const coder = new BorshCoder(idl);
    const encoded = coder.instruction.encode("initialize", { required: true });
    const decoded = coder.instruction.decode(encoded);

    assert.deepStrictEqual(
      [...encoded],
      [
        ...idl.instructions[0].discriminator,
        1, // required bool
        0, // someArg: None
      ]
    );
    assert.strictEqual(decoded?.data["someArg"], null);
  });

  test("encodes undefined generic aliased option instruction arguments as null", () => {
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
              name: "required",
              type: "bool",
            },
            {
              name: "someArg",
              type: {
                defined: {
                  name: "Identity",
                  generics: [{ kind: "type", type: { option: "u64" } }],
                },
              },
            },
            {
              name: "someArg2",
              type: {
                defined: {
                  name: "Identity",
                  generics: [{ kind: "type", type: { option: "u64" } }],
                },
              },
            },
          ],
        },
      ],
      types: [
        {
          name: "Identity",
          generics: [{ kind: "type", name: "T" }],
          type: {
            kind: "type",
            alias: {
              generic: "T",
            },
          },
        },
      ],
    };

    const idlIx = idl.instructions[0];
    const coder = new BorshCoder(idl);
    const ix = toInstruction(idlIx, true, undefined, new BN(0x3030));

    const encoded = coder.instruction.encode(idlIx.name, ix);
    const decoded = coder.instruction.decode(encoded);

    assert.deepStrictEqual(
      [...encoded],
      [
        ...idlIx.discriminator,
        1, // required bool
        0, // someArg: None
        1, // someArg2: Some
        0x30,
        0x30,
        0,
        0,
        0,
        0,
        0,
        0,
      ]
    );
    assert.strictEqual(decoded?.data["someArg"], null);
    assert.ok(decoded?.data["someArg2"].eq(new BN(0x3030)));
  });

  test("allows missing generic aliased option instruction arguments", () => {
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
              name: "required",
              type: "bool",
            },
            {
              name: "someArg",
              type: {
                defined: {
                  name: "Identity",
                  generics: [{ kind: "type", type: { option: "u64" } }],
                },
              },
            },
          ],
        },
      ],
      types: [
        {
          name: "Identity",
          generics: [{ kind: "type", name: "T" }],
          type: {
            kind: "type",
            alias: {
              generic: "T",
            },
          },
        },
      ],
    };

    const coder = new BorshCoder(idl);
    const encoded = coder.instruction.encode("initialize", { required: true });
    const decoded = coder.instruction.decode(encoded);

    assert.deepStrictEqual(
      [...encoded],
      [
        ...idl.instructions[0].discriminator,
        1, // required bool
        0, // someArg: None
      ]
    );
    assert.strictEqual(decoded?.data["someArg"], null);
  });

  test("encodes undefined options inside generic defined structs as null", () => {
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
              name: "wrapper",
              type: {
                defined: {
                  name: "Wrapper",
                  generics: [{ kind: "type", type: { option: "u64" } }],
                },
              },
            },
          ],
        },
      ],
      types: [
        {
          name: "Wrapper",
          generics: [{ kind: "type", name: "T" }],
          type: {
            kind: "struct",
            fields: [
              { name: "someArg", type: { generic: "T" } },
              { name: "someArg2", type: { generic: "T" } },
            ],
          },
        },
      ],
    };

    const idlIx = idl.instructions[0];
    const coder = new BorshCoder(idl);

    const encoded = coder.instruction.encode(idlIx.name, {
      wrapper: { someArg: undefined, someArg2: new BN(0x3030) },
    });
    const decoded = coder.instruction.decode(encoded);

    assert.deepStrictEqual(
      [...encoded],
      [
        ...idlIx.discriminator,
        0, // wrapper.someArg: None
        1, // wrapper.someArg2: Some
        0x30,
        0x30,
        0,
        0,
        0,
        0,
        0,
        0,
      ]
    );
    assert.strictEqual(decoded?.data["wrapper"].someArg, null);
    assert.ok(decoded?.data["wrapper"].someArg2.eq(new BN(0x3030)));
  });

  test("encodes undefined options inside generic defined enums as null", () => {
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
              name: "wrapper",
              type: {
                defined: {
                  name: "WrapperEnum",
                  generics: [{ kind: "type", type: { option: "u64" } }],
                },
              },
            },
          ],
        },
      ],
      types: [
        {
          name: "WrapperEnum",
          generics: [{ kind: "type", name: "T" }],
          type: {
            kind: "enum",
            variants: [
              {
                name: "Fields",
                fields: [
                  { name: "someArg", type: { generic: "T" } },
                  { name: "someArg2", type: { generic: "T" } },
                ],
              },
            ],
          },
        },
      ],
    };

    const idlIx = idl.instructions[0];
    const coder = new BorshCoder(idl);

    const encoded = coder.instruction.encode(idlIx.name, {
      wrapper: { Fields: { someArg: undefined, someArg2: new BN(0x3030) } },
    });
    const decoded = coder.instruction.decode(encoded);

    assert.deepStrictEqual(
      [...encoded],
      [
        ...idlIx.discriminator,
        0, // WrapperEnum::Fields discriminator
        0, // wrapper.Fields.someArg: None
        1, // wrapper.Fields.someArg2: Some
        0x30,
        0x30,
        0,
        0,
        0,
        0,
        0,
        0,
      ]
    );
    assert.strictEqual(decoded?.data["wrapper"].Fields.someArg, null);
    assert.ok(decoded?.data["wrapper"].Fields.someArg2.eq(new BN(0x3030)));
  });
});
