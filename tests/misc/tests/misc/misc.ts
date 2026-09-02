import * as anchor from "@anchor-lang/core";
import { Program, AnchorError, Wallet } from "@anchor-lang/core";
import {
  PublicKey,
  Keypair,
  SystemProgram,
  Message,
  VersionedTransaction,
  AddressLookupTableProgram,
  TransactionMessage,
} from "@solana/web3.js";
import {
  TOKEN_PROGRAM_ID,
  Token,
  ASSOCIATED_TOKEN_PROGRAM_ID,
  AccountLayout,
  MintLayout,
} from "@solana/spl-token";
import { assert, expect } from "chai";

import { Misc } from "../../target/types/misc";
import { MiscOptional } from "../../target/types/misc_optional";

const utf8 = anchor.utils.bytes.utf8;
const nativeAssert = require("assert");
const miscIdl = require("../../target/idl/misc.json");

const TOKEN_2022_PROGRAM_ID = new anchor.web3.PublicKey(
  "TokenzQdBNbLqP5VEhdkAS6EPFLC1PHnBqCXEpPxuEb"
);

const miscTest = (
  program: anchor.Program<Misc> | anchor.Program<MiscOptional>
) => {
  return () => {
    // Configure the client to use the local cluster.
    const provider = anchor.AnchorProvider.env();
    const wallet = provider.wallet as Wallet;
    anchor.setProvider(provider);

    describe("Data Account", () => {
      const data = anchor.web3.Keypair.generate();
      const udata = new anchor.BN(1);
      const idata = new anchor.BN(2);

      it("Can initialize data account", async () => {
        await program.methods
          .initialize(udata, idata)
          .accounts({ data: data.publicKey })
          .signers([data])
          .preInstructions([await program.account.data.createInstruction(data)])
          .rpc();
        const dataAccount = await program.account.data.fetch(data.publicKey);
        assert(dataAccount.udata.eq(udata));
        assert(dataAccount.idata.eq(idata));
      });

      it("Can use base58 strings to fetch an account", async () => {
        const dataAccount = await program.account.data.fetch(
          data.publicKey.toString()
        );
        assert(dataAccount.udata.eq(udata));
        assert(dataAccount.idata.eq(idata));
      });

      it("Can use the owner constraint", async () => {
        await program.methods
          .testOwner()
          .accounts({
            data: data.publicKey,
            misc: program.programId,
          })
          .rpc();

        await nativeAssert.rejects(
          async () => {
            await program.methods
              .testOwner()
              .accounts({
                data: provider.wallet.publicKey,
                misc: program.programId,
              })
              .rpc();
          },
          (err) => {
            return true;
          }
        );
      });

      it("Should fail to close an account when sending lamports to itself", async () => {
        try {
          await program.methods
            .testClose()
            .accounts({
              data: data.publicKey,
              solDest: data.publicKey,
            })
            .rpc();
          expect(false).to.be.true;
        } catch (err) {
          assert.strictEqual(
            err.error.errorMessage,
            "A close constraint was violated"
          );
          assert.strictEqual(err.error.errorCode.number, 2011);
        }
      });

      it("Can close an account", async () => {
        const connection = program.provider.connection;
        const openAccount = await connection.getAccountInfo(data.publicKey);

        assert.isNotNull(openAccount);
        const openAccountBalance = openAccount.lamports;
        // double balance to calculate closed balance correctly
        const transferIx = anchor.web3.SystemProgram.transfer({
          fromPubkey: provider.wallet.publicKey,
          toPubkey: data.publicKey,
          lamports: openAccountBalance,
        });
        const transferTransaction = new anchor.web3.Transaction().add(
          transferIx
        );
        await provider.sendAndConfirm(transferTransaction);

        let beforeBalance = (
          await connection.getAccountInfo(provider.wallet.publicKey)
        ).lamports;

        await program.methods
          .testClose()
          .accounts({
            data: data.publicKey,
            solDest: provider.wallet.publicKey,
          })
          .postInstructions([transferIx])
          .rpc();

        let afterBalance = (
          await connection.getAccountInfo(provider.wallet.publicKey)
        ).lamports;

        // Retrieved rent exemption sol.
        expect(afterBalance > beforeBalance).to.be.true;

        const closedAccount = await connection.getAccountInfo(data.publicKey);

        assert.isTrue(closedAccount.data.length === 0);
        assert.isTrue(closedAccount.owner.equals(SystemProgram.programId));
      });

      it("Can close an account twice", async () => {
        const data = anchor.web3.Keypair.generate();
        await program.methods
          .initialize(new anchor.BN(10), new anchor.BN(10))
          .accounts({ data: data.publicKey })
          .preInstructions([await program.account.data.createInstruction(data)])
          .signers([data])
          .rpc();

        const connection = program.provider.connection;
        const openAccount = await connection.getAccountInfo(data.publicKey);
        assert.isNotNull(openAccount);

        const openAccountBalance = openAccount.lamports;
        // double balance to calculate closed balance correctly
        const transferIx = anchor.web3.SystemProgram.transfer({
          fromPubkey: provider.wallet.publicKey,
          toPubkey: data.publicKey,
          lamports: openAccountBalance,
        });
        const transferTransaction = new anchor.web3.Transaction().add(
          transferIx
        );
        await provider.sendAndConfirm(transferTransaction);

        let beforeBalance = (
          await connection.getAccountInfo(provider.wallet.publicKey)
        ).lamports;

        await program.methods
          .testCloseTwice()
          .accounts({
            data: data.publicKey,
            solDest: provider.wallet.publicKey,
          })
          .postInstructions([transferIx])
          .rpc();

        let afterBalance = (
          await connection.getAccountInfo(provider.wallet.publicKey)
        ).lamports;

        // Retrieved rent exemption sol.
        expect(afterBalance > beforeBalance).to.be.true;

        const closedAccount = await connection.getAccountInfo(data.publicKey);
        assert.isTrue(closedAccount.data.length === 0);
        assert.isTrue(closedAccount.owner.equals(SystemProgram.programId));
      });

      it("Can close a mut account manually", async () => {
        const data = anchor.web3.Keypair.generate();
        await program.methods
          .initialize(new anchor.BN(10), new anchor.BN(10))
          .accounts({ data: data.publicKey })
          .preInstructions([await program.account.data.createInstruction(data)])
          .signers([data])
          .rpc();

        const connection = program.provider.connection;
        const openAccount = await connection.getAccountInfo(data.publicKey);

        assert.isNotNull(openAccount);
        const openAccountBalance = openAccount.lamports;
        // double balance to calculate closed balance correctly
        const transferIx = anchor.web3.SystemProgram.transfer({
          fromPubkey: provider.wallet.publicKey,
          toPubkey: data.publicKey,
          lamports: openAccountBalance,
        });
        const transferTransaction = new anchor.web3.Transaction().add(
          transferIx
        );
        await provider.sendAndConfirm(transferTransaction);

        let beforeBalance = (
          await connection.getAccountInfo(provider.wallet.publicKey)
        ).lamports;

        await program.methods
          .testCloseMut()
          .accounts({
            data: data.publicKey,
            solDest: provider.wallet.publicKey,
          })
          .postInstructions([transferIx])
          .rpc();

        let afterBalance = (
          await connection.getAccountInfo(provider.wallet.publicKey)
        ).lamports;

        // Retrieved rent exemption sol.
        expect(afterBalance > beforeBalance).to.be.true;

        const closedAccount = await connection.getAccountInfo(data.publicKey);
        assert.isTrue(closedAccount.data.length === 0);
        assert.isTrue(closedAccount.owner.equals(SystemProgram.programId));
      });
    });

    it("Can send VersionedTransaction", async () => {
      // Create the lookup table
      const recentSlot = await provider.connection.getSlot();
      const [loookupTableInstruction, lookupTableAddress] =
        AddressLookupTableProgram.createLookupTable({
          authority: provider.publicKey,
          payer: provider.publicKey,
          recentSlot,
        });
      const extendInstruction = AddressLookupTableProgram.extendLookupTable({
        payer: provider.publicKey,
        authority: provider.publicKey,
        lookupTable: lookupTableAddress,
        addresses: [provider.publicKey, SystemProgram.programId],
      });
      let createLookupTableTx = new VersionedTransaction(
        new TransactionMessage({
          instructions: [loookupTableInstruction, extendInstruction],
          payerKey: program.provider.publicKey,
          recentBlockhash: (await provider.connection.getLatestBlockhash())
            .blockhash,
        }).compileToV0Message()
      );
      type SendParams = Parameters<typeof provider.sendAndConfirm>;
      const testThis: SendParams = [
        new VersionedTransaction(
          new TransactionMessage({
            instructions: [loookupTableInstruction, extendInstruction],
            payerKey: program.provider.publicKey,
            recentBlockhash: (await provider.connection.getLatestBlockhash())
              .blockhash,
          }).compileToV0Message()
        ),
      ];
      await provider.sendAndConfirm(createLookupTableTx, [], {
        skipPreflight: true,
      });

      // Use the lookup table in a transaction
      const transferAmount = 1_000_000;
      const lookupTableAccount = await provider.connection
        .getAddressLookupTable(lookupTableAddress)
        .then((res) => res.value);
      const target = Keypair.generate();
      let transferInstruction = SystemProgram.transfer({
        fromPubkey: provider.publicKey,
        lamports: transferAmount,
        toPubkey: target.publicKey,
      });
      let transferUsingLookupTx = new VersionedTransaction(
        new TransactionMessage({
          instructions: [transferInstruction],
          payerKey: program.provider.publicKey,
          recentBlockhash: (await provider.connection.getLatestBlockhash())
            .blockhash,
        }).compileToV0Message([lookupTableAccount])
      );
      await provider.simulate(transferUsingLookupTx, [], "processed");
      await provider.sendAndConfirm(transferUsingLookupTx, [], {
        skipPreflight: true,
        commitment: "confirmed",
      });
      let newBalance = await provider.connection.getBalance(
        target.publicKey,
        "confirmed"
      );
      assert.strictEqual(newBalance, transferAmount);

      // Test sendAll with versioned transaction
      let oneTransferUsingLookupTx = new VersionedTransaction(
        new TransactionMessage({
          instructions: [
            SystemProgram.transfer({
              fromPubkey: provider.publicKey,
              // Needed to make the transactions distinct
              lamports: transferAmount + 1,
              toPubkey: target.publicKey,
            }),
          ],
          payerKey: program.provider.publicKey,
          recentBlockhash: (await provider.connection.getLatestBlockhash())
            .blockhash,
        }).compileToV0Message([lookupTableAccount])
      );
      let twoTransferUsingLookupTx = new VersionedTransaction(
        new TransactionMessage({
          instructions: [
            SystemProgram.transfer({
              fromPubkey: provider.publicKey,
              lamports: transferAmount,
              toPubkey: target.publicKey,
            }),
          ],
          payerKey: program.provider.publicKey,
          recentBlockhash: (await provider.connection.getLatestBlockhash())
            .blockhash,
        }).compileToV0Message([lookupTableAccount])
      );
      await provider.sendAll(
        [{ tx: oneTransferUsingLookupTx }, { tx: twoTransferUsingLookupTx }],
        { skipPreflight: true, commitment: "confirmed" }
      );
      newBalance = await provider.connection.getBalance(
        target.publicKey,
        "confirmed"
      );
      assert.strictEqual(newBalance, transferAmount * 3 + 1);
    });

    it("Can send VersionedTransaction with extra signatures", async () => {
      // Test sending with signatures
      const initSpace = 100;
      const rentExemptAmount =
        await provider.connection.getMinimumBalanceForRentExemption(initSpace);

      const newAccount = Keypair.generate();
      let createAccountIx = SystemProgram.createAccount({
        fromPubkey: provider.publicKey,
        lamports: rentExemptAmount,
        newAccountPubkey: newAccount.publicKey,
        programId: program.programId,
        space: initSpace,
      });
      let createAccountTx = new VersionedTransaction(
        new TransactionMessage({
          instructions: [createAccountIx],
          payerKey: provider.publicKey,
          recentBlockhash: (await provider.connection.getLatestBlockhash())
            .blockhash,
        }).compileToV0Message()
      );
      await provider.simulate(createAccountTx, [], "processed");
      await provider.sendAndConfirm(createAccountTx, [newAccount], {
        skipPreflight: false,
        commitment: "confirmed",
      });
      let newAccountInfo = await provider.connection.getAccountInfo(
        newAccount.publicKey
      );
      assert.strictEqual(
        newAccountInfo.owner.toBase58(),
        program.programId.toBase58()
      );
    });

    it("Can embed programs into genesis from the Anchor.toml", async () => {
      const pid = new anchor.web3.PublicKey(
        "FtMNMKp9DZHKWUyVAsj3Q5QV8ow4P3fUPP7ZrWEQJzKr"
      );
      let accInfo = await anchor.getProvider().connection.getAccountInfo(pid);
      assert.isTrue(accInfo.executable);
    });

    it("Can use the executable attribute", async () => {
      await program.rpc.testExecutable({
        accounts: {
          program: program.programId,
        },
      });

      await nativeAssert.rejects(
        async () => {
          await program.rpc.testExecutable({
            accounts: {
              program: provider.wallet.publicKey,
            },
          });
        },
        (err) => {
          return true;
        }
      );
    });

    it("Can retrieve events when simulating a transaction", async () => {
      const resp = await program.methods.testSimulate(44).simulate();
      const expectedRaw = [
        `Program ${program.programId.toString()} invoke [1]`,
        "Program log: Instruction: TestSimulate",
        "Program data: NgyCA9omwbMsAAAA",
        "Program data: fPhuIELK/k7SBAAA",
        "Program data: jvbowsvlmkcJAAAA",
        "Program data: zxM5neEnS1kBAgMEBQYHCAkK",
        "Program data: g06Ei2GL1gIBAgMEBQYHCAkKCw==",
      ];

      assert.deepStrictEqual(expectedRaw, resp.raw.slice(0, -2));
      assert.strictEqual(resp.events[0].name, "e1");
      assert.strictEqual(resp.events[0].data.data, 44);
      assert.strictEqual(resp.events[1].name, "e2");
      assert.strictEqual(resp.events[1].data.data, 1234);
      assert.strictEqual(resp.events[2].name, "e3");
      assert.strictEqual(resp.events[2].data.data, 9);
      assert.strictEqual(resp.events[3].name, "e5");
      assert.deepStrictEqual(
        resp.events[3].data.data,
        [1, 2, 3, 4, 5, 6, 7, 8, 9, 10]
      );
      assert.strictEqual(resp.events[4].name, "e6");
      assert.deepStrictEqual(
        resp.events[4].data.data,
        [1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11]
      );
    });

    it("Can use instruction data in accounts constraints", async () => {
      // b"my-seed"
      const seed = Buffer.from([109, 121, 45, 115, 101, 101, 100]);
      const [myPda, nonce] = await PublicKey.findProgramAddress(
        [seed, anchor.web3.SYSVAR_RENT_PUBKEY.toBuffer()],
        program.programId
      );

      await program.rpc.testInstructionConstraint(nonce, {
        accounts: {
          myPda,
          myAccount: anchor.web3.SYSVAR_RENT_PUBKEY,
        },
      });
    });

    it("Can create a PDA account with instruction data", async () => {
      const seed = Buffer.from([1, 2, 3, 4]);
      const domain = "my-domain";
      const foo = anchor.web3.SYSVAR_RENT_PUBKEY;
      const [myPda, nonce] = await PublicKey.findProgramAddress(
        [
          Buffer.from(anchor.utils.bytes.utf8.encode("my-seed")),
          Buffer.from(anchor.utils.bytes.utf8.encode(domain)),
          foo.toBuffer(),
          seed,
        ],
        program.programId
      );

      await program.rpc.testPdaInit(domain, seed, nonce, {
        accounts: {
          myPda,
          myPayer: provider.wallet.publicKey,
          foo,
          systemProgram: anchor.web3.SystemProgram.programId,
        },
      });

      const myPdaAccount = await program.account.dataU16.fetch(myPda);
      assert.strictEqual(myPdaAccount.data, 6);
    });

    it("Can create a zero copy PDA account", async () => {
      const [myPda, nonce] = await PublicKey.findProgramAddress(
        [Buffer.from(anchor.utils.bytes.utf8.encode("my-seed"))],
        program.programId
      );
      await program.rpc.testPdaInitZeroCopy({
        accounts: {
          myPda,
          myPayer: provider.wallet.publicKey,
          systemProgram: anchor.web3.SystemProgram.programId,
        },
      });

      const myPdaAccount = await program.account.dataZeroCopy.fetch(myPda);
      assert.strictEqual(myPdaAccount.data, 9);
      assert.strictEqual(myPdaAccount.bump, nonce);
    });

    it("Can write to a zero copy PDA account", async () => {
      const [myPda, bump] = await PublicKey.findProgramAddress(
        [Buffer.from(anchor.utils.bytes.utf8.encode("my-seed"))],
        program.programId
      );
      await program.rpc.testPdaMutZeroCopy({
        accounts: {
          myPda,
          myPayer: provider.wallet.publicKey,
        },
      });

      const myPdaAccount = await program.account.dataZeroCopy.fetch(myPda);
      assert.strictEqual(myPdaAccount.data, 1234);
      assert.strictEqual(myPdaAccount.bump, bump);
    });

    it("Can create a token account from seeds pda", async () => {
      const [mint, mint_bump] = await PublicKey.findProgramAddress(
        [Buffer.from(anchor.utils.bytes.utf8.encode("my-mint-seed"))],
        program.programId
      );
      const [myPda, token_bump] = await PublicKey.findProgramAddress(
        [Buffer.from(anchor.utils.bytes.utf8.encode("my-token-seed"))],
        program.programId
      );
      await program.rpc.testTokenSeedsInit({
        accounts: {
          myPda,
          mint,
          authority: provider.wallet.publicKey,
          systemProgram: anchor.web3.SystemProgram.programId,
          tokenProgram: TOKEN_PROGRAM_ID,
        },
      });

      const mintAccount = new Token(
        program.provider.connection,
        mint,
        TOKEN_PROGRAM_ID,
        wallet.payer
      );
      const account = await mintAccount.getAccountInfo(myPda);
      // @ts-expect-error
      assert.strictEqual(account.state, 1);
      assert.strictEqual(account.amount.toNumber(), 0);
      assert.isTrue(account.isInitialized);
      assert.isTrue(account.owner.equals(provider.wallet.publicKey));
      assert.isTrue(account.mint.equals(mint));
    });

    it("Can execute a fallback function", async () => {
      await nativeAssert.rejects(
        async () => {
          await anchor.utils.rpc.invoke(program.programId);
        },
        (err) => {
          assert.isTrue(err.toString().includes("custom program error: 0x4d2"));
          return true;
        }
      );
    });

    it("Can init a random account", async () => {
      const data = anchor.web3.Keypair.generate();
      await program.rpc.testInit({
        accounts: {
          data: data.publicKey,
          payer: provider.wallet.publicKey,
          systemProgram: anchor.web3.SystemProgram.programId,
        },
        signers: [data],
      });

      const account = await program.account.dataI8.fetch(data.publicKey);
      assert.strictEqual(account.data, 3);
    });

    it("Can init a random account prefunded", async () => {
      const data = anchor.web3.Keypair.generate();
      await program.rpc.testInit({
        accounts: {
          data: data.publicKey,
          payer: provider.wallet.publicKey,
          systemProgram: anchor.web3.SystemProgram.programId,
        },
        signers: [data],
        instructions: [
          anchor.web3.SystemProgram.transfer({
            fromPubkey: provider.wallet.publicKey,
            toPubkey: data.publicKey,
            lamports: 4039280,
          }),
        ],
      });

      const account = await program.account.dataI8.fetch(data.publicKey);
      assert.strictEqual(account.data, 3);
    });

    it("Should fail when trying to init the payer as a program account", async () => {
      try {
        await program.rpc.testInit({
          accounts: {
            data: provider.wallet.publicKey,
            payer: provider.wallet.publicKey,
            systemProgram: anchor.web3.SystemProgram.programId,
          },
        });
        assert.fail("Transaction should fail");
      } catch (e) {
        // "Error Code: TryingToInitPayerAsProgramAccount. Error Number: 4101. Error Message: You cannot/should not initialize the payer account as a program account."
        assert.strictEqual(e.error.errorCode.number, 4101);
      }
    });

    it("Can init a random zero copy account", async () => {
      const data = anchor.web3.Keypair.generate();
      await program.rpc.testInitZeroCopy({
        accounts: {
          data: data.publicKey,
          payer: provider.wallet.publicKey,
          systemProgram: anchor.web3.SystemProgram.programId,
        },
        signers: [data],
      });
      const account = await program.account.dataZeroCopy.fetch(data.publicKey);
      assert.strictEqual(account.data, 10);
      assert.strictEqual(account.bump, 2);
    });

    let mint = undefined;

    it("Can create a random mint account", async () => {
      mint = anchor.web3.Keypair.generate();
      await program.rpc.testInitMint({
        accounts: {
          mint: mint.publicKey,
          payer: provider.wallet.publicKey,
          systemProgram: anchor.web3.SystemProgram.programId,
          tokenProgram: TOKEN_PROGRAM_ID,
        },
        signers: [mint],
      });
      const client = new Token(
        program.provider.connection,
        mint.publicKey,
        TOKEN_PROGRAM_ID,
        wallet.payer
      );
      const mintAccount = await client.getMintInfo();
      assert.strictEqual(mintAccount.decimals, 6);
      assert.isTrue(
        mintAccount.mintAuthority.equals(provider.wallet.publicKey)
      );
      assert.isTrue(
        mintAccount.freezeAuthority.equals(provider.wallet.publicKey)
      );
    });

    it("Can create a random mint account prefunded", async () => {
      mint = anchor.web3.Keypair.generate();
      await program.rpc.testInitMint({
        accounts: {
          mint: mint.publicKey,
          payer: provider.wallet.publicKey,
          systemProgram: anchor.web3.SystemProgram.programId,
          tokenProgram: TOKEN_PROGRAM_ID,
        },
        signers: [mint],
        instructions: [
          anchor.web3.SystemProgram.transfer({
            fromPubkey: provider.wallet.publicKey,
            toPubkey: mint.publicKey,
            lamports: 4039280,
          }),
        ],
      });
      const client = new Token(
        program.provider.connection,
        mint.publicKey,
        TOKEN_PROGRAM_ID,
        wallet.payer
      );
      const mintAccount = await client.getMintInfo();
      assert.strictEqual(mintAccount.decimals, 6);
      assert.isTrue(
        mintAccount.mintAuthority.equals(provider.wallet.publicKey)
      );
    });

    it("Can create a random token account", async () => {
      const token = anchor.web3.Keypair.generate();
      await program.rpc.testInitToken({
        accounts: {
          token: token.publicKey,
          mint: mint.publicKey,
          payer: provider.wallet.publicKey,
          systemProgram: anchor.web3.SystemProgram.programId,
          tokenProgram: TOKEN_PROGRAM_ID,
        },
        signers: [token],
      });
      const client = new Token(
        program.provider.connection,
        mint.publicKey,
        TOKEN_PROGRAM_ID,
        wallet.payer
      );
      const account = await client.getAccountInfo(token.publicKey);
      // @ts-expect-error
      assert.strictEqual(account.state, 1);
      assert.strictEqual(account.amount.toNumber(), 0);
      assert.isTrue(account.isInitialized);
      assert.isTrue(account.owner.equals(provider.wallet.publicKey));
      assert.isTrue(account.mint.equals(mint.publicKey));
    });

    it("Can create a random token with prefunding", async () => {
      const token = anchor.web3.Keypair.generate();
      await program.rpc.testInitToken({
        accounts: {
          token: token.publicKey,
          mint: mint.publicKey,
          payer: provider.wallet.publicKey,
          systemProgram: anchor.web3.SystemProgram.programId,
          tokenProgram: TOKEN_PROGRAM_ID,
        },
        signers: [token],
        instructions: [
          anchor.web3.SystemProgram.transfer({
            fromPubkey: provider.wallet.publicKey,
            toPubkey: token.publicKey,
            lamports: 4039280,
          }),
        ],
      });
      const client = new Token(
        program.provider.connection,
        mint.publicKey,
        TOKEN_PROGRAM_ID,
        wallet.payer
      );
      const account = await client.getAccountInfo(token.publicKey);
      // @ts-expect-error
      assert.strictEqual(account.state, 1);
      assert.strictEqual(account.amount.toNumber(), 0);
      assert.isTrue(account.isInitialized);
      assert.isTrue(account.owner.equals(provider.wallet.publicKey));
      assert.isTrue(account.mint.equals(mint.publicKey));
    });

    it("Can create a random token with prefunding under the rent exemption", async () => {
      const token = anchor.web3.Keypair.generate();
      await program.rpc.testInitToken({
        accounts: {
          token: token.publicKey,
          mint: mint.publicKey,
          payer: provider.wallet.publicKey,
          systemProgram: anchor.web3.SystemProgram.programId,
          tokenProgram: TOKEN_PROGRAM_ID,
        },
        signers: [token],
        instructions: [
          anchor.web3.SystemProgram.transfer({
            fromPubkey: provider.wallet.publicKey,
            toPubkey: token.publicKey,
            lamports: 1,
          }),
        ],
      });
      const client = new Token(
        program.provider.connection,
        mint.publicKey,
        TOKEN_PROGRAM_ID,
        wallet.payer
      );
      const account = await client.getAccountInfo(token.publicKey);
      // @ts-expect-error
      assert.strictEqual(account.state, 1);
      assert.strictEqual(account.amount.toNumber(), 0);
      assert.isTrue(account.isInitialized);
      assert.isTrue(account.owner.equals(provider.wallet.publicKey));
      assert.isTrue(account.mint.equals(mint.publicKey));
    });

    it("Can initialize multiple accounts via a composite payer", async () => {
      const data1 = anchor.web3.Keypair.generate();
      const data2 = anchor.web3.Keypair.generate();

      const tx = await program.methods
        .testCompositePayer()
        .accounts({
          composite: {
            data: data1.publicKey,
            payer: provider.wallet.publicKey,
            systemProgram: anchor.web3.SystemProgram.programId,
          },
          data: data2.publicKey,
          systemProgram: anchor.web3.SystemProgram.programId,
        })
        .signers([data1, data2])
        .rpc();

      const account1 = await program.account.dataI8.fetch(data1.publicKey);
      assert.strictEqual(account1.data, 1);

      const account2 = await program.account.data.fetch(data2.publicKey);
      assert.strictEqual(account2.udata.toNumber(), 2);
      assert.strictEqual(account2.idata.toNumber(), 3);
    });

    it("Can create a random mint account with token program", async () => {
      const newMint = anchor.web3.Keypair.generate();
      await program.rpc.testInitMintWithTokenProgram({
        accounts: {
          mint: newMint.publicKey,
          payer: provider.wallet.publicKey,
          systemProgram: anchor.web3.SystemProgram.programId,
          mintTokenProgram: TOKEN_2022_PROGRAM_ID,
        },
        signers: [newMint],
      });
      const rawAccount = await provider.connection.getAccountInfo(
        newMint.publicKey
      );
      const mintAccount = MintLayout.decode(rawAccount.data);
      assert.strictEqual(mintAccount.decimals, 6);
      assert.strictEqual(
        new PublicKey(mintAccount.mintAuthority).toString(),
        provider.wallet.publicKey.toString()
      );
      assert.strictEqual(
        new PublicKey(mintAccount.freezeAuthority).toString(),
        provider.wallet.publicKey.toString()
      );
      assert.strictEqual(
        rawAccount.owner.toString(),
        TOKEN_2022_PROGRAM_ID.toString()
      );
    });

    it("Can create a random token account with token program", async () => {
      const token = anchor.web3.Keypair.generate();
      await program.rpc.testInitTokenWithTokenProgram({
        accounts: {
          token: token.publicKey,
          mint: mint.publicKey,
          payer: provider.wallet.publicKey,
          systemProgram: anchor.web3.SystemProgram.programId,
          tokenTokenProgram: TOKEN_PROGRAM_ID,
        },
        signers: [token],
      });

      const rawAccount = await provider.connection.getAccountInfo(
        token.publicKey
      );
      const ataAccount = AccountLayout.decode(rawAccount.data);
      assert.strictEqual(ataAccount.state, 1);
      assert.strictEqual(new anchor.BN(ataAccount.amount).toNumber(), 0);
      assert.strictEqual(
        new PublicKey(ataAccount.owner).toString(),
        provider.wallet.publicKey.toString()
      );
      assert.strictEqual(
        new PublicKey(ataAccount.mint).toString(),
        mint.publicKey.toString()
      );
    });

    describe("associated_token constraints", () => {
      let associatedToken = null;
      // apparently cannot await here so doing it in the 'it' statements
      let client = Token.createMint(
        program.provider.connection,
        wallet.payer,
        provider.wallet.publicKey,
        provider.wallet.publicKey,
        9,
        TOKEN_PROGRAM_ID
      );

      it("Can create an associated token account", async () => {
        const localClient = await client;
        associatedToken = await Token.getAssociatedTokenAddress(
          ASSOCIATED_TOKEN_PROGRAM_ID,
          TOKEN_PROGRAM_ID,
          localClient.publicKey,
          provider.wallet.publicKey
        );

        await program.rpc.testInitAssociatedToken({
          accounts: {
            token: associatedToken,
            mint: localClient.publicKey,
            payer: provider.wallet.publicKey,
            systemProgram: anchor.web3.SystemProgram.programId,
            tokenProgram: TOKEN_PROGRAM_ID,
            associatedTokenProgram: ASSOCIATED_TOKEN_PROGRAM_ID,
          },
        });

        const account = await localClient.getAccountInfo(associatedToken);
        // @ts-expect-error
        assert.strictEqual(account.state, 1);
        assert.strictEqual(account.amount.toNumber(), 0);
        assert.isTrue(account.isInitialized);
        assert.isTrue(account.owner.equals(provider.wallet.publicKey));
        assert.isTrue(account.mint.equals(localClient.publicKey));
      });

      it("Can create an associated token account with token program", async () => {
        const newMint = anchor.web3.Keypair.generate();
        await program.rpc.testInitMintWithTokenProgram({
          accounts: {
            mint: newMint.publicKey,
            payer: provider.wallet.publicKey,
            systemProgram: anchor.web3.SystemProgram.programId,
            mintTokenProgram: TOKEN_2022_PROGRAM_ID,
          },
          signers: [newMint],
        });

        const associatedToken = await Token.getAssociatedTokenAddress(
          ASSOCIATED_TOKEN_PROGRAM_ID,
          TOKEN_2022_PROGRAM_ID,
          newMint.publicKey,
          provider.wallet.publicKey
        );

        await program.rpc.testInitAssociatedTokenWithTokenProgram({
          accounts: {
            token: associatedToken,
            mint: newMint.publicKey,
            payer: provider.wallet.publicKey,
            systemProgram: anchor.web3.SystemProgram.programId,
            associatedTokenTokenProgram: TOKEN_2022_PROGRAM_ID,
            associatedTokenProgram: ASSOCIATED_TOKEN_PROGRAM_ID,
          },
        });

        const rawAta = await provider.connection.getAccountInfo(
          associatedToken
        );
        const ataAccount = AccountLayout.decode(rawAta.data);
        assert.strictEqual(ataAccount.state, 1);
        assert.strictEqual(new anchor.BN(ataAccount.amount).toNumber(), 0);
        assert.strictEqual(
          new PublicKey(ataAccount.owner).toString(),
          provider.wallet.publicKey.toString()
        );
        assert.strictEqual(
          new PublicKey(ataAccount.mint).toString(),
          newMint.publicKey.toString()
        );
        assert.strictEqual(
          rawAta.owner.toBase58(),
          TOKEN_2022_PROGRAM_ID.toBase58()
        );
      });

      it("Can use fetchNullable() on accounts with only a balance", async () => {
        const account = anchor.web3.Keypair.generate();

        // Airdrop 1 SOL to the account.
        const signature = await program.provider.connection.requestAirdrop(
          account.publicKey,
          anchor.web3.LAMPORTS_PER_SOL
        );
        await program.provider.connection.confirmTransaction(signature);

        const data = await program.account.data.fetchNullable(
          account.publicKey
        );
        assert.isNull(data);
      });

      it("Can validate associated_token constraints", async () => {
        const localClient = await client;
        await program.methods
          .testValidateAssociatedToken()
          .accountsStrict({
            token: associatedToken,
            mint: localClient.publicKey,
            wallet: provider.wallet.publicKey,
          })
          .rpc({ commitment: "confirmed" });

        // Create the second mint through the Anchor provider (program instruction)
        // rather than the legacy `Token.createMint` helper.
        const otherMint = anchor.web3.Keypair.generate();
        await program.rpc.testInitMint({
          accounts: {
            mint: otherMint.publicKey,
            payer: provider.wallet.publicKey,
            systemProgram: anchor.web3.SystemProgram.programId,
            tokenProgram: TOKEN_PROGRAM_ID,
          },
          signers: [otherMint],
        });

        await nativeAssert.rejects(
          async () => {
            await program.methods
              .testValidateAssociatedToken()
              .accountsStrict({
                token: associatedToken,
                mint: otherMint.publicKey,
                wallet: provider.wallet.publicKey,
              })
              .rpc({ commitment: "confirmed" });
          },
          (err) => {
            assert.strictEqual(err.error.errorCode.number, 2009);
            return true;
          }
        );
      });

      it("associated_token constraints check do not allow authority change", async () => {
        const localClient = await client;
        await program.methods
          .testValidateAssociatedToken()
          .accountsStrict({
            token: associatedToken,
            mint: localClient.publicKey,
            wallet: provider.wallet.publicKey,
          })
          .rpc({ commitment: "confirmed" });

        // Change the token account's owner via a SetAuthority instruction sent
        // through the Anchor provider instead of the legacy `setAuthority` helper
        await program.provider.sendAndConfirm(
          new anchor.web3.Transaction().add(
            Token.createSetAuthorityInstruction(
              TOKEN_PROGRAM_ID,
              associatedToken,
              anchor.web3.Keypair.generate().publicKey,
              "AccountOwner",
              provider.wallet.publicKey,
              []
            )
          )
        );

        await nativeAssert.rejects(
          async () => {
            await program.methods
              .testValidateAssociatedToken()
              .accountsStrict({
                token: associatedToken,
                mint: localClient.publicKey,
                wallet: provider.wallet.publicKey,
              })
              .rpc({ commitment: "confirmed" });
          },
          (err) => {
            assert.strictEqual(err.error.errorCode.number, 2015);
            return true;
          }
        );
      });

      it("associated_token constraints (no init) - Can make with associated_token::token_program", async () => {
        const mint = anchor.web3.Keypair.generate();
        await program.rpc.testInitMintWithTokenProgram({
          accounts: {
            mint: mint.publicKey,
            payer: provider.wallet.publicKey,
            systemProgram: anchor.web3.SystemProgram.programId,
            mintTokenProgram: TOKEN_2022_PROGRAM_ID,
          },
          signers: [mint],
        });

        const associatedToken = await Token.getAssociatedTokenAddress(
          ASSOCIATED_TOKEN_PROGRAM_ID,
          TOKEN_2022_PROGRAM_ID,
          mint.publicKey,
          provider.wallet.publicKey
        );

        await program.rpc.testInitAssociatedTokenWithTokenProgram({
          accounts: {
            token: associatedToken,
            mint: mint.publicKey,
            payer: provider.wallet.publicKey,
            systemProgram: anchor.web3.SystemProgram.programId,
            associatedTokenProgram: ASSOCIATED_TOKEN_PROGRAM_ID,
            associatedTokenTokenProgram: TOKEN_2022_PROGRAM_ID,
          },
          signers: [],
        });
        await program.rpc.testAssociatedTokenWithTokenProgramConstraint({
          accounts: {
            token: associatedToken,
            mint: mint.publicKey,
            authority: provider.wallet.publicKey,
            associatedTokenTokenProgram: TOKEN_2022_PROGRAM_ID,
          },
        });

        const account = await provider.connection.getAccountInfo(
          associatedToken
        );
        assert.strictEqual(
          account.owner.toString(),
          TOKEN_2022_PROGRAM_ID.toString()
        );
      });

      it("associated_token constraints (no init) - throws if associated_token::token_program mismatch", async () => {
        const mint = anchor.web3.Keypair.generate();
        await program.rpc.testInitMint({
          accounts: {
            mint: mint.publicKey,
            payer: provider.wallet.publicKey,
            systemProgram: anchor.web3.SystemProgram.programId,
            tokenProgram: TOKEN_PROGRAM_ID,
          },
          signers: [mint],
        });

        const associatedToken = await Token.getAssociatedTokenAddress(
          ASSOCIATED_TOKEN_PROGRAM_ID,
          TOKEN_PROGRAM_ID,
          mint.publicKey,
          provider.wallet.publicKey
        );

        await program.rpc.testInitAssociatedToken({
          accounts: {
            token: associatedToken,
            mint: mint.publicKey,
            payer: provider.wallet.publicKey,
            systemProgram: anchor.web3.SystemProgram.programId,
            tokenProgram: TOKEN_PROGRAM_ID,
            associatedTokenProgram: ASSOCIATED_TOKEN_PROGRAM_ID,
          },
          signers: [],
        });
        const fakeTokenProgram = Keypair.generate();
        try {
          await program.rpc.testAssociatedTokenWithTokenProgramConstraint({
            accounts: {
              token: associatedToken,
              mint: mint.publicKey,
              authority: provider.wallet.publicKey,
              associatedTokenTokenProgram: fakeTokenProgram.publicKey,
            },
          });
          assert.isTrue(false);
        } catch (_err) {
          assert.isTrue(_err instanceof AnchorError);
          const err: AnchorError = _err;
          assert.strictEqual(err.error.errorCode.number, 2023);
          assert.strictEqual(
            err.error.errorCode.code,
            "ConstraintAssociatedTokenTokenProgram"
          );
        }
      });
    });

    it("Can fetch all accounts of a given type", async () => {
      // Initialize the accounts.
      const data1 = anchor.web3.Keypair.generate();
      const data2 = anchor.web3.Keypair.generate();
      const data3 = anchor.web3.Keypair.generate();
      const data4 = anchor.web3.Keypair.generate();
      // Initialize filterable data.
      const filterable1 = anchor.web3.Keypair.generate().publicKey;
      const filterable2 = anchor.web3.Keypair.generate().publicKey;
      // Set up a secondary wallet and program.
      const anotherProvider = new anchor.AnchorProvider(
        program.provider.connection,
        new anchor.Wallet(anchor.web3.Keypair.generate()),
        { commitment: program.provider.connection.commitment }
      );

      const anotherProgram = new anchor.Program(
        { ...miscIdl, address: program.programId },
        anotherProvider
      );
      // Request airdrop for secondary wallet.
      const signature = await program.provider.connection.requestAirdrop(
        anotherProvider.wallet.publicKey,
        anchor.web3.LAMPORTS_PER_SOL
      );
      await program.provider.connection.confirmTransaction(signature);
      // Create all the accounts.
      await program.rpc.testFetchAll(filterable1, {
        accounts: {
          data: data1.publicKey,
          authority: provider.wallet.publicKey,
          systemProgram: anchor.web3.SystemProgram.programId,
        },
        signers: [data1],
      });
      await program.rpc.testFetchAll(filterable1, {
        accounts: {
          data: data2.publicKey,
          authority: provider.wallet.publicKey,
          systemProgram: anchor.web3.SystemProgram.programId,
        },
        signers: [data2],
      });
      await program.rpc.testFetchAll(filterable2, {
        accounts: {
          data: data3.publicKey,
          authority: provider.wallet.publicKey,
          systemProgram: anchor.web3.SystemProgram.programId,
        },
        signers: [data3],
      });
      await anotherProgram.rpc.testFetchAll(filterable1, {
        accounts: {
          data: data4.publicKey,
          authority: anotherProvider.wallet.publicKey,
          systemProgram: anchor.web3.SystemProgram.programId,
        },
        signers: [data4],
      });
      // Call for multiple kinds of .all.
      const allAccounts = await program.account.dataWithFilter.all();
      const allAccountsFilteredByBuffer =
        await program.account.dataWithFilter.all(
          provider.wallet.publicKey.toBuffer()
        );
      const allAccountsFilteredByProgramFilters1 =
        await program.account.dataWithFilter.all([
          {
            memcmp: {
              offset: 8,
              bytes: provider.wallet.publicKey.toBase58(),
            },
          },
          { memcmp: { offset: 40, bytes: filterable1.toBase58() } },
        ]);
      const allAccountsFilteredByProgramFilters2 =
        await program.account.dataWithFilter.all([
          {
            memcmp: {
              offset: 8,
              bytes: provider.wallet.publicKey.toBase58(),
            },
          },
          { memcmp: { offset: 40, bytes: filterable2.toBase58() } },
        ]);
      // Without filters there should be 4 accounts.
      assert.lengthOf(allAccounts, 4);
      // Filtering by main wallet there should be 3 accounts.
      assert.lengthOf(allAccountsFilteredByBuffer, 3);
      // Filtering all the main wallet accounts and matching the filterable1 value
      // results in a 2 accounts.
      assert.lengthOf(allAccountsFilteredByProgramFilters1, 2);
      // Filtering all the main wallet accounts and matching the filterable2 value
      // results in 1 account.
      assert.lengthOf(allAccountsFilteredByProgramFilters2, 1);
    });

    it("Can use pdas with empty seeds", async () => {
      const [pda, bump] = await PublicKey.findProgramAddress(
        [],
        program.programId
      );

      await program.rpc.testInitWithEmptySeeds({
        accounts: {
          pda: pda,
          authority: provider.wallet.publicKey,
          systemProgram: anchor.web3.SystemProgram.programId,
        },
      });
      await program.rpc.testEmptySeedsConstraint({
        accounts: {
          pda: pda,
        },
      });

      const [pda2] = await PublicKey.findProgramAddress(
        [anchor.utils.bytes.utf8.encode("non-empty")],
        program.programId
      );
      await nativeAssert.rejects(
        program.rpc.testEmptySeedsConstraint({
          accounts: {
            pda: pda2,
          },
        }),
        (err) => {
          assert.equal(err.error.errorCode.number, 2006);
          return true;
        }
      );
    });

    const ifNeededAcc = anchor.web3.Keypair.generate();

    it("Can init if needed a new account", async () => {
      await program.rpc.testInitIfNeeded(1, {
        accounts: {
          data: ifNeededAcc.publicKey,
          systemProgram: anchor.web3.SystemProgram.programId,
          payer: provider.wallet.publicKey,
        },
        signers: [ifNeededAcc],
      });
      const account = await program.account.dataU16.fetch(
        ifNeededAcc.publicKey
      );
      assert.strictEqual(account.data, 1);
    });

    it("Can init if needed a previously created account", async () => {
      await program.rpc.testInitIfNeeded(3, {
        accounts: {
          data: ifNeededAcc.publicKey,
          systemProgram: anchor.web3.SystemProgram.programId,
          payer: provider.wallet.publicKey,
        },
        signers: [ifNeededAcc],
      });
      const account = await program.account.dataU16.fetch(
        ifNeededAcc.publicKey
      );
      assert.strictEqual(account.data, 3);
    });

    it("Can use const for array size", async () => {
      const data = anchor.web3.Keypair.generate();
      const tx = await program.rpc.testConstArraySize(99, {
        accounts: {
          data: data.publicKey,
        },
        signers: [data],
        instructions: [
          await program.account.dataConstArraySize.createInstruction(data),
        ],
      });
      const dataAccount = await program.account.dataConstArraySize.fetch(
        data.publicKey
      );
      assert.deepStrictEqual(dataAccount.data, [99, ...new Array(9).fill(0)]);
    });

    it("Can use const for instruction data size", async () => {
      const data = anchor.web3.Keypair.generate();
      const dataArray = [99, ...new Array(9).fill(0)];
      const tx = await program.rpc.testConstIxDataSize(dataArray, {
        accounts: {
          data: data.publicKey,
        },
        signers: [data],
        instructions: [
          await program.account.dataConstArraySize.createInstruction(data),
        ],
      });
      const dataAccount = await program.account.dataConstArraySize.fetch(
        data.publicKey
      );
      assert.deepStrictEqual(dataAccount.data, dataArray);
    });

    it("init_if_needed creates mint account if not exists", async () => {
      const newMint = anchor.web3.Keypair.generate();

      await program.rpc.testInitMintIfNeeded(6, {
        accounts: {
          mint: newMint.publicKey,
          payer: provider.wallet.publicKey,
          systemProgram: anchor.web3.SystemProgram.programId,
          tokenProgram: TOKEN_PROGRAM_ID,
          mintAuthority: provider.wallet.publicKey,
          freezeAuthority: provider.wallet.publicKey,
        },
        signers: [newMint],
      });

      const mintClient = new Token(
        provider.connection,
        newMint.publicKey,
        TOKEN_PROGRAM_ID,
        wallet.payer
      );

      const mintAccount = await mintClient.getMintInfo();
      assert.strictEqual(mintAccount.decimals, 6);
      assert.strictEqual(
        mintAccount.mintAuthority.toString(),
        provider.wallet.publicKey.toString()
      );
      assert.strictEqual(
        mintAccount.freezeAuthority.toString(),
        provider.wallet.publicKey.toString()
      );
      assert.strictEqual(mintAccount.supply.toNumber(), 0);
      const rawAccount = await provider.connection.getAccountInfo(
        newMint.publicKey
      );
      assert.strictEqual(
        rawAccount.owner.toString(),
        TOKEN_PROGRAM_ID.toString()
      );
    });

    it("init_if_needed creates mint account if not exists with token program", async () => {
      const newMint = anchor.web3.Keypair.generate();

      await program.rpc.testInitMintIfNeededWithTokenProgram({
        accounts: {
          mint: newMint.publicKey,
          payer: provider.wallet.publicKey,
          systemProgram: anchor.web3.SystemProgram.programId,
          mintTokenProgram: TOKEN_PROGRAM_ID,
          mintAuthority: provider.wallet.publicKey,
          freezeAuthority: provider.wallet.publicKey,
        },
        signers: [newMint],
      });

      const mintClient = new Token(
        provider.connection,
        newMint.publicKey,
        TOKEN_PROGRAM_ID,
        wallet.payer
      );

      const mintAccount = await mintClient.getMintInfo();
      assert.strictEqual(mintAccount.decimals, 6);
      assert.strictEqual(
        mintAccount.mintAuthority.toString(),
        provider.wallet.publicKey.toString()
      );
      assert.strictEqual(
        mintAccount.freezeAuthority.toString(),
        provider.wallet.publicKey.toString()
      );
      assert.strictEqual(mintAccount.supply.toNumber(), 0);
      const rawAccount = await provider.connection.getAccountInfo(
        newMint.publicKey
      );
      assert.strictEqual(
        rawAccount.owner.toString(),
        TOKEN_PROGRAM_ID.toString()
      );
    });

    it("init_if_needed creates token account if not exists", async () => {
      const newMint = anchor.web3.Keypair.generate();
      await program.rpc.testInitMint({
        accounts: {
          mint: newMint.publicKey,
          payer: provider.wallet.publicKey,
          systemProgram: anchor.web3.SystemProgram.programId,
          tokenProgram: TOKEN_PROGRAM_ID,
        },
        signers: [newMint],
      });

      const newToken = anchor.web3.Keypair.generate();
      await program.rpc.testInitTokenIfNeeded({
        accounts: {
          token: newToken.publicKey,
          mint: newMint.publicKey,
          payer: provider.wallet.publicKey,
          systemProgram: anchor.web3.SystemProgram.programId,
          tokenProgram: TOKEN_PROGRAM_ID,
          authority: provider.wallet.publicKey,
        },
        signers: [newToken],
      });
      const mintClient = new Token(
        provider.connection,
        newMint.publicKey,
        TOKEN_PROGRAM_ID,
        wallet.payer
      );
      const tokenAccount = await mintClient.getAccountInfo(newToken.publicKey);
      assert.strictEqual(tokenAccount.amount.toNumber(), 0);
      assert.strictEqual(
        tokenAccount.mint.toString(),
        newMint.publicKey.toString()
      );
      assert.strictEqual(
        tokenAccount.owner.toString(),
        provider.wallet.publicKey.toString()
      );
      const rawAccount = await provider.connection.getAccountInfo(
        newToken.publicKey
      );
      assert.strictEqual(
        rawAccount.owner.toString(),
        TOKEN_PROGRAM_ID.toString()
      );
    });

    it("init_if_needed creates token account if not exists with token program", async () => {
      const newMint = anchor.web3.Keypair.generate();
      await program.rpc.testInitMint({
        accounts: {
          mint: newMint.publicKey,
          payer: provider.wallet.publicKey,
          systemProgram: anchor.web3.SystemProgram.programId,
          tokenProgram: TOKEN_PROGRAM_ID,
        },
        signers: [newMint],
      });

      const newToken = anchor.web3.Keypair.generate();
      await program.rpc.testInitTokenIfNeededWithTokenProgram({
        accounts: {
          token: newToken.publicKey,
          mint: newMint.publicKey,
          payer: provider.wallet.publicKey,
          systemProgram: anchor.web3.SystemProgram.programId,
          tokenTokenProgram: TOKEN_PROGRAM_ID,
          authority: provider.wallet.publicKey,
        },
        signers: [newToken],
      });

      const rawAccount = await provider.connection.getAccountInfo(
        newToken.publicKey
      );
      const ataAccount = AccountLayout.decode(rawAccount.data);
      assert.strictEqual(new anchor.BN(ataAccount.amount).toNumber(), 0);
      assert.strictEqual(
        new PublicKey(ataAccount.mint).toString(),
        newMint.publicKey.toString()
      );
      assert.strictEqual(
        new PublicKey(ataAccount.owner).toString(),
        provider.wallet.publicKey.toString()
      );

      assert.strictEqual(
        rawAccount.owner.toString(),
        TOKEN_PROGRAM_ID.toString()
      );
    });

    it("init_if_needed creates associated token account if not exists", async () => {
      const newMint = anchor.web3.Keypair.generate();
      await program.rpc.testInitMint({
        accounts: {
          mint: newMint.publicKey,
          payer: provider.wallet.publicKey,
          systemProgram: anchor.web3.SystemProgram.programId,
          tokenProgram: TOKEN_PROGRAM_ID,
        },
        signers: [newMint],
      });

      const associatedToken = await Token.getAssociatedTokenAddress(
        ASSOCIATED_TOKEN_PROGRAM_ID,
        TOKEN_PROGRAM_ID,
        newMint.publicKey,
        provider.wallet.publicKey
      );

      await program.rpc.testInitAssociatedTokenIfNeeded({
        accounts: {
          token: associatedToken,
          mint: newMint.publicKey,
          payer: provider.wallet.publicKey,
          systemProgram: anchor.web3.SystemProgram.programId,
          associatedTokenProgram: ASSOCIATED_TOKEN_PROGRAM_ID,
          tokenProgram: TOKEN_PROGRAM_ID,
          authority: provider.wallet.publicKey,
        },
      });

      const mintClient = new Token(
        provider.connection,
        newMint.publicKey,
        TOKEN_PROGRAM_ID,
        wallet.payer
      );
      const ataAccount = await mintClient.getAccountInfo(associatedToken);
      assert.strictEqual(ataAccount.amount.toNumber(), 0);
      assert.strictEqual(
        ataAccount.mint.toString(),
        newMint.publicKey.toString()
      );
      assert.strictEqual(
        ataAccount.owner.toString(),
        provider.wallet.publicKey.toString()
      );
      const rawAccount = await provider.connection.getAccountInfo(
        associatedToken
      );
      assert.strictEqual(
        rawAccount.owner.toString(),
        TOKEN_PROGRAM_ID.toString()
      );
    });

    it("init_if_needed creates associated token account if not exists with token program", async () => {
      const newMint = anchor.web3.Keypair.generate();
      await program.rpc.testInitMintWithTokenProgram({
        accounts: {
          mint: newMint.publicKey,
          payer: provider.wallet.publicKey,
          systemProgram: anchor.web3.SystemProgram.programId,
          mintTokenProgram: TOKEN_2022_PROGRAM_ID,
        },
        signers: [newMint],
      });

      const associatedToken = await Token.getAssociatedTokenAddress(
        ASSOCIATED_TOKEN_PROGRAM_ID,
        TOKEN_2022_PROGRAM_ID,
        newMint.publicKey,
        provider.wallet.publicKey
      );

      await program.rpc.testInitAssociatedTokenIfNeededWithTokenProgram({
        accounts: {
          token: associatedToken,
          mint: newMint.publicKey,
          payer: provider.wallet.publicKey,
          systemProgram: anchor.web3.SystemProgram.programId,
          associatedTokenProgram: ASSOCIATED_TOKEN_PROGRAM_ID,
          associatedTokenTokenProgram: TOKEN_2022_PROGRAM_ID,
          authority: provider.wallet.publicKey,
        },
      });

      const rawAccount = await provider.connection.getAccountInfo(
        associatedToken
      );
      const ataAccount = AccountLayout.decode(rawAccount.data);
      assert.strictEqual(new anchor.BN(ataAccount.amount).toNumber(), 0);
      assert.strictEqual(
        new PublicKey(ataAccount.mint).toString(),
        newMint.publicKey.toString()
      );
      assert.strictEqual(
        new PublicKey(ataAccount.owner).toString(),
        provider.wallet.publicKey.toString()
      );
      assert.strictEqual(
        rawAccount.owner.toString(),
        TOKEN_2022_PROGRAM_ID.toString()
      );
    });

    it("init_if_needed throws if account exists but is not owned by the expected program", async () => {
      const newAcc = await anchor.web3.PublicKey.findProgramAddress(
        [utf8.encode("hello")],
        program.programId
      );
      await program.rpc.testInitIfNeededChecksOwner({
        accounts: {
          data: newAcc[0],
          systemProgram: anchor.web3.SystemProgram.programId,
          payer: provider.wallet.publicKey,
          owner: program.programId,
        },
      });

      try {
        await program.rpc.testInitIfNeededChecksOwner({
          accounts: {
            data: newAcc[0],
            systemProgram: anchor.web3.SystemProgram.programId,
            payer: provider.wallet.publicKey,
            owner: anchor.web3.Keypair.generate().publicKey,
          },
        });
        expect(false).to.be.true;
      } catch (_err) {
        assert.isTrue(_err instanceof AnchorError);
        const err: AnchorError = _err;
        assert.strictEqual(err.error.errorCode.number, 2004);
      }
    });

    it("init_if_needed throws if pda account exists but does not have the expected seeds", async () => {
      const newAcc = await anchor.web3.PublicKey.findProgramAddress(
        [utf8.encode("nothello")],
        program.programId
      );
      await program.rpc.testInitIfNeededChecksSeeds("nothello", {
        accounts: {
          data: newAcc[0],
          systemProgram: anchor.web3.SystemProgram.programId,
          payer: provider.wallet.publicKey,
        },
      });

      // this will throw if it is not a proper PDA
      // we need this so we know that the following tx failed
      // not because it couldn't create this pda
      // but because the two pdas were different
      anchor.web3.PublicKey.createProgramAddress(
        [utf8.encode("hello")],
        program.programId
      );

      try {
        await program.rpc.testInitIfNeededChecksSeeds("hello", {
          accounts: {
            data: newAcc[0],
            systemProgram: anchor.web3.SystemProgram.programId,
            payer: provider.wallet.publicKey,
          },
        });
        expect(false).to.be.true;
      } catch (_err) {
        assert.isTrue(_err instanceof AnchorError);
        const err: AnchorError = _err;
        assert.strictEqual(err.error.errorCode.number, 2006);
      }
    });

    it("init_if_needed throws if account exists but is not the expected space", async () => {
      const newAcc = anchor.web3.Keypair.generate();
      const _irrelevantForTest = 3;
      await program.rpc.initWithSpace(_irrelevantForTest, {
        accounts: {
          data: newAcc.publicKey,
          systemProgram: anchor.web3.SystemProgram.programId,
          payer: provider.wallet.publicKey,
        },
        signers: [newAcc],
      });

      try {
        await program.rpc.testInitIfNeeded(_irrelevantForTest, {
          accounts: {
            data: newAcc.publicKey,
            systemProgram: anchor.web3.SystemProgram.programId,
            payer: provider.wallet.publicKey,
          },
          signers: [newAcc],
        });
        expect(false).to.be.true;
      } catch (_err) {
        assert.isTrue(_err instanceof AnchorError);
        const err: AnchorError = _err;
        assert.strictEqual(err.error.errorCode.number, 2019);
      }
    });

    it("init_if_needed throws if mint exists but has the wrong mint authority", async () => {
      const mint = anchor.web3.Keypair.generate();
      await program.rpc.testInitMint({
        accounts: {
          mint: mint.publicKey,
          payer: provider.wallet.publicKey,
          systemProgram: anchor.web3.SystemProgram.programId,
          tokenProgram: TOKEN_PROGRAM_ID,
        },
        signers: [mint],
      });

      try {
        await program.rpc.testInitMintIfNeeded(6, {
          accounts: {
            mint: mint.publicKey,
            payer: provider.wallet.publicKey,
            systemProgram: anchor.web3.SystemProgram.programId,
            tokenProgram: TOKEN_PROGRAM_ID,
            mintAuthority: anchor.web3.Keypair.generate().publicKey,
            freezeAuthority: provider.wallet.publicKey,
          },
          signers: [mint],
        });
        expect(false).to.be.true;
      } catch (_err) {
        assert.isTrue(_err instanceof AnchorError);
        const err: AnchorError = _err;
        assert.strictEqual(err.error.errorCode.number, 2016);
      }
    });

    it("init_if_needed throws if mint exists but has the wrong freeze authority", async () => {
      const mint = anchor.web3.Keypair.generate();
      await program.rpc.testInitMint({
        accounts: {
          mint: mint.publicKey,
          payer: provider.wallet.publicKey,
          systemProgram: anchor.web3.SystemProgram.programId,
          tokenProgram: TOKEN_PROGRAM_ID,
        },
        signers: [mint],
      });

      try {
        await program.rpc.testInitMintIfNeeded(6, {
          accounts: {
            mint: mint.publicKey,
            payer: provider.wallet.publicKey,
            systemProgram: anchor.web3.SystemProgram.programId,
            tokenProgram: TOKEN_PROGRAM_ID,
            mintAuthority: provider.wallet.publicKey,
            freezeAuthority: anchor.web3.Keypair.generate().publicKey,
          },
          signers: [mint],
        });
        expect(false).to.be.true;
      } catch (_err) {
        assert.isTrue(_err instanceof AnchorError);
        const err: AnchorError = _err;
        assert.strictEqual(err.error.errorCode.number, 2017);
      }
    });

    it("init_if_needed throws if mint exists but has the wrong decimals", async () => {
      const mint = anchor.web3.Keypair.generate();
      await program.rpc.testInitMint({
        accounts: {
          mint: mint.publicKey,
          payer: provider.wallet.publicKey,
          systemProgram: anchor.web3.SystemProgram.programId,
          tokenProgram: TOKEN_PROGRAM_ID,
        },
        signers: [mint],
      });

      try {
        await program.rpc.testInitMintIfNeeded(9, {
          accounts: {
            mint: mint.publicKey,
            payer: provider.wallet.publicKey,
            systemProgram: anchor.web3.SystemProgram.programId,
            tokenProgram: TOKEN_PROGRAM_ID,
            mintAuthority: provider.wallet.publicKey,
            freezeAuthority: provider.wallet.publicKey,
          },
          signers: [mint],
        });
        expect(false).to.be.true;
      } catch (_err) {
        assert.isTrue(_err instanceof AnchorError);
        const err: AnchorError = _err;
        assert.strictEqual(err.error.errorCode.number, 2018);
      }
    });

    it("init_if_needed pass if mint exists with token program", async () => {
      const newMint = anchor.web3.Keypair.generate();
      await program.rpc.testInitMint({
        accounts: {
          mint: newMint.publicKey,
          payer: provider.wallet.publicKey,
          systemProgram: anchor.web3.SystemProgram.programId,
          tokenProgram: TOKEN_PROGRAM_ID,
        },
        signers: [newMint],
      });

      await program.rpc.testInitMintIfNeededWithTokenProgram({
        accounts: {
          mint: newMint.publicKey,
          payer: provider.wallet.publicKey,
          systemProgram: anchor.web3.SystemProgram.programId,
          mintTokenProgram: TOKEN_PROGRAM_ID,
          mintAuthority: provider.wallet.publicKey,
          freezeAuthority: provider.wallet.publicKey,
        },
        signers: [newMint],
      });
    });

    it("init_if_needed throws if mint exists but has the wrong token program", async () => {
      const newMint = anchor.web3.Keypair.generate();
      await program.rpc.testInitMint({
        accounts: {
          mint: newMint.publicKey,
          payer: provider.wallet.publicKey,
          systemProgram: anchor.web3.SystemProgram.programId,
          tokenProgram: TOKEN_PROGRAM_ID,
        },
        signers: [newMint],
      });

      const fakeTokenProgram = Keypair.generate();
      try {
        await program.rpc.testInitMintIfNeededWithTokenProgram({
          accounts: {
            mint: newMint.publicKey,
            payer: provider.wallet.publicKey,
            systemProgram: anchor.web3.SystemProgram.programId,
            mintTokenProgram: fakeTokenProgram.publicKey,
            mintAuthority: provider.wallet.publicKey,
            freezeAuthority: provider.wallet.publicKey,
          },
          signers: [newMint],
        });
        expect(false).to.be.true;
      } catch (_err) {
        assert.isTrue(_err instanceof AnchorError);
        const err: AnchorError = _err;
        assert.strictEqual(err.error.errorCode.number, 2022);
        assert.strictEqual(
          err.error.errorCode.code,
          "ConstraintMintTokenProgram"
        );
      }
    });

    it("init_if_needed throws if token exists but has the wrong owner", async () => {
      const mint = anchor.web3.Keypair.generate();
      await program.rpc.testInitMint({
        accounts: {
          mint: mint.publicKey,
          payer: provider.wallet.publicKey,
          systemProgram: anchor.web3.SystemProgram.programId,
          tokenProgram: TOKEN_PROGRAM_ID,
        },
        signers: [mint],
      });

      const token = anchor.web3.Keypair.generate();
      await program.rpc.testInitToken({
        accounts: {
          token: token.publicKey,
          mint: mint.publicKey,
          payer: provider.wallet.publicKey,
          systemProgram: anchor.web3.SystemProgram.programId,
          tokenProgram: TOKEN_PROGRAM_ID,
        },
        signers: [token],
      });

      try {
        await program.rpc.testInitTokenIfNeeded({
          accounts: {
            token: token.publicKey,
            mint: mint.publicKey,
            payer: provider.wallet.publicKey,
            systemProgram: anchor.web3.SystemProgram.programId,
            tokenProgram: TOKEN_PROGRAM_ID,
            authority: anchor.web3.Keypair.generate().publicKey,
          },
          signers: [token],
        });
        expect(false).to.be.true;
      } catch (_err) {
        assert.isTrue(_err instanceof AnchorError);
        const err: AnchorError = _err;
        assert.strictEqual(err.error.errorCode.number, 2015);
      }
    });

    it("init_if_needed throws if token exists but has the wrong mint", async () => {
      const mint = anchor.web3.Keypair.generate();
      await program.rpc.testInitMint({
        accounts: {
          mint: mint.publicKey,
          payer: provider.wallet.publicKey,
          systemProgram: anchor.web3.SystemProgram.programId,
          tokenProgram: TOKEN_PROGRAM_ID,
        },
        signers: [mint],
      });

      const mint2 = anchor.web3.Keypair.generate();
      await program.rpc.testInitMint({
        accounts: {
          mint: mint2.publicKey,
          payer: provider.wallet.publicKey,
          systemProgram: anchor.web3.SystemProgram.programId,
          tokenProgram: TOKEN_PROGRAM_ID,
        },
        signers: [mint2],
      });

      const token = anchor.web3.Keypair.generate();
      await program.rpc.testInitToken({
        accounts: {
          token: token.publicKey,
          mint: mint.publicKey,
          payer: provider.wallet.publicKey,
          systemProgram: anchor.web3.SystemProgram.programId,
          tokenProgram: TOKEN_PROGRAM_ID,
        },
        signers: [token],
      });

      try {
        await program.rpc.testInitTokenIfNeeded({
          accounts: {
            token: token.publicKey,
            mint: mint2.publicKey,
            payer: provider.wallet.publicKey,
            systemProgram: anchor.web3.SystemProgram.programId,
            tokenProgram: TOKEN_PROGRAM_ID,
            authority: provider.wallet.publicKey,
          },
          signers: [token],
        });
        expect(false).to.be.true;
      } catch (_err) {
        assert.isTrue(_err instanceof AnchorError);
        const err: AnchorError = _err;
        assert.strictEqual(err.error.errorCode.number, 2014);
      }
    });

    it("init_if_needed pass if token exists with token program", async () => {
      const mint = anchor.web3.Keypair.generate();
      await program.rpc.testInitMint({
        accounts: {
          mint: mint.publicKey,
          payer: provider.wallet.publicKey,
          systemProgram: anchor.web3.SystemProgram.programId,
          tokenProgram: TOKEN_PROGRAM_ID,
        },
        signers: [mint],
      });

      const token = anchor.web3.Keypair.generate();
      await program.rpc.testInitToken({
        accounts: {
          token: token.publicKey,
          mint: mint.publicKey,
          payer: provider.wallet.publicKey,
          systemProgram: anchor.web3.SystemProgram.programId,
          tokenProgram: TOKEN_PROGRAM_ID,
        },
        signers: [token],
      });

      await program.rpc.testInitTokenIfNeededWithTokenProgram({
        accounts: {
          token: token.publicKey,
          mint: mint.publicKey,
          payer: provider.wallet.publicKey,
          systemProgram: anchor.web3.SystemProgram.programId,
          tokenTokenProgram: TOKEN_PROGRAM_ID,
          authority: provider.wallet.publicKey,
        },
        signers: [token],
      });
    });

    it("init_if_needed throws if token exists but has the wrong token program", async () => {
      const mint = anchor.web3.Keypair.generate();
      await program.rpc.testInitMint({
        accounts: {
          mint: mint.publicKey,
          payer: provider.wallet.publicKey,
          systemProgram: anchor.web3.SystemProgram.programId,
          tokenProgram: TOKEN_PROGRAM_ID,
        },
        signers: [mint],
      });

      const token = anchor.web3.Keypair.generate();
      await program.rpc.testInitToken({
        accounts: {
          token: token.publicKey,
          mint: mint.publicKey,
          payer: provider.wallet.publicKey,
          systemProgram: anchor.web3.SystemProgram.programId,
          tokenProgram: TOKEN_PROGRAM_ID,
        },
        signers: [token],
      });

      const fakeTokenProgram = Keypair.generate();
      try {
        await program.rpc.testInitTokenIfNeededWithTokenProgram({
          accounts: {
            token: token.publicKey,
            mint: mint.publicKey,
            payer: provider.wallet.publicKey,
            systemProgram: anchor.web3.SystemProgram.programId,
            tokenTokenProgram: fakeTokenProgram.publicKey,
            authority: provider.wallet.publicKey,
          },
          signers: [token],
        });
        expect(false).to.be.true;
      } catch (_err) {
        assert.isTrue(_err instanceof AnchorError);
        const err: AnchorError = _err;
        assert.strictEqual(err.error.errorCode.number, 2021);
        assert.strictEqual(
          err.error.errorCode.code,
          "ConstraintTokenTokenProgram"
        );
      }
    });

    it("init_if_needed throws if associated token exists but has the wrong owner", async () => {
      const mint = Keypair.generate();
      await program.rpc.testInitMint({
        accounts: {
          mint: mint.publicKey,
          payer: provider.wallet.publicKey,
          systemProgram: SystemProgram.programId,
          tokenProgram: TOKEN_PROGRAM_ID,
        },
        signers: [mint],
      });

      const associatedToken = await Token.getAssociatedTokenAddress(
        ASSOCIATED_TOKEN_PROGRAM_ID,
        TOKEN_PROGRAM_ID,
        mint.publicKey,
        provider.wallet.publicKey
      );

      await program.rpc.testInitAssociatedToken({
        accounts: {
          token: associatedToken,
          mint: mint.publicKey,
          payer: provider.wallet.publicKey,
          systemProgram: anchor.web3.SystemProgram.programId,
          tokenProgram: TOKEN_PROGRAM_ID,
          associatedTokenProgram: ASSOCIATED_TOKEN_PROGRAM_ID,
        },
      });

      try {
        await program.rpc.testInitAssociatedTokenIfNeeded({
          accounts: {
            token: associatedToken,
            mint: mint.publicKey,
            payer: provider.wallet.publicKey,
            systemProgram: anchor.web3.SystemProgram.programId,
            tokenProgram: TOKEN_PROGRAM_ID,
            associatedTokenProgram: ASSOCIATED_TOKEN_PROGRAM_ID,
            authority: anchor.web3.Keypair.generate().publicKey,
          },
        });
        expect(false).to.be.true;
      } catch (_err) {
        assert.isTrue(_err instanceof AnchorError);
        const err: AnchorError = _err;
        assert.strictEqual(err.error.errorCode.number, 2015);
      }
    });

    it("init_if_needed throws if associated token exists but has the wrong mint", async () => {
      const mint = anchor.web3.Keypair.generate();
      await program.rpc.testInitMint({
        accounts: {
          mint: mint.publicKey,
          payer: provider.wallet.publicKey,
          systemProgram: anchor.web3.SystemProgram.programId,
          tokenProgram: TOKEN_PROGRAM_ID,
        },
        signers: [mint],
      });

      const mint2 = anchor.web3.Keypair.generate();
      await program.rpc.testInitMint({
        accounts: {
          mint: mint2.publicKey,
          payer: provider.wallet.publicKey,
          systemProgram: anchor.web3.SystemProgram.programId,
          tokenProgram: TOKEN_PROGRAM_ID,
        },
        signers: [mint2],
      });

      const associatedToken = await Token.getAssociatedTokenAddress(
        ASSOCIATED_TOKEN_PROGRAM_ID,
        TOKEN_PROGRAM_ID,
        mint.publicKey,
        provider.wallet.publicKey
      );

      const txn = await program.rpc.testInitAssociatedToken({
        accounts: {
          token: associatedToken,
          mint: mint.publicKey,
          payer: provider.wallet.publicKey,
          systemProgram: anchor.web3.SystemProgram.programId,
          tokenProgram: TOKEN_PROGRAM_ID,
          associatedTokenProgram: ASSOCIATED_TOKEN_PROGRAM_ID,
        },
      });

      try {
        await program.rpc.testInitAssociatedTokenIfNeeded({
          accounts: {
            token: associatedToken,
            mint: mint2.publicKey,
            payer: provider.wallet.publicKey,
            systemProgram: anchor.web3.SystemProgram.programId,
            tokenProgram: TOKEN_PROGRAM_ID,
            associatedTokenProgram: ASSOCIATED_TOKEN_PROGRAM_ID,
            authority: provider.wallet.publicKey,
          },
        });
        expect(false).to.be.true;
      } catch (_err) {
        assert.isTrue(_err instanceof AnchorError);
        const err: AnchorError = _err;
        assert.strictEqual(err.error.errorCode.number, 2014);
      }
    });

    it("init_if_needed throws if token exists with correct owner and mint but is not the ATA", async () => {
      const mint = anchor.web3.Keypair.generate();
      await program.rpc.testInitMint({
        accounts: {
          mint: mint.publicKey,
          payer: provider.wallet.publicKey,
          systemProgram: anchor.web3.SystemProgram.programId,
          tokenProgram: TOKEN_PROGRAM_ID,
        },
        signers: [mint],
      });

      const associatedToken = await Token.getAssociatedTokenAddress(
        ASSOCIATED_TOKEN_PROGRAM_ID,
        TOKEN_PROGRAM_ID,
        mint.publicKey,
        provider.wallet.publicKey
      );

      await program.rpc.testInitAssociatedToken({
        accounts: {
          token: associatedToken,
          mint: mint.publicKey,
          payer: provider.wallet.publicKey,

          systemProgram: anchor.web3.SystemProgram.programId,
          tokenProgram: TOKEN_PROGRAM_ID,
          associatedTokenProgram: ASSOCIATED_TOKEN_PROGRAM_ID,
        },
      });

      const token = anchor.web3.Keypair.generate();
      await program.rpc.testInitToken({
        accounts: {
          token: token.publicKey,
          mint: mint.publicKey,
          payer: provider.wallet.publicKey,
          systemProgram: anchor.web3.SystemProgram.programId,
          tokenProgram: TOKEN_PROGRAM_ID,
        },
        signers: [token],
      });

      try {
        await program.rpc.testInitAssociatedTokenIfNeeded({
          accounts: {
            token: token.publicKey,
            mint: mint.publicKey,
            payer: provider.wallet.publicKey,

            systemProgram: anchor.web3.SystemProgram.programId,
            tokenProgram: TOKEN_PROGRAM_ID,
            associatedTokenProgram: ASSOCIATED_TOKEN_PROGRAM_ID,
            authority: provider.wallet.publicKey,
          },
        });
        expect(false).to.be.true;
      } catch (_err) {
        assert.isTrue(_err instanceof AnchorError);
        const err: AnchorError = _err;
        assert.strictEqual(err.error.errorCode.number, 3014);
      }
    });

    it("init_if_needed pass if associated token exists", async () => {
      const mint = anchor.web3.Keypair.generate();
      await program.rpc.testInitMint({
        accounts: {
          mint: mint.publicKey,
          payer: provider.wallet.publicKey,
          systemProgram: anchor.web3.SystemProgram.programId,
          tokenProgram: TOKEN_PROGRAM_ID,
        },
        signers: [mint],
      });

      const associatedToken = await Token.getAssociatedTokenAddress(
        ASSOCIATED_TOKEN_PROGRAM_ID,
        TOKEN_PROGRAM_ID,
        mint.publicKey,
        provider.wallet.publicKey
      );

      await program.rpc.testInitAssociatedToken({
        accounts: {
          token: associatedToken,
          mint: mint.publicKey,
          payer: provider.wallet.publicKey,
          systemProgram: anchor.web3.SystemProgram.programId,
          tokenProgram: TOKEN_PROGRAM_ID,
          associatedTokenProgram: ASSOCIATED_TOKEN_PROGRAM_ID,
        },
      });

      await program.rpc.testInitAssociatedTokenIfNeeded({
        accounts: {
          token: associatedToken,
          mint: mint.publicKey,
          payer: provider.wallet.publicKey,
          systemProgram: anchor.web3.SystemProgram.programId,
          tokenProgram: TOKEN_PROGRAM_ID,
          associatedTokenProgram: ASSOCIATED_TOKEN_PROGRAM_ID,
          authority: provider.wallet.publicKey,
        },
      });
    });

    it("init_if_needed pass if associated token exists with token program", async () => {
      const mint = anchor.web3.Keypair.generate();
      await program.rpc.testInitMintWithTokenProgram({
        accounts: {
          mint: mint.publicKey,
          payer: provider.wallet.publicKey,
          systemProgram: anchor.web3.SystemProgram.programId,
          mintTokenProgram: TOKEN_2022_PROGRAM_ID,
        },
        signers: [mint],
      });

      const associatedToken = await Token.getAssociatedTokenAddress(
        ASSOCIATED_TOKEN_PROGRAM_ID,
        TOKEN_2022_PROGRAM_ID,
        mint.publicKey,
        provider.wallet.publicKey
      );

      await program.rpc.testInitAssociatedTokenWithTokenProgram({
        accounts: {
          token: associatedToken,
          mint: mint.publicKey,
          payer: provider.wallet.publicKey,
          systemProgram: anchor.web3.SystemProgram.programId,
          associatedTokenTokenProgram: TOKEN_2022_PROGRAM_ID,
          associatedTokenProgram: ASSOCIATED_TOKEN_PROGRAM_ID,
        },
      });

      await program.rpc.testInitAssociatedTokenIfNeededWithTokenProgram({
        accounts: {
          token: associatedToken,
          mint: mint.publicKey,
          payer: provider.wallet.publicKey,
          systemProgram: anchor.web3.SystemProgram.programId,
          associatedTokenTokenProgram: TOKEN_2022_PROGRAM_ID,
          associatedTokenProgram: ASSOCIATED_TOKEN_PROGRAM_ID,
          authority: provider.wallet.publicKey,
        },
      });
    });

    it("init_if_needed throws if associated token exists but has the wrong token program", async () => {
      const mint = anchor.web3.Keypair.generate();
      await program.rpc.testInitMint({
        accounts: {
          mint: mint.publicKey,
          payer: provider.wallet.publicKey,
          systemProgram: anchor.web3.SystemProgram.programId,
          tokenProgram: TOKEN_PROGRAM_ID,
        },
        signers: [mint],
      });

      const associatedToken = await Token.getAssociatedTokenAddress(
        ASSOCIATED_TOKEN_PROGRAM_ID,
        TOKEN_PROGRAM_ID,
        mint.publicKey,
        provider.wallet.publicKey
      );

      await program.rpc.testInitAssociatedToken({
        accounts: {
          token: associatedToken,
          mint: mint.publicKey,
          payer: provider.wallet.publicKey,
          systemProgram: anchor.web3.SystemProgram.programId,
          tokenProgram: TOKEN_PROGRAM_ID,
          associatedTokenProgram: ASSOCIATED_TOKEN_PROGRAM_ID,
        },
      });

      const fakeTokenProgram = Keypair.generate();
      try {
        await program.rpc.testInitAssociatedTokenIfNeededWithTokenProgram({
          accounts: {
            token: associatedToken,
            mint: mint.publicKey,
            payer: provider.wallet.publicKey,
            systemProgram: anchor.web3.SystemProgram.programId,
            associatedTokenTokenProgram: fakeTokenProgram.publicKey,
            associatedTokenProgram: ASSOCIATED_TOKEN_PROGRAM_ID,
            authority: provider.wallet.publicKey,
          },
        });
        expect(false).to.be.true;
      } catch (_err) {
        assert.isTrue(_err instanceof AnchorError);
        const err: AnchorError = _err;
        assert.strictEqual(err.error.errorCode.number, 2023);
        assert.strictEqual(
          err.error.errorCode.code,
          "ConstraintAssociatedTokenTokenProgram"
        );
      }
    });

    it("Can use multidimensional array", async () => {
      const array2d = new Array(10).fill(new Array(10).fill(99));
      const data = anchor.web3.Keypair.generate();
      await program.rpc.testMultidimensionalArray(array2d, {
        accounts: {
          data: data.publicKey,
        },
        signers: [data],
        instructions: [
          await program.account.dataMultidimensionalArray.createInstruction(
            data
          ),
        ],
      });
      const dataAccount = await program.account.dataMultidimensionalArray.fetch(
        data.publicKey
      );
      assert.deepStrictEqual(dataAccount.data, array2d);
    });

    it("Can use multidimensional array with const sizes", async () => {
      const array2d = new Array(10).fill(new Array(11).fill(22));
      const data = anchor.web3.Keypair.generate();
      await program.rpc.testMultidimensionalArrayConstSizes(array2d, {
        accounts: {
          data: data.publicKey,
        },
        signers: [data],
        instructions: [
          await program.account.dataMultidimensionalArrayConstSizes.createInstruction(
            data
          ),
        ],
      });
      const dataAccount =
        await program.account.dataMultidimensionalArrayConstSizes.fetch(
          data.publicKey
        );
      assert.deepStrictEqual(dataAccount.data, array2d);
    });

    it("Can initialize 5 associated token accounts in one instruction", async () => {
      const mint = anchor.web3.Keypair.generate();
      await program.methods
        .testInitManyAssociatedTokenAccounts()
        .accounts({ mint: mint.publicKey, user: provider.wallet.publicKey })
        .signers([mint])
        .rpc();
    });

    describe("Can validate PDAs derived from other program ids", () => {
      it("With bumps using create_program_address", async () => {
        const [firstPDA, firstBump] =
          await anchor.web3.PublicKey.findProgramAddress(
            [anchor.utils.bytes.utf8.encode("seed")],
            ASSOCIATED_TOKEN_PROGRAM_ID
          );
        const [secondPDA, secondBump] =
          await anchor.web3.PublicKey.findProgramAddress(
            [anchor.utils.bytes.utf8.encode("seed")],
            program.programId
          );

        // correct bump but wrong address
        const wrongAddress = anchor.web3.Keypair.generate().publicKey;
        try {
          await program.rpc.testProgramIdConstraint(firstBump, secondBump, {
            accounts: {
              first: wrongAddress,
              second: secondPDA,
            },
          });
          expect(false).to.be.true;
        } catch (_err) {
          assert.isTrue(_err instanceof AnchorError);
          const err: AnchorError = _err;
          assert.strictEqual(err.error.errorCode.number, 2006);
        }

        // matching bump seed for wrong address but derived from wrong program
        try {
          await program.rpc.testProgramIdConstraint(secondBump, secondBump, {
            accounts: {
              first: secondPDA,
              second: secondPDA,
            },
          });
          expect(false).to.be.true;
        } catch (_err) {
          assert.isTrue(_err instanceof AnchorError);
          const err: AnchorError = _err;
          assert.strictEqual(err.error.errorCode.number, 2006);
        }

        // correct inputs should lead to successful tx
        await program.rpc.testProgramIdConstraint(firstBump, secondBump, {
          accounts: {
            first: firstPDA,
            second: secondPDA,
          },
        });
      });

      it("With bumps using find_program_address", async () => {
        const firstPDA = (
          await anchor.web3.PublicKey.findProgramAddress(
            [anchor.utils.bytes.utf8.encode("seed")],
            ASSOCIATED_TOKEN_PROGRAM_ID
          )
        )[0];
        const secondPDA = (
          await anchor.web3.PublicKey.findProgramAddress(
            [anchor.utils.bytes.utf8.encode("seed")],
            program.programId
          )
        )[0];

        // random wrong address
        const wrongAddress = anchor.web3.Keypair.generate().publicKey;
        try {
          await program.rpc.testProgramIdConstraintFindPda({
            accounts: {
              first: wrongAddress,
              second: secondPDA,
            },
          });
          expect(false).to.be.true;
        } catch (_err) {
          assert.isTrue(_err instanceof AnchorError);
          const err: AnchorError = _err;
          assert.strictEqual(err.error.errorCode.number, 2006);
        }

        // same seeds but derived from wrong program
        try {
          await program.rpc.testProgramIdConstraintFindPda({
            accounts: {
              first: secondPDA,
              second: secondPDA,
            },
          });
          expect(false).to.be.true;
        } catch (_err) {
          assert.isTrue(_err instanceof AnchorError);
          const err: AnchorError = _err;
          assert.strictEqual(err.error.errorCode.number, 2006);
        }

        // correct inputs should lead to successful tx
        await program.rpc.testProgramIdConstraintFindPda({
          accounts: {
            first: firstPDA,
            second: secondPDA,
          },
        });
      });
    });

    describe("Token Constraint Test", () => {
      it("Token Constraint Test(no init) - Can make token::mint and token::authority", async () => {
        const mint = anchor.web3.Keypair.generate();
        await program.rpc.testInitMint({
          accounts: {
            mint: mint.publicKey,
            payer: provider.wallet.publicKey,
            systemProgram: anchor.web3.SystemProgram.programId,
            tokenProgram: TOKEN_PROGRAM_ID,
          },
          signers: [mint],
        });

        const token = anchor.web3.Keypair.generate();
        await program.rpc.testInitToken({
          accounts: {
            token: token.publicKey,
            mint: mint.publicKey,
            payer: provider.wallet.publicKey,
            systemProgram: anchor.web3.SystemProgram.programId,
            tokenProgram: TOKEN_PROGRAM_ID,
          },
          signers: [token],
        });
        await program.rpc.testTokenConstraint({
          accounts: {
            token: token.publicKey,
            mint: mint.publicKey,
            payer: provider.wallet.publicKey,
          },
        });
        const mintAccount = new Token(
          program.provider.connection,
          mint.publicKey,
          TOKEN_PROGRAM_ID,
          wallet.payer
        );
        const account = await mintAccount.getAccountInfo(token.publicKey);
        assert.isTrue(account.owner.equals(provider.wallet.publicKey));
        assert.isTrue(account.mint.equals(mint.publicKey));
      });

      it("Token Constraint Test(no init) - Can make only token::authority", async () => {
        const mint = anchor.web3.Keypair.generate();
        await program.rpc.testInitMint({
          accounts: {
            mint: mint.publicKey,
            payer: provider.wallet.publicKey,
            systemProgram: anchor.web3.SystemProgram.programId,
            tokenProgram: TOKEN_PROGRAM_ID,
          },
          signers: [mint],
        });

        const token = anchor.web3.Keypair.generate();
        await program.rpc.testInitToken({
          accounts: {
            token: token.publicKey,
            mint: mint.publicKey,
            payer: provider.wallet.publicKey,
            systemProgram: anchor.web3.SystemProgram.programId,
            tokenProgram: TOKEN_PROGRAM_ID,
          },
          signers: [token],
        });
        await program.rpc.testOnlyAuthConstraint({
          accounts: {
            token: token.publicKey,
            mint: mint.publicKey,
            payer: provider.wallet.publicKey,
          },
        });
        const mintAccount = new Token(
          program.provider.connection,
          mint.publicKey,
          TOKEN_PROGRAM_ID,
          wallet.payer
        );
        const account = await mintAccount.getAccountInfo(token.publicKey);
        assert.isTrue(account.owner.equals(provider.wallet.publicKey));
      });

      it("Token Constraint Test(no init) - Can make only token::mint", async () => {
        const mint = anchor.web3.Keypair.generate();
        await program.rpc.testInitMint({
          accounts: {
            mint: mint.publicKey,
            payer: provider.wallet.publicKey,
            systemProgram: anchor.web3.SystemProgram.programId,
            tokenProgram: TOKEN_PROGRAM_ID,
          },
          signers: [mint],
        });

        const token = anchor.web3.Keypair.generate();
        await program.rpc.testInitToken({
          accounts: {
            token: token.publicKey,
            mint: mint.publicKey,
            payer: provider.wallet.publicKey,
            systemProgram: anchor.web3.SystemProgram.programId,
            tokenProgram: TOKEN_PROGRAM_ID,
          },
          signers: [token],
        });
        await program.rpc.testOnlyMintConstraint({
          accounts: {
            token: token.publicKey,
            mint: mint.publicKey,
          },
        });
        const mintAccount = new Token(
          program.provider.connection,
          mint.publicKey,
          TOKEN_PROGRAM_ID,
          wallet.payer
        );
        const account = await mintAccount.getAccountInfo(token.publicKey);
        assert.isTrue(account.mint.equals(mint.publicKey));
      });

      it("Token Constraint Test(no init) - Can make only token::token_program", async () => {
        const mint = anchor.web3.Keypair.generate();
        await program.rpc.testInitMint({
          accounts: {
            mint: mint.publicKey,
            payer: provider.wallet.publicKey,
            systemProgram: anchor.web3.SystemProgram.programId,
            tokenProgram: TOKEN_PROGRAM_ID,
          },
          signers: [mint],
        });

        const token = anchor.web3.Keypair.generate();
        await program.rpc.testInitToken({
          accounts: {
            token: token.publicKey,
            mint: mint.publicKey,
            payer: provider.wallet.publicKey,
            systemProgram: anchor.web3.SystemProgram.programId,
            tokenProgram: TOKEN_PROGRAM_ID,
          },
          signers: [token],
        });
        await program.rpc.testOnlyTokenProgramConstraint({
          accounts: {
            token: token.publicKey,
            tokenTokenProgram: TOKEN_PROGRAM_ID,
          },
        });

        const account = await provider.connection.getAccountInfo(
          token.publicKey
        );
        assert.strictEqual(
          account.owner.toString(),
          TOKEN_PROGRAM_ID.toString()
        );
      });

      it("Token Constraint Test(no init) - throws if token::mint mismatch", async () => {
        const mint = anchor.web3.Keypair.generate();
        await program.rpc.testInitMint({
          accounts: {
            mint: mint.publicKey,
            payer: provider.wallet.publicKey,
            systemProgram: anchor.web3.SystemProgram.programId,
            tokenProgram: TOKEN_PROGRAM_ID,
          },
          signers: [mint],
        });

        const mint1 = anchor.web3.Keypair.generate();
        await program.rpc.testInitMint({
          accounts: {
            mint: mint1.publicKey,
            payer: provider.wallet.publicKey,
            systemProgram: anchor.web3.SystemProgram.programId,
            tokenProgram: TOKEN_PROGRAM_ID,
          },
          signers: [mint1],
        });

        const token = anchor.web3.Keypair.generate();
        await program.rpc.testInitToken({
          accounts: {
            token: token.publicKey,
            mint: mint.publicKey,
            payer: provider.wallet.publicKey,
            systemProgram: anchor.web3.SystemProgram.programId,
            tokenProgram: TOKEN_PROGRAM_ID,
          },
          signers: [token],
        });
        try {
          await program.rpc.testTokenConstraint({
            accounts: {
              token: token.publicKey,
              mint: mint1.publicKey,
              payer: provider.wallet.publicKey,
            },
          });
          assert.isTrue(false);
        } catch (_err) {
          assert.isTrue(_err instanceof AnchorError);
          const err: AnchorError = _err;
          assert.strictEqual(err.error.errorCode.number, 2014);
          assert.strictEqual(err.error.errorCode.code, "ConstraintTokenMint");
        }
      });

      it("Token Constraint Test(no init) - throws if token::authority mismatch", async () => {
        const mint = anchor.web3.Keypair.generate();
        await program.rpc.testInitMint({
          accounts: {
            mint: mint.publicKey,
            payer: provider.wallet.publicKey,
            systemProgram: anchor.web3.SystemProgram.programId,
            tokenProgram: TOKEN_PROGRAM_ID,
          },
          signers: [mint],
        });
        const token = anchor.web3.Keypair.generate();
        await program.rpc.testInitToken({
          accounts: {
            token: token.publicKey,
            mint: mint.publicKey,
            payer: provider.wallet.publicKey,
            systemProgram: anchor.web3.SystemProgram.programId,
            tokenProgram: TOKEN_PROGRAM_ID,
          },
          signers: [token],
        });
        const fakeAuthority = Keypair.generate();
        try {
          await program.rpc.testTokenAuthConstraint({
            accounts: {
              token: token.publicKey,
              mint: mint.publicKey,
              fakeAuthority: fakeAuthority.publicKey,
            },
          });
          assert.isTrue(false);
        } catch (_err) {
          assert.isTrue(_err instanceof AnchorError);
          const err: AnchorError = _err;
          assert.strictEqual(err.error.errorCode.number, 2015);
          assert.strictEqual(err.error.errorCode.code, "ConstraintTokenOwner");
        }
      });

      it("Token Constraint Test(no init) - throws if both token::authority, token::mint mismatch", async () => {
        const mint = anchor.web3.Keypair.generate();
        await program.rpc.testInitMint({
          accounts: {
            mint: mint.publicKey,
            payer: provider.wallet.publicKey,
            systemProgram: anchor.web3.SystemProgram.programId,
            tokenProgram: TOKEN_PROGRAM_ID,
          },
          signers: [mint],
        });
        const mint1 = anchor.web3.Keypair.generate();
        await program.rpc.testInitMint({
          accounts: {
            mint: mint1.publicKey,
            payer: provider.wallet.publicKey,
            systemProgram: anchor.web3.SystemProgram.programId,
            tokenProgram: TOKEN_PROGRAM_ID,
          },
          signers: [mint1],
        });
        const token = anchor.web3.Keypair.generate();
        await program.rpc.testInitToken({
          accounts: {
            token: token.publicKey,
            mint: mint.publicKey,
            payer: provider.wallet.publicKey,
            systemProgram: anchor.web3.SystemProgram.programId,
            tokenProgram: TOKEN_PROGRAM_ID,
          },
          signers: [token],
        });
        const fakeAuthority = Keypair.generate();
        try {
          await program.rpc.testTokenAuthConstraint({
            accounts: {
              token: token.publicKey,
              mint: mint1.publicKey,
              fakeAuthority: fakeAuthority.publicKey,
            },
          });
          assert.isTrue(false);
        } catch (_err) {
          assert.isTrue(_err instanceof AnchorError);
          const err: AnchorError = _err;
          assert.strictEqual(err.error.errorCode.number, 2015);
          assert.strictEqual(err.error.errorCode.code, "ConstraintTokenOwner");
        }
      });

      it("Token Constraint Test(no init) - throws if token::token_program mismatch", async () => {
        const mint = anchor.web3.Keypair.generate();
        await program.rpc.testInitMint({
          accounts: {
            mint: mint.publicKey,
            payer: provider.wallet.publicKey,
            systemProgram: anchor.web3.SystemProgram.programId,
            tokenProgram: TOKEN_PROGRAM_ID,
          },
          signers: [mint],
        });
        const token = anchor.web3.Keypair.generate();
        await program.rpc.testInitToken({
          accounts: {
            token: token.publicKey,
            mint: mint.publicKey,
            payer: provider.wallet.publicKey,
            systemProgram: anchor.web3.SystemProgram.programId,
            tokenProgram: TOKEN_PROGRAM_ID,
          },
          signers: [token],
        });
        const fakeTokenProgram = Keypair.generate();
        try {
          await program.rpc.testOnlyTokenProgramConstraint({
            accounts: {
              token: token.publicKey,
              tokenTokenProgram: fakeTokenProgram.publicKey,
            },
          });
          assert.isTrue(false);
        } catch (_err) {
          assert.isTrue(_err instanceof AnchorError);
          const err: AnchorError = _err;
          assert.strictEqual(err.error.errorCode.number, 2021);
          assert.strictEqual(
            err.error.errorCode.code,
            "ConstraintTokenTokenProgram"
          );
        }
      });

      it("Mint Constraint Test(no init) - mint::decimals, mint::authority, mint::freeze_authority", async () => {
        const mint = anchor.web3.Keypair.generate();
        await program.rpc.testInitMint({
          accounts: {
            mint: mint.publicKey,
            payer: provider.wallet.publicKey,
            systemProgram: anchor.web3.SystemProgram.programId,
            tokenProgram: TOKEN_PROGRAM_ID,
          },
          signers: [mint],
        });
        await program.rpc.testMintConstraint(6, {
          accounts: {
            mint: mint.publicKey,
            mintAuthority: provider.wallet.publicKey,
            freezeAuthority: provider.wallet.publicKey,
          },
        });
        const client = new Token(
          program.provider.connection,
          mint.publicKey,
          TOKEN_PROGRAM_ID,
          wallet.payer
        );
        const mintAccount = await client.getMintInfo();
        assert.strictEqual(mintAccount.decimals, 6);
        assert.isTrue(
          mintAccount.mintAuthority.equals(provider.wallet.publicKey)
        );
        assert.isTrue(
          mintAccount.freezeAuthority.equals(provider.wallet.publicKey)
        );
      });

      it("Mint Constraint Test(no init) - throws if mint::decimals mismatch", async () => {
        const mint = anchor.web3.Keypair.generate();
        await program.rpc.testInitMint({
          accounts: {
            mint: mint.publicKey,
            payer: provider.wallet.publicKey,
            systemProgram: anchor.web3.SystemProgram.programId,
            tokenProgram: TOKEN_PROGRAM_ID,
          },
          signers: [mint],
        });
        const fakeDecimal = 5;
        try {
          await program.rpc.testMintConstraint(fakeDecimal, {
            accounts: {
              mint: mint.publicKey,
              mintAuthority: provider.wallet.publicKey,
              freezeAuthority: provider.wallet.publicKey,
            },
          });
          assert.isTrue(false);
        } catch (_err) {
          assert.isTrue(_err instanceof AnchorError);
          const err: AnchorError = _err;
          assert.strictEqual(err.error.errorCode.number, 2018);
          assert.strictEqual(
            err.error.errorCode.code,
            "ConstraintMintDecimals"
          );
        }
      });

      it("Mint Constraint Test(no init) - throws if mint::mint_authority mismatch", async () => {
        const mint = anchor.web3.Keypair.generate();
        await program.rpc.testInitMint({
          accounts: {
            mint: mint.publicKey,
            payer: provider.wallet.publicKey,
            systemProgram: anchor.web3.SystemProgram.programId,
            tokenProgram: TOKEN_PROGRAM_ID,
          },
          signers: [mint],
        });

        const fakeAuthority = Keypair.generate();
        try {
          await program.rpc.testMintConstraint(6, {
            accounts: {
              mint: mint.publicKey,
              mintAuthority: fakeAuthority.publicKey,
              freezeAuthority: provider.wallet.publicKey,
            },
          });
          assert.isTrue(false);
        } catch (_err) {
          assert.isTrue(_err instanceof AnchorError);
          const err: AnchorError = _err;
          assert.strictEqual(err.error.errorCode.number, 2016);
          assert.strictEqual(
            err.error.errorCode.code,
            "ConstraintMintMintAuthority"
          );
        }
      });

      it("Mint Constraint Test(no init) - throws if mint::freeze_authority mismatch", async () => {
        const mint = anchor.web3.Keypair.generate();
        await program.rpc.testInitMint({
          accounts: {
            mint: mint.publicKey,
            payer: provider.wallet.publicKey,
            systemProgram: anchor.web3.SystemProgram.programId,
            tokenProgram: TOKEN_PROGRAM_ID,
          },
          signers: [mint],
        });

        const fakeAuthority = Keypair.generate();
        try {
          await program.rpc.testMintConstraint(6, {
            accounts: {
              mint: mint.publicKey,
              mintAuthority: provider.wallet.publicKey,
              freezeAuthority: fakeAuthority.publicKey,
            },
          });
          assert.isTrue(false);
        } catch (_err) {
          assert.isTrue(_err instanceof AnchorError);
          const err: AnchorError = _err;
          assert.strictEqual(err.error.errorCode.number, 2017);
          assert.strictEqual(
            err.error.errorCode.code,
            "ConstraintMintFreezeAuthority"
          );
        }
      });

      it("Mint Constraint Test(no init) - throws if mint::token_program mismatch", async () => {
        const mint = anchor.web3.Keypair.generate();
        await program.rpc.testInitMint({
          accounts: {
            mint: mint.publicKey,
            payer: provider.wallet.publicKey,
            systemProgram: anchor.web3.SystemProgram.programId,
            tokenProgram: TOKEN_PROGRAM_ID,
          },
          signers: [mint],
        });

        const fakeTokenProgram = Keypair.generate();
        try {
          await program.rpc.testMintOnlyTokenProgramConstraint({
            accounts: {
              mint: mint.publicKey,
              mintTokenProgram: fakeTokenProgram.publicKey,
            },
          });
          assert.isTrue(false);
        } catch (_err) {
          assert.isTrue(_err instanceof AnchorError);
          const err: AnchorError = _err;
          assert.strictEqual(err.error.errorCode.number, 2022);
          assert.strictEqual(
            err.error.errorCode.code,
            "ConstraintMintTokenProgram"
          );
        }
      });

      it("Mint Constraint Test(no init) - can write only mint::decimals", async () => {
        const mint = anchor.web3.Keypair.generate();
        await program.rpc.testInitMint({
          accounts: {
            mint: mint.publicKey,
            payer: provider.wallet.publicKey,
            systemProgram: anchor.web3.SystemProgram.programId,
            tokenProgram: TOKEN_PROGRAM_ID,
          },
          signers: [mint],
        });

        await program.rpc.testMintOnlyDecimalsConstraint(6, {
          accounts: {
            mint: mint.publicKey,
          },
        });
        const client = new Token(
          program.provider.connection,
          mint.publicKey,
          TOKEN_PROGRAM_ID,
          wallet.payer
        );
        const mintAccount = await client.getMintInfo();
        assert.strictEqual(mintAccount.decimals, 6);
      });

      it("Mint Constraint Test(no init) - can write only mint::authority and mint::freeze_authority", async () => {
        const mint = anchor.web3.Keypair.generate();
        await program.rpc.testInitMint({
          accounts: {
            mint: mint.publicKey,
            payer: provider.wallet.publicKey,
            systemProgram: anchor.web3.SystemProgram.programId,
            tokenProgram: TOKEN_PROGRAM_ID,
          },
          signers: [mint],
        });

        await program.rpc.testMintOnlyAuthConstraint({
          accounts: {
            mint: mint.publicKey,
            mintAuthority: provider.wallet.publicKey,
            freezeAuthority: provider.wallet.publicKey,
          },
        });
        const client = new Token(
          program.provider.connection,
          mint.publicKey,
          TOKEN_PROGRAM_ID,
          wallet.payer
        );
        const mintAccount = await client.getMintInfo();
        assert.isTrue(
          mintAccount.mintAuthority.equals(provider.wallet.publicKey)
        );
        assert.isTrue(
          mintAccount.freezeAuthority.equals(provider.wallet.publicKey)
        );
      });

      it("Mint Constraint Test(no init) - can write only mint::authority", async () => {
        const mint = anchor.web3.Keypair.generate();
        await program.rpc.testInitMint({
          accounts: {
            mint: mint.publicKey,
            payer: provider.wallet.publicKey,
            systemProgram: anchor.web3.SystemProgram.programId,
            tokenProgram: TOKEN_PROGRAM_ID,
          },
          signers: [mint],
        });

        await program.rpc.testMintOnlyOneAuthConstraint({
          accounts: {
            mint: mint.publicKey,
            mintAuthority: provider.wallet.publicKey,
          },
        });
        const client = new Token(
          program.provider.connection,
          mint.publicKey,
          TOKEN_PROGRAM_ID,
          wallet.payer
        );
        const mintAccount = await client.getMintInfo();
        assert.isTrue(
          mintAccount.mintAuthority.equals(provider.wallet.publicKey)
        );
      });

      it("Mint Constraint Test(no init) - can write only mint::decimals and mint::freeze_authority", async () => {
        const mint = anchor.web3.Keypair.generate();
        await program.rpc.testInitMint({
          accounts: {
            mint: mint.publicKey,
            payer: provider.wallet.publicKey,
            systemProgram: anchor.web3.SystemProgram.programId,
            tokenProgram: TOKEN_PROGRAM_ID,
          },
          signers: [mint],
        });

        await program.rpc.testMintMissMintAuthConstraint(6, {
          accounts: {
            mint: mint.publicKey,
            freezeAuthority: provider.wallet.publicKey,
          },
        });
        const client = new Token(
          program.provider.connection,
          mint.publicKey,
          TOKEN_PROGRAM_ID,
          wallet.payer
        );
        const mintAccount = await client.getMintInfo();
        assert.strictEqual(mintAccount.decimals, 6);
        assert.isTrue(
          mintAccount.freezeAuthority.equals(provider.wallet.publicKey)
        );
      });

      it("Mint Constraint Test(no init) - can write only mint::token_program", async () => {
        const mint = anchor.web3.Keypair.generate();
        await program.rpc.testInitMint({
          accounts: {
            mint: mint.publicKey,
            payer: provider.wallet.publicKey,
            systemProgram: anchor.web3.SystemProgram.programId,
            tokenProgram: TOKEN_PROGRAM_ID,
          },
          signers: [mint],
        });

        await program.rpc.testMintOnlyTokenProgramConstraint({
          accounts: {
            mint: mint.publicKey,
            mintTokenProgram: TOKEN_PROGRAM_ID,
          },
        });
        const mintAccount = await provider.connection.getAccountInfo(
          mint.publicKey
        );
        assert.strictEqual(
          mintAccount.owner.toString(),
          TOKEN_PROGRAM_ID.toString()
        );
      });

      it("check versioned transaction is now available", async () => {
        let thisTx = new VersionedTransaction(
          new Message({
            header: {
              numReadonlySignedAccounts: 0,
              numReadonlyUnsignedAccounts: 0,
              numRequiredSignatures: 0,
            },
            accountKeys: [new PublicKey([0]).toString()],
            instructions: [{ accounts: [0], data: "", programIdIndex: 0 }],
            recentBlockhash: "",
          })
        );
        assert.isDefined(thisTx);
      });
    });

    describe("`zero` constraint unique account checks", () => {
      it("Works with different accounts (multiple `zero`)", async () => {
        const oneKp = anchor.web3.Keypair.generate();
        const twoKp = anchor.web3.Keypair.generate();
        await program.methods
          .testMultipleZeroConstraint()
          .preInstructions(
            await Promise.all([
              program.account.data.createInstruction(oneKp),
              program.account.data.createInstruction(twoKp),
            ])
          )
          .accounts({ one: oneKp.publicKey, two: twoKp.publicKey })
          .signers([oneKp, twoKp])
          .rpc();
      });

      it("Throws with the same account (multiple `zero`)", async () => {
        const kp = anchor.web3.Keypair.generate();
        try {
          await program.methods
            .testMultipleZeroConstraint()
            .preInstructions([await program.account.data.createInstruction(kp)])
            .accounts({ one: kp.publicKey, two: kp.publicKey })
            .signers([kp])
            .rpc();
          assert.fail("Transaction did not fail!");
        } catch (e) {
          assert(e instanceof AnchorError);
          const err: AnchorError = e;
          assert.strictEqual(
            err.error.errorCode.number,
            anchor.LangErrorCode.ConstraintZero
          );
        }
      });

      it("Works with different accounts (`init` and `zero`)", async () => {
        const initKp = anchor.web3.Keypair.generate();
        const zeroKp = anchor.web3.Keypair.generate();
        await program.methods
          .testInitAndZero()
          .preInstructions([
            await program.account.data.createInstruction(zeroKp),
          ])
          .accounts({ init: initKp.publicKey, zero: zeroKp.publicKey })
          .signers([initKp, zeroKp])
          .rpc();
      });

      it("Throws with the same account (`init` and `zero`)", async () => {
        const kp = anchor.web3.Keypair.generate();
        try {
          await program.methods
            .testInitAndZero()
            .accounts({ init: kp.publicKey, zero: kp.publicKey })
            .signers([kp])
            .rpc();
          assert.fail("Transaction did not fail!");
        } catch (e) {
          assert(e instanceof AnchorError);
          const err: AnchorError = e;
          assert.strictEqual(
            err.error.errorCode.number,
            anchor.LangErrorCode.ConstraintZero
          );
        }
      });
    });
  };
};

export default miscTest;

describe("misc", miscTest(anchor.workspace.misc as Program<Misc>));

describe(
  "misc-optional",
  miscTest(anchor.workspace.miscOptional as Program<MiscOptional>)
);
