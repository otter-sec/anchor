import * as anchor from "@anchor-lang/core";
import { Program } from "@anchor-lang/core";
import { Example } from "../target/types/example";
import { assert } from "chai";

describe("multiple-scripts: default test suite", () => {
  anchor.setProvider(anchor.AnchorProvider.env());

  const program = anchor.workspace.example as Program<Example>;

  it("initializes", async () => {
    const tx = await program.methods.initialize().rpc();
    assert.ok(tx, "Transaction should have a signature");
  });
});
