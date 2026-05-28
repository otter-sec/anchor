import {
  Connection,
  Keypair,
  PublicKey,
  SystemProgram,
  Transaction,
} from "@solana/web3.js";

import { AnchorProvider, Wallet } from "../src";

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
  let getLatestBlockhashCalls = 0;

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
    sendRawTransaction: async () => "fake-sig",
    confirmTransaction: async () => ({ value: { err: null } }),
  } as unknown as Connection;

  const wallet: Wallet = {
    publicKey: walletKp.publicKey,
    signTransaction: async (tx: any) => tx,
    // Reject after the sendAll loop has already mutated each tx — lets us
    // inspect the post-mutation state without exercising serialize / RPC.
    signAllTransactions: async () => {
      throw STOP;
    },
    payer: walletKp,
  } as any;

  return {
    provider: new AnchorProvider(connection, wallet),
    getLatestBlockhashCalls: () => getLatestBlockhashCalls,
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
