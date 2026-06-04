import * as anchor from "@anchor-lang/core";
import { Program } from "@anchor-lang/core";
import { Example } from "../target/types/example";
import { assert } from "chai";

describe("multiple-scripts: extended test suite (selected via --script test2)", () => {
  anchor.setProvider(anchor.AnchorProvider.env());

  const program = anchor.workspace.Example as Program<Example>;

  it("initializes repeatedly", async () => {
    const signatures: string[] = [];
    for (let i = 0; i < 3; i++) {
      const tx = await program.methods.initialize().rpc();
      assert.ok(tx, `Transaction ${i} should have a signature`);
      signatures.push(tx);
    }
    assert.strictEqual(
      new Set(signatures).size,
      signatures.length,
      "Each call should produce a distinct signature"
    );
  });
});
