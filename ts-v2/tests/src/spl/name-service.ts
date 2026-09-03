import { createHash } from "crypto";
import { splNameServiceProgram } from "@anchor-lang/spl-name-service";
import { BN } from "@anchor-lang/core";

import { SPL_NAME_SERVICE_PROGRAM_ID } from "../constants";
import { confirmTx, getProvider, loadKp, test } from "../utils";
import { Keypair, PublicKey, SystemProgram } from "@solana/web3.js";

export async function nameServiceTests() {
  const provider = await getProvider();
  const program = splNameServiceProgram({
    provider,
    programId: SPL_NAME_SERVICE_PROGRAM_ID,
  });
  const kp = await loadKp();

  const getHashedName = (s: string) => {
    return createHash("sha256")
      .update(PREFIX + s, "utf8")
      .digest();
  };

  const PREFIX = "SPL Name Service";
  const NAME = "anchor" + Math.floor(Math.random() * 1000);
  const UPDATE_NAME = "acheron";

  const newOwnerKp = new Keypair();
  let nameAccountPk: PublicKey;

  async function create() {
    const HASHED_NAME = getHashedName(NAME);
    const seeds = [HASHED_NAME, Buffer.alloc(32), Buffer.alloc(32)];
    [nameAccountPk] = await PublicKey.findProgramAddress(
      seeds,
      program.programId
    );

    const nameAccountTotalSpace = HASHED_NAME.byteLength * 10;
    const nameAccountLamports =
      await provider.connection.getMinimumBalanceForRentExemption(
        nameAccountTotalSpace
      );

    await program.methods
      .create(
        HASHED_NAME,
        new BN(nameAccountLamports),
        nameAccountTotalSpace - program.account.nameRecordHeader.size
      )
      .accounts({
        systemProgram: SystemProgram.programId,
        payer: kp.publicKey,
        nameAccount: nameAccountPk,
        nameOwner: kp.publicKey,
      })
      .remainingAccounts([
        {
          pubkey: PublicKey.default,
          isSigner: false,
          isWritable: false,
        },
        {
          pubkey: PublicKey.default,
          isSigner: false,
          isWritable: false,
        },
      ])
      .rpc();
  }

  async function update() {
    const HASHED_UPDATE_NAME = getHashedName(UPDATE_NAME);
    await program.methods
      .update(0, HASHED_UPDATE_NAME)
      .accounts({
        nameAccount: nameAccountPk,
        nameUpdateSigner: kp.publicKey,
      })
      .rpc();
  }

  async function transfer() {
    await program.methods
      .transfer(newOwnerKp.publicKey)
      .accounts({
        nameAccount: nameAccountPk,
        nameOwner: kp.publicKey,
      })
      .rpc();
  }

  async function del() {
    const txHash = await program.methods
      .delete()
      .accounts({
        nameAccount: nameAccountPk,
        nameOwner: newOwnerKp.publicKey,
        refundTarget: kp.publicKey,
      })
      .signers([newOwnerKp])
      .rpc();

    await confirmTx(txHash);
  }

  async function fetchNameRecord() {
    try {
      await program.account.nameRecordHeader.fetch(nameAccountPk);
      throw new Error("Account should not exist.");
    } catch {}
  }

  await test(create);
  await test(update);
  await test(transfer);
  await test(del);
  await test(fetchNameRecord);
}
