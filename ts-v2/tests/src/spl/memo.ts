import assert from "assert";
import { splMemoProgram } from "@anchor-lang/spl-memo";
import { Keypair } from "@solana/web3.js";

import { SPL_MEMO_PROGRAM_ID } from "../constants";
import { confirmTx, getProvider, loadKp, test } from "../utils";

export async function memoTests() {
  const provider = await getProvider();
  const program = splMemoProgram({
    provider,
    programId: SPL_MEMO_PROGRAM_ID,
  });
  const kp = await loadKp();

  const msg = "Memo from Anchor";
  let memoTxHash: string;

  async function memo() {
    memoTxHash = await program.methods.addMemo(msg).accounts({}).rpc();
  }

  async function fetchMemoTx() {
    await confirmTx(memoTxHash);
    const tx = await provider.connection.getTransaction(memoTxHash);
    const logs = tx?.meta?.logMessages;
    if (!logs) throw new Error("No transaction logs!");

    const memoLine = logs.find((l) => l.startsWith("Program log: Memo"));
    if (!memoLine) throw new Error("No memo line!");

    assert(memoLine === `Program log: Memo (len ${msg.length}): "${msg}"`);
  }

  async function memoWithSigners() {
    const otherSigner = new Keypair();
    await program.methods
      .addMemo("Memo with signers")
      .remainingAccounts([
        { pubkey: kp.publicKey, isSigner: true, isWritable: false },
        { pubkey: otherSigner.publicKey, isSigner: true, isWritable: false },
      ])
      .signers([otherSigner])
      .rpc();
  }

  await test(memo);
  await test(fetchMemoTx);
  await test(memoWithSigners);
}
