import * as anchor from "@anchor-lang/core";
import { Program } from "@anchor-lang/core";
import { AnchorCliLegacyIdl } from "../target/types/anchor_cli_legacy_idl";
import { assert } from "chai";
import { execSync } from "child_process";
import * as fs from "fs";
import { fail } from "assert";
import { PublicKey, SendTransactionError } from "@solana/web3.js";

describe("anchor-cli-legacy-idl", () => {
  // Configure the client to use the local cluster.
  const provider = anchor.AnchorProvider.env();
  anchor.setProvider(provider);

  const program = anchor.workspace
    .anchorCliLegacyIdl as Program<AnchorCliLegacyIdl>;

  // test/fixtures/dummy_auth.json
  const dummyAuth = new PublicKey(
    "9MvnEdtFadjv96b5bHtkaErq1Cj7GsW1ZxoeJH5fXQiv"
  );
  let bufferAddress: PublicKey | undefined = undefined;

  before("setup", async () => {
    await provider.connection.requestAirdrop(dummyAuth, 1_000_000_000);
  });

  it("Can initialize legacy IDL account", async () => {
    const initIdlPath = "target/idl/anchor_cli_legacy_idl.json";
    const fetchedIdlPath = "fetched_idl.json";

    execSync(
      `anchor legacy-idl init --filepath ${initIdlPath} ${program.programId}`,
      { stdio: "inherit" }
    );

    execSync(
      `anchor legacy-idl fetch ${program.programId} -o ${fetchedIdlPath}`,
      { stdio: "inherit" }
    );

    const initialIdl = JSON.parse(fs.readFileSync(initIdlPath, "utf-8"));
    const fetchedIdl = JSON.parse(fs.readFileSync(fetchedIdlPath, "utf-8"));
    assert.deepEqual(
      fetchedIdl,
      initialIdl,
      "Fetched IDL should match the initialized IDL"
    );
  });

  it("Can write a new legacy IDL using the upgrade command", async () => {
    const upgradeIdlPath = "tests/fixtures/fake_idl.json";
    const fetchedIdlPath = "fetched_idl2.json";

    execSync(
      `anchor legacy-idl upgrade --filepath ${upgradeIdlPath} ${program.programId}`,
      { stdio: "inherit" }
    );

    execSync(
      `anchor legacy-idl fetch ${program.programId} -o ${fetchedIdlPath}`,
      { stdio: "inherit" }
    );

    const upgradeIdl = JSON.parse(fs.readFileSync(upgradeIdlPath, "utf-8"));
    const fetchedIdl = JSON.parse(fs.readFileSync(fetchedIdlPath, "utf-8"));
    assert.deepEqual(
      fetchedIdl,
      upgradeIdl,
      "Fetched IDL should match the upgraded IDL"
    );
  });

  it("Can write a new legacy IDL using the write-buffer command", async () => {
    const upgradeIdlPath = "target/idl/anchor_cli_legacy_idl.json";
    const fetchedIdlPath = "fetched_idl3.json";

    const writeBufferOutput = execSync(
      `anchor legacy-idl write-buffer --filepath ${upgradeIdlPath} ${program.programId}`
    ).toString();

    const lastLine = writeBufferOutput.trimEnd().split("\n").pop()!;
    bufferAddress = new PublicKey(
      lastLine.split("Idl buffer created: ")[1].trim()
    );

    execSync(`anchor legacy-idl fetch ${bufferAddress} -o ${fetchedIdlPath}`, {
      stdio: "inherit",
    });

    const writeIdl = JSON.parse(fs.readFileSync(upgradeIdlPath, "utf-8"));
    const fetchedIdl = JSON.parse(fs.readFileSync(fetchedIdlPath, "utf-8"));
    assert.deepEqual(
      fetchedIdl,
      writeIdl,
      "Fetched IDL should match the upgraded IDL"
    );
  });

  it("Can set-authority on buffer account", async () => {
    execSync(
      `anchor legacy-idl set-authority ${bufferAddress} --new-authority ${dummyAuth} -p ${program.programId}`,
      { stdio: "inherit" }
    );

    let authorityStdOut = execSync(
      `anchor legacy-idl authority ${bufferAddress}`
    ).toString();
    let lastLine = authorityStdOut.trimEnd().split("\n").pop()!;
    let address = lastLine.split(" ").pop()!;

    assert.equal(address, dummyAuth.toBase58());

    execSync(
      `anchor legacy-idl set-authority ${bufferAddress} --new-authority  ${provider.publicKey} --provider.wallet tests/fixtures/dummy_auth.json  -p ${program.programId}`,
      { stdio: "inherit" }
    );

    authorityStdOut = execSync(
      `anchor legacy-idl authority ${bufferAddress}`
    ).toString();
    lastLine = authorityStdOut.trimEnd().split("\n").pop()!;
    address = lastLine.split(" ").pop()!;

    assert.equal(address, provider.publicKey.toBase58());
  });

  it("Can change IDL via the set-buffer command", async () => {
    const upgradeIdlPath = "target/idl/anchor_cli_legacy_idl.json";
    const fetchedIdlPath = "fetched_idl4.json";

    execSync(
      `anchor legacy-idl set-buffer --buffer ${bufferAddress} ${program.programId}`,
      { stdio: "inherit" }
    );

    execSync(
      `anchor legacy-idl fetch ${program.programId} -o ${fetchedIdlPath}`,
      { stdio: "inherit" }
    );

    const writeIdl = JSON.parse(fs.readFileSync(upgradeIdlPath, "utf-8"));
    const fetchedIdl = JSON.parse(fs.readFileSync(fetchedIdlPath, "utf-8"));
    assert.deepEqual(
      fetchedIdl,
      writeIdl,
      "Fetched IDL should match the upgraded IDL"
    );
  });

  it("Can close legacy IDL using the close command", async () => {
    execSync(`anchor legacy-idl close ${program.programId}`, {
      stdio: "inherit",
    });

    const programSigner = anchor.web3.PublicKey.findProgramAddressSync(
      [],
      program.programId
    )[0];
    const idlAddress = await anchor.web3.PublicKey.createWithSeed(
      programSigner,
      "anchor:idl",
      program.programId
    );

    const idlAccount = await provider.connection.getAccountInfo(idlAddress);

    assert(!idlAccount);
  });

  it("Can erase-authority on the legacy IDL account", async () => {
    const initIdlPath = "target/idl/anchor_cli_legacy_idl.json";

    execSync(
      `anchor legacy-idl init --filepath ${initIdlPath} ${program.programId}`,
      { stdio: "inherit" }
    );

    let authorityStdOut = execSync(
      `anchor legacy-idl authority ${bufferAddress}`
    ).toString();
    let lastLine = authorityStdOut.trimEnd().split("\n").pop()!;
    let address = lastLine.split(" ").pop()!;

    assert.equal(address, provider.publicKey.toBase58());

    execSync(
      `anchor legacy-idl erase-authority --program-id ${program.programId}`,
      { input: "y\n", stdio: ["pipe", "inherit", "inherit"] }
    );

    authorityStdOut = execSync(
      `anchor legacy-idl authority ${program.programId}`
    ).toString();
    lastLine = authorityStdOut.trimEnd().split("\n").pop()!;
    address = lastLine.split(" ").pop()!;

    assert.equal(address, anchor.web3.SystemProgram.programId.toBase58());
  });

  it("Can't call CreateBuffer without buffer signing", async () => {
    const kp = anchor.web3.Keypair.generate();

    const createIx = anchor.web3.SystemProgram.createAccount({
      fromPubkey: provider.publicKey,
      lamports: 1_000_000_000,
      newAccountPubkey: kp.publicKey,
      programId: program.programId,
      space: 50,
    });

    await program.methods
      .initialize()
      .accounts({
        acc: kp.publicKey,
      })
      .preInstructions([createIx])
      .signers([kp])
      .rpc();

    const IDL_IX_TAG = [64, 244, 188, 120, 167, 233, 105, 10];
    const IDL_CREATE_BUFFER_IX_TAG = [1];

    const data = Buffer.concat([
      Buffer.from(IDL_IX_TAG),
      Buffer.from(IDL_CREATE_BUFFER_IX_TAG),
    ]);

    const ix = new anchor.web3.TransactionInstruction({
      keys: [
        {
          isSigner: false,
          isWritable: true,
          pubkey: kp.publicKey,
        },
        {
          isSigner: true,
          isWritable: false,
          pubkey: provider.publicKey,
        },
      ],
      programId: program.programId,
      data,
    });

    const tx = new anchor.web3.Transaction();
    tx.add(ix);

    try {
      await provider.sendAndConfirm(tx);
      assert.fail("should have thrown");
    } catch (e) {
      assert.include(
        e.toString(),
        "AnchorError caused by account: buffer. Error Code: ConstraintSigner. Error Number: 2002. Error Message: A signer constraint was violated"
      );
    }
  });
});
