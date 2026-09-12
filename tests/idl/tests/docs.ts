import * as anchor from "@anchor-lang/core";
import { Program } from "@anchor-lang/core";
import { assert } from "chai";

import { Docs } from "../target/types/docs";

describe("Docs", () => {
  anchor.setProvider(anchor.AnchorProvider.env());
  const program = anchor.workspace.docs as Program<Docs>;

  const instruction = program.idl.instructions.find(
    (i) => i.name === "testIdlDocParse"
  )!;

  it("includes instruction doc comment", () => {
    assert.deepEqual(instruction.docs, [
      "This instruction doc should appear in the IDL",
    ]);
  });

  it("includes account doc comment", () => {
    const act = instruction.accounts.find((i) => i.name === "act")!;
    assert.deepEqual(act.docs, [
      "This account doc comment should appear in the IDL",
      "This is a multi-line comment",
    ]);
  });

  const dataWithDoc = program.idl.types.find(
    (ty) => ty.name === "dataWithDoc"
  )!;

  it("includes type doc comment", () => {
    assert.deepEqual(dataWithDoc.docs, [
      "Custom account doc comment should appear in the IDL",
    ]);
  });

  it("includes account attribute doc comment", () => {
    const dataField = dataWithDoc.type.fields.find((i) => i.name === "data")!;
    assert.deepEqual(dataField.docs, [
      "Account attribute doc comment should appear in the IDL",
    ]);
  });

  it("includes constant doc comment", () => {
    const myConst = program.idl.constants.find((c) => c.name === "myConst")!;
    assert.deepEqual(myConst.docs, ["Documentation comment for constant"]);
  });
});
