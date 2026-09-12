const anchor = require("@anchor-lang/core");
const { assert } = require("chai");
const {
  splTokenProgram,
  SPL_TOKEN_PROGRAM_ID,
} = require("@anchor-lang/spl-token");

describe("program", () => {
  const provider = anchor.AnchorProvider.local();

  const TEST_PROGRAM_IDS = [
    SPL_TOKEN_PROGRAM_ID,
    new anchor.web3.PublicKey("TokenzQdBNbLqP5VEhdkAS6EPFLC1PHnBqCXEpPxuEb"),
  ];
  const TOKEN_PROGRAMS = TEST_PROGRAM_IDS.map((programId) =>
    splTokenProgram({
      provider,
      programId,
    })
  );

  // Configure the client to use the local cluster.
  anchor.setProvider(provider);

  const program = anchor.workspace.TokenProxy;

  TOKEN_PROGRAMS.forEach((tokenProgram) => {
    const name = tokenProgram.programId.equals(SPL_TOKEN_PROGRAM_ID)
      ? "token"
      : "token-2022";
    describe(name, () => {
      let mint = null;
      let from = null;
      let to = null;

      it("Initializes test state", async () => {
        mint = await createMint(tokenProgram);
        from = await createTokenAccount(
          tokenProgram,
          mint,
          provider.wallet.publicKey
        );
        to = await createTokenAccount(
          tokenProgram,
          mint,
          provider.wallet.publicKey
        );
      });

      it("Creates a token account", async () => {
        const newMint = await createMint(tokenProgram);
        const authority = provider.wallet.publicKey;
        const [tokenAccount] = anchor.web3.PublicKey.findProgramAddressSync(
          [
            authority.toBytes(),
            newMint.toBytes(),
            Buffer.from("token-proxy-account"),
          ],
          program.programId
        );
        await program.rpc.proxyCreateTokenAccount({
          accounts: {
            authority,
            mint: newMint,
            tokenAccount,
            systemProgram: anchor.web3.SystemProgram.programId,
            tokenProgram: tokenProgram.programId,
          },
        });
        const account = await getTokenAccount(provider, tokenAccount);
        assert.isTrue(account.amount.eq(new anchor.BN(0)));
      });

      it("Creates an associated token account", async () => {
        const newMint = await createMint(tokenProgram);
        const authority = provider.wallet.publicKey;
        const associatedTokenProgram = new anchor.web3.PublicKey(
          "ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL"
        );
        const [tokenAccount] = anchor.web3.PublicKey.findProgramAddressSync(
          [
            authority.toBytes(),
            tokenProgram.programId.toBytes(),
            newMint.toBytes(),
          ],
          associatedTokenProgram
        );

        await program.rpc.proxyCreateAssociatedTokenAccount({
          accounts: {
            tokenAccount,
            mint: newMint,
            authority,
            systemProgram: anchor.web3.SystemProgram.programId,
            tokenProgram: tokenProgram.programId,
            associatedTokenProgram,
          },
        });
        const account = await getTokenAccount(provider, tokenAccount);
        assert.isTrue(account.amount.eq(new anchor.BN(0)));
      });

      it("Creates a mint", async () => {
        const authority = provider.wallet.publicKey;
        const [newMint] = anchor.web3.PublicKey.findProgramAddressSync(
          [
            authority.toBytes(),
            Buffer.from(name),
            Buffer.from("token-proxy-mint"),
          ],
          program.programId
        );
        await program.rpc.proxyCreateMint(name, {
          accounts: {
            authority,
            mint: newMint,
            systemProgram: anchor.web3.SystemProgram.programId,
            tokenProgram: tokenProgram.programId,
          },
        });
      });

      it("Mints a token", async () => {
        await program.rpc.proxyMintTo(new anchor.BN(1000), {
          accounts: {
            authority: provider.wallet.publicKey,
            mint,
            to: from,
            tokenProgram: tokenProgram.programId,
          },
        });

        const fromAccount = await getTokenAccount(provider, from);

        assert.isTrue(fromAccount.amount.eq(new anchor.BN(1000)));
      });

      it("Transfers a token", async () => {
        const preFromAccount = await getTokenAccount(provider, from);
        const preToAccount = await getTokenAccount(provider, to);

        const transferAmount = new anchor.BN(400);

        await program.rpc.proxyTransfer(transferAmount, {
          accounts: {
            authority: provider.wallet.publicKey,
            to,
            from,
            tokenProgram: tokenProgram.programId,
          },
        });

        const postFromAccount = await getTokenAccount(provider, from);
        const postToAccount = await getTokenAccount(provider, to);

        assert.isTrue(
          postFromAccount.amount.eq(preFromAccount.amount.sub(transferAmount))
        );
        assert.isTrue(
          postToAccount.amount.eq(preToAccount.amount.add(transferAmount))
        );
      });

      it("Transfers a token with optional accounts", async () => {
        const preFromAccount = await getTokenAccount(provider, from);
        const preToAccount = await getTokenAccount(provider, to);

        const transferAmount = new anchor.BN(10);

        await program.rpc.proxyOptionalTransfer(transferAmount, {
          accounts: {
            authority: provider.wallet.publicKey,
            to,
            from,
            mint,
            tokenProgram: tokenProgram.programId,
          },
        });

        const postFromAccount = await getTokenAccount(provider, from);
        const postToAccount = await getTokenAccount(provider, to);

        assert.isTrue(
          postFromAccount.amount.eq(preFromAccount.amount.sub(transferAmount))
        );
        assert.isTrue(
          postToAccount.amount.eq(preToAccount.amount.add(transferAmount))
        );
      });

      it("Does not transfer a token without optional accounts", async () => {
        const preFromAccount = await getTokenAccount(provider, from);
        const preToAccount = await getTokenAccount(provider, to);

        const optionalTransferIx = await program.methods
          .proxyOptionalTransfer(new anchor.BN(10))
          .accounts({
            authority: provider.wallet.publicKey,
            to,
            from,
            mint: null,
            tokenProgram: null,
          })
          .instruction();
        const tx = new anchor.web3.Transaction().add(optionalTransferIx);
        await provider.sendAndConfirm(tx);

        const postFromAccount = await getTokenAccount(provider, from);
        const postToAccount = await getTokenAccount(provider, to);

        assert.isTrue(postFromAccount.amount.eq(preFromAccount.amount));
        assert.isTrue(postToAccount.amount.eq(preToAccount.amount));
      });

      it("Burns a token", async () => {
        const preAccount = await getTokenAccount(provider, to);
        const burnAmount = new anchor.BN(300);
        await program.rpc.proxyBurn(burnAmount, {
          accounts: {
            authority: provider.wallet.publicKey,
            mint,
            from: to,
            tokenProgram: tokenProgram.programId,
          },
        });

        const postAccount = await getTokenAccount(provider, to);
        assert.isTrue(postAccount.amount.eq(preAccount.amount.sub(burnAmount)));
      });

      it("Set new mint authority", async () => {
        const newMintAuthority = anchor.web3.Keypair.generate();
        await program.rpc.proxySetAuthority(
          { mintTokens: {} },
          newMintAuthority.publicKey,
          {
            accounts: {
              accountOrMint: mint,
              currentAuthority: provider.wallet.publicKey,
              tokenProgram: tokenProgram.programId,
            },
          }
        );

        const mintInfo = await getMintInfo(provider, mint);
        assert.isTrue(
          mintInfo.mintAuthority.equals(newMintAuthority.publicKey)
        );
      });
    });
  });
});

// SPL token client boilerplate for test initialization. Everything below here is
// mostly irrelevant to the point of the example.

const serumCmn = require("@project-serum/common");

async function getTokenAccount(provider, addr) {
  return await serumCmn.getTokenAccount(provider, addr);
}

async function getMintInfo(provider, mintAddr) {
  return await serumCmn.getMintInfo(provider, mintAddr);
}

async function createMint(tokenProgram) {
  const mint = anchor.web3.Keypair.generate();
  const authority = tokenProgram.provider.wallet.publicKey;
  const createMintIx = await tokenProgram.account.mint.createInstruction(mint);
  const initMintIx = await tokenProgram.methods
    .initializeMint2(0, authority, null)
    .accounts({ mint: mint.publicKey })
    .instruction();

  const tx = new anchor.web3.Transaction();
  tx.add(createMintIx, initMintIx);

  await tokenProgram.provider.sendAndConfirm(tx, [mint]);

  return mint.publicKey;
}

async function createTokenAccount(tokenProgram, mint, owner) {
  const vault = anchor.web3.Keypair.generate();
  const tx = new anchor.web3.Transaction();
  const createTokenAccountIx =
    await tokenProgram.account.account.createInstruction(vault);
  const initTokenAccountIx = await tokenProgram.methods
    .initializeAccount3(owner)
    .accounts({ account: vault.publicKey, mint })
    .instruction();
  tx.add(createTokenAccountIx, initTokenAccountIx);
  await tokenProgram.provider.sendAndConfirm(tx, [vault]);
  return vault.publicKey;
}
