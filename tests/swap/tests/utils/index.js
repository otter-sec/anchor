// Boilerplate utils to bootstrap an orderbook for testing on a localnet.
// not super relevant to the point of the example, though may be useful to
// include into your own workspace for testing.
//
// TODO: Modernize all these apis. This is all quite clunky.

const Token = require("@solana/spl-token").Token;
const TOKEN_PROGRAM_ID = require("@solana/spl-token").TOKEN_PROGRAM_ID;
const TokenInstructions = require("@project-serum/serum").TokenInstructions;
const Market = require("@project-serum/serum").Market;
const DexInstructions = require("@project-serum/serum").DexInstructions;
const web3 = require("@anchor-lang/core").web3;
const Connection = web3.Connection;
const BN = require("@anchor-lang/core").BN;
const serumCmn = require("@project-serum/common");
const Account = web3.Account;
const Transaction = web3.Transaction;
const PublicKey = web3.PublicKey;
const SystemProgram = web3.SystemProgram;
const DEX_PID = new PublicKey("srmqPvymJeFKQ4zGQed1GFppgkRHL9kaELCbyksJtPX");

async function setupTwoMarkets({ provider }) {
  // Setup mints with initial tokens owned by the provider.
  const decimals = 6;
  const [MINT_A, GOD_A] = await serumCmn.createMintAndVault(
    provider,
    new BN(1000000000000000),
    undefined,
    decimals
  );
  const [MINT_B, GOD_B] = await serumCmn.createMintAndVault(
    provider,
    new BN(1000000000000000),
    undefined,
    decimals
  );
  const [USDC, GOD_USDC] = await serumCmn.createMintAndVault(
    provider,
    new BN(1000000000000000),
    undefined,
    decimals
  );

  // Create a funded account to act as market maker.
  const amount = 100000 * 10 ** decimals;
  const marketMaker = await fundAccount({
    provider,
    mints: [
      { god: GOD_A, mint: MINT_A, amount, decimals },
      { god: GOD_B, mint: MINT_B, amount, decimals },
      { god: GOD_USDC, mint: USDC, amount, decimals },
    ],
  });

  // Setup A/USDC and B/USDC markets with resting orders.
  const asks = [
    [6.041, 7.8],
    [6.051, 72.3],
    [6.055, 5.4],
    [6.067, 15.7],
    [6.077, 390.0],
    [6.09, 24.0],
    [6.11, 36.3],
    [6.133, 300.0],
    [6.167, 687.8],
  ];
  const bids = [
    [6.004, 8.5],
    [5.995, 12.9],
    [5.987, 6.2],
    [5.978, 15.3],
    [5.965, 82.8],
    [5.961, 25.4],
  ];

  MARKET_A_USDC = await setupMarket({
    baseMint: MINT_A,
    quoteMint: USDC,
    marketMaker: {
      account: marketMaker.account,
      baseToken: marketMaker.tokens[MINT_A.toString()],
      quoteToken: marketMaker.tokens[USDC.toString()],
    },
    bids,
    asks,
    provider,
  });
  MARKET_B_USDC = await setupMarket({
    baseMint: MINT_B,
    quoteMint: USDC,
    marketMaker: {
      account: marketMaker.account,
      baseToken: marketMaker.tokens[MINT_B.toString()],
      quoteToken: marketMaker.tokens[USDC.toString()],
    },
    bids,
    asks,
    provider,
  });

  return {
    marketA: MARKET_A_USDC,
    marketB: MARKET_B_USDC,
    marketMaker,
    mintA: MINT_A,
    mintB: MINT_B,
    usdc: USDC,
    godA: GOD_A,
    godB: GOD_B,
    godUsdc: GOD_USDC,
  };
}

// Creates everything needed for an orderbook to be running
//
// * Mints for both the base and quote currencies.
// * Lists the market.
// * Provides resting orders on the market.
//
// Returns a client that can be used to interact with the market
// (and some other data, e.g., the mints and market maker account).
async function initOrderbook({ provider, bids, asks }) {
  if (!bids || !asks) {
    asks = [
      [6.041, 7.8],
      [6.051, 72.3],
      [6.055, 5.4],
      [6.067, 15.7],
      [6.077, 390.0],
      [6.09, 24.0],
      [6.11, 36.3],
      [6.133, 300.0],
      [6.167, 687.8],
    ];
    bids = [
      [6.004, 8.5],
      [5.995, 12.9],
      [5.987, 6.2],
      [5.978, 15.3],
      [5.965, 82.8],
      [5.961, 25.4],
    ];
  }
  // Create base and quote currency mints.
  const decimals = 6;
  const [MINT_A, GOD_A] = await serumCmn.createMintAndVault(
    provider,
    new BN(1000000000000000),
    undefined,
    decimals
  );
  const [USDC, GOD_USDC] = await serumCmn.createMintAndVault(
    provider,
    new BN(1000000000000000),
    undefined,
    decimals
  );

  // Create a funded account to act as market maker.
  const amount = 100000 * 10 ** decimals;
  const marketMaker = await fundAccount({
    provider,
    mints: [
      { god: GOD_A, mint: MINT_A, amount, decimals },
      { god: GOD_USDC, mint: USDC, amount, decimals },
    ],
  });

  marketClient = await setupMarket({
    baseMint: MINT_A,
    quoteMint: USDC,
    marketMaker: {
      account: marketMaker.account,
      baseToken: marketMaker.tokens[MINT_A.toString()],
      quoteToken: marketMaker.tokens[USDC.toString()],
    },
    bids,
    asks,
    provider,
  });

  return {
    marketClient,
    baseMint: MINT_A,
    quoteMint: USDC,
    marketMaker,
  };
}

async function fundAccount({ provider, mints }) {
  const MARKET_MAKER = new Account();

  const marketMaker = {
    tokens: {},
    account: MARKET_MAKER,
  };

  // Transfer lamports to market maker.
  await provider.sendAndConfirm(
    (() => {
      const tx = new Transaction();
      tx.add(
        SystemProgram.transfer({
          fromPubkey: provider.wallet.publicKey,
          toPubkey: MARKET_MAKER.publicKey,
          lamports: 100000000000,
        })
      );
      return tx;
    })()
  );

  // Transfer SPL tokens to the market maker.
  for (let k = 0; k < mints.length; k += 1) {
    const { mint, god, amount, decimals } = mints[k];
    let MINT_A = mint;
    let GOD_A = god;
    // Setup token accounts owned by the market maker.
    const mintAClient = new Token(
      provider.connection,
      MINT_A,
      TOKEN_PROGRAM_ID,
      provider.wallet.payer // node only
    );
    const marketMakerTokenA = await mintAClient.createAccount(
      MARKET_MAKER.publicKey
    );

    await provider.sendAndConfirm(
      (() => {
        const tx = new Transaction();
        tx.add(
          Token.createTransferCheckedInstruction(
            TOKEN_PROGRAM_ID,
            GOD_A,
            MINT_A,
            marketMakerTokenA,
            provider.wallet.publicKey,
            [],
            amount,
            decimals
          )
        );
        return tx;
      })()
    );

    marketMaker.tokens[mint.toString()] = marketMakerTokenA;
  }

  return marketMaker;
}

async function setupMarket({
  provider,
  marketMaker,
  baseMint,
  quoteMint,
  bids,
  asks,
}) {
  const marketAPublicKey = await listMarket({
    connection: provider.connection,
    wallet: provider.wallet,
    baseMint: baseMint,
    quoteMint: quoteMint,
    baseLotSize: 100000,
    quoteLotSize: 100,
    dexProgramId: DEX_PID,
    feeRateBps: 0,
  });
  const MARKET_A_USDC = await Market.load(
    provider.connection,
    marketAPublicKey,
    { commitment: "processed" },
    DEX_PID
  );
  for (let k = 0; k < asks.length; k += 1) {
    let ask = asks[k];
    const { transaction, signers } =
      await MARKET_A_USDC.makePlaceOrderTransaction(provider.connection, {
        owner: marketMaker.account,
        payer: marketMaker.baseToken,
        side: "sell",
        price: ask[0],
        size: ask[1],
        orderType: "postOnly",
        clientId: undefined,
        openOrdersAddressKey: undefined,
        openOrdersAccount: undefined,
        feeDiscountPubkey: null,
        selfTradeBehavior: "abortTransaction",
      });
    await provider.sendAndConfirm(
      transaction,
      signers.concat(marketMaker.account)
    );
  }

  for (let k = 0; k < bids.length; k += 1) {
    let bid = bids[k];
    const { transaction, signers } =
      await MARKET_A_USDC.makePlaceOrderTransaction(provider.connection, {
        owner: marketMaker.account,
        payer: marketMaker.quoteToken,
        side: "buy",
        price: bid[0],
        size: bid[1],
        orderType: "postOnly",
        clientId: undefined,
        openOrdersAddressKey: undefined,
        openOrdersAccount: undefined,
        feeDiscountPubkey: null,
        selfTradeBehavior: "abortTransaction",
      });
    await provider.sendAndConfirm(
      transaction,
      signers.concat(marketMaker.account)
    );
  }

  return MARKET_A_USDC;
}

async function listMarket({
  connection,
  wallet,
  baseMint,
  quoteMint,
  baseLotSize,
  quoteLotSize,
  dexProgramId,
  feeRateBps,
}) {
  const market = new Account();
  const requestQueue = new Account();
  const eventQueue = new Account();
  const bids = new Account();
  const asks = new Account();
  const baseVault = new Account();
  const quoteVault = new Account();
  const quoteDustThreshold = new BN(100);

  const [vaultOwner, vaultSignerNonce] = await getVaultOwnerAndNonce(
    market.publicKey,
    dexProgramId
  );

  const tx1 = new Transaction();
  tx1.add(
    SystemProgram.createAccount({
      fromPubkey: wallet.publicKey,
      newAccountPubkey: baseVault.publicKey,
      lamports: await connection.getMinimumBalanceForRentExemption(165),
      space: 165,
      programId: TOKEN_PROGRAM_ID,
    }),
    SystemProgram.createAccount({
      fromPubkey: wallet.publicKey,
      newAccountPubkey: quoteVault.publicKey,
      lamports: await connection.getMinimumBalanceForRentExemption(165),
      space: 165,
      programId: TOKEN_PROGRAM_ID,
    }),
    TokenInstructions.initializeAccount({
      account: baseVault.publicKey,
      mint: baseMint,
      owner: vaultOwner,
    }),
    TokenInstructions.initializeAccount({
      account: quoteVault.publicKey,
      mint: quoteMint,
      owner: vaultOwner,
    })
  );

  const tx2 = new Transaction();
  tx2.add(
    SystemProgram.createAccount({
      fromPubkey: wallet.publicKey,
      newAccountPubkey: market.publicKey,
      lamports: await connection.getMinimumBalanceForRentExemption(
        Market.getLayout(dexProgramId).span
      ),
      space: Market.getLayout(dexProgramId).span,
      programId: dexProgramId,
    }),
    SystemProgram.createAccount({
      fromPubkey: wallet.publicKey,
      newAccountPubkey: requestQueue.publicKey,
      lamports: await connection.getMinimumBalanceForRentExemption(5120 + 12),
      space: 5120 + 12,
      programId: dexProgramId,
    }),
    SystemProgram.createAccount({
      fromPubkey: wallet.publicKey,
      newAccountPubkey: eventQueue.publicKey,
      lamports: await connection.getMinimumBalanceForRentExemption(262144 + 12),
      space: 262144 + 12,
      programId: dexProgramId,
    }),
    SystemProgram.createAccount({
      fromPubkey: wallet.publicKey,
      newAccountPubkey: bids.publicKey,
      lamports: await connection.getMinimumBalanceForRentExemption(65536 + 12),
      space: 65536 + 12,
      programId: dexProgramId,
    }),
    SystemProgram.createAccount({
      fromPubkey: wallet.publicKey,
      newAccountPubkey: asks.publicKey,
      lamports: await connection.getMinimumBalanceForRentExemption(65536 + 12),
      space: 65536 + 12,
      programId: dexProgramId,
    }),
    DexInstructions.initializeMarket({
      market: market.publicKey,
      requestQueue: requestQueue.publicKey,
      eventQueue: eventQueue.publicKey,
      bids: bids.publicKey,
      asks: asks.publicKey,
      baseVault: baseVault.publicKey,
      quoteVault: quoteVault.publicKey,
      baseMint,
      quoteMint,
      baseLotSize: new BN(baseLotSize),
      quoteLotSize: new BN(quoteLotSize),
      feeRateBps,
      vaultSignerNonce,
      quoteDustThreshold,
      programId: dexProgramId,
    })
  );

  const signedTransactions = await signTransactions({
    transactionsAndSigners: [
      { transaction: tx1, signers: [baseVault, quoteVault] },
      {
        transaction: tx2,
        signers: [market, requestQueue, eventQueue, bids, asks],
      },
    ],
    wallet,
    connection,
  });
  for (let signedTransaction of signedTransactions) {
    await sendAndConfirmRawTransaction(
      connection,
      signedTransaction.serialize()
    );
  }
  const acc = await connection.getAccountInfo(market.publicKey);

  return market.publicKey;
}

async function signTransactions({
  transactionsAndSigners,
  wallet,
  connection,
}) {
  const blockhash = (await connection.getLatestBlockhash("finalized"))
    .blockhash;
  transactionsAndSigners.forEach(({ transaction, signers = [] }) => {
    transaction.recentBlockhash = blockhash;
    transaction.setSigners(
      wallet.publicKey,
      ...signers.map((s) => s.publicKey)
    );
    if (signers.length > 0) {
      transaction.partialSign(...signers);
    }
  });
  return await wallet.signAllTransactions(
    transactionsAndSigners.map(({ transaction }) => transaction)
  );
}

async function sendAndConfirmRawTransaction(
  connection,
  raw,
  commitment = "processed"
) {
  let tx = await connection.sendRawTransaction(raw, {
    skipPreflight: true,
  });
  return await connection.confirmTransaction(tx, commitment);
}

async function getVaultOwnerAndNonce(marketPublicKey, dexProgramId = DEX_PID) {
  const nonce = new BN(0);
  while (nonce.toNumber() < 255) {
    try {
      const vaultOwner = await PublicKey.createProgramAddress(
        [marketPublicKey.toBuffer(), nonce.toArrayLike(Buffer, "le", 8)],
        dexProgramId
      );
      return [vaultOwner, nonce];
    } catch (e) {
      nonce.iaddn(1);
    }
  }
  throw new Error("Unable to find nonce");
}

module.exports = {
  fundAccount,
  setupMarket,
  initOrderbook,
  setupTwoMarkets,
  DEX_PID,
  getVaultOwnerAndNonce,
};
