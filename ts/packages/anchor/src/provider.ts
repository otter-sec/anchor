import {
  Address,
  assertIsSendableTransaction,
  assertIsTransactionWithBlockhashLifetime,
  assertIsTransactionWithinSizeLimit,
  Blockhash,
  ClientWithRpc,
  ClientWithRpcSubscriptions,
  Commitment,
  compileTransaction,
  createSolanaRpc,
  createSolanaRpcSubscriptions,
  getBase64EncodedWireTransaction,
  getSignatureFromTransaction,
  getSignersFromTransactionMessage,
  isSolanaError,
  isTransactionMessageWithBlockhashLifetime,
  isTransactionMessageWithDurableNonceLifetime,
  isTransactionModifyingSigner,
  isTransactionPartialSigner,
  isTransactionWithDurableNonceLifetime,
  partiallySignTransactionWithSigners,
  Rpc,
  RpcSubscriptions,
  sendAndConfirmDurableNonceTransactionFactory,
  sendAndConfirmTransactionFactory,
  setTransactionMessageFeePayer,
  setTransactionMessageLifetimeUsingBlockhash,
  Signature,
  SolanaRpcApiMainnet,
  SolanaRpcSubscriptionsApi,
  SOLANA_ERROR__JSON_RPC__SERVER_ERROR_SEND_TRANSACTION_PREFLIGHT_FAILURE,
  SOLANA_ERROR__TRANSACTION_ERROR__ALREADY_PROCESSED,
  Transaction,
  TransactionMessage,
  TransactionMessageWithFeePayer,
  TransactionMessageWithLifetime,
  TransactionMessageWithSigners,
  TransactionModifyingSigner,
  TransactionPartialSigner,
  TransactionSigner,
  TransactionWithLifetime,
} from "@solana/kit";
import { findSolanaError, isBrowser } from "./utils/common.js";
import { SuccessfulTxSimulationResponse } from "./utils/rpc.js";
import { createLocalWallet } from "./wallet.js";

/**
 * A Kit-style client carrying the RPC capabilities the provider needs: an
 * `rpc` object for the Solana JSON-RPC API and an `rpcSubscriptions` object
 * for the Solana RPC subscriptions API. `SolanaRpcApiMainnet` is the common
 * denominator across clusters, so cluster-specific clients are accepted too.
 */
export type SolanaClient = ClientWithRpc<SolanaRpcApiMainnet> &
  ClientWithRpcSubscriptions<SolanaRpcSubscriptionsApi>;

/**
 * A Kit signer that can sign transactions before they are sent, as required
 * by the provider's wallet: sending-only signers cannot pre-sign and are
 * therefore not supported.
 */
export type WalletSigner =
  | TransactionPartialSigner
  | TransactionModifyingSigner;

/**
 * Endpoints used to construct an {@link AnchorProvider}. When given a single
 * URL, the websocket endpoint is derived from it the same way web3.js used
 * to: `http(s)` becomes `ws(s)` and any explicit port is incremented by one.
 */
export type ClusterEndpoints = string | { url: string; websocketUrl?: string };

/**
 * Options controlling how transactions are sent and confirmed.
 */
export type ConfirmOptions = {
  /** The commitment level to confirm the transaction at. */
  commitment?: Commitment;
  /** The commitment level to simulate the transaction at before sending. */
  preflightCommitment?: Commitment;
  /** Whether to skip the preflight simulation. */
  skipPreflight?: boolean;
  /** The maximum number of times the RPC node retries sending the transaction. */
  maxRetries?: number;
  /** The minimum slot the request may be evaluated at. */
  minContextSlot?: number;
};

/**
 * A transaction message together with the signers it needs on top of those
 * already attached to it.
 */
export type TransactionMessageWithExtraSigners = {
  message: TransactionMessage;
  signers?: TransactionSigner[];
};

export default interface Provider {
  /** Kit RPC client for the Solana JSON-RPC API. */
  readonly rpc: Rpc<SolanaRpcApiMainnet>;
  /** Kit RPC client for the Solana RPC subscriptions API. */
  readonly rpcSubscriptions?: RpcSubscriptions<SolanaRpcSubscriptionsApi>;
  /** The signer paying for and co-signing transactions sent by this provider. */
  readonly wallet?: WalletSigner;
  /**
   * Default options for sending transactions. Reads default to its
   * `commitment` too, so that an account written at a given commitment can
   * be read straight back at the same one; when unset, reads use the Kit
   * client's own default.
   */
  readonly opts?: ConfirmOptions;

  sendAndConfirm?(
    message: TransactionMessage,
    signers?: TransactionSigner[],
    opts?: ConfirmOptions
  ): Promise<Signature>;
  sendAll?(
    messages: TransactionMessageWithExtraSigners[],
    opts?: ConfirmOptions
  ): Promise<Signature[]>;
  simulate?(
    message: TransactionMessage,
    signers?: TransactionSigner[],
    commitment?: Commitment,
    includeAccounts?: boolean | Address[]
  ): Promise<SuccessfulTxSimulationResponse>;
}

/**
 * The network and wallet context used to send transactions paid for and signed
 * by the provider.
 */
export class AnchorProvider implements Provider {
  readonly rpc: Rpc<SolanaRpcApiMainnet>;
  readonly rpcSubscriptions: RpcSubscriptions<SolanaRpcSubscriptionsApi>;
  /**
   * Default confirmation options, completed from {@link defaultOptions} so
   * that partial options (e.g. `{ skipPreflight: true }`) still carry a
   * commitment for sends and reads alike.
   */
  readonly opts: ConfirmOptions;

  #sendAndConfirmTransaction: ReturnType<
    typeof sendAndConfirmTransactionFactory
  >;
  #sendAndConfirmDurableNonceTransaction: ReturnType<
    typeof sendAndConfirmDurableNonceTransactionFactory
  >;

  /**
   * @param client The cluster endpoints to connect to, or a Kit client
   *               carrying `rpc` and `rpcSubscriptions` objects.
   * @param wallet The signer paying for and co-signing all transactions.
   * @param opts   Transaction confirmation options to use by default,
   *               completed from {@link defaultOptions}.
   */
  constructor(
    client: ClusterEndpoints | SolanaClient,
    readonly wallet: WalletSigner,
    opts: ConfirmOptions = {}
  ) {
    this.opts = { ...AnchorProvider.defaultOptions(), ...opts };
    if (typeof client === "object" && "rpc" in client) {
      this.rpc = client.rpc;
      this.rpcSubscriptions = client.rpcSubscriptions;
    } else {
      const { url, websocketUrl } =
        typeof client === "string"
          ? { url: client, websocketUrl: undefined }
          : client;
      this.rpc = createSolanaRpc(url);
      this.rpcSubscriptions = createSolanaRpcSubscriptions(
        websocketUrl ?? makeWebsocketUrl(url)
      );
    }
    this.#sendAndConfirmTransaction = sendAndConfirmTransactionFactory({
      rpc: this.rpc,
      rpcSubscriptions: this.rpcSubscriptions,
    });
    this.#sendAndConfirmDurableNonceTransaction =
      sendAndConfirmDurableNonceTransactionFactory({
        rpc: this.rpc,
        rpcSubscriptions: this.rpcSubscriptions,
      });
  }

  /**
   * The default confirmation options: `confirmed` for both sending and
   * preflight, matching Kit's own client default.
   */
  static defaultOptions(): ConfirmOptions {
    return {
      preflightCommitment: "confirmed",
      commitment: "confirmed",
    };
  }

  /**
   * Returns a `Provider` with a wallet read from the local filesystem.
   *
   * @param url  The network cluster url.
   * @param opts The default transaction confirmation options, completed
   *             from {@link defaultOptions}.
   *
   * (This api is for Node only.)
   */
  static local(url?: string, opts?: ConfirmOptions): AnchorProvider {
    if (isBrowser) {
      throw new Error(`Provider local is not available on browser.`);
    }

    return new AnchorProvider(
      url ?? "http://127.0.0.1:8899",
      createLocalWallet(),
      opts
    );
  }

  /**
   * Returns a `Provider` read from the `ANCHOR_PROVIDER_URL` environment
   * variable
   *
   * (This api is for Node only.)
   */
  static env(): AnchorProvider {
    if (isBrowser) {
      throw new Error(`Provider env is not available on browser.`);
    }

    const process = require("process");
    const url = process.env.ANCHOR_PROVIDER_URL;
    if (url === undefined) {
      throw new Error("ANCHOR_PROVIDER_URL is not defined");
    }

    return new AnchorProvider(url, createLocalWallet());
  }

  /**
   * Sends the given transaction message, paid for and signed by the
   * provider's wallet.
   *
   * The wallet pays the fees unless the message already has a fee payer. A
   * message without a lifetime is given the latest blockhash. A message
   * carrying a lifetime is sent as is and confirmed accordingly: until its
   * blockhash expires, or, for a durable nonce (see Kit's
   * `setTransactionMessageLifetimeUsingDurableNonce`), until the nonce
   * account advances.
   *
   * @param message The transaction message to send.
   * @param signers Signers needed on top of those attached to the message.
   * @param opts    Transaction confirmation options.
   */
  async sendAndConfirm(
    message: TransactionMessage,
    signers?: TransactionSigner[],
    opts?: ConfirmOptions
  ): Promise<Signature> {
    opts = { ...this.opts, ...opts };
    const commitment = opts.commitment ?? "confirmed";
    const prepared = this.#prepare(message, signers ?? []);

    if (hasLifetime(prepared.message)) {
      const [signed] = await this.#walletSign([
        await this.#signWithSigners(prepared.message, prepared.signers),
      ]);
      return await this.#sendSigned(signed, opts, commitment);
    }

    // Retry loop: on fast local validators (e.g. surfpool with 400ms slot
    // time), repeat calls to `getLatestBlockhash` can return the same
    // blockhash, producing byte-identical txs with identical signatures and
    // tripping "already processed". On that specific error, wait past one
    // slot, refresh the blockhash, and retry.
    const ALREADY_PROCESSED_MAX_ATTEMPTS = 3;
    const ALREADY_PROCESSED_RETRY_DELAY_MS = 500;

    for (let attempt = 0; attempt < ALREADY_PROCESSED_MAX_ATTEMPTS; attempt++) {
      const lifetime = await this.#latestBlockhash(
        opts.preflightCommitment ?? commitment
      );
      const [signed] = await this.#walletSign([
        await this.#signWithSigners(
          setTransactionMessageLifetimeUsingBlockhash(
            lifetime,
            prepared.message
          ),
          prepared.signers
        ),
      ]);

      try {
        return await this.#sendSigned(signed, opts, commitment);
      } catch (err) {
        // The message check is a fallback: Kit strips human-readable error
        // messages from production builds, so control flow must rely on the
        // error code in the cause chain.
        const isAlreadyProcessed =
          findSolanaError(
            err,
            SOLANA_ERROR__TRANSACTION_ERROR__ALREADY_PROCESSED
          ) !== undefined ||
          (err instanceof Error &&
            err.message.includes("already been processed"));
        const canRetry =
          isAlreadyProcessed && attempt < ALREADY_PROCESSED_MAX_ATTEMPTS - 1;
        if (!canRetry) {
          throw err;
        }
        await new Promise((r) =>
          setTimeout(r, ALREADY_PROCESSED_RETRY_DELAY_MS)
        );
      }
    }
    throw new Error("unreachable: sendAndConfirm retry loop fell through");
  }

  /**
   * Similar to `sendAndConfirm`, but for an array of transaction messages.
   *
   * The wallet co-signs the whole batch in a single request so that wallets
   * prompting the user for each signature only prompt once. As in v1, the
   * other signers sign before the wallet, so a modifying wallet that alters
   * a message invalidates their signatures.
   *
   * @param messages Transaction messages, each with its extra signers.
   * @param opts     Transaction confirmation options.
   */
  async sendAll(
    messages: TransactionMessageWithExtraSigners[],
    opts?: ConfirmOptions
  ): Promise<Signature[]> {
    opts = { ...this.opts, ...opts };
    const commitment = opts.commitment ?? "confirmed";
    let lifetime: BlockhashLifetime | undefined;

    const pending: (Transaction & TransactionWithLifetime)[] = [];
    for (const { message, signers } of messages) {
      const prepared = this.#prepare(message, signers ?? []);
      let withLifetime: PreparedMessage["message"] &
        TransactionMessageWithLifetime;
      if (hasLifetime(prepared.message)) {
        withLifetime = prepared.message;
      } else {
        lifetime ??= await this.#latestBlockhash(
          opts.preflightCommitment ?? commitment
        );
        withLifetime = setTransactionMessageLifetimeUsingBlockhash(
          lifetime,
          prepared.message
        );
      }
      pending.push(await this.#signWithSigners(withLifetime, prepared.signers));
    }

    const signedAll = await this.#walletSign(pending);

    const signatures: Signature[] = [];
    for (const signed of signedAll) {
      signatures.push(await this.#sendSigned(signed, opts, commitment));
    }
    return signatures;
  }

  /**
   * Simulates the given transaction message, returning emitted logs from
   * execution.
   *
   * @param message   The transaction message to simulate.
   * @param signers   Signers needed on top of those attached to the message.
   *                  If unset, the transaction is simulated without
   *                  signature verification, which allows simulating without
   *                  asking the wallet to sign.
   * @param commitment The commitment to simulate against.
   * @param includeAccounts Post-simulation accounts to include in the
   *                  response: either an explicit list of addresses, or
   *                  `true` for every non-program account referenced by the
   *                  transaction.
   */
  async simulate(
    message: TransactionMessage,
    signers?: TransactionSigner[],
    commitment?: Commitment,
    includeAccounts?: boolean | Address[]
  ): Promise<SuccessfulTxSimulationResponse> {
    const kitCommitment = commitment ?? this.opts.commitment ?? "confirmed";
    const sigVerify = !!signers && signers.length > 0;

    const prepared = this.#prepare(message, signers ?? []);
    const withLifetime = hasLifetime(prepared.message)
      ? prepared.message
      : setTransactionMessageLifetimeUsingBlockhash(
          await this.#latestBlockhash(kitCommitment),
          prepared.message
        );

    const transaction = sigVerify
      ? (
          await this.#walletSign([
            await this.#signWithSigners(withLifetime, prepared.signers),
          ])
        )[0]
      : compileTransaction(withLifetime);
    const wire = getBase64EncodedWireTransaction(transaction);

    const addresses = includeAccounts
      ? Array.isArray(includeAccounts)
        ? includeAccounts
        : nonProgramAddresses(withLifetime)
      : undefined;

    const base = { encoding: "base64", commitment: kitCommitment } as const;
    const result = addresses
      ? sigVerify
        ? await this.rpc
            .simulateTransaction(wire, {
              ...base,
              sigVerify: true,
              accounts: { encoding: "base64", addresses },
            })
            .send()
        : await this.rpc
            .simulateTransaction(wire, {
              ...base,
              accounts: { encoding: "base64", addresses },
            })
            .send()
      : sigVerify
      ? await this.rpc
          .simulateTransaction(wire, { ...base, sigVerify: true })
          .send()
      : await this.rpc.simulateTransaction(wire, base).send();

    if (result.value.err) {
      throw new SimulateError(result.value);
    }

    return result.value;
  }

  /**
   * Gives the message a fee payer (the wallet, unless one is already set) and
   * gathers every signer it needs besides the wallet: those attached to the
   * message plus the extra ones provided.
   */
  #prepare(
    message: TransactionMessage,
    extraSigners: TransactionSigner[]
  ): PreparedMessage {
    const withFeePayer = hasFeePayer(message)
      ? message
      : setTransactionMessageFeePayer(this.wallet.address, message);

    const signers = new Map<Address, TransactionSigner>();
    for (const signer of [
      ...getSignersFromTransactionMessage(
        message as TransactionMessage & TransactionMessageWithSigners
      ),
      ...extraSigners,
    ]) {
      if (
        signer.address !== this.wallet.address &&
        !signers.has(signer.address)
      ) {
        signers.set(signer.address, signer);
      }
    }

    return { message: withFeePayer, signers: [...signers.values()] };
  }

  /**
   * Compiles the message and has every signer it requires, besides the
   * wallet, sign it. Signers not required by the compiled transaction are
   * skipped, as Kit's key pair signers refuse to sign transactions they are
   * not part of.
   */
  async #signWithSigners(
    message: PreparedMessage["message"] & TransactionMessageWithLifetime,
    signers: TransactionSigner[]
  ): Promise<Transaction & TransactionWithLifetime> {
    const transaction = compileTransaction(message);
    const required = signers.filter(
      (signer) => signer.address in transaction.signatures
    );
    if (required.length === 0) {
      return transaction;
    }
    return await partiallySignTransactionWithSigners(
      required.map((signer) => {
        if (
          !isTransactionPartialSigner(signer) &&
          !isTransactionModifyingSigner(signer)
        ) {
          throw new Error(
            `Signer ${signer.address} can only sign and send transactions ` +
              "itself. The provider needs signers that can sign transactions " +
              "before they are sent."
          );
        }
        return signer;
      }),
      transaction
    );
  }

  /**
   * Has the wallet sign the given transactions it is a required signer of,
   * supporting both partial and modifying signers. The whole batch is signed
   * in a single request so that wallets prompting the user only prompt once.
   */
  async #walletSign(
    transactions: (Transaction & TransactionWithLifetime)[]
  ): Promise<(Transaction & TransactionWithLifetime)[]> {
    const indices = transactions.flatMap((tx, index) =>
      this.wallet.address in tx.signatures ? [index] : []
    );
    if (indices.length === 0) {
      return transactions;
    }
    const toSign = indices.map((index) => {
      const tx = transactions[index];
      assertIsTransactionWithinSizeLimit(tx);
      return tx;
    });

    // Unlike Kit, which favours modifying signers, a wallet implementing
    // both interfaces signs partially: it signs last, after the other
    // signers, so modifying the transaction would invalidate their
    // signatures.
    let signed: readonly (Transaction & TransactionWithLifetime)[];
    if (isTransactionPartialSigner(this.wallet)) {
      const signatureDictionaries = await this.wallet.signTransactions(toSign);
      signed = toSign.map((tx, index) => ({
        ...tx,
        signatures: Object.freeze({
          ...tx.signatures,
          ...signatureDictionaries[index],
        }),
      }));
    } else if (isTransactionModifyingSigner(this.wallet)) {
      signed = await this.wallet.modifyAndSignTransactions(toSign);
    } else {
      throw new Error(
        "The provider wallet must implement `signTransactions` or " +
          "`modifyAndSignTransactions` to sign transactions before sending."
      );
    }

    const result = [...transactions];
    indices.forEach((index, position) => {
      result[index] = signed[position];
    });
    return result;
  }

  async #latestBlockhash(commitment: Commitment): Promise<BlockhashLifetime> {
    const { value } = await this.rpc.getLatestBlockhash({ commitment }).send();
    return value;
  }

  async #sendSigned(
    transaction: Transaction & TransactionWithLifetime,
    opts: ConfirmOptions,
    commitment: Commitment
  ): Promise<Signature> {
    assertIsSendableTransaction(transaction);
    const signature = getSignatureFromTransaction(transaction);
    const config = {
      commitment,
      skipPreflight: opts.skipPreflight,
      preflightCommitment: opts.preflightCommitment ?? commitment,
      maxRetries: opts.maxRetries != null ? BigInt(opts.maxRetries) : undefined,
      minContextSlot:
        opts.minContextSlot != null ? BigInt(opts.minContextSlot) : undefined,
    };
    try {
      if (isTransactionWithDurableNonceLifetime(transaction)) {
        await this.#sendAndConfirmDurableNonceTransaction(transaction, config);
      } else {
        assertIsTransactionWithBlockhashLifetime(transaction);
        await this.#sendAndConfirmTransaction(transaction, config);
      }
      return signature;
    } catch (err) {
      throw await this.#enrichSendError(err, signature);
    }
  }

  /**
   * Surfaces program logs on send failures so that errors can be translated
   * into Anchor errors downstream. Preflight failures carry their logs in the
   * error context; for transactions that landed but failed, the logs are
   * recovered with an extra RPC call.
   */
  async #enrichSendError(err: unknown, signature: Signature): Promise<unknown> {
    if (
      isSolanaError(
        err,
        SOLANA_ERROR__JSON_RPC__SERVER_ERROR_SEND_TRANSACTION_PREFLIGHT_FAILURE
      )
    ) {
      // The transaction error is nested as the error's cause, e.g.
      // "Custom program error: #6000". Compose it into the message so
      // errors can be identified from it downstream.
      const message =
        err.cause instanceof Error
          ? `${err.message}: ${err.cause.message}`
          : err.message;
      return new ProviderError(message, err.context.logs ?? undefined, {
        cause: err,
      });
    }

    const failedTx = await this.rpc
      .getTransaction(signature, {
        commitment: "confirmed",
        encoding: "json",
        maxSupportedTransactionVersion: 0,
      })
      .send()
      .catch(() => null);
    const logs = failedTx?.meta?.logMessages;
    if (!logs || logs.length === 0) {
      return err;
    }
    return new ProviderError(
      err instanceof Error ? err.message : String(err),
      [...logs],
      { cause: err }
    );
  }
}

/**
 * An error thrown when sending a transaction fails, carrying the program
 * logs emitted before the failure when they could be recovered.
 */
export class ProviderError extends Error {
  constructor(
    message: string,
    readonly logs?: string[],
    options?: { cause?: unknown }
  ) {
    super(message, options);
    this.name = "ProviderError";
  }
}

/**
 * An error thrown when a transaction simulation fails, carrying the full
 * simulation response including its logs.
 */
export class SimulateError extends Error {
  constructor(
    readonly simulationResponse: SuccessfulTxSimulationResponse & {
      err: unknown;
    },
    message?: string
  ) {
    super(
      message ??
        `Transaction simulation failed: ${JSON.stringify(
          simulationResponse.err,
          (_, value) => (typeof value === "bigint" ? Number(value) : value)
        )}`
    );
    this.name = "SimulateError";
  }

  get logs(): readonly string[] | null {
    return this.simulationResponse.logs;
  }
}

/**
 * A transaction message with a fee payer, alongside the signers it requires
 * besides the wallet.
 */
type PreparedMessage = {
  message: TransactionMessage & TransactionMessageWithFeePayer;
  signers: TransactionSigner[];
};

type BlockhashLifetime = Readonly<{
  blockhash: Blockhash;
  lastValidBlockHeight: bigint;
}>;

function hasFeePayer(
  message: TransactionMessage
): message is TransactionMessage & TransactionMessageWithFeePayer {
  return "feePayer" in message && message.feePayer != null;
}

/** Whether the message carries a lifetime, blockhash or durable nonce. */
function hasLifetime<TMessage extends TransactionMessage>(
  message: TMessage
): message is TMessage & TransactionMessageWithLifetime {
  return (
    isTransactionMessageWithBlockhashLifetime(message) ||
    isTransactionMessageWithDurableNonceLifetime(message)
  );
}

/**
 * Derives a websocket endpoint from an HTTP endpoint the same way web3.js
 * used to: `http(s)` becomes `ws(s)` and any explicit port is incremented by
 * one (e.g. `http://127.0.0.1:8899` becomes `ws://127.0.0.1:8900`).
 */
function makeWebsocketUrl(url: string): string {
  const endpoint = new URL(url);
  endpoint.protocol = endpoint.protocol === "https:" ? "wss:" : "ws:";
  if (endpoint.port !== "") {
    endpoint.port = String(Number(endpoint.port) + 1);
  }
  return endpoint.toString();
}

/**
 * Every non-program account referenced by the given transaction message,
 * mirroring the account list web3.js used to compile its messages from.
 */
function nonProgramAddresses(
  message: TransactionMessage & TransactionMessageWithFeePayer
): Address[] {
  const programAddresses = new Set<Address>(
    message.instructions.map((ix) => ix.programAddress)
  );
  const accounts = new Set<Address>([message.feePayer.address]);
  for (const ix of message.instructions) {
    for (const meta of ix.accounts ?? []) {
      accounts.add(meta.address);
    }
  }
  return [...accounts].filter((account) => !programAddresses.has(account));
}

/**
 * Sets the default provider on the client.
 */
export function setProvider(provider: Provider) {
  _provider = provider;
}

/**
 * Returns the default provider being used by the client.
 */
export function getProvider(): Provider {
  if (_provider === null) {
    return AnchorProvider.local();
  }
  return _provider;
}

// Global provider used as the default when a provider is not given.
let _provider: Provider | null = null;
