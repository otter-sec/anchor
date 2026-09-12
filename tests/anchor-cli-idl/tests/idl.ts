import * as anchor from "@anchor-lang/core";
import { Program } from "@anchor-lang/core";
import { IdlCommandsOne } from "../target/types/idl_commands_one";
import { IdlCommandsTwo } from "../target/types/idl_commands_two";
import { assert } from "chai";
import { execSync } from "child_process";
import * as fs from "fs";

describe("Test CLI IDL commands", () => {
  // Configure the client to use the local cluster.
  const provider = anchor.AnchorProvider.env();

  anchor.setProvider(provider);

  const programOne = anchor.workspace.IdlCommandsOne as Program<IdlCommandsOne>;
  const programTwo = anchor.workspace.IdlCommandsTwo as Program<IdlCommandsTwo>;

  it("Can initialize IDL account", async () => {
    execSync(
      `anchor idl init --filepath target/idl/idl_commands_one.json ${programOne.programId}`,
      { stdio: "inherit" }
    );
  });

  it("Can fetch an IDL using the TypeScript client", async () => {
    const idl = await anchor.Program.fetchIdl(programOne.programId, provider);
    assert.deepEqual(idl, programOne.rawIdl);
  });

  it("Can fetch an IDL via the CLI", async () => {
    const idl = execSync(`anchor idl fetch ${programOne.programId}`).toString();
    assert.deepEqual(JSON.parse(idl), programOne.rawIdl);
  });

  it("Can write a new IDL using the upgrade command", async () => {
    // Upgrade the IDL of program one to the IDL of program two to test upgrade
    execSync(
      `anchor idl upgrade --filepath target/idl/idl_commands_two.json ${programOne.programId}`,
      { stdio: "inherit" }
    );
    const idl = await anchor.Program.fetchIdl(programOne.programId, provider);
    assert.deepEqual(idl, programTwo.rawIdl);
  });

  it("Can write a new IDL using write-buffer and set-buffer", async () => {
    // "Upgrade" back to program one via write-buffer set-buffer
    let buffer = execSync(
      `anchor idl write-buffer --filepath target/idl/idl_commands_one.json ${programOne.programId}`
    ).toString();
    buffer = buffer.replace("Idl buffer created: ", "").trim();
    execSync(
      `anchor idl set-buffer --buffer ${buffer} ${programOne.programId}`,
      { stdio: "inherit" }
    );
    const idl = await anchor.Program.fetchIdl(programOne.programId, provider);
    assert.deepEqual(idl, programOne.rawIdl);
  });

  it("Can fetch an IDL authority via the CLI", async () => {
    const authority = execSync(`anchor idl authority ${programOne.programId}`)
      .toString()
      .trim();

    assert.equal(authority, provider.wallet.publicKey.toString());
  });

  it("Can close IDL account", async () => {
    execSync(`anchor idl close ${programOne.programId}`, { stdio: "inherit" });
    const idl = await anchor.Program.fetchIdl(programOne.programId, provider);
    assert.isNull(idl);
  });

  it("Can initialize super massive IDL account", async () => {
    execSync(
      `anchor idl init --filepath testLargeIdl.json ${programOne.programId}`,
      { stdio: "inherit" }
    );
    const idlActual = await anchor.Program.fetchIdl(
      programOne.programId,
      provider
    );
    const idlExpected = JSON.parse(
      fs.readFileSync("testLargeIdl.json", "utf8")
    );
    assert.deepEqual(idlActual, idlExpected);
  });
});
