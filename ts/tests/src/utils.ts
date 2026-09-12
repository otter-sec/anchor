import * as fs from "fs/promises";
import { splTokenProgram } from "@anchor-lang/spl-token";
import { splAssociatedTokenAccountProgram } from "@anchor-lang/spl-associated-token-account";
import {
  Connection,
  Keypair,
  LAMPORTS_PER_SOL,
  PublicKey,
  Signer,
  SystemProgram,
  SYSVAR_RENT_PUBKEY,
  Transaction,
  TransactionInstruction,
} from "@solana/web3.js";
import { AnchorProvider, Wallet } from "@anchor-lang/core";

import { SPL_ATA_PROGRAM_ID, SPL_TOKEN_PROGRAM_ID } from "./constants";

const KEYPAIR_PATH = "test-keypair.json";

let totalTestCount = 0;
let totalErrorCount = 0;
export async function mainTest(cb: () => Promise<void>) {
  info(`Running tests...`);

  await cb();

  if (totalErrorCount) {
    error(
      `${totalErrorCount}/${totalTestCount} test${
        totalErrorCount > 1 ? "s" : ""
      } failed.`
    );
  } else {
    success(`All tests passed.(${totalTestCount})`);
  }
}

let testCount = 0;
let errorCount = 0;
export async function programTest(cb: () => Promise<void>) {
  const tab = "   ";
  info(`${tab}Running '${cb.name}'...`);

  await cb();

  if (errorCount) {
    error(
      `${tab}${errorCount}/${testCount} test${errorCount > 1 ? "s" : ""} in '${
        cb.name
      }' failed.`
    );
    totalErrorCount += errorCount;
  } else {
    success(`${tab}All '${cb.name}' tests passed.`);
  }

  totalTestCount += testCount;

  testCount = 0;
  errorCount = 0;
}

export async function test(cb: () => Promise<void>) {
  const tab = "      ";
  console.log(`${tab}Running test '\x1b[1m${cb.name}\x1b[0m'`);

  try {
    await cb();
    success(`${tab}Test '${cb.name}' passed.`);
  } catch (e) {
    error(`${tab}Test '${cb.name}' failed. Reason: ${e}`);
    errorCount++;
  } finally {
    testCount++;
  }
}

export async function loadKp() {
  try {
    const kpBytes = await fs.readFile(KEYPAIR_PATH);
    const kp = Keypair.fromSecretKey(
      Uint8Array.from(JSON.parse(kpBytes.toString()))
    );

    return kp;
  } catch {
    info("Creating test keypair file...");
    const randomKp = new Keypair();
    await fs.writeFile(
      KEYPAIR_PATH,
      JSON.stringify(Array.from(randomKp.secretKey))
    );
    return randomKp;
  }
}

let hasBalance = false;
export async function getProvider() {
  const kp = await loadKp();
  const ENDPOINT = "http://localhost:8899";
  // const ENDPOINT = "https://api.devnet.solana.com";
  const conn = new Connection(ENDPOINT, {
    commitment: "confirmed",
  });
  const wallet = new Wallet(kp);

  const provider = new AnchorProvider(
    conn,
    wallet,
    AnchorProvider.defaultOptions()
  );

  if (!hasBalance && !(await provider.connection.getBalance(kp.publicKey))) {
    const txHash = await provider.connection.requestAirdrop(
      kp.publicKey,
      1000 * LAMPORTS_PER_SOL
    );
    await confirmTx(txHash);
    hasBalance = true;
  }

  return provider;
}

export async function sleep(ms: number = 500) {
  return new Promise((res) => setTimeout((s) => res(s), ms));
}

export async function confirmTx(txHash: string) {
  const provider = await getProvider();
  const blockhashInfo = await provider.connection.getLatestBlockhash();
  await provider.connection.confirmTransaction({
    blockhash: blockhashInfo.blockhash,
    lastValidBlockHeight: blockhashInfo.lastValidBlockHeight,
    signature: txHash,
  });
}

export async function sendAndConfirmTx(
  ixs: TransactionInstruction[],
  signers: Signer[]
) {
  const provider = await getProvider();
  const blockhashInfo = await provider.connection.getLatestBlockhash();
  const tx = new Transaction().add(...ixs);
  tx.feePayer = provider.publicKey;
  tx.recentBlockhash = blockhashInfo.blockhash;
  tx.sign(...signers);
  const txHash = await provider.connection.sendRawTransaction(tx.serialize());
  await confirmTx(txHash);

  return txHash;
}

export async function simulateTx(
  ixs: TransactionInstruction[],
  signers: Signer[]
) {
  const provider = await getProvider();
  const blockhashInfo = await provider.connection.getLatestBlockhash();
  const tx = new Transaction().add(...ixs);
  tx.feePayer = provider.publicKey;
  tx.recentBlockhash = blockhashInfo.blockhash;
  tx.sign(...signers);
  const simulationResult = await provider.connection.simulateTransaction(tx);

  return simulationResult;
}

export async function createMint(ownerPk?: PublicKey) {
  const provider = await getProvider();
  const kp = await loadKp();
  if (!ownerPk) ownerPk = kp.publicKey;
  const tokenProgram = splTokenProgram({
    provider,
    programId: SPL_TOKEN_PROGRAM_ID,
  });
  const mintKp = new Keypair();
  const createMintAccountIx = await tokenProgram.account.mint.createInstruction(
    mintKp
  );
  const initMintIx = await tokenProgram.methods
    .initializeMint(6, ownerPk, null)
    .accounts({
      mint: mintKp.publicKey,
      rent: SYSVAR_RENT_PUBKEY,
    })
    .instruction();

  await sendAndConfirmTx([createMintAccountIx, initMintIx], [kp, mintKp]);

  return mintKp.publicKey;
}

export async function createTokenAccount(
  mintPk: PublicKey,
  programId: PublicKey = SPL_TOKEN_PROGRAM_ID
) {
  const provider = await getProvider();
  const kp = await loadKp();
  const tokenProgram = splTokenProgram({
    provider,
    programId,
  });

  const accountKp = new Keypair();
  const createTokenAccountIx =
    await tokenProgram.account.account.createInstruction(accountKp);
  const initAccountIx = await tokenProgram.methods
    .initializeAccount()
    .accounts({
      account: accountKp.publicKey,
      mint: mintPk,
      owner: kp.publicKey,
      rent: SYSVAR_RENT_PUBKEY,
    })
    .instruction();

  await sendAndConfirmTx(
    [createTokenAccountIx, initAccountIx],
    [kp, accountKp]
  );

  return accountKp.publicKey;
}

export async function getAta(mintPk: PublicKey, ownerPk: PublicKey) {
  return (
    await PublicKey.findProgramAddress(
      [ownerPk.toBuffer(), SPL_TOKEN_PROGRAM_ID.toBuffer(), mintPk.toBuffer()],
      SPL_ATA_PROGRAM_ID
    )
  )[0];
}

export async function createAta(mintPk: PublicKey, ownerPk: PublicKey) {
  const provider = await getProvider();
  const ataPk = await getAta(mintPk, ownerPk);

  if (!(await provider.connection.getAccountInfo(ataPk))) {
    const ataProgram = splAssociatedTokenAccountProgram({
      provider,
      programId: SPL_ATA_PROGRAM_ID,
    });
    await ataProgram.methods
      .create()
      .accounts({
        associatedAccountAddress: ataPk,
        fundingAddress: provider.publicKey,
        systemProgram: SystemProgram.programId,
        tokenMintAddress: mintPk,
        tokenProgram: SPL_TOKEN_PROGRAM_ID,
        walletAddress: ownerPk,
      })
      .rpc();
  }

  return ataPk;
}

const info = (s: string) => {
  console.log(`\x1b[1;36m${s}\x1b[0m`);
};

const success = (s: string) => {
  console.log(`\x1b[1;32m${s}\x1b[0m`);
};

const error = (s: string) => {
  console.log(`\x1b[1;31m${s}\x1b[0m`);
};
