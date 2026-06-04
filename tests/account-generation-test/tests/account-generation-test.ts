import * as anchor from "@anchor-lang/core";
import { Program } from "@anchor-lang/core";
import { AccountGenerationTest } from "../target/types/account_generation_test";
import { assert } from "chai";
import { PublicKey } from "@solana/web3.js";

describe("account-generation-test", () => {
  anchor.setProvider(anchor.AnchorProvider.env());

  const program = anchor.workspace
    .AccountGenerationTest as Program<AccountGenerationTest>;
  const provider = anchor.getProvider() as anchor.AnchorProvider;

  const FUNDED_ACCOUNT_1 = new PublicKey(
    "9WzDXwBbmkg8ZTbNMqUxvQRAyrZzDsGYdLVL9zYtAWWM"
  );
  const FUNDED_ACCOUNT_2 = new PublicKey(
    "7xKXtg2CW87d97TXJSDpbD5jBkheTqA83TZRuJosgAsU"
  );
  const NEW_FUNDED_ACCOUNT_TARGETS = [15_000_000_000_000, 20_000_000_000_000];
  const EXPECTED_MINT_COUNT = 3;
  const EXPECTED_TOKEN_ACCOUNT_COUNT = 3;

  it("Funded accounts should have correct lamports", async () => {
    const account1Info = await provider.connection.getAccountInfo(
      FUNDED_ACCOUNT_1
    );
    assert.isNotNull(account1Info, "Funded account 1 should exist");

    assert.isTrue(
      account1Info!.lamports === 2000000000,
      `Account 1 should have exactly 2 SOL (has ${
        account1Info!.lamports
      } lamports)`
    );

    const account2Info = await provider.connection.getAccountInfo(
      FUNDED_ACCOUNT_2
    );
    assert.isNotNull(account2Info, "Funded account 2 should exist");

    assert.isTrue(
      account2Info!.lamports === 1000000000,
      `Account 2 should have exactly 1 SOL (has ${
        account2Info!.lamports
      } lamports)`
    );
  });

  it("Funded accounts should be usable for transactions", async () => {
    const account1Info = await provider.connection.getAccountInfo(
      FUNDED_ACCOUNT_1
    );
    assert.isNotNull(account1Info, "Funded account should exist");
    assert.isTrue(
      account1Info!.lamports > 0,
      "Funded account should have lamports"
    );

    assert.equal(
      account1Info!.owner.toBase58(),
      "11111111111111111111111111111111",
      "Account should be owned by system program"
    );
  });

  it("Provider wallet should be funded by validator", async () => {
    const walletBalance = await provider.connection.getBalance(
      provider.wallet.publicKey
    );
    assert.isTrue(
      walletBalance > 0,
      "Provider wallet should have lamports from validator"
    );
    assert.isTrue(
      walletBalance >= 1_000_000_000,
      "Provider wallet should have at least 1 SOL"
    );
  });

  const loadGeneratedFundedAccounts = async () => {
    const fs = require("fs");
    const path = require("path");
    const accountsDir = path.join(
      __dirname,
      "..",
      ".anchor",
      "generated_accounts"
    );

    const files = fs.readdirSync(accountsDir);
    const keypairFilesWithTimes = files
      .filter(
        (f: string) =>
          f.endsWith(".keypair.json") &&
          !f.endsWith(".token_account.json") &&
          !f.endsWith(".owner.json") &&
          !f.endsWith(".mint.json") &&
          f.length >= 56
      )
      .map((f: string) => {
        const filePath = path.join(accountsDir, f);
        const stats = fs.statSync(filePath);
        return { name: f, mtime: stats.mtime.getTime() };
      })
      .sort((a: { mtime: number }, b: { mtime: number }) => b.mtime - a.mtime);

    const generatedAccounts: Array<{ pubkey: PublicKey; lamports: number }> =
      [];
    for (const entry of keypairFilesWithTimes.slice(
      0,
      NEW_FUNDED_ACCOUNT_TARGETS.length
    )) {
      const pubkey = new PublicKey(entry.name.replace(".keypair.json", ""));
      const accountInfo = await provider.connection.getAccountInfo(pubkey);
      assert.isNotNull(accountInfo, `Generated account ${pubkey} should exist`);
      generatedAccounts.push({
        pubkey,
        lamports: accountInfo!.lamports,
      });
    }

    return generatedAccounts;
  };

  it("Generated 'new' accounts should exist and keep distinct funding targets", async () => {
    const generatedAccounts = await loadGeneratedFundedAccounts();
    const sortedLamports = generatedAccounts
      .map((account) => account.lamports)
      .sort((a, b) => a - b);

    assert.equal(
      generatedAccounts.length,
      NEW_FUNDED_ACCOUNT_TARGETS.length,
      "Should have one generated keypair per 'new' funded account"
    );
    assert.equal(
      new Set(generatedAccounts.map((account) => account.pubkey.toBase58()))
        .size,
      NEW_FUNDED_ACCOUNT_TARGETS.length,
      "Each generated funded account should resolve to a distinct pubkey"
    );
    NEW_FUNDED_ACCOUNT_TARGETS.forEach((targetLamports, index) => {
      assert.equal(
        sortedLamports[index],
        targetLamports,
        `Generated account ${index} should have exactly ${targetLamports} lamports`
      );
    });
  });

  const loadAllMints = async () => {
    const fs = require("fs");
    const path = require("path");
    const accountsDir = path.join(
      __dirname,
      "..",
      ".anchor",
      "generated_accounts"
    );
    const mintFiles = fs
      .readdirSync(accountsDir)
      .filter((f: string) => f.endsWith(".mint.json"));
    const parsed: Array<{
      pubkey: PublicKey;
      owner: PublicKey;
      decimals: number;
      supply: bigint;
      mintAuthorityIsSome: boolean;
      freezeAuthorityIsSome: boolean;
      raw: Buffer;
    }> = [];
    for (const f of mintFiles) {
      const pubkey = new PublicKey(f.replace(".mint.json", ""));
      const info = await provider.connection.getAccountInfo(pubkey);
      if (!info || info.data.length < 82) continue;
      const data = info.data;
      const mintAuthorityIsSome =
        Buffer.from(data.slice(0, 4)).readUInt32LE(0) === 1;
      const supply = Buffer.from(data.slice(36, 44)).readBigUInt64LE(0);
      const decimals = data[44];
      const freezeAuthorityIsSome =
        Buffer.from(data.slice(46, 50)).readUInt32LE(0) === 1;
      parsed.push({
        pubkey,
        owner: info.owner,
        decimals,
        supply,
        mintAuthorityIsSome,
        freezeAuthorityIsSome,
        raw: data,
      });
    }
    return parsed;
  };

  it("Generated mint should exist and be initialized", async () => {
    const mints = await loadAllMints();
    assert.isAtLeast(
      mints.length,
      EXPECTED_MINT_COUNT,
      "Generated mints should be loaded on-chain"
    );
    assert.equal(
      mints[0].owner.toBase58(),
      "TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA",
      "Mint should be owned by SPL Token Program"
    );
    assert.isTrue(
      mints[0].raw.length >= 82,
      `Mint account data should be at least 82 bytes (has ${mints[0].raw.length})`
    );
  });

  it("Generated token account should exist and be initialized", async () => {
    const all = await loadAllTokenAccounts();
    assert.isAtLeast(
      all.length,
      EXPECTED_TOKEN_ACCOUNT_COUNT,
      "Generated token accounts should be loaded on-chain"
    );
    assert.isTrue(
      all[0].raw.length >= 165,
      `Token account data should be at least 165 bytes (has ${all[0].raw.length})`
    );
  });

  it("Should fund account with specific address and lamports", async () => {
    const pubkey = new PublicKey(
      "9WzDXwBbmkg8ZTbNMqUxvQRAyrZzDsGYdLVL9zYtAWWM"
    );
    const accountInfo = await provider.connection.getAccountInfo(pubkey);
    assert.isNotNull(accountInfo);
    assert.equal(accountInfo!.lamports, 2_000_000_000);
  });

  it("Should fund account with specific address without lamports (defaults to 1 SOL)", async () => {
    const pubkey = new PublicKey(
      "7xKXtg2CW87d97TXJSDpbD5jBkheTqA83TZRuJosgAsU"
    );
    const accountInfo = await provider.connection.getAccountInfo(pubkey);
    assert.isNotNull(accountInfo);
    assert.equal(accountInfo!.lamports, 1_000_000_000);
  });

  it("Should create multiple mints with different configurations", async () => {
    const fs = require("fs");
    const path = require("path");
    const accountsDir = path.join(
      __dirname,
      "..",
      ".anchor",
      "generated_accounts"
    );
    const files = fs.readdirSync(accountsDir);
    const mintFiles = files.filter((f: string) => f.endsWith(".mint.json"));
    assert.isTrue(mintFiles.length >= 3);
  });

  it("Should create mint with mint_authority and freeze_authority", async () => {
    const mints = await loadAllMints();
    const match = mints.find(
      (m) => m.mintAuthorityIsSome && m.freezeAuthorityIsSome
    );
    assert.isDefined(
      match,
      "expected a mint with both mint_authority and freeze_authority set"
    );
    assert.equal(
      match!.owner.toBase58(),
      "TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA"
    );
  });

  it("Should create mint without supply (defaults to 0)", async () => {
    const mints = await loadAllMints();
    const match = mints.find((m) => m.supply === 0n);
    assert.isDefined(match, "expected a mint with supply = 0");
    assert.equal(match!.decimals, 8);
  });

  const loadAllTokenAccounts = async () => {
    const fs = require("fs");
    const path = require("path");
    const accountsDir = path.join(
      __dirname,
      "..",
      ".anchor",
      "generated_accounts"
    );
    const tokenAccountFiles = fs
      .readdirSync(accountsDir)
      .filter((f: string) => f.endsWith(".token_account.json"));
    const parsed: Array<{
      pubkey: PublicKey;
      owner: PublicKey;
      amount: bigint;
      raw: Buffer;
    }> = [];
    for (const f of tokenAccountFiles) {
      const pubkey = new PublicKey(f.replace(".token_account.json", ""));
      const info = await provider.connection.getAccountInfo(pubkey);
      if (!info) continue;
      const data = info.data;
      const owner = new PublicKey(data.slice(32, 64));
      const amount = Buffer.from(data.slice(64, 72)).readBigUInt64LE(0);
      parsed.push({ pubkey, owner, amount, raw: data });
    }
    return parsed;
  };

  it("Should create token account with mint=new owner=new", async () => {
    const all = await loadAllTokenAccounts();
    const match = all.find((ta) => ta.amount === 500000000n);
    assert.isDefined(match, "expected a token_account with amount=500000000");
    assert.isTrue(match!.raw.length >= 165);
  });

  it("Should create token account with mint=new owner=specific", async () => {
    const all = await loadAllTokenAccounts();
    const match = all.find(
      (ta) =>
        ta.owner.toBase58() === "9WzDXwBbmkg8ZTbNMqUxvQRAyrZzDsGYdLVL9zYtAWWM"
    );
    assert.isDefined(match, "expected a token_account with the specific owner");
    assert.equal(match!.amount.toString(), "1000000000");
  });

  it("Should create token account with mint=new owner=new address=new", async () => {
    const all = await loadAllTokenAccounts();
    const match = all.find((ta) => ta.amount === 250000000n);
    assert.isDefined(match, "expected a token_account with amount=250000000");
  });

  it("Should save owner keypairs when owner=new", async () => {
    const fs = require("fs");
    const path = require("path");
    const accountsDir = path.join(
      __dirname,
      "..",
      ".anchor",
      "generated_accounts"
    );
    const files = fs.readdirSync(accountsDir);
    const ownerFiles = files.filter((f: string) => f.endsWith(".owner.json"));
    assert.isTrue(ownerFiles.length >= 2);
  });

  it("Should use most recent mint when mint=new", async () => {
    const mints = await loadAllMints();
    const tokenAccounts = await loadAllTokenAccounts();
    const lastMint = mints.find((m) => m.decimals === 8 && m.supply === 0n);
    assert.isDefined(
      lastMint,
      "expected mints[2] (decimals=8, supply=0) on-chain"
    );
    for (const ta of tokenAccounts) {
      const taMintPubkey = new PublicKey(ta.raw.slice(0, 32));
      assert.equal(taMintPubkey.toBase58(), lastMint!.pubkey.toBase58());
    }
  });

  it("Should create accounts with correct rent-exempt lamports", async () => {
    const fs = require("fs");
    const path = require("path");
    const accountsDir = path.join(
      __dirname,
      "..",
      ".anchor",
      "generated_accounts"
    );
    const files = fs.readdirSync(accountsDir);
    const mintFiles = files.filter((f: string) => f.endsWith(".mint.json"));
    const tokenAccountFiles = files.filter((f: string) =>
      f.endsWith(".token_account.json")
    );
    if (mintFiles.length > 0) {
      const mintFile = mintFiles[0];
      const mintPubkeyStr = mintFile.replace(".mint.json", "");
      const mintPubkey = new PublicKey(mintPubkeyStr);
      const mintInfo = await provider.connection.getAccountInfo(mintPubkey);
      if (mintInfo) {
        assert.isTrue(mintInfo.lamports >= 1_461_600);
      }
    }
    if (tokenAccountFiles.length > 0) {
      const tokenAccountFile = tokenAccountFiles[0];
      const tokenAccountPubkeyStr = tokenAccountFile.replace(
        ".token_account.json",
        ""
      );
      const tokenAccountPubkey = new PublicKey(tokenAccountPubkeyStr);
      const tokenAccountInfo = await provider.connection.getAccountInfo(
        tokenAccountPubkey
      );
      if (tokenAccountInfo) {
        assert.isTrue(tokenAccountInfo.lamports >= 2_039_280);
      }
    }
  });

  it("Should handle multiple token accounts referencing same mint", async () => {
    const tokenAccounts = await loadAllTokenAccounts();
    assert.isAtLeast(
      tokenAccounts.length,
      2,
      "expected multiple token accounts to be loaded on-chain"
    );
    const uniqueMints = new Set(
      tokenAccounts.map((ta) => new PublicKey(ta.raw.slice(0, 32)).toBase58())
    );
    assert.equal(
      uniqueMints.size,
      1,
      "all token accounts (with mint=new) should reference the same mint"
    );
  });

  it("Should create all account JSON files", async () => {
    const fs = require("fs");
    const path = require("path");
    const accountsDir = path.join(
      __dirname,
      "..",
      ".anchor",
      "generated_accounts"
    );
    const files = fs.readdirSync(accountsDir);
    const accountJsonFiles = files.filter(
      (f: string) =>
        f.endsWith(".json") &&
        !f.endsWith(".keypair.json") &&
        !f.endsWith(".mint.json") &&
        !f.endsWith(".token_account.json") &&
        !f.endsWith(".owner.json")
    );
    assert.isTrue(accountJsonFiles.length >= 5);
  });

  it("Should verify mint supply matches configuration", async () => {
    const mints = await loadAllMints();
    assert.isAtLeast(
      mints.length,
      EXPECTED_MINT_COUNT,
      "Generated mints should be loaded on-chain"
    );
    const match = mints.find((m) => m.supply === 1_000_000_000n);
    assert.isDefined(match, "expected a mint with supply=1_000_000_000");
    assert.equal(match!.decimals, 9);
  });

  it("Should verify mint decimals match configuration", async () => {
    const mints = await loadAllMints();
    assert.isAtLeast(
      mints.length,
      EXPECTED_MINT_COUNT,
      "Generated mints should be loaded on-chain"
    );
    const match = mints.find((m) => m.decimals === 6);
    assert.isDefined(match, "expected a mint with decimals=6");
    assert.equal(match!.supply.toString(), "500000000");
  });

  it("Should support specific pubkey address for mints", async () => {
    const specificMintPubkey = new PublicKey(
      "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v"
    );
    const mintInfo = await provider.connection.getAccountInfo(
      specificMintPubkey
    );
    if (mintInfo) {
      assert.equal(
        mintInfo.owner.toBase58(),
        "TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA"
      );
      assert.isTrue(mintInfo.data.length >= 82);
      const decimals = mintInfo.data[44];
      assert.equal(decimals, 6);
      const supplyBytes = mintInfo.data.slice(36, 44);
      const supply = Buffer.from(supplyBytes).readBigUInt64LE(0);
      assert.equal(supply.toString(), "1000000");
    }
  });
});
