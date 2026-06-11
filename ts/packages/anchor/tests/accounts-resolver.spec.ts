import { PublicKey } from "@solana/web3.js";

import { AccountsResolver } from "../src/program/accounts-resolver";
import { Idl } from "../src";

// When a PDA seed references an instruction argument whose name doesn't match
// the function's parameter name (and therefore isn't present in the IDL
// `instructions[].args` list), `AccountsResolver.resolve()` used to swallow
// the underlying `Unable to find argument for seed` error inside an empty
// `catch {}` block and then surface only a generic
// `Reached maximum depth for account resolution` message after 16 unsuccessful
// passes. That made the real root cause invisible to users.
describe("AccountsResolver", () => {
  it("surfaces the underlying error when a seed references an unknown arg", async () => {
    const idl: Idl = {
      address: "Test111111111111111111111111111111111111111",
      metadata: { name: "test", version: "0.0.0", spec: "0.1.0" },
      instructions: [
        {
          name: "doThing",
          discriminator: [0, 0, 0, 0, 0, 0, 0, 0],
          // The instruction declares an arg named `my_param`, but the PDA
          // seed below references `different_name` — simulating the
          // `#[instruction(different_name)]` / fn-signature mismatch
          args: [{ name: "my_param", type: "u64" }],
          accounts: [
            {
              name: "pda",
              pda: {
                seeds: [{ kind: "arg", path: "different_name" }],
              },
            },
          ],
        },
      ],
    };

    const resolver = new AccountsResolver(
      [1], // _args
      {}, // _accounts (none pre-set)
      {} as any, // _provider (unused on the failing path)
      new PublicKey("Test111111111111111111111111111111111111111"),
      idl.instructions[0] as any,
      {} as any, // accountNamespace (unused on the failing path)
      [] // idlTypes
    );

    let caught: unknown;
    try {
      await resolver.resolve();
    } catch (err) {
      caught = err;
    }

    expect(caught).toBeInstanceOf(Error);
    const err = caught as Error & { cause?: unknown };
    // The thrown error must still indicate that resolution gave up...
    expect(err.message).toMatch(/maximum depth/i);
    // ...but it must now also carry the root cause both in the message and
    // on `.cause` so the user can see what really went wrong.
    expect(err.message).toMatch(
      /Unable to find argument for seed: different_name/
    );
    expect(err.cause).toBeInstanceOf(Error);
    expect((err.cause as Error).message).toMatch(
      /Unable to find argument for seed: different_name/
    );
  });
});
