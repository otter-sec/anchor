import { PublicKey } from "@solana/web3.js";
import { AccountsResolver } from "../src/program/accounts-resolver";

describe("accounts resolver depth", () => {
  test("resolves successfully when all accounts resolved at depth boundary", async () => {
    // Regression test for otter-sec/anchor#4663
    // 17 accounts with reverse-ordered PDA chain: account16 has direct address,
    // account15 depends on account16, ..., account0 depends on account1.
    // This requires exactly 16 passes to resolve all accounts.
    // Previously threw false max-depth error even though all accounts resolved.
    const programId = new PublicKey("11111111111111111111111111111111");
    const resolvedAccounts: Record<string, PublicKey> = {};
    const accounts = Array.from({ length: 17 }, (_, index) => {
      const name = `account${index}`;
      if (index === 16) {
        return { name, address: programId.toBase58() };
      }

      return {
        name,
        pda: {
          seeds: [{ kind: "account", path: `account${index + 1}` }],
        },
      };
    });

    const resolver = new AccountsResolver(
      [],
      resolvedAccounts,
      {} as any,
      programId,
      { name: "depthRegression", discriminator: [], accounts, args: [] } as any,
      {} as any,
      []
    );

    // Should NOT throw — all accounts resolve successfully
    await expect(resolver.resolve()).resolves.toBeUndefined();

    // Verify all 17 accounts were resolved
    for (let index = 0; index < 17; index++) {
      expect(resolvedAccounts[`account${index}`]).toBeInstanceOf(PublicKey);
    }
  });

  test("throws when accounts remain unresolved at depth boundary", async () => {
    // 18 accounts with circular dependency: cannot resolve all within 16 passes.
    // account17 has no address and depends on account0 (circular).
    const programId = new PublicKey("11111111111111111111111111111111");
    const resolvedAccounts: Record<string, PublicKey> = {};
    const accounts = Array.from({ length: 18 }, (_, index) => {
      const name = `account${index}`;
      if (index === 17) {
        // No address, depends on account0 — creates circular dependency
        return {
          name,
          pda: {
            seeds: [{ kind: "account", path: "account0" }],
          },
        };
      }
      if (index === 16) {
        return { name, address: programId.toBase58() };
      }

      return {
        name,
        pda: {
          seeds: [{ kind: "account", path: `account${index + 1}` }],
        },
      };
    });

    const resolver = new AccountsResolver(
      [],
      resolvedAccounts,
      {} as any,
      programId,
      { name: "depthCircular", discriminator: [], accounts, args: [] } as any,
      {} as any,
      []
    );

    let thrown: Error | undefined;
    try {
      await resolver.resolve();
    } catch (error) {
      thrown = error as Error;
    }

    expect(thrown).toBeDefined();
    expect(thrown?.message).toContain("Reached maximum depth");
  });
});