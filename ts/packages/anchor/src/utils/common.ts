import { isSolanaError, SolanaError, SolanaErrorCode } from "@solana/kit";
import { Transaction, VersionedTransaction } from "@solana/web3.js";

/**
 * Returns true if being run inside a web browser,
 * false if in a Node process or electron app.
 */
export const isBrowser =
  process.env.ANCHOR_BROWSER ||
  (typeof window !== "undefined" && !window.process?.hasOwnProperty("type"));

/**
 * Splits an array into chunks
 *
 * @param array Array of objects to chunk.
 * @param size The max size of a chunk.
 * @returns A two dimensional array where each T[] length is < the provided size.
 */
export function chunks<T>(array: T[], size: number): T[][] {
  return Array.apply(0, new Array(Math.ceil(array.length / size))).map(
    (_, index) => array.slice(index * size, (index + 1) * size)
  );
}

/**
 * Check if a transaction object is a VersionedTransaction or not
 *
 * @param tx
 * @returns bool
 */
export const isVersionedTransaction = (
  tx: Transaction | VersionedTransaction
): tx is VersionedTransaction => {
  return "version" in tx;
};

/**
 * Finds a Kit `SolanaError` with the given code in the cause chain of the
 * given error, including the error itself.
 */
export function findSolanaError<TCode extends SolanaErrorCode>(
  err: unknown,
  code: TCode
): SolanaError<TCode> | undefined {
  for (let cause: unknown = err; cause instanceof Error; cause = cause.cause) {
    if (isSolanaError(cause, code)) {
      return cause;
    }
  }
  return undefined;
}
