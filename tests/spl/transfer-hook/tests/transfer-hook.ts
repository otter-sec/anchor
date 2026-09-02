import * as anchor from "@anchor-lang/core";
import { Program } from "@anchor-lang/core";
import {
  PublicKey,
  Keypair,
  SystemProgram,
  sendAndConfirmTransaction,
  Transaction,
  AccountInfo,
} from "@solana/web3.js";
import {
  getExtraAccountMetaAddress,
  ExtraAccountMeta,
  getMintLen,
  ExtensionType,
  createInitializeTransferHookInstruction,
  createInitializeMintInstruction,
  createAssociatedTokenAccountInstruction,
  getAssociatedTokenAddressSync,
  createMintToInstruction,
  createTransferCheckedInstruction,
  getAccount,
  addExtraAccountsToInstruction,
} from "@solana/spl-token";
import { assert } from "chai";
import { TransferHook } from "../target/types/transfer_hook";

describe("transfer hook", () => {
  const provider = anchor.AnchorProvider.env();
  anchor.setProvider(provider);

  const TOKEN_2022_PROGRAM_ID = new anchor.web3.PublicKey(
    "TokenzQdBNbLqP5VEhdkAS6EPFLC1PHnBqCXEpPxuEb"
  );
  const program = anchor.workspace.transferHook as Program<TransferHook>;

  const decimals = 2;
  const mintAmount = 100;
  const transferAmount = 10;

  const payer = Keypair.generate();
  const mintAuthority = Keypair.generate();
  const mint = Keypair.generate();

  const sourceAuthority = Keypair.generate();
  const destinationAuthority = Keypair.generate().publicKey;
  let source: PublicKey = null;
  let destination: PublicKey = null;

  let extraMetasAddress: PublicKey = null;
  const validationLen = 8 + 4 + 4 + 2 * 35; // Discriminator, length, pod slice length, pod slice with 2 extra metas
  const extraMetas: ExtraAccountMeta[] = [
    {
      discriminator: 0,
      addressConfig: Keypair.generate().publicKey.toBuffer(),
      isWritable: false,
      isSigner: false,
    },
    {
      discriminator: 0,
      addressConfig: Keypair.generate().publicKey.toBuffer(),
      isWritable: false,
      isSigner: false,
    },
  ];

  before(async () => {
    const { programId } = program;
    const extensions = [ExtensionType.TransferHook];
    const mintLen = getMintLen(extensions);
    const lamports =
      await provider.connection.getMinimumBalanceForRentExemption(mintLen);

    source = getAssociatedTokenAddressSync(
      mint.publicKey,
      sourceAuthority.publicKey,
      false,
      TOKEN_2022_PROGRAM_ID
    );
    destination = getAssociatedTokenAddressSync(
      mint.publicKey,
      destinationAuthority,
      false,
      TOKEN_2022_PROGRAM_ID
    );

    extraMetasAddress = getExtraAccountMetaAddress(mint.publicKey, programId);

    const transaction = new Transaction().add(
      SystemProgram.createAccount({
        fromPubkey: payer.publicKey,
        newAccountPubkey: mint.publicKey,
        space: mintLen,
        lamports,
        programId: TOKEN_2022_PROGRAM_ID,
      }),
      createInitializeTransferHookInstruction(
        mint.publicKey,
        mintAuthority.publicKey,
        programId,
        TOKEN_2022_PROGRAM_ID
      ),
      createInitializeMintInstruction(
        mint.publicKey,
        decimals,
        mintAuthority.publicKey,
        mintAuthority.publicKey,
        TOKEN_2022_PROGRAM_ID
      ),
      createAssociatedTokenAccountInstruction(
        payer.publicKey,
        source,
        sourceAuthority.publicKey,
        mint.publicKey,
        TOKEN_2022_PROGRAM_ID
      ),
      createAssociatedTokenAccountInstruction(
        payer.publicKey,
        destination,
        destinationAuthority,
        mint.publicKey,
        TOKEN_2022_PROGRAM_ID
      ),
      createMintToInstruction(
        mint.publicKey,
        source,
        mintAuthority.publicKey,
        mintAmount,
        [],
        TOKEN_2022_PROGRAM_ID
      )
    );

    await provider.connection.confirmTransaction(
      await provider.connection.requestAirdrop(payer.publicKey, 10000000000),
      "confirmed"
    );

    await sendAndConfirmTransaction(
      provider.connection,
      transaction,
      [payer, mint, mintAuthority],
      { maxRetries: 3, skipPreflight: true }
    );
  });

  it("can create an `InitializeExtraAccountMetaList` instruction with the proper discriminator", async () => {
    const ix = await program.methods
      .initialize(extraMetas as any[])
      .accounts({
        extraMetasAccount: extraMetasAddress,
        mint: mint.publicKey,
        mintAuthority: mintAuthority.publicKey,
        systemProgram: SystemProgram.programId,
      })
      .instruction();
    assert.equal(
      ix.data.subarray(0, 8).compare(
        Buffer.from([43, 34, 13, 49, 167, 88, 235, 235]) // SPL discriminator for `InitializeExtraAccountMetaList` from interface
      ),
      0
    );
    const { name, data } = new anchor.BorshInstructionCoder(program.idl).decode(
      ix.data,
      "hex"
    );
    assert.equal(name, "initialize");
    assert.property(data, "metas");
    assert.isArray(data.metas);
    assert.equal(data.metas.length, extraMetas.length);
  });

  it("can create an `Execute` instruction with the proper discriminator", async () => {
    const ix = await program.methods
      .execute(new anchor.BN(transferAmount))
      .accounts({
        sourceAccount: source,
        mint: mint.publicKey,
        destinationAccount: destination,
        ownerDelegate: sourceAuthority.publicKey,
        extraMetasAccount: extraMetasAddress,
        secondaryAuthority1: new PublicKey(extraMetas[0].addressConfig),
        secondaryAuthority2: new PublicKey(extraMetas[1].addressConfig),
      })
      .instruction();
    assert.equal(
      ix.data.subarray(0, 8).compare(
        Buffer.from([105, 37, 101, 197, 75, 251, 102, 26]) // SPL discriminator for `Execute` from interface
      ),
      0
    );
    const { name, data } = new anchor.BorshInstructionCoder(program.idl).decode(
      ix.data,
      "hex"
    );
    assert.equal(name, "execute");
    assert.property(data, "amount");
    assert.isTrue(anchor.BN.isBN(data.amount));
    assert.isTrue(data.amount.eq(new anchor.BN(transferAmount)));
  });

  it("can transfer with extra account metas", async () => {
    // Initialize the extra metas
    await program.methods
      .initialize(extraMetas as any[])
      .accounts({
        extraMetasAccount: extraMetasAddress,
        mint: mint.publicKey,
        mintAuthority: mintAuthority.publicKey,
        systemProgram: SystemProgram.programId,
      })
      .signers([mintAuthority])
      .rpc();

    // Check the account data
    await provider.connection
      .getAccountInfo(extraMetasAddress)
      .then((account: AccountInfo<Buffer>) => {
        assert.equal(account.data.length, validationLen);
        assert.equal(
          account.data.subarray(0, 8).compare(
            Buffer.from([105, 37, 101, 197, 75, 251, 102, 26]) // SPL discriminator for `Execute` from interface
          ),
          0
        );
        assert.equal(
          account.data.subarray(8, 12).compare(
            Buffer.from([74, 0, 0, 0]) // Little endian 74
          ),
          0
        );
        assert.equal(
          account.data.subarray(12, 16).compare(
            Buffer.from([2, 0, 0, 0]) // Little endian 2
          ),
          0
        );
        const extraMetaToBuffer = (extraMeta: ExtraAccountMeta) => {
          const buf = Buffer.alloc(35);
          buf.set(extraMeta.addressConfig, 1);
          buf.writeUInt8(0, 33); // isSigner
          buf.writeUInt8(0, 34); // isWritable
          return buf;
        };
        assert.equal(
          account.data
            .subarray(16, 51)
            .compare(extraMetaToBuffer(extraMetas[0])),
          0
        );
        assert.equal(
          account.data
            .subarray(51, 86)
            .compare(extraMetaToBuffer(extraMetas[1])),
          0
        );
      });

    const ix = await addExtraAccountsToInstruction(
      provider.connection,
      createTransferCheckedInstruction(
        source,
        mint.publicKey,
        destination,
        sourceAuthority.publicKey,
        transferAmount,
        decimals,
        undefined,
        TOKEN_2022_PROGRAM_ID
      ),
      mint.publicKey,
      undefined,
      TOKEN_2022_PROGRAM_ID
    );

    await sendAndConfirmTransaction(
      provider.connection,
      new Transaction().add(ix),
      [payer, sourceAuthority],
      { maxRetries: 3, skipPreflight: true }
    );

    // Check the resulting token balances
    await getAccount(
      provider.connection,
      source,
      undefined,
      TOKEN_2022_PROGRAM_ID
    ).then((account) => {
      assert.equal(account.amount, BigInt(mintAmount - transferAmount));
    });
    await getAccount(
      provider.connection,
      destination,
      undefined,
      TOKEN_2022_PROGRAM_ID
    ).then((account) => {
      assert.equal(account.amount, BigInt(transferAmount));
    });
  });
});
