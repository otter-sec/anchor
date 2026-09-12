import assert from "assert";
import { splBinaryOraclePairProgram } from "@anchor-lang/spl-binary-oracle-pair";
import { splTokenProgram } from "@anchor-lang/spl-token";
import { BN } from "@anchor-lang/core";
import {
  Keypair,
  PublicKey,
  SYSVAR_CLOCK_PUBKEY,
  SYSVAR_RENT_PUBKEY,
} from "@solana/web3.js";

import {
  SPL_BINARY_ORACLE_PAIR_PROGRAM_ID,
  SPL_TOKEN_PROGRAM_ID,
} from "../constants";
import {
  createMint,
  createTokenAccount,
  getProvider,
  loadKp,
  sendAndConfirmTx,
  sleep,
  test,
} from "../utils";

export async function binaryOraclePairTests() {
  const provider = await getProvider();
  const program = splBinaryOraclePairProgram({
    provider,
    programId: SPL_BINARY_ORACLE_PAIR_PROGRAM_ID,
  });
  const tokenProgram = splTokenProgram({
    provider,
    programId: SPL_TOKEN_PROGRAM_ID,
  });
  const kp = await loadKp();

  let mintEndSlot: number;
  let decideEndSlot: number;

  const userTransferAuthorityKp = new Keypair();

  let poolPk: PublicKey;
  let poolAuthorityPk: PublicKey;
  let poolDepositMintPk: PublicKey;
  let poolDepositTokenAccountPk: PublicKey;
  let tokenPassMintPk: PublicKey;
  let tokenFailMintPk: PublicKey;
  let userPoolTokenAccountPk: PublicKey;
  let userTokenPassAccountPk: PublicKey;
  let userTokenFailAccountPk: PublicKey;

  async function initPool() {
    const poolKp = new Keypair();
    poolPk = poolKp.publicKey;
    const createPoolAccountIx = await program.account.pool.createInstruction(
      poolKp
    );
    // Accounts that are expected to be initialized
    poolDepositMintPk = await createMint();

    // Accounts that are expected to be uninitialized
    const depositTokenAccountKp = new Keypair();
    poolDepositTokenAccountPk = depositTokenAccountKp.publicKey;
    const createDepositTokenAccountIx =
      await tokenProgram.account.account.createInstruction(
        depositTokenAccountKp
      );

    const tokenPassMintKp = new Keypair();
    tokenPassMintPk = tokenPassMintKp.publicKey;
    const createTokenPassMintIx =
      await tokenProgram.account.mint.createInstruction(tokenPassMintKp);

    const tokenFailMintKp = new Keypair();
    tokenFailMintPk = tokenFailMintKp.publicKey;
    const createTokenFailMintIx =
      await tokenProgram.account.mint.createInstruction(tokenFailMintKp);

    const [authorityPk, bump] = await PublicKey.findProgramAddress(
      [poolPk.toBuffer()],
      program.programId
    );
    poolAuthorityPk = authorityPk;

    // Get current slot
    const currentSlot = await provider.connection.getSlot();
    mintEndSlot = currentSlot + 10;
    decideEndSlot = currentSlot + 20;

    const initPoolIx = await program.methods
      .initPool(new BN(mintEndSlot), new BN(decideEndSlot), bump)
      .accounts({
        pool: poolPk,
        authority: poolAuthorityPk,
        decider: kp.publicKey,
        depositTokenMint: poolDepositMintPk,
        depositAccount: poolDepositTokenAccountPk,
        tokenPassMint: tokenPassMintPk,
        tokenFailMint: tokenFailMintPk,
        rent: SYSVAR_RENT_PUBKEY,
        tokenProgram: tokenProgram.programId,
      })
      .instruction();

    await sendAndConfirmTx(
      [
        createPoolAccountIx,
        createDepositTokenAccountIx,
        createTokenPassMintIx,
        createTokenFailMintIx,
        initPoolIx,
      ],
      [kp, poolKp, depositTokenAccountKp, tokenPassMintKp, tokenFailMintKp]
    );
  }

  async function deposit() {
    userPoolTokenAccountPk = await createTokenAccount(poolDepositMintPk);
    userTokenPassAccountPk = await createTokenAccount(tokenPassMintPk);
    userTokenFailAccountPk = await createTokenAccount(tokenFailMintPk);

    const depositAmount = 100;

    const mintIx = await tokenProgram.methods
      .mintTo(new BN(depositAmount))
      .accounts({
        account: userPoolTokenAccountPk,
        mint: poolDepositMintPk,
        owner: kp.publicKey,
      })
      .instruction();
    const approveIx = await tokenProgram.methods
      .approve(new BN(depositAmount))
      .accounts({
        delegate: userTransferAuthorityKp.publicKey,
        owner: kp.publicKey,
        source: userPoolTokenAccountPk,
      })
      .instruction();
    const depositIx = await program.methods
      .deposit(new BN(depositAmount))
      .accounts({
        pool: poolPk,
        authority: poolAuthorityPk,
        userTransferAuthority: userTransferAuthorityKp.publicKey,
        userTokenAccount: userPoolTokenAccountPk,
        poolDepositTokenAccount: poolDepositTokenAccountPk,
        tokenPassMint: tokenPassMintPk,
        tokenFailMint: tokenFailMintPk,
        tokenPassDestinationAccount: userTokenPassAccountPk,
        tokenFailDestinationAccount: userTokenFailAccountPk,
        clock: SYSVAR_CLOCK_PUBKEY,
        tokenProgram: tokenProgram.programId,
      })
      .instruction();

    await sendAndConfirmTx(
      [mintIx, approveIx, depositIx],
      [kp, userTransferAuthorityKp]
    );
  }

  async function withdraw() {
    const withdrawAmount = 50;

    const approveUserPassAccountIx = await tokenProgram.methods
      .approve(new BN(withdrawAmount))
      .accounts({
        delegate: userTransferAuthorityKp.publicKey,
        owner: kp.publicKey,
        source: userTokenPassAccountPk,
      })
      .instruction();
    const approveUserFailAccountIx = await tokenProgram.methods
      .approve(new BN(withdrawAmount))
      .accounts({
        delegate: userTransferAuthorityKp.publicKey,
        owner: kp.publicKey,
        source: userTokenFailAccountPk,
      })
      .instruction();

    const withdrawIx = await program.methods
      .withdraw(new BN(withdrawAmount))
      .accounts({
        pool: poolPk,
        authority: poolAuthorityPk,
        userTransferAuthority: userTransferAuthorityKp.publicKey,
        poolDepositTokenAccount: poolDepositTokenAccountPk,
        tokenPassUserAccount: userTokenPassAccountPk,
        tokenFailUserAccount: userTokenFailAccountPk,
        tokenPassMint: tokenPassMintPk,
        tokenFailMint: tokenFailMintPk,
        userTokenDestinationAccount: userPoolTokenAccountPk,
        clock: SYSVAR_CLOCK_PUBKEY,
        tokenProgram: tokenProgram.programId,
      })
      .instruction();

    await sendAndConfirmTx(
      [approveUserPassAccountIx, approveUserFailAccountIx, withdrawIx],
      [kp, userTransferAuthorityKp]
    );
  }

  async function decide() {
    // Call only succeeds once and if current slot > mint_end slot AND < decide_end slot
    const currentSlot = await provider.connection.getSlot();
    if (currentSlot > mintEndSlot) {
      if (currentSlot < decideEndSlot) {
        await program.methods
          .decide(true)
          .accounts({
            pool: poolPk,
            decider: kp.publicKey,
            clock: SYSVAR_CLOCK_PUBKEY,
          })
          .rpc();
      }
    } else {
      await sleep();
      await decide();
    }
  }

  async function fetchPool() {
    const pool = await program.account.pool.fetch(poolPk);
    assert(pool.decider.equals(kp.publicKey));
    assert(pool.tokenPassMint.equals(tokenPassMintPk));
    assert(pool.tokenFailMint.equals(tokenFailMintPk));
    assert(pool.tokenProgramId.equals(tokenProgram.programId));
    assert(pool.mintEndSlot.toNumber() === mintEndSlot);
    assert(pool.decideEndSlot.toNumber() === decideEndSlot);
  }

  await test(initPool);
  await test(deposit);
  await test(withdraw);
  await test(decide);
  await test(fetchPool);
}
