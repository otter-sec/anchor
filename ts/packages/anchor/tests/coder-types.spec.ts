import * as assert from "assert";
import { BorshCoder, Idl } from "../src";
import BN from "bn.js";

function recursiveCoderIdl(
  typeName: string,
  types: NonNullable<Idl["types"]>
): Idl {
  return {
    address: "Test111111111111111111111111111111111111111",
    metadata: {
      name: "recursive",
      version: "0.0.0",
      spec: "0.1.0",
    },
    instructions: [
      {
        name: "visit",
        accounts: [],
        args: [
          {
            name: "value",
            type: {
              defined: {
                name: typeName,
              },
            },
          },
        ],
        discriminator: [1, 2, 3, 4, 5, 6, 7, 8],
      },
    ],
    accounts: [
      {
        name: typeName,
        discriminator: [8, 7, 6, 5, 4, 3, 2, 1],
      },
    ],
    types,
  };
}

async function assertRecursiveCoderRoundTrip(
  idl: Idl,
  typeName: string,
  value: unknown,
  recursiveName = typeName
) {
  const coder = new BorshCoder(idl);

  const encodedType = coder.types.encode(typeName, value);
  assert.deepEqual(coder.types.decode(typeName, encodedType), value);

  const encodedIx = coder.instruction.encode("visit", { value });
  assert.deepEqual(coder.instruction.decode(encodedIx)?.data, { value });

  const encodedAccount = await coder.accounts.encode(typeName, value);
  assert.deepEqual(coder.accounts.decode(typeName, encodedAccount), value);
  assert.throws(
    () => coder.accounts.size(typeName),
    new RegExp(`Recursive types do not have a static size: ${recursiveName}`)
  );

  return coder;
}

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

    const testing = {
      unsigned: new BN(2588012355),
      signed: new BN(-93842345),
    };

    const coder = new BorshCoder(idl);
    const encoded = coder.types.encode("IntegerTest", testing);
    assert.strictEqual(
      coder.types.decode("IntegerTest", encoded).toString(),
      testing.toString()
    );
  });

  test("Can encode and decode self-recursive vec user-defined types", async () => {
    const idl = recursiveCoderIdl("RecursiveNode", [
      {
        name: "RecursiveNode",
        type: {
          kind: "enum",
          variants: [
            {
              name: "leaf",
            },
            {
              name: "branch",
              fields: [
                {
                  name: "children",
                  type: {
                    vec: {
                      defined: {
                        name: "RecursiveNode",
                      },
                    },
                  },
                },
              ],
            },
          ],
        },
      },
    ]);
    const recursiveNode = {
      branch: {
        children: [
          { leaf: {} },
          {
            branch: {
              children: [{ leaf: {} }],
            },
          },
        ],
      },
    };

    await assertRecursiveCoderRoundTrip(idl, "RecursiveNode", recursiveNode);
  });

  test("Can encode and decode mutually recursive user-defined types", async () => {
    const idl = recursiveCoderIdl("A", [
      {
        name: "A",
        type: {
          kind: "enum",
          variants: [
            {
              name: "leaf",
            },
            {
              name: "branch",
              fields: [
                {
                  name: "children",
                  type: {
                    vec: {
                      defined: {
                        name: "B",
                      },
                    },
                  },
                },
              ],
            },
          ],
        },
      },
      {
        name: "B",
        type: {
          kind: "enum",
          variants: [
            {
              name: "leaf",
            },
            {
              name: "branch",
              fields: [
                {
                  name: "children",
                  type: {
                    vec: {
                      defined: {
                        name: "A",
                      },
                    },
                  },
                },
              ],
            },
          ],
        },
      },
    ]);
    const a = {
      branch: {
        children: [
          { leaf: {} },
          {
            branch: {
              children: [{ leaf: {} }],
            },
          },
        ],
      },
    };

    const coder = await assertRecursiveCoderRoundTrip(idl, "A", a);
    const b = {
      branch: {
        children: [{ leaf: {} }],
      },
    };
    const encodedB = coder.types.encode("B", b);
    assert.deepEqual(coder.types.decode("B", encodedB), b);
  });

  test("Can encode and decode option-recursive user-defined types", async () => {
    const idl = recursiveCoderIdl("OptionalNode", [
      {
        name: "OptionalNode",
        type: {
          kind: "enum",
          variants: [
            {
              name: "leaf",
            },
            {
              name: "branch",
              fields: [
                {
                  name: "child",
                  type: {
                    option: {
                      defined: {
                        name: "OptionalNode",
                      },
                    },
                  },
                },
              ],
            },
          ],
        },
      },
    ]);
    const optionalNode = {
      branch: {
        child: {
          branch: {
            child: { leaf: {} },
          },
        },
      },
    };

    await assertRecursiveCoderRoundTrip(idl, "OptionalNode", optionalNode);
  });

  test("Can encode and decode recursive type aliases", async () => {
    const idl = recursiveCoderIdl("Tree", [
      {
        name: "Tree",
        type: {
          kind: "type",
          alias: {
            vec: {
              defined: {
                name: "Tree",
              },
            },
          },
        },
      },
    ]);
    const tree = [[], [[]]];

    await assertRecursiveCoderRoundTrip(idl, "Tree", tree);
  });

  test("Rejects recursive decodes beyond maximum layout depth", () => {
    const idl = recursiveCoderIdl("Tree", [
      {
        name: "Tree",
        type: {
          kind: "type",
          alias: {
            vec: {
              defined: {
                name: "Tree",
              },
            },
          },
        },
      },
    ]);
    const coder = new BorshCoder(idl);
    const depth = 300;
    const data = Buffer.alloc((depth + 1) * 4);
    for (let i = 0; i < depth; i += 1) {
      data.writeUInt32LE(1, i * 4);
    }
    data.writeUInt32LE(0, depth * 4);

    assert.throws(
      () => coder.types.decode("Tree", data),
      /Recursive IDL layout exceeded maximum depth/
    );
  });

  test("Rejects unguarded recursive user-defined types", () => {
    const idl = recursiveCoderIdl("BadNode", [
      {
        name: "BadNode",
        type: {
          kind: "struct",
          fields: [
            {
              name: "child",
              type: {
                defined: {
                  name: "BadNode",
                },
              },
            },
          ],
        },
      },
    ]);

    assert.throws(
      () => new BorshCoder(idl),
      /Recursive type must be wrapped in an option or vector: BadNode/
    );
  });
});
