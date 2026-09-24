import { Buffer } from "buffer";
import {
  Account,
  Address as KitAddress,
  Base58EncodedBytes,
  Commitment,
  createDecoder,
  DataPublisher,
  decodeAccount,
  Decoder,
  FetchAccountConfig,
  FetchAccountsConfig,
  getDataPublisherFromEventEmitter,
  GetProgramAccountsDatasizeFilter,
  GetProgramAccountsMemcmpFilter,
  Instruction,
  MaybeAccount,
  parseBase64RpcAccount,
  ReadonlyUint8Array,
  Slot,
  TransactionSigner,
  TypedEventTarget,
} from "@solana/kit";
import { getCreateAccountInstruction } from "@solana-program/system";
import { PublicKey } from "@solana/web3.js";
import Provider, { getProvider } from "../../provider.js";
import { Idl, IdlAccount } from "../../idl.js";
import { Coder, BorshCoder } from "../../coder/index.js";
import { Address, toAddress } from "../common.js";
import { withProviderDefaults } from "../../utils/common.js";
import { AllAccountsMap, IdlAccounts } from "./types.js";
import * as rpcUtil from "../../utils/rpc.js";

export default class AccountFactory {
  public static build<IDL extends Idl>(
    idl: IDL,
    coder: Coder,
    programId: PublicKey,
    provider?: Provider
  ): AccountNamespace<IDL> {
    return (idl.accounts ?? []).reduce((accountFns, acc) => {
      accountFns[acc.name] = new AccountClient<IDL>(
        idl,
        acc,
        programId,
        provider,
        coder
      );
      return accountFns;
    }, {}) as AccountNamespace<IDL>;
  }
}

type NullableIdlAccount<IDL extends Idl> = IDL["accounts"] extends undefined
  ? IdlAccount
  : NonNullable<IDL["accounts"]>[number];

/**
 * The namespace provides handles to an [[AccountClient]] object for each
 * account in a program.
 *
 * ## Usage
 *
 * ```javascript
 * account.<account-client>
 * ```
 *
 * ## Example
 *
 * To fetch a `Counter` account from the above example,
 *
 * ```javascript
 * const counter = await program.account.counter.fetch(address);
 * console.log(counter.data.count);
 * ```
 *
 * For the full API, see the [[AccountClient]] reference.
 */
export type AccountNamespace<I extends Idl = Idl> = {
  [A in keyof AllAccountsMap<I>]: AccountClient<I, A>;
};

/**
 * A filter narrowing the accounts returned by {@link AccountClient.all}: raw
 * bytes to match right after the discriminator, or a list of Kit
 * `getProgramAccounts` filters appended to the discriminator filter.
 */
export type AccountFilters =
  | ReadonlyUint8Array
  | readonly (
      | GetProgramAccountsMemcmpFilter
      | GetProgramAccountsDatasizeFilter
    )[];

/**
 * The events published by an account subscription: `change` with the latest
 * decoded account, and `error` when the subscription fails.
 */
export type AccountSubscriptionEvents<T extends object> = {
  change: Account<T>;
  error: unknown;
};

export class AccountClient<
  IDL extends Idl = Idl,
  A extends keyof IdlAccounts<IDL> = keyof IdlAccounts<IDL>,
  N extends NullableIdlAccount<IDL> = NullableIdlAccount<IDL>,
  T extends object = IdlAccounts<IDL>[A] extends Record<string, unknown>
    ? IdlAccounts<IDL>[A]
    : never
> {
  /**
   * Returns the number of bytes in this account.
   */
  get size(): number {
    return this._size;
  }
  private _size: number;

  /**
   * Returns the program ID owning all accounts.
   */
  get programId(): PublicKey {
    return this._programId;
  }
  private _programId: PublicKey;

  /**
   * Returns the client's wallet and network provider.
   */
  get provider(): Provider {
    return this._provider;
  }
  private _provider: Provider;

  /**
   * Returns the coder.
   */
  get coder(): Coder {
    return this._coder;
  }
  private _coder: Coder;

  private _idlAccount: N;
  private _programAddress: KitAddress;
  private _decoder: Decoder<T>;

  constructor(
    idl: IDL,
    idlAccount: N,
    programId: PublicKey,
    provider?: Provider,
    coder?: Coder
  ) {
    this._idlAccount = idlAccount;
    this._programId = programId;
    this._programAddress = toAddress(programId);
    this._provider = provider ?? getProvider();
    this._coder = coder ?? new BorshCoder(idl);
    this._size = this._coder.accounts.size(idlAccount.name);
    this._decoder = createDecoder({
      read: (bytes, offset) => [
        this._coder.accounts.decode<T>(
          idlAccount.name,
          Buffer.from(bytes.subarray(offset))
        ),
        bytes.length,
      ],
    });
  }

  /**
   * Returns the account at the given address, whether it exists or not.
   *
   * @param address The address of the account to fetch.
   */
  async fetchNullable(
    address: Address,
    config?: FetchAccountConfig
  ): Promise<MaybeAccount<T>> {
    const { account } = await this.fetchNullableAndContext(address, config);
    return account;
  }

  /**
   * Returns the account at the given address, whether it exists or not,
   * along with the slot it was read at.
   *
   * @param address The address of the account to fetch.
   */
  async fetchNullableAndContext(
    address: Address,
    config: FetchAccountConfig = {}
  ): Promise<{ account: MaybeAccount<T>; context: { slot: Slot } }> {
    const kitAddress = toAddress(address);
    const { abortSignal, ...rpcConfig } = withProviderDefaults(
      this._provider,
      config
    );
    const { value, context } = await this._provider.rpc
      .getAccountInfo(kitAddress, { ...rpcConfig, encoding: "base64" })
      .send({ abortSignal });
    return {
      account: decodeAccount(
        parseBase64RpcAccount(kitAddress, value),
        this._decoder
      ),
      context,
    };
  }

  /**
   * Returns the account at the given address, throwing if it does not exist.
   *
   * @param address The address of the account to fetch.
   */
  async fetch(
    address: Address,
    config?: FetchAccountConfig
  ): Promise<Account<T>> {
    const { account } = await this.fetchAndContext(address, config);
    return account;
  }

  /**
   * Returns the account at the given address along with the slot it was read
   * at, throwing if it does not exist.
   *
   * @param address The address of the account to fetch.
   */
  async fetchAndContext(
    address: Address,
    config?: FetchAccountConfig
  ): Promise<{ account: Account<T>; context: { slot: Slot } }> {
    const { account, context } = await this.fetchNullableAndContext(
      address,
      config
    );
    if (!account.exists) {
      throw new Error(`Account does not exist ${account.address}`);
    }
    return { account, context };
  }

  /**
   * Returns the accounts at the given addresses, whether they exist or not.
   * The call fails if any account holds data of another type.
   *
   * @param addresses The addresses of the accounts to fetch.
   */
  async fetchMultiple(
    addresses: Address[],
    config?: FetchAccountsConfig
  ): Promise<MaybeAccount<T>[]> {
    const batches = await this.fetchMultipleAndContext(addresses, config);
    return batches.flatMap((batch) => batch.accounts);
  }

  /**
   * Returns the accounts at the given addresses, whether they exist or not,
   * in batches of at most 100 (the RPC limit) each read at a single slot.
   *
   * @param addresses The addresses of the accounts to fetch.
   */
  async fetchMultipleAndContext(
    addresses: Address[],
    config?: FetchAccountsConfig
  ): Promise<{ accounts: MaybeAccount<T>[]; context: { slot: Slot } }[]> {
    const batches = await rpcUtil.getMultipleAccountsAndContext(
      this._provider.rpc,
      addresses.map(toAddress),
      withProviderDefaults(this._provider, config)
    );
    return batches.map(({ accounts, context }) => ({
      accounts: accounts.map((account) =>
        decodeAccount(account, this._decoder)
      ),
      context,
    }));
  }

  /**
   * Returns all instances of this account type for the program.
   *
   * @param filters Narrows the results: bytes to match right after the
   *                discriminator, or Kit `getProgramAccounts` filters
   *                appended to the discriminator filter. Unset returns
   *                every instance.
   */
  async all(
    filters?: AccountFilters,
    config: FetchAccountConfig = {}
  ): Promise<Account<T>[]> {
    // Coders describe their accounts with a memcmp filter (offset + bytes),
    // a data size, or both; the system coder, for instance, only knows the
    // size of a nonce account.
    const coderFilter: { offset?: number; bytes?: string; dataSize?: number } =
      this._coder.accounts.memcmp(
        this._idlAccount.name,
        filters && !Array.isArray(filters)
          ? Buffer.from(filters as ReadonlyUint8Array)
          : undefined
      );
    const coderFilters: (
      | GetProgramAccountsMemcmpFilter
      | GetProgramAccountsDatasizeFilter
    )[] = [];
    if (coderFilter.offset != undefined && coderFilter.bytes != undefined) {
      coderFilters.push({
        memcmp: {
          offset: BigInt(coderFilter.offset),
          bytes: coderFilter.bytes as Base58EncodedBytes,
          encoding: "base58",
        },
      });
    }
    if (coderFilter.dataSize != undefined) {
      coderFilters.push({ dataSize: BigInt(coderFilter.dataSize) });
    }
    const { abortSignal, ...rpcConfig } = withProviderDefaults(
      this._provider,
      config
    );
    const accounts = await this._provider.rpc
      .getProgramAccounts(this._programAddress, {
        ...rpcConfig,
        encoding: "base64",
        filters: [...coderFilters, ...(Array.isArray(filters) ? filters : [])],
      })
      .send({ abortSignal });

    return accounts.map(({ pubkey, account }) =>
      decodeAccount(parseBase64RpcAccount(pubkey, account), this._decoder)
    );
  }

  /**
   * Subscribes to changes of the account at the given address, publishing
   * each new state on the `change` channel and failures on `error`.
   *
   * ```typescript
   * const controller = new AbortController();
   * program.account.counter
   *   .subscribe(address, { abortSignal: controller.signal })
   *   .on("change", (counter) => console.log(counter.data.count));
   * // Later, to stop listening:
   * controller.abort();
   * ```
   *
   * Aborting the signal is the only way to stop listening. Turn the
   * subscription into an async iterable with Kit's
   * `createAsyncIterableFromDataPublisher` if preferred.
   */
  subscribe(
    address: Address,
    config: { abortSignal: AbortSignal; commitment?: Commitment }
  ): DataPublisher<AccountSubscriptionEvents<T>> {
    const { rpcSubscriptions } = this._provider;
    if (!rpcSubscriptions) {
      throw new Error(
        "Subscribing to accounts requires the provider to have an " +
          "`rpcSubscriptions` client."
      );
    }
    const kitAddress = toAddress(address);
    const { abortSignal, ...subscriptionConfig } = withProviderDefaults(
      this._provider,
      config
    );
    const target = new EventTarget() as TypedEventTarget<{
      change: CustomEvent<Account<T>>;
      error: CustomEvent<unknown>;
    }>;

    (async () => {
      const notifications = await rpcSubscriptions
        .accountNotifications(kitAddress, {
          ...subscriptionConfig,
          encoding: "base64",
        })
        .subscribe({ abortSignal });
      for await (const { value } of notifications) {
        const account = decodeAccount(
          parseBase64RpcAccount(kitAddress, value),
          this._decoder
        );
        target.dispatchEvent(new CustomEvent("change", { detail: account }));
      }
    })().catch((error) => {
      if (!abortSignal.aborted) {
        target.dispatchEvent(new CustomEvent("error", { detail: error }));
      }
    });

    return getDataPublisherFromEventEmitter(target);
  }

  /**
   * Returns an instruction creating an account of this type, paid for by the
   * provider's wallet and owned by the program.
   *
   * @param newAccount   The signer of the account to create.
   * @param sizeOverride The account size, defaulting to this type's size.
   */
  async createInstruction(
    newAccount: TransactionSigner,
    sizeOverride?: number
  ): Promise<Instruction> {
    const { wallet } = this._provider;
    if (!wallet) {
      throw new Error(
        "Creating accounts requires the provider to have a `wallet`."
      );
    }
    const space = BigInt(sizeOverride ?? this.size);
    const lamports = await this._provider.rpc
      .getMinimumBalanceForRentExemption(
        space,
        withProviderDefaults(this._provider)
      )
      .send();
    return getCreateAccountInstruction({
      payer: wallet,
      newAccount,
      lamports,
      space,
      programAddress: this._programAddress,
    });
  }
}
