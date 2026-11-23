import { Connection } from "@solana/web3.js";
import { Idl, Program } from "../src";

// Jest 27 does not expose Node's structuredClone in its test environment.
if (typeof globalThis.structuredClone !== "function") {
  globalThis.structuredClone = (value) => JSON.parse(JSON.stringify(value));
}

describe("views", () => {
  const provider = { connection: new Connection("http://localhost:8899") };
  const idl: Idl = {
    address: "Test111111111111111111111111111111111111111",
    metadata: { name: "views", version: "0.0.0", spec: "0.1.0" },
    instructions: [
      {
        name: "nested_write",
        discriminator: [1, 2, 3, 4, 5, 6, 7, 8],
        accounts: [
          {
            name: "group",
            accounts: [{ name: "config" }, { name: "target", writable: true }],
          },
        ],
        args: [],
        returns: "u64",
      },
      {
        name: "flat_write",
        discriminator: [9, 9, 9, 9, 9, 9, 9, 9],
        accounts: [{ name: "target", writable: true }],
        args: [],
        returns: "u64",
      },
      {
        name: "nested_read",
        discriminator: [8, 8, 8, 8, 8, 8, 8, 8],
        accounts: [{ name: "group", accounts: [{ name: "target" }] }],
        args: [],
        returns: "u64",
      },
      {
        name: "flat_read",
        discriminator: [7, 7, 7, 7, 7, 7, 7, 7],
        accounts: [{ name: "target" }],
        args: [],
        returns: "u64",
      },
    ],
  };
  const program = new Program(idl, provider);

  it("exposes views only for instructions with no writable accounts", () => {
    expect(Object.keys(program.views ?? {}).sort()).toEqual([
      "flatRead",
      "nestedRead",
    ]);
  });

  it("rejects .view() when a writable account is nested in a group", async () => {
    await expect(program.methods.nestedWrite().view()).rejects.toThrow(
      "Method does not support views"
    );
  });
});
