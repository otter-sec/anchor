import * as assert from "assert";
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
});
