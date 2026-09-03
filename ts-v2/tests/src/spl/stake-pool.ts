import assert from "assert";
import { splStakePoolProgram } from "@anchor-lang/spl-stake-pool";
import { splTokenProgram } from "@anchor-lang/spl-token";
import { BN } from "@anchor-lang/core";
import {
  Keypair,
  LAMPORTS_PER_SOL,
  PublicKey,
  StakeProgram,
  STAKE_CONFIG_ID,
  SystemProgram,
  SYSVAR_CLOCK_PUBKEY,
  SYSVAR_RENT_PUBKEY,
  SYSVAR_STAKE_HISTORY_PUBKEY,
  VoteProgram,
} from "@solana/web3.js";

import { SPL_STAKE_POOL_PROGRAM_ID, SPL_TOKEN_PROGRAM_ID } from "../constants";
import {
  createAta,
  createMint,
  createTokenAccount,
  getProvider,
  loadKp,
  sendAndConfirmTx,
  simulateTx,
  test,
} from "../utils";

export async function stakePoolTests() {
  const provider = await getProvider();
  const program = splStakePoolProgram({
    provider,
    programId: SPL_STAKE_POOL_PROGRAM_ID,
  });
  const tokenProgram = splTokenProgram({
    provider,
    programId: SPL_TOKEN_PROGRAM_ID,
  });
  const kp = await loadKp();

  const VALIDATOR_LIST_COUNT = 10;
  const TRANSIENT_STAKE_SEED = 0;

  let stakePoolPk: PublicKey;
  let withdrawAuthorityPk: PublicKey;
  let poolMintPk: PublicKey;
  let managerPoolTokenAccountPk: PublicKey;
  let validatorListPk: PublicKey;
  let reserveStakePk: PublicKey;
  let stakeAccountPk: PublicKey;
  let voteAccountPk: PublicKey;
  let transientStakePk: PublicKey;
  let userPoolTokenAccountPk: PublicKey;

  async function initialize() {
    const stakePoolKp = new Keypair();
    stakePoolPk = stakePoolKp.publicKey;
    const createStakePoolAccountIx =
      await program.account.stakePool.createInstruction(stakePoolKp, 5000);

    [withdrawAuthorityPk] = await PublicKey.findProgramAddress(
      [stakePoolPk.toBuffer(), Buffer.from("withdraw")],
      program.programId
    );

    poolMintPk = await createMint(withdrawAuthorityPk);
    userPoolTokenAccountPk = await createAta(poolMintPk, kp.publicKey);

    // Fee account
    managerPoolTokenAccountPk = await createTokenAccount(poolMintPk);

    const validatorListKp = new Keypair();
    validatorListPk = validatorListKp.publicKey;
    const createValidatorListAccountIx =
      await program.account.validatorList.createInstruction(
        validatorListKp,
        5 + 4 + 73 * VALIDATOR_LIST_COUNT
      );

    const reserveStakeKp = new Keypair();
    reserveStakePk = reserveStakeKp.publicKey;
    const createReserveStakeAccountIxs = StakeProgram.createAccount({
      authorized: {
        staker: withdrawAuthorityPk,
        withdrawer: withdrawAuthorityPk,
      },
      fromPubkey: kp.publicKey,
      lamports:
        (await provider.connection.getMinimumBalanceForRentExemption(
          StakeProgram.space
        )) +
        LAMPORTS_PER_SOL * 11,
      stakePubkey: reserveStakePk,
    }).instructions;

    const initStakePoolIx = await program.methods
      .initialize(
        {
          denominator: new BN(10),
          numerator: new BN(1),
        },
        {
          denominator: new BN(10),
          numerator: new BN(1),
        },
        {
          denominator: new BN(10),
          numerator: new BN(1),
        },
        10,
        VALIDATOR_LIST_COUNT
      )
      .accounts({
        stakePool: stakePoolKp.publicKey,
        manager: kp.publicKey,
        staker: kp.publicKey,
        stakePoolWithdrawAuthority: withdrawAuthorityPk,
        validatorList: validatorListKp.publicKey,
        reserveStake: reserveStakePk,
        poolMint: poolMintPk,
        managerPoolAccount: managerPoolTokenAccountPk,
        tokenProgram: tokenProgram.programId,
      })
      .instruction();

    await sendAndConfirmTx(
      [
        createStakePoolAccountIx,
        ...createReserveStakeAccountIxs,
        createValidatorListAccountIx,
        initStakePoolIx,
      ],
      [kp, stakePoolKp, validatorListKp, reserveStakeKp]
    );
  }

  async function addValidatorToPool() {
    const voteAccountKp = new Keypair();
    voteAccountPk = voteAccountKp.publicKey;
    const identityKp = new Keypair();
    const createVoteAccountIxs = VoteProgram.createAccount({
      fromPubkey: kp.publicKey,
      lamports: await provider.connection.getMinimumBalanceForRentExemption(
        VoteProgram.space
      ),
      voteInit: {
        authorizedVoter: kp.publicKey,
        authorizedWithdrawer: kp.publicKey,
        commission: 1,
        nodePubkey: identityKp.publicKey,
      },
      votePubkey: voteAccountKp.publicKey,
    }).instructions;

    [stakeAccountPk] = await PublicKey.findProgramAddress(
      [voteAccountPk.toBuffer(), stakePoolPk.toBuffer()],
      program.programId
    );

    [transientStakePk] = await PublicKey.findProgramAddress(
      [
        Buffer.from("transient"),
        voteAccountPk.toBuffer(),
        stakePoolPk.toBuffer(),
        new BN(TRANSIENT_STAKE_SEED).toBuffer("le", 8), // Transient seed suffix start
      ],
      program.programId
    );

    const addValidatorIx = await program.methods
      .addValidatorToPool()
      .accounts({
        stakePool: stakePoolPk,
        staker: kp.publicKey,
        funder: kp.publicKey,
        stakePoolWithdraw: withdrawAuthorityPk,
        validatorList: validatorListPk,
        stake: stakeAccountPk,
        validator: voteAccountPk,
        rent: SYSVAR_RENT_PUBKEY,
        clock: SYSVAR_CLOCK_PUBKEY,
        sysvarStakeHistory: SYSVAR_STAKE_HISTORY_PUBKEY,
        stakeConfig: STAKE_CONFIG_ID,
        systemProgram: SystemProgram.programId,
        stakeProgram: StakeProgram.programId,
      })
      .instruction();

    await sendAndConfirmTx(
      [...createVoteAccountIxs, addValidatorIx],
      [kp, voteAccountKp, identityKp]
    );
  }

  async function removeValidatorFromPool() {
    const destinationStakeAccountKp = new Keypair();
    const createDestinationStakeAccountIx = SystemProgram.createAccount({
      fromPubkey: kp.publicKey,
      lamports: 0,
      newAccountPubkey: destinationStakeAccountKp.publicKey,
      programId: StakeProgram.programId,
      space: StakeProgram.space,
    });

    const removeValidatorIx = await program.methods
      .removeValidatorFromPool()
      .accounts({
        stakePool: stakePoolPk,
        staker: kp.publicKey,
        stakePoolWithdraw: withdrawAuthorityPk,
        newStakeAuthority: kp.publicKey,
        validatorList: validatorListPk,
        stakeAccount: stakeAccountPk,
        transientStakeAccount: transientStakePk,
        destinationStakeAccount: destinationStakeAccountKp.publicKey,
        clock: SYSVAR_CLOCK_PUBKEY,
        stakeProgram: StakeProgram.programId,
      })
      .instruction();
    await sendAndConfirmTx(
      [createDestinationStakeAccountIx, removeValidatorIx],
      [kp, destinationStakeAccountKp]
    );
  }

  async function increaseValidatorStake() {
    await program.methods
      .increaseValidatorStake(
        new BN(LAMPORTS_PER_SOL),
        new BN(TRANSIENT_STAKE_SEED)
      )
      .accounts({
        stakePool: stakePoolPk,
        staker: kp.publicKey,
        stakePoolWithdrawAuthority: withdrawAuthorityPk,
        validatorList: validatorListPk,
        reserveStake: reserveStakePk,
        transientStake: transientStakePk,
        validatorStake: stakeAccountPk,
        validator: voteAccountPk,
        clock: SYSVAR_CLOCK_PUBKEY,
        rent: SYSVAR_RENT_PUBKEY,
        sysvarStakeHistory: SYSVAR_STAKE_HISTORY_PUBKEY,
        stakeConfig: STAKE_CONFIG_ID,
        systemProgram: SystemProgram.programId,
        stakeProgram: StakeProgram.programId,
      })
      .rpc();
  }

  async function decreaseValidatorStake() {
    const decreaseValidatorStakeIx = await program.methods
      .decreaseValidatorStake(
        new BN(LAMPORTS_PER_SOL),
        new BN(TRANSIENT_STAKE_SEED + 1)
      )
      .accounts({
        stakePool: stakePoolPk,
        staker: kp.publicKey,
        stakePoolWithdrawAuthority: withdrawAuthorityPk,
        validatorList: validatorListPk,
        validatorStake: stakeAccountPk,
        transientStake: (
          await PublicKey.findProgramAddress(
            [
              Buffer.from("transient"),
              voteAccountPk.toBuffer(),
              stakePoolPk.toBuffer(),
              new BN(TRANSIENT_STAKE_SEED + 1).toBuffer("le", 8),
            ],
            program.programId
          )
        )[0],
        clock: SYSVAR_CLOCK_PUBKEY,
        rent: SYSVAR_RENT_PUBKEY,
        systemProgram: SystemProgram.programId,
        stakeProgram: StakeProgram.programId,
      })
      .instruction();
    await simulateTx([decreaseValidatorStakeIx], [kp]);
  }

  async function setPreferredValidator() {
    await program.methods
      .setPreferredValidator({ deposit: {} }, null)
      .accounts({
        stakePoolAddress: stakePoolPk,
        staker: kp.publicKey,
        validatorListAddress: validatorListPk,
      })
      .rpc();
  }

  async function updateValidatorListBalance() {
    await program.methods
      .updateValidatorListBalance(0, true)
      .accounts({
        stakePool: stakePoolPk,
        stakePoolWithdrawAuthority: withdrawAuthorityPk,
        validatorListAddress: validatorListPk,
        reserveStake: reserveStakePk,
        clock: SYSVAR_CLOCK_PUBKEY,
        sysvarStakeHistory: SYSVAR_STAKE_HISTORY_PUBKEY,
        stakeProgram: StakeProgram.programId,
      })
      .rpc();
  }

  async function updateStakePoolBalance() {
    await program.methods
      .updateStakePoolBalance()
      .accounts({
        stakePool: stakePoolPk,
        withdrawAuthority: withdrawAuthorityPk,
        validatorListStorage: validatorListPk,
        reserveStake: reserveStakePk,
        managerFeeAccount: managerPoolTokenAccountPk,
        stakePoolMint: poolMintPk,
        tokenProgram: tokenProgram.programId,
      })
      .rpc();
  }

  async function cleanupRemovedValidatorEntries() {
    await program.methods
      .cleanupRemovedValidatorEntries()
      .accounts({
        stakePool: stakePoolPk,
        validatorListStorage: validatorListPk,
      })
      .rpc();
  }

  async function depositStake() {
    const DEPOSIT_AMOUNT = LAMPORTS_PER_SOL;

    const [poolDepositAuthorityPk] = await PublicKey.findProgramAddress(
      [stakePoolPk.toBuffer(), Buffer.from("deposit")],
      program.programId
    );

    const userStakeAccountKp = new Keypair();
    const createUserStakeAccountIxs = StakeProgram.createAccount({
      authorized: { staker: kp.publicKey, withdrawer: kp.publicKey },
      fromPubkey: kp.publicKey,
      lamports:
        (await provider.connection.getMinimumBalanceForRentExemption(
          StakeProgram.space
        )) + DEPOSIT_AMOUNT,
      stakePubkey: userStakeAccountKp.publicKey,
    }).instructions;

    const delegateUserStakeAccountIxs = StakeProgram.delegate({
      authorizedPubkey: kp.publicKey,
      stakePubkey: userStakeAccountKp.publicKey,
      votePubkey: voteAccountPk,
    }).instructions;

    const authorizeStakerIxs = StakeProgram.authorize({
      authorizedPubkey: kp.publicKey,
      newAuthorizedPubkey: poolDepositAuthorityPk,
      stakeAuthorizationType: { index: 0 },
      stakePubkey: userStakeAccountKp.publicKey,
    }).instructions;

    const authorizeWithdrawerIxs = StakeProgram.authorize({
      authorizedPubkey: kp.publicKey,
      newAuthorizedPubkey: poolDepositAuthorityPk,
      stakeAuthorizationType: { index: 1 },
      stakePubkey: userStakeAccountKp.publicKey,
    }).instructions;

    const depositStakeIx = await program.methods
      .depositStake()
      .accounts({
        stakePool: stakePoolPk,
        validatorListStorage: validatorListPk,
        stakePoolDepositAuthority: poolDepositAuthorityPk,
        stakePoolWithdrawAuthority: withdrawAuthorityPk,
        depositStakeAddress: userStakeAccountKp.publicKey,
        validatorStakeAccount: stakeAccountPk,
        reserveStakeAccount: reserveStakePk,
        poolTokensTo: userPoolTokenAccountPk,
        managerFeeAccount: managerPoolTokenAccountPk,
        referrerPoolTokensAccount: managerPoolTokenAccountPk,
        poolMint: poolMintPk,
        clock: SYSVAR_CLOCK_PUBKEY,
        sysvarStakeHistory: SYSVAR_STAKE_HISTORY_PUBKEY,
        tokenProgram: tokenProgram.programId,
        stakeProgram: StakeProgram.programId,
      })
      .instruction();
    await sendAndConfirmTx(
      [
        ...createUserStakeAccountIxs,
        ...delegateUserStakeAccountIxs,
        ...authorizeStakerIxs,
        ...authorizeWithdrawerIxs,
        depositStakeIx,
      ],
      [kp, userStakeAccountKp]
    );
  }

  async function withdrawStake() {
    const userStakeAccountKp = new Keypair();
    const createUserStakeAccountIx = SystemProgram.createAccount({
      fromPubkey: kp.publicKey,
      lamports: await provider.connection.getMinimumBalanceForRentExemption(
        StakeProgram.space
      ),
      newAccountPubkey: userStakeAccountKp.publicKey,
      programId: StakeProgram.programId,
      space: StakeProgram.space,
    });

    const withdrawStakeIx = await program.methods
      .withdrawStake(new BN(1))
      .accounts({
        stakePool: stakePoolPk,
        validatorListStorage: validatorListPk,
        stakePoolWithdraw: withdrawAuthorityPk,
        stakeToSplit: stakeAccountPk,
        stakeToReceive: userStakeAccountKp.publicKey,
        userStakeAuthority: kp.publicKey,
        userTransferAuthority: kp.publicKey,
        userPoolTokenAccount: userPoolTokenAccountPk,
        managerFeeAccount: managerPoolTokenAccountPk,
        poolMint: poolMintPk,
        clock: SYSVAR_CLOCK_PUBKEY,
        tokenProgram: tokenProgram.programId,
        stakeProgram: StakeProgram.programId,
      })
      .instruction();

    await sendAndConfirmTx(
      [createUserStakeAccountIx, withdrawStakeIx],
      [kp, userStakeAccountKp]
    );
  }

  async function setManager() {
    await program.methods
      .setManager()
      .accounts({
        stakePool: stakePoolPk,
        manager: kp.publicKey,
        newManager: kp.publicKey,
        newFeeReceiver: managerPoolTokenAccountPk,
      })
      .rpc();
  }

  async function setFee() {
    await program.methods
      .setFee({ solReferral: [5] })
      .accounts({
        stakePool: stakePoolPk,
        manager: kp.publicKey,
      })
      .rpc();
  }

  async function setStaker() {
    await program.methods
      .setStaker()
      .accounts({
        stakePool: stakePoolPk,
        setStakerAuthority: kp.publicKey,
        newStaker: kp.publicKey,
      })
      .rpc();
  }

  async function depositSol() {
    await program.methods
      .depositSol(new BN(1))
      .accounts({
        stakePool: stakePoolPk,
        stakePoolWithdrawAuthority: withdrawAuthorityPk,
        reserveStakeAccount: reserveStakePk,
        lamportsFrom: kp.publicKey,
        poolTokensTo: userPoolTokenAccountPk,
        managerFeeAccount: managerPoolTokenAccountPk,
        referrerPoolTokensAccount: managerPoolTokenAccountPk,
        poolMint: poolMintPk,
        systemProgram: SystemProgram.programId,
        tokenProgram: tokenProgram.programId,
      })
      .rpc();
  }

  async function setFundingAuthority() {
    await program.methods
      .setFundingAuthority({ stakeDeposit: {} })
      .accounts({
        stakePool: stakePoolPk,
        manager: kp.publicKey,
      })
      .rpc();
  }

  async function withdrawSol() {
    await program.methods
      .withdrawSol(new BN(1))
      .accounts({
        stakePool: stakePoolPk,
        stakePoolWithdrawAuthority: withdrawAuthorityPk,
        userTransferAuthority: kp.publicKey,
        poolTokensFrom: userPoolTokenAccountPk,
        reserveStakeAccount: reserveStakePk,
        lamportsTo: kp.publicKey,
        managerFeeAccount: managerPoolTokenAccountPk,
        poolMint: poolMintPk,
        clock: SYSVAR_CLOCK_PUBKEY,
        sysvarStakeHistory: SYSVAR_STAKE_HISTORY_PUBKEY,
        stakeProgram: StakeProgram.programId,
        tokenProgram: tokenProgram.programId,
      })
      .rpc();
  }

  // TODO: this should work but it's not tested
  // async function createTokenMetadata() {
  // await program.methods.createTokenMetadata(
  //   "acheron",
  //   "ACH",
  //   "https://github.com/acheroncrypto"
  // ).accounts({
  //   stakePool: stakePoolPk,
  //   manager: kp.publicKey,
  //   poolMint: poolMintPk,
  //   payer: kp.publicKey,
  //   tokenMetadata: ,
  //   mplTokenMetadata: ,
  //   systemProgram: SystemProgram.programId,
  //   rent: SYSVAR_RENT_PUBKEY
  // }).rpc();
  // }

  // TODO: this should work but it's not tested
  // async function updateTokenMetadata() {
  // await program.methods.updateTokenMetadata(
  //   "acheron",
  //   "ACH",
  //   "https://twitter.com/acheroncrypto"
  // ).accounts({
  //   stakePool: stakePoolPk,
  //   manager: kp.publicKey,
  //   stakePoolWithdrawAuthority: withdrawAuthorityPk,
  //   tokenMetadata: ,
  //   mplTokenMetadata: ,
  // }).rpc();
  // }

  async function fetchStakePool() {
    const stakePool = await program.account.stakePool.fetch(stakePoolPk);
    assert(stakePool.manager.equals(kp.publicKey));
  }

  async function fetchValidatorList() {
    const validatorList = await program.account.validatorList.fetch(
      validatorListPk
    );
    assert(validatorList.header.maxValidators === VALIDATOR_LIST_COUNT);
  }

  await test(initialize);
  await test(addValidatorToPool);
  await test(removeValidatorFromPool);
  // Re-adding validator for other tests to pass
  await test(addValidatorToPool);
  await test(increaseValidatorStake);
  await test(decreaseValidatorStake);
  await test(setPreferredValidator);
  await test(updateValidatorListBalance);
  await test(updateStakePoolBalance);
  await test(cleanupRemovedValidatorEntries);
  await test(depositStake);
  await test(withdrawStake);
  await test(setManager);
  await test(setFee);
  await test(setStaker);
  await test(depositSol);
  await test(setFundingAuthority);
  await test(withdrawSol);
  // await test(createTokenMetadata);
  // await test(updateTokenMetadata);
  await test(fetchStakePool);
  await test(fetchValidatorList);
}
