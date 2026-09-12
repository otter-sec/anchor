const { assert } = require("chai");
const anchor = require("@anchor-lang/core");
const BN = anchor.BN;
const OpenOrders = require("@project-serum/serum").OpenOrders;
const TOKEN_PROGRAM_ID = require("@solana/spl-token").TOKEN_PROGRAM_ID;
const serumCmn = require("@project-serum/common");
const utils = require("./utils");

// Taker fee rate (bps).
const TAKER_FEE = 0.0022;

describe("swap", () => {
  // Configure the client to use the local cluster.
  const provider = anchor.AnchorProvider.env();
  // hack so we don't have to update serum-common library
  // to the new AnchorProvider class and Provider interface
  provider.send = provider.sendAndConfirm;
  anchor.setProvider(provider);

  // Swap program client.
  const program = anchor.workspace.Swap;

  // Accounts used to setup the orderbook.
  let ORDERBOOK_ENV,
    // Accounts used for A -> USDC swap transactions.
    SWAP_A_USDC_ACCOUNTS,
    // Accounts used for  USDC -> A swap transactions.
    SWAP_USDC_A_ACCOUNTS,
    // Serum DEX vault PDA for market A/USDC.
    marketAVaultSigner,
    // Serum DEX vault PDA for market B/USDC.
    marketBVaultSigner;

  // Open orders accounts on the two markets for the provider.
  const openOrdersA = anchor.web3.Keypair.generate();
  const openOrdersB = anchor.web3.Keypair.generate();

  it("BOILERPLATE: Sets up two markets with resting orders", async () => {
    ORDERBOOK_ENV = await utils.setupTwoMarkets({
      provider: program.provider,
    });
  });

  it("BOILERPLATE: Sets up reusable accounts", async () => {
    const marketA = ORDERBOOK_ENV.marketA;
    const marketB = ORDERBOOK_ENV.marketB;

    const [vaultSignerA] = await utils.getVaultOwnerAndNonce(
      marketA._decoded.ownAddress
    );
    const [vaultSignerB] = await utils.getVaultOwnerAndNonce(
      marketB._decoded.ownAddress
    );
    marketAVaultSigner = vaultSignerA;
    marketBVaultSigner = vaultSignerB;

    SWAP_USDC_A_ACCOUNTS = {
      market: {
        market: marketA._decoded.ownAddress,
        requestQueue: marketA._decoded.requestQueue,
        eventQueue: marketA._decoded.eventQueue,
        bids: marketA._decoded.bids,
        asks: marketA._decoded.asks,
        coinVault: marketA._decoded.baseVault,
        pcVault: marketA._decoded.quoteVault,
        vaultSigner: marketAVaultSigner,
        // User params.
        openOrders: openOrdersA.publicKey,
        orderPayerTokenAccount: ORDERBOOK_ENV.godUsdc,
        coinWallet: ORDERBOOK_ENV.godA,
      },
      pcWallet: ORDERBOOK_ENV.godUsdc,
      authority: program.provider.wallet.publicKey,
      dexProgram: utils.DEX_PID,
      tokenProgram: TOKEN_PROGRAM_ID,
      rent: anchor.web3.SYSVAR_RENT_PUBKEY,
    };
    SWAP_A_USDC_ACCOUNTS = {
      ...SWAP_USDC_A_ACCOUNTS,
      market: {
        ...SWAP_USDC_A_ACCOUNTS.market,
        orderPayerTokenAccount: ORDERBOOK_ENV.godA,
      },
    };
  });

  it("Swaps from USDC to Token A", async () => {
    const marketA = ORDERBOOK_ENV.marketA;

    // Swap exactly enough USDC to get 1.2 A tokens (best offer price is 6.041 USDC).
    const expectedResultantAmount = 7.2;
    const bestOfferPrice = 6.041;
    const amountToSpend = expectedResultantAmount * bestOfferPrice;
    const swapAmount = new BN((amountToSpend / (1 - TAKER_FEE)) * 10 ** 6);

    const [tokenAChange, usdcChange] = await withBalanceChange(
      program.provider,
      [ORDERBOOK_ENV.godA, ORDERBOOK_ENV.godUsdc],
      async () => {
        await program.rpc.swap(Side.Bid, swapAmount, new BN(1.0), {
          accounts: SWAP_USDC_A_ACCOUNTS,
          instructions: [
            // First order to this market so one must create the open orders account.
            await OpenOrders.makeCreateAccountTransaction(
              program.provider.connection,
              marketA._decoded.ownAddress,
              program.provider.wallet.publicKey,
              openOrdersA.publicKey,
              utils.DEX_PID
            ),
            // Might as well create the second open orders account while we're here.
            // In prod, this should actually be done within the same tx as an
            // order to market B.
            await OpenOrders.makeCreateAccountTransaction(
              program.provider.connection,
              ORDERBOOK_ENV.marketB._decoded.ownAddress,
              program.provider.wallet.publicKey,
              openOrdersB.publicKey,
              utils.DEX_PID
            ),
          ],
          signers: [openOrdersA, openOrdersB],
        });
      }
    );

    assert.strictEqual(tokenAChange, expectedResultantAmount);
    assert.strictEqual(usdcChange, -swapAmount.toNumber() / 10 ** 6);
  });

  it("Swaps from Token A to USDC", async () => {
    const marketA = ORDERBOOK_ENV.marketA;

    // Swap out A tokens for USDC.
    const swapAmount = 8.1;
    const bestBidPrice = 6.004;
    const amountToFill = swapAmount * bestBidPrice;
    const takerFee = 0.0022;
    const resultantAmount = new BN(amountToFill * (1 - TAKER_FEE) * 10 ** 6);

    const [tokenAChange, usdcChange] = await withBalanceChange(
      program.provider,
      [ORDERBOOK_ENV.godA, ORDERBOOK_ENV.godUsdc],
      async () => {
        await program.rpc.swap(
          Side.Ask,
          new BN(swapAmount * 10 ** 6),
          new BN(swapAmount),
          {
            accounts: SWAP_A_USDC_ACCOUNTS,
          }
        );
      }
    );

    assert.strictEqual(tokenAChange, -swapAmount);
    assert.strictEqual(usdcChange, resultantAmount.toNumber() / 10 ** 6);
  });

  it("Swaps from Token A to Token B", async () => {
    const marketA = ORDERBOOK_ENV.marketA;
    const marketB = ORDERBOOK_ENV.marketB;
    const swapAmount = 10;
    const [tokenAChange, tokenBChange, usdcChange] = await withBalanceChange(
      program.provider,
      [ORDERBOOK_ENV.godA, ORDERBOOK_ENV.godB, ORDERBOOK_ENV.godUsdc],
      async () => {
        // Perform the actual swap.
        await program.rpc.swapTransitive(
          new BN(swapAmount * 10 ** 6),
          new BN(swapAmount - 1),
          {
            accounts: {
              from: {
                market: marketA._decoded.ownAddress,
                requestQueue: marketA._decoded.requestQueue,
                eventQueue: marketA._decoded.eventQueue,
                bids: marketA._decoded.bids,
                asks: marketA._decoded.asks,
                coinVault: marketA._decoded.baseVault,
                pcVault: marketA._decoded.quoteVault,
                vaultSigner: marketAVaultSigner,
                // User params.
                openOrders: openOrdersA.publicKey,
                // Swapping from A -> USDC.
                orderPayerTokenAccount: ORDERBOOK_ENV.godA,
                coinWallet: ORDERBOOK_ENV.godA,
              },
              to: {
                market: marketB._decoded.ownAddress,
                requestQueue: marketB._decoded.requestQueue,
                eventQueue: marketB._decoded.eventQueue,
                bids: marketB._decoded.bids,
                asks: marketB._decoded.asks,
                coinVault: marketB._decoded.baseVault,
                pcVault: marketB._decoded.quoteVault,
                vaultSigner: marketBVaultSigner,
                // User params.
                openOrders: openOrdersB.publicKey,
                // Swapping from USDC -> B.
                orderPayerTokenAccount: ORDERBOOK_ENV.godUsdc,
                coinWallet: ORDERBOOK_ENV.godB,
              },
              pcWallet: ORDERBOOK_ENV.godUsdc,
              authority: program.provider.wallet.publicKey,
              dexProgram: utils.DEX_PID,
              tokenProgram: TOKEN_PROGRAM_ID,
              rent: anchor.web3.SYSVAR_RENT_PUBKEY,
            },
          }
        );
      }
    );

    assert.strictEqual(tokenAChange, -swapAmount);
    // TODO: calculate this dynamically from the swap amount.
    assert.strictEqual(tokenBChange, 9.8);
    assert.strictEqual(usdcChange, 0);
  });

  it("Swaps from Token B to Token A", async () => {
    const marketA = ORDERBOOK_ENV.marketA;
    const marketB = ORDERBOOK_ENV.marketB;
    const swapAmount = 23;
    const [tokenAChange, tokenBChange, usdcChange] = await withBalanceChange(
      program.provider,
      [ORDERBOOK_ENV.godA, ORDERBOOK_ENV.godB, ORDERBOOK_ENV.godUsdc],
      async () => {
        // Perform the actual swap.
        await program.rpc.swapTransitive(
          new BN(swapAmount * 10 ** 6),
          new BN(swapAmount - 1),
          {
            accounts: {
              from: {
                market: marketB._decoded.ownAddress,
                requestQueue: marketB._decoded.requestQueue,
                eventQueue: marketB._decoded.eventQueue,
                bids: marketB._decoded.bids,
                asks: marketB._decoded.asks,
                coinVault: marketB._decoded.baseVault,
                pcVault: marketB._decoded.quoteVault,
                vaultSigner: marketBVaultSigner,
                // User params.
                openOrders: openOrdersB.publicKey,
                // Swapping from B -> USDC.
                orderPayerTokenAccount: ORDERBOOK_ENV.godB,
                coinWallet: ORDERBOOK_ENV.godB,
              },
              to: {
                market: marketA._decoded.ownAddress,
                requestQueue: marketA._decoded.requestQueue,
                eventQueue: marketA._decoded.eventQueue,
                bids: marketA._decoded.bids,
                asks: marketA._decoded.asks,
                coinVault: marketA._decoded.baseVault,
                pcVault: marketA._decoded.quoteVault,
                vaultSigner: marketAVaultSigner,
                // User params.
                openOrders: openOrdersA.publicKey,
                // Swapping from USDC -> A.
                orderPayerTokenAccount: ORDERBOOK_ENV.godUsdc,
                coinWallet: ORDERBOOK_ENV.godA,
              },
              pcWallet: ORDERBOOK_ENV.godUsdc,
              authority: program.provider.wallet.publicKey,
              dexProgram: utils.DEX_PID,
              tokenProgram: TOKEN_PROGRAM_ID,
              rent: anchor.web3.SYSVAR_RENT_PUBKEY,
            },
          }
        );
      }
    );

    // TODO: calculate this dynamically from the swap amount.
    assert.strictEqual(tokenAChange, 22.6);
    assert.strictEqual(tokenBChange, -swapAmount);
    assert.strictEqual(usdcChange, 0);
  });
});

// Side rust enum used for the program's RPC API.
const Side = {
  Bid: { bid: {} },
  Ask: { ask: {} },
};

// Executes a closure. Returning the change in balances from before and after
// its execution.
async function withBalanceChange(provider, addrs, fn) {
  const beforeBalances = [];
  for (let k = 0; k < addrs.length; k += 1) {
    beforeBalances.push(
      (await serumCmn.getTokenAccount(provider, addrs[k])).amount
    );
  }

  await fn();

  const afterBalances = [];
  for (let k = 0; k < addrs.length; k += 1) {
    afterBalances.push(
      (await serumCmn.getTokenAccount(provider, addrs[k])).amount
    );
  }

  const deltas = [];
  for (let k = 0; k < addrs.length; k += 1) {
    deltas.push(
      (afterBalances[k].toNumber() - beforeBalances[k].toNumber()) / 10 ** 6
    );
  }
  return deltas;
}
