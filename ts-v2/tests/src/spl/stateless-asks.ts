import { splStatelessAsksProgram } from "@anchor-lang/spl-stateless-asks";
import { splTokenProgram } from "@anchor-lang/spl-token";
import { BN } from "@anchor-lang/core";
import { Keypair, PublicKey } from "@solana/web3.js";

import {
  SPL_STATELESS_ASKS_PROGRAM_ID,
  SPL_TOKEN_PROGRAM_ID,
} from "../constants";
import {
  createAta,
  createMint,
  getProvider,
  loadKp,
  sendAndConfirmTx,
  test,
} from "../utils";

export async function statelessAsksTests() {
  const provider = await getProvider();
  const program = splStatelessAsksProgram({
    provider,
    programId: SPL_STATELESS_ASKS_PROGRAM_ID,
  });
  const tokenProgram = splTokenProgram({
    provider,
    programId: SPL_TOKEN_PROGRAM_ID,
  });
  const kp = await loadKp();

  async function acceptOffer() {
    const makerKp = new Keypair();
    const makerPk = makerKp.publicKey;
    const makerMintPk = await createMint();
    const takerMintPk = await createMint();
    const makerSrcTokenAccountPk = await createAta(makerMintPk, makerPk);
    const makerDstTokenAccountPk = await createAta(takerMintPk, makerPk);

    const takerSrcTokenAccountPk = await createAta(takerMintPk, kp.publicKey);
    const takerDstTokenAccountPk = await createAta(makerMintPk, kp.publicKey);

    const makerSize = new BN(500);
    const takerSize = new BN(400);

    const [authorityPk, bump] = await PublicKey.findProgramAddress(
      [
        Buffer.from("stateless_offer"),
        makerPk.toBuffer(),
        makerMintPk.toBuffer(),
        takerMintPk.toBuffer(),
        makerSize.toBuffer("le", 8),
        takerSize.toBuffer("le", 8),
      ],
      program.programId
    );

    const mintMakerSrcIx = await tokenProgram.methods
      .mintTo(makerSize)
      .accounts({
        account: makerSrcTokenAccountPk,
        mint: makerMintPk,
        owner: kp.publicKey,
      })
      .instruction();
    const makerSrcApproveIx = await tokenProgram.methods
      .approve(makerSize)
      .accounts({
        delegate: authorityPk,
        owner: makerPk,
        source: makerSrcTokenAccountPk,
      })
      .instruction();

    const mintTakerSrcIx = await tokenProgram.methods
      .mintTo(takerSize)
      .accounts({
        account: takerSrcTokenAccountPk,
        mint: takerMintPk,
        owner: kp.publicKey,
      })
      .instruction();

    const acceptOfferIx = await program.methods
      .acceptOffer(false, makerSize, takerSize, bump)
      .accounts({
        makerWallet: makerPk,
        takerWallet: kp.publicKey,
        makerSrcAccount: makerSrcTokenAccountPk,
        makerDstAccount: makerDstTokenAccountPk,
        takerSrcAccount: takerSrcTokenAccountPk,
        takerDstAccount: takerDstTokenAccountPk,
        makerMint: makerMintPk,
        takerMint: takerMintPk,
        authority: authorityPk,
        tokenProgram: tokenProgram.programId,
      })
      .instruction();

    await sendAndConfirmTx(
      [mintMakerSrcIx, makerSrcApproveIx, mintTakerSrcIx, acceptOfferIx],
      [kp, makerKp]
    );
  }

  await test(acceptOffer);
}
