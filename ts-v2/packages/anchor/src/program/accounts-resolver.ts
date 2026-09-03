import BN from "bn.js";
import { PublicKey } from "@solana/web3.js";
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
import { AccountNamespace } from "./namespace/account.js";
import { BorshAccountsCoder } from "src/coder/index.js";
import { decodeTokenAccount } from "./token-account-layout";
import { Address, Program, translateAddress } from "./index.js";
import {
  PartialAccounts,
  flattenPartialAccounts,
  isPartialAccounts,
} from "./namespace/methods";

export type AccountsGeneric = {
  [name: string]: PublicKey | AccountsGeneric;
};

export function isAccountsGeneric(
  accounts: PublicKey | AccountsGeneric
): accounts is AccountsGeneric {
  return !(accounts instanceof PublicKey);
}

export type CustomAccountResolver<IDL extends Idl> = (params: {
  args: Array<any>;
  accounts: AccountsGeneric;
  provider: Provider;
  programId: PublicKey;
  idlIx: AllInstructions<IDL>;
}) => Promise<{ accounts: AccountsGeneric; resolved: number }>;

// Populates a given accounts context with PDAs and common missing accounts.
export class AccountsResolver<IDL extends Idl> {
  private _accountStore: AccountStore<IDL>;

  constructor(
    private _args: any[],
    private _accounts: AccountsGeneric,
    private _provider: Provider,
    private _programId: PublicKey,
    private _idlIx: AllInstructions<IDL>,
    accountNamespace: AccountNamespace<IDL>,
    private _idlTypes: IdlTypeDef[],
    private _customResolver?: CustomAccountResolver<IDL>
  ) {
    this._accountStore = new AccountStore(
      _provider,
      accountNamespace,
      _programId
    );
  }

  public args(args: Array<any>): void {
    this._args = args;
  }

  // Note: We serially resolve PDAs one by one rather than doing them
  //       in parallel because there can be dependencies between
  //       addresses. That is, one PDA can be used as a seed in another.
  public async resolve() {
    this.resolveEventCpi(this._idlIx.accounts);
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

  private get(path: string[]): PublicKey | undefined {
    // Only return if pubkey
    const ret = path.reduce(
      (acc, subPath) => acc && acc[subPath],
      this._accounts
    );

    if (ret && ret.toBase58) {
      return ret as PublicKey;
    }
  }

  private set(path: string[], value: PublicKey): void {
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
          nestedAccountsGeneric[accountName] = translateAddress(
            partialAccount as Address
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
        programId: this._programId,
        idlIx: this._idlIx,
      });
      this._accounts = accounts;
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
  private resolveEventCpi(
    accounts: IdlInstructionAccountItem[],
    path: string[] = []
  ): void {
    for (const i in accounts) {
      const accountOrAccounts = accounts[i];
      if (isCompositeAccounts(accountOrAccounts)) {
        this.resolveEventCpi(accountOrAccounts.accounts, [
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
          this.set(
            currentPath,
            PublicKey.findProgramAddressSync(
              [Buffer.from("__event_authority")],
              this._programId
            )[0]
          );
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
          // Default signers to the provider
          if (account.signer) {
            if (!this._provider.publicKey) {
              throw new Error(
                "This function requires the `Provider` interface implementor to have a `publicKey` field."
              );
            }
            this.set([...path, name], this._provider.publicKey);
          }

          // Set based on `address` field
          if (account.address) {
            this.set([...path, name], translateAddress(account.address));
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
                account.pda.seeds.map((seed) => this.toBuffer(seed, path))
              );
              if (seeds.some((seed) => !seed)) {
                continue;
              }

              const programId = await this.parseProgramId(account, path);
              const [pubkey] = PublicKey.findProgramAddressSync(
                seeds as Buffer[],
                programId
              );

              this.set([...path, name], pubkey);
            }
          } catch {}

          try {
            if (account.relations) {
              const accountKey = this.get([...path, account.relations[0]]);
              if (accountKey) {
                const account = await this._accountStore.fetchAccount({
                  publicKey: accountKey,
                });
                this.set([...path, name], account[name]);
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
  ): Promise<PublicKey> {
    if (!account.pda?.program) {
      return this._programId;
    }

    const buf = await this.toBuffer(account.pda.program, path);
    if (!buf) {
      throw new Error(`Program seed not resolved: ${account.name}`);
    }

    return new PublicKey(buf);
  }

  private async toBuffer(
    seed: IdlSeed,
    path: string[] = []
  ): Promise<Buffer | undefined> {
    switch (seed.kind) {
      case "const":
        return this.toBufferConst(seed);
      case "arg":
        return await this.toBufferArg(seed);
      case "account":
        return await this.toBufferAccount(seed, path);
      default:
        throw new Error(`Unexpected seed: ${seed}`);
    }
  }

  private toBufferConst(seed: IdlSeedConst): Buffer {
    return this.toBufferValue("bytes", seed.value);
  }

  private async toBufferArg(seed: IdlSeedArg): Promise<Buffer | undefined> {
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
    return this.toBufferValue(type, value);
  }

  private async toBufferAccount(
    seed: IdlSeedAccount,
    path: string[] = []
  ): Promise<Buffer | undefined> {
    const [name, ...paths] = seed.path.split(".");
    const fieldPubkey = this.get([...path, name]);
    if (!fieldPubkey) return;

    // The seed is a pubkey of the account.
    if (!paths.length) {
      return this.toBufferValue("pubkey", fieldPubkey);
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
      publicKey: fieldPubkey,
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
    return this.toBufferValue(type, accountValue);
  }

  /**
   * Converts the given idl valaue into a Buffer. The values here must be
   * primitives, e.g. no structs.
   */
  private toBufferValue(type: any, value: any): Buffer {
    switch (type) {
      case "u8":
      case "i8":
        return Buffer.from([value]);
      case "u16":
      case "i16":
        return new BN(value).toArrayLike(Buffer, "le", 2);
      case "u32":
      case "i32":
        return new BN(value).toArrayLike(Buffer, "le", 4);
      case "u64":
      case "i64":
        return new BN(value).toArrayLike(Buffer, "le", 8);
      case "u128":
      case "i128":
        return new BN(value).toArrayLike(Buffer, "le", 16);
      case "u256":
      case "i256":
        return new BN(value).toArrayLike(Buffer, "le", 32);
      case "string":
        return Buffer.from(value);
      case "pubkey":
        return value.toBuffer();
      case "bytes":
        return Buffer.from(value);
      default:
        if (type?.array) {
          return Buffer.from(value);
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

// TODO: this should be configurable to avoid unnecessary requests.
class AccountStore<IDL extends Idl> {
  private _cache = new Map<string, any>();
  private _idls: Record<string, AccountNamespace<any>> = {};

  constructor(
    private _provider: Provider,
    accounts: AccountNamespace<IDL>,
    programId: PublicKey
  ) {
    this._idls[programId.toBase58()] = accounts;
  }

  public async fetchAccount<T = any>({
    publicKey,
    name,
  }: {
    publicKey: PublicKey;
    name?: string;
    programId?: PublicKey;
  }): Promise<T> {
    const address = publicKey.toBase58();
    if (!this._cache.has(address)) {
      const accountInfo = await this._provider.connection.getAccountInfo(
        publicKey
      );
      if (accountInfo === null) {
        throw new Error(`Account not found: ${address}`);
      }

      if (name === "tokenAccount") {
        const account = decodeTokenAccount(accountInfo.data);
        this._cache.set(address, account);
      } else {
        const accounts = await this.getAccountsNs(accountInfo.owner);
        if (accounts) {
          const accountNs = Object.values(accounts)[0] as any;
          if (accountNs) {
            const account = (
              accountNs.coder.accounts as BorshAccountsCoder
            ).decodeAny(accountInfo.data);
            this._cache.set(address, account);
          }
        }
      }
    }

    return this._cache.get(address);
  }

  private async getAccountsNs(
    programId: PublicKey
  ): Promise<AccountNamespace<any> | undefined> {
    const programIdStr = programId.toBase58();
    if (!this._idls[programIdStr]) {
      const idl = await Program.fetchIdl(programId, this._provider);
      if (idl) {
        const program = new Program(idl, this._provider);
        this._idls[programIdStr] = program.account;
      }
    }

    return this._idls[programIdStr];
  }
}
