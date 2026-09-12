import * as anchor from "@anchor-lang/core";
import { Program } from "@anchor-lang/core";
import { assert } from "chai";
import { MultipleSuitesRunSingle } from "../../target/types/multiple_suites_run_single";

describe("multiple-suites-run-single", () => {
  // Configure the client to use the local cluster.
  anchor.setProvider(anchor.AnchorProvider.env());

  const program = anchor.workspace
    .MultipleSuitesRunSingle as Program<MultipleSuitesRunSingle>;

  it("Is initialized!", async () => {
    const tx = await program.methods.initialize().rpc();
    console.log("Your transaction signature", tx);
  });
});
