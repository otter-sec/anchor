import {
  Address,
  fetchEncodedAccount,
  getAddressDecoder,
  getAddressEncoder,
  getI16Encoder,
  getI32Encoder,
  getI64Encoder,
  getI128Encoder,
  getI256Encoder,
  getI8Encoder,
  getProgramDerivedAddress,
  getU16Encoder,
  getU32Encoder,
  getU64Encoder,
  getU128Encoder,
  getU256Encoder,
  getU8Encoder,
  getUtf8Encoder,
  ReadonlyUint8Array,
} from "@solana/kit";
import { getTokenDecoder } from "@solana-program/token";
import { Buffer } from "buffer";
import {
  Idl,
  IdlSeed,
  IdlInstructionAccountItem,
  IdlInstructionAccount,
  IdlTypeDef,
  IdlTypeDefTyStruct,
  IdlType,
  isCompositeAccounts,
  IdlSeedConst,
  IdlSeedArg,
  IdlSeedAccount,
  IdlTypeDefined,
  IdlDefinedFieldsNamed,
} from "../idl.js";
import { AllInstructions } from "./namespace/types.js";
import Provider from "../provider.js";
import { AccountsCoder, BorshAccountsCoder } from "../coder/index.js";
import { withProviderDefaults } from "../utils/common.js";
import { Address as AnchorAddress, Program, toAddress } from "./index.js";
import {
  PartialAccounts,
  flattenPartialAccounts,
  isPartialAccounts,
} from "./namespace/methods";

/**
 * Resolved instruction accounts: addresses keyed by account name, nested
 * for composite accounts.
 */
export type AccountsGeneric = {
  [name: string]: Address | AccountsGeneric;
};

export function isAccountsGeneric(
  accounts: Address | AccountsGeneric
): accounts is AccountsGeneric {
  return typeof accounts !== "string";
}

export type CustomAccountResolver<IDL extends Idl> = (params: {
  args: Array<any>;
  accounts: AccountsGeneric;
  provider: Provider;
  programAddress: Address;
  idlIx: AllInstructions<IDL>;
}) => Promise<{ accounts: AccountsGeneric; resolved: number }>;

// Populates a given accounts context with PDAs and common missing accounts.
export class AccountsResolver<IDL extends Idl> {
  private _accountStore: AccountStore;

  constructor(
    private _args: any[],
    private _accounts: AccountsGeneric,
    private _provider: Provider,
    private _programId: Address,
    private _idlIx: AllInstructions<IDL>,
    accountsCoder: AccountsCoder,
    private _idlTypes: IdlTypeDef[],
    private _customResolver?: CustomAccountResolver<IDL>
  ) {
    this._accountStore = new AccountStore(_provider, accountsCoder, _programId);
  }

  public args(args: Array<any>): void {
    this._args = args;
  }

  // Note: We serially resolve PDAs one by one rather than doing them
  //       in parallel because there can be dependencies between
  //       addresses. That is, one PDA can be used as a seed in another.
  public async resolve() {
    await this.resolveEventCpi(this._idlIx.accounts);
    this.resolveConst(this._idlIx.accounts);

    // Auto populate pdas and relations until we stop finding new accounts
    let depth = 0;
    while (
      (await this.resolvePdasAndRelations(this._idlIx.accounts)) +
        (await this.resolveCustom()) >
      0
    ) {
      depth++;
      if (depth === 16) {
        const isResolvable = (acc: IdlInstructionAccountItem) => {
          if (!isCompositeAccounts(acc)) {
            return !!(acc.address || acc.pda || acc.relations);
          }

          return acc.accounts.some(isResolvable);
        };

        const getPaths = (
          accs: IdlInstructionAccountItem[],
          path: string[] = [],
          paths: string[][] = []
        ) => {
          for (const acc of accs) {
            if (isCompositeAccounts(acc)) {
              paths.push(...getPaths(acc.accounts, [...path, acc.name]));
            } else {
              paths.push([...path, acc.name]);
            }
          }

          return paths;
        };

        const resolvableAccs = this._idlIx.accounts.filter(isResolvable);
        const unresolvedAccs = getPaths(resolvableAccs)
          .filter((path) => !this.get(path))
          .map((path) => path.reduce((acc, p) => acc + "." + p))
          .map((acc) => `\`${acc}\``)
          .join(", ");

        throw new Error(
          [
            `Reached maximum depth for account resolution.`,
            `Unresolved accounts: ${unresolvedAccs}`,
          ].join(" ")
        );
      }
    }
  }

  public resolveOptionals(accounts: PartialAccounts) {
    Object.assign(
      this._accounts,
      this.resolveOptionalsHelper(accounts, this._idlIx.accounts)
    );
  }

  private get(path: string[]): Address | undefined {
    // Only return if address
    const ret = path.reduce(
      (acc, subPath) => acc && acc[subPath],
      this._accounts
    );

    if (typeof ret === "string") {
      return ret;
    }
  }

  private set(path: string[], value: Address): void {
    let cur = this._accounts;
    path.forEach((p, i) => {
      const isLast = i === path.length - 1;
      if (isLast) {
        cur[p] = value;
      }

      cur[p] = cur[p] ?? {};
      cur = cur[p] as AccountsGeneric;
    });
  }

  private resolveOptionalsHelper(
    partialAccounts: PartialAccounts,
    accounts: IdlInstructionAccountItem[]
  ): AccountsGeneric {
    const nestedAccountsGeneric: AccountsGeneric = {};
    // Looping through accountItem array instead of on partialAccounts, so
    // we only traverse array once
    for (const accountItem of accounts) {
      const accountName = accountItem.name;
      const partialAccount = partialAccounts[accountName];
      // Skip if the account isn't included (thus would be undefined)
      if (partialAccount === undefined) continue;

      if (isPartialAccounts(partialAccount)) {
        // is compound accounts, recurse one level deeper
        if (isCompositeAccounts(accountItem)) {
          nestedAccountsGeneric[accountName] = this.resolveOptionalsHelper(
            partialAccount,
            accountItem["accounts"]
          );
        } else {
          // Here we try our best to recover gracefully. If there are optionals we can't check, we will fail then.
          nestedAccountsGeneric[accountName] = flattenPartialAccounts(
            partialAccount,
            true
          );
        }
      } else {
        // if not compound accounts, do null/optional check and proceed
        if (partialAccount !== null) {
          nestedAccountsGeneric[accountName] = toAddress(
            partialAccount as AnchorAddress
          );
        } else if (accountItem["optional"]) {
          nestedAccountsGeneric[accountName] = this._programId;
        }
      }
    }
    return nestedAccountsGeneric;
  }

  private async resolveCustom() {
    if (this._customResolver) {
      const { accounts, resolved } = await this._customResolver({
        args: this._args,
        accounts: this._accounts,
        provider: this._provider,
        programAddress: this._programId,
        idlIx: this._idlIx,
      });
      // The methods builder shares this object, so a resolver returning a
      // fresh one is written back in place; legacy public keys it may
      // return are normalised to addresses like every other input.
      const normalised = normaliseAccounts(accounts);
      for (const name of Object.keys(this._accounts)) {
        delete this._accounts[name];
      }
      Object.assign(this._accounts, normalised);
      return resolved;
    }

    return 0;
  }

  /**
   * Resolve event CPI accounts `eventAuthority` and `program`.
   *
   * Accounts will only be resolved if they are declared next to each other to
   * reduce the chance of name collision.
   */
  private async resolveEventCpi(
    accounts: IdlInstructionAccountItem[],
    path: string[] = []
  ): Promise<void> {
    for (const i in accounts) {
      const accountOrAccounts = accounts[i];
      if (isCompositeAccounts(accountOrAccounts)) {
        await this.resolveEventCpi(accountOrAccounts.accounts, [
          ...path,
          accountOrAccounts.name,
        ]);
      }

      // Validate next index exists
      const nextIndex = +i + 1;
      if (nextIndex === accounts.length) return;

      const currentName = accounts[i].name;
      const nextName = accounts[nextIndex].name;

      // Populate event CPI accounts if they exist
      if (currentName === "eventAuthority" && nextName === "program") {
        const currentPath = [...path, currentName];
        const nextPath = [...path, nextName];

        if (!this.get(currentPath)) {
          const [eventAuthority] = await getProgramDerivedAddress({
            programAddress: this._programId,
            seeds: ["__event_authority"],
          });
          this.set(currentPath, eventAuthority);
        }
        if (!this.get(nextPath)) {
          this.set(nextPath, this._programId);
        }

        return;
      }
    }
  }

  private resolveConst(
    accounts: IdlInstructionAccountItem[],
    path: string[] = []
  ) {
    for (const accountOrAccounts of accounts) {
      const name = accountOrAccounts.name;
      if (isCompositeAccounts(accountOrAccounts)) {
        this.resolveConst(accountOrAccounts.accounts, [...path, name]);
      } else {
        const account = accountOrAccounts;

        if ((account.signer || account.address) && !this.get([...path, name])) {
          // Default signers to the provider's wallet
          if (account.signer) {
            if (!this._provider.wallet) {
              throw new Error(
                "This function requires the `Provider` interface implementor to have a `wallet` field."
              );
            }
            this.set([...path, name], this._provider.wallet.address);
          }

          // Set based on `address` field
          if (account.address) {
            this.set([...path, name], toAddress(account.address));
          }
        }
      }
    }
  }

  private async resolvePdasAndRelations(
    accounts: IdlInstructionAccountItem[],
    path: string[] = []
  ): Promise<number> {
    let found = 0;
    for (const accountOrAccounts of accounts) {
      const name = accountOrAccounts.name;
      if (isCompositeAccounts(accountOrAccounts)) {
        found += await this.resolvePdasAndRelations(
          accountOrAccounts.accounts,
          [...path, name]
        );
      } else {
        const account = accountOrAccounts;
        if ((account.pda || account.relations) && !this.get([...path, name])) {
          found++;

          // Accounts might not get resolved successfully if a seed depends on
          // another seed to be resolved *and* the accounts for resolution are
          // out of order. In this case, skip the accounts that throw in order
          // to resolve those accounts later.
          try {
            if (account.pda) {
              const seeds = await Promise.all(
                account.pda.seeds.map((seed) => this.toSeed(seed, path))
              );
              if (seeds.some((seed) => !seed)) {
                continue;
              }

              const programAddress = await this.parseProgramId(account, path);
              const [address] = await getProgramDerivedAddress({
                programAddress,
                seeds: seeds as ReadonlyUint8Array[],
              });

              this.set([...path, name], address);
            }
          } catch {}

          try {
            if (account.relations) {
              const accountKey = this.get([...path, account.relations[0]]);
              if (accountKey) {
                const account = await this._accountStore.fetchAccount({
                  address: accountKey,
                });
                this.set([...path, name], toAddress(account[name]));
              }
            }
          } catch {}
        }
      }
    }

    return found;
  }

  private async parseProgramId(
    account: IdlInstructionAccount,
    path: string[] = []
  ): Promise<Address> {
    if (!account.pda?.program) {
      return this._programId;
    }

    const bytes = await this.toSeed(account.pda.program, path);
    if (!bytes) {
      throw new Error(`Program seed not resolved: ${account.name}`);
    }

    return getAddressDecoder().decode(bytes);
  }

  private async toSeed(
    seed: IdlSeed,
    path: string[] = []
  ): Promise<ReadonlyUint8Array | undefined> {
    switch (seed.kind) {
      case "const":
        return this.toSeedConst(seed);
      case "arg":
        return await this.toSeedArg(seed);
      case "account":
        return await this.toSeedAccount(seed, path);
      default:
        throw new Error(`Unexpected seed: ${seed}`);
    }
  }

  private toSeedConst(seed: IdlSeedConst): ReadonlyUint8Array {
    return this.toSeedValue("bytes", seed.value);
  }

  private async toSeedArg(
    seed: IdlSeedArg
  ): Promise<ReadonlyUint8Array | undefined> {
    const [name, ...path] = seed.path.split(".");

    const index = this._idlIx.args.findIndex((arg) => arg.name === name);
    if (index === -1) {
      throw new Error(`Unable to find argument for seed: ${name}`);
    }

    const value = path.reduce(
      (acc, path) => (acc ?? {})[path],
      this._args[index]
    );
    if (value === undefined) {
      return;
    }

    const type = this.getType(this._idlIx.args[index].type, path);
    return this.toSeedValue(type, value);
  }

  private async toSeedAccount(
    seed: IdlSeedAccount,
    path: string[] = []
  ): Promise<ReadonlyUint8Array | undefined> {
    const [name, ...paths] = seed.path.split(".");
    const fieldAddress = this.get([...path, name]);
    if (!fieldAddress) return;

    // The seed is the address of the account.
    if (!paths.length) {
      return this.toSeedValue("pubkey", fieldAddress);
    }

    if (!seed.account) {
      throw new Error(
        `Seed account is required in order to resolve type: ${seed.path}`
      );
    }

    // The key is account data.
    //
    // Fetch and deserialize it.
    const account = await this._accountStore.fetchAccount({
      address: fieldAddress,
      name: seed.account,
    });

    // Dereference all fields in the path to get the field value
    // used in the seed.
    let accountValue = account;
    let currentPaths = paths;
    while (currentPaths.length > 0) {
      accountValue = accountValue[currentPaths[0]];
      currentPaths = currentPaths.slice(1);
    }
    if (accountValue === undefined) return;

    const type = this.getType({ defined: { name: seed.account } }, paths);
    return this.toSeedValue(type, accountValue);
  }

  /**
   * Encodes the given IDL value as PDA seed bytes. The values here must be
   * primitives, e.g. no structs.
   */
  private toSeedValue(type: any, value: any): ReadonlyUint8Array {
    switch (type) {
      case "u8":
        return getU8Encoder().encode(value);
      case "i8":
        return getI8Encoder().encode(value);
      case "u16":
        return getU16Encoder().encode(value);
      case "i16":
        return getI16Encoder().encode(value);
      case "u32":
        return getU32Encoder().encode(value);
      case "i32":
        return getI32Encoder().encode(value);
      case "u64":
        return getU64Encoder().encode(value);
      case "i64":
        return getI64Encoder().encode(value);
      case "u128":
        return getU128Encoder().encode(value);
      case "i128":
        return getI128Encoder().encode(value);
      case "u256":
        return getU256Encoder().encode(value);
      case "i256":
        return getI256Encoder().encode(value);
      case "string":
        return getUtf8Encoder().encode(value);
      case "pubkey":
        return getAddressEncoder().encode(toAddress(value));
      case "bytes":
        return toBytes(value);
      default:
        if (type?.array) {
          return toBytes(value);
        }

        throw new Error(`Unexpected seed type: ${type}`);
    }
  }

  /**
   * Recursively get the type at some path of either a primitive or a user
   * defined struct.
   */
  private getType(
    type: IdlType,
    path: string[] = []
  ): Extract<IdlType, string> {
    const typeName = (type as IdlTypeDefined)?.defined?.name;
    if (typeName) {
      // Handle token account separately
      if (typeName === "tokenAccount") {
        switch (path.at(0)) {
          case "mint":
          case "owner":
            return "pubkey";
          case "amount":
          case "delegatedAmount":
            return "u64";
          default:
            throw new Error(`Unknown token account path: ${path}`);
        }
      }

      const definedType = this._idlTypes.find((t) => t.name === typeName);
      if (!definedType) {
        throw new Error(`Type not found: ${typeName}`);
      }

      // Only named structs are supported
      const [fieldName, ...subPath] = path;
      const fields = (definedType.type as IdlTypeDefTyStruct)
        .fields as IdlDefinedFieldsNamed;
      const field = fields.find((field) => field.name === fieldName);
      if (!field) {
        throw new Error(`Field not found: ${fieldName}`);
      }

      return this.getType(field.type, subPath);
    }

    return type as Extract<IdlType, string>;
  }
}

/**
 * Encodes raw seed bytes given as a byte array or, as `Buffer.from` used to
 * accept, a UTF-8 string.
 */
function toBytes(value: string | ArrayLike<number>): ReadonlyUint8Array {
  return typeof value === "string"
    ? getUtf8Encoder().encode(value)
    : Uint8Array.from(value);
}

/**
 * Converts every address in the given accounts, including legacy public
 * keys, to a Kit address, preserving nesting. Accounts left `null` or
 * `undefined`, e.g. by a custom resolver that has not figured them out yet,
 * are dropped so they read as unresolved.
 */
function normaliseAccounts(accounts: AccountsGeneric): AccountsGeneric {
  return Object.fromEntries(
    Object.entries(accounts)
      .filter(([, value]) => value != null)
      .map(([name, value]) => [
        name,
        typeof value === "object" && !("toBase58" in value)
          ? normaliseAccounts(value)
          : toAddress(value as AnchorAddress),
      ])
  );
}

// TODO: this should be configurable to avoid unnecessary requests.
class AccountStore {
  private _cache = new Map<Address, any>();
  private _coders: Record<Address, AccountsCoder> = {};

  constructor(
    private _provider: Provider,
    accountsCoder: AccountsCoder,
    programId: Address
  ) {
    this._coders[programId] = accountsCoder;
  }

  public async fetchAccount<T = any>({
    address,
    name,
  }: {
    address: Address;
    name?: string;
  }): Promise<T> {
    if (!this._cache.has(address)) {
      const accountInfo = await fetchEncodedAccount(
        this._provider.rpc,
        address,
        withProviderDefaults(this._provider)
      );
      if (!accountInfo.exists) {
        throw new Error(`Account not found: ${address}`);
      }

      if (name === "tokenAccount") {
        const account = getTokenDecoder().decode(accountInfo.data);
        this._cache.set(address, account);
      } else {
        const coder = await this.getAccountsCoder(accountInfo.programAddress);
        if (coder) {
          const account = (coder as BorshAccountsCoder).decodeAny(
            Buffer.from(accountInfo.data)
          );
          this._cache.set(address, account);
        }
      }
    }

    return this._cache.get(address);
  }

  private async getAccountsCoder(
    programId: Address
  ): Promise<AccountsCoder | undefined> {
    if (!this._coders[programId]) {
      const idl = await Program.fetchIdl(programId, this._provider);
      if (idl) {
        const program = new Program(idl, this._provider);
        this._coders[programId] = program.coder.accounts;
      }
    }

    return this._coders[programId];
  }
}
