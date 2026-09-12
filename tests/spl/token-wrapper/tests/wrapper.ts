import * as anchor from "@anchor-lang/core";
import { Program, BN, IdlAccounts } from "@anchor-lang/core";
import { PublicKey, Keypair, SystemProgram } from "@solana/web3.js";
import { TOKEN_PROGRAM_ID, Token } from "@solana/spl-token";
import { assert } from "chai";
import { TokenWrapper } from "../target/types/token_wrapper";

describe("wrapper", () => {
  const provider = anchor.AnchorProvider.env();
  anchor.setProvider(provider);

  const TOKEN_2022_PROGRAM_ID = new anchor.web3.PublicKey(
    "TokenzQdBNbLqP5VEhdkAS6EPFLC1PHnBqCXEpPxuEb"
  );
  const TEST_TOKEN_PROGRAM_IDS = [
    [TOKEN_PROGRAM_ID, TOKEN_PROGRAM_ID],
    [TOKEN_2022_PROGRAM_ID, TOKEN_2022_PROGRAM_ID],
    [TOKEN_PROGRAM_ID, TOKEN_2022_PROGRAM_ID],
    [TOKEN_2022_PROGRAM_ID, TOKEN_PROGRAM_ID],
  ];
  const program = anchor.workspace.TokenWrapper as Program<TokenWrapper>;

  let depositMint: Token = null;
  let wrappedMint: Token = null;
  let initializerDepositTokenAccount: PublicKey = null;
  let userWrappedTokenAccount: PublicKey = null;
  let userDepositTokenAccount: PublicKey = null;
  let depositTokenVault: PublicKey = null;
  let wrapperAuthority: PublicKey = null;

  const wrapAmount = 1000;
  const initializerAmount = 500;

  const payer = Keypair.generate();
  const mintAuthority = Keypair.generate();

  TEST_TOKEN_PROGRAM_IDS.forEach((tokenProgramIds) => {
    const [depositTokenProgram, wrappedTokenProgram] = tokenProgramIds;
    let name;
    if (depositTokenProgram === wrappedTokenProgram) {
      name = wrappedTokenProgram === TOKEN_PROGRAM_ID ? "token" : "token-2022";
    } else {
      name =
        wrappedTokenProgram === TOKEN_PROGRAM_ID
          ? "mixed-wrapped-token"
          : "mixed-wrapped-token-2022";
    }
    describe(name, () => {
      it("Initialise wrapper", async () => {
        // Airdropping tokens to a payer.
        await provider.connection.confirmTransaction(
          await provider.connection.requestAirdrop(
            payer.publicKey,
            10000000000
          ),
          "confirmed"
        );

        depositMint = await Token.createMint(
          provider.connection,
          payer,
          mintAuthority.publicKey,
          null,
          6,
          depositTokenProgram
        );

        initializerDepositTokenAccount = await depositMint.createAccount(
          provider.wallet.publicKey
        );

        await depositMint.mintTo(
          initializerDepositTokenAccount,
          mintAuthority.publicKey,
          [mintAuthority],
          initializerAmount
        );

        const wrappedMintKP = new Keypair();
        const initializerWrappedTokenAccountKP = new Keypair();

        // Get the PDA that is assigned authority to the deposit vault account
        const [_wrapperAuthority, _] = PublicKey.findProgramAddressSync(
          [
            Buffer.from(anchor.utils.bytes.utf8.encode("wrapr")),
            depositMint.publicKey.toBuffer(),
            wrappedMintKP.publicKey.toBuffer(),
          ],
          program.programId
        );
        wrapperAuthority = _wrapperAuthority;

        // Get the deposit vault account PDA
        const [_depositTokenVault, __] = PublicKey.findProgramAddressSync(
          [
            Buffer.from(anchor.utils.bytes.utf8.encode("vault")),
            depositMint.publicKey.toBuffer(),
            wrappedMintKP.publicKey.toBuffer(),
          ],
          program.programId
        );
        depositTokenVault = _depositTokenVault;

        await program.rpc.initialize(new BN(initializerAmount), {
          accounts: {
            initializer: provider.wallet.publicKey,
            depositMint: depositMint.publicKey,
            wrappedMint: wrappedMintKP.publicKey,
            depositTokenVault,
            initializerDepositTokenAccount,
            initializerWrappedTokenAccount:
              initializerWrappedTokenAccountKP.publicKey,
            wrapperAuthority,
            systemProgram: SystemProgram.programId,
            depositTokenProgram,
            wrappedTokenProgram,
          },
          signers: [wrappedMintKP, initializerWrappedTokenAccountKP],
        });

        wrappedMint = new Token(
          provider.connection,
          wrappedMintKP.publicKey,
          wrappedTokenProgram,
          payer
        );

        let _initializerDepositTokenAccount = await depositMint.getAccountInfo(
          initializerDepositTokenAccount
        );
        let _initializerWrappedTokenAccount = await wrappedMint.getAccountInfo(
          initializerWrappedTokenAccountKP.publicKey
        );

        assert.strictEqual(
          _initializerDepositTokenAccount.amount.toNumber(),
          0
        );
        assert.strictEqual(
          _initializerWrappedTokenAccount.amount.toNumber(),
          initializerAmount
        );
      });

      it("Wrap", async () => {
        userDepositTokenAccount = await depositMint.createAccount(
          provider.wallet.publicKey
        );

        userWrappedTokenAccount = await wrappedMint.createAccount(
          provider.wallet.publicKey
        );

        await depositMint.mintTo(
          userDepositTokenAccount,
          mintAuthority.publicKey,
          [mintAuthority],
          wrapAmount
        );

        await program.rpc.wrap(new BN(wrapAmount), {
          accounts: {
            signer: provider.wallet.publicKey,
            depositMint: depositMint.publicKey,
            wrappedMint: wrappedMint.publicKey,
            depositTokenVault: depositTokenVault,
            userDepositTokenAccount,
            userWrappedTokenAccount,
            wrapperAuthority,
            depositTokenProgram,
            wrappedTokenProgram,
          },
        });

        let _userDepositTokenAccount = await depositMint.getAccountInfo(
          userDepositTokenAccount
        );
        let _userWrappedTokenAccount = await wrappedMint.getAccountInfo(
          userWrappedTokenAccount
        );

        assert.strictEqual(_userDepositTokenAccount.amount.toNumber(), 0);
        assert.strictEqual(
          _userWrappedTokenAccount.amount.toNumber(),
          wrapAmount
        );
      });

      it("Unwrap", async () => {
        await program.rpc.unwrap(new BN(wrapAmount - 1), {
          accounts: {
            signer: provider.wallet.publicKey,
            depositMint: depositMint.publicKey,
            wrappedMint: wrappedMint.publicKey,
            depositTokenVault: depositTokenVault,
            userDepositTokenAccount,
            userWrappedTokenAccount,
            wrapperAuthority,
            depositTokenProgram,
            wrappedTokenProgram,
          },
        });

        let _userDepositTokenAccount = await depositMint.getAccountInfo(
          userDepositTokenAccount
        );
        let _userWrappedTokenAccount = await wrappedMint.getAccountInfo(
          userWrappedTokenAccount
        );

        assert.strictEqual(
          _userDepositTokenAccount.amount.toNumber(),
          wrapAmount - 1
        );
        assert.strictEqual(_userWrappedTokenAccount.amount.toNumber(), 1);
      });
    });
  });
});
