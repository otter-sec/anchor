import assert from "assert";
import { splRecordProgram } from "@anchor-lang/spl-record";
import { Keypair, PublicKey } from "@solana/web3.js";
import { BN } from "@anchor-lang/core";

import { SPL_RECORD_PROGRAM_ID } from "../constants";
import {
  confirmTx,
  getProvider,
  loadKp,
  sendAndConfirmTx,
  test,
} from "../utils";

export async function recordTests() {
  const provider = await getProvider();
  const program = splRecordProgram({
    provider,
    programId: SPL_RECORD_PROGRAM_ID,
  });
  const kp = await loadKp();

  const RECORD_DATA = Buffer.alloc(8).fill(1);
  const newAuthorityKp = new Keypair();
  let recordPk: PublicKey;

  async function initialize() {
    const recordKp = new Keypair();
    recordPk = recordKp.publicKey;
    const createRecordAccountIx =
      await program.account.recordData.createInstruction(recordKp);
    const initIx = await program.methods
      .initialize()
      .accounts({
        recordAccount: recordKp.publicKey,
        authority: kp.publicKey,
      })
      .instruction();

    await sendAndConfirmTx([createRecordAccountIx, initIx], [kp, recordKp]);
  }

  async function write() {
    await program.methods
      .write(new BN(0), RECORD_DATA)
      .accounts({
        recordAccount: recordPk,
        signer: kp.publicKey,
      })
      .rpc();
  }

  async function setAuthority() {
    await program.methods
      .setAuthority()
      .accounts({
        recordAccount: recordPk,
        signer: kp.publicKey,
        newAuthority: newAuthorityKp.publicKey,
      })
      .rpc();

    try {
      await write();
      throw new Error("Authority did not update.");
    } catch {}
  }

  async function fetchRecordDataAccount() {
    const record = await program.account.recordData.fetch(recordPk);
    assert(record.authority.equals(newAuthorityKp.publicKey));
    assert(record.data.bytes.every((b, i) => b === RECORD_DATA[i]));
  }

  async function closeAccount() {
    const txHash = await program.methods
      .closeAccount()
      .accounts({
        recordAccount: recordPk,
        signer: newAuthorityKp.publicKey,
        receiver: kp.publicKey,
      })
      .signers([newAuthorityKp])
      .rpc();

    await confirmTx(txHash);

    try {
      await program.account.recordData.fetch(recordPk);
      throw new Error("Record account did not close.");
    } catch {}
  }

  await test(initialize);
  await test(write);
  await test(setAuthority);
  await test(fetchRecordDataAccount);
  await test(closeAccount);
}
