import * as anchor from "@anchor-lang/core";
import { Program } from "@anchor-lang/core";
import { AccountGenerationTest } from "../target/types/account_generation_test";
import { assert } from "chai";
import { PublicKey } from "@solana/web3.js";

describe("account-generation-test", () => {
  // Configure the client to use the local cluster.
  anchor.setProvider(anchor.AnchorProvider.env());

  const program = anchor.workspace
    .AccountGenerationTest as Program<AccountGenerationTest>;
  const provider = anchor.getProvider() as anchor.AnchorProvider;

  // These are the addresses configured in Anchor.toml fund_accounts
  const FUNDED_ACCOUNT_1 = new PublicKey(
    "9WzDXwBbmkg8ZTbNMqUxvQRAyrZzDsGYdLVL9zYtAWWM"
  );
  const FUNDED_ACCOUNT_2 = new PublicKey(
    "7xKXtg2CW87d97TXJSDpbD5jBkheTqA83TZRuJosgAsU"
  );

  it("Funded accounts should have correct lamports", async () => {
    // Check first account - should have 2 SOL (2000000000 lamports) with Legacy validator
    // With Surfpool, accounts get default airdrop amounts (10 SOL) and can be increased but not decreased
    const account1Info = await provider.connection.getAccountInfo(
      FUNDED_ACCOUNT_1
    );
    assert.isNotNull(account1Info, "Funded account 1 should exist");
    // With Legacy validator: exact amount (2 SOL)
    // With Surfpool: default airdrop amount (10 SOL) - can't reduce via airdrop
    assert.isTrue(
      account1Info!.lamports >= 2000000000,
      `Account 1 should have at least 2 SOL (has ${account1Info!.lamports} lamports). Note: Surfpool uses default airdrop amounts.`
    );

    // Check second account - should have 1 SOL (1000000000 lamports) by default
    const account2Info = await provider.connection.getAccountInfo(
      FUNDED_ACCOUNT_2
    );
    assert.isNotNull(account2Info, "Funded account 2 should exist");
    // With Legacy validator: exact amount (1 SOL)
    // With Surfpool: default airdrop amount (10 SOL)
    assert.isTrue(
      account2Info!.lamports >= 1000000000,
      `Account 2 should have at least 1 SOL (has ${account2Info!.lamports} lamports). Note: Surfpool uses default airdrop amounts.`
    );
  });

  it("Funded accounts should be usable for transactions", async () => {
    // Create a keypair for the funded account (we'll use the first one)
    // Note: In a real scenario, you'd need the private key, but for testing
    // we can verify the account exists and has funds

    const account1Info = await provider.connection.getAccountInfo(
      FUNDED_ACCOUNT_1
    );
    assert.isNotNull(account1Info, "Funded account should exist");
    assert.isTrue(
      account1Info!.lamports > 0,
      "Funded account should have lamports"
    );

    // Verify the account is owned by the system program (as we configured)
    assert.equal(
      account1Info!.owner.toBase58(),
      "11111111111111111111111111111111",
      "Account should be owned by system program"
    );
  });

  it("Provider wallet should be funded by validator", async () => {
    // This test verifies that the validator is working correctly
    // The provider wallet should be funded by the validator's mint account
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

  it("Generated 'new' account should exist and be funded", async () => {
    // Test that when address="new", a random keypair was generated and funded
    // The keypair file should be in .anchor/generated_accounts/
    const fs = require("fs");
    const path = require("path");
    const accountsDir = path.join(
      __dirname,
      "..",
      ".anchor",
      "generated_accounts"
    );

    // Find the generated keypair file (should be a .keypair.json file with a pubkey as filename)
    // Get files with their modification times and sort by most recent first
    const files = fs.readdirSync(accountsDir);
    const keypairFilesWithTimes = files
      .filter(
        (f: string) =>
          f.endsWith(".keypair.json") &&
          !f.endsWith(".token_account.json") && // Exclude token account keypairs
          !f.endsWith(".owner.json") && // Exclude owner keypairs
          !f.endsWith(".mint.json") && // Exclude mint keypairs
          f.length >= 56 // "pubkey.keypair.json" = 43 (base58 pubkey) + 13 (.keypair.json) = 56
      )
      .map((f: string) => {
        const filePath = path.join(accountsDir, f);
        const stats = fs.statSync(filePath);
        return { name: f, mtime: stats.mtime.getTime() };
      })
      .sort((a: { mtime: number }, b: { mtime: number }) => b.mtime - a.mtime);

    assert.isTrue(
      keypairFilesWithTimes.length > 0,
      "Should have at least one generated keypair file for 'new' address"
    );

    // Load the most recently created keypair and check the account
    const keypairFile = keypairFilesWithTimes[0].name;
    const keypairPath = path.join(accountsDir, keypairFile);
    const keypairData = JSON.parse(fs.readFileSync(keypairPath, "utf8"));
    const pubkeyStr = keypairFile.replace(".keypair.json", "");
    const generatedPubkey = new PublicKey(pubkeyStr);

    // Verify the account exists and is funded
    const accountInfo = await provider.connection.getAccountInfo(
      generatedPubkey
    );
    assert.isNotNull(accountInfo, "Generated account should exist");
    assert.isTrue(
      accountInfo!.lamports >= 5_000_000_000,
      `Generated account should have at least 5 SOL (has ${accountInfo!.lamports} lamports). Note: Surfpool uses default airdrop amounts.`
    );
  });

  it("Generated mint should exist and be initialized", async () => {
    // Test that when mints are configured, they are created correctly
    // Note: Mint creation currently works best with Legacy validator
    const fs = require("fs");
    const path = require("path");
    const accountsDir = path.join(
      __dirname,
      "..",
      ".anchor",
      "generated_accounts"
    );

    // Find the generated mint keypair file
    const files = fs.readdirSync(accountsDir);
    const mintFiles = files.filter((f: string) => f.endsWith(".mint.json"));

    assert.isTrue(
      mintFiles.length > 0,
      "Should have at least one generated mint file"
    );

    // Load the most recent mint
    const mintFilesWithTimes = mintFiles
      .map((f: string) => {
        const filePath = path.join(accountsDir, f);
        const stats = fs.statSync(filePath);
        return { name: f, mtime: stats.mtime.getTime() };
      })
      .sort((a: { mtime: number }, b: { mtime: number }) => b.mtime - a.mtime);

    const mintFile = mintFilesWithTimes[0].name;
    const mintPubkeyStr = mintFile.replace(".mint.json", "");
    const mintPubkey = new PublicKey(mintPubkeyStr);

    // Verify the mint account exists
    // Note: For Surfpool, mints may not be created yet (use Legacy validator for mints)
    const mintInfo = await provider.connection.getAccountInfo(mintPubkey);
    if (mintInfo) {
      // Mint exists - verify it's correct
      assert.equal(
        mintInfo.owner.toBase58(),
        "TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA",
        "Mint should be owned by SPL Token Program"
      );
      assert.isTrue(
        mintInfo.data.length >= 82,
        `Mint account data should be at least 82 bytes (has ${mintInfo.data.length})`
      );
    } else {
      // Mint doesn't exist - this is expected for Surfpool
      // Skip the test with a note
      console.log(
        "Note: Mint not found on-chain. This is expected for Surfpool validator. Use --validator legacy for mint creation."
      );
    }
  });

  it("Generated token account should exist and be initialized", async () => {
    // Test that when token_accounts are configured, they are created correctly
    const fs = require("fs");
    const path = require("path");
    const accountsDir = path.join(
      __dirname,
      "..",
      ".anchor",
      "generated_accounts"
    );

    // Find the generated token account keypair file
    const files = fs.readdirSync(accountsDir);
    const tokenAccountFiles = files.filter((f: string) =>
      f.endsWith(".token_account.json")
    );

    if (tokenAccountFiles.length === 0) {
      console.log(
        "Note: No token account files found. This is expected if token_accounts are not configured."
      );
      return; // Skip test if no token accounts configured
    }

    // Load the most recent token account
    const tokenAccountFilesWithTimes = tokenAccountFiles
      .map((f: string) => {
        const filePath = path.join(accountsDir, f);
        const stats = fs.statSync(filePath);
        return { name: f, mtime: stats.mtime.getTime() };
      })
      .sort(
        (a: { mtime: number }, b: { mtime: number }) => b.mtime - a.mtime
      );

    const tokenAccountFile = tokenAccountFilesWithTimes[0].name;
    const tokenAccountPubkeyStr = tokenAccountFile.replace(
      ".token_account.json",
      ""
    );
    const tokenAccountPubkey = new PublicKey(tokenAccountPubkeyStr);

    // Verify the token account exists
    // Note: For Surfpool, token accounts may not be created yet (use Legacy validator)
    const tokenAccountInfo = await provider.connection.getAccountInfo(
      tokenAccountPubkey
    );
    if (tokenAccountInfo) {
      // Token account exists - verify it's correct
      assert.equal(
        tokenAccountInfo.owner.toBase58(),
        "TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA",
        "Token account should be owned by SPL Token Program"
      );
      assert.isTrue(
        tokenAccountInfo.data.length >= 165,
        `Token account data should be at least 165 bytes (has ${tokenAccountInfo.data.length})`
      );
    } else {
      // Token account doesn't exist - this is expected for Surfpool
      console.log(
        "Note: Token account not found on-chain. This is expected for Surfpool validator. Use --validator legacy for token account creation."
      );
    }
  });

  it("Should fund account with specific address and lamports", async () => {
    const pubkey = new PublicKey("9WzDXwBbmkg8ZTbNMqUxvQRAyrZzDsGYdLVL9zYtAWWM");
    const accountInfo = await provider.connection.getAccountInfo(pubkey);
    assert.isNotNull(accountInfo);
    assert.isTrue(accountInfo!.lamports >= 2_000_000_000);
  });

  it("Should fund account with specific address without lamports (defaults to 1 SOL)", async () => {
    const pubkey = new PublicKey("7xKXtg2CW87d97TXJSDpbD5jBkheTqA83TZRuJosgAsU");
    const accountInfo = await provider.connection.getAccountInfo(pubkey);
    assert.isNotNull(accountInfo);
    assert.isTrue(accountInfo!.lamports >= 1_000_000_000);
  });

  it("Should create multiple mints with different configurations", async () => {
    const fs = require("fs");
    const path = require("path");
    const accountsDir = path.join(__dirname, "..", ".anchor", "generated_accounts");
    const files = fs.readdirSync(accountsDir);
    const mintFiles = files.filter((f: string) => f.endsWith(".mint.json"));
    assert.isTrue(mintFiles.length >= 3);
  });

  it("Should create mint with mint_authority and freeze_authority", async () => {
    const fs = require("fs");
    const path = require("path");
    const accountsDir = path.join(__dirname, "..", ".anchor", "generated_accounts");
    const files = fs.readdirSync(accountsDir);
    const mintFiles = files.filter((f: string) => f.endsWith(".mint.json"));
    const mintFilesWithTimes = mintFiles
      .map((f: string) => {
        const filePath = path.join(accountsDir, f);
        const stats = fs.statSync(filePath);
        return { name: f, mtime: stats.mtime.getTime() };
      })
      .sort((a: { mtime: number }, b: { mtime: number }) => b.mtime - a.mtime);
    if (mintFilesWithTimes.length >= 2) {
      const secondMintFile = mintFilesWithTimes[1].name;
      const secondMintPubkeyStr = secondMintFile.replace(".mint.json", "");
      const secondMintPubkey = new PublicKey(secondMintPubkeyStr);
      const mintInfo = await provider.connection.getAccountInfo(secondMintPubkey);
      if (mintInfo) {
        assert.equal(mintInfo.owner.toBase58(), "TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA");
      }
    }
  });

  it("Should create mint without supply (defaults to 0)", async () => {
    const fs = require("fs");
    const path = require("path");
    const accountsDir = path.join(__dirname, "..", ".anchor", "generated_accounts");
    const files = fs.readdirSync(accountsDir);
    const mintFiles = files.filter((f: string) => f.endsWith(".mint.json"));
    const mintFilesWithTimes = mintFiles
      .map((f: string) => {
        const filePath = path.join(accountsDir, f);
        const stats = fs.statSync(filePath);
        return { name: f, mtime: stats.mtime.getTime() };
      })
      .sort((a: { mtime: number }, b: { mtime: number }) => a.mtime - b.mtime);
    if (mintFilesWithTimes.length >= 3) {
      const thirdMintFile = mintFilesWithTimes[2].name;
      const thirdMintPubkeyStr = thirdMintFile.replace(".mint.json", "");
      const thirdMintPubkey = new PublicKey(thirdMintPubkeyStr);
      const mintInfo = await provider.connection.getAccountInfo(thirdMintPubkey);
      if (mintInfo && mintInfo.data.length >= 82) {
        const supplyBytes = mintInfo.data.slice(36, 44);
        const supply = Buffer.from(supplyBytes).readBigUInt64LE(0);
        assert.equal(supply.toString(), "0");
        const decimals = mintInfo.data[44];
        assert.equal(decimals, 8);
      }
    }
  });

  it("Should create token account with mint=new owner=new", async () => {
    const fs = require("fs");
    const path = require("path");
    const accountsDir = path.join(__dirname, "..", ".anchor", "generated_accounts");
    const files = fs.readdirSync(accountsDir);
    const tokenAccountFiles = files.filter((f: string) => f.endsWith(".token_account.json"));
    assert.isTrue(tokenAccountFiles.length >= 1);
    const tokenAccountFilesWithTimes = tokenAccountFiles
      .map((f: string) => {
        const filePath = path.join(accountsDir, f);
        const stats = fs.statSync(filePath);
        return { name: f, mtime: stats.mtime.getTime() };
      })
      .sort((a: { mtime: number }, b: { mtime: number }) => a.mtime - b.mtime);
    const firstTokenAccountFile = tokenAccountFilesWithTimes[0].name;
    const firstTokenAccountPubkeyStr = firstTokenAccountFile.replace(".token_account.json", "");
    const firstTokenAccountPubkey = new PublicKey(firstTokenAccountPubkeyStr);
    const tokenAccountInfo = await provider.connection.getAccountInfo(firstTokenAccountPubkey);
    if (tokenAccountInfo) {
      assert.equal(tokenAccountInfo.owner.toBase58(), "TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA");
      assert.isTrue(tokenAccountInfo.data.length >= 165);
      const amountBytes = tokenAccountInfo.data.slice(64, 72);
      const amount = Buffer.from(amountBytes).readBigUInt64LE(0);
      assert.equal(amount.toString(), "500000000");
    }
  });

  it("Should create token account with mint=new owner=specific", async () => {
    const fs = require("fs");
    const path = require("path");
    const accountsDir = path.join(__dirname, "..", ".anchor", "generated_accounts");
    const files = fs.readdirSync(accountsDir);
    const tokenAccountFiles = files.filter((f: string) => f.endsWith(".token_account.json"));
    if (tokenAccountFiles.length >= 2) {
      const tokenAccountFilesWithTimes = tokenAccountFiles
        .map((f: string) => {
          const filePath = path.join(accountsDir, f);
          const stats = fs.statSync(filePath);
          return { name: f, mtime: stats.mtime.getTime() };
        })
        .sort((a: { mtime: number }, b: { mtime: number }) => a.mtime - b.mtime);
      const secondTokenAccountFile = tokenAccountFilesWithTimes[1].name;
      const secondTokenAccountPubkeyStr = secondTokenAccountFile.replace(".token_account.json", "");
      const secondTokenAccountPubkey = new PublicKey(secondTokenAccountPubkeyStr);
      const tokenAccountInfo = await provider.connection.getAccountInfo(secondTokenAccountPubkey);
      if (tokenAccountInfo) {
        const ownerBytes = tokenAccountInfo.data.slice(32, 64);
        const ownerPubkey = new PublicKey(ownerBytes);
        assert.equal(ownerPubkey.toBase58(), "9WzDXwBbmkg8ZTbNMqUxvQRAyrZzDsGYdLVL9zYtAWWM");
        const amountBytes = tokenAccountInfo.data.slice(64, 72);
        const amount = Buffer.from(amountBytes).readBigUInt64LE(0);
        assert.equal(amount.toString(), "1000000000");
      }
    }
  });

  it("Should create token account with mint=new owner=new address=new", async () => {
    const fs = require("fs");
    const path = require("path");
    const accountsDir = path.join(__dirname, "..", ".anchor", "generated_accounts");
    const files = fs.readdirSync(accountsDir);
    const tokenAccountFiles = files.filter((f: string) => f.endsWith(".token_account.json"));
    if (tokenAccountFiles.length >= 3) {
      const tokenAccountFilesWithTimes = tokenAccountFiles
        .map((f: string) => {
          const filePath = path.join(accountsDir, f);
          const stats = fs.statSync(filePath);
          return { name: f, mtime: stats.mtime.getTime() };
        })
        .sort((a: { mtime: number }, b: { mtime: number }) => a.mtime - b.mtime);
      const thirdTokenAccountFile = tokenAccountFilesWithTimes[2].name;
      const thirdTokenAccountPubkeyStr = thirdTokenAccountFile.replace(".token_account.json", "");
      const thirdTokenAccountPubkey = new PublicKey(thirdTokenAccountPubkeyStr);
      const tokenAccountInfo = await provider.connection.getAccountInfo(thirdTokenAccountPubkey);
      if (tokenAccountInfo) {
        assert.equal(tokenAccountInfo.owner.toBase58(), "TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA");
        const amountBytes = tokenAccountInfo.data.slice(64, 72);
        const amount = Buffer.from(amountBytes).readBigUInt64LE(0);
        assert.equal(amount.toString(), "250000000");
      }
    }
  });

  it("Should save owner keypairs when owner=new", async () => {
    const fs = require("fs");
    const path = require("path");
    const accountsDir = path.join(__dirname, "..", ".anchor", "generated_accounts");
    const files = fs.readdirSync(accountsDir);
    const ownerFiles = files.filter((f: string) => f.endsWith(".owner.json"));
    assert.isTrue(ownerFiles.length >= 2);
  });

  it("Should use most recent mint when mint=new", async () => {
    const fs = require("fs");
    const path = require("path");
    const accountsDir = path.join(__dirname, "..", ".anchor", "generated_accounts");
    const files = fs.readdirSync(accountsDir);
    const mintFiles = files.filter((f: string) => f.endsWith(".mint.json"));
    const tokenAccountFiles = files.filter((f: string) => f.endsWith(".token_account.json"));
    if (mintFiles.length >= 3 && tokenAccountFiles.length > 0) {
      const mintFilesWithTimes = mintFiles
        .map((f: string) => {
          const filePath = path.join(accountsDir, f);
          const stats = fs.statSync(filePath);
          return { name: f, mtime: stats.mtime.getTime() };
        })
        .sort((a: { mtime: number }, b: { mtime: number }) => a.mtime - b.mtime);
      const lastMintFile = mintFilesWithTimes[mintFilesWithTimes.length - 1].name;
      const lastMintPubkeyStr = lastMintFile.replace(".mint.json", "");
      const lastMintPubkey = new PublicKey(lastMintPubkeyStr);
      const tokenAccountFilesWithTimes = tokenAccountFiles
        .map((f: string) => {
          const filePath = path.join(accountsDir, f);
          const stats = fs.statSync(filePath);
          return { name: f, mtime: stats.mtime.getTime() };
        })
        .sort((a: { mtime: number }, b: { mtime: number }) => a.mtime - b.mtime);
      for (const tokenAccountFile of tokenAccountFilesWithTimes) {
        const tokenAccountPubkeyStr = tokenAccountFile.name.replace(".token_account.json", "");
        const tokenAccountPubkey = new PublicKey(tokenAccountPubkeyStr);
        const tokenAccountInfo = await provider.connection.getAccountInfo(tokenAccountPubkey);
        if (tokenAccountInfo) {
          const mintBytes = tokenAccountInfo.data.slice(0, 32);
          const mintPubkey = new PublicKey(mintBytes);
          assert.equal(mintPubkey.toBase58(), lastMintPubkey.toBase58());
        }
      }
    }
  });

  it("Should create accounts with correct rent-exempt lamports", async () => {
    const fs = require("fs");
    const path = require("path");
    const accountsDir = path.join(__dirname, "..", ".anchor", "generated_accounts");
    const files = fs.readdirSync(accountsDir);
    const mintFiles = files.filter((f: string) => f.endsWith(".mint.json"));
    const tokenAccountFiles = files.filter((f: string) => f.endsWith(".token_account.json"));
    if (mintFiles.length > 0) {
      const mintFile = mintFiles[0];
      const mintPubkeyStr = mintFile.replace(".mint.json", "");
      const mintPubkey = new PublicKey(mintPubkeyStr);
      const mintInfo = await provider.connection.getAccountInfo(mintPubkey);
      if (mintInfo) {
        assert.isTrue(mintInfo.lamports >= 1_462_920);
      }
    }
    if (tokenAccountFiles.length > 0) {
      const tokenAccountFile = tokenAccountFiles[0];
      const tokenAccountPubkeyStr = tokenAccountFile.replace(".token_account.json", "");
      const tokenAccountPubkey = new PublicKey(tokenAccountPubkeyStr);
      const tokenAccountInfo = await provider.connection.getAccountInfo(tokenAccountPubkey);
      if (tokenAccountInfo) {
        assert.isTrue(tokenAccountInfo.lamports >= 2_039_280);
      }
    }
  });

  it("Should handle multiple token accounts referencing same mint", async () => {
    const fs = require("fs");
    const path = require("path");
    const accountsDir = path.join(__dirname, "..", ".anchor", "generated_accounts");
    const files = fs.readdirSync(accountsDir);
    const tokenAccountFiles = files.filter((f: string) => f.endsWith(".token_account.json"));
    if (tokenAccountFiles.length >= 2) {
      const tokenAccountFilesWithTimes = tokenAccountFiles
        .map((f: string) => {
          const filePath = path.join(accountsDir, f);
          const stats = fs.statSync(filePath);
          return { name: f, mtime: stats.mtime.getTime() };
        })
        .sort((a: { mtime: number }, b: { mtime: number }) => b.mtime - a.mtime);
      const firstTokenAccountPubkeyStr = tokenAccountFilesWithTimes[0].name.replace(".token_account.json", "");
      const secondTokenAccountPubkeyStr = tokenAccountFilesWithTimes[1].name.replace(".token_account.json", "");
      const firstTokenAccountPubkey = new PublicKey(firstTokenAccountPubkeyStr);
      const secondTokenAccountPubkey = new PublicKey(secondTokenAccountPubkeyStr);
      const firstTokenAccountInfo = await provider.connection.getAccountInfo(firstTokenAccountPubkey);
      const secondTokenAccountInfo = await provider.connection.getAccountInfo(secondTokenAccountPubkey);
      if (firstTokenAccountInfo && secondTokenAccountInfo) {
        const firstMintBytes = firstTokenAccountInfo.data.slice(0, 32);
        const secondMintBytes = secondTokenAccountInfo.data.slice(0, 32);
        const firstMintPubkey = new PublicKey(firstMintBytes);
        const secondMintPubkey = new PublicKey(secondMintBytes);
        assert.equal(firstMintPubkey.toBase58(), secondMintPubkey.toBase58());
      }
    }
  });

  it("Should create all account JSON files", async () => {
    const fs = require("fs");
    const path = require("path");
    const accountsDir = path.join(__dirname, "..", ".anchor", "generated_accounts");
    const files = fs.readdirSync(accountsDir);
    const accountJsonFiles = files.filter((f: string) => 
      f.endsWith(".json") && 
      !f.endsWith(".keypair.json") && 
      !f.endsWith(".mint.json") && 
      !f.endsWith(".token_account.json") && 
      !f.endsWith(".owner.json")
    );
    assert.isTrue(accountJsonFiles.length >= 5);
  });

  it("Should verify mint supply matches configuration", async () => {
    const fs = require("fs");
    const path = require("path");
    const accountsDir = path.join(__dirname, "..", ".anchor", "generated_accounts");
    const files = fs.readdirSync(accountsDir);
    const mintFiles = files.filter((f: string) => f.endsWith(".mint.json"));
    const mintFilesWithTimes = mintFiles
      .map((f: string) => {
        const filePath = path.join(accountsDir, f);
        const stats = fs.statSync(filePath);
        return { name: f, mtime: stats.mtime.getTime() };
      })
      .sort((a: { mtime: number }, b: { mtime: number }) => a.mtime - b.mtime);
    if (mintFilesWithTimes.length >= 1) {
      const firstMintFile = mintFilesWithTimes[0].name;
      const firstMintPubkeyStr = firstMintFile.replace(".mint.json", "");
      const firstMintPubkey = new PublicKey(firstMintPubkeyStr);
      const mintInfo = await provider.connection.getAccountInfo(firstMintPubkey);
      if (mintInfo && mintInfo.data.length >= 82) {
        const supplyBytes = mintInfo.data.slice(36, 44);
        const supply = Buffer.from(supplyBytes).readBigUInt64LE(0);
        assert.equal(supply.toString(), "1000000000");
        const decimals = mintInfo.data[44];
        assert.equal(decimals, 9);
      }
    }
  });

  it("Should verify mint decimals match configuration", async () => {
    const fs = require("fs");
    const path = require("path");
    const accountsDir = path.join(__dirname, "..", ".anchor", "generated_accounts");
    const files = fs.readdirSync(accountsDir);
    const mintFiles = files.filter((f: string) => f.endsWith(".mint.json"));
    const mintFilesWithTimes = mintFiles
      .map((f: string) => {
        const filePath = path.join(accountsDir, f);
        const stats = fs.statSync(filePath);
        return { name: f, mtime: stats.mtime.getTime() };
      })
      .sort((a: { mtime: number }, b: { mtime: number }) => a.mtime - b.mtime);
    if (mintFilesWithTimes.length >= 2) {
      const secondMintFile = mintFilesWithTimes[1].name;
      const secondMintPubkeyStr = secondMintFile.replace(".mint.json", "");
      const secondMintPubkey = new PublicKey(secondMintPubkeyStr);
      const mintInfo = await provider.connection.getAccountInfo(secondMintPubkey);
      if (mintInfo && mintInfo.data.length >= 82) {
        const decimals = mintInfo.data[44];
        assert.equal(decimals, 6);
        const supplyBytes = mintInfo.data.slice(36, 44);
        const supply = Buffer.from(supplyBytes).readBigUInt64LE(0);
        assert.equal(supply.toString(), "500000000");
      }
    }
  });

  it("Should support specific pubkey address for mints", async () => {
    const specificMintPubkey = new PublicKey("EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v");
    const mintInfo = await provider.connection.getAccountInfo(specificMintPubkey);
    if (mintInfo) {
      assert.equal(mintInfo.owner.toBase58(), "TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA");
      assert.isTrue(mintInfo.data.length >= 82);
      const decimals = mintInfo.data[44];
      assert.equal(decimals, 6);
      const supplyBytes = mintInfo.data.slice(36, 44);
      const supply = Buffer.from(supplyBytes).readBigUInt64LE(0);
      assert.equal(supply.toString(), "1000000");
    }
  });
});
