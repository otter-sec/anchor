// NOTE: We are not able to test token-lending on local validator without
// changing it's code. Instructions and accounts are being serialized and
// deserialized correctly but the tests are incomplete.

import assert from "assert";
import { splTokenLendingProgram } from "@anchor-lang/spl-token-lending";
import { splTokenProgram } from "@anchor-lang/spl-token";
import { BN } from "@anchor-lang/core";
import {
  Keypair,
  PublicKey,
  SYSVAR_CLOCK_PUBKEY,
  SYSVAR_RENT_PUBKEY,
} from "@solana/web3.js";

import {
  NATIVE_MINT_PK,
  SPL_TOKEN_LENDING_PROGRAM_ID,
  SPL_TOKEN_PROGRAM_ID,
} from "../constants";
import {
  createAta,
  getProvider,
  loadKp,
  sendAndConfirmTx,
  test,
} from "../utils";

export async function tokenLendingTests() {
  const provider = await getProvider();
  const program = splTokenLendingProgram({
    provider,
    programId: SPL_TOKEN_LENDING_PROGRAM_ID,
  });
  const tokenProgram = splTokenProgram({
    provider,
    programId: SPL_TOKEN_PROGRAM_ID,
  });
  const kp = await loadKp();

  // Reserve
  const RESERVE_INIT_LIQUIDITY_AMOUNT = 1_000_000;

  // Oracle
  const PYTH_PROGRAM_ID = new PublicKey(
    "gSbePebfvPy7tRqimPoVecS2UsBvYv46ynrzWocc92s"
  );
  const PYTH_PRODUCT_ID = new PublicKey(
    "3Mnn2fX6rQyUsyELYms1sBJyChWofzSNRoqYzvgMVz5E"
  );
  const PYTH_PRICE_ID = new PublicKey(
    "J83w4HKfqxwcq3BEMMkPFSppX3gqekLyLJBexebFVkix"
  );

  let lendingMarketPk: PublicKey;
  let reservePk: PublicKey;
  let colleteralMintPk: PublicKey;
  let colleteralSupplyAccountPk: PublicKey;
  let userColleteralAccountPk: PublicKey;
  let liquiditySupplyAccountPk: PublicKey;
  let liquidityFeeReceiverAccountPk: PublicKey;
  let sourceLiquidityPk: PublicKey;
  let lendingMarketAuthorityPk: PublicKey;

  async function initLendingMarket() {
    const quoteCurrency = NATIVE_MINT_PK;

    const lendingMarketKp = new Keypair();
    // lendingMarketPk = lendingMarketKp.publicKey;
    const createLendingMarketAccountIx =
      await program.account.lendingMarket.createInstruction(lendingMarketKp);

    const initLendingMarketIx = await program.methods
      .initLendingMarket(
        // @ts-ignore
        kp.publicKey,
        quoteCurrency.toBuffer()
      )
      .accounts({
        lendingMarket: lendingMarketPk,
        rent: SYSVAR_RENT_PUBKEY,
        tokenProgram: tokenProgram.programId,
        oracleProgram: PYTH_PROGRAM_ID,
      })
      .instruction();

    await sendAndConfirmTx(
      [createLendingMarketAccountIx, initLendingMarketIx],
      [kp, lendingMarketKp]
    );
  }

  async function setLendingMarketOwner() {
    await program.methods
      .setLendingMarketOwner(kp.publicKey)
      .accounts({
        lendingMarket: lendingMarketPk,
        lendingMarketOwner: kp.publicKey,
      })
      .rpc();
  }

  async function initReserve() {
    // Create reserve account
    const reserveKp = new Keypair();
    reservePk = reserveKp.publicKey;
    const createReserveAccountIx =
      await program.account.reserve.createInstruction(reserveKp);

    // Create colleteral mint account
    const colleteralMintKp = new Keypair();
    colleteralMintPk = colleteralMintKp.publicKey;
    const createColleteralMintIx =
      await tokenProgram.account.mint.createInstruction(colleteralMintKp);

    // Create colleteral supply account
    const colleteralSupplyAccountKp = new Keypair();
    colleteralSupplyAccountPk = colleteralSupplyAccountKp.publicKey;
    const createColleteralSupplyAccountIx =
      await tokenProgram.account.account.createInstruction(
        colleteralSupplyAccountKp
      );

    // Create user colleteral account
    const userColleteralAccountKp = new Keypair();
    userColleteralAccountPk = userColleteralAccountKp.publicKey;
    const createUserColleteralAccountIx =
      await tokenProgram.account.account.createInstruction(
        userColleteralAccountKp
      );

    // Create liquidity supply account
    const liquiditySupplyAccountKp = new Keypair();
    liquiditySupplyAccountPk = liquiditySupplyAccountKp.publicKey;
    const createLiquiditySupplyAccountIx =
      await tokenProgram.account.account.createInstruction(
        liquiditySupplyAccountKp
      );

    // Create liquidity fee receiver account
    const liquitidyFeeReceiverAccountKp = new Keypair();
    liquidityFeeReceiverAccountPk = liquitidyFeeReceiverAccountKp.publicKey;
    const createLiquidityFeeReceiverAccountIx =
      await tokenProgram.account.account.createInstruction(
        liquitidyFeeReceiverAccountKp
      );

    // Send setup transaction
    await sendAndConfirmTx(
      [
        createReserveAccountIx,
        createColleteralMintIx,
        createColleteralSupplyAccountIx,
        createUserColleteralAccountIx,
        createLiquiditySupplyAccountIx,
        createLiquidityFeeReceiverAccountIx,
      ],
      [
        kp,
        reserveKp,
        colleteralMintKp,
        colleteralSupplyAccountKp,
        userColleteralAccountKp,
        liquiditySupplyAccountKp,
        liquitidyFeeReceiverAccountKp,
      ]
    );

    // Instructions for initializing a reserve
    const userTransferAuthorityKp = new Keypair();
    sourceLiquidityPk = await createAta(NATIVE_MINT_PK, kp.publicKey);

    lendingMarketAuthorityPk = (
      await PublicKey.findProgramAddress(
        [lendingMarketPk.toBuffer()],
        program.programId
      )
    )[0];

    const approveIx = await tokenProgram.methods
      .approve(new BN(RESERVE_INIT_LIQUIDITY_AMOUNT))
      .accounts({
        delegate: userTransferAuthorityKp.publicKey,
        owner: kp.publicKey,
        source: sourceLiquidityPk,
      })
      .instruction();

    const initReserveIx = await program.methods
      // @ts-ignore
      .initReserve(new BN(RESERVE_INIT_LIQUIDITY_AMOUNT), {
        optimalUtilizationRate: 50,
        loanToValueRatio: 1,
        liquidationBonus: 10,
        liquidationThreshold: 5,
        minBorrowRate: 2,
        optimalBorrowRate: 4,
        maxBorrowRate: 10,
        fees: {
          borrowFeeWad: new BN(10000),
          flashLoanFeeWad: new BN(3000000),
          hostFeePercentage: 1,
        },
      })
      .accounts({
        sourceLiquidity: sourceLiquidityPk,
        destinationCollateral: userColleteralAccountPk,
        reserve: reservePk,
        reserveLiquidityMint: NATIVE_MINT_PK,
        reserveLiquiditySupply: liquiditySupplyAccountPk,
        reserveLiquidityFeeReceiver: liquidityFeeReceiverAccountPk,
        reserveCollateralMint: colleteralMintPk,
        reserveCollateralSupply: colleteralSupplyAccountPk,
        pythProduct: PYTH_PRODUCT_ID,
        pythPrice: PYTH_PRICE_ID,
        lendingMarket: lendingMarketPk,
        lendingMarketAuthority: lendingMarketAuthorityPk,
        lendingMarketOwner: kp.publicKey,
        userTransferAuthority: userTransferAuthorityKp.publicKey,
        clock: SYSVAR_CLOCK_PUBKEY,
        rent: SYSVAR_RENT_PUBKEY,
        tokenProgram: tokenProgram.programId,
      })
      .instruction();

    const revokeIx = await tokenProgram.methods
      .revoke()
      .accounts({
        owner: kp.publicKey,
        source: sourceLiquidityPk,
      })
      .instruction();

    await sendAndConfirmTx(
      [approveIx, initReserveIx, revokeIx],
      [kp, userTransferAuthorityKp]
    );
  }

  async function refreshReserve() {}

  async function depositReserveLiquidity() {}

  async function redeemReserveCollateral() {}

  async function initObligation() {}

  async function refreshObligation() {}

  async function depositObligationCollateral() {}

  async function withdrawObligationCollateral() {}

  async function borrowObligationLiquidity() {}

  async function repayObligationLiquidity() {}

  async function liquidateObligation() {}

  async function flashLoan() {}

  async function fetchLendingMarket() {
    const lendingMarket = await program.account.lendingMarket.fetch(
      lendingMarketPk
    );
    assert(lendingMarket.owner.equals(kp.publicKey));
  }

  async function fetchReserve() {}

  async function fetchObligation() {}

  await test(initLendingMarket);
  await test(setLendingMarketOwner);
  // await test(initReserve);
  // await test(refreshReserve);
  // await test(depositReserveLiquidity);
  // await test(redeemReserveCollateral);
  // await test(initObligation);
  // await test(refreshObligation);
  // await test(depositObligationCollateral);
  // await test(withdrawObligationCollateral);
  // await test(borrowObligationLiquidity);
  // await test(repayObligationLiquidity);
  // await test(liquidateObligation);
  // await test(flashLoan);
  await test(fetchLendingMarket);
  // await test(fetchReserve);
  // await test(fetchObligation);
}
