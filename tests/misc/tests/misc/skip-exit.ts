import * as anchor from "@anchor-lang/core";
import { Program } from "@anchor-lang/core";
import { assert } from "chai";

import { Misc } from "../../target/types/misc";

describe("skip_exit", () => {
  const provider = anchor.AnchorProvider.env();
  anchor.setProvider(provider);
  const program = anchor.workspace.misc as Program<Misc>;

  it("does not persist in-memory Account mutations", async () => {
    const data = anchor.web3.Keypair.generate();
    const initialUdata = new anchor.BN(1);
    const initialIdata = new anchor.BN(2);

    await program.methods
      .initialize(initialUdata, initialIdata)
      .accounts({ data: data.publicKey })
      .signers([data])
      .preInstructions([await program.account.data.createInstruction(data)])
      .rpc();

    await program.methods
      .testSkipExit(new anchor.BN(100), new anchor.BN(200))
      .accounts({ data: data.publicKey })
      .rpc();

    const dataAccount = await program.account.data.fetch(data.publicKey);
    assert(dataAccount.udata.eq(initialUdata));
    assert(dataAccount.idata.eq(initialIdata));
  });
});
