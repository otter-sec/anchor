import * as anchor from "@anchor-lang/core";
import { Program, AnchorError } from "@anchor-lang/core";
import { DuplicateMutableAccounts } from "../target/types/duplicate_mutable_accounts";
import { assert } from "chai";

describe("duplicate-mutable-accounts", () => {
  anchor.setProvider(anchor.AnchorProvider.env());
  const provider = anchor.getProvider() as anchor.AnchorProvider;
  const program = anchor.workspace
    .DuplicateMutableAccounts as Program<DuplicateMutableAccounts>;

  // Payer used by #[account(init, payer = user, ...)]
  const user_wallet = anchor.web3.Keypair.generate();

  // Two regular system accounts to hold Counter state (must sign on init)
  const dataAccount1 = anchor.web3.Keypair.generate();
  const dataAccount2 = anchor.web3.Keypair.generate();

  it("Initialize accounts", async () => {
    // 1) Fund user_wallet so it can pay rent
    const airdropSig = await provider.connection.requestAirdrop(
      user_wallet.publicKey,
      2 * anchor.web3.LAMPORTS_PER_SOL
    );
    await provider.connection.confirmTransaction(airdropSig);

    // 2) Create & init dataAccount1 (must sign with dataAccount1)
    await program.methods
      .initialize(new anchor.BN(100))
      .accounts({
        dataAccount: dataAccount1.publicKey,
        user: user_wallet.publicKey,
        systemProgram: anchor.web3.SystemProgram.programId,
      })
      .signers([user_wallet, dataAccount1]) // <- include the new account keypair
      .rpc();

    // 3) Create & init dataAccount2
    await program.methods
      .initialize(new anchor.BN(300))
      .accounts({
        dataAccount: dataAccount2.publicKey,
        user: user_wallet.publicKey,
        systemProgram: anchor.web3.SystemProgram.programId,
      })
      .signers([user_wallet, dataAccount2]) // <- include the new account keypair
      .rpc();
  });

  it("Should fail with duplicate mutable accounts", async () => {
    // Ensure the accounts are initialized
    const account1 = await program.account.counter.fetch(
      dataAccount1.publicKey
    );
    const account2 = await program.account.counter.fetch(
      dataAccount2.publicKey
    );
    assert.strictEqual(account1.count.toNumber(), 100);
    assert.strictEqual(account2.count.toNumber(), 300);

    try {
      await program.methods
        .failsDuplicateMutable()
        .accounts({
          account1: dataAccount1.publicKey,
          account2: dataAccount1.publicKey, // <- SAME account to trigger the check
        })
        .rpc();
      assert.fail("Expected duplicate mutable violation");
    } catch (e) {
      assert.instanceOf(e, AnchorError);
      const err = e as AnchorError;
      assert.strictEqual(
        err.error.errorCode.code,
        "ConstraintDuplicateMutableAccount"
      );
      assert.strictEqual(err.error.errorCode.number, 2040);
    }
  });

  it("Should succeed with duplicate mutable accounts when using dup constraint", async () => {
    // This instruction MUST have `#[account(mut, dup)]` on at least one account
    await program.methods
      .allowsDuplicateMutable()
      .accounts({
        account1: dataAccount1.publicKey,
        account2: dataAccount1.publicKey, // same account allowed via `dup`
      })
      .rpc();
    assert.ok(true);
  });

  it("Should allow duplicate readonly accounts", async () => {
    // Readonly accounts can be duplicated without any constraint
    await program.methods
      .allowsDuplicateReadonly()
      .accounts({
        account1: dataAccount1.publicKey,
        account2: dataAccount1.publicKey, // same account, both readonly
      })
      .rpc();
    assert.ok(true, "Readonly duplicates are allowed");
  });

  // --- Composite field duplicate mutable account tests ---
  it("Should block mixed duplicate: direct Field + CompositeField with same account", async () => {
    try {
      await program.methods
        .mixedDuplicate()
        .accounts({
          account1: dataAccount1.publicKey,
          wrapper: {
            counter: dataAccount1.publicKey, // same account in composite
          },
        })
        .rpc();

      assert.fail(
        "Same account as direct field and inside composite should be blocked"
      );
    } catch (e) {
      assert.instanceOf(e, AnchorError);
      const err = e as AnchorError;
      assert.strictEqual(
        err.error.errorCode.code,
        "ConstraintDuplicateMutableAccount",
        "Expected ConstraintDuplicateMutableAccount but got: " +
          err.error.errorCode.code
      );
    }
  });

  it("Should block nested duplicate: two CompositeFields with same account", async () => {
    try {
      await program.methods
        .nestedDuplicate()
        .accounts({
          wrapper1: {
            counter: dataAccount1.publicKey,
          },
          wrapper2: {
            counter: dataAccount1.publicKey, // same account in both wrappers
          },
        })
        .rpc();

      assert.fail("Same account in two composite fields should be blocked");
    } catch (e) {
      assert.instanceOf(e, AnchorError);
      const err = e as AnchorError;
      assert.strictEqual(
        err.error.errorCode.code,
        "ConstraintDuplicateMutableAccount",
        "Expected ConstraintDuplicateMutableAccount but got: " +
          err.error.errorCode.code
      );
    }
  });

  it("Should allow duplicate in remainingAccounts", async () => {
    await program.methods
      .failsDuplicateMutable()
      .accounts({
        account1: dataAccount1.publicKey,
        account2: dataAccount2.publicKey, // Different account
      })
      .remainingAccounts([
        {
          pubkey: dataAccount1.publicKey, // duplicate via remainingAccounts is allowed
          isWritable: true,
          isSigner: false,
        },
      ])
      .rpc();
  });

  it("Should allow using remaining_accounts without duplicates", async () => {
    // Get initial counts
    const beforeAccount1 = await program.account.counter.fetch(
      dataAccount1.publicKey
    );

    // Call with valid remaining accounts (no duplicates)
    await program.methods
      .useRemainingAccounts()
      .accounts({
        account1: dataAccount1.publicKey,
      })
      .remainingAccounts([
        {
          pubkey: dataAccount2.publicKey,
          isWritable: true,
          isSigner: false,
        },
      ])
      .rpc();

    // Verify account was incremented
    const afterAccount1 = await program.account.counter.fetch(
      dataAccount1.publicKey
    );

    assert.equal(
      afterAccount1.count.toNumber(),
      beforeAccount1.count.toNumber() + 1,
      "Account1 should be incremented"
    );
  });

  it("Should allow initializing multiple accounts with the same payer", async () => {
    // Create two new keypairs for the accounts to be initialized
    const newAccount1 = anchor.web3.Keypair.generate();
    const newAccount2 = anchor.web3.Keypair.generate();

    // Initialize both accounts using the same payer
    // This should succeed because:
    // 1. The payer is a Signer, which is excluded from duplicate checks
    // 2. The accounts being initialized are different (newAccount1 vs newAccount2)
    await program.methods
      .initMultipleWithSamePayer(new anchor.BN(500), new anchor.BN(600))
      .accounts({
        dataAccount1: newAccount1.publicKey,
        dataAccount2: newAccount2.publicKey,
        user: user_wallet.publicKey,
        systemProgram: anchor.web3.SystemProgram.programId,
      })
      .signers([user_wallet, newAccount1, newAccount2])
      .rpc();

    // Verify both accounts were created with correct values
    const account1Data = await program.account.counter.fetch(
      newAccount1.publicKey
    );
    const account2Data = await program.account.counter.fetch(
      newAccount2.publicKey
    );

    assert.equal(
      account1Data.count.toNumber(),
      500,
      "First account should have count 500"
    );
    assert.equal(
      account2Data.count.toNumber(),
      600,
      "Second account should have count 600"
    );
  });

  it("Should block init_if_needed duplicate with mut account (already initialized)", async () => {
    // dataAccount1 is already initialized from earlier tests.
    // Passing it as both the init_if_needed field and the mut field should
    // trigger the duplicate mutable account check.
    try {
      await program.methods
        .initIfNeededDuplicateMutable()
        .accounts({
          accountInit: dataAccount1.publicKey,
          accountMut: dataAccount1.publicKey, // same account — should be caught
          payer: user_wallet.publicKey,
          systemProgram: anchor.web3.SystemProgram.programId,
        })
        .signers([user_wallet, dataAccount1])
        .rpc();
      assert.fail("init_if_needed + mut with same account should be blocked");
    } catch (e) {
      assert.instanceOf(e, AnchorError);
      const err = e as AnchorError;
      assert.strictEqual(
        err.error.errorCode.code,
        "ConstraintDuplicateMutableAccount",
        "Expected ConstraintDuplicateMutableAccount but got: " +
          err.error.errorCode.code
      );
    }
  });
});
