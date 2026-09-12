import * as anchor from "@anchor-lang/core";
import { assert } from "chai";

describe("Workspace", () => {
  anchor.setProvider(anchor.AnchorProvider.env());

  it("Can lazy load workspace programs", () => {
    assert.doesNotThrow(() => {
      // Program exists, should not throw
      anchor.workspace.relationsDerivation;
    });

    assert.throws(() => {
      // IDL path in Anchor.toml doesn't exist but other tests still run
      // successfully because workspace programs are getting loaded on-demand
      anchor.workspace.nonExistent;
    }, /non-existent\.json/);
  });

  it("Can get workspace programs by their name independent of casing", () => {
    const camel = anchor.workspace.relationsDerivation;
    const pascal = anchor.workspace.RelationsDerivation;
    const kebab = anchor.workspace["relations-derivation"];
    const snake = anchor.workspace["relations_derivation"];

    const compareProgramNames = (...programs: anchor.Program[]) => {
      return programs.every(
        (program) => program.rawIdl.metadata.name === "relations_derivation"
      );
    };

    assert(compareProgramNames(camel, pascal, kebab, snake));
  });

  it("Can use numbers in program names", () => {
    assert.doesNotThrow(() => {
      anchor.workspace.numbers123;
      anchor.workspace.Numbers123;
      anchor.workspace["numbers-123"];
      anchor.workspace["numbers_123"];
    });
  });
});
