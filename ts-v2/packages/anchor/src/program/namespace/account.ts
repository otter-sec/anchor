import EventEmitter from "eventemitter3";
import {
  Signer,
  PublicKey,
  SystemProgram,
  TransactionInstruction,
  Commitment,
  GetProgramAccountsFilter,
  AccountInfo,
  RpcResponseAndContext,
  Context,
} from "@solana/web3.js";
import Provider, { getProvider } from "../../provider.js";
import { Idl, IdlAccount } from "../../idl.js";
import { Coder, BorshCoder } from "../../coder/index.js";
import { Subscription, Address, translateAddress } from "../common.js";
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
 * ```
 *
 * For the full API, see the [[AccountClient]] reference.
 */
export type AccountNamespace<I extends Idl = Idl> = {
  [A in keyof AllAccountsMap<I>]: AccountClient<I, A>;
};

export class AccountClient<
  IDL extends Idl = Idl,
  A extends keyof IdlAccounts<IDL> = keyof IdlAccounts<IDL>,
  N extends NullableIdlAccount<IDL> = NullableIdlAccount<IDL>,
  T = IdlAccounts<IDL>[A] extends Record<string, unknown>
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

  constructor(
    idl: IDL,
    idlAccount: N,
    programId: PublicKey,
    provider?: Provider,
    coder?: Coder
  ) {
    this._idlAccount = idlAccount;
    this._programId = programId;
    this._provider = provider ?? getProvider();
    this._coder = coder ?? new BorshCoder(idl);
    this._size = this._coder.accounts.size(idlAccount.name);
  }

  /**
   * Returns a deserialized account, returning null if it doesn't exist.
   *
   * @param address The address of the account to fetch.
   */
  async fetchNullable(
    address: Address,
    commitment?: Commitment
  ): Promise<T | null> {
    const { data } = await this.fetchNullableAndContext(address, commitment);
    return data;
  }

  /**
   * Returns a deserialized account along with the associated rpc response context, returning null if it doesn't exist.
   *
   * @param address The address of the account to fetch.
   */
  async fetchNullableAndContext(
    address: Address,
    commitment?: Commitment
  ): Promise<{ data: T | null; context: Context }> {
    const accountInfo = await this.getAccountInfoAndContext(
      address,
      commitment
    );
    const { value, context } = accountInfo;
    return {
      data:
        value && value.data.length !== 0
          ? this._coder.accounts.decode<T>(this._idlAccount.name, value.data)
          : null,
      context,
    };
  }

  /**
   * Returns a deserialized account.
   *
   * @param address The address of the account to fetch.
   */
  async fetch(address: Address, commitment?: Commitment): Promise<T> {
    const { data } = await this.fetchNullableAndContext(address, commitment);
    if (data === null) {
      throw new Error(
        `Account does not exist or has no data ${address.toString()}`
      );
    }
    return data;
  }

  /**
   * Returns a deserialized account along with the associated rpc response context.
   *
   * @param address The address of the account to fetch.
   */
  async fetchAndContext(
    address: Address,
    commitment?: Commitment
  ): Promise<{ data: T | null; context: Context }> {
    const { data, context } = await this.fetchNullableAndContext(
      address,
      commitment
    );
    if (data === null) {
      throw new Error(`Account does not exist ${address.toString()}`);
    }
    return { data, context };
  }

  /**
   * Returns multiple deserialized accounts.
   * Accounts not found or with wrong discriminator are returned as null.
   *
   * @param addresses The addresses of the accounts to fetch.
   */
  async fetchMultiple(
    addresses: Address[],
    commitment?: Commitment
  ): Promise<(T | null)[]> {
    const accounts = await this.fetchMultipleAndContext(addresses, commitment);
    return accounts.map((account) => (account ? account.data : null));
  }

  /**
   * Returns multiple deserialized accounts.
   * Accounts not found or with wrong discriminator are returned as null.
   *
   * @param addresses The addresses of the accounts to fetch.
   */
  async fetchMultipleAndContext(
    addresses: Address[],
    commitment?: Commitment
  ): Promise<({ data: T; context: Context } | null)[]> {
    const accounts = await rpcUtil.getMultipleAccountsAndContext(
      this._provider.connection,
      addresses.map((address) => translateAddress(address)),
      commitment
    );

    // Decode accounts where discriminator is correct, null otherwise
    return accounts.map((result) => {
      if (result == null) {
        return null;
      }
      const { account, context } = result;
      return {
        data: this._coder.accounts.decode(this._idlAccount.name, account.data),
        context,
      };
    });
  }

  /**
   * Returns all instances of this account type for the program.
   *
   * @param filters User-provided filters to narrow the results from `connection.getProgramAccounts`.
   *
   *                When filters are not defined this method returns all
   *                the account instances.
   *
   *                When filters are of type `Buffer`, the filters are appended
   *                after the discriminator.
   *
   *                When filters are of type `GetProgramAccountsFilter[]`,
   *                filters are appended after the discriminator filter.
   */
  async all(
    filters?: Buffer | GetProgramAccountsFilter[]
  ): Promise<ProgramAccount<T>[]> {
    const filter: { offset?: number; bytes?: string; dataSize?: number } =
      this.coder.accounts.memcmp(
        this._idlAccount.name,
        filters instanceof Buffer ? filters : undefined
      );
    const coderFilters: GetProgramAccountsFilter[] = [];
    if (filter?.offset != undefined && filter?.bytes != undefined) {
      coderFilters.push({
        memcmp: { offset: filter.offset, bytes: filter.bytes },
      });
    }
    if (filter?.dataSize != undefined) {
      coderFilters.push({ dataSize: filter.dataSize });
    }
    let resp = await this._provider.connection.getProgramAccounts(
      this._programId,
      {
        commitment: this._provider.connection.commitment,
        filters: [...coderFilters, ...(Array.isArray(filters) ? filters : [])],
      }
    );

    return resp.map(({ pubkey, account }) => {
      return {
        publicKey: pubkey,
        account: this._coder.accounts.decode(
          this._idlAccount.name,
          account.data
        ),
      };
    });
  }

  /**
   * Returns an `EventEmitter` emitting a "change" event whenever the account
   * changes.
   */
  subscribe(address: Address, commitment?: Commitment): EventEmitter {
    const sub = subscriptions.get(address.toString());
    if (sub) {
      return sub.ee;
    }

    const ee = new EventEmitter();
    address = translateAddress(address);
    const listener = this._provider.connection.onAccountChange(
      address,
      (acc) => {
        const account = this._coder.accounts.decode(
          this._idlAccount.name,
          acc.data
        );
        ee.emit("change", account);
      },
      commitment
    );

    subscriptions.set(address.toString(), {
      ee,
      listener,
    });

    return ee;
  }

  /**
   * Unsubscribes from the account at the given address.
   */
  async unsubscribe(address: Address) {
    let sub = subscriptions.get(address.toString());
    if (!sub) {
      console.warn("Address is not subscribed");
      return;
    }
    if (subscriptions) {
      await this._provider.connection
        .removeAccountChangeListener(sub.listener)
        .then(() => {
          subscriptions.delete(address.toString());
        })
        .catch(console.error);
    }
  }

  /**
   * Returns an instruction for creating this account.
   */
  async createInstruction(
    signer: Signer,
    sizeOverride?: number
  ): Promise<TransactionInstruction> {
    const size = this.size;

    if (this._provider.publicKey === undefined) {
      throw new Error(
        "This function requires the Provider interface implementor to have a 'publicKey' field."
      );
    }

    return SystemProgram.createAccount({
      fromPubkey: this._provider.publicKey,
      newAccountPubkey: signer.publicKey,
      space: sizeOverride ?? size,
      lamports:
        await this._provider.connection.getMinimumBalanceForRentExemption(
          sizeOverride ?? size
        ),
      programId: this._programId,
    });
  }

  async getAccountInfo(
    address: Address,
    commitment?: Commitment
  ): Promise<AccountInfo<Buffer> | null> {
    return await this._provider.connection.getAccountInfo(
      translateAddress(address),
      commitment
    );
  }

  async getAccountInfoAndContext(
    address: Address,
    commitment?: Commitment
  ): Promise<RpcResponseAndContext<AccountInfo<Buffer> | null>> {
    return await this._provider.connection.getAccountInfoAndContext(
      translateAddress(address),
      commitment
    );
  }
}

/**
 * @hidden
 *
 * Deserialized account owned by a program.
 */
export type ProgramAccount<T = any> = {
  publicKey: PublicKey;
  account: T;
};

// Tracks all subscriptions.
const subscriptions: Map<string, Subscription> = new Map();
