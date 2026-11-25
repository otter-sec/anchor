import { PublicKey } from "@solana/web3.js";
import BN from "bn.js";

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
      /Errors encountered while resolving: `pda`: Unable to find argument for seed: different_name/
    );
    expect(err.cause).toBeInstanceOf(Error);
    expect((err.cause as Error).message).toMatch(
      /Unable to find argument for seed: different_name/
    );
  });

  it("only surfaces stored errors for unresolved accounts that failed", async () => {
    const idl: Idl = {
      address: "Test111111111111111111111111111111111111111",
      metadata: { name: "test", version: "0.0.0", spec: "0.1.0" },
      instructions: [
        {
          name: "doThing",
          discriminator: [0, 0, 0, 0, 0, 0, 0, 0],
          args: [{ name: "my_param", type: "u64" }],
          accounts: [
            {
              name: "pdaWaiting",
              pda: {
                seeds: [{ kind: "account", path: "missingSeed" }],
              },
            },
            {
              name: "badPda",
              pda: {
                seeds: [{ kind: "arg", path: "different_name" }],
              },
            },
          ],
        },
      ],
    };

    const resolver = new AccountsResolver(
      [1],
      {},
      {} as any,
      new PublicKey("Test111111111111111111111111111111111111111"),
      idl.instructions[0] as any,
      {} as any,
      []
    );

    let caught: unknown;
    try {
      await resolver.resolve();
    } catch (err) {
      caught = err;
    }

    expect(caught).toBeInstanceOf(Error);
    const err = caught as Error;
    expect(err.message).toMatch(/`badPda`: Unable to find argument for seed/);
    expect(err.message).not.toMatch(/`pdaWaiting`:/);
  });

  it("includes every account error in cause when multiple accounts fail", async () => {
    const idl: Idl = {
      address: "Test111111111111111111111111111111111111111",
      metadata: { name: "test", version: "0.0.0", spec: "0.1.0" },
      instructions: [
        {
          name: "doThing",
          discriminator: [0, 0, 0, 0, 0, 0, 0, 0],
          args: [{ name: "my_param", type: "u64" }],
          accounts: [
            {
              name: "badPda1",
              pda: {
                seeds: [{ kind: "arg", path: "missing_one" }],
              },
            },
            {
              name: "badPda2",
              pda: {
                seeds: [{ kind: "arg", path: "missing_two" }],
              },
            },
            {
              name: "badPda3",
              pda: {
                seeds: [{ kind: "arg", path: "missing_three" }],
              },
            },
          ],
        },
      ],
    };

    const resolver = new AccountsResolver(
      [1],
      {},
      {} as any,
      new PublicKey("Test111111111111111111111111111111111111111"),
      idl.instructions[0] as any,
      {} as any,
      []
    );

    let caught: unknown;
    try {
      await resolver.resolve();
    } catch (err) {
      caught = err;
    }

    expect(caught).toBeInstanceOf(Error);
    const err = caught as Error & { cause?: unknown };
    expect(err.message).toMatch(
      /Errors encountered while resolving: `badPda1`: Unable to find argument for seed: missing_one; `badPda2`: Unable to find argument for seed: missing_two; `badPda3`: Unable to find argument for seed: missing_three/
    );
    expect(err.cause).toBeInstanceOf(Error);
    expect((err.cause as Error).message).toBe(
      "`badPda1`: Unable to find argument for seed: missing_one; `badPda2`: Unable to find argument for seed: missing_two; `badPda3`: Unable to find argument for seed: missing_three"
    );
  });

  it("does not surface errors for accounts that resolved successfully", async () => {
    const providedSeed = new PublicKey(
      "SysvarC1ock11111111111111111111111111111111"
    );
    const idl: Idl = {
      address: "Test111111111111111111111111111111111111111",
      metadata: { name: "test", version: "0.0.0", spec: "0.1.0" },
      instructions: [
        {
          name: "doThing",
          discriminator: [0, 0, 0, 0, 0, 0, 0, 0],
          args: [{ name: "my_param", type: "u64" }],
          accounts: [
            {
              name: "resolvedPda",
              pda: {
                seeds: [{ kind: "account", path: "providedSeed" }],
              },
            },
            {
              name: "badPda",
              pda: {
                seeds: [{ kind: "arg", path: "different_name" }],
              },
            },
          ],
        },
      ],
    };

    const resolver = new AccountsResolver(
      [1],
      { providedSeed },
      {} as any,
      new PublicKey("Test111111111111111111111111111111111111111"),
      idl.instructions[0] as any,
      {} as any,
      []
    );

    let caught: unknown;
    try {
      await resolver.resolve();
    } catch (err) {
      caught = err;
    }

    expect(caught).toBeInstanceOf(Error);
    const err = caught as Error;
    expect(err.message).toMatch(/Unresolved accounts: `badPda`/);
    expect(err.message).not.toMatch(/`resolvedPda`/);
    expect(err.message).toMatch(/`badPda`: Unable to find argument for seed/);
  });

  it("resolves token account delegatedAmount seed", async () => {
    const tokenAccountPubkey = new PublicKey(
      "TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA"
    );
    const programId = new PublicKey(
      "Test111111111111111111111111111111111111111"
    );

    // Mock token account buffer (165 bytes).
    // DelegatedAmount is a u64 at byte offset 121.
    const tokenAccountData = Buffer.alloc(165);
    const delegatedAmountVal = 42;
    new BN(delegatedAmountVal)
      .toArrayLike(Buffer, "le", 8)
      .copy(tokenAccountData, 121);

    const idl: Idl = {
      address: programId.toBase58(),
      metadata: { name: "test", version: "0.0.0", spec: "0.1.0" },
      instructions: [
        {
          name: "doThing",
          discriminator: [0, 0, 0, 0, 0, 0, 0, 0],
          args: [],
          accounts: [
            {
              name: "tokenAccount",
            },
            {
              name: "pda",
              pda: {
                seeds: [
                  {
                    kind: "account",
                    path: "tokenAccount.delegatedAmount",
                    account: "tokenAccount",
                  },
                ],
              },
            },
          ],
        },
      ],
    };

    const mockProvider = {
      connection: {
        getAccountInfo: jest.fn().mockResolvedValue({
          data: tokenAccountData,
        }),
      },
    };

    const accounts = { tokenAccount: tokenAccountPubkey };
    const resolver = new AccountsResolver(
      [],
      accounts,
      mockProvider as any,
      programId,
      idl.instructions[0] as any,
      {} as any,
      []
    );

    await resolver.resolve();

    const expectedSeedBuffer = new BN(delegatedAmountVal).toArrayLike(
      Buffer,
      "le",
      8
    );
    const [expectedPda] = PublicKey.findProgramAddressSync(
      [expectedSeedBuffer],
      programId
    );

    expect(accounts).toEqual({
      tokenAccount: tokenAccountPubkey,
      pda: expectedPda,
    });
  });
});

// Builds an instruction whose PDAs form a chain that is declared in reverse
// dependency order: `account0` depends on `account1`, ..., and the last
// account has a fixed address. Each pass of the resolver can only resolve one
// more account, so a chain of `n` PDAs needs exactly `n` passes.
function reverseChainIx(pdaCount: number, programId: PublicKey) {
  const accounts = Array.from({ length: pdaCount + 1 }, (_, i) =>
    i === pdaCount
      ? { name: `account${i}`, address: programId.toBase58() }
      : {
          name: `account${i}`,
          pda: { seeds: [{ kind: "account", path: `account${i + 1}` }] },
        }
  );
  return {
    name: "chain",
    discriminator: [0, 0, 0, 0, 0, 0, 0, 0],
    args: [],
    accounts,
  };
}

async function resolveError(resolver: AccountsResolver<Idl>) {
  try {
    await resolver.resolve();
  } catch (err) {
    return err as Error;
  }
}

describe("AccountsResolver max depth", () => {
  const programId = new PublicKey(
    "Test111111111111111111111111111111111111111"
  );

  // https://github.com/otter-sec/anchor/issues/4663
  it("resolves accounts that need exactly 16 passes", async () => {
    const accounts: Record<string, PublicKey> = {};
    const resolver = new AccountsResolver(
      [],
      accounts,
      {} as any,
      programId,
      reverseChainIx(16, programId) as any,
      {} as any,
      []
    );

    await resolver.resolve();

    for (let i = 0; i <= 16; i++) {
      expect(accounts[`account${i}`]).toBeInstanceOf(PublicKey);
    }
  });

  it("errors when accounts need more than 16 passes", async () => {
    const accounts: Record<string, PublicKey> = {};
    const resolver = new AccountsResolver(
      [],
      accounts,
      {} as any,
      programId,
      reverseChainIx(17, programId) as any,
      {} as any,
      []
    );

    const err = await resolveError(resolver);

    expect(err).toBeInstanceOf(Error);
    expect(err!.message).toBe(
      "Reached maximum depth for account resolution. " +
        "Unresolved accounts: `account0`"
    );
    expect(accounts.account0).toBeUndefined();
    expect(accounts.account1).toBeInstanceOf(PublicKey);
  });

  it("errors on PDAs that depend on each other", async () => {
    const resolver = new AccountsResolver(
      [],
      {},
      {} as any,
      programId,
      {
        name: "cycle",
        discriminator: [0, 0, 0, 0, 0, 0, 0, 0],
        args: [],
        accounts: [
          { name: "a", pda: { seeds: [{ kind: "account", path: "b" }] } },
          { name: "b", pda: { seeds: [{ kind: "account", path: "a" }] } },
        ],
      } as any,
      {} as any,
      []
    );

    const err = await resolveError(resolver);

    expect(err).toBeInstanceOf(Error);
    expect(err!.message).toBe(
      "Reached maximum depth for account resolution. " +
        "Unresolved accounts: `a`, `b`"
    );
  });

  // A custom resolver that keeps reporting progress must still be stopped,
  // even when every IDL account is already resolved.
  it("stops a custom resolver that always reports progress", async () => {
    let calls = 0;
    const customResolver = async ({ accounts }) => {
      // Fail instead of hanging the test if the depth limit is missing.
      if (++calls > 100) throw new Error("custom resolver was not stopped");
      return { accounts, resolved: 1 };
    };
    const resolver = new AccountsResolver(
      [],
      {},
      {} as any,
      programId,
      reverseChainIx(1, programId) as any,
      {} as any,
      [],
      customResolver
    );

    const err = await resolveError(resolver);

    expect(calls).toBe(16);
    expect(err).toBeInstanceOf(Error);
    expect(err!.message).toBe(
      "Reached maximum depth for account resolution. " +
        "The custom account resolver returned a non-zero `resolved` " +
        "count on every pass."
    );
  });
});
