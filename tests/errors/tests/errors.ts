import * as anchor from "@anchor-lang/core";
import { Program, AnchorError } from "@anchor-lang/core";
import { Keypair, Transaction, TransactionInstruction } from "@solana/web3.js";
import { TOKEN_PROGRAM_ID, Token } from "@solana/spl-token";
import { assert, expect } from "chai";
import { Errors } from "../target/types/errors";
import { sleep } from "@project-serum/common";

const withLogTest = async (callback, expectedLogs) => {
  let logTestOk = false;
  const listener = anchor.getProvider().connection.onLogs("all", (logs) => {
    const index = logs.logs.findIndex((logLine) => logLine === expectedLogs[0]);
    if (index === -1) {
      console.log("Expected: ");
      console.log(expectedLogs);
      console.log("Actual: ");
      console.log(logs);
    } else {
      const actualLogs = logs.logs.slice(index, index + expectedLogs.length);
      for (let i = 0; i < expectedLogs.length; i++) {
        if (actualLogs[i] !== expectedLogs[i]) {
          console.log("Expected: ");
          console.log(expectedLogs);
          console.log("Actual: ");
          console.log(logs);
          return;
        }
      }
      logTestOk = true;
    }
  });
  try {
    await callback();
  } catch (err) {
    anchor.getProvider().connection.removeOnLogsListener(listener);
    throw err;
  }
  // wait for a max of 5 seconds to receive logs
  for (let i = 0; i < 50; i++) {
    if (logTestOk) {
      break;
    }
    await sleep(100);
  }
  anchor.getProvider().connection.removeOnLogsListener(listener);
  assert.isTrue(logTestOk);
};

describe("errors", () => {
  // Configure the client to use the local cluster.
  const provider = anchor.AnchorProvider.local();
  provider.opts.skipPreflight = true;
  // processed failed tx do not result in AnchorErrors in the client
  // because we cannot get logs for them (only through overkill `onLogs`)
  provider.opts.commitment = "confirmed";
  anchor.setProvider(provider);
  provider.opts.maxRetries = 3;

  const program = anchor.workspace.errors as Program<Errors>;

  it("Emits a Hello error", async () => {
    await withLogTest(async () => {
      try {
        const tx = await program.methods.hello().rpc();
        assert.ok(false);
      } catch (_err) {
        assert.isTrue(_err instanceof AnchorError);
        const err: AnchorError = _err;
        const errMsg =
          "This is an error message clients will automatically display";
        const fullErrMsg =
          "AnchorError thrown in programs/errors/src/lib.rs:13. Error Code: Hello. Error Number: 6000. Error Message: This is an error message clients will automatically display.";
        assert.strictEqual(err.toString(), fullErrMsg);
        assert.strictEqual(err.error.errorMessage, errMsg);
        assert.strictEqual(err.error.errorCode.number, 6000);
        assert.strictEqual(
          err.program.toString(),
          program.programId.toString()
        );
        expect(err.error.origin).to.deep.equal({
          file: "programs/errors/src/lib.rs",
          line: 13,
        });
      }
    }, [
      "Program log: AnchorError thrown in programs/errors/src/lib.rs:13. Error Code: Hello. Error Number: 6000. Error Message: This is an error message clients will automatically display.",
    ]);
  });

  it("Emits a Hello error via require!", async () => {
    try {
      const tx = await program.methods.testRequire().rpc();
      assert.ok(false);
    } catch (_err) {
      assert.isTrue(_err instanceof AnchorError);
      const err: AnchorError = _err;
      const errMsg =
        "This is an error message clients will automatically display";
      assert.strictEqual(err.error.errorMessage, errMsg);
      assert.strictEqual(err.error.errorCode.number, 6000);
      assert.strictEqual(err.error.errorCode.code, "Hello");
    }
  });

  it("Emits a Hello error via err!", async () => {
    try {
      const tx = await program.methods.testErr().rpc();
      assert.ok(false);
    } catch (_err) {
      assert.isTrue(_err instanceof AnchorError);
      const err: AnchorError = _err;
      const errMsg =
        "This is an error message clients will automatically display";
      assert.strictEqual(err.error.errorMessage, errMsg);
      assert.strictEqual(err.error.errorCode.number, 6000);
    }
  });

  it("Logs a ProgramError", async () => {
    await withLogTest(async () => {
      try {
        const tx = await program.methods.testProgramError().rpc();
        assert.ok(false);
      } catch (err) {
        expect(err.programErrorStack.map((pk) => pk.toString())).to.deep.equal([
          program.programId.toString(),
        ]);
        expect(err.program.toString()).to.equal(program.programId.toString());
      }
    }, [
      "Program log: ProgramError occurred. Error Code: InvalidAccountData. Error Number: 17179869184. Error Message: An account's data contents was invalid.",
    ]);
  });

  it("Logs a ProgramError with source", async () => {
    await withLogTest(async () => {
      try {
        const tx = await program.methods.testProgramErrorWithSource().rpc();
        assert.ok(false);
      } catch (err) {
        expect(err.programErrorStack.map((pk) => pk.toString())).to.deep.equal([
          program.programId.toString(),
        ]);
      }
    }, [
      "Program log: ProgramError thrown in programs/errors/src/lib.rs:38. Error Code: InvalidAccountData. Error Number: 17179869184. Error Message: An account's data contents was invalid.",
    ]);
  });

  it("Emits a HelloNoMsg error", async () => {
    try {
      const tx = await program.methods.helloNoMsg().rpc();
      assert.ok(false);
    } catch (_err) {
      assert.isTrue(_err instanceof AnchorError);
      const err: AnchorError = _err;
      assert.strictEqual(err.error.errorMessage, "HelloNoMsg");
      assert.strictEqual(err.error.errorCode.number, 6123);
    }
  });

  it("Emits a HelloNext error", async () => {
    try {
      const tx = await program.methods.helloNext().rpc();
      assert.ok(false);
    } catch (_err) {
      assert.isTrue(_err instanceof AnchorError);
      const err: AnchorError = _err;
      assert.strictEqual(err.error.errorMessage, "HelloNext");
      assert.strictEqual(err.error.errorCode.number, 6124);
    }
  });

  it("Emits a mut error", async () => {
    await withLogTest(async () => {
      try {
        const tx = await program.rpc.mutError({
          accounts: {
            myAccount: anchor.web3.SYSVAR_RENT_PUBKEY,
          },
        });
        assert.ok(false);
      } catch (_err) {
        assert.isTrue(_err instanceof AnchorError);
        const err: AnchorError = _err;
        assert.strictEqual(
          err.error.errorMessage,
          "A mut constraint was violated"
        );
        assert.strictEqual(err.error.errorCode.number, 2000);
        assert.strictEqual(err.error.origin, "my_account");
      }
    }, [
      "Program log: AnchorError caused by account: my_account. Error Code: ConstraintMut. Error Number: 2000. Error Message: A mut constraint was violated.",
    ]);
  });

  it("Emits a has one error", async () => {
    await withLogTest(async () => {
      try {
        const account = new Keypair();
        const tx = await program.rpc.hasOneError({
          accounts: {
            myAccount: account.publicKey,
            owner: anchor.web3.SYSVAR_RENT_PUBKEY,
          },
          // this initializes the account.owner variable with Pubkey::default
          instructions: [
            await program.account.hasOneAccount.createInstruction(account),
          ],
          signers: [account],
        });
        assert.ok(false);
      } catch (_err) {
        assert.isTrue(_err instanceof AnchorError);
        const err: AnchorError = _err;
        assert.strictEqual(
          err.error.errorMessage,
          "A has one constraint was violated"
        );
        assert.strictEqual(err.error.errorCode.number, 2001);
        assert.strictEqual(err.error.errorCode.code, "ConstraintHasOne");
        assert.strictEqual(err.error.origin, "my_account");
        assert.strictEqual(
          err.program.toString(),
          program.programId.toString()
        );
        expect(
          err.error.comparedValues.map((pk) => pk.toString())
        ).to.deep.equal([
          "11111111111111111111111111111111",
          "SysvarRent111111111111111111111111111111111",
        ]);
      }
    }, [
      "Program log: AnchorError caused by account: my_account. Error Code: ConstraintHasOne. Error Number: 2001. Error Message: A has one constraint was violated.",
      "Program log: Left:",
      "Program log: 11111111111111111111111111111111",
      "Program log: Right:",
      "Program log: SysvarRent111111111111111111111111111111111",
    ]);
  });

  // This test uses a raw transaction and provider instead of a program
  // instance since the client won't allow one to send a transaction
  // with an invalid signer account.
  it("Emits a signer error", async () => {
    let signature;
    const listener = anchor
      .getProvider()
      .connection.onLogs("all", (logs) => (signature = logs.signature));
    try {
      const tx = new Transaction();
      tx.add(
        new TransactionInstruction({
          keys: [
            {
              pubkey: anchor.web3.SYSVAR_RENT_PUBKEY,
              isWritable: false,
              isSigner: false,
            },
          ],
          programId: program.programId,
          data: program.coder.instruction.encode("signerError", {}),
        })
      );
      await program.provider.sendAndConfirm(tx);
      assert.ok(false);
    } catch (err) {
      // wait for logs to be captured
      for (let i = 0; i < 20; i++) {
        if (signature) {
          break;
        }
        await sleep(100);
      }
      anchor.getProvider().connection.removeOnLogsListener(listener);
      const errMsg = `Error: Raw transaction ${signature} failed ({"err":{"InstructionError":[0,{"Custom":3010}]}})`;
      assert.strictEqual(err.toString(), errMsg);
    } finally {
      anchor.getProvider().connection.removeOnLogsListener(listener);
    }
  });

  it("Emits a raw custom error", async () => {
    try {
      const tx = await program.rpc.rawCustomError({
        accounts: {
          myAccount: anchor.web3.SYSVAR_RENT_PUBKEY,
        },
      });
      assert.ok(false);
    } catch (_err) {
      assert.isTrue(_err instanceof AnchorError);
      const err: AnchorError = _err;
      const errMsg = "HelloCustom";
      assert.strictEqual(err.error.errorMessage, errMsg);
      assert.strictEqual(err.error.errorCode.number, 6125);
    }
  });

  it("Emits a account not initialized error", async () => {
    await withLogTest(async () => {
      try {
        const tx = await program.rpc.accountNotInitializedError({
          accounts: {
            notInitializedAccount: new anchor.web3.Keypair().publicKey,
          },
        });
        assert.fail(
          "Unexpected success in creating a transaction that should have fail with `AccountNotInitialized` error"
        );
      } catch (_err) {
        assert.isTrue(_err instanceof AnchorError);
        const err: AnchorError = _err;
        const errMsg =
          "The program expected this account to be already initialized";
        assert.strictEqual(err.error.errorMessage, errMsg);
      }
    }, [
      "Program log: AnchorError caused by account: not_initialized_account. Error Code: AccountNotInitialized. Error Number: 3012. Error Message: The program expected this account to be already initialized.",
    ]);
  });

  it("Emits an AccountOwnedByWrongProgram error", async () => {
    let client = await Token.createMint(
      program.provider.connection,
      (provider.wallet as anchor.Wallet).payer,
      provider.wallet.publicKey,
      provider.wallet.publicKey,
      9,
      TOKEN_PROGRAM_ID
    );

    await withLogTest(async () => {
      try {
        const tx = await program.rpc.accountOwnedByWrongProgramError({
          accounts: {
            wrongAccount: client.publicKey,
          },
        });
        assert.fail(
          "Unexpected success in creating a transaction that should have failed with `AccountOwnedByWrongProgram` error"
        );
      } catch (_err) {
        assert.isTrue(_err instanceof AnchorError);
        const err: AnchorError = _err;
        const errMsg =
          "The given account is owned by a different program than expected";
        assert.strictEqual(err.error.errorMessage, errMsg);
      }
    }, [
      "Program log: AnchorError caused by account: wrong_account. Error Code: AccountOwnedByWrongProgram. Error Number: 3007. Error Message: The given account is owned by a different program than expected.",
      "Program log: Left:",
      `Program log: ${TOKEN_PROGRAM_ID}`,
      "Program log: Right:",
      `Program log: ${program.programId.toString()}`,
    ]);
  });

  it("Emits a ValueMismatch error via require_eq", async () => {
    await withLogTest(async () => {
      try {
        const tx = await program.methods.requireEq().rpc();
        assert.fail(
          "Unexpected success in creating a transaction that should have failed with `ValueMismatch` error"
        );
      } catch (_err) {
        assert.isTrue(_err instanceof AnchorError);
        const err: AnchorError = _err;
        assert.strictEqual(err.error.errorCode.number, 6126);
        expect(err.error.comparedValues).to.deep.equal(["5241", "124124124"]);
      }
    }, [
      "Program log: AnchorError thrown in programs/errors/src/lib.rs:68. Error Code: ValueMismatch. Error Number: 6126. Error Message: ValueMismatch.",
      "Program log: Left: 5241",
      "Program log: Right: 124124124",
    ]);
  });

  it("Emits a RequireEqViolated error via require_eq", async () => {
    await withLogTest(async () => {
      try {
        const tx = await program.methods.requireEqDefaultError().rpc();
        assert.fail(
          "Unexpected success in creating a transaction that should have failed with `ValueMismatch` error"
        );
      } catch (_err) {
        assert.isTrue(_err instanceof AnchorError);
        const err: AnchorError = _err;
        assert.strictEqual(err.error.errorCode.number, 2501);
      }
    }, [
      "Program log: AnchorError thrown in programs/errors/src/lib.rs:73. Error Code: RequireEqViolated. Error Number: 2501. Error Message: A require_eq expression was violated.",
      "Program log: Left: 5241",
      "Program log: Right: 124124124",
    ]);
  });

  it("Emits a ValueMatch error via require_neq", async () => {
    await withLogTest(async () => {
      try {
        const tx = await program.methods.requireNeq().rpc();
        assert.fail(
          "Unexpected success in creating a transaction that should have failed with `ValueMatch` error"
        );
      } catch (_err) {
        assert.isTrue(_err instanceof AnchorError);
        const err: AnchorError = _err;
        assert.strictEqual(err.error.errorCode.number, 6127);
      }
    }, [
      "Program log: AnchorError thrown in programs/errors/src/lib.rs:78. Error Code: ValueMatch. Error Number: 6127. Error Message: ValueMatch.",
      "Program log: Left: 500",
      "Program log: Right: 500",
    ]);
  });

  it("Emits a RequireNeqViolated error via require_neq", async () => {
    await withLogTest(async () => {
      try {
        const tx = await program.methods.requireNeqDefaultError().rpc();
        assert.fail(
          "Unexpected success in creating a transaction that should have failed with `RequireNeqViolated` error"
        );
      } catch (_err) {
        assert.isTrue(_err instanceof AnchorError);
        const err: AnchorError = _err;
        assert.strictEqual(err.error.errorCode.number, 2503);
      }
    }, [
      "Program log: AnchorError thrown in programs/errors/src/lib.rs:83. Error Code: RequireNeqViolated. Error Number: 2503. Error Message: A require_neq expression was violated.",
      "Program log: Left: 500",
      "Program log: Right: 500",
    ]);
  });

  it("Emits a ValueMismatch error via require_keys_eq", async () => {
    const someAccount = anchor.web3.Keypair.generate().publicKey;
    await withLogTest(async () => {
      try {
        const tx = await program.rpc.requireKeysEq({
          accounts: {
            someAccount,
          },
        });
        assert.fail(
          "Unexpected success in creating a transaction that should have failed with `ValueMismatch` error"
        );
      } catch (_err) {
        assert.isTrue(_err instanceof AnchorError);
        const err: AnchorError = _err;
        assert.strictEqual(err.error.errorCode.number, 6126);
      }
    }, [
      "Program log: AnchorError thrown in programs/errors/src/lib.rs:88. Error Code: ValueMismatch. Error Number: 6126. Error Message: ValueMismatch.",
      "Program log: Left:",
      `Program log: ${someAccount}`,
      "Program log: Right:",
      `Program log: ${program.programId}`,
    ]);
  });

  it("Emits a RequireKeysEqViolated error via require_keys_eq", async () => {
    const someAccount = anchor.web3.Keypair.generate().publicKey;
    await withLogTest(async () => {
      try {
        const tx = await program.rpc.requireKeysEqDefaultError({
          accounts: {
            someAccount,
          },
        });
        assert.fail(
          "Unexpected success in creating a transaction that should have failed with `ValueMismatch` error"
        );
      } catch (_err) {
        assert.isTrue(_err instanceof AnchorError);
        const err: AnchorError = _err;
        assert.strictEqual(err.error.errorCode.number, 2502);
      }
    }, [
      "Program log: AnchorError thrown in programs/errors/src/lib.rs:97. Error Code: RequireKeysEqViolated. Error Number: 2502. Error Message: A require_keys_eq expression was violated.",
      "Program log: Left:",
      `Program log: ${someAccount}`,
      "Program log: Right:",
      `Program log: ${program.programId}`,
    ]);
  });

  it("Emits a ValueMatch error via require_keys_neq", async () => {
    const someAccount = program.programId;
    await withLogTest(async () => {
      try {
        const tx = await program.rpc.requireKeysNeq({
          accounts: {
            someAccount,
          },
        });
        assert.fail(
          "Unexpected success in creating a transaction that should have failed with `ValueMatch` error"
        );
      } catch (_err) {
        assert.isTrue(_err instanceof AnchorError);
        const err: AnchorError = _err;
        assert.strictEqual(err.error.errorCode.number, 6127);
      }
    }, [
      "Program log: AnchorError thrown in programs/errors/src/lib.rs:102. Error Code: ValueMatch. Error Number: 6127. Error Message: ValueMatch.",
      "Program log: Left:",
      `Program log: ${someAccount}`,
      "Program log: Right:",
      `Program log: ${program.programId}`,
    ]);
  });

  it("Emits a RequireKeysNeqViolated error via require_keys_neq", async () => {
    const someAccount = program.programId;
    await withLogTest(async () => {
      try {
        const tx = await program.rpc.requireKeysNeqDefaultError({
          accounts: {
            someAccount,
          },
        });
        assert.fail(
          "Unexpected success in creating a transaction that should have failed with `RequireKeysNeqViolated` error"
        );
      } catch (_err) {
        assert.isTrue(_err instanceof AnchorError);
        const err: AnchorError = _err;
        assert.strictEqual(err.error.errorCode.number, 2504);
      }
    }, [
      "Program log: AnchorError thrown in programs/errors/src/lib.rs:111. Error Code: RequireKeysNeqViolated. Error Number: 2504. Error Message: A require_keys_neq expression was violated.",
      "Program log: Left:",
      `Program log: ${someAccount}`,
      "Program log: Right:",
      `Program log: ${program.programId}`,
    ]);
  });

  it("Emits a ValueLessOrEqual error via require_gt", async () => {
    await withLogTest(async () => {
      try {
        const tx = await program.methods.requireGt().rpc();
        assert.fail(
          "Unexpected success in creating a transaction that should have failed with `ValueLessOrEqual` error"
        );
      } catch (_err) {
        assert.isTrue(_err instanceof AnchorError);
        const err: AnchorError = _err;
        assert.strictEqual(err.error.errorCode.number, 6129);
      }
    }, [
      "Program log: AnchorError thrown in programs/errors/src/lib.rs:116. Error Code: ValueLessOrEqual. Error Number: 6129. Error Message: ValueLessOrEqual.",
      "Program log: Left: 5",
      "Program log: Right: 10",
    ]);
  });

  it("Emits a RequireGtViolated error via require_gt", async () => {
    await withLogTest(async () => {
      try {
        const tx = await program.methods.requireGtDefaultError().rpc();
        assert.fail(
          "Unexpected success in creating a transaction that should have failed with `RequireGtViolated` error"
        );
      } catch (_err) {
        assert.isTrue(_err instanceof AnchorError);
        const err: AnchorError = _err;
        assert.strictEqual(err.error.errorCode.number, 2505);
      }
    }, [
      "Program log: AnchorError thrown in programs/errors/src/lib.rs:121. Error Code: RequireGtViolated. Error Number: 2505. Error Message: A require_gt expression was violated.",
      "Program log: Left: 10",
      "Program log: Right: 10",
    ]);
  });

  it("Emits a ValueLess error via require_gte", async () => {
    await withLogTest(async () => {
      try {
        const tx = await program.methods.requireGte().rpc();
        assert.fail(
          "Unexpected success in creating a transaction that should have failed with `ValueLess` error"
        );
      } catch (_err) {
        assert.isTrue(_err instanceof AnchorError);
        const err: AnchorError = _err;
        assert.strictEqual(err.error.errorCode.number, 6128);
      }
    }, [
      "Program log: AnchorError thrown in programs/errors/src/lib.rs:126. Error Code: ValueLess. Error Number: 6128. Error Message: ValueLess.",
      "Program log: Left: 5",
      "Program log: Right: 10",
    ]);
  });

  it("Emits a RequireGteViolated error via require_gte", async () => {
    await withLogTest(async () => {
      try {
        const tx = await program.methods.requireGteDefaultError().rpc();
        assert.fail(
          "Unexpected success in creating a transaction that should have failed with `RequireGteViolated` error"
        );
      } catch (_err) {
        assert.isTrue(_err instanceof AnchorError);
        const err: AnchorError = _err;
        assert.strictEqual(err.error.errorCode.number, 2506);
      }
    }, [
      "Program log: AnchorError thrown in programs/errors/src/lib.rs:131. Error Code: RequireGteViolated. Error Number: 2506. Error Message: A require_gte expression was violated.",
      "Program log: Left: 5",
      "Program log: Right: 10",
    ]);
  });

  it("Emits a InvalidNumericConversion error via try_into", async () => {
    await withLogTest(async () => {
      try {
        const tx = await program.methods.tryIntoInteger().rpc();
        assert.fail(
          "Unexpected success in creating a transaction that should have failed with `InvalidNumericConversion` error"
        );
      } catch (_err) {
        assert.isTrue(_err instanceof AnchorError);
        const err: AnchorError = _err;
        assert.strictEqual(err.error.errorCode.number, 4102);
      }
    }, [
      "Program log: AnchorError occurred. Error Code: InvalidNumericConversion. Error Number: 4102. Error Message: out of range integral type conversion attempted.",
    ]);
  });
});
