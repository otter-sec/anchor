import {
  AccountRole,
  Address,
  address,
  addSignersToTransactionMessage,
  appendTransactionMessageInstructions,
  assertIsSendableTransaction,
  assertIsTransactionWithBlockhashLifetime,
  assertIsTransactionWithinSizeLimit,
  Blockhash,
  ClientWithRpc,
  ClientWithRpcSubscriptions,
  Commitment,
  compileTransaction,
  createKeyPairSignerFromBytes,
  createSolanaRpc,
  createSolanaRpcSubscriptions,
  createTransactionMessage,
  getBase64EncodedWireTransaction,
  getCompiledTransactionMessageDecoder,
  getSignatureFromTransaction,
  getTransactionDecoder,
  Instruction,
  isSolanaError,
  isTransactionModifyingSigner,
  isTransactionPartialSigner,
  partiallySignTransactionMessageWithSigners,
  pipe,
  Rpc,
  RpcSubscriptions,
  sendAndConfirmTransactionFactory,
  setTransactionMessageFeePayer,
  setTransactionMessageFeePayerSigner,
  setTransactionMessageLifetimeUsingBlockhash,
  Signature,
  signTransactionMessageWithSigners,
  SolanaRpcApiMainnet,
  SolanaRpcSubscriptionsApi,
  SOLANA_ERROR__JSON_RPC__SERVER_ERROR_SEND_TRANSACTION_PREFLIGHT_FAILURE,
  SOLANA_ERROR__TRANSACTION_ERROR__ALREADY_PROCESSED,
  Transaction as KitTransaction,
  TransactionModifyingSigner,
  TransactionPartialSigner,
  TransactionSigner,
  TransactionWithBlockhashLifetime,
  TransactionWithLifetime,
} from "@solana/kit";
import {
  BlockhashWithExpiryBlockHeight,
  Commitment as LegacyCommitment,
  ConfirmOptions,
  Connection,
  PublicKey,
  Signer,
  Transaction,
  TransactionInstruction,
  TransactionSignature,
  VersionedTransaction,
} from "@solana/web3.js";
import {
  findSolanaError,
  isBrowser,
  isVersionedTransaction,
} from "./utils/common.js";
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

export default interface Provider {
  /** Kit RPC client for the Solana JSON-RPC API. */
  readonly rpc: Rpc<SolanaRpcApiMainnet>;
  /** Kit RPC client for the Solana RPC subscriptions API. */
  readonly rpcSubscriptions?: RpcSubscriptions<SolanaRpcSubscriptionsApi>;
  /** The signer paying for and co-signing transactions sent by this provider. */
  readonly wallet?: WalletSigner;

  /**
   * @deprecated Legacy web3.js bridge, consumed by the program namespaces
   * until their own migration to Kit. Requires the provider to know its
   * cluster URL.
   */
  readonly connection: Connection;
  /** @deprecated Use `wallet.address` instead. */
  readonly publicKey?: PublicKey;

  sendAndConfirm?(
    tx: Transaction | VersionedTransaction,
    signers?: Signer[],
    opts?: ConfirmOptionsWithBlockhash
  ): Promise<TransactionSignature>;
  sendAll?<T extends Transaction | VersionedTransaction>(
    txWithSigners: {
      tx: T;
      signers?: Signer[];
    }[],
    opts?: ConfirmOptions
  ): Promise<Array<TransactionSignature>>;
  simulate?(
    tx: Transaction | VersionedTransaction,
    signers?: Signer[],
    commitment?: LegacyCommitment,
    includeAccounts?: boolean | PublicKey[]
  ): Promise<SuccessfulTxSimulationResponse>;
}

/**
 * The network and wallet context used to send transactions paid for and signed
 * by the provider.
 */
export class AnchorProvider implements Provider {
  readonly rpc: Rpc<SolanaRpcApiMainnet>;
  readonly rpcSubscriptions: RpcSubscriptions<SolanaRpcSubscriptionsApi>;
  readonly publicKey: PublicKey;

  #url?: string;
  #connection?: Connection;
  #sendAndConfirmTransaction: ReturnType<
    typeof sendAndConfirmTransactionFactory
  >;

  /**
   * @param client The cluster endpoints to connect to, or a Kit client
   *               carrying `rpc` and `rpcSubscriptions` objects.
   * @param wallet The signer paying for and co-signing all transactions.
   * @param opts   Transaction confirmation options to use by default.
   */
  constructor(
    client: ClusterEndpoints | SolanaClient,
    readonly wallet: WalletSigner,
    readonly opts: ConfirmOptions = AnchorProvider.defaultOptions()
  ) {
    if (typeof client === "object" && "rpc" in client) {
      this.rpc = client.rpc;
      this.rpcSubscriptions = client.rpcSubscriptions;
    } else {
      const { url, websocketUrl } =
        typeof client === "string"
          ? { url: client, websocketUrl: undefined }
          : client;
      this.#url = url;
      this.rpc = createSolanaRpc(url);
      this.rpcSubscriptions = createSolanaRpcSubscriptions(
        websocketUrl ?? makeWebsocketUrl(url)
      );
    }
    this.publicKey = new PublicKey(wallet.address);
    this.#sendAndConfirmTransaction = sendAndConfirmTransactionFactory({
      rpc: this.rpc,
      rpcSubscriptions: this.rpcSubscriptions,
    });
  }

  /**
   * @deprecated Legacy web3.js bridge, consumed by the program namespaces
   * until their own migration to Kit.
   */
  get connection(): Connection {
    if (!this.#connection) {
      if (!this.#url) {
        throw new Error(
          "The deprecated `connection` bridge is only available when the " +
            "provider is constructed from cluster endpoints rather than a " +
            "Kit client."
        );
      }
      this.#connection = new Connection(this.#url, this.opts.commitment);
    }
    return this.#connection;
  }

  static defaultOptions(): ConfirmOptions {
    return {
      preflightCommitment: "processed",
      commitment: "processed",
    };
  }

  /**
   * Returns a `Provider` with a wallet read from the local filesystem.
   *
   * @param url  The network cluster url.
   * @param opts The default transaction confirmation options.
   *
   * (This api is for Node only.)
   */
  static local(
    url?: string,
    opts: ConfirmOptions = AnchorProvider.defaultOptions()
  ): AnchorProvider {
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
   * Sends the given transaction, paid for and signed by the provider's wallet.
   *
   * @param tx      The transaction to send.
   * @param signers The signers of the transaction.
   * @param opts    Transaction confirmation options.
   */
  async sendAndConfirm(
    tx: Transaction | VersionedTransaction,
    signers?: Signer[],
    opts?: ConfirmOptionsWithBlockhash
  ): Promise<TransactionSignature> {
    if (opts === undefined) {
      opts = this.opts;
    }
    const commitment = toCommitment(opts.commitment) ?? "processed";

    if (isVersionedTransaction(tx)) {
      if (signers) {
        tx.sign(signers);
      }
      const [signed] = await this.#walletSign([
        attachBlockhashLifetime(
          getTransactionDecoder().decode(tx.serialize()),
          await this.#lastValidBlockHeight(opts, commitment)
        ),
      ]);
      assertIsTransactionWithBlockhashLifetime(signed);
      return await this.#sendSigned(signed, opts, commitment);
    }

    const extraSigners = await fromLegacySigners(signers ?? []);

    // Retry loop: on fast local validators (e.g. surfpool with 400ms slot
    // time), repeat calls to `getLatestBlockhash` can return the same
    // blockhash, producing byte-identical txs with identical signatures and
    // tripping "already processed". On that specific error, wait past one
    // slot, refresh the blockhash, and retry.
    const ALREADY_PROCESSED_MAX_ATTEMPTS = 3;
    const ALREADY_PROCESSED_RETRY_DELAY_MS = 500;
    const callerSetBlockhash =
      !!tx.recentBlockhash && tx.recentBlockhash !== DEFAULT_RECENT_BLOCKHASH;

    for (let attempt = 0; attempt < ALREADY_PROCESSED_MAX_ATTEMPTS; attempt++) {
      const lifetime = callerSetBlockhash
        ? {
            blockhash: tx.recentBlockhash as Blockhash,
            lastValidBlockHeight: await this.#lastValidBlockHeight(
              opts,
              commitment
            ),
          }
        : (
            await this.rpc
              .getLatestBlockhash({
                commitment:
                  toCommitment(opts.preflightCommitment) ?? commitment,
              })
              .send()
          ).value;

      const signed = await signTransactionMessageWithSigners(
        this.#fromLegacyTransaction(tx, extraSigners, lifetime)
      );
      assertIsTransactionWithBlockhashLifetime(signed);

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
          isAlreadyProcessed &&
          !callerSetBlockhash &&
          attempt < ALREADY_PROCESSED_MAX_ATTEMPTS - 1;
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
   * Similar to `sendAndConfirm`, but for an array of transactions and signers.
   *
   * The wallet co-signs the whole batch in a single request so that wallets
   * prompting the user for each signature only prompt once. As in v1, the
   * extra signers sign before the wallet, so a modifying wallet that alters
   * a message invalidates their signatures.
   *
   * @param txWithSigners Array of transactions and signers.
   * @param opts          Transaction confirmation options.
   */
  async sendAll<T extends Transaction | VersionedTransaction>(
    txWithSigners: {
      tx: T;
      signers?: Signer[];
    }[],
    opts?: ConfirmOptions
  ): Promise<Array<TransactionSignature>> {
    if (opts === undefined) {
      opts = this.opts;
    }
    const commitment = toCommitment(opts.commitment) ?? "processed";
    const lifetime = (
      await this.rpc
        .getLatestBlockhash({
          commitment: toCommitment(opts.preflightCommitment) ?? commitment,
        })
        .send()
    ).value;

    const pending: (KitTransaction & TransactionWithLifetime)[] = [];
    for (const { tx, signers } of txWithSigners) {
      if (isVersionedTransaction(tx)) {
        if (signers) {
          tx.sign(signers);
        }
        pending.push(
          attachBlockhashLifetime(
            getTransactionDecoder().decode(tx.serialize()),
            lifetime.lastValidBlockHeight
          )
        );
      } else {
        const extraSigners = await fromLegacySigners(signers ?? []);
        pending.push(
          await partiallySignTransactionMessageWithSigners(
            this.#fromLegacyTransaction(tx, extraSigners, lifetime, {
              walletSigns: false,
            })
          )
        );
      }
    }

    const signedAll = await this.#walletSign(pending);

    const sigs: TransactionSignature[] = [];
    for (const signed of signedAll) {
      assertIsTransactionWithBlockhashLifetime(signed);
      sigs.push(await this.#sendSigned(signed, opts, commitment));
    }
    return sigs;
  }

  /**
   * Simulates the given transaction, returning emitted logs from execution.
   *
   * @param tx        The transaction to simulate.
   * @param signers   The signers of the transaction. If unset, the
   *                  transaction is simulated without signature verification,
   *                  which allows simulating without asking the wallet to
   *                  sign.
   * @param commitment The commitment to simulate against.
   * @param includeAccounts Post-simulation accounts to include in the
   *                  response: either an explicit list of addresses, or
   *                  `true` for every non-program account referenced by the
   *                  transaction. Only supported for legacy transactions.
   */
  async simulate(
    tx: Transaction | VersionedTransaction,
    signers?: Signer[],
    commitment?: LegacyCommitment,
    includeAccounts?: boolean | PublicKey[]
  ): Promise<SuccessfulTxSimulationResponse> {
    const kitCommitment =
      toCommitment(commitment) ??
      toCommitment(this.opts.commitment) ??
      "processed";
    const sigVerify = !!signers && signers.length > 0;

    let wire: ReturnType<typeof getBase64EncodedWireTransaction>;
    let addresses: Address[] | undefined;
    if (isVersionedTransaction(tx)) {
      if (sigVerify) {
        tx.sign(signers!);
        const { value } = await this.rpc
          .getLatestBlockhash({ commitment: kitCommitment })
          .send();
        const [signed] = await this.#walletSign([
          attachBlockhashLifetime(
            getTransactionDecoder().decode(tx.serialize()),
            value.lastValidBlockHeight
          ),
        ]);
        wire = getBase64EncodedWireTransaction(signed);
      } else {
        wire = getBase64EncodedWireTransaction(
          getTransactionDecoder().decode(tx.serialize())
        );
      }
    } else {
      const extraSigners = await fromLegacySigners(signers ?? []);
      const lifetime = (
        await this.rpc.getLatestBlockhash({ commitment: kitCommitment }).send()
      ).value;
      const message = this.#fromLegacyTransaction(tx, extraSigners, lifetime);
      wire = getBase64EncodedWireTransaction(
        sigVerify
          ? await signTransactionMessageWithSigners(message)
          : compileTransaction(message)
      );

      if (includeAccounts) {
        addresses = Array.isArray(includeAccounts)
          ? includeAccounts.map((key) => address(key.toBase58()))
          : nonProgramAddresses(tx, this.publicKey);
      }
    }

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
   * Builds a Kit transaction message from a legacy web3.js transaction.
   *
   * The wallet signs as the fee payer unless another fee payer is set on the
   * transaction, in which case the wallet still co-signs any account it is
   * referenced by. Pass `walletSigns: false` to leave every wallet signature
   * slot open, e.g. to batch-sign afterwards.
   */
  #fromLegacyTransaction(
    tx: Transaction,
    extraSigners: TransactionSigner[],
    lifetime: Readonly<{ blockhash: Blockhash; lastValidBlockHeight: bigint }>,
    { walletSigns = true }: { walletSigns?: boolean } = {}
  ) {
    if (tx.nonceInfo) {
      throw new Error(
        "Durable nonce transactions are not supported by the provider yet."
      );
    }

    const feePayer = address((tx.feePayer ?? this.publicKey).toBase58());
    const signers = walletSigns ? [this.wallet, ...extraSigners] : extraSigners;
    const feePayerSigner = signers.find(
      (signer) => signer.address === feePayer
    );
    return pipe(
      createTransactionMessage({ version: "legacy" }),
      (message) =>
        feePayerSigner
          ? setTransactionMessageFeePayerSigner(feePayerSigner, message)
          : setTransactionMessageFeePayer(feePayer, message),
      (message) =>
        setTransactionMessageLifetimeUsingBlockhash(lifetime, message),
      (message) =>
        appendTransactionMessageInstructions(
          tx.instructions.map(fromLegacyInstruction),
          message
        ),
      (message) => addSignersToTransactionMessage(signers, message)
    );
  }

  /**
   * Has the wallet sign the given Kit transactions, supporting both partial
   * and modifying signers. The whole batch is signed in a single request so
   * that wallets prompting the user only prompt once.
   */
  async #walletSign(
    transactions: (KitTransaction & TransactionWithLifetime)[]
  ): Promise<readonly (KitTransaction & TransactionWithLifetime)[]> {
    if (isTransactionPartialSigner(this.wallet)) {
      const sized = transactions.map((tx) => {
        assertIsTransactionWithinSizeLimit(tx);
        return tx;
      });
      const signatureDictionaries = await this.wallet.signTransactions(sized);
      return sized.map((tx, index) => ({
        ...tx,
        signatures: Object.freeze({
          ...tx.signatures,
          ...signatureDictionaries[index],
        }),
      }));
    }
    if (isTransactionModifyingSigner(this.wallet)) {
      return await this.wallet.modifyAndSignTransactions(transactions);
    }
    throw new Error(
      "The provider wallet must implement `signTransactions` or " +
        "`modifyAndSignTransactions` to sign transactions before sending."
    );
  }

  /**
   * The block height until which a transaction whose blockhash was provided
   * by the caller is considered alive. The exact expiry of that blockhash is
   * unknown, so the current blockhash's expiry serves as an upper bound.
   */
  async #lastValidBlockHeight(
    opts: ConfirmOptionsWithBlockhash,
    commitment: Commitment
  ): Promise<bigint> {
    if (opts.blockhash) {
      return BigInt(opts.blockhash.lastValidBlockHeight);
    }
    const { value } = await this.rpc.getLatestBlockhash({ commitment }).send();
    return value.lastValidBlockHeight;
  }

  async #sendSigned(
    transaction: KitTransaction & TransactionWithBlockhashLifetime,
    opts: ConfirmOptions,
    commitment: Commitment
  ): Promise<TransactionSignature> {
    assertIsSendableTransaction(transaction);
    const signature = getSignatureFromTransaction(transaction);
    try {
      await this.#sendAndConfirmTransaction(transaction, {
        commitment,
        skipPreflight: opts.skipPreflight,
        preflightCommitment:
          toCommitment(opts.preflightCommitment ?? opts.commitment) ??
          commitment,
        maxRetries:
          opts.maxRetries != null ? BigInt(opts.maxRetries) : undefined,
        minContextSlot:
          opts.minContextSlot != null ? BigInt(opts.minContextSlot) : undefined,
      });
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

export type ConfirmOptionsWithBlockhash = ConfirmOptions & {
  blockhash?: BlockhashWithExpiryBlockHeight;
};

// The recentBlockhash placeholder web3.js serialises when none was set.
const DEFAULT_RECENT_BLOCKHASH = "11111111111111111111111111111111";

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
 * Coerces a legacy web3.js commitment (which includes deprecated aliases)
 * into a Kit commitment.
 */
function toCommitment(
  commitment: LegacyCommitment | undefined
): Commitment | undefined {
  switch (commitment) {
    case undefined:
      return undefined;
    case "processed":
    case "recent":
      return "processed";
    case "confirmed":
    case "single":
    case "singleGossip":
      return "confirmed";
    case "finalized":
    case "root":
    case "max":
      return "finalized";
  }
}

/**
 * Attaches a blockhash lifetime to a Kit transaction decoded from wire
 * bytes. The blockhash is read back from the compiled message; its exact
 * expiry is unknown, so the caller provides an upper bound (typically the
 * expiry of the latest blockhash).
 */
function attachBlockhashLifetime(
  tx: KitTransaction,
  lastValidBlockHeight: bigint
): KitTransaction & TransactionWithBlockhashLifetime {
  const message = getCompiledTransactionMessageDecoder().decode(
    tx.messageBytes
  );
  return {
    ...tx,
    lifetimeConstraint: {
      blockhash: message.lifetimeToken as Blockhash,
      lastValidBlockHeight,
    },
  };
}

function fromLegacyInstruction(ix: TransactionInstruction): Instruction {
  return {
    programAddress: address(ix.programId.toBase58()),
    accounts: ix.keys.map((meta) => ({
      address: address(meta.pubkey.toBase58()),
      role: meta.isSigner
        ? meta.isWritable
          ? AccountRole.WRITABLE_SIGNER
          : AccountRole.READONLY_SIGNER
        : meta.isWritable
        ? AccountRole.WRITABLE
        : AccountRole.READONLY,
    })),
    ...(ix.data.length > 0 ? { data: new Uint8Array(ix.data) } : {}),
  };
}

async function fromLegacySigners(
  signers: Signer[]
): Promise<TransactionSigner[]> {
  return await Promise.all(
    signers.map((signer) => createKeyPairSignerFromBytes(signer.secretKey))
  );
}

/**
 * Every non-program account referenced by the given legacy transaction,
 * mirroring the account list web3.js used to compile its messages from.
 */
function nonProgramAddresses(
  tx: Transaction,
  defaultFeePayer: PublicKey
): Address[] {
  const programIds = new Set(
    tx.instructions.map((ix) => ix.programId.toBase58())
  );
  const accounts = new Set<string>([
    (tx.feePayer ?? defaultFeePayer).toBase58(),
  ]);
  for (const ix of tx.instructions) {
    for (const meta of ix.keys) {
      accounts.add(meta.pubkey.toBase58());
    }
  }
  return [...accounts]
    .filter((account) => !programIds.has(account))
    .map((account) => address(account));
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
