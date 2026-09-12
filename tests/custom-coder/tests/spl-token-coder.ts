import { AnchorProvider, setProvider } from "@anchor-lang/core";
import { splTokenProgram } from "@anchor-lang/spl-token";
import { Keypair, SYSVAR_RENT_PUBKEY, PublicKey } from "@solana/web3.js";
import BN from "bn.js";
import { assert } from "chai";

describe("spl-token", () => {
  // Configure the client to use the local cluster.
  const provider = AnchorProvider.env();
  setProvider(provider);

  // Client.
  const program = splTokenProgram({
    provider,
  });

  // Constants.
  const mintKeypair = Keypair.generate();
  const aliceTokenKeypair = Keypair.generate();
  const bobTokenKeypair = Keypair.generate();
  const rent = SYSVAR_RENT_PUBKEY;

  it("Creates a mint", async () => {
    await program.methods
      .initializeMint(6, provider.wallet.publicKey, null)
      .accounts({
        mint: mintKeypair.publicKey,
        rent,
      })
      .signers([mintKeypair])
      .preInstructions([
        await program.account.mint.createInstruction(mintKeypair),
      ])
      .rpc();
    const mintAccount = await program.account.mint.fetch(mintKeypair.publicKey);
    assert.isTrue(
      (mintAccount.mintAuthority as PublicKey).equals(provider.wallet.publicKey)
    );
    assert.isNull(mintAccount.freezeAuthority);
    assert.strictEqual(mintAccount.decimals, 6);
    assert.isTrue(mintAccount.isInitialized);
    assert.strictEqual(mintAccount.supply.toNumber(), 0);
  });

  it("Creates a token account for alice", async () => {
    await program.methods
      .initializeAccount()
      .accounts({
        account: aliceTokenKeypair.publicKey,
        mint: mintKeypair.publicKey,
        owner: provider.wallet.publicKey,
        rent,
      })
      .signers([aliceTokenKeypair])
      .preInstructions([
        await program.account.account.createInstruction(aliceTokenKeypair),
      ])
      .rpc();
    const token = await program.account.account.fetch(
      aliceTokenKeypair.publicKey
    );
    assert.isTrue(token.owner.equals(provider.wallet.publicKey));
    assert.isTrue(token.mint.equals(mintKeypair.publicKey));
    assert.strictEqual(token.amount.toNumber(), 0);
    assert.isNull(token.delegate);
    assert.strictEqual(Object.keys(token.state)[0], "initialized");
    assert.isNull(token.isNative);
    assert.strictEqual(token.delegatedAmount.toNumber(), 0);
    assert.isNull(token.closeAuthority);
  });

  it("Mints a token to alice", async () => {
    await program.methods
      .mintTo(new BN(2))
      .accounts({
        mint: mintKeypair.publicKey,
        account: aliceTokenKeypair.publicKey,
        owner: provider.wallet.publicKey,
      })
      .rpc();

    const token = await program.account.account.fetch(
      aliceTokenKeypair.publicKey
    );
    const mint = await program.account.mint.fetch(mintKeypair.publicKey);
    assert.strictEqual(token.amount.toNumber(), 2);
    assert.strictEqual(mint.supply.toNumber(), 2);
  });

  it("Creates a token for bob", async () => {
    await program.methods
      .initializeAccount()
      .accounts({
        account: bobTokenKeypair.publicKey,
        mint: mintKeypair.publicKey,
        owner: provider.wallet.publicKey,
        rent,
      })
      .signers([bobTokenKeypair])
      .preInstructions([
        await program.account.account.createInstruction(bobTokenKeypair),
      ])
      .rpc();
  });

  it("Transfer a token from alice to bob", async () => {
    await program.methods
      .transfer(new BN(1))
      .accounts({
        source: aliceTokenKeypair.publicKey,
        destination: bobTokenKeypair.publicKey,
        authority: provider.wallet.publicKey,
      })
      .rpc();
    const aliceToken = await program.account.account.fetch(
      aliceTokenKeypair.publicKey
    );
    const bobToken = await program.account.account.fetch(
      bobTokenKeypair.publicKey
    );
    assert.strictEqual(aliceToken.amount.toNumber(), 1);
    assert.strictEqual(bobToken.amount.toNumber(), 1);
  });

  it("Alice burns a token", async () => {
    await program.methods
      .burn(new BN(1))
      .accounts({
        account: aliceTokenKeypair.publicKey,
        mint: mintKeypair.publicKey,
        authority: provider.wallet.publicKey,
      })
      .rpc();
    const aliceToken = await program.account.account.fetch(
      aliceTokenKeypair.publicKey
    );
    const mint = await program.account.mint.fetch(mintKeypair.publicKey);
    assert.strictEqual(aliceToken.amount.toNumber(), 0);
    assert.strictEqual(mint.supply.toNumber(), 1);
  });
});
