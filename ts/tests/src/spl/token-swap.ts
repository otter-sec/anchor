import assert from "assert";
import { splTokenProgram } from "@anchor-lang/spl-token";
import { splTokenSwapProgram } from "@anchor-lang/spl-token-swap";
import { Keypair, PublicKey, SYSVAR_RENT_PUBKEY } from "@solana/web3.js";
import { BN } from "@anchor-lang/core";

import { SPL_TOKEN_PROGRAM_ID, SPL_TOKEN_SWAP_PROGRAM_ID } from "../constants";
import {
  createTokenAccount,
  getProvider,
  loadKp,
  sendAndConfirmTx,
  test,
} from "../utils";

export async function tokenSwapTests() {
  const provider = await getProvider();
  const program = splTokenSwapProgram({
    provider,
    programId: SPL_TOKEN_SWAP_PROGRAM_ID,
  });
  const tokenProgram = splTokenProgram({
    provider,
    programId: SPL_TOKEN_PROGRAM_ID,
  });
  const kp = await loadKp();

  const userTransferAuthorityKp = new Keypair();
  const userTransferAuthorityPk = userTransferAuthorityKp.publicKey;

  // Pool constants
  const POOL_TOKEN_AMOUNT = 10_000_000;
  const INITIAL_TOKEN_A = 1_000_000;
  const INITIAL_TOKEN_B = 1_000_000;

  // Pool fees
  const TRADING_FEE_NUMERATOR = 25;
  const TRADING_FEE_DENOMINATOR = 10000;
  const OWNER_TRADING_FEE_NUMERATOR = 5;
  const OWNER_TRADING_FEE_DENOMINATOR = 10000;
  const OWNER_WITHDRAW_FEE_NUMERATOR = 1;
  const OWNER_WITHDRAW_FEE_DENOMINATOR = 6;
  const HOST_FEE_NUMERATOR = 20;
  const HOST_FEE_DENOMINATOR = 100;

  // Swap constants
  const SWAP_AMOUNT_IN = 100000;
  const SWAP_AMOUNT_OUT = 90674;

  let swapPk: PublicKey;
  let swapAuthorityPk: PublicKey;
  let poolMintPk: PublicKey;
  let poolTokenAccountPk: PublicKey;
  let feeAccountPk: PublicKey;
  let mintAPk: PublicKey;
  let mintBPk: PublicKey;
  let tokenAccountAPk: PublicKey;
  let tokenAccountBPk: PublicKey;

  async function initialize() {
    const swapKp = new Keypair();
    swapPk = swapKp.publicKey;

    swapAuthorityPk = (
      await PublicKey.findProgramAddress([swapPk.toBuffer()], program.programId)
    )[0];

    const poolMintKp = new Keypair();
    poolMintPk = poolMintKp.publicKey;
    const createPoolMintAccountIx =
      await tokenProgram.account.mint.createInstruction(poolMintKp);
    const initPoolMintAccountIx = await tokenProgram.methods
      .initializeMint(2, swapAuthorityPk, null)
      .accounts({
        mint: poolMintPk,
        rent: SYSVAR_RENT_PUBKEY,
      })
      .instruction();

    const poolTokenAccountKp = new Keypair();
    poolTokenAccountPk = poolTokenAccountKp.publicKey;
    const createPoolTokenAccountIx =
      await tokenProgram.account.account.createInstruction(poolTokenAccountKp);
    const initPoolTokenAccountIx = await tokenProgram.methods
      .initializeAccount()
      .accounts({
        account: poolTokenAccountPk,
        mint: poolMintPk,
        owner: kp.publicKey,
        rent: SYSVAR_RENT_PUBKEY,
      })
      .instruction();

    const feeAccountKp = new Keypair();
    feeAccountPk = feeAccountKp.publicKey;
    const createFeeTokenAccountIx =
      await tokenProgram.account.account.createInstruction(feeAccountKp);
    const initFeeTokenAccountIx = await tokenProgram.methods
      .initializeAccount()
      .accounts({
        account: feeAccountPk,
        mint: poolMintPk,
        owner: kp.publicKey,
        rent: SYSVAR_RENT_PUBKEY,
      })
      .instruction();

    const mintAKp = new Keypair();
    mintAPk = mintAKp.publicKey;
    const createMintAIx = await tokenProgram.account.mint.createInstruction(
      mintAKp
    );
    const initMintAIx = await tokenProgram.methods
      .initializeMint(2, kp.publicKey, null)
      .accounts({ mint: mintAPk, rent: SYSVAR_RENT_PUBKEY })
      .instruction();

    const tokenAccountA = new Keypair();
    tokenAccountAPk = tokenAccountA.publicKey;
    const createTokenAccountAIx =
      await tokenProgram.account.account.createInstruction(tokenAccountA);
    const initTokenAccountAIx = await tokenProgram.methods
      .initializeAccount()
      .accounts({
        account: tokenAccountAPk,
        mint: mintAPk,
        owner: swapAuthorityPk,
        rent: SYSVAR_RENT_PUBKEY,
      })
      .instruction();
    const mintTokenAToSwapIx = await tokenProgram.methods
      .mintTo(new BN(INITIAL_TOKEN_A))
      .accounts({
        account: tokenAccountAPk,
        mint: mintAPk,
        owner: kp.publicKey,
      })
      .instruction();

    const mintBKp = new Keypair();
    mintBPk = mintBKp.publicKey;
    const createMintBIx = await tokenProgram.account.mint.createInstruction(
      mintBKp
    );
    const initMintBIx = await tokenProgram.methods
      .initializeMint(2, kp.publicKey, null)
      .accounts({ mint: mintBPk, rent: SYSVAR_RENT_PUBKEY })
      .instruction();

    const tokenAccountB = new Keypair();
    tokenAccountBPk = tokenAccountB.publicKey;
    const createTokenAccountBIx =
      await tokenProgram.account.account.createInstruction(tokenAccountB);
    const initTokenAccountBIx = await tokenProgram.methods
      .initializeAccount()
      .accounts({
        account: tokenAccountBPk,
        mint: mintBPk,
        owner: swapAuthorityPk,
        rent: SYSVAR_RENT_PUBKEY,
      })
      .instruction();

    const mintTokenBToSwapIx = await tokenProgram.methods
      .mintTo(new BN(INITIAL_TOKEN_B))
      .accounts({
        account: tokenAccountBPk,
        mint: mintBPk,
        owner: kp.publicKey,
      })
      .instruction();

    const createTokenSwapAccountIx =
      await program.account.swap.createInstruction(swapKp);
    const calculator = new Uint8Array(32).fill(0);
    // calculator[0] = 8;
    const initTokenSwapIx = await program.methods
      .initialize(
        // @ts-ignore
        {
          hostFeeDenominator: new BN(HOST_FEE_DENOMINATOR),
          hostFeeNumerator: new BN(HOST_FEE_NUMERATOR),
          ownerTradeFeeDenominator: new BN(OWNER_TRADING_FEE_DENOMINATOR),
          ownerTradeFeeNumerator: new BN(OWNER_TRADING_FEE_NUMERATOR),
          ownerWithdrawFeeDenominator: new BN(OWNER_WITHDRAW_FEE_DENOMINATOR),
          ownerWithdrawFeeNumerator: new BN(OWNER_WITHDRAW_FEE_NUMERATOR),
          tradeFeeDenominator: new BN(TRADING_FEE_DENOMINATOR),
          tradeFeeNumerator: new BN(TRADING_FEE_NUMERATOR),
        },
        {
          curveType: { constantProduct: {} },
          calculator,
        }
      )
      .accounts({
        authority: swapAuthorityPk,
        destination: poolTokenAccountPk,
        fee: feeAccountPk,
        pool: poolMintPk,
        swap: swapPk,
        tokenA: tokenAccountAPk,
        tokenB: tokenAccountBPk,
        tokenProgram: tokenProgram.programId,
      })
      .instruction();

    await sendAndConfirmTx(
      [
        createPoolMintAccountIx,
        initPoolMintAccountIx,
        createPoolTokenAccountIx,
        initPoolTokenAccountIx,
        createFeeTokenAccountIx,
        initFeeTokenAccountIx,
        createMintAIx,
        initMintAIx,
        createTokenAccountAIx,
        initTokenAccountAIx,
        mintTokenAToSwapIx,
      ],
      [kp, poolMintKp, poolTokenAccountKp, feeAccountKp, mintAKp, tokenAccountA]
    );

    await sendAndConfirmTx(
      [
        createMintBIx,
        initMintBIx,
        createTokenAccountBIx,
        initTokenAccountBIx,
        mintTokenBToSwapIx,
      ],
      [kp, mintBKp, tokenAccountB]
    );

    await sendAndConfirmTx(
      [createTokenSwapAccountIx, initTokenSwapIx],
      [kp, swapKp]
    );
  }

  async function depositAllTokenTypes() {
    const poolMintSupply = (await tokenProgram.account.mint.fetch(poolMintPk))
      .supply;
    const swapTokenAAmount = (
      await tokenProgram.account.account.fetch(tokenAccountAPk)
    ).amount;
    const swapTokenBAmount = (
      await tokenProgram.account.account.fetch(tokenAccountBPk)
    ).amount;
    const tokenAmountA = Math.floor(
      (swapTokenAAmount.toNumber() * POOL_TOKEN_AMOUNT) /
        poolMintSupply.toNumber()
    );
    const tokenAmountB = Math.floor(
      (swapTokenBAmount.toNumber() * POOL_TOKEN_AMOUNT) /
        poolMintSupply.toNumber()
    );

    const userDepositPoolTokenAccount = await createTokenAccount(poolMintPk);

    const depositTokenAccountA = await createTokenAccount(mintAPk);
    const mintAIx = await tokenProgram.methods
      .mintTo(new BN(tokenAmountA))
      .accounts({
        account: depositTokenAccountA,
        mint: mintAPk,
        owner: kp.publicKey,
      })
      .instruction();
    const approveIxA = await tokenProgram.methods
      .approve(new BN(tokenAmountA))
      .accounts({
        delegate: userTransferAuthorityPk,
        owner: kp.publicKey,
        source: depositTokenAccountA,
      })
      .instruction();

    const depositTokenAccountB = await createTokenAccount(mintBPk);
    const mintBIx = await tokenProgram.methods
      .mintTo(new BN(tokenAmountB))
      .accounts({
        account: depositTokenAccountB,
        mint: mintBPk,
        owner: kp.publicKey,
      })
      .instruction();
    const approveIxB = await tokenProgram.methods
      .approve(new BN(tokenAmountB))
      .accounts({
        delegate: userTransferAuthorityPk,
        owner: kp.publicKey,
        source: depositTokenAccountB,
      })
      .instruction();

    const depositIx = await program.methods
      .depositAllTokenTypes(
        new BN(POOL_TOKEN_AMOUNT),
        new BN(tokenAmountA),
        new BN(tokenAmountB)
      )
      .accounts({
        swap: swapPk,
        authority: swapAuthorityPk,
        userTransferAuthority: userTransferAuthorityPk,
        depositTokenA: depositTokenAccountA,
        depositTokenB: depositTokenAccountB,
        swapTokenA: tokenAccountAPk,
        swapTokenB: tokenAccountBPk,
        poolMint: poolMintPk,
        destination: userDepositPoolTokenAccount,
        tokenProgram: tokenProgram.programId,
      })
      .instruction();

    await sendAndConfirmTx(
      [mintAIx, approveIxA, mintBIx, approveIxB, depositIx],
      [kp, userTransferAuthorityKp]
    );
  }

  async function withdrawAllTokenTypes() {
    const withdrawUserAccountAPk = await createTokenAccount(mintAPk);
    const withdrawUserAccountBPk = await createTokenAccount(mintBPk);

    const poolMintSupply = (await tokenProgram.account.mint.fetch(poolMintPk))
      .supply;
    const swapTokenAAmount = (
      await tokenProgram.account.account.fetch(tokenAccountAPk)
    ).amount;
    const swapTokenBAmount = (
      await tokenProgram.account.account.fetch(tokenAccountBPk)
    ).amount;
    const feeAmount = Math.floor(
      (POOL_TOKEN_AMOUNT * OWNER_WITHDRAW_FEE_NUMERATOR) /
        OWNER_WITHDRAW_FEE_DENOMINATOR
    );
    const poolTokenNetAmount = POOL_TOKEN_AMOUNT - feeAmount;
    const tokenAmountA = Math.floor(
      (swapTokenAAmount.toNumber() * poolTokenNetAmount) /
        poolMintSupply.toNumber()
    );
    const tokenAmountB = Math.floor(
      (swapTokenBAmount.toNumber() * poolTokenNetAmount) /
        poolMintSupply.toNumber()
    );

    const approveIx = await tokenProgram.methods
      .approve(new BN(POOL_TOKEN_AMOUNT))
      .accounts({
        delegate: userTransferAuthorityPk,
        owner: kp.publicKey,
        source: poolTokenAccountPk,
      })
      .instruction();

    const withdrawIx = await program.methods
      .withdrawAllTokenTypes(
        new BN(POOL_TOKEN_AMOUNT),
        new BN(tokenAmountA),
        new BN(tokenAmountB)
      )
      .accounts({
        swap: swapPk,
        authority: swapAuthorityPk,
        userTransferAuthority: userTransferAuthorityPk,
        poolMint: poolMintPk,
        source: poolTokenAccountPk,
        swapTokenA: tokenAccountAPk,
        swapTokenB: tokenAccountBPk,
        destinationTokenA: withdrawUserAccountAPk,
        destinationTokenB: withdrawUserAccountBPk,
        feeAccount: feeAccountPk,
        tokenProgram: tokenProgram.programId,
      })
      .instruction();

    await sendAndConfirmTx(
      [approveIx, withdrawIx],
      [kp, userTransferAuthorityKp]
    );
  }

  async function swap() {
    const userAccountAPk = await createTokenAccount(mintAPk);
    const mintAIx = await tokenProgram.methods
      .mintTo(new BN(SWAP_AMOUNT_IN))
      .accounts({
        account: userAccountAPk,
        mint: mintAPk,
        owner: kp.publicKey,
      })
      .instruction();
    const approveAIx = await tokenProgram.methods
      .approve(new BN(SWAP_AMOUNT_IN))
      .accounts({
        delegate: userTransferAuthorityPk,
        owner: kp.publicKey,
        source: userAccountAPk,
      })
      .instruction();

    const userAccountBPk = await createTokenAccount(mintBPk);

    const swapIx = await program.methods
      .swap(new BN(SWAP_AMOUNT_IN), new BN(SWAP_AMOUNT_OUT))
      .accounts({
        swap: swapPk,
        authority: swapAuthorityPk,
        userTransferAuthority: userTransferAuthorityPk,
        source: userAccountAPk,
        swapSource: tokenAccountAPk,
        swapDestination: tokenAccountBPk,
        destination: userAccountBPk,
        poolMint: poolMintPk,
        poolFee: feeAccountPk,
        tokenProgram: tokenProgram.programId,
      })
      .instruction();

    await sendAndConfirmTx(
      [mintAIx, approveAIx, swapIx],
      [kp, userTransferAuthorityKp]
    );
  }

  async function depositSingleTokenTypeExactAmountIn() {
    const depositAmount = 1_000_000;

    const poolMintSupply = (await tokenProgram.account.mint.fetch(poolMintPk))
      .supply;
    const swapTokenAAmount = (
      await tokenProgram.account.account.fetch(tokenAccountAPk)
    ).amount;
    const poolTokenA = tradingTokensToPoolTokens(
      depositAmount,
      swapTokenAAmount.toNumber(),
      poolMintSupply.toNumber()
    );
    const userAccountAPk = await createTokenAccount(mintAPk);
    const mintAIx = await tokenProgram.methods
      .mintTo(new BN(depositAmount))
      .accounts({
        account: userAccountAPk,
        mint: mintAPk,
        owner: kp.publicKey,
      })
      .instruction();
    const approveAIx = await tokenProgram.methods
      .approve(new BN(depositAmount))
      .accounts({
        delegate: userTransferAuthorityPk,
        owner: kp.publicKey,
        source: userAccountAPk,
      })
      .instruction();

    const userDepositPoolTokenAccount = await createTokenAccount(poolMintPk);

    const depositSingleTokenTypeExactAmountInAIx = await program.methods
      .depositSingleTokenTypeExactAmountIn(
        new BN(depositAmount),
        new BN(poolTokenA)
      )
      .accounts({
        swap: swapPk,
        authority: swapAuthorityPk,
        userTransferAuthority: userTransferAuthorityPk,
        sourceToken: userAccountAPk,
        swapTokenA: tokenAccountAPk,
        swapTokenB: tokenAccountBPk,
        poolMint: poolMintPk,
        destination: userDepositPoolTokenAccount,
        tokenProgram: tokenProgram.programId,
      })
      .instruction();

    const swapTokenBAmount = (
      await tokenProgram.account.account.fetch(tokenAccountBPk)
    ).amount;
    const poolTokenB = tradingTokensToPoolTokens(
      depositAmount,
      swapTokenBAmount.toNumber(),
      poolMintSupply.toNumber()
    );
    const userAccountBPk = await createTokenAccount(mintBPk);
    const mintBIx = await tokenProgram.methods
      .mintTo(new BN(depositAmount))
      .accounts({
        account: userAccountBPk,
        mint: mintBPk,
        owner: kp.publicKey,
      })
      .instruction();
    const approveBIx = await tokenProgram.methods
      .approve(new BN(depositAmount))
      .accounts({
        delegate: userTransferAuthorityPk,
        owner: kp.publicKey,
        source: userAccountBPk,
      })
      .instruction();

    const depositSingleTokenTypeExactAmountInBIx = await program.methods
      .depositSingleTokenTypeExactAmountIn(
        new BN(depositAmount),
        new BN(poolTokenB)
      )
      .accounts({
        swap: swapPk,
        authority: swapAuthorityPk,
        userTransferAuthority: userTransferAuthorityPk,
        sourceToken: userAccountBPk,
        swapTokenA: tokenAccountAPk,
        swapTokenB: tokenAccountBPk,
        poolMint: poolMintPk,
        destination: userDepositPoolTokenAccount,
        tokenProgram: tokenProgram.programId,
      })
      .instruction();

    await sendAndConfirmTx(
      [
        mintAIx,
        approveAIx,
        depositSingleTokenTypeExactAmountInAIx,
        mintBIx,
        approveBIx,
        depositSingleTokenTypeExactAmountInBIx,
      ],
      [kp, userTransferAuthorityKp]
    );
  }

  async function withdrawSingleTokenTypeExactAmountOut() {
    const withdrawAmount = 50_000;
    const multiplier = 1.04;

    const poolMintSupply = (await tokenProgram.account.mint.fetch(poolMintPk))
      .supply;
    const swapTokenAAmount = (
      await tokenProgram.account.account.fetch(tokenAccountAPk)
    ).amount;
    const poolTokenA = tradingTokensToPoolTokens(
      withdrawAmount,
      swapTokenAAmount.toNumber(),
      poolMintSupply.toNumber()
    );
    const maximumPoolTokenAmountA =
      poolTokenA *
      multiplier *
      (1 + OWNER_WITHDRAW_FEE_NUMERATOR / OWNER_WITHDRAW_FEE_DENOMINATOR);

    const swapTokenBAmount = (
      await tokenProgram.account.account.fetch(tokenAccountBPk)
    ).amount;
    const poolTokenB = tradingTokensToPoolTokens(
      withdrawAmount,
      swapTokenBAmount.toNumber(),
      poolMintSupply.toNumber()
    );
    const maximumPoolTokenAmountB =
      poolTokenB *
      multiplier *
      (1 + OWNER_WITHDRAW_FEE_NUMERATOR / OWNER_WITHDRAW_FEE_DENOMINATOR);

    const poolTokenApproveIx = await tokenProgram.methods
      .approve(new BN(maximumPoolTokenAmountA + maximumPoolTokenAmountB))
      .accounts({
        delegate: userTransferAuthorityPk,
        owner: kp.publicKey,
        source: poolTokenAccountPk,
      })
      .instruction();

    const withdrawUserTokenAccountAPk = await createTokenAccount(mintAPk);
    const withdrawUserTokenAccountBPk = await createTokenAccount(mintBPk);

    const withdrawSingleTokenTypeExactAmountOutAIx = await program.methods
      .withdrawSingleTokenTypeExactAmountOut(
        new BN(withdrawAmount),
        new BN(maximumPoolTokenAmountA)
      )
      .accounts({
        swap: swapPk,
        authority: swapAuthorityPk,
        userTransferAuthority: userTransferAuthorityPk,
        poolMint: poolMintPk,
        poolTokenSource: poolTokenAccountPk,
        swapTokenA: tokenAccountAPk,
        swapTokenB: tokenAccountBPk,
        destination: withdrawUserTokenAccountAPk,
        feeAccount: feeAccountPk,
        tokenProgram: tokenProgram.programId,
      })
      .instruction();

    const withdrawSingleTokenTypeExactAmountOutBIx = await program.methods
      .withdrawSingleTokenTypeExactAmountOut(
        new BN(withdrawAmount),
        new BN(maximumPoolTokenAmountB)
      )
      .accounts({
        swap: swapPk,
        authority: swapAuthorityPk,
        userTransferAuthority: userTransferAuthorityPk,
        poolMint: poolMintPk,
        poolTokenSource: poolTokenAccountPk,
        swapTokenA: tokenAccountAPk,
        swapTokenB: tokenAccountBPk,
        destination: withdrawUserTokenAccountBPk,
        feeAccount: feeAccountPk,
        tokenProgram: tokenProgram.programId,
      })
      .instruction();

    await sendAndConfirmTx(
      [
        poolTokenApproveIx,
        withdrawSingleTokenTypeExactAmountOutAIx,
        withdrawSingleTokenTypeExactAmountOutBIx,
      ],
      [kp, userTransferAuthorityKp]
    );
  }

  async function fetchSwap() {
    const swap = await program.account.swap.fetch(swapPk);
    assert(swap.isInitialized === true);
    assert(swap.poolFeeAccount.equals(feeAccountPk));
    assert(swap.poolMint.equals(poolMintPk));
    assert(swap.tokenA.equals(tokenAccountAPk));
    assert(swap.tokenAMint.equals(mintAPk));
    assert(swap.tokenB.equals(tokenAccountBPk));
    assert(swap.tokenBMint.equals(mintBPk));
    assert(swap.tokenProgramId.equals(tokenProgram.programId));
  }

  function tradingTokensToPoolTokens(
    sourceAmount: number,
    swapSourceAmount: number,
    poolAmount: number
  ): number {
    const tradingFee =
      (sourceAmount / 2) * (TRADING_FEE_NUMERATOR / TRADING_FEE_DENOMINATOR);
    const sourceAmountPostFee = sourceAmount - tradingFee;
    const root = Math.sqrt(sourceAmountPostFee / swapSourceAmount + 1);
    return Math.floor(poolAmount * (root - 1));
  }

  await test(initialize);
  await test(depositAllTokenTypes);
  await test(withdrawAllTokenTypes);
  await test(swap);
  await test(depositSingleTokenTypeExactAmountIn);
  await test(withdrawSingleTokenTypeExactAmountOut);
  await test(fetchSwap);
}
