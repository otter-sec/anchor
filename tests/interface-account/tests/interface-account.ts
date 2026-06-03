import * as anchor from "@anchor-lang/core";
import assert from "assert";

import type { InterfaceAccount } from "../target/types/interface_account";
import type { New } from "../target/types/new";
import type { Old } from "../target/types/old";

describe("interface-account", () => {
  anchor.setProvider(anchor.AnchorProvider.env());
  const program: anchor.Program<InterfaceAccount> =
    anchor.workspace.interfaceAccount;
  const oldProgram: anchor.Program<Old> = anchor.workspace.old;
  const newProgram: anchor.Program<New> = anchor.workspace.new;

  const oldExpectedAccount = anchor.web3.Keypair.generate();
  const newExpectedAccount = anchor.web3.Keypair.generate();
  const anotherAccount = anchor.web3.Keypair.generate();

  // Initialize accounts
  before(async () => {
    // Initialize the expected account of the old program
    await oldProgram.methods
      .init()
      .accounts({ expectedAccount: oldExpectedAccount.publicKey })
      .signers([oldExpectedAccount])
      .rpc();

    // Initialize the expected account of the new program
    await newProgram.methods
      .init()
      .accounts({ expectedAccount: newExpectedAccount.publicKey })
      .signers([newExpectedAccount])
      .rpc();

    // Initialize another account of the new program
    await newProgram.methods
      .initAnother()
      .accounts({ anotherAccount: anotherAccount.publicKey })
      .signers([anotherAccount])
      .rpc();
  });

  it("Allows old exptected accounts", async () => {
    await program.methods
      .test()
      .accounts({ expectedAccount: oldExpectedAccount.publicKey })
      .rpc();
  });

  it("Allows new exptected accounts", async () => {
    await program.methods
      .test()
      .accounts({ expectedAccount: newExpectedAccount.publicKey })
      .rpc();
  });

  it("Doesn't allow accounts owned by other programs", async () => {
    try {
      await program.methods
        .test()
        .accounts({ expectedAccount: program.provider.wallet!.publicKey })
        .rpc();

      assert.fail("Allowed unexpected account substitution!");
    } catch (e) {
      assert(e instanceof anchor.AnchorError);
      assert.strictEqual(
        e.error.errorCode.number,
        anchor.LangErrorCode.AccountOwnedByWrongProgram
      );
    }
  });

  it("Doesn't allow unexpected accounts owned by the expected programs", async () => {
    try {
      await program.methods
        .test()
        .accounts({ expectedAccount: anotherAccount.publicKey })
        .rpc();

      assert.fail("Allowed unexpected account substitution!");
    } catch (e) {
      assert(e instanceof anchor.AnchorError);
      assert.strictEqual(
        e.error.errorCode.number,
        anchor.LangErrorCode.AccountDiscriminatorMismatch
      );
    }
  });
});
