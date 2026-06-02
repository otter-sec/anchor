import { Buffer } from "buffer";
import { Commitment, PublicKey } from "@solana/web3.js";
import { BorshCoder, Coder } from "../coder/index.js";
import {
  Idl,
  IdlInstruction,
  convertIdlToCamelCase,
  decodeIdlAccount,
  idlAddress,
} from "../idl.js";
import Provider, { getProvider } from "../provider.js";
import { CustomAccountResolver } from "./accounts-resolver.js";
import { Address, translateAddress } from "./common.js";
import { EventManager } from "./event.js";
import NamespaceFactory, {
  AccountNamespace,
  IdlEvents,
  InstructionNamespace,
  MethodsNamespace,
  RpcNamespace,
  SimulateNamespace,
  TransactionNamespace,
  ViewNamespace,
} from "./namespace/index.js";

export * from "./common.js";
export * from "./context.js";
export * from "./event.js";
export * from "./namespace/index.js";

/**
 * ## Program
 *
 * Program provides the IDL deserialized client representation of an Anchor
 * program.
 *
 * This API is the one stop shop for all things related to communicating with
 * on-chain programs. Among other things, one can send transactions, fetch
 * deserialized accounts, decode instruction data, subscribe to account
 * changes, and listen to events.
 *
 * In addition to field accessors and methods, the object provides a set of
 * dynamically generated properties, also known as namespaces, that
 * map one-to-one to program methods and accounts. These namespaces generally
 *  can be used as follows:
 *
 * ## Usage
 *
 * ```javascript
 * program.<namespace>.<program-specific-method>
 * ```
 *
 * API specifics are namespace dependent. The examples used in the documentation
 * below will refer to the two counter examples found
 * [here](https://github.com/otter-sec/anchor#examples).
 */
export class Program<IDL extends Idl = Idl> {
  /**
   * Async methods to send signed transactions to *non*-state methods on the
   * program, returning a [[TransactionSignature]].
   *
   * ## Usage
   *
   * ```javascript
   * rpc.<method>(...args, ctx);
   * ```
   *
   * ## Parameters
   *
   * 1. `args` - The positional arguments for the program. The type and number
   *    of these arguments depend on the program being used.
   * 2. `ctx`  - [[Context]] non-argument parameters to pass to the method.
   *    Always the last parameter in the method call.
   *
   * ## Example
   *
   * To send a transaction invoking the `increment` method above,
   *
   * ```javascript
   * const txSignature = await program.rpc.increment({
   *   accounts: {
   *     counter,
   *     authority,
   *   },
   * });
   * ```
   * @deprecated
   * Use program.methods.<method>(...args).rpc() instead
   */
  readonly rpc: RpcNamespace<IDL>;

  /**
   * The namespace provides handles to an [[AccountClient]] object for each
   * account in the program.
   *
   * ## Usage
   *
   * ```javascript
   * program.account.<account-client>
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
  readonly account: AccountNamespace<IDL>;

  /**
   * The namespace provides functions to build [[TransactionInstruction]]
   * objects for each method of a program.
   *
   * ## Usage
   *
   * ```javascript
   * program.instruction.<method>(...args, ctx);
   * ```
   *
   * ## Parameters
   *
   * 1. `args` - The positional arguments for the program. The type and number
   *    of these arguments depend on the program being used.
   * 2. `ctx`  - [[Context]] non-argument parameters to pass to the method.
   *    Always the last parameter in the method call.
   *
   * ## Example
   *
   * To create an instruction for the `increment` method above,
   *
   * ```javascript
   * const tx = await program.instruction.increment({
   *   accounts: {
   *     counter,
   *   },
   * });
   * ```
   * @deprecated
   */
  readonly instruction: InstructionNamespace<IDL>;

  /**
   * The namespace provides functions to build [[Transaction]] objects for each
   * method of a program.
   *
   * ## Usage
   *
   * ```javascript
   * program.transaction.<method>(...args, ctx);
   * ```
   *
   * ## Parameters
   *
   * 1. `args` - The positional arguments for the program. The type and number
   *    of these arguments depend on the program being used.
   * 2. `ctx`  - [[Context]] non-argument parameters to pass to the method.
   *    Always the last parameter in the method call.
   *
   * ## Example
   *
   * To create an instruction for the `increment` method above,
   *
   * ```javascript
   * const tx = await program.transaction.increment({
   *   accounts: {
   *     counter,
   *   },
   * });
   * ```
   * @deprecated
   */
  readonly transaction: TransactionNamespace<IDL>;

  /**
   * The namespace provides functions to simulate transactions for each method
   * of a program, returning a list of deserialized events *and* raw program
   * logs.
   *
   * One can use this to read data calculated from a program on chain, by
   * emitting an event in the program and reading the emitted event client side
   * via the `simulate` namespace.
   *
   * ## simulate
   *
   * ```javascript
   * program.simulate.<method>(...args, ctx);
   * ```
   *
   * ## Parameters
   *
   * 1. `args` - The positional arguments for the program. The type and number
   *    of these arguments depend on the program being used.
   * 2. `ctx`  - [[Context]] non-argument parameters to pass to the method.
   *    Always the last parameter in the method call.
   *
   * ## Example
   *
   * To simulate the `increment` method above,
   *
   * ```javascript
   * const events = await program.simulate.increment({
   *   accounts: {
   *     counter,
   *   },
   * });
   * ```
   * @deprecated
   */
  readonly simulate: SimulateNamespace<IDL>;

  /**
   * The namespace provides a builder API for all APIs on the program.
   * This is an alternative to using namespace the other namespaces..
   */
  readonly methods: MethodsNamespace<IDL>;

  readonly views?: ViewNamespace<IDL>;

  /**
   * Address of the program.
   */
  public get programId(): PublicKey {
    return this._programId;
  }
  private _programId: PublicKey;

  /**
   * IDL in camelCase format to work in TypeScript.
   *
   * See {@link rawIdl} field if you need the original IDL.
   */
  public get idl(): IDL {
    return this._idl;
  }
  private _idl: IDL;

  /**
   * Raw IDL i.e. the original IDL without camelCase conversion.
   *
   * See {@link idl} field if you need the camelCased version of the IDL.
   */
  public get rawIdl(): Idl {
    return this._rawIdl;
  }
  private _rawIdl: Idl;

  /**
   * Coder for serializing requests.
   */
  public get coder(): Coder {
    return this._coder;
  }
  private _coder: Coder;

  /**
   * Wallet and network provider.
   */
  public get provider(): Provider {
    return this._provider;
  }
  private _provider: Provider;

  /**
   * Handles event subscriptions.
   */
  private _events: EventManager;

  /**
   * @param idl       The interface definition.
   * @param provider  The network and wallet context to use. If not provided
   *                  then uses [[getProvider]].
   * @param getCustomResolver A function that returns a custom account resolver
   *                          for the given instruction. This is useful for resolving
   *                          public keys of missing accounts when building instructions
   */
  public constructor(
    idl: any,
    provider: Provider = getProvider(),
    coder?: Coder,
    getCustomResolver?: (
      instruction: IdlInstruction
    ) => CustomAccountResolver<IDL> | undefined
  ) {
    // Fields.
    this._idl = convertIdlToCamelCase(idl);
    this._rawIdl = idl;
    this._provider = provider;
    this._programId = translateAddress(idl.address);
    this._coder = coder ?? new BorshCoder(this._idl);
    this._events = new EventManager(this._programId, provider, this._coder);

    // Dynamic namespaces.
    const [rpc, instruction, transaction, account, simulate, methods, views] =
      NamespaceFactory.build(
        this._idl,
        this._coder,
        this._programId,
        provider,
        getCustomResolver
      );
    this.rpc = rpc;
    this.instruction = instruction;
    this.transaction = transaction;
    this.account = account;
    this.simulate = simulate;
    this.methods = methods;
    this.views = views;
  }

  /**
   * Generates a Program client by fetching the IDL from the network.
   *
   * In order to use this method, an IDL must have been previously initialized
   * via the anchor CLI's `anchor idl init` command.
   *
   * @param programId The on-chain address of the program.
   * @param provider  The network and wallet context.
   */
  public static async at<IDL extends Idl = Idl>(
    address: Address,
    provider?: Provider
  ): Promise<Program<IDL>> {
    const programId = translateAddress(address);

    const idl = await Program.fetchIdl<IDL>(programId, provider);
    if (!idl) {
      throw new Error(`IDL not found for program: ${address.toString()}`);
    }

    return new Program(idl, provider);
  }

  /**
   * Fetches an idl from the blockchain.
   *
   * In order to use this method, an IDL must have been previously initialized
   * via the anchor CLI's `anchor idl init` command.
   *
   * @param programId The on-chain address of the program.
   * @param provider  The network and wallet context.
   */
  public static async fetchIdl<IDL extends Idl = Idl>(
    programAddress: Address,
    provider?: Provider
  ): Promise<IDL | null> {
    provider = provider ?? getProvider();
    const programId = translateAddress(programAddress);
    const idlAddr = idlAddress(programId);
    const accountInfo = await provider.connection.getAccountInfo(idlAddr);
    if (!accountInfo) return null;

    return decodeIdlAccount<IDL>(accountInfo.data);
  }

  /**
   * Invokes the given callback every time the given event is emitted.
   *
   * @param eventName The PascalCase name of the event, provided by the IDL.
   * @param callback  The function to invoke whenever the event is emitted from
   *                  program logs.
   */
  public addEventListener<E extends keyof IdlEvents<IDL>>(
    eventName: E & string,
    callback: (
      event: IdlEvents<IDL>[E],
      slot: number,
      signature: string
    ) => void,
    commitment?: Commitment
  ): number {
    return this._events.addEventListener(eventName, callback, commitment);
  }

  /**
   * Unsubscribes from the given eventName.
   */
  public async removeEventListener(listener: number): Promise<void> {
    return await this._events.removeEventListener(listener);
  }
}
