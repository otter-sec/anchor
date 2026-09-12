import * as anchor from "@anchor-lang/core";

import { Lamports } from "../../target/types/lamports";

describe("lamports", () => {
  // Configure the client to use the local cluster
  anchor.setProvider(anchor.AnchorProvider.env());

  const program = anchor.workspace.Lamports as anchor.Program<Lamports>;

  it("Can transfer from/to PDA", async () => {
    await program.methods
      .transfer(new anchor.BN(anchor.web3.LAMPORTS_PER_SOL))
      .rpc();
  });

  it("Returns an error on overflow", async () => {
    await program.methods.overflow().rpc();
  });
});
