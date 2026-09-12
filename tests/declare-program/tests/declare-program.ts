import * as anchor from "@anchor-lang/core";
import assert from "assert";

import type { DeclareProgram } from "../target/types/declare_program";
import type { External } from "../target/types/external";

describe("declare-program", () => {
  anchor.setProvider(anchor.AnchorProvider.env());
  const program: anchor.Program<DeclareProgram> =
    anchor.workspace.declareProgram;
  const externalProgram: anchor.Program<External> = anchor.workspace.external;

  // TODO: Add a utility type that does this?
  let pubkeys: Awaited<
    ReturnType<
      ReturnType<typeof externalProgram["methods"]["init"]>["rpcAndKeys"]
    >
  >["pubkeys"];

  before(async () => {
    pubkeys = (await externalProgram.methods.init().rpcAndKeys()).pubkeys;
  });

  it("Can CPI", async () => {
    const value = 5;
    await program.methods
      .cpi(value)
      .accounts({ cpiMyAccount: pubkeys.myAccount })
      .rpc();

    const myAccount = await externalProgram.account.myAccount.fetch(
      pubkeys.myAccount
    );
    assert.strictEqual(myAccount.field, value);
  });

  it("Can CPI composite", async () => {
    const value = 3;
    await program.methods
      .cpiComposite(value)
      .accounts({ cpiMyAccount: pubkeys.myAccount })
      .rpc();

    const myAccount = await externalProgram.account.myAccount.fetch(
      pubkeys.myAccount
    );
    assert.strictEqual(myAccount.field, value);
  });

  it("Can use account utils", async () => {
    await program.methods.accountUtils().rpc();
  });

  it("Can use event utils", async () => {
    await program.methods.eventUtils().rpc();
  });
});
