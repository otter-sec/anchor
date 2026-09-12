import * as anchor from "@anchor-lang/core";
import { Program, BN, IdlAccounts } from "@anchor-lang/core";
import { PublicKey, Keypair, SystemProgram } from "@solana/web3.js";
import { TOKEN_PROGRAM_ID, Token } from "@solana/spl-token";
import { assert } from "chai";
import { Escrow } from "../target/types/escrow";

type EscrowAccount = IdlAccounts<Escrow>["escrowAccount"];

describe("escrow", () => {
  const provider = anchor.AnchorProvider.env();
  anchor.setProvider(provider);

  const TOKEN_2022_PROGRAM_ID = new anchor.web3.PublicKey(
    "TokenzQdBNbLqP5VEhdkAS6EPFLC1PHnBqCXEpPxuEb"
  );
  const TEST_PROGRAM_IDS = [
    [TOKEN_PROGRAM_ID, TOKEN_PROGRAM_ID],
    [TOKEN_2022_PROGRAM_ID, TOKEN_2022_PROGRAM_ID],
    [TOKEN_PROGRAM_ID, TOKEN_2022_PROGRAM_ID],
  ];
  const program = anchor.workspace.Escrow as Program<Escrow>;

  let mintA: Token = null;
  let mintB: Token = null;
  let initializerTokenAccountA: PublicKey = null;
  let initializerTokenAccountB: PublicKey = null;
  let takerTokenAccountA: PublicKey = null;
  let takerTokenAccountB: PublicKey = null;
  let pda: PublicKey = null;

  const takerAmount = 1000;
  const initializerAmount = 500;

  const payer = Keypair.generate();
  const mintAuthority = Keypair.generate();

  TEST_PROGRAM_IDS.forEach((tokenProgramIds) => {
    const escrowAccount = Keypair.generate();
    const [tokenProgramIdA, tokenProgramIdB] = tokenProgramIds;
    let name;
    if (tokenProgramIdA === tokenProgramIdB) {
      name = tokenProgramIdA === TOKEN_PROGRAM_ID ? "token" : "token-2022";
    } else {
      name = "mixed";
    }
    describe(name, () => {
      it("Initialise escrow state", async () => {
        // Airdropping tokens to a payer.
        await provider.connection.confirmTransaction(
          await provider.connection.requestAirdrop(
            payer.publicKey,
            10000000000
          ),
          "confirmed"
        );

        mintA = await Token.createMint(
          provider.connection,
          payer,
          mintAuthority.publicKey,
          null,
          0,
          tokenProgramIdA
        );

        mintB = await Token.createMint(
          provider.connection,
          payer,
          mintAuthority.publicKey,
          null,
          0,
          tokenProgramIdB
        );

        initializerTokenAccountA = await mintA.createAccount(
          provider.wallet.publicKey
        );
        takerTokenAccountA = await mintA.createAccount(
          provider.wallet.publicKey
        );

        initializerTokenAccountB = await mintB.createAccount(
          provider.wallet.publicKey
        );
        takerTokenAccountB = await mintB.createAccount(
          provider.wallet.publicKey
        );

        await mintA.mintTo(
          initializerTokenAccountA,
          mintAuthority.publicKey,
          [mintAuthority],
          initializerAmount
        );

        await mintB.mintTo(
          takerTokenAccountB,
          mintAuthority.publicKey,
          [mintAuthority],
          takerAmount
        );

        let _initializerTokenAccountA = await mintA.getAccountInfo(
          initializerTokenAccountA
        );
        let _takerTokenAccountB = await mintB.getAccountInfo(
          takerTokenAccountB
        );

        assert.strictEqual(
          _initializerTokenAccountA.amount.toNumber(),
          initializerAmount
        );
        assert.strictEqual(_takerTokenAccountB.amount.toNumber(), takerAmount);
      });

      it("Initialize escrow", async () => {
        await program.rpc.initializeEscrow(
          new BN(initializerAmount),
          new BN(takerAmount),
          {
            accounts: {
              initializer: provider.wallet.publicKey,
              initializerDepositTokenAccount: initializerTokenAccountA,
              initializerReceiveTokenAccount: initializerTokenAccountB,
              escrowAccount: escrowAccount.publicKey,
              systemProgram: SystemProgram.programId,
              tokenProgram: tokenProgramIdA,
            },
            signers: [escrowAccount],
          }
        );

        // Get the PDA that is assigned authority to token account.
        const [_pda, _nonce] = await PublicKey.findProgramAddress(
          [Buffer.from(anchor.utils.bytes.utf8.encode("escrow"))],
          program.programId
        );

        pda = _pda;

        let _initializerTokenAccountA = await mintA.getAccountInfo(
          initializerTokenAccountA
        );

        let _escrowAccount: EscrowAccount =
          await program.account.escrowAccount.fetch(escrowAccount.publicKey);

        // Check that the new owner is the PDA.
        assert.isTrue(_initializerTokenAccountA.owner.equals(pda));

        // Check that the values in the escrow account match what we expect.
        assert.isTrue(
          _escrowAccount.initializerKey.equals(provider.wallet.publicKey)
        );
        assert.strictEqual(
          _escrowAccount.initializerAmount.toNumber(),
          initializerAmount
        );
        assert.strictEqual(_escrowAccount.takerAmount.toNumber(), takerAmount);
        assert.isTrue(
          _escrowAccount.initializerDepositTokenAccount.equals(
            initializerTokenAccountA
          )
        );
        assert.isTrue(
          _escrowAccount.initializerReceiveTokenAccount.equals(
            initializerTokenAccountB
          )
        );
      });

      it("Exchange escrow", async () => {
        await program.rpc.exchange({
          accounts: {
            taker: provider.wallet.publicKey,
            takerDepositTokenAccount: takerTokenAccountB,
            takerReceiveTokenAccount: takerTokenAccountA,
            pdaDepositTokenAccount: initializerTokenAccountA,
            initializerReceiveTokenAccount: initializerTokenAccountB,
            initializerMainAccount: provider.wallet.publicKey,
            escrowAccount: escrowAccount.publicKey,
            pdaAccount: pda,
            depositMint: mintB.publicKey,
            receiveMint: mintA.publicKey,
            depositTokenProgram: tokenProgramIdB,
            receiveTokenProgram: tokenProgramIdA,
          },
        });

        let _takerTokenAccountA = await mintA.getAccountInfo(
          takerTokenAccountA
        );
        let _takerTokenAccountB = await mintB.getAccountInfo(
          takerTokenAccountB
        );
        let _initializerTokenAccountA = await mintA.getAccountInfo(
          initializerTokenAccountA
        );
        let _initializerTokenAccountB = await mintB.getAccountInfo(
          initializerTokenAccountB
        );

        // Check that the initializer gets back ownership of their token account.
        assert.isTrue(
          _takerTokenAccountA.owner.equals(provider.wallet.publicKey)
        );

        assert.strictEqual(
          _takerTokenAccountA.amount.toNumber(),
          initializerAmount
        );
        assert.strictEqual(_initializerTokenAccountA.amount.toNumber(), 0);
        assert.strictEqual(
          _initializerTokenAccountB.amount.toNumber(),
          takerAmount
        );
        assert.strictEqual(_takerTokenAccountB.amount.toNumber(), 0);
      });

      let newEscrow = Keypair.generate();

      it("Initialize escrow and cancel escrow", async () => {
        // Put back tokens into initializer token A account.
        await mintA.mintTo(
          initializerTokenAccountA,
          mintAuthority.publicKey,
          [mintAuthority],
          initializerAmount
        );

        await program.rpc.initializeEscrow(
          new BN(initializerAmount),
          new BN(takerAmount),
          {
            accounts: {
              initializer: provider.wallet.publicKey,
              initializerDepositTokenAccount: initializerTokenAccountA,
              initializerReceiveTokenAccount: initializerTokenAccountB,
              escrowAccount: newEscrow.publicKey,
              systemProgram: SystemProgram.programId,
              tokenProgram: tokenProgramIdA,
            },
            signers: [newEscrow],
          }
        );

        let _initializerTokenAccountA = await mintA.getAccountInfo(
          initializerTokenAccountA
        );

        // Check that the new owner is the PDA.
        assert.isTrue(_initializerTokenAccountA.owner.equals(pda));

        // Cancel the escrow.
        await program.rpc.cancelEscrow({
          accounts: {
            initializer: provider.wallet.publicKey,
            pdaDepositTokenAccount: initializerTokenAccountA,
            pdaAccount: pda,
            escrowAccount: newEscrow.publicKey,
            tokenProgram: tokenProgramIdA,
          },
        });

        // Check the final owner should be the provider public key.
        _initializerTokenAccountA = await mintA.getAccountInfo(
          initializerTokenAccountA
        );
        assert.isTrue(
          _initializerTokenAccountA.owner.equals(provider.wallet.publicKey)
        );

        // Check all the funds are still there.
        assert.strictEqual(
          _initializerTokenAccountA.amount.toNumber(),
          initializerAmount
        );
      });
    });
  });
});
