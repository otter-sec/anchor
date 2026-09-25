import { Keypair } from "@solana/web3.js";
import {
  AccountRole,
  address,
  createAsyncIterableFromDataPublisher,
  getBase58Decoder,
  getBase64Decoder,
  getU64Codec,
  isSolanaError,
  SOLANA_ERROR__ACCOUNTS__FAILED_TO_DECODE_ACCOUNT,
} from "@solana/kit";
import { BorshCoder, Coder, Idl, Program, Provider } from "../src";
import {
  mockProvider,
  randomAddress,
  randomSigner,
  SYSTEM_PROGRAM,
} from "./helpers/mock-provider";

const PROGRAM_ADDRESS = address("Test111111111111111111111111111111111111111");
const DISCRIMINATOR = [255, 176, 4, 245, 188, 253, 124, 25];

const idl = {
  address: PROGRAM_ADDRESS,
  metadata: { name: "counter", version: "0.1.0", spec: "0.1.0" },
  instructions: [],
  accounts: [{ name: "counter", discriminator: DISCRIMINATOR }],
  types: [
    {
      name: "counter",
      type: { kind: "struct", fields: [{ name: "count", type: "u64" }] },
    },
  ],
} as const satisfies Idl;
type CounterIdl = typeof idl;

/** RPC account info for a `Counter` holding `count`, base64 encoded. */
function counterAccount(
  count: bigint,
  overrides: Record<string, unknown> = {}
) {
  const data = new Uint8Array([
    ...DISCRIMINATOR,
    ...getU64Codec().encode(count),
  ]);
  return {
    data: [getBase64Decoder().decode(data), "base64"],
    executable: false,
    lamports: 1_000_000,
    owner: PROGRAM_ADDRESS,
    rentEpoch: 0,
    space: data.length,
    ...overrides,
  };
}

function withContext<T>(value: T, slot = 42) {
  return { context: { slot }, value };
}

describe("AccountClient", () => {
  describe("fetch", () => {
    it("returns a Kit account with the decoded data", async () => {
      const { provider, requests } = mockProvider({
        getAccountInfo: () => withContext(counterAccount(7n)),
      });
      const program = new Program<CounterIdl>(idl, provider);
      const target = randomAddress();

      const account = await program.account.counter.fetch(target);

      expect(account).toMatchObject({
        address: target,
        data: { count: 7n },
        executable: false,
        lamports: 1_000_000n,
        programAddress: PROGRAM_ADDRESS,
        space: 16n,
      });
      const request = requests.find((r) => r.method === "getAccountInfo")!;
      expect(request.params[0]).toBe(target);
      expect((request.params[1] as any).encoding).toBe("base64");
    });

    it("accepts legacy public keys and forwards fetch options", async () => {
      const { provider, requests } = mockProvider({
        getAccountInfo: () => withContext(counterAccount(1n)),
      });
      const program = new Program<CounterIdl>(idl, provider);
      const keypair = Keypair.generate();

      await program.account.counter.fetch(keypair.publicKey, {
        commitment: "confirmed",
        minContextSlot: 10n,
      });

      const request = requests.find((r) => r.method === "getAccountInfo")!;
      expect(request.params[0]).toBe(keypair.publicKey.toBase58());
      expect(request.params[1]).toMatchObject({
        commitment: "confirmed",
        minContextSlot: 10n,
      });
    });

    it("throws when the account does not exist", async () => {
      const { provider } = mockProvider({
        getAccountInfo: () => withContext(null),
      });
      const program = new Program<CounterIdl>(idl, provider);

      await expect(
        program.account.counter.fetch(randomAddress())
      ).rejects.toThrow("Account does not exist");
    });

    it("returns a non-existent maybe account from fetchNullable", async () => {
      const { provider } = mockProvider({
        getAccountInfo: () => withContext(null),
      });
      const program = new Program<CounterIdl>(idl, provider);
      const target = randomAddress();

      const account = await program.account.counter.fetchNullable(target);

      expect(account).toEqual({ address: target, exists: false });
    });

    it("returns the slot alongside the account", async () => {
      const { provider } = mockProvider({
        getAccountInfo: () => withContext(counterAccount(3n), 99),
      });
      const program = new Program<CounterIdl>(idl, provider);

      const { account, context } =
        await program.account.counter.fetchAndContext(randomAddress());

      expect(account.data.count).toBe(3n);
      expect(context.slot).toBe(99n);
    });

    it("fails with a Kit error when the data is of another type", async () => {
      const { provider } = mockProvider({
        getAccountInfo: () =>
          withContext(
            counterAccount(1n, {
              data: [getBase64Decoder().decode(new Uint8Array(16)), "base64"],
            })
          ),
      });
      const program = new Program<CounterIdl>(idl, provider);

      const error = await program.account.counter
        .fetch(randomAddress())
        .catch((e) => e);
      expect(
        isSolanaError(error, SOLANA_ERROR__ACCOUNTS__FAILED_TO_DECODE_ACCOUNT)
      ).toBe(true);
    });
  });

  describe("commitment", () => {
    it("reads at the provider's default commitment", async () => {
      const { provider, requests } = mockProvider({
        getAccountInfo: () => withContext(counterAccount(1n)),
        getMultipleAccounts: () => withContext([counterAccount(1n)]),
        getProgramAccounts: () => [],
      });
      const program = new Program<CounterIdl>(idl, provider);

      await program.account.counter.fetch(randomAddress());
      await program.account.counter.fetchMultiple([randomAddress()]);
      await program.account.counter.all();

      // Kit's own client default, so an account written through the
      // provider can be read straight back.
      expect(requests.map((r) => (r.params[1] as any).commitment)).toEqual([
        "confirmed",
        "confirmed",
        "confirmed",
      ]);
    });

    it("follows a custom provider commitment and lets callers override it", async () => {
      const { provider, requests } = mockProvider(
        {
          getAccountInfo: () => withContext(counterAccount(1n)),
          getMultipleAccounts: () => withContext([counterAccount(1n)]),
          getProgramAccounts: () => [],
        },
        { opts: { commitment: "processed" } }
      );
      const program = new Program<CounterIdl>(idl, provider);

      await program.account.counter.fetch(randomAddress());
      await program.account.counter.fetchMultiple([randomAddress()], {
        commitment: "confirmed",
      });
      await program.account.counter.all(undefined, { minContextSlot: 5n });

      expect(
        requests.map((r) => {
          const { commitment, minContextSlot } = r.params[1] as any;
          return [commitment, minContextSlot];
        })
      ).toEqual([
        ["processed", undefined],
        ["confirmed", undefined],
        ["processed", 5n],
      ]);
    });
  });

  describe("commitment with partial or missing provider options", () => {
    it("completes partial provider options with the defaults", async () => {
      const { provider, requests } = mockProvider(
        { getAccountInfo: () => withContext(counterAccount(1n)) },
        { opts: { skipPreflight: true } }
      );
      const program = new Program<CounterIdl>(idl, provider);

      await program.account.counter.fetch(randomAddress());

      expect(provider.opts).toEqual({
        skipPreflight: true,
        commitment: "confirmed",
        preflightCommitment: "confirmed",
      });
      expect((requests[0].params[1] as any).commitment).toBe("confirmed");
    });

    it("falls back to the Kit default for providers without options", async () => {
      const { provider: inner, requests } = mockProvider({
        getAccountInfo: () => withContext(counterAccount(1n)),
      });
      // A custom provider implementing only the reading side.
      const provider: Provider = {
        rpc: inner.rpc,
        rpcSubscriptions: inner.rpcSubscriptions,
      };
      const program = new Program<CounterIdl>(idl, provider);

      await program.account.counter.fetch(randomAddress());

      // Kit only applies its client default when the key is absent; an
      // explicit `commitment: undefined` is stripped and reads at the
      // server default instead.
      expect((requests[0].params[1] as any).commitment).toBe("confirmed");
    });

    it("subscribes at the completed default commitment", async () => {
      const calls: unknown[][] = [];
      const { provider } = mockProvider(
        {},
        {
          opts: { skipPreflight: true },
          subscriptions: {
            accountNotifications: (...args: unknown[]) => {
              calls.push(args);
              return {
                subscribe: async () =>
                  (async function* () {
                    await new Promise(() => {});
                  })(),
              };
            },
          },
        }
      );
      const program = new Program<CounterIdl>(idl, provider);
      const controller = new AbortController();

      program.account.counter.subscribe(randomAddress(), {
        abortSignal: controller.signal,
      });
      await new Promise((resolve) => setTimeout(resolve, 0));
      controller.abort();

      expect(calls[0][1]).toEqual({
        commitment: "confirmed",
        encoding: "base64",
      });
    });
  });

  describe("fetchMultiple", () => {
    it("fetches in batches of 100 and keeps the address order", async () => {
      const addresses = Array.from({ length: 150 }, () => randomAddress());
      const { provider, requests } = mockProvider({
        getMultipleAccounts: (request) =>
          withContext(
            (request.params[0] as string[]).map((_, index) =>
              index % 2 === 0 ? counterAccount(BigInt(index)) : null
            )
          ),
      });
      const program = new Program<CounterIdl>(idl, provider);

      const accounts = await program.account.counter.fetchMultiple(addresses);

      const batches = requests.filter(
        (r) => r.method === "getMultipleAccounts"
      );
      expect(batches.map((r) => (r.params[0] as string[]).length)).toEqual([
        100, 50,
      ]);
      expect(accounts).toHaveLength(150);
      expect(accounts.map((a) => a.address)).toEqual(addresses);
      expect(accounts[0]).toMatchObject({ exists: true, data: { count: 0n } });
      expect(accounts[1]).toEqual({ address: addresses[1], exists: false });
      expect(accounts[100]).toMatchObject({
        exists: true,
        data: { count: 0n },
      });
    });
  });

  describe("all", () => {
    it("filters by discriminator and returns Kit accounts", async () => {
      const first = randomAddress();
      const second = randomAddress();
      const { provider, requests } = mockProvider({
        getProgramAccounts: () => [
          { pubkey: first, account: counterAccount(1n) },
          { pubkey: second, account: counterAccount(2n) },
        ],
      });
      const program = new Program<CounterIdl>(idl, provider);

      const accounts = await program.account.counter.all();

      expect(accounts.map((a) => [a.address, a.data.count])).toEqual([
        [first, 1n],
        [second, 2n],
      ]);
      const request = requests.find((r) => r.method === "getProgramAccounts")!;
      expect(request.params[0]).toBe(PROGRAM_ADDRESS);
      expect(request.params[1]).toMatchObject({
        encoding: "base64",
        filters: [
          {
            memcmp: {
              offset: 0n,
              bytes: getBase58Decoder().decode(new Uint8Array(DISCRIMINATOR)),
              encoding: "base58",
            },
          },
        ],
      });
    });

    it("appends raw bytes to the discriminator filter", async () => {
      const { provider, requests } = mockProvider({
        getProgramAccounts: () => [],
      });
      const program = new Program<CounterIdl>(idl, provider);

      await program.account.counter.all(getU64Codec().encode(5n));

      const request = requests.find((r) => r.method === "getProgramAccounts")!;
      const [filter] = (request.params[1] as any).filters;
      expect(filter.memcmp.bytes).toBe(
        getBase58Decoder().decode(
          new Uint8Array([...DISCRIMINATOR, ...getU64Codec().encode(5n)])
        )
      );
    });

    it("appends Kit filters after the discriminator filter", async () => {
      const { provider, requests } = mockProvider({
        getProgramAccounts: () => [],
      });
      const program = new Program<CounterIdl>(idl, provider);

      await program.account.counter.all([{ dataSize: 16n }], {
        commitment: "finalized",
      });

      const request = requests.find((r) => r.method === "getProgramAccounts")!;
      const config = request.params[1] as any;
      expect(config.filters).toHaveLength(2);
      expect(config.filters[1]).toEqual({ dataSize: 16n });
      // "finalized" is the server default, which Kit omits.
      expect(config.commitment).toBeUndefined();
    });

    it("translates size-only and combined coder filters", async () => {
      // Coders may describe an account by size alone (e.g. the system
      // coder's nonce account) or by size and prefix bytes.
      async function filtersFor(memcmp: () => unknown) {
        const { provider, requests } = mockProvider({
          getProgramAccounts: () => [],
        });
        const coder: Coder = new BorshCoder(idl);
        (coder.accounts as any).memcmp = memcmp;
        const program = new Program<CounterIdl>(idl, provider, coder);
        await program.account.counter.all();
        const request = requests.find(
          (r) => r.method === "getProgramAccounts"
        )!;
        return (request.params[1] as any).filters;
      }

      expect(await filtersFor(() => ({ dataSize: 80 }))).toEqual([
        { dataSize: 80n },
      ]);
      expect(
        await filtersFor(() => ({ offset: 0, bytes: "abc", dataSize: 80 }))
      ).toEqual([
        { memcmp: { offset: 0n, bytes: "abc", encoding: "base58" } },
        { dataSize: 80n },
      ]);
    });
  });

  describe("subscribe", () => {
    function accountNotifications(
      notifications: ReturnType<typeof withContext>[],
      calls: unknown[][] = []
    ) {
      return {
        accountNotifications: (...args: unknown[]) => {
          calls.push(args);
          return {
            subscribe: async ({ abortSignal }: { abortSignal: AbortSignal }) =>
              (async function* () {
                for (const notification of notifications) {
                  if (abortSignal.aborted) return;
                  yield notification;
                }
                await new Promise<void>((resolve) =>
                  abortSignal.addEventListener("abort", () => resolve())
                );
              })(),
          };
        },
      };
    }

    it("publishes decoded accounts on the change channel", async () => {
      const calls: unknown[][] = [];
      const { provider } = mockProvider(
        {},
        {
          subscriptions: accountNotifications(
            [withContext(counterAccount(1n)), withContext(counterAccount(2n))],
            calls
          ),
        }
      );
      const program = new Program<CounterIdl>(idl, provider);
      const target = randomAddress();
      const controller = new AbortController();

      const changes: bigint[] = [];
      const publisher = program.account.counter.subscribe(target, {
        commitment: "confirmed",
        abortSignal: controller.signal,
      });
      await new Promise<void>((resolve) => {
        publisher.on("change", (account) => {
          changes.push(account.data.count);
          if (changes.length === 2) resolve();
        });
      });
      controller.abort();

      expect(changes).toEqual([1n, 2n]);
      expect(calls).toEqual([
        [target, { commitment: "confirmed", encoding: "base64" }],
      ]);
    });

    it("can be consumed as an async iterable", async () => {
      const calls: unknown[][] = [];
      const { provider } = mockProvider(
        {},
        {
          subscriptions: accountNotifications(
            [withContext(counterAccount(9n))],
            calls
          ),
        }
      );
      const program = new Program<CounterIdl>(idl, provider);
      const controller = new AbortController();

      const iterable = createAsyncIterableFromDataPublisher<{
        data: { count: bigint };
      }>({
        abortSignal: controller.signal,
        dataChannelName: "change",
        dataPublisher: program.account.counter.subscribe(randomAddress(), {
          abortSignal: controller.signal,
        }),
        errorChannelName: "error",
      });

      for await (const account of iterable) {
        expect(account.data.count).toBe(9n);
        controller.abort();
      }
      // Subscriptions default to the provider commitment too.
      expect(calls[0][1]).toEqual({
        commitment: "confirmed",
        encoding: "base64",
      });
    });

    it("publishes decoding failures on the error channel", async () => {
      const { provider } = mockProvider(
        {},
        {
          subscriptions: accountNotifications([
            withContext(
              counterAccount(1n, {
                data: [getBase64Decoder().decode(new Uint8Array(16)), "base64"],
              })
            ),
          ]),
        }
      );
      const program = new Program<CounterIdl>(idl, provider);

      const error = await new Promise<unknown>((resolve) => {
        program.account.counter
          .subscribe(randomAddress(), {
            abortSignal: new AbortController().signal,
          })
          .on("error", (e) => resolve(e));
      });

      expect(
        isSolanaError(error, SOLANA_ERROR__ACCOUNTS__FAILED_TO_DECODE_ACCOUNT)
      ).toBe(true);
    });
  });

  describe("createInstruction", () => {
    it("builds a system create account instruction paid by the wallet", async () => {
      const { provider, wallet, requests } = mockProvider({
        getMinimumBalanceForRentExemption: () => 1_500_000,
      });
      const program = new Program<CounterIdl>(idl, provider);
      const newAccount = randomSigner();

      const instruction = await program.account.counter.createInstruction(
        newAccount.signer
      );

      const rent = requests.find(
        (r) => r.method === "getMinimumBalanceForRentExemption"
      )!;
      expect(rent.params[0]).toBe(16n);
      // Read at the provider commitment like every other read.
      expect(rent.params[1]).toEqual({ commitment: "confirmed" });
      expect(instruction.programAddress).toBe(SYSTEM_PROGRAM);
      expect(instruction.accounts).toMatchObject([
        {
          address: wallet.address,
          role: AccountRole.WRITABLE_SIGNER,
          signer: wallet,
        },
        {
          address: newAccount.address,
          role: AccountRole.WRITABLE_SIGNER,
          signer: newAccount.signer,
        },
      ]);
      // u32 discriminator 0, u64 lamports, u64 space, owner.
      expect(instruction.data).toHaveLength(4 + 8 + 8 + 32);
    });
  });
});

describe("Program.fetchIdl", () => {
  it("reads the IDL account through the Kit rpc", async () => {
    const { provider, requests } = mockProvider({
      getAccountInfo: () => withContext(null),
    });

    const result = await Program.fetchIdl(PROGRAM_ADDRESS, provider);

    expect(result).toBeNull();
    const request = requests.find((r) => r.method === "getAccountInfo")!;
    expect(typeof request.params[0]).toBe("string");
    expect(request.params[1]).toMatchObject({
      encoding: "base64",
      commitment: "confirmed",
    });
  });
});
