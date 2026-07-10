import * as anchor from "@anchor-lang/core";
import {
  Program,
  web3,
  BN,
  AnchorError,
  LangErrorCode,
  LangErrorMessage,
  translateError,
  parseIdlErrors,
} from "@anchor-lang/core";
import { Optional } from "../target/types/optional";
import { AllowMissingOptionals } from "../target/types/allow_missing_optionals";
import { assert, expect } from "chai";

describe("Optional", () => {
  // configure the client to use the local cluster
  anchor.setProvider(anchor.AnchorProvider.env());
  const anchorProvider = anchor.AnchorProvider.env();
  const program = anchor.workspace.optional as Program<Optional>;

  const DATA_PDA_PREFIX = "data_pda";

  const makeDataPdaSeeds = (dataAccount: web3.PublicKey) => {
    return [Buffer.from(DATA_PDA_PREFIX), dataAccount.toBuffer()];
  };

  const findDataPda = (
    dataAccount: web3.PublicKey
  ): [web3.PublicKey, number] => {
    return web3.PublicKey.findProgramAddressSync(
      makeDataPdaSeeds(dataAccount),
      program.programId
    );
  };

  // payer of the transactions
  const payerWallet = (program.provider as anchor.AnchorProvider).wallet;
  const payer = payerWallet.publicKey;
  const systemProgram = web3.SystemProgram.programId;

  const requiredKeypair1 = web3.Keypair.generate();
  const requiredKeypair2 = web3.Keypair.generate();

  let createRequiredIx1: web3.TransactionInstruction;
  let createRequiredIx2: web3.TransactionInstruction;

  const dataAccountKeypair1 = web3.Keypair.generate();
  const dataAccountKeypair2 = web3.Keypair.generate();

  const dataPda1 = findDataPda(dataAccountKeypair1.publicKey);
  const dataPda2 = findDataPda(dataAccountKeypair2.publicKey);

  const initializeValue1 = new BN(10);
  const initializeValue2 = new BN(100);
  const initializeKey = web3.PublicKey.default;

  const createRequired = async (
    requiredKeypair?: web3.Keypair
  ): Promise<[web3.Keypair, web3.TransactionInstruction]> => {
    const keypair = requiredKeypair ?? new web3.Keypair();
    const createIx = await program.account.dataAccount.createInstruction(
      keypair
    );
    return [keypair, createIx];
  };

  before("Setup async stuff", async () => {
    createRequiredIx1 = (await createRequired(requiredKeypair1))[1];
    createRequiredIx2 = (await createRequired(requiredKeypair2))[1];
  });

  describe("Missing optionals feature tests", async () => {
    it("Fails with missing optional accounts at the end by default", async () => {
      const [requiredKeypair, createRequiredIx] = await createRequired();
      const initializeIx = await program.methods
        .initialize(initializeValue1, initializeKey)
        .accountsPartial({
          payer: null,
          optionalAccount: null,
          systemProgram,
          required: requiredKeypair.publicKey,
          optionalPda: null,
        })
        .signers([requiredKeypair])
        .instruction();
      initializeIx.keys.pop();
      const initializeTxn = new web3.Transaction()
        .add(createRequiredIx)
        .add(initializeIx);
      try {
        await anchorProvider
          .sendAndConfirm(initializeTxn, [requiredKeypair])
          .catch((e) => {
            throw translateError(e, parseIdlErrors(program.idl));
          });
        assert.fail(
          "Unexpected success in creating a transaction that should have failed with `AccountNotEnoughKeys` error"
        );
      } catch (e) {
        // @ts-ignore
        assert.isTrue(e instanceof AnchorError, e.toString());
        const err = e as AnchorError;
        const errorCode = LangErrorCode.AccountNotEnoughKeys;
        assert.strictEqual(
          err.error.errorMessage,
          LangErrorMessage.get(errorCode)
        );
        assert.strictEqual(err.error.errorCode.number, errorCode);
      }
    });

    it("Succeeds with missing optional accounts at the end with the feature on", async () => {
      const allowMissingOptionals = anchor.workspace
        .AllowMissingOptionals as Program<AllowMissingOptionals>;
      const doStuffIx = await allowMissingOptionals.methods
        .doStuff()
        .accountsPartial({
          payer,
          systemProgram,
          optional2: null,
        })
        .instruction();
      doStuffIx.keys.pop();
      doStuffIx.keys.pop();
      const doStuffTxn = new web3.Transaction().add(doStuffIx);
      await anchorProvider.sendAndConfirm(doStuffTxn);
    });
  });

  describe("Initialize tests", async () => {
    it("Initialize with required null fails anchor-ts validation", async () => {
      const [requiredKeypair, createRequiredIx] = await createRequired();
      try {
        await program.methods
          .initialize(initializeValue1, initializeKey)
          .preInstructions([createRequiredIx])
          .accountsPartial({
            payer,
            systemProgram,
            // @ts-ignore
            required: null, //requiredKeypair.publicKey,
            optionalPda: null,
            optionalAccount: null,
          })
          .signers([requiredKeypair])
          .rpc();
        assert.fail(
          "Unexpected success in creating a transaction that should have failed at the client level"
        );
      } catch (e) {
        const errMsg = "Account `required` not provided";
        // @ts-ignore
        let error: string = e.toString();
        assert(error.includes(errMsg), `Unexpected error: ${e}`);
      }
    });

    it("Can initialize with no payer and no optionals", async () => {
      const [requiredKeypair, createRequiredIx] = await createRequired();
      await program.methods
        .initialize(initializeValue1, initializeKey)
        .preInstructions([createRequiredIx])
        .accountsPartial({
          payer: null,
          systemProgram,
          required: requiredKeypair.publicKey,
          optionalPda: null,
          optionalAccount: null,
        })
        .signers([requiredKeypair])
        .rpc();

      let required = await program.account.dataAccount.fetch(
        requiredKeypair.publicKey
      );
      expect(required.data.toNumber()).to.equal(0);
    });

    it("Can initialize with no optionals", async () => {
      const [requiredKeypair, createRequiredIx] = await createRequired();
      await program.methods
        .initialize(initializeValue1, initializeKey)
        .preInstructions([createRequiredIx])
        .accountsPartial({
          payer: null,
          systemProgram: null,
          required: requiredKeypair.publicKey,
          optionalPda: null,
          optionalAccount: null,
        })
        .signers([requiredKeypair])
        .rpc();

      let required = await program.account.dataAccount.fetch(
        requiredKeypair.publicKey
      );
      expect(required.data.toNumber()).to.equal(0);
    });

    it("Initialize with optionals and missing system program fails optional checks", async () => {
      const [requiredKeypair, createRequiredIx] = await createRequired();
      const dataAccount = new web3.Keypair();
      try {
        await program.methods
          .initialize(initializeValue1, initializeKey)
          .preInstructions([createRequiredIx])
          .accountsStrict({
            payer,
            systemProgram: null,
            required: requiredKeypair.publicKey,
            optionalPda: null,
            optionalAccount: dataAccount.publicKey,
          })
          .signers([requiredKeypair, dataAccount])
          .rpc();
        assert.fail(
          "Unexpected success in creating a transaction that should have failed with `ConstraintAccountIsNone` error"
        );
      } catch (e) {
        // @ts-ignore
        assert.isTrue(e instanceof AnchorError, e.toString());
        const err = e as AnchorError;
        const errorCode = LangErrorCode.ConstraintAccountIsNone;
        assert.strictEqual(
          err.error.errorMessage,
          LangErrorMessage.get(errorCode)
        );
        assert.strictEqual(err.error.errorCode.number, errorCode);
      }
    });

    it("Unwrapping None account in constraint panics", async () => {
      const [requiredKeypair, createRequiredIx] = await createRequired();
      const dataAccount = new web3.Keypair();
      const [dataPda] = findDataPda(dataAccount.publicKey);
      try {
        await program.methods
          .initialize(initializeValue1, initializeKey)
          .preInstructions([createRequiredIx])
          .accountsPartial({
            payer,
            systemProgram,
            required: requiredKeypair.publicKey,
            optionalPda: dataPda,
            optionalAccount: null,
          })
          .signers([requiredKeypair])
          .rpc();
        assert.fail(
          "Unexpected success in creating a transaction that should have failed with `ProgramFailedToComplete` error"
        );
      } catch (e) {
        const errMsg = "Program failed to complete";
        // @ts-ignore
        let error: string = e.toString();
        assert(error.includes(errMsg), `Unexpected error: ${e}`);
      }
    });

    it("Can initialize with required and optional account", async () => {
      await program.methods
        .initialize(initializeValue1, initializeKey)
        .preInstructions([createRequiredIx1])
        .accountsPartial({
          payer,
          systemProgram,
          required: requiredKeypair1.publicKey,
          optionalPda: null,
          optionalAccount: dataAccountKeypair1.publicKey,
        })
        .signers([requiredKeypair1, dataAccountKeypair1])
        .rpc();

      const requiredDataAccount = await program.account.dataAccount.fetch(
        requiredKeypair1.publicKey
      );
      expect(requiredDataAccount.data.toNumber()).to.equal(0);

      const optionalDataAccount = await program.account.dataAccount.fetch(
        dataAccountKeypair1.publicKey
      );
      expect(optionalDataAccount.data.toNumber()).to.equal(
        initializeValue1.muln(2).toNumber()
      );
    });

    it("Invalid seeds with all accounts provided fails", async () => {
      try {
        await program.methods
          .initialize(initializeValue2, initializeKey)
          .preInstructions([createRequiredIx2])
          .accountsPartial({
            payer,
            systemProgram,
            required: requiredKeypair2.publicKey,
            optionalPda: dataPda1[0],
            optionalAccount: dataAccountKeypair2.publicKey,
          })
          .signers([requiredKeypair2, dataAccountKeypair2])
          .rpc();
        assert.fail(
          "Unexpected success in creating a transaction that should have failed with `ConstraintSeeds` error"
        );
      } catch (e) {
        // @ts-ignore
        assert.isTrue(e instanceof AnchorError, e.toString());
        const err = e as AnchorError;
        const errorCode = LangErrorCode.ConstraintSeeds;
        assert.strictEqual(
          err.error.errorMessage,
          LangErrorMessage.get(errorCode)
        );
        assert.strictEqual(err.error.errorCode.number, errorCode);
      }
    });

    it("Can initialize with all accounts provided", async () => {
      await program.methods
        .initialize(initializeValue2, initializeKey)
        .preInstructions([createRequiredIx2])
        .accountsPartial({
          payer,
          systemProgram,
          required: requiredKeypair2.publicKey,
          optionalPda: dataPda2[0],
          optionalAccount: dataAccountKeypair2.publicKey,
        })
        .signers([requiredKeypair2, dataAccountKeypair2])
        .rpc();

      const requiredDataAccount = await program.account.dataAccount.fetch(
        requiredKeypair2.publicKey
      );
      expect(requiredDataAccount.data.toNumber()).to.equal(0);

      const optionalDataAccount = await program.account.dataAccount.fetch(
        dataAccountKeypair2.publicKey
      );
      expect(optionalDataAccount.data.toNumber()).to.equal(
        initializeValue2.toNumber()
      );

      const optionalDataPda = await program.account.dataPda.fetch(dataPda2[0]);
      expect(optionalDataPda.dataAccount.toString()).to.equal(
        initializeKey.toString()
      );
    });
  });

  describe("Update tests", async () => {
    it("Can update with invalid explicit pda bump with no pda", async () => {
      await program.methods
        .update(initializeValue2, initializeKey, dataPda2[1] - 1)
        .accountsPartial({
          payer,
          optionalPda: null,
          optionalAccount: null,
        })
        .rpc();
    });

    it("Errors with invalid explicit pda bump with pda included", async () => {
      try {
        await program.methods
          .update(initializeValue2, initializeKey, dataPda2[1] - 1)
          .accountsPartial({
            payer,
            optionalPda: dataPda2[0],
            optionalAccount: dataAccountKeypair2.publicKey,
          })
          .signers([dataAccountKeypair2])
          .rpc();
        assert.fail(
          "Unexpected success in creating a transaction that should have failed with `ConstraintSeeds` error"
        );
      } catch (e) {
        // @ts-ignore
        assert.isTrue(e instanceof AnchorError, e.toString());
        const err = e as AnchorError;
        const errorCode = LangErrorCode.ConstraintSeeds;
        assert.strictEqual(
          err.error.errorMessage,
          LangErrorMessage.get(errorCode)
        );
        assert.strictEqual(err.error.errorCode.number, errorCode);
      }
    });

    it("Fails with a missing signer", async () => {
      try {
        let txn = await program.methods
          .update(initializeValue2, initializeKey, dataPda2[1])
          .accountsPartial({
            payer,
            optionalPda: dataPda2[0],
            optionalAccount: dataAccountKeypair2.publicKey,
          })
          .transaction();
        txn.instructions[0].keys.forEach((meta) => {
          if (meta.pubkey.equals(dataAccountKeypair2.publicKey)) {
            meta.isSigner = false;
          }
        });
        await anchorProvider.sendAndConfirm(txn);
        assert.fail(
          "Unexpected success in creating a transaction that should have failed with `ConstraintSigner` error"
        );
      } catch (e) {
        // @ts-ignore
        assert.isTrue(e instanceof web3.SendTransactionError, e.toString());
        const err = e as web3.SendTransactionError;
        const anchorError = AnchorError.parse(err.logs!)!;
        const errorCode = LangErrorCode.ConstraintSigner;
        assert.strictEqual(
          anchorError.error.errorMessage,
          LangErrorMessage.get(errorCode)
        );
        assert.strictEqual(anchorError.error.errorCode.number, errorCode);
      }
    });

    it("Can trigger raw constraint violations with references to optional accounts", async () => {
      try {
        await program.methods
          .update(initializeValue2, initializeKey, dataPda2[1])
          .accountsPartial({
            payer: null,
            optionalPda: dataPda2[0],
            optionalAccount: dataAccountKeypair2.publicKey,
          })
          .signers([dataAccountKeypair2])
          .rpc();
        assert.fail(
          "Unexpected success in creating a transaction that should have failed with `ConstraintRaw` error"
        );
      } catch (e) {
        // @ts-ignore
        assert.isTrue(e instanceof AnchorError, e.toString());
        const err = e as AnchorError;
        const errorCode = LangErrorCode.ConstraintRaw;
        assert.strictEqual(
          err.error.errorMessage,
          LangErrorMessage.get(errorCode)
        );
        assert.strictEqual(err.error.errorCode.number, errorCode);
      }
    });

    it("Can update an optional account", async () => {
      await program.methods
        .update(initializeValue2.muln(3), initializeKey, dataPda2[1])
        .accountsPartial({
          payer,
          optionalPda: null,
          optionalAccount: dataAccountKeypair2.publicKey,
        })
        .signers([dataAccountKeypair2])
        .rpc();

      const dataAccount = await program.account.dataAccount.fetch(
        dataAccountKeypair2.publicKey
      );
      expect(dataAccount.data.toNumber()).to.equal(
        initializeValue2.muln(3).toNumber()
      );
    });

    it("Can update both accounts", async () => {
      const newKey = web3.PublicKey.unique();
      await program.methods
        .update(initializeValue2, newKey, dataPda2[1])
        .accountsPartial({
          payer,
          optionalPda: dataPda2[0],
          optionalAccount: dataAccountKeypair2.publicKey,
        })
        .signers([dataAccountKeypair2])
        .rpc();

      const dataPda = await program.account.dataPda.fetch(dataPda2[0]);
      expect(dataPda.dataAccount.toString()).to.equal(newKey.toString());

      const dataAccount = await program.account.dataAccount.fetch(
        dataAccountKeypair2.publicKey
      );
      expect(dataAccount.data.toNumber()).to.equal(initializeValue2.toNumber());
    });
  });

  describe("Realloc tests", async () => {
    it("Realloc with no payer fails", async () => {
      try {
        await program.methods
          .realloc(new BN(100))
          .accountsPartial({
            payer: null,
            required: dataAccountKeypair1.publicKey,
            optionalPda: null,
            optionalAccount: dataAccountKeypair2.publicKey,
            systemProgram,
          })
          .signers([dataAccountKeypair2])
          .rpc();
        assert.fail(
          "Unexpected success in creating a transaction that should have failed with `ConstraintAccountIsNone` error"
        );
      } catch (e) {
        // @ts-ignore
        assert.isTrue(e instanceof AnchorError, e.toString());
        const err = e as AnchorError;
        const errorCode = LangErrorCode.ConstraintAccountIsNone;
        assert.strictEqual(
          err.error.errorMessage,
          LangErrorMessage.get(errorCode)
        );
        assert.strictEqual(err.error.errorCode.number, errorCode);
      }
    });

    it("Realloc with no system program fails", async () => {
      try {
        await program.methods
          .realloc(new BN(100))
          .accountsStrict({
            payer,
            required: dataAccountKeypair1.publicKey,
            optionalPda: null,
            optionalAccount: dataAccountKeypair2.publicKey,
            systemProgram: null,
          })
          .signers([dataAccountKeypair2])
          .rpc();
        assert.fail(
          "Unexpected success in creating a transaction that should have failed with `ConstraintAccountIsNone` error"
        );
      } catch (e) {
        // @ts-ignore
        assert.isTrue(e instanceof AnchorError, e.toString());
        const err = e as AnchorError;
        const errorCode = LangErrorCode.ConstraintAccountIsNone;
        assert.strictEqual(
          err.error.errorMessage,
          LangErrorMessage.get(errorCode)
        );
        assert.strictEqual(err.error.errorCode.number, errorCode);
      }
    });

    it("Wrong type of account is caught for optional accounts", async () => {
      try {
        await program.methods
          .realloc(new BN(100))
          .accountsPartial({
            payer,
            required: dataAccountKeypair1.publicKey,
            optionalPda: dataAccountKeypair2.publicKey,
            optionalAccount: null,
            systemProgram,
          })
          .rpc();
        assert.fail(
          "Unexpected success in creating a transaction that should have failed with `AccountDiscriminatorMismatch` error"
        );
      } catch (e) {
        // @ts-ignore
        assert.isTrue(e instanceof AnchorError, e.toString());
        const err = e as AnchorError;
        const errorCode = LangErrorCode.AccountDiscriminatorMismatch;
        assert.strictEqual(
          err.error.errorMessage,
          LangErrorMessage.get(errorCode)
        );
        assert.strictEqual(err.error.errorCode.number, errorCode);
      }
    });

    it("Can realloc with optional accounts", async () => {
      const newLength = 100;
      await program.methods
        .realloc(new BN(newLength))
        .accountsPartial({
          payer,
          required: dataAccountKeypair1.publicKey,
          optionalPda: null,
          optionalAccount: dataAccountKeypair2.publicKey,
          systemProgram,
        })
        .signers([dataAccountKeypair2])
        .rpc();
      const dataAccount = await program.provider.connection.getAccountInfo(
        dataAccountKeypair2.publicKey
      );
      assert.exists(dataAccount);
      expect(dataAccount!.data.length).to.equal(newLength);
    });

    it("Can realloc back to original size with optional accounts", async () => {
      const newLength = program.account.dataAccount.size;
      await program.methods
        .realloc(new BN(newLength))
        .accountsPartial({
          payer,
          required: dataAccountKeypair1.publicKey,
          optionalPda: null,
          optionalAccount: dataAccountKeypair2.publicKey,
          systemProgram,
        })
        .signers([dataAccountKeypair2])
        .rpc();
      const dataAccount = await program.provider.connection.getAccountInfo(
        dataAccountKeypair2.publicKey
      );
      assert.exists(dataAccount);
      expect(dataAccount!.data.length).to.equal(newLength);
    });

    it("Can realloc multiple optional accounts", async () => {
      const newLength = 100;
      await program.methods
        .realloc(new BN(newLength))
        .accountsPartial({
          payer,
          required: dataAccountKeypair1.publicKey,
          optionalPda: dataPda2[0],
          optionalAccount: dataAccountKeypair2.publicKey,
          systemProgram,
        })
        .signers([dataAccountKeypair2])
        .rpc();
      const dataAccount = await program.provider.connection.getAccountInfo(
        dataAccountKeypair2.publicKey
      );
      assert.exists(dataAccount);
      expect(dataAccount!.data.length).to.equal(newLength);

      const dataPda = await program.provider.connection.getAccountInfo(
        dataPda2[0]
      );
      assert.exists(dataPda);
      expect(dataPda!.data.length).to.equal(newLength);
    });
  });

  describe("Close tests", async () => {
    const requiredKeypair3 = web3.Keypair.generate();
    const requiredKeypair4 = web3.Keypair.generate();

    let createRequiredIx3: web3.TransactionInstruction;
    let createRequiredIx4: web3.TransactionInstruction;

    const dataAccountKeypair3 = web3.Keypair.generate();
    const dataAccountKeypair4 = web3.Keypair.generate();

    const dataPda3 = findDataPda(dataAccountKeypair3.publicKey);
    const dataPda4 = findDataPda(dataAccountKeypair4.publicKey);

    const initializeValue3 = new BN(50);
    const initializeValue4 = new BN(1000);

    before("Setup additional accounts", async () => {
      createRequiredIx3 = (await createRequired(requiredKeypair3))[1];
      createRequiredIx4 = (await createRequired(requiredKeypair4))[1];
      const assertInitSuccess = async (
        requiredPubkey: web3.PublicKey,
        dataPdaPubkey: web3.PublicKey,
        dataAccountPubkey: web3.PublicKey,
        initializeValue: BN
      ) => {
        const requiredDataAccount = await program.account.dataAccount.fetch(
          requiredPubkey
        );
        expect(requiredDataAccount.data.toNumber()).to.equal(0);

        const optionalDataAccount = await program.account.dataAccount.fetch(
          dataAccountPubkey
        );
        expect(optionalDataAccount.data.toNumber()).to.equal(
          initializeValue.toNumber()
        );

        const optionalDataPda = await program.account.dataPda.fetch(
          dataPdaPubkey
        );
        expect(optionalDataPda.dataAccount.toString()).to.equal(
          initializeKey.toString()
        );
      };

      await program.methods
        .initialize(initializeValue3, initializeKey)
        .preInstructions([createRequiredIx3])
        .accountsPartial({
          payer,
          systemProgram,
          required: requiredKeypair3.publicKey,
          optionalPda: dataPda3[0],
          optionalAccount: dataAccountKeypair3.publicKey,
        })
        .signers([requiredKeypair3, dataAccountKeypair3])
        .rpc();
      await assertInitSuccess(
        requiredKeypair3.publicKey,
        dataPda3[0],
        dataAccountKeypair3.publicKey,
        initializeValue3
      );
      await program.methods
        .initialize(initializeValue4, initializeKey)
        .preInstructions([createRequiredIx4])
        .accountsPartial({
          payer,
          systemProgram,
          required: requiredKeypair4.publicKey,
          optionalPda: dataPda4[0],
          optionalAccount: dataAccountKeypair4.publicKey,
        })
        .signers([requiredKeypair4, dataAccountKeypair4])
        .rpc();
      await assertInitSuccess(
        requiredKeypair4.publicKey,
        dataPda4[0],
        dataAccountKeypair4.publicKey,
        initializeValue4
      );

      await program.methods
        .update(initializeValue3, dataAccountKeypair3.publicKey, dataPda3[1])
        .accountsPartial({
          payer,
          optionalPda: dataPda3[0],
          optionalAccount: dataAccountKeypair3.publicKey,
        })
        .signers([dataAccountKeypair3])
        .rpc();
      const optionalPda3 = await program.account.dataPda.fetch(dataPda3[0]);
      expect(optionalPda3.dataAccount.toString()).to.equal(
        dataAccountKeypair3.publicKey.toString()
      );
      await program.methods
        .update(initializeValue4, dataAccountKeypair4.publicKey, dataPda4[1])
        .accountsPartial({
          payer,
          optionalPda: dataPda4[0],
          optionalAccount: dataAccountKeypair4.publicKey,
        })
        .signers([dataAccountKeypair4])
        .rpc();
      const optionalPda4 = await program.account.dataPda.fetch(dataPda4[0]);
      expect(optionalPda4.dataAccount.toString()).to.equal(
        dataAccountKeypair4.publicKey.toString()
      );
    });

    it("Close with no close target fails", async () => {
      try {
        await program.methods
          .close()
          .accountsPartial({
            payer: null,
            optionalPda: null,
            dataAccount: dataAccountKeypair3.publicKey,
            systemProgram,
          })
          .signers([dataAccountKeypair3])
          .rpc();
        assert.fail(
          "Unexpected success in creating a transaction that should have failed with `ConstraintAccountIsNone` error"
        );
      } catch (e) {
        // @ts-ignore
        assert.isTrue(e instanceof AnchorError, e.toString());
        const err = e as AnchorError;
        const errorCode = LangErrorCode.ConstraintAccountIsNone;
        assert.strictEqual(
          err.error.errorMessage,
          LangErrorMessage.get(errorCode)
        );
        assert.strictEqual(err.error.errorCode.number, errorCode);
      }
    });

    it("Has one constraints are caught with optional accounts", async () => {
      try {
        await program.methods
          .close()
          .accountsPartial({
            payer,
            optionalPda: dataPda4[0],
            dataAccount: dataAccountKeypair3.publicKey,
            systemProgram,
          })
          .signers([dataAccountKeypair3])
          .rpc();
        assert.fail(
          "Unexpected success in creating a transaction that should have failed with `ConstraintHasOne` error"
        );
      } catch (e) {
        // @ts-ignore
        assert.isTrue(e instanceof AnchorError, e.toString());
        const err = e as AnchorError;
        const errorCode = LangErrorCode.ConstraintHasOne;
        assert.strictEqual(
          err.error.errorMessage,
          LangErrorMessage.get(errorCode)
        );
        assert.strictEqual(err.error.errorCode.number, errorCode);
      }
    });

    it("Can close an optional account", async () => {
      await program.methods
        .close()
        .accountsPartial({
          payer,
          optionalPda: null,
          dataAccount: dataAccountKeypair3.publicKey,
          systemProgram,
        })
        .signers([dataAccountKeypair3])
        .rpc();
      const dataAccount = await program.provider.connection.getAccountInfo(
        dataAccountKeypair3.publicKey
      );
      assert.isNull(dataAccount);
    });

    it("Can close multiple optional accounts", async () => {
      await program.methods
        .close()
        .accountsPartial({
          payer,
          optionalPda: dataPda4[0],
          dataAccount: dataAccountKeypair4.publicKey,
          systemProgram,
        })
        .signers([dataAccountKeypair4])
        .rpc();
      const dataAccount = await program.provider.connection.getAccountInfo(
        dataAccountKeypair4.publicKey
      );
      assert.isNull(dataAccount);
    });
  });
});
