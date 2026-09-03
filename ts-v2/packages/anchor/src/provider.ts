import {
  Connection,
  Signer,
  PublicKey,
  Transaction,
  TransactionSignature,
  ConfirmOptions,
  SimulatedTransactionResponse,
  Commitment,
  SendTransactionError,
  SendOptions,
  VersionedTransaction,
  RpcResponseAndContext,
  BlockhashWithExpiryBlockHeight,
  SignatureResult,
  Keypair,
} from "@solana/web3.js";
import { bs58 } from "./utils/bytes/index.js";
import { isBrowser, isVersionedTransaction } from "./utils/common.js";
import {
  simulateTransaction,
  SuccessfulTxSimulationResponse,
} from "./utils/rpc.js";

export default interface Provider {
  readonly connection: Connection;
  readonly publicKey?: PublicKey;
  readonly wallet?: Wallet;

  send?(
    tx: Transaction | VersionedTransaction,
    signers?: Signer[],
    opts?: SendOptions
  ): Promise<TransactionSignature>;
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
    commitment?: Commitment,
    includeAccounts?: boolean | PublicKey[]
  ): Promise<SuccessfulTxSimulationResponse>;
}

/**
 * The network and wallet context used to send transactions paid for and signed
 * by the provider.
 */
export class AnchorProvider implements Provider {
  readonly publicKey: PublicKey;

  /**
   * @param connection The cluster connection where the program is deployed.
   * @param wallet     The wallet used to pay for and sign all transactions.
   * @param opts       Transaction confirmation options to use by default.
   */
  constructor(
    readonly connection: Connection,
    readonly wallet: Wallet,
    readonly opts: ConfirmOptions = AnchorProvider.defaultOptions()
  ) {
    this.publicKey = wallet?.publicKey;
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

    const connection = new Connection(
      url ?? "http://127.0.0.1:8899",
      opts.preflightCommitment
    );
    const NodeWallet = require("./nodewallet.js").default;
    const wallet = NodeWallet.local();
    return new AnchorProvider(connection, wallet, opts);
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
    const options = AnchorProvider.defaultOptions();
    const connection = new Connection(url, options.commitment);
    const NodeWallet = require("./nodewallet.js").default;
    const wallet = NodeWallet.local();

    return new AnchorProvider(connection, wallet, options);
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

    if (isVersionedTransaction(tx)) {
      if (signers) {
        tx.sign(signers);
      }
      tx = await this.wallet.signTransaction(tx);
      return await this._sendWithConfirmErrorHandling(tx, opts);
    }

    tx.feePayer = tx.feePayer ?? this.wallet.publicKey;

    // Retry loop: on fast local validators (e.g. surfpool with 400ms slot
    // time), repeat calls to `getLatestBlockhash` can return the same
    // blockhash, producing byte-identical txs with identical signatures and
    // tripping "already processed". On that specific error, wait past one
    // slot, refresh the blockhash, and retry.
    const ALREADY_PROCESSED_MAX_ATTEMPTS = 3;
    const ALREADY_PROCESSED_RETRY_DELAY_MS = 500;
    const callerSetBlockhash =
      !!tx.recentBlockhash &&
      tx.recentBlockhash !== "11111111111111111111111111111111";

    for (let attempt = 0; attempt < ALREADY_PROCESSED_MAX_ATTEMPTS; attempt++) {
      if (!callerSetBlockhash || attempt > 0) {
        tx.recentBlockhash = (
          await this.connection.getLatestBlockhash(opts.preflightCommitment)
        ).blockhash;
      }
      tx.signatures = [];
      if (signers) {
        for (const signer of signers) {
          tx.partialSign(signer);
        }
      }
      const signed = await this.wallet.signTransaction(tx);

      try {
        return await this._sendWithConfirmErrorHandling(signed, opts);
      } catch (err) {
        const isAlreadyProcessed =
          err instanceof Error &&
          err.message.includes("already been processed");
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

  private async _sendWithConfirmErrorHandling(
    tx: Transaction | VersionedTransaction,
    opts: ConfirmOptionsWithBlockhash
  ): Promise<TransactionSignature> {
    const rawTx = tx.serialize();
    try {
      return await sendAndConfirmRawTransaction(this.connection, rawTx, opts);
    } catch (err) {
      // thrown if the underlying 'confirmTransaction' encounters a failed tx
      // the 'confirmTransaction' error does not return logs so we make another rpc call to get them
      if (err instanceof ConfirmError) {
        // choose the shortest available commitment for 'getTransaction'
        // (the json RPC does not support any shorter than "confirmed" for 'getTransaction')
        // because that will see the tx sent with `sendAndConfirmRawTransaction` no matter which
        // commitment `sendAndConfirmRawTransaction` used
        const txSig = bs58.encode(
          isVersionedTransaction(tx)
            ? tx.signatures?.[0] || new Uint8Array()
            : tx.signature ?? new Uint8Array()
        );
        const maxVer = isVersionedTransaction(tx) ? 0 : undefined;
        const failedTx = await this.connection.getTransaction(txSig, {
          commitment: "confirmed",
          maxSupportedTransactionVersion: maxVer,
        });
        if (!failedTx) {
          throw err;
        } else {
          const logs = failedTx.meta?.logMessages;
          throw !logs ? err : new SendTransactionError(err.message, logs);
        }
      } else {
        throw err;
      }
    }
  }

  /**
   * Similar to `send`, but for an array of transactions and signers.
   * All transactions need to be of the same type, it doesn't support a mix of `VersionedTransaction`s and `Transaction`s.
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
    const recentBlockhash = (
      await this.connection.getLatestBlockhash(opts.preflightCommitment)
    ).blockhash;

    let txs = txWithSigners.map((r) => {
      if (isVersionedTransaction(r.tx)) {
        let tx: VersionedTransaction = r.tx;
        if (r.signers) {
          tx.sign(r.signers);
        }
        return tx;
      } else {
        let tx: Transaction = r.tx;
        let signers = r.signers ?? [];

        tx.feePayer = tx.feePayer ?? this.wallet.publicKey;
        tx.recentBlockhash = recentBlockhash;

        signers.forEach((kp) => {
          tx.partialSign(kp);
        });
        return tx;
      }
    });

    const signedTxs = await this.wallet.signAllTransactions(txs);

    const sigs: TransactionSignature[] = [];

    for (let k = 0; k < txs.length; k += 1) {
      const tx = signedTxs[k];
      const rawTx = tx.serialize();

      try {
        sigs.push(
          await sendAndConfirmRawTransaction(this.connection, rawTx, opts)
        );
      } catch (err) {
        // thrown if the underlying 'confirmTransaction' encounters a failed tx
        // the 'confirmTransaction' error does not return logs so we make another rpc call to get them
        if (err instanceof ConfirmError) {
          // choose the shortest available commitment for 'getTransaction'
          // (the json RPC does not support any shorter than "confirmed" for 'getTransaction')
          // because that will see the tx sent with `sendAndConfirmRawTransaction` no matter which
          // commitment `sendAndConfirmRawTransaction` used
          const txSig = bs58.encode(
            isVersionedTransaction(tx)
              ? tx.signatures?.[0] || new Uint8Array()
              : tx.signature ?? new Uint8Array()
          );
          const maxVer = isVersionedTransaction(tx) ? 0 : undefined;
          const failedTx = await this.connection.getTransaction(txSig, {
            commitment: "confirmed",
            maxSupportedTransactionVersion: maxVer,
          });
          if (!failedTx) {
            throw err;
          } else {
            const logs = failedTx.meta?.logMessages;
            throw !logs ? err : new SendTransactionError(err.message, logs);
          }
        } else {
          throw err;
        }
      }
    }

    return sigs;
  }

  /**
   * Simulates the given transaction, returning emitted logs from execution.
   *
   * @param tx      The transaction to send.
   * @param signers The signers of the transaction. If unset, the transaction
   *                will be simulated with the "sigVerify: false" option. This
   *                allows for simulation of transactions without asking the
   *                wallet for a signature.
   * @param opts    Transaction confirmation options.
   */
  async simulate(
    tx: Transaction | VersionedTransaction,
    signers?: Signer[],
    commitment?: Commitment,
    includeAccounts?: boolean | PublicKey[]
  ): Promise<SuccessfulTxSimulationResponse> {
    let recentBlockhash = (
      await this.connection.getLatestBlockhash(
        commitment ?? this.connection.commitment
      )
    ).blockhash;

    let result: RpcResponseAndContext<SimulatedTransactionResponse>;
    if (isVersionedTransaction(tx)) {
      if (signers && signers.length > 0) {
        tx.sign(signers);
        tx = await this.wallet.signTransaction(tx);
      }

      // Doesn't support includeAccounts which has been changed to something
      // else in later versions of this function.
      result = await this.connection.simulateTransaction(tx, { commitment });
    } else {
      tx.feePayer = tx.feePayer || this.wallet.publicKey;
      tx.recentBlockhash = recentBlockhash;

      if (signers && signers.length > 0) {
        tx = await this.wallet.signTransaction(tx);
      }
      result = await simulateTransaction(
        this.connection,
        tx,
        signers,
        commitment,
        includeAccounts
      );
    }

    if (result.value.err) {
      throw new SimulateError(result.value);
    }

    return result.value;
  }
}

class SimulateError extends Error {
  constructor(
    readonly simulationResponse: SimulatedTransactionResponse,
    message?: string
  ) {
    super(message);
  }
}

export type SendTxRequest = {
  tx: Transaction;
  signers: Array<Signer | undefined>;
};

export type ConfirmOptionsWithBlockhash = ConfirmOptions & {
  blockhash?: BlockhashWithExpiryBlockHeight;
};

/**
 * Wallet interface for objects that can be used to sign provider transactions.
 * VersionedTransactions sign everything at once
 */
export interface Wallet {
  signTransaction<T extends Transaction | VersionedTransaction>(
    tx: T
  ): Promise<T>;
  signAllTransactions<T extends Transaction | VersionedTransaction>(
    txs: T[]
  ): Promise<T[]>;
  publicKey: PublicKey;
  /** Keypair of the configured payer (Node only) */
  payer?: Keypair;
}

// Copy of Connection.sendAndConfirmRawTransaction that throws
// a better error if 'confirmTransaction` returns an error status
async function sendAndConfirmRawTransaction(
  connection: Connection,
  rawTransaction: Buffer | Uint8Array,
  options?: ConfirmOptionsWithBlockhash
): Promise<TransactionSignature> {
  const sendOptions: SendOptions = options
    ? {
        skipPreflight: options.skipPreflight,
        preflightCommitment: options.preflightCommitment || options.commitment,
        maxRetries: options.maxRetries,
        minContextSlot: options.minContextSlot,
      }
    : {};

  let status: SignatureResult;

  const startTime = Date.now();
  while (Date.now() - startTime < 60_000) {
    try {
      const signature = await connection.sendRawTransaction(
        rawTransaction,
        sendOptions
      );

      if (options?.blockhash) {
        if (sendOptions.maxRetries === 0) {
          const abortSignal = AbortSignal.timeout(15_000);
          status = (
            await connection.confirmTransaction(
              { abortSignal, signature, ...options.blockhash },
              options && options.commitment
            )
          ).value;
        } else {
          status = (
            await connection.confirmTransaction(
              { signature, ...options.blockhash },
              options && options.commitment
            )
          ).value;
        }
      } else {
        status = (
          await connection.confirmTransaction(
            signature,
            options && options.commitment
          )
        ).value;
      }

      if (status.err) {
        throw new ConfirmError(
          `Raw transaction ${signature} failed (${JSON.stringify(status)})`
        );
      }

      return signature;
    } catch (err) {
      if (err.name === "TimeoutError") {
        continue;
      }
      throw err;
    }
  }

  throw Error("Transaction failed to confirm in 60s");
}

class ConfirmError extends Error {
  constructor(message?: string) {
    super(message);
  }
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
