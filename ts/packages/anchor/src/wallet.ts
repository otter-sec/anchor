import { Buffer } from "buffer";
import {
  createLazyKeyPairSignerFromBytes,
  ReadonlyUint8Array,
  TransactionPartialSigner,
} from "@solana/kit";
import { isBrowser } from "./utils/common.js";

/**
 * Creates a {@link TransactionSigner} from a 64-byte ed25519 secret key: a
 * 32-byte private key seed followed by the 32-byte public key, as stored in
 * Solana keypair files and in web3.js `Keypair.secretKey`.
 *
 * This is Kit's `createLazyKeyPairSignerFromBytes`: the address is derived
 * directly from the public key half of the secret key and the WebCrypto key
 * import is deferred until the first signature is requested, so the wallet
 * is created synchronously. `AnchorProvider.local()`/`env()` and the
 * `getProvider()` fallback rely on this (e.g. `setProvider(AnchorProvider.env())`
 * at the top of CommonJS test files, where no top-level await is available).
 */
export function createWallet(
  secretKey: ReadonlyUint8Array
): TransactionPartialSigner {
  return createLazyKeyPairSignerFromBytes(secretKey);
}

/**
 * Creates a {@link TransactionSigner} from the keypair file at the path in
 * the `ANCHOR_WALLET` environment variable.
 *
 * (This API is for Node only.)
 */
export function createLocalWallet(): TransactionPartialSigner {
  if (isBrowser) {
    throw new Error("Local wallet is not available in the browser.");
  }

  const process = require("process");
  if (!process.env.ANCHOR_WALLET || process.env.ANCHOR_WALLET === "") {
    throw new Error(
      "expected environment variable `ANCHOR_WALLET` is not set."
    );
  }

  return createWallet(
    Buffer.from(
      JSON.parse(
        require("fs").readFileSync(process.env.ANCHOR_WALLET, {
          encoding: "utf-8",
        })
      )
    )
  );
}
