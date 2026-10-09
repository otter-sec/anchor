import type * as anchor from "../src";
import type { PublicKey } from "@solana/web3.js";

type TestIdl = {
  address: "Test111111111111111111111111111111111111111";
  metadata: { name: "test"; version: "0.0.0"; spec: "0.1.0" };
  instructions: [
    {
      name: "myMethod";
      discriminator: [];
      accounts: [
        { name: "payer"; signer: true; writable: true },
        {
          name: "nested";
          accounts: [
            { name: "authority" },
            { name: "optionalAccount"; optional: true }
          ];
        }
      ];
      args: [];
    },
    {
      name: "otherMethod";
      discriminator: [];
      accounts: [{ name: "otherAccount" }];
      args: [];
    },
    { name: "noAccounts"; discriminator: []; accounts: []; args: [] }
  ];
};

type Equal<A, B> = (<T>() => T extends A ? 1 : 2) extends <T>() => T extends B
  ? 1
  : 2
  ? true
  : false;

// Unknown methods must be rejected rather than producing an empty account map.
// @ts-expect-error This method does not exist in the IDL.
type InvalidMethod = anchor.MethodPubkeys<TestIdl, "missingMethod">;

describe("MethodPubkeys", () => {
  test("matches rpcAndKeys through the public package export", () => {
    type RpcPubkeys = Awaited<
      ReturnType<
        ReturnType<anchor.Program<TestIdl>["methods"]["myMethod"]>["rpcAndKeys"]
      >
    >["pubkeys"];
    const matches: Equal<
      anchor.MethodPubkeys<TestIdl, "myMethod">,
      RpcPubkeys
    > = true;
    expect(matches).toBe(true);
  });

  test("preserves nested accounts and PublicKey leaf types", () => {
    const matches: Equal<
      anchor.MethodPubkeys<TestIdl, "myMethod">,
      {
        payer: PublicKey;
        nested: { authority: PublicKey; optionalAccount: PublicKey };
      }
    > = true;
    expect(matches).toBe(true);
  });

  test("selects only the named method's accounts", () => {
    const matches: Equal<
      anchor.MethodPubkeys<TestIdl, "otherMethod">,
      { otherAccount: PublicKey }
    > = true;
    expect(matches).toBe(true);
  });

  test("supports methods with no accounts", () => {
    const matches: Equal<
      anchor.MethodPubkeys<TestIdl, "noAccounts">,
      {}
    > = true;
    expect(matches).toBe(true);
  });
});
