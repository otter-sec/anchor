import * as anchor from "@anchor-lang/core";
import { Native } from "@anchor-lang/core";
import { TOKEN_PROGRAM_ID } from "@solana/spl-token";
import {
  Keypair,
  LAMPORTS_PER_SOL,
  NONCE_ACCOUNT_LENGTH,
  PublicKey,
  SystemProgram,
  SYSVAR_RECENT_BLOCKHASHES_PUBKEY,
} from "@solana/web3.js";
import * as assert from "assert";
import BN from "bn.js";

describe("system-coder", () => {
  // Configure the client to use the local cluster.
  const provider = anchor.AnchorProvider.env();
  anchor.setProvider(provider);

  // Client.
  const program = Native.system();

  // Constants.
  const aliceKeypair = Keypair.generate();

  it("Creates an account", async () => {
    // arrange
    const space = 100;
    const lamports =
      await program.provider.connection.getMinimumBalanceForRentExemption(
        space
      );
    const owner = SystemProgram.programId;
    // act
    await program.methods
      .createAccount(new BN(lamports), new BN(space), owner)
      .accounts({
        from: provider.wallet.publicKey,
        to: aliceKeypair.publicKey,
      })
      .signers([aliceKeypair])
      .rpc();
    // assert
    const aliceAccount = await program.provider.connection.getAccountInfo(
      aliceKeypair.publicKey
    );
    assert.notEqual(aliceAccount, null);
    assert.ok(owner.equals(aliceAccount.owner));
    assert.equal(lamports, aliceAccount.lamports);
  });

  it("Assigns an account to a program", async () => {
    // arrange
    const owner = TOKEN_PROGRAM_ID;
    // act
    await program.methods
      .assign(owner)
      .accounts({
        pubkey: aliceKeypair.publicKey,
      })
      .signers([aliceKeypair])
      .rpc();
    // assert
    const aliceAccount = await program.provider.connection.getAccountInfo(
      aliceKeypair.publicKey
    );
    assert.notEqual(aliceAccount, null);
    assert.ok(owner.equals(aliceAccount.owner));
  });

  it("Allocates space to an account", async () => {
    // arrange
    const newKeypair = Keypair.generate();
    const space = 100;
    const lamports =
      await program.provider.connection.getMinimumBalanceForRentExemption(
        space
      );
    // act
    await program.methods
      .allocate(new BN(space))
      .accounts({
        pubkey: newKeypair.publicKey,
      })
      .postInstructions([
        await program.methods
          .transfer(new BN(lamports))
          .accounts({
            from: provider.wallet.publicKey,
            to: newKeypair.publicKey,
          })
          .instruction(),
      ])
      .signers([newKeypair])
      .rpc();
    // assert
    const newAccountAfter = await program.provider.connection.getAccountInfo(
      newKeypair.publicKey
    );
    assert.equal(space, newAccountAfter.data.byteLength);
  });

  it("Creates an account with seed", async () => {
    const space = 100;
    const lamports =
      await program.provider.connection.getMinimumBalanceForRentExemption(
        space
      );
    const owner = SystemProgram.programId;
    const seed = "seeds";
    const bobPublicKey = await PublicKey.createWithSeed(
      aliceKeypair.publicKey,
      seed,
      owner
    );
    // act
    await program.methods
      .createAccountWithSeed(
        aliceKeypair.publicKey,
        seed,
        new BN(lamports),
        new BN(space),
        owner
      )
      .accounts({
        base: aliceKeypair.publicKey,
        from: provider.wallet.publicKey,
        to: bobPublicKey,
      })
      .signers([aliceKeypair])
      .rpc();
    // assert
    const bobAccount = await program.provider.connection.getAccountInfo(
      bobPublicKey
    );
    assert.notEqual(bobAccount, null);
  });

  it("Allocates and assigns an account with seed", async () => {
    const owner = TOKEN_PROGRAM_ID;
    const seed = "seeds2";
    const space = 100;
    const lamports =
      await program.provider.connection.getMinimumBalanceForRentExemption(
        space
      );
    const bobPublicKey = await PublicKey.createWithSeed(
      aliceKeypair.publicKey,
      seed,
      owner
    );
    // act
    await program.methods
      .allocateWithSeed(aliceKeypair.publicKey, seed, new BN(space), owner)
      .accounts({
        base: aliceKeypair.publicKey,
        account: bobPublicKey,
      })
      .postInstructions([
        await program.methods
          .transfer(new BN(lamports))
          .accounts({
            from: provider.wallet.publicKey,
            to: bobPublicKey,
          })
          .instruction(),
        await program.methods
          .assignWithSeed(aliceKeypair.publicKey, seed, owner)
          .accounts({
            base: aliceKeypair.publicKey,
            account: bobPublicKey,
          })
          .instruction(),
      ])
      .signers([aliceKeypair])
      .rpc();
    // assert
    const bobAccount = await program.provider.connection.getAccountInfo(
      bobPublicKey
    );
    assert.notEqual(bobAccount, null);
    assert.ok(owner.equals(bobAccount.owner));
  });

  it("Transfers from account with seed", async () => {
    const lamports = 1 * LAMPORTS_PER_SOL;
    const owner = SystemProgram.programId;
    const seed = "seeds3";
    const bobPublicKey = await PublicKey.createWithSeed(
      aliceKeypair.publicKey,
      seed,
      owner
    );
    const aliceAccountBefore = await program.provider.connection.getAccountInfo(
      aliceKeypair.publicKey
    );
    // act
    await program.methods
      .transfer(new BN(lamports))
      .accounts({
        from: provider.wallet.publicKey,
        to: bobPublicKey,
      })
      .rpc();
    await program.methods
      .transferWithSeed(new BN(lamports), seed, owner)
      .accounts({
        from: bobPublicKey,
        base: aliceKeypair.publicKey,
        to: aliceKeypair.publicKey,
      })
      .signers([aliceKeypair])
      .rpc();
    // assert
    const aliceAccountAfter = await program.provider.connection.getAccountInfo(
      aliceKeypair.publicKey
    );
    assert.equal(
      aliceAccountBefore.lamports + lamports,
      aliceAccountAfter.lamports
    );
  });

  it("Transfers lamports", async () => {
    // arrange
    const receiverKeypair = Keypair.generate();
    const lamports = 0.1 * LAMPORTS_PER_SOL;
    // act
    await program.methods
      .transfer(new BN(lamports))
      .accounts({
        from: provider.wallet.publicKey,
        to: receiverKeypair.publicKey,
      })
      .rpc();
    // assert
    const receiverAccount = await program.provider.connection.getAccountInfo(
      receiverKeypair.publicKey
    );
    assert.notEqual(receiverAccount, null);
    assert.equal(lamports, receiverAccount.lamports);
  });

  it("Initializes nonce account", async () => {
    // arrange
    const nonceKeypair = Keypair.generate();
    const owner = SystemProgram.programId;
    const space = NONCE_ACCOUNT_LENGTH;
    const lamports =
      await provider.connection.getMinimumBalanceForRentExemption(space);
    // act
    await program.methods
      .initializeNonceAccount(provider.wallet.publicKey)
      .accounts({
        nonce: nonceKeypair.publicKey,
        recentBlockhashes: SYSVAR_RECENT_BLOCKHASHES_PUBKEY,
      })
      .preInstructions([
        await program.methods
          .createAccount(new BN(lamports), new BN(space), owner)
          .accounts({
            from: provider.wallet.publicKey,
            to: nonceKeypair.publicKey,
          })
          .instruction(),
      ])
      .signers([nonceKeypair])
      .rpc();
    // assert
    const nonceAccount = await program.account.nonce.fetch(
      nonceKeypair.publicKey
    );
    assert.notEqual(nonceAccount, null);
    assert.ok(nonceAccount.authorizedPubkey.equals(provider.wallet.publicKey));
  });

  it("Advances a nonce account", async () => {
    // arrange
    const nonceKeypair = Keypair.generate();
    const owner = SystemProgram.programId;
    const space = NONCE_ACCOUNT_LENGTH;
    const lamports =
      await provider.connection.getMinimumBalanceForRentExemption(space);
    // act
    await program.methods
      .initializeNonceAccount(provider.wallet.publicKey)
      .accounts({
        nonce: nonceKeypair.publicKey,
        recentBlockhashes: SYSVAR_RECENT_BLOCKHASHES_PUBKEY,
      })
      .preInstructions([
        await program.methods
          .createAccount(new BN(lamports), new BN(space), owner)
          .accounts({
            from: provider.wallet.publicKey,
            to: nonceKeypair.publicKey,
          })
          .instruction(),
      ])
      .signers([nonceKeypair])
      .rpc();
    // These have to be separate to make sure advance is in another slot.
    await program.methods
      .advanceNonceAccount(provider.wallet.publicKey)
      .accounts({
        nonce: nonceKeypair.publicKey,
        recentBlockhashes: SYSVAR_RECENT_BLOCKHASHES_PUBKEY,
      })
      .rpc();
    // assert
    const nonceAccount = await program.account.nonce.fetch(
      nonceKeypair.publicKey
    );
    assert.notEqual(nonceAccount, null);
  });

  it("Authorizes a nonce account", async () => {
    // arrange
    const nonceKeypair = Keypair.generate();
    const owner = SystemProgram.programId;
    const space = NONCE_ACCOUNT_LENGTH;
    const lamports =
      await provider.connection.getMinimumBalanceForRentExemption(space);
    // act
    await program.methods
      .initializeNonceAccount(provider.wallet.publicKey)
      .accounts({
        nonce: nonceKeypair.publicKey,
        recentBlockhashes: SYSVAR_RECENT_BLOCKHASHES_PUBKEY,
      })
      .preInstructions([
        await program.methods
          .createAccount(new BN(lamports), new BN(space), owner)
          .accounts({
            from: provider.wallet.publicKey,
            to: nonceKeypair.publicKey,
          })
          .instruction(),
      ])
      .signers([nonceKeypair])
      .rpc();
    await program.methods
      .authorizeNonceAccount(aliceKeypair.publicKey)
      .accounts({
        nonce: nonceKeypair.publicKey,
        authorized: provider.wallet.publicKey,
      })
      .rpc();
    // assert
    const nonceAccount = await program.account.nonce.fetch(
      nonceKeypair.publicKey
    );
    assert.notEqual(nonceAccount, null);
    assert.ok(nonceAccount.authorizedPubkey.equals(aliceKeypair.publicKey));
  });

  it("Withdraws from nonce account", async () => {
    // arrange
    const nonceKeypair = Keypair.generate();
    const owner = SystemProgram.programId;
    const space = NONCE_ACCOUNT_LENGTH;
    const lamports =
      await provider.connection.getMinimumBalanceForRentExemption(space);
    const amount = 0.1 * LAMPORTS_PER_SOL;
    const aliceBalanceBefore = (
      await program.provider.connection.getAccountInfo(aliceKeypair.publicKey)
    ).lamports;
    // act
    await program.methods
      .initializeNonceAccount(provider.wallet.publicKey)
      .accounts({
        nonce: nonceKeypair.publicKey,
        recentBlockhashes: SYSVAR_RECENT_BLOCKHASHES_PUBKEY,
      })
      .preInstructions([
        await program.methods
          .createAccount(new BN(lamports), new BN(space), owner)
          .accounts({
            from: provider.wallet.publicKey,
            to: nonceKeypair.publicKey,
          })
          .instruction(),
      ])
      .signers([nonceKeypair])
      .rpc();
    await program.methods
      .advanceNonceAccount(provider.wallet.publicKey)
      .accounts({
        nonce: nonceKeypair.publicKey,
        recentBlockhashes: SYSVAR_RECENT_BLOCKHASHES_PUBKEY,
      })
      .postInstructions([
        await program.methods
          .transfer(new BN(amount))
          .accounts({
            from: provider.wallet.publicKey,
            to: nonceKeypair.publicKey,
          })
          .instruction(),
      ])
      .rpc();
    await program.methods
      .authorizeNonceAccount(aliceKeypair.publicKey)
      .accounts({
        nonce: nonceKeypair.publicKey,
        authorized: provider.wallet.publicKey,
      })
      .rpc();
    await program.methods
      .withdrawNonceAccount(new BN(amount))
      .accounts({
        authorized: aliceKeypair.publicKey,
        nonce: nonceKeypair.publicKey,
        recentBlockhashes: SYSVAR_RECENT_BLOCKHASHES_PUBKEY,
        to: aliceKeypair.publicKey,
      })
      .signers([aliceKeypair])
      .rpc();
    // assert
    const aliceBalanceAfter = (
      await program.provider.connection.getAccountInfo(aliceKeypair.publicKey)
    ).lamports;
    assert.equal(aliceBalanceAfter - aliceBalanceBefore, amount);
  });
});
