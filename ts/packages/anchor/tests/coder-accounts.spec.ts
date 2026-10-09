import * as assert from "assert";
import BN from "bn.js";
import { BorshCoder, Idl } from "../src";

describe("coder.accounts", () => {
  test("Can encode and decode user-defined accounts, including those with consecutive capital letters", () => {
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
          discriminator: [],
          accounts: [],
          args: [],
        },
      ],
      accounts: [
        {
          name: "MemberDAO",
          discriminator: [0, 1, 2, 3, 4, 5, 6, 7],
        },
      ],
      types: [
        {
          name: "MemberDAO",
          type: {
            kind: "struct",
            fields: [
              {
                name: "name",
                type: "string",
              },
            ],
          },
        },
      ],
    };
    const coder = new BorshCoder(idl);

    const memberDAO = {
      name: "test",
    };

    coder.accounts.encode("MemberDAO", memberDAO).then((encoded) => {
      assert.deepEqual(coder.accounts.decode("MemberDAO", encoded), memberDAO);
    });
  });

  test("Can encode and decode user-defined accounts, including those with more nested & multiple const generics", () => {
    const idl: Idl = {
      address: "EQoYLkj17hXm8yc9qLq5Cm7FgCqujRVHd2ZhdEfAmrMF",
      metadata: {
        name: "gen_idl",
        version: "0.1.0",
        spec: "0.1.0",
        description: "Created with Anchor",
      },
      instructions: [
        {
          name: "initialize",
          discriminator: [175, 175, 109, 31, 13, 152, 155, 237],
          accounts: [
            {
              name: "my_acc",
              pda: {
                seeds: [
                  {
                    kind: "const",
                    value: [
                      115, 109, 97, 108, 108, 95, 109, 101, 109, 112, 111, 111,
                      108,
                    ],
                  },
                ],
              },
            },
          ],
          args: [],
        },
      ],
      accounts: [
        {
          name: "MyAcc",
          discriminator: [123, 153, 151, 118, 126, 71, 73, 92],
        },
      ],
      types: [
        {
          name: "MaxHeap",
          serialization: "bytemuckunsafe",
          repr: {
            kind: "c",
          },
          generics: [
            {
              kind: "type",
              name: "T",
            },
            {
              kind: "const",
              name: "SIZE",
              type: "usize",
            },
            {
              kind: "const",
              name: "PADDING",
              type: "usize",
            },
          ],
          type: {
            kind: "struct",
            fields: [
              {
                name: "entries",
                type: {
                  array: [
                    {
                      generic: "T",
                    },
                    {
                      generic: "SIZE",
                    },
                  ],
                },
              },
              {
                name: "count",
                type: "u16",
              },
              {
                name: "padding",
                type: {
                  array: [
                    "u8",
                    {
                      generic: "PADDING",
                    },
                  ],
                },
              },
            ],
          },
        },
        {
          name: "MyStruct",
          serialization: "bytemuck",
          repr: {
            kind: "c",
          },
          type: {
            kind: "struct",
            fields: [
              {
                name: "a",
                type: "u16",
              },
              {
                name: "b",
                type: "u16",
              },
            ],
          },
        },
        {
          name: "MyAcc",
          serialization: "bytemuck",
          repr: {
            kind: "transparent",
          },
          type: {
            kind: "struct",
            fields: [
              {
                name: "inner",
                type: {
                  defined: {
                    name: "MaxHeap",
                    generics: [
                      {
                        kind: "type",
                        type: {
                          defined: {
                            name: "MyStruct",
                          },
                        },
                      },
                      {
                        kind: "const",
                        value: "3",
                      },
                      {
                        kind: "const",
                        value: "10",
                      },
                    ],
                  },
                },
              },
            ],
          },
        },
      ],
    };

    const coder = new BorshCoder(idl);

    const myAcc = {
      inner: {
        entries: [
          {
            a: 1,
            b: 2,
          },
          {
            a: 3,
            b: 4,
          },
          {
            a: 5,
            b: 6,
          },
        ],
        count: 2,
        padding: new Array(10).fill(0),
      },
    };

    coder.accounts.encode("MyAcc", myAcc).then((encoded) => {
      assert.deepEqual(coder.accounts.decode("MyAcc", encoded), myAcc);
    });
  });

  test("Can encode and decode generic types nested in a generic type", async () => {
    // Trimmed from tests/idl/programs/generics: a concrete argument
    // (`Nested<u32, U>`), a generic type as an argument
    // (`Nested<T, Nested<T, U>>`, also in a `Vec`) and a const generic
    // passed through by name (`GenericEnum<T, U, N>`).
    const idl: Idl = {
      address: "Generics111111111111111111111111111111111111",
      metadata: { name: "generics", version: "0.1.0", spec: "0.1.0" },
      instructions: [],
      accounts: [
        { name: "GenericAccount", discriminator: [1, 2, 3, 4, 5, 6, 7, 8] },
      ],
      types: [
        {
          name: "GenericAccount",
          type: {
            kind: "struct",
            fields: [
              {
                name: "data",
                type: {
                  defined: {
                    name: "GenericType",
                    generics: [
                      { kind: "type", type: "u16" },
                      { kind: "type", type: "u64" },
                      { kind: "const", value: "3" },
                    ],
                  },
                },
              },
            ],
          },
        },
        {
          name: "GenericEnum",
          generics: [
            { kind: "type", name: "T" },
            { kind: "type", name: "U" },
            { kind: "const", name: "N", type: "usize" },
          ],
          type: {
            kind: "enum",
            variants: [
              { name: "Unnamed", fields: [{ generic: "T" }, { generic: "U" }] },
              {
                name: "Arr",
                fields: [{ array: [{ generic: "T" }, { generic: "N" }] }],
              },
            ],
          },
        },
        {
          name: "Nested",
          generics: [
            { kind: "type", name: "V" },
            { kind: "type", name: "Z" },
          ],
          type: {
            kind: "struct",
            fields: [
              { name: "gen1", type: { generic: "V" } },
              { name: "gen2", type: { generic: "Z" } },
            ],
          },
        },
        {
          name: "GenericType",
          generics: [
            { kind: "type", name: "T" },
            { kind: "type", name: "U" },
            { kind: "const", name: "N", type: "usize" },
          ],
          type: {
            kind: "struct",
            fields: [
              { name: "gen1", type: { generic: "T" } },
              {
                name: "gen3",
                type: {
                  defined: {
                    name: "Nested",
                    generics: [
                      { kind: "type", type: "u32" },
                      { kind: "type", type: { generic: "U" } },
                    ],
                  },
                },
              },
              {
                name: "gen7",
                type: {
                  defined: {
                    name: "Nested",
                    generics: [
                      { kind: "type", type: { generic: "T" } },
                      {
                        kind: "type",
                        type: {
                          defined: {
                            name: "Nested",
                            generics: [
                              { kind: "type", type: { generic: "T" } },
                              { kind: "type", type: { generic: "U" } },
                            ],
                          },
                        },
                      },
                    ],
                  },
                },
              },
              {
                name: "list",
                type: {
                  vec: {
                    defined: {
                      name: "Nested",
                      generics: [
                        { kind: "type", type: { generic: "T" } },
                        {
                          kind: "type",
                          type: {
                            defined: {
                              name: "Nested",
                              generics: [
                                { kind: "type", type: { generic: "T" } },
                                { kind: "type", type: { generic: "U" } },
                              ],
                            },
                          },
                        },
                      ],
                    },
                  },
                },
              },
              {
                name: "enm",
                type: {
                  defined: {
                    name: "GenericEnum",
                    generics: [
                      { kind: "type", type: { generic: "T" } },
                      { kind: "type", type: { generic: "U" } },
                      { kind: "const", value: "N" },
                    ],
                  },
                },
              },
            ],
          },
        },
      ],
    };

    const coder = new BorshCoder(idl);
    const account = {
      data: {
        gen1: 1,
        gen3: { gen1: 2, gen2: new BN(3) },
        gen7: { gen1: 4, gen2: { gen1: 5, gen2: new BN(6) } },
        list: [{ gen1: 10, gen2: { gen1: 11, gen2: new BN(12) } }],
        enm: { Arr: { "0": [7, 8, 9] } },
      },
    };
    const encoded = await coder.accounts.encode("GenericAccount", account);

    // Borsh for GenericAccount { data: GenericType<u16, u64, 3> }.
    const expected = Buffer.from([
      ...[1, 2, 3, 4, 5, 6, 7, 8], // discriminator
      ...[1, 0], // gen1: u16
      ...[2, 0, 0, 0], // gen3.gen1: u32
      ...[3, 0, 0, 0, 0, 0, 0, 0], // gen3.gen2: u64
      ...[4, 0], // gen7.gen1: u16
      ...[5, 0], // gen7.gen2.gen1: u16
      ...[6, 0, 0, 0, 0, 0, 0, 0], // gen7.gen2.gen2: u64
      ...[1, 0, 0, 0, 10, 0, 11, 0], // list: one element, two u16s
      ...[12, 0, 0, 0, 0, 0, 0, 0], // list[0].gen2.gen2: u64
      ...[1, 7, 0, 8, 0, 9, 0], // enm: Arr([u16; 3])
    ]);
    assert.deepEqual(encoded, expected);
    // `BN` pads its internal words differently once decoded; compare values.
    assert.strictEqual(
      JSON.stringify(coder.accounts.decode("GenericAccount", encoded)),
      JSON.stringify(account)
    );
    // 8 + 2 + 12 + 12, 1 for the vec, and the largest variant, Unnamed
    // (1 + 2 + 8). `size` counts a variable-length type as 1.
    assert.strictEqual(coder.accounts.size("GenericAccount"), 46);
  });
});
