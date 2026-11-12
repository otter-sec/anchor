import {
  Connection,
  Keypair,
  PublicKey,
  SystemProgram,
  Transaction,
  TransactionInstruction,
  VersionedTransaction,
} from "@solana/web3.js";

import { AnchorProvider, Wallet as NodeWallet } from "../src";
import type { Wallet } from "../src/provider";
import {
  deserializeTransaction as deserializeV1Transaction,
  toTransaction as toV1Transaction,
  type V1TransactionConfig,
} from "../src/utils/v1";

// `AnchorProvider.sendAll` and `AnchorProvider.simulate` used to unconditionally
// overwrite `tx.recentBlockhash` with a freshly-fetched value, which clobbered
// caller-provided blockhashes (notably durable nonces). Both call sites now
// only fetch / assign when the field is missing or set to the all-zeros
// sentinel — matching the existing behavior of `sendAndConfirm`.

const PRESET_BLOCKHASH = "GfVcyD5tT6Aua4yzGiVy5gM2y2qXmZRr5GnMaCu2FQVc";
const FETCHED_BLOCKHASH = "EkSnNWid2cvwEVnVx9aBqawnmiCNiDgp3gUdkDPTKN1N";
const STOP = new Error("__stop_after_blockhash_assignment__");

function makeFixture() {
  const walletKp = Keypair.generate();
  const nodeWallet = new NodeWallet(walletKp);
  let getLatestBlockhashCalls = 0;
  let rawTransaction: Uint8Array | undefined;
  let simulatedTransaction: VersionedTransaction | undefined;

  const connection = {
    commitment: "processed",
    getLatestBlockhash: async () => {
      getLatestBlockhashCalls++;
      return { blockhash: FETCHED_BLOCKHASH, lastValidBlockHeight: 0 };
    },
    // Anchor's local `simulateTransaction` helper calls `_rpcRequest` after
    // the provider has already assigned the blockhash. Reject to halt the
    // flow before serialization / network access.
    _rpcRequest: async () => {
      throw STOP;
    },
    sendRawTransaction: async (raw: Uint8Array) => {
      rawTransaction = raw;
      return "fake-sig";
    },
    confirmTransaction: async () => ({ value: { err: null } }),
    simulateTransaction: async (tx: VersionedTransaction) => {
      simulatedTransaction = tx;
      return { context: { slot: 1 }, value: { err: null, logs: [] } };
    },
  } as unknown as Connection;

  const wallet: Wallet = {
    publicKey: walletKp.publicKey,
    signTransaction: jest.fn(async (tx) => nodeWallet.signTransaction(tx)),
    // Reject after the sendAll loop has already mutated each tx — lets us
    // inspect the post-mutation state without exercising serialize / RPC.
    signAllTransactions: async () => {
      throw STOP;
    },
  };

  return {
    provider: new AnchorProvider(connection, wallet),
    getLatestBlockhashCalls: () => getLatestBlockhashCalls,
    rawTransaction: () => rawTransaction,
    simulatedTransaction: () => simulatedTransaction,
    payer: walletKp,
    wallet,
  };
}

function legacyTx(blockhash?: string): Transaction {
  const tx = new Transaction();
  tx.add(
    SystemProgram.transfer({
      fromPubkey: PublicKey.default,
      toPubkey: PublicKey.default,
      lamports: 1,
    })
  );
  if (blockhash) tx.recentBlockhash = blockhash;
  return tx;
}

describe("AnchorProvider blockhash handling (#3375)", () => {
  describe("transaction-v1", () => {
    const v1Config: V1TransactionConfig = {
      priorityFeeLamports: BigInt(5_000),
      computeUnitLimit: 300_000,
      loadedAccountsDataSizeLimit: 65_536,
      heapSize: 32_768,
    };

    it("compiles legacy Anchor instructions into a v1 transaction", () => {
      const tx = legacyTx(PRESET_BLOCKHASH);
      const v1 = toV1Transaction(tx, {
        payerKey: PublicKey.default,
        recentBlockhash: PRESET_BLOCKHASH,
        transactionConfig: v1Config,
      });

      expect(v1.version).toBe(1);
      expect(v1.message.transactionConfig).toEqual(v1Config);
      expect(v1.message.compiledInstructions).toHaveLength(1);
    });

    it("signs and sends a v1 transaction with the provider payer", async () => {
      const { provider, getLatestBlockhashCalls, rawTransaction } =
        makeFixture();

      await expect(provider.sendV1(legacyTx(), v1Config)).resolves.toBe(
        "fake-sig"
      );

      expect(getLatestBlockhashCalls()).toBe(1);
      const raw = rawTransaction();
      expect(raw).toBeDefined();
      if (!raw) throw new Error("Expected a serialized transaction");
      expect(VersionedTransaction.deserialize(raw).version).toBe(1);
      expect(deserializeV1Transaction(raw).version).toBe(1);
    });

    it.each(["sendV1", "simulateV1"] as const)(
      "%s signs through the wallet interface and preserves extra signatures",
      async (method) => {
        const {
          provider,
          wallet,
          payer,
          rawTransaction,
          simulatedTransaction,
        } = makeFixture();
        expect(wallet.payer).toBeUndefined();
        wallet.signTransaction = jest.fn(async (tx) => {
          expect(tx).toBeInstanceOf(VersionedTransaction);
          const versioned = tx as VersionedTransaction;
          expect(versioned.version).toBe(1);
          expect(
            versioned.message.staticAccountKeys[0].equals(wallet.publicKey)
          ).toBe(true);
          expect(versioned.serialize()[0]).toBe(0x81);
          versioned.sign([payer]);
          // Wallet adapters may return a new transaction rather than mutate the input.
          return VersionedTransaction.deserialize(
            versioned.serialize()
          ) as typeof tx;
        });
        const extraSigner = Keypair.generate();
        const tx = new Transaction().add(
          new TransactionInstruction({
            programId: SystemProgram.programId,
            keys: [
              {
                pubkey: extraSigner.publicKey,
                isSigner: true,
                isWritable: false,
              },
            ],
            data: Buffer.alloc(0),
          })
        );

        await provider[method](tx, v1Config, [extraSigner]);
        expect(wallet.signTransaction).toHaveBeenCalledTimes(1);

        const raw =
          method === "sendV1"
            ? rawTransaction()
            : simulatedTransaction()?.serialize();
        if (!raw) throw new Error("Expected a serialized transaction");
        const transaction = VersionedTransaction.deserialize(raw);
        expect(transaction.signatures).toHaveLength(2);
        const messageBytes = raw.subarray(
          0,
          raw.length - transaction.signatures.length * 64
        );
        for (const [index, publicKey] of transaction.message.staticAccountKeys
          .slice(0, 2)
          .entries()) {
          const key = await crypto.subtle.importKey(
            "raw",
            publicKey.toBytes(),
            "Ed25519",
            false,
            ["verify"]
          );
          expect(
            await crypto.subtle.verify(
              "Ed25519",
              key,
              transaction.signatures[index],
              messageBytes
            )
          ).toBe(true);
        }
      }
    );

    it.each(["sendV1", "simulateV1"] as const)(
      "%s propagates wallet rejection before submitting the transaction",
      async (method) => {
        const { provider, wallet, rawTransaction, simulatedTransaction } =
          makeFixture();
        const rejection = new Error("Wallet rejected signing");
        wallet.signTransaction = jest.fn().mockRejectedValue(rejection);
        await expect(provider[method](legacyTx(), v1Config)).rejects.toBe(
          rejection
        );
        expect(rawTransaction()).toBeUndefined();
        expect(simulatedTransaction()).toBeUndefined();
      }
    );

    it("uses a durable nonce without fetching a recent blockhash", async () => {
      const { provider, getLatestBlockhashCalls, rawTransaction, payer } =
        makeFixture();
      const nonceInstruction = SystemProgram.nonceAdvance({
        noncePubkey: Keypair.generate().publicKey,
        authorizedPubkey: payer.publicKey,
      });
      const tx = legacyTx();
      tx.nonceInfo = { nonce: PRESET_BLOCKHASH, nonceInstruction };

      await expect(provider.sendV1(tx, v1Config)).resolves.toBe("fake-sig");

      expect(getLatestBlockhashCalls()).toBe(0);
      const raw = rawTransaction();
      if (!raw) throw new Error("Expected a serialized transaction");
      const transaction = VersionedTransaction.deserialize(raw);
      expect(transaction.message.recentBlockhash).toBe(PRESET_BLOCKHASH);
      expect(transaction.message.compiledInstructions).toHaveLength(2);
    });

    it("simulates a signed v1 transaction", async () => {
      const { provider, simulatedTransaction } = makeFixture();

      await expect(provider.simulateV1(legacyTx(), v1Config)).resolves.toEqual({
        err: null,
        logs: [],
      });
      expect(simulatedTransaction()?.version).toBe(1);
    });
  });

  describe("sendAll", () => {
    it("preserves a caller-provided recentBlockhash", async () => {
      const { provider, getLatestBlockhashCalls } = makeFixture();
      const tx = legacyTx(PRESET_BLOCKHASH);

      await expect(provider.sendAll([{ tx }])).rejects.toBe(STOP);

      expect(tx.recentBlockhash).toBe(PRESET_BLOCKHASH);
      expect(getLatestBlockhashCalls()).toBe(0);
    });

    it("fetches and assigns only for txs missing a blockhash", async () => {
      const { provider, getLatestBlockhashCalls } = makeFixture();
      const txWith = legacyTx(PRESET_BLOCKHASH);
      const txWithout = legacyTx();

      await expect(
        provider.sendAll([{ tx: txWith }, { tx: txWithout }])
      ).rejects.toBe(STOP);

      expect(txWith.recentBlockhash).toBe(PRESET_BLOCKHASH);
      expect(txWithout.recentBlockhash).toBe(FETCHED_BLOCKHASH);
      expect(getLatestBlockhashCalls()).toBe(1);
    });

    it("reuses a single fetched blockhash across multiple missing txs", async () => {
      const { provider, getLatestBlockhashCalls } = makeFixture();
      const a = legacyTx();
      const b = legacyTx();

      await expect(provider.sendAll([{ tx: a }, { tx: b }])).rejects.toBe(STOP);

      expect(a.recentBlockhash).toBe(FETCHED_BLOCKHASH);
      expect(b.recentBlockhash).toBe(FETCHED_BLOCKHASH);
      expect(getLatestBlockhashCalls()).toBe(1);
    });

    it("treats the all-zeros sentinel as missing", async () => {
      const { provider, getLatestBlockhashCalls } = makeFixture();
      const tx = legacyTx("11111111111111111111111111111111");

      await expect(provider.sendAll([{ tx }])).rejects.toBe(STOP);

      expect(tx.recentBlockhash).toBe(FETCHED_BLOCKHASH);
      expect(getLatestBlockhashCalls()).toBe(1);
    });
  });

  describe("simulate", () => {
    it("preserves a caller-provided recentBlockhash", async () => {
      const { provider, getLatestBlockhashCalls } = makeFixture();
      const tx = legacyTx(PRESET_BLOCKHASH);

      await expect(provider.simulate(tx)).rejects.toBe(STOP);

      expect(tx.recentBlockhash).toBe(PRESET_BLOCKHASH);
      expect(getLatestBlockhashCalls()).toBe(0);
    });

    it("fetches a blockhash only when missing", async () => {
      const { provider, getLatestBlockhashCalls } = makeFixture();
      const tx = legacyTx();

      await expect(provider.simulate(tx)).rejects.toBe(STOP);

      expect(tx.recentBlockhash).toBe(FETCHED_BLOCKHASH);
      expect(getLatestBlockhashCalls()).toBe(1);
    });
  });
});
