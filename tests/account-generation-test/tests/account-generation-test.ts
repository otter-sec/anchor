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
          f.length >= 57 // "pubkey.keypair.json" = 44 (pubkey) + 13 (.keypair.json)
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
});
