import { splFeatureProposalProgram } from "@anchor-lang/spl-feature-proposal";
import { splTokenProgram } from "@anchor-lang/spl-token";
import { BN } from "@anchor-lang/core";
import {
  Keypair,
  PublicKey,
  SystemProgram,
  SYSVAR_CLOCK_PUBKEY,
  SYSVAR_RENT_PUBKEY,
} from "@solana/web3.js";

import {
  SPL_FEATURE_PROPOSAL_PROGRAM_ID,
  SPL_TOKEN_PROGRAM_ID,
} from "../constants";
import { getProvider, loadKp, sendAndConfirmTx, test } from "../utils";

export async function featureProposalTests() {
  const provider = await getProvider();
  const program = splFeatureProposalProgram({
    provider,
    programId: SPL_FEATURE_PROPOSAL_PROGRAM_ID,
  });
  const tokenProgram = splTokenProgram({
    provider,
    programId: SPL_TOKEN_PROGRAM_ID,
  });
  const kp = await loadKp();

  let featureProposalPk: PublicKey;
  let acceptanceTokenAccountPk: PublicKey;
  let featureIdPk: PublicKey;

  async function propose() {
    const featureProposalKp = new Keypair();
    featureProposalPk = featureProposalKp.publicKey;

    const [mintPk] = await PublicKey.findProgramAddress(
      [featureProposalPk.toBuffer(), Buffer.from("mint")],
      program.programId
    );
    const [distributorTokenAccountPk] = await PublicKey.findProgramAddress(
      [featureProposalPk.toBuffer(), Buffer.from("distributor")],
      program.programId
    );
    [acceptanceTokenAccountPk] = await PublicKey.findProgramAddress(
      [featureProposalPk.toBuffer(), Buffer.from("acceptance")],
      program.programId
    );
    [featureIdPk] = await PublicKey.findProgramAddress(
      [featureProposalPk.toBuffer(), Buffer.from("feature-id")],
      program.programId
    );

    const proposeIx = await program.methods
      .propose(new BN(10), { tokensRequired: new BN(5), deadline: new BN(24) })
      .accounts({
        fundingAddress: kp.publicKey,
        featureProposalAddress: featureProposalPk,
        mintAddress: mintPk,
        distributorTokenAddress: distributorTokenAccountPk,
        acceptanceTokenAddress: acceptanceTokenAccountPk,
        feature: featureIdPk,
        systemProgram: SystemProgram.programId,
        tokenProgram: tokenProgram.programId,
        rent: SYSVAR_RENT_PUBKEY,
      })
      .instruction();

    await sendAndConfirmTx([proposeIx], [kp, featureProposalKp]);
  }

  async function tally() {
    const tallyIx = await program.methods
      .tally()
      .accounts({
        featureProposalAddress: featureProposalPk,
        acceptanceTokenAddress: acceptanceTokenAccountPk,
        feature: featureIdPk,
        systemProgram: SystemProgram.programId,
        clock: SYSVAR_CLOCK_PUBKEY,
      })
      .instruction();

    await sendAndConfirmTx([tallyIx], [kp]);
  }

  async function fetchFeatureProposal() {
    const featureProposal = await program.account.featureProposal.fetch(
      featureProposalPk
    );
    if (!featureProposal.expired) {
      throw new Error("Feature should be expired.");
    }
  }

  await test(propose);
  await test(tally);
  await test(fetchFeatureProposal);
}
