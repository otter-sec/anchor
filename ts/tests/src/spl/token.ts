import assert from "assert";
import { splTokenProgram } from "@anchor-lang/spl-token";
import { Keypair, PublicKey, SYSVAR_RENT_PUBKEY } from "@solana/web3.js";
import { BN } from "@anchor-lang/core";

import { NATIVE_MINT_PK, SPL_TOKEN_PROGRAM_ID } from "../constants";
import {
  createTokenAccount,
  getProvider,
  loadKp,
  sendAndConfirmTx,
  test,
} from "../utils";

export async function splTokenTests() {
  const provider = await getProvider();
  const program = splTokenProgram({
    provider,
    programId: SPL_TOKEN_PROGRAM_ID,
  });
  const kp = await loadKp();
  const delegateKp = new Keypair();

  const DECIMALS = 6;
  const MULTISIG_COUNT = 2;
  let mintPk: PublicKey;
  let tokenAccountPk: PublicKey;
  let multisigPk: PublicKey;

  async function initializeMint() {
    const mintKp = new Keypair();
    mintPk = mintKp.publicKey;
    const createMintAccountIx = await program.account.mint.createInstruction(
      mintKp
    );
    const initMintIx = await program.methods
      .initializeMint(DECIMALS, kp.publicKey, kp.publicKey)
      .accounts({
        mint: mintPk,
        rent: SYSVAR_RENT_PUBKEY,
      })
      .instruction();

    await sendAndConfirmTx([createMintAccountIx, initMintIx], [kp, mintKp]);
  }

  async function initializeAccount() {
    const accountKp = new Keypair();
    tokenAccountPk = accountKp.publicKey;
    const createTokenAccountIx =
      await program.account.account.createInstruction(accountKp);
    const initAccountIx = await program.methods
      .initializeAccount()
      .accounts({
        account: tokenAccountPk,
        mint: mintPk,
        owner: kp.publicKey,
        rent: SYSVAR_RENT_PUBKEY,
      })
      .instruction();

    await sendAndConfirmTx(
      [createTokenAccountIx, initAccountIx],
      [kp, accountKp]
    );
  }

  async function initializeMultisig() {
    const multisigKp = new Keypair();
    multisigPk = multisigKp.publicKey;
    const multisig1 = new Keypair();
    const createTokenAccountIx =
      await program.account.multisig.createInstruction(multisigKp);
    const initAccountIx = await program.methods
      .initializeMultisig(MULTISIG_COUNT)
      .accounts({
        multisig: multisigKp.publicKey,
        rent: SYSVAR_RENT_PUBKEY,
      })
      .remainingAccounts([
        { isSigner: true, isWritable: true, pubkey: kp.publicKey },
        { isSigner: true, isWritable: true, pubkey: multisig1.publicKey },
      ])
      .instruction();

    await sendAndConfirmTx(
      [createTokenAccountIx, initAccountIx],
      [kp, multisigKp, multisig1]
    );
  }

  async function mintTo() {
    await program.methods
      .mintTo(new BN(2 * 10 ** DECIMALS))
      .accounts({
        account: tokenAccountPk,
        mint: mintPk,
        owner: kp.publicKey,
      })
      .rpc();
  }

  async function mintToChecked() {
    await program.methods
      .mintToChecked(new BN(2 * 10 ** DECIMALS), DECIMALS)
      .accounts({
        account: tokenAccountPk,
        mint: mintPk,
        owner: kp.publicKey,
      })
      .rpc();
  }

  async function burn() {
    await program.methods
      .burn(new BN(1 * 10 ** DECIMALS))
      .accounts({
        account: tokenAccountPk,
        authority: kp.publicKey,
        mint: mintPk,
      })
      .rpc();
  }

  async function burnChecked() {
    await program.methods
      .burnChecked(new BN(1 * 10 ** DECIMALS), DECIMALS)
      .accounts({
        account: tokenAccountPk,
        authority: kp.publicKey,
        mint: mintPk,
      })
      .rpc();
  }

  async function transfer() {
    await program.methods
      .transfer(new BN(1 * 10 ** DECIMALS))
      .accounts({
        authority: kp.publicKey,
        destination: await createTokenAccount(mintPk),
        source: tokenAccountPk,
      })
      .rpc();
  }

  async function transferChecked() {
    await program.methods
      .transferChecked(new BN(1 * 10 ** DECIMALS), DECIMALS)
      .accounts({
        authority: kp.publicKey,
        destination: await createTokenAccount(mintPk),
        mint: mintPk,
        source: tokenAccountPk,
      })
      .rpc();
  }

  async function approve() {
    await program.methods
      .approve(new BN(1 * 10 ** DECIMALS))
      .accounts({
        delegate: delegateKp.publicKey,
        owner: kp.publicKey,
        source: tokenAccountPk,
      })
      .rpc();
  }

  async function approveChecked() {
    await program.methods
      .approveChecked(new BN(1 * 10 ** DECIMALS), DECIMALS)
      .accounts({
        delegate: delegateKp.publicKey,
        mint: mintPk,
        owner: kp.publicKey,
        source: tokenAccountPk,
      })
      .rpc();
  }

  async function revoke() {
    await program.methods
      .revoke()
      .accounts({
        owner: kp.publicKey,
        source: tokenAccountPk,
      })
      .rpc();
  }

  async function setAuthority() {
    await program.methods
      .setAuthority({ mintTokens: {} }, null)
      .accounts({
        owned: mintPk,
        owner: kp.publicKey,
        signer: kp.publicKey,
      })
      .rpc();
  }

  async function freezeAccount() {
    await program.methods
      .freezeAccount()
      .accounts({
        account: tokenAccountPk,
        mint: mintPk,
        owner: kp.publicKey,
      })
      .rpc();
  }

  async function thawAccount() {
    await program.methods
      .thawAccount()
      .accounts({
        account: tokenAccountPk,
        mint: mintPk,
        owner: kp.publicKey,
      })
      .rpc();
  }

  async function closeAccount() {
    await program.methods
      .closeAccount()
      .accounts({
        account: tokenAccountPk,
        destination: kp.publicKey,
        owner: kp.publicKey,
      })
      .rpc();
  }

  async function initializeMint2() {
    const mintKp = new Keypair();
    mintPk = mintKp.publicKey;
    const createMintAccountIx = await program.account.mint.createInstruction(
      mintKp
    );
    const initMintIx = await program.methods
      .initializeMint2(DECIMALS, kp.publicKey, kp.publicKey)
      .accounts({
        mint: mintPk,
      })
      .instruction();

    await sendAndConfirmTx([createMintAccountIx, initMintIx], [kp, mintKp]);
  }

  async function initializeAccount2() {
    const accountKp = new Keypair();
    tokenAccountPk = accountKp.publicKey;
    const createTokenAccountIx =
      await program.account.account.createInstruction(accountKp);
    const initAccountIx = await program.methods
      .initializeAccount2(kp.publicKey)
      .accounts({
        account: accountKp.publicKey,
        mint: mintPk,
        rent: SYSVAR_RENT_PUBKEY,
      })
      .instruction();

    await sendAndConfirmTx(
      [createTokenAccountIx, initAccountIx],
      [kp, accountKp]
    );
  }

  async function initializeAccount3() {
    const accountKp = new Keypair();
    tokenAccountPk = accountKp.publicKey;
    const createTokenAccountIx =
      await program.account.account.createInstruction(accountKp);
    const initAccountIx = await program.methods
      .initializeAccount3(kp.publicKey)
      .accounts({
        account: accountKp.publicKey,
        mint: mintPk,
      })
      .instruction();

    await sendAndConfirmTx(
      [createTokenAccountIx, initAccountIx],
      [kp, accountKp]
    );
  }

  async function initializeMultisig2() {
    const multisigKp = new Keypair();
    const multisig1 = new Keypair();
    const createTokenAccountIx =
      await program.account.multisig.createInstruction(multisigKp);
    const initAccountIx = await program.methods
      .initializeMultisig2(2)
      .accounts({
        multisig: multisigKp.publicKey,
        signer: kp.publicKey,
      })
      .remainingAccounts([
        { isSigner: true, isWritable: true, pubkey: kp.publicKey },
        { isSigner: true, isWritable: true, pubkey: multisig1.publicKey },
      ])
      .instruction();

    await sendAndConfirmTx(
      [createTokenAccountIx, initAccountIx],
      [kp, multisigKp, multisig1]
    );
  }

  async function syncNative() {
    const wrappedSolAccount = await createTokenAccount(NATIVE_MINT_PK);
    await program.methods
      .syncNative()
      .accounts({
        account: wrappedSolAccount,
      })
      .rpc();
  }

  async function getAccountDataSize() {
    await program.methods
      .getAccountDataSize()
      .accounts({
        mint: mintPk,
      })
      .rpc();
  }

  async function initializeImmutableOwner() {
    const accountKp = new Keypair();
    const createAccountIx = await program.account.account.createInstruction(
      accountKp
    );
    const initImmutableOwnerIx = await program.methods
      .initializeImmutableOwner()
      .accounts({
        account: accountKp.publicKey,
      })
      .instruction();

    await sendAndConfirmTx(
      [createAccountIx, initImmutableOwnerIx],
      [kp, accountKp]
    );
  }

  async function amountToUiAmount() {
    await program.methods
      .amountToUiAmount(new BN(1 * 10 ** DECIMALS))
      .accounts({
        mint: mintPk,
      })
      .rpc();
  }

  async function fetchMint() {
    const mint = await program.account.mint.fetch(mintPk);
    assert((mint.mintAuthority as PublicKey).equals(kp.publicKey));
  }

  async function fetchTokenAccount() {
    const tokenAccount = await program.account.account.fetch(tokenAccountPk);
    assert(tokenAccount.owner.equals(kp.publicKey));
  }

  async function fetchMultisig() {
    const multisig = await program.account.multisig.fetch(multisigPk);
    assert(multisig.m === MULTISIG_COUNT);
  }

  await test(initializeMint);
  await test(initializeAccount);
  await test(initializeMultisig);
  await test(mintTo);
  await test(mintToChecked);
  await test(burn);
  await test(burnChecked);
  await test(transfer);
  await test(transferChecked);
  await test(approve);
  await test(approveChecked);
  await test(revoke);
  await test(setAuthority);
  await test(freezeAccount);
  await test(thawAccount);
  await test(closeAccount);
  await test(initializeMint2);
  await test(initializeAccount2);
  await test(initializeAccount3);
  await test(initializeAccount3);
  await test(initializeMultisig2);
  await test(syncNative);
  await test(getAccountDataSize);
  await test(initializeImmutableOwner);
  await test(amountToUiAmount);
  await test(fetchMint);
  await test(fetchTokenAccount);
  await test(fetchMultisig);
}
