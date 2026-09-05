import {
  Keypair as V1Keypair,
  PublicKey as V1Web3PublicKey,
  TransactionMessage as V1TransactionMessage,
  VersionedTransaction as V1VersionedTransaction,
} from "@anchor-lang/solana-web3-v3";
import { PublicKey, Signer, Transaction } from "@solana/web3.js";

/** Message-level resource limits and priority fee for transaction-v1. */
export type V1TransactionConfig = {
  computeUnitLimit?: number;
  heapSize?: number;
  loadedAccountsDataSizeLimit?: number;
  priorityFeeLamports?: bigint;
};

/** A public key in a decoded transaction-v1 message. */
export type V1PublicKey = { toBase58(): string };

/** The portion of a decoded transaction-v1 message exposed by Anchor. */
export type V1Message = {
  compiledInstructions: readonly unknown[];
  staticAccountKeys: readonly V1PublicKey[];
  transactionConfig?: V1TransactionConfig;
};

/** A signed or unsigned transaction-v1 wire message. */
export type V1Transaction = {
  readonly message: V1Message;
  readonly version: 1;
  serialize(): Uint8Array;
};

/** A transaction-v1 signer from the Solana web3 rc3 client. */
export type V1Signer = object;

/** The inputs required to compile an existing Anchor transaction as v1. */
export type TransactionV1Options = {
  /** Fee payer for the v1 message. */
  payerKey: PublicKey;
  /** Recent blockhash or durable nonce value for the v1 message. */
  recentBlockhash: string;
  /** Message-level resource limits and priority fee. */
  transactionConfig: V1TransactionConfig;
};

/**
 * Compiles legacy Anchor instructions into a Solana transaction-v1 message.
 *
 * Anchor continues to expose the stable web3.js legacy API. This conversion is
 * intentionally isolated because transaction-v1 is currently available only
 * from the Solana web3 rc3 client.
 */
export function toTransaction(
  transaction: Transaction,
  options: TransactionV1Options
): V1Transaction {
  const message = new V1TransactionMessage({
    payerKey: new V1Web3PublicKey(options.payerKey.toBase58()) as never,
    recentBlockhash: options.recentBlockhash as never,
    instructions: transaction.instructions.map((instruction) => ({
      programId: new V1Web3PublicKey(instruction.programId.toBase58()),
      keys: instruction.keys.map((key) => ({
        ...key,
        pubkey: new V1Web3PublicKey(key.pubkey.toBase58()),
      })),
      data: Uint8Array.from(instruction.data),
    })) as never,
  }).compileToV1Message(options.transactionConfig as never);

  return new V1VersionedTransaction(message) as unknown as V1Transaction;
}

/**
 * Converts a legacy Keypair signer into the signer type required by a v1
 * transaction. This is asynchronous because the v1 signer uses WebCrypto.
 */
export async function signerFromLegacyKeypair(
  signer: Signer
): Promise<V1Signer> {
  return await V1Keypair.fromSecretKey(signer.secretKey);
}

/** Sign a v1 transaction with transaction-v1 signers. */
export async function signTransaction(
  transaction: V1Transaction,
  signers: V1Signer[]
): Promise<void> {
  await (transaction as unknown as V1VersionedTransaction).sign(
    signers as never
  );
}

/** Deserialize a wire-format Solana transaction-v1 transaction. */
export function deserializeTransaction(
  serializedTransaction: Uint8Array
): V1Transaction {
  const transaction = V1VersionedTransaction.deserialize(serializedTransaction);
  if (transaction.version !== 1) {
    throw new Error(
      `Expected a transaction-v1 message, got v${transaction.version}`
    );
  }
  return transaction as unknown as V1Transaction;
}
