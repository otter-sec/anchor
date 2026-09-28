import { PublicKey } from "@solana/web3.js";
import {
  Address,
  address,
  getAddressEncoder,
  getBase64Decoder,
  getProgramDerivedAddress,
  getU64Encoder,
  getU8Encoder,
  getUtf8Encoder,
  ReadonlyUint8Array,
} from "@solana/kit";
import { getTokenEncoder, AccountState } from "@solana-program/token";
import { BorshCoder, CustomAccountResolver, Idl, Program } from "../src";
import {
  mockProvider,
  randomAddress,
  SYSTEM_PROGRAM,
} from "./helpers/mock-provider";

const PROGRAM_ADDRESS = address("Test111111111111111111111111111111111111111");
const OTHER_PROGRAM = address("TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA");
const CONFIG_DISCRIMINATOR = [155, 12, 170, 224, 30, 250, 204, 130];

const idl = {
  address: PROGRAM_ADDRESS,
  metadata: { name: "resolver", version: "0.1.0", spec: "0.1.0" },
  instructions: [
    {
      name: "open",
      discriminator: [1, 2, 3, 4, 5, 6, 7, 8],
      accounts: [
        { name: "authority", signer: true },
        {
          name: "vault",
          pda: {
            seeds: [
              { kind: "const", value: [118, 97, 117, 108, 116] }, // "vault"
              { kind: "account", path: "authority" },
              { kind: "arg", path: "id" },
              { kind: "arg", path: "params.tag" },
            ],
          },
        },
        {
          name: "foreignVault",
          pda: {
            seeds: [{ kind: "account", path: "authority" }],
            program: { kind: "account", path: "vaultProgram" },
          },
        },
        { name: "vaultProgram" },
        { name: "systemProgram", address: SYSTEM_PROGRAM },
        { name: "eventAuthority" },
        { name: "program" },
      ],
      args: [
        { name: "id", type: "u64" },
        { name: "params", type: { defined: { name: "params" } } },
      ],
    },
    {
      name: "relate",
      discriminator: [8, 7, 6, 5, 4, 3, 2, 1],
      accounts: [
        { name: "config" },
        { name: "admin", relations: ["config"] },
        {
          name: "byMint",
          pda: {
            seeds: [
              {
                kind: "account",
                path: "tokenAccount.mint",
                account: "tokenAccount",
              },
            ],
          },
        },
        { name: "tokenAccount" },
        { name: "maybe", optional: true },
      ],
      args: [],
    },
  ],
  accounts: [{ name: "config", discriminator: CONFIG_DISCRIMINATOR }],
  types: [
    {
      name: "params",
      type: { kind: "struct", fields: [{ name: "tag", type: "u8" }] },
    },
    {
      name: "config",
      type: { kind: "struct", fields: [{ name: "admin", type: "pubkey" }] },
    },
  ],
} as const satisfies Idl;
type ResolverIdl = typeof idl;

function encodedAccount(
  data: ReadonlyUint8Array,
  owner: Address = PROGRAM_ADDRESS
) {
  return {
    context: { slot: 1 },
    value: {
      data: [getBase64Decoder().decode(data), "base64"],
      executable: false,
      lamports: 1,
      owner,
      rentEpoch: 0,
      space: data.length,
    },
  };
}

describe("AccountsResolver", () => {
  it("derives PDAs from const, account and argument seeds", async () => {
    const { provider, wallet } = mockProvider({});
    const program = new Program<ResolverIdl>(idl, provider);
    const vaultProgram = randomAddress();

    const keys = await program.methods
      .open(7n, { tag: 3 })
      // `accountsPartial`: the types do not know event CPI accounts resolve.
      .accountsPartial({ vaultProgram })
      .pubkeys();

    const [vault] = await getProgramDerivedAddress({
      programAddress: PROGRAM_ADDRESS,
      seeds: [
        getUtf8Encoder().encode("vault"),
        getAddressEncoder().encode(wallet.address),
        getU64Encoder().encode(7n),
        getU8Encoder().encode(3),
      ],
    });
    const [foreignVault] = await getProgramDerivedAddress({
      programAddress: vaultProgram,
      seeds: [getAddressEncoder().encode(wallet.address)],
    });
    const [eventAuthority] = await getProgramDerivedAddress({
      programAddress: PROGRAM_ADDRESS,
      seeds: ["__event_authority"],
    });

    // Every resolved value is a Kit address: the wallet fills the signer,
    // fixed addresses come from the IDL, and event CPI accounts are derived.
    expect(keys).toEqual({
      authority: wallet.address,
      vault,
      foreignVault,
      vaultProgram,
      systemProgram: SYSTEM_PROGRAM,
      eventAuthority,
      program: PROGRAM_ADDRESS,
    });
    for (const value of Object.values(keys)) {
      expect(typeof value).toBe("string");
    }
  });

  it("accepts legacy public keys as inputs", async () => {
    const { provider } = mockProvider({});
    const program = new Program<ResolverIdl>(idl, provider);
    const vaultProgram = new PublicKey(randomAddress());

    const keys = await program.methods
      .open(1n, { tag: 0 })
      // `accountsPartial`: the types do not know event CPI accounts resolve.
      .accountsPartial({ vaultProgram })
      .pubkeys();

    expect(keys.vaultProgram).toBe(vaultProgram.toBase58());
  });

  it("resolves relations and token account seeds from fetched accounts", async () => {
    const admin = randomAddress();
    const mint = randomAddress();
    const config = randomAddress();
    const tokenAccount = randomAddress();
    const coder = new BorshCoder(idl);
    const configData = await coder.accounts.encode("config", { admin });
    const tokenData = getTokenEncoder().encode({
      mint,
      owner: randomAddress(),
      amount: 5n,
      delegate: null,
      state: AccountState.Initialized,
      isNative: null,
      delegatedAmount: 0n,
      closeAuthority: null,
    });
    const { provider, requests } = mockProvider({
      getAccountInfo: (request) =>
        request.params[0] === config
          ? encodedAccount(new Uint8Array(configData))
          : encodedAccount(tokenData, OTHER_PROGRAM),
    });
    const program = new Program<ResolverIdl>(idl, provider);

    const keys = await program.methods
      .relate()
      .accounts({ config, tokenAccount, maybe: null })
      .pubkeys();

    const [byMint] = await getProgramDerivedAddress({
      programAddress: PROGRAM_ADDRESS,
      seeds: [getAddressEncoder().encode(mint)],
    });
    expect(keys).toEqual({
      config,
      admin,
      byMint,
      tokenAccount,
      // Optional accounts left out are passed as the program itself.
      maybe: PROGRAM_ADDRESS,
    });
    // Each account is fetched once, at the provider's commitment.
    const fetched = requests.filter((r) => r.method === "getAccountInfo");
    expect(fetched.map((r) => r.params[0]).sort()).toEqual(
      [config, tokenAccount].sort()
    );
    expect((fetched[0].params[1] as any).commitment).toBe("confirmed");
  });

  it("hands resolved addresses to custom resolvers", async () => {
    const { provider, wallet } = mockProvider({});
    const seen: unknown[] = [];
    const resolver: CustomAccountResolver<ResolverIdl> = async ({
      accounts,
      programAddress,
    }) => {
      seen.push({ accounts: { ...accounts }, programAddress });
      return { accounts, resolved: 0 };
    };
    const program = new Program<ResolverIdl>(
      idl,
      provider,
      undefined,
      () => resolver
    );

    await program.methods
      .open(1n, { tag: 0 })
      .accountsPartial({ vaultProgram: randomAddress() })
      .pubkeys();

    expect(seen[0]).toMatchObject({
      programAddress: PROGRAM_ADDRESS,
      accounts: { authority: wallet.address, program: PROGRAM_ADDRESS },
    });
  });

  it("normalises fresh custom resolver results into the shared accounts", async () => {
    const { provider, wallet } = mockProvider({});
    const vaultProgram = new PublicKey(randomAddress());
    // A resolver that ignores the object it was given, returns a new one,
    // and still speaks web3.js.
    const resolver: CustomAccountResolver<ResolverIdl> = async ({
      accounts,
    }) => ({
      accounts: { ...accounts, vaultProgram } as any,
      resolved: "vaultProgram" in accounts ? 0 : 1,
    });
    const program = new Program<ResolverIdl>(
      idl,
      provider,
      undefined,
      () => resolver
    );

    const keys = await program.methods.open(1n, { tag: 0 }).pubkeys();

    // The builder sees the resolver's result, as an address, and the PDA
    // seeded from it resolved.
    expect(keys.vaultProgram).toBe(vaultProgram.toBase58());
    const [foreignVault] = await getProgramDerivedAddress({
      programAddress: vaultProgram.toBase58() as Address,
      seeds: [getAddressEncoder().encode(wallet.address)],
    });
    expect(keys.foreignVault).toBe(foreignVault);
  });

  it("treats holes in custom resolver results as unresolved", async () => {
    const { provider } = mockProvider({});
    const vaultProgram = randomAddress();
    let calls = 0;
    // First pass: not figured out yet; later passes: resolved once.
    const resolver: CustomAccountResolver<ResolverIdl> = async ({
      accounts,
    }) => {
      calls++;
      if (calls === 1) {
        return {
          accounts: { ...accounts, vaultProgram: undefined } as any,
          resolved: 0,
        };
      }
      return {
        accounts: { ...accounts, vaultProgram },
        resolved: "vaultProgram" in accounts ? 0 : 1,
      };
    };
    const program = new Program<ResolverIdl>(
      idl,
      provider,
      undefined,
      () => resolver
    );

    const keys = await program.methods.open(1n, { tag: 0 }).pubkeys();

    expect(keys.vaultProgram).toBe(vaultProgram);
    expect(calls).toBeGreaterThanOrEqual(2);
  });

  it("encodes string byte seeds as UTF-8", async () => {
    const bytesIdl = {
      ...idl,
      instructions: [
        {
          name: "tag",
          discriminator: [1, 1, 1, 1, 1, 1, 1, 1],
          accounts: [
            {
              name: "tagged",
              pda: { seeds: [{ kind: "arg", path: "label" }] },
            },
          ],
          args: [{ name: "label", type: "bytes" }],
        },
      ],
    } as const satisfies Idl;
    const { provider } = mockProvider({});
    const program = new Program<typeof bytesIdl>(bytesIdl, provider);

    const { tagged } = await program.methods
      .tag("hello" as unknown as Uint8Array)
      .pubkeys();

    const [expected] = await getProgramDerivedAddress({
      programAddress: PROGRAM_ADDRESS,
      seeds: [getUtf8Encoder().encode("hello")],
    });
    expect(tagged).toBe(expected);
  });

  it("reports the accounts it could not resolve", async () => {
    const { provider } = mockProvider({});
    const program = new Program<ResolverIdl>(idl, provider);

    // `foreignVault` needs `vaultProgram`, which is never provided.
    await expect(
      program.methods.open(1n, { tag: 0 }).pubkeys()
    ).rejects.toThrow("Unresolved accounts: `foreignVault`");
  });
});
