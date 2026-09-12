const { assert } = require("chai");
const { Token } = require("@solana/spl-token");
const anchor = require("@anchor-lang/core");
const serumCmn = require("@project-serum/common");
const { Market } = require("@project-serum/serum");
const utf8 = anchor.utils.bytes.utf8;
const { PublicKey, SystemProgram, Keypair, SYSVAR_RENT_PUBKEY } = anchor.web3;
const utils = require("./utils");
const { setupStakePool } = require("./utils/stake");

const DEX_PID = new PublicKey("srmqPvymJeFKQ4zGQed1GFppgkRHL9kaELCbyksJtPX");
const SWAP_PID = new PublicKey("22Y43yTVxuUkoRKdm9thyRhQ3SdgQS7c7kB6UNCiaczD");
const TOKEN_PID = new PublicKey("TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA");
const REGISTRY_PID = new PublicKey(
  "GrAkKfEpTKQuVHG2Y97Y2FF4i7y7Q5AHLK94JBy7Y5yv"
);
const LOCKUP_PID = new PublicKey(
  "6ebQNeTPZ1j7k3TtkCCtEPRvG7GQsucQrZ7sSEDQi9Ks"
);
const SYSVAR_INSTRUCTIONS_PUBKEY = new PublicKey(
  "Sysvar1nstructions1111111111111111111111111"
);
const FEES = "6160355581";

describe("cfo", () => {
  const provider = anchor.AnchorProvider.env();
  anchor.setProvider(provider);

  const program = anchor.workspace.Cfo;
  // hack so we don't have to update serum-common library
  // to the new AnchorProvider class and Provider interface
  program.provider.send = provider.sendAndConfirm;
  const sweepAuthority = program.provider.wallet.publicKey;
  let officer, srmVault, usdcVault, bVault, stake, treasury;
  let officerBump, srmBump, usdcBump, bBump, stakeBump, treasuryBump;
  let openOrders, openOrdersBump;
  let openOrdersB, openOrdersBumpB;
  let USDC_TOKEN_CLIENT, A_TOKEN_CLIENT, B_TOKEN_CLIENT;
  let officerAccount;
  let marketAClient, marketBClient;
  let marketAuth, marketAuthBump;
  let marketAuthB, marketAuthBumpB;
  let distribution;

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

  let registrar, msrmRegistrar;

  it("BOILERPLATE: Sets up a market with funded fees", async () => {
    ORDERBOOK_ENV = await utils.initMarket({
      provider: program.provider,
    });
    console.log("Token A: ", ORDERBOOK_ENV.marketA.baseMintAddress.toString());
    console.log(
      "Token USDC: ",
      ORDERBOOK_ENV.marketA.quoteMintAddress.toString()
    );
    USDC_TOKEN_CLIENT = new Token(
      program.provider.connection,
      ORDERBOOK_ENV.usdc,
      TOKEN_PID,
      program.provider.wallet.payer
    );
    SRM_TOKEN_CLIENT = new Token(
      program.provider.connection,
      ORDERBOOK_ENV.mintA,
      TOKEN_PID,
      program.provider.wallet.payer
    );
    B_TOKEN_CLIENT = new Token(
      program.provider.connection,
      ORDERBOOK_ENV.mintB,
      TOKEN_PID,
      program.provider.wallet.payer
    );

    await USDC_TOKEN_CLIENT.transfer(
      ORDERBOOK_ENV.godUsdc,
      ORDERBOOK_ENV.marketA._decoded.quoteVault,
      program.provider.wallet.payer,
      [],
      10000000000000
    );

    const tokenAccount = await USDC_TOKEN_CLIENT.getAccountInfo(
      ORDERBOOK_ENV.marketA._decoded.quoteVault
    );
    assert.strictEqual(tokenAccount.amount.toString(), "10000902263700");
  });

  it("BOILERPLATE: Executes trades to generate fees", async () => {
    await utils.runTradeBot(
      ORDERBOOK_ENV.marketA._decoded.ownAddress,
      program.provider,
      1
    );
    marketAClient = await Market.load(
      program.provider.connection,
      ORDERBOOK_ENV.marketA.address,
      { commitment: "processed" },
      DEX_PID
    );
    marketBClient = await Market.load(
      program.provider.connection,
      ORDERBOOK_ENV.marketB.address,
      { commitment: "processed" },
      DEX_PID
    );
    assert.strictEqual(
      marketAClient._decoded.quoteFeesAccrued.toString(),
      FEES
    );
  });

  it("BOILERPLATE: Sets up the staking pools", async () => {
    await setupStakePool(ORDERBOOK_ENV.mintA, ORDERBOOK_ENV.godA);
    registrar = ORDERBOOK_ENV.usdc;
    msrmRegistrar = registrar;
  });

  it("BOILERPLATE: Finds PDA addresses", async () => {
    const [_officer, _officerBump] = await PublicKey.findProgramAddress(
      [DEX_PID.toBuffer()],
      program.programId
    );
    const [_openOrders, _openOrdersBump] = await PublicKey.findProgramAddress(
      [
        utf8.encode("open-orders"),
        _officer.toBuffer(),
        ORDERBOOK_ENV.marketA.address.toBuffer(),
      ],
      program.programId
    );
    const [_openOrdersB, _openOrdersBumpB] = await PublicKey.findProgramAddress(
      [
        utf8.encode("open-orders"),
        _officer.toBuffer(),
        ORDERBOOK_ENV.marketB.address.toBuffer(),
      ],
      program.programId
    );
    const [_srmVault, _srmBump] = await PublicKey.findProgramAddress(
      [
        utf8.encode("token"),
        _officer.toBuffer(),
        ORDERBOOK_ENV.mintA.toBuffer(),
      ],
      program.programId
    );
    const [_bVault, _bBump] = await PublicKey.findProgramAddress(
      [
        utf8.encode("token"),
        _officer.toBuffer(),
        ORDERBOOK_ENV.mintB.toBuffer(),
      ],
      program.programId
    );
    const [_usdcVault, _usdcBump] = await PublicKey.findProgramAddress(
      [
        utf8.encode("token"),
        _officer.toBuffer(),
        ORDERBOOK_ENV.usdc.toBuffer(),
      ],
      program.programId
    );
    const [_stake, _stakeBump] = await PublicKey.findProgramAddress(
      [utf8.encode("stake"), _officer.toBuffer()],
      program.programId
    );
    const [_treasury, _treasuryBump] = await PublicKey.findProgramAddress(
      [utf8.encode("treasury"), _officer.toBuffer()],
      program.programId
    );
    const [_marketAuth, _marketAuthBump] = await PublicKey.findProgramAddress(
      [
        utf8.encode("market-auth"),
        _officer.toBuffer(),
        ORDERBOOK_ENV.marketA.address.toBuffer(),
      ],
      program.programId
    );
    const [_marketAuthB, _marketAuthBumpB] = await PublicKey.findProgramAddress(
      [
        utf8.encode("market-auth"),
        _officer.toBuffer(),
        ORDERBOOK_ENV.marketB.address.toBuffer(),
      ],
      program.programId
    );

    officer = _officer;
    officerBump = _officerBump;
    openOrders = _openOrders;
    openOrdersBump = _openOrdersBump;
    openOrdersB = _openOrdersB;
    openOrdersBumpB = _openOrdersBumpB;
    srmVault = _srmVault;
    srmBump = _srmBump;
    usdcVault = _usdcVault;
    usdcBump = _usdcBump;
    bVault = _bVault;
    bBump = _bBump;
    stake = _stake;
    stakeBump = _stakeBump;
    treasury = _treasury;
    treasuryBump = _treasuryBump;
    marketAuth = _marketAuth;
    marketAuthBump = _marketAuthBump;
    marketAuthB = _marketAuthB;
    marketAuthBumpB = _marketAuthBumpB;
  });

  it("Creates a CFO!", async () => {
    distribution = {
      burn: 80,
      stake: 20,
      treasury: 0,
    };
    const bumps = {
      bump: officerBump,
      srm: srmBump,
      usdc: usdcBump,
      stake: stakeBump,
      treasury: treasuryBump,
    };
    await program.methods
      .createOfficer(bumps, distribution, registrar, msrmRegistrar)
      .accounts({
        officer,
        srmVault,
        usdcVault,
        stake,
        treasury,
        srmMint: ORDERBOOK_ENV.mintA,
        usdcMint: ORDERBOOK_ENV.usdc,
        authority: program.provider.wallet.publicKey,
        dexProgram: DEX_PID,
        swapProgram: SWAP_PID,
        tokenProgram: TOKEN_PID,
        systemProgram: SystemProgram.programId,
        rent: SYSVAR_RENT_PUBKEY,
      })
      .rpc();

    officerAccount = await program.account.officer.fetch(officer);
    assert.isTrue(
      officerAccount.authority.equals(program.provider.wallet.publicKey)
    );
    assert.strictEqual(
      JSON.stringify(officerAccount.distribution),
      JSON.stringify(distribution)
    );
  });

  it("Creates a token account for the officer associated with the market", async () => {
    await program.methods
      .createOfficerToken(bBump)
      .accounts({
        officer,
        token: bVault,
        mint: ORDERBOOK_ENV.mintB,
        payer: program.provider.wallet.publicKey,
        systemProgram: SystemProgram.programId,
        tokenProgram: TOKEN_PID,
        rent: SYSVAR_RENT_PUBKEY,
      })
      .rpc();
    const tokenAccount = await B_TOKEN_CLIENT.getAccountInfo(bVault);
    assert.strictEqual(tokenAccount.state, 1);
    assert.isTrue(tokenAccount.isInitialized);
  });

  it("Creates an open orders account for the officer", async () => {
    await program.methods
      .createOfficerOpenOrders(openOrdersBump)
      .accounts({
        officer,
        openOrders,
        payer: program.provider.wallet.publicKey,
        dexProgram: DEX_PID,
        systemProgram: SystemProgram.programId,
        rent: SYSVAR_RENT_PUBKEY,
        market: ORDERBOOK_ENV.marketA.address,
      })
      .rpc();
    await program.rpc.createOfficerOpenOrders(openOrdersBumpB, {
      accounts: {
        officer,
        openOrders: openOrdersB,
        payer: program.provider.wallet.publicKey,
        dexProgram: DEX_PID,
        systemProgram: SystemProgram.programId,
        rent: SYSVAR_RENT_PUBKEY,
        market: ORDERBOOK_ENV.marketB.address,
      },
    });
  });

  it("Sweeps fees", async () => {
    const [sweepVault, bump] = await PublicKey.findProgramAddress(
      [utf8.encode("token"), officer.toBuffer(), ORDERBOOK_ENV.usdc.toBuffer()],
      program.programId
    );
    const beforeTokenAccount = await serumCmn.getTokenAccount(
      program.provider,
      sweepVault
    );
    await program.methods
      .sweepFees()
      .accounts({
        officer,
        sweepVault,
        mint: ORDERBOOK_ENV.usdc,
        dex: {
          market: ORDERBOOK_ENV.marketA._decoded.ownAddress,
          pcVault: ORDERBOOK_ENV.marketA._decoded.quoteVault,
          sweepAuthority,
          vaultSigner: ORDERBOOK_ENV.marketAVaultSigner,
          dexProgram: DEX_PID,
          tokenProgram: TOKEN_PID,
        },
      })
      .rpc();
    const afterTokenAccount = await serumCmn.getTokenAccount(
      program.provider,
      sweepVault
    );
    assert.strictEqual(
      afterTokenAccount.amount.sub(beforeTokenAccount.amount).toString(),
      FEES
    );
  });

  it("Creates a market auth token", async () => {
    await program.methods
      .authorizeMarket(marketAuthBump)
      .accounts({
        officer,
        authority: program.provider.wallet.publicKey,
        marketAuth,
        payer: program.provider.wallet.publicKey,
        market: ORDERBOOK_ENV.marketA.address,
        systemProgram: SystemProgram.programId,
      })
      .rpc();
    await program.methods
      .authorizeMarket(marketAuthBumpB)
      .accounts({
        officer,
        authority: program.provider.wallet.publicKey,
        marketAuth: marketAuthB,
        payer: program.provider.wallet.publicKey,
        market: ORDERBOOK_ENV.marketB.address,
        systemProgram: SystemProgram.programId,
      })
      .rpc();
  });

  it("Transfers into the mintB vault", async () => {
    await B_TOKEN_CLIENT.transfer(
      ORDERBOOK_ENV.godB,
      bVault,
      program.provider.wallet.payer,
      [],
      616035558100
    );
  });

  it("Swaps from B token to USDC", async () => {
    const bVaultBefore = await B_TOKEN_CLIENT.getAccountInfo(bVault);
    const usdcVaultBefore = await USDC_TOKEN_CLIENT.getAccountInfo(usdcVault);

    const minExchangeRate = {
      rate: new anchor.BN(0),
      fromDecimals: 6,
      quoteDecimals: 6,
      strict: false,
    };
    await program.methods
      .swapToUsdc(minExchangeRate)
      .accounts({
        officer,
        market: {
          market: marketBClient.address,
          openOrders: openOrdersB,
          requestQueue: marketBClient.decoded.requestQueue,
          eventQueue: marketBClient.decoded.eventQueue,
          bids: marketBClient.decoded.bids,
          asks: marketBClient.decoded.asks,
          orderPayerTokenAccount: bVault,
          coinVault: marketBClient.decoded.baseVault,
          pcVault: marketBClient.decoded.quoteVault,
          vaultSigner: ORDERBOOK_ENV.marketBVaultSigner,
        },
        marketAuth: marketAuthB,
        usdcVault,
        fromVault: bVault,
        usdcMint: ORDERBOOK_ENV.usdc,
        swapProgram: SWAP_PID,
        dexProgram: DEX_PID,
        tokenProgram: TOKEN_PID,
        instructions: SYSVAR_INSTRUCTIONS_PUBKEY,
        rent: SYSVAR_RENT_PUBKEY,
      })
      .rpc();

    const bVaultAfter = await B_TOKEN_CLIENT.getAccountInfo(bVault);
    const usdcVaultAfter = await USDC_TOKEN_CLIENT.getAccountInfo(usdcVault);

    assert.strictEqual(bVaultBefore.amount.toNumber(), 616035558100);
    assert.strictEqual(usdcVaultBefore.amount.toNumber(), 6160355581);
    assert.strictEqual(bVaultAfter.amount.toNumber(), 615884458100);
    assert.strictEqual(usdcVaultAfter.amount.toNumber(), 7060634298);
  });

  it("Swaps to SRM", async () => {
    const srmVaultBefore = await SRM_TOKEN_CLIENT.getAccountInfo(srmVault);
    const usdcVaultBefore = await USDC_TOKEN_CLIENT.getAccountInfo(usdcVault);

    const minExchangeRate = {
      rate: new anchor.BN(0),
      fromDecimals: 6,
      quoteDecimals: 6,
      strict: false,
    };
    await program.methods
      .swapToSrm(minExchangeRate)
      .accounts({
        officer,
        market: {
          market: marketAClient.address,
          openOrders,
          requestQueue: marketAClient.decoded.requestQueue,
          eventQueue: marketAClient.decoded.eventQueue,
          bids: marketAClient.decoded.bids,
          asks: marketAClient.decoded.asks,
          orderPayerTokenAccount: usdcVault,
          coinVault: marketAClient.decoded.baseVault,
          pcVault: marketAClient.decoded.quoteVault,
          vaultSigner: ORDERBOOK_ENV.marketAVaultSigner,
        },
        marketAuth,
        usdcVault,
        srmVault,
        usdcMint: ORDERBOOK_ENV.usdc,
        srmMint: ORDERBOOK_ENV.mintA,
        swapProgram: SWAP_PID,
        dexProgram: DEX_PID,
        tokenProgram: TOKEN_PID,
        instructions: SYSVAR_INSTRUCTIONS_PUBKEY,
        rent: SYSVAR_RENT_PUBKEY,
      })
      .rpc();

    const srmVaultAfter = await SRM_TOKEN_CLIENT.getAccountInfo(srmVault);
    const usdcVaultAfter = await USDC_TOKEN_CLIENT.getAccountInfo(usdcVault);

    assert.strictEqual(srmVaultBefore.amount.toNumber(), 0);
    assert.strictEqual(srmVaultAfter.amount.toNumber(), 1152000000);
    assert.strictEqual(usdcVaultBefore.amount.toNumber(), 7060634298);
    assert.strictEqual(usdcVaultAfter.amount.toNumber(), 530863);
  });

  it("Distributes the tokens to categories", async () => {
    const srmVaultBefore = await SRM_TOKEN_CLIENT.getAccountInfo(srmVault);
    const treasuryBefore = await SRM_TOKEN_CLIENT.getAccountInfo(treasury);
    const stakeBefore = await SRM_TOKEN_CLIENT.getAccountInfo(stake);
    const mintInfoBefore = await SRM_TOKEN_CLIENT.getMintInfo();

    await program.methods
      .distribute()
      .accounts({
        officer,
        treasury,
        stake,
        srmVault,
        srmMint: ORDERBOOK_ENV.mintA,
        tokenProgram: TOKEN_PID,
        dexProgram: DEX_PID,
      })
      .rpc();

    const srmVaultAfter = await SRM_TOKEN_CLIENT.getAccountInfo(srmVault);
    const treasuryAfter = await SRM_TOKEN_CLIENT.getAccountInfo(treasury);
    const stakeAfter = await SRM_TOKEN_CLIENT.getAccountInfo(stake);
    const mintInfoAfter = await SRM_TOKEN_CLIENT.getMintInfo();

    const beforeAmount = 1152000000;
    assert.strictEqual(srmVaultBefore.amount.toNumber(), beforeAmount);
    assert.strictEqual(srmVaultAfter.amount.toNumber(), 0); // Fully distributed.
    assert.strictEqual(
      stakeAfter.amount.toNumber(),
      beforeAmount * (distribution.stake / 100.0)
    );
    assert.strictEqual(
      treasuryAfter.amount.toNumber(),
      beforeAmount * (distribution.treasury / 100.0)
    );
    // Check burn amount.
    assert.strictEqual(mintInfoBefore.supply.toString(), "1000000000000000000");
    assert.strictEqual(
      mintInfoBefore.supply.sub(mintInfoAfter.supply).toNumber(),
      beforeAmount * (distribution.burn / 100.0)
    );
  });
});
