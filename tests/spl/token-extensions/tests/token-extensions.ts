import * as anchor from "@anchor-lang/core";
import { AnchorError, Program } from "@anchor-lang/core";
import { strict as assert } from "node:assert";
import { PublicKey, Keypair } from "@solana/web3.js";
import { TokenExtensions } from "../target/types/token_extensions";
import { ASSOCIATED_PROGRAM_ID } from "@anchor-lang/core/dist/cjs/utils/token";
import {
  createInitializeMintInstruction,
  createMint,
  ExtensionType,
  getExtensionTypes,
  getMint,
  getMintLen,
  NATIVE_MINT_2022,
} from "@solana/spl-token";

const TOKEN_2022_PROGRAM_ID = new anchor.web3.PublicKey(
  "TokenzQdBNbLqP5VEhdkAS6EPFLC1PHnBqCXEpPxuEb"
);

export function associatedAddress({
  mint,
  owner,
}: {
  mint: PublicKey;
  owner: PublicKey;
}): PublicKey {
  return PublicKey.findProgramAddressSync(
    [owner.toBuffer(), TOKEN_2022_PROGRAM_ID.toBuffer(), mint.toBuffer()],
    ASSOCIATED_PROGRAM_ID
  )[0];
}

describe("token extensions", () => {
  const provider = anchor.AnchorProvider.env();
  anchor.setProvider(provider);

  const program = anchor.workspace.TokenExtensions as Program<TokenExtensions>;

  const payer = Keypair.generate();

  it("airdrop payer", async () => {
    await provider.connection.confirmTransaction(
      await provider.connection.requestAirdrop(payer.publicKey, 10000000000),
      "confirmed"
    );
  });

  let mint = new Keypair();

  it("Create mint account test passes", async () => {
    const [extraMetasAccount] = PublicKey.findProgramAddressSync(
      [
        anchor.utils.bytes.utf8.encode("extra-account-metas"),
        mint.publicKey.toBuffer(),
      ],
      program.programId
    );
    await program.methods
      .createMintAccount({
        name: "hello",
        symbol: "hi",
        uri: "https://hi.com",
      })
      .accountsStrict({
        payer: payer.publicKey,
        authority: payer.publicKey,
        receiver: payer.publicKey,
        mint: mint.publicKey,
        mintTokenAccount: associatedAddress({
          mint: mint.publicKey,
          owner: payer.publicKey,
        }),
        extraMetasAccount: extraMetasAccount,
        systemProgram: anchor.web3.SystemProgram.programId,
        associatedTokenProgram: ASSOCIATED_PROGRAM_ID,
        tokenProgram: TOKEN_2022_PROGRAM_ID,
      })
      .signers([mint, payer])
      .rpc();
  });

  it("mint extension constraints test passes", async () => {
    await program.methods
      .checkMintExtensionsConstraints()
      .accountsStrict({
        authority: payer.publicKey,
        mint: mint.publicKey,
      })
      .signers([payer])
      .rpc();
  });

  describe("group_pointer_update", () => {
    let groupPointerMint = new Keypair();

    it("Create mint with group pointer extension", async () => {
      await program.methods
        .createGroupPointerMint()
        .accountsStrict({
          payer: payer.publicKey,
          authority: payer.publicKey,
          mint: groupPointerMint.publicKey,
          systemProgram: anchor.web3.SystemProgram.programId,
          tokenProgram: TOKEN_2022_PROGRAM_ID,
        })
        .signers([payer, groupPointerMint])
        .rpc();
    });

    it("Update group pointer via CPI succeeds", async () => {
      const newGroupAddress = Keypair.generate().publicKey;
      await program.methods
        .updateGroupPointer(newGroupAddress)
        .accountsStrict({
          authority: payer.publicKey,
          mint: groupPointerMint.publicKey,
          tokenProgram: TOKEN_2022_PROGRAM_ID,
        })
        .signers([payer])
        .rpc();
    });

    it("Update group pointer to None via CPI succeeds", async () => {
      await program.methods
        .updateGroupPointer(null)
        .accountsStrict({
          authority: payer.publicKey,
          mint: groupPointerMint.publicKey,
          tokenProgram: TOKEN_2022_PROGRAM_ID,
        })
        .signers([payer])
        .rpc();
    });
  });

  it("pausable toggle test passes", async () => {
    await program.methods
      .checkTogglePause()
      .accountsStrict({
        authority: payer.publicKey,
        mint: mint.publicKey,
        tokenProgram: TOKEN_2022_PROGRAM_ID,
      })
      .signers([payer])
      .rpc();
  });

  it("pausable authority constraint fails on mismatched authority", async () => {
    const wrongAuthority = Keypair.generate();
    await provider.connection.confirmTransaction(
      await provider.connection.requestAirdrop(
        wrongAuthority.publicKey,
        1000000000
      ),
      "confirmed"
    );

    try {
      await program.methods
        .checkPausableAuthorityConstraint()
        .accountsStrict({
          authority: wrongAuthority.publicKey,
          mint: mint.publicKey,
        })
        .signers([wrongAuthority])
        .rpc();
      assert.fail("expected ConstraintMintPausableAuthority");
    } catch (err) {
      assert.ok(err instanceof AnchorError);
      assert.equal(
        (err as AnchorError).error.errorCode.code,
        "ConstraintMintPausableAuthority"
      );
      assert.equal((err as AnchorError).error.errorCode.number, 2044);
    }
  });

  async function assertNativeMintState() {
    const accountInfo = await provider.connection.getAccountInfo(
      NATIVE_MINT_2022,
      "confirmed"
    );
    assert.ok(accountInfo !== null, "native mint must exist after CPI");
    assert.ok(
      accountInfo.owner.equals(TOKEN_2022_PROGRAM_ID),
      "native mint must be owned by Token-2022"
    );
    assert.ok(accountInfo.lamports > 0, "native mint must be rent-funded");

    const mintAccount = await getMint(
      provider.connection,
      NATIVE_MINT_2022,
      "confirmed",
      TOKEN_2022_PROGRAM_ID
    );
    assert.equal(mintAccount.decimals, 9);
    assert.equal(mintAccount.mintAuthority, null);
    assert.equal(mintAccount.freezeAuthority, null);
  }

  describe("create_native_mint", () => {
    it("Creates the Token-2022 native mint via CPI", async () => {
      try {
        await program.methods
          .cpiCreateNativeMint()
          .accountsStrict({
            payer: payer.publicKey,
            nativeMint: NATIVE_MINT_2022,
            systemProgram: anchor.web3.SystemProgram.programId,
            tokenProgram: TOKEN_2022_PROGRAM_ID,
          })
          .signers([payer])
          .rpc();
      } catch (err) {
        // The canonical native mint can only be created once per cluster; on a
        // rerun the System Program will reject the create_account with
        // "account already in use". Fall through and still verify state below.
        const msg = (err as Error).message ?? "";
        if (!/already in use/i.test(msg)) throw err;
      }

      await assertNativeMintState();
    });

    it("Fails when the system_program account is wrong", async () => {
      await assert.rejects(
        program.methods
          .cpiCreateNativeMint()
          .accountsStrict({
            payer: payer.publicKey,
            nativeMint: NATIVE_MINT_2022,
            // Token-2022's create_native_mint expects the real System Program;
            // passing anything else must fail.
            systemProgram: TOKEN_2022_PROGRAM_ID,
            tokenProgram: TOKEN_2022_PROGRAM_ID,
          })
          .signers([payer])
          .rpc()
      );
    });
  });

  describe("initialize_non_transferable_mint", () => {
    async function allocateMint(
      mintKeypair: Keypair,
      extensions: ExtensionType[]
    ) {
      const mintLen = getMintLen(extensions);
      const lamports =
        await provider.connection.getMinimumBalanceForRentExemption(mintLen);
      const tx = new anchor.web3.Transaction().add(
        anchor.web3.SystemProgram.createAccount({
          fromPubkey: payer.publicKey,
          newAccountPubkey: mintKeypair.publicKey,
          space: mintLen,
          lamports,
          programId: TOKEN_2022_PROGRAM_ID,
        })
      );
      await anchor.web3.sendAndConfirmTransaction(
        provider.connection,
        tx,
        [payer, mintKeypair],
        { commitment: "confirmed" }
      );
    }

    it("Initializes the non-transferable extension on a new mint via CPI", async () => {
      const mintKeypair = Keypair.generate();
      await allocateMint(mintKeypair, [ExtensionType.NonTransferable]);

      await program.methods
        .cpiInitializeNonTransferableMint()
        .accountsStrict({
          mint: mintKeypair.publicKey,
          tokenProgram: TOKEN_2022_PROGRAM_ID,
        })
        .postInstructions([
          createInitializeMintInstruction(
            mintKeypair.publicKey,
            0,
            payer.publicKey,
            null,
            TOKEN_2022_PROGRAM_ID
          ),
        ])
        .rpc();

      const mintAccount = await getMint(
        provider.connection,
        mintKeypair.publicKey,
        "confirmed",
        TOKEN_2022_PROGRAM_ID
      );
      const extensions = getExtensionTypes(mintAccount.tlvData);
      assert.ok(
        extensions.includes(ExtensionType.NonTransferable),
        "mint should have the NonTransferable extension"
      );
    });

    it("Fails when the mint account is too small for the extension", async () => {
      const mintKeypair = Keypair.generate();
      // Allocate only the base mint size — no room for the NonTransferable TLV.
      await allocateMint(mintKeypair, []);

      await assert.rejects(
        program.methods
          .cpiInitializeNonTransferableMint()
          .accountsStrict({
            mint: mintKeypair.publicKey,
            tokenProgram: TOKEN_2022_PROGRAM_ID,
          })
          .rpc()
      );
    });
  });

  it("pausable authority constraint fails when mint has no pausable extension", async () => {
    const plainMint = await createMint(
      provider.connection,
      payer,
      payer.publicKey,
      null,
      9,
      Keypair.generate(),
      { commitment: "confirmed" },
      TOKEN_2022_PROGRAM_ID
    );

    try {
      await program.methods
        .checkPausableAuthorityConstraint()
        .accountsStrict({
          authority: payer.publicKey,
          mint: plainMint,
        })
        .signers([payer])
        .rpc();
      assert.fail("expected ConstraintMintPausableExtension");
    } catch (err) {
      assert.ok(err instanceof AnchorError);
      assert.equal(
        (err as AnchorError).error.errorCode.code,
        "ConstraintMintPausableExtension"
      );
      assert.equal((err as AnchorError).error.errorCode.number, 2043);
    }
  });

  it("mint metadata update and remove test passes", async () => {
    //update_and_remove_token_metadata
    await program.methods
      .updateAndRemoveTokenMetadata()
      .accountsStrict({
        authority: payer.publicKey,
        mint: mint.publicKey,
        tokenProgram: TOKEN_2022_PROGRAM_ID,
      })
      .signers([payer])
      .rpc();
  });
});
