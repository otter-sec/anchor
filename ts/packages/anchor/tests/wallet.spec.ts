import * as assert from "assert";
import { Keypair } from "@solana/web3.js";
import {
  address,
  appendTransactionMessageInstructions,
  assertIsFullySignedTransaction,
  assertIsTransactionWithinSizeLimit,
  Blockhash,
  compileTransaction,
  createKeyPairSignerFromBytes,
  createTransactionMessage,
  isSolanaError,
  pipe,
  setTransactionMessageFeePayer,
  setTransactionMessageLifetimeUsingBlockhash,
  SOLANA_ERROR__KEYS__INVALID_KEY_PAIR_BYTE_LENGTH,
} from "@solana/kit";
import { createWallet } from "../src/wallet";

function testTransaction(feePayer: string) {
  const tx = pipe(
    createTransactionMessage({ version: "legacy" }),
    (m) => setTransactionMessageFeePayer(address(feePayer), m),
    (m) =>
      setTransactionMessageLifetimeUsingBlockhash(
        {
          blockhash: "11111111111111111111111111111111" as Blockhash,
          lastValidBlockHeight: 100n,
        },
        m
      ),
    (m) =>
      appendTransactionMessageInstructions(
        [
          {
            programAddress: address(
              "Memo1UhkJRfHyvLMcVucJwxXeuD728EqVDDwQDxFMNo"
            ),
          },
        ],
        m
      ),
    compileTransaction
  );
  assertIsTransactionWithinSizeLimit(tx);
  return tx;
}

describe("createWallet", () => {
  it("derives the address synchronously from the secret key", () => {
    const keypair = Keypair.generate();
    const signer = createWallet(keypair.secretKey);
    expect(signer.address).toBe(keypair.publicKey.toBase58());
  });

  it("rejects secret keys that are not 64 bytes", () => {
    assert.throws(
      () => createWallet(new Uint8Array(32)),
      (error) =>
        isSolanaError(error, SOLANA_ERROR__KEYS__INVALID_KEY_PAIR_BYTE_LENGTH)
    );
  });

  it("produces the same signatures as Kit's key pair signer", async () => {
    const keypair = Keypair.generate();
    const walletSigner = createWallet(keypair.secretKey);
    const kitSigner = await createKeyPairSignerFromBytes(keypair.secretKey);

    const tx = testTransaction(walletSigner.address);
    const [walletSignatures] = await walletSigner.signTransactions([tx]);
    const [kitSignatures] = await kitSigner.signTransactions([tx]);

    const signature = walletSignatures[walletSigner.address];
    expect(signature).toHaveLength(64);
    expect(signature).toStrictEqual(kitSignatures[kitSigner.address]);
  });

  it("fully signs a transaction it is the only signer of", async () => {
    const keypair = Keypair.generate();
    const signer = createWallet(keypair.secretKey);

    const tx = testTransaction(signer.address);
    const [signatures] = await signer.signTransactions([tx]);
    const signed = {
      ...tx,
      signatures: Object.freeze({ ...tx.signatures, ...signatures }),
    };

    expect(() => assertIsFullySignedTransaction(signed)).not.toThrow();
  });
});
