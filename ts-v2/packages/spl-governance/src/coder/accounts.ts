// @ts-nocheck
import * as B from "@native-to-anchor/buffer-layout";
import { AccountsCoder, Idl } from "@anchor-lang/core";
import { IdlTypeDef } from "@anchor-lang/core/dist/cjs/idl";

export class SplGovernanceAccountsCoder<A extends string = string>
  implements AccountsCoder
{
  constructor(_idl: Idl) {}

  public async encode<T = any>(accountName: A, account: T): Promise<Buffer> {
    switch (accountName) {
      case "realmV2": {
        const buffer = Buffer.alloc(10485760); // Space is variable
        const len = REALM_V2_LAYOUT.encode(account, buffer);
        return buffer.slice(0, len);
      }
      case "proposalV2": {
        const buffer = Buffer.alloc(10485760); // Space is variable
        const len = PROPOSAL_V2_LAYOUT.encode(account, buffer);
        return buffer.slice(0, len);
      }
      case "programMetadata": {
        const buffer = Buffer.alloc(10485760); // Space is variable
        const len = PROGRAM_METADATA_LAYOUT.encode(account, buffer);
        return buffer.slice(0, len);
      }
      case "signatoryRecordV2": {
        const buffer = Buffer.alloc(74);
        const len = SIGNATORY_RECORD_V2_LAYOUT.encode(account, buffer);
        return buffer.slice(0, len);
      }
      case "realmV1": {
        const buffer = Buffer.alloc(10485760); // Space is variable
        const len = REALM_V1_LAYOUT.encode(account, buffer);
        return buffer.slice(0, len);
      }
      case "tokenOwnerRecordV1": {
        const buffer = Buffer.alloc(10485760); // Space is variable
        const len = TOKEN_OWNER_RECORD_V1_LAYOUT.encode(account, buffer);
        return buffer.slice(0, len);
      }
      case "governanceV1": {
        const buffer = Buffer.alloc(108);
        const len = GOVERNANCE_V1_LAYOUT.encode(account, buffer);
        return buffer.slice(0, len);
      }
      case "proposalV1": {
        const buffer = Buffer.alloc(10485760); // Space is variable
        const len = PROPOSAL_V1_LAYOUT.encode(account, buffer);
        return buffer.slice(0, len);
      }
      case "signatoryRecordV1": {
        const buffer = Buffer.alloc(66);
        const len = SIGNATORY_RECORD_V1_LAYOUT.encode(account, buffer);
        return buffer.slice(0, len);
      }
      case "voteRecordV1": {
        const buffer = Buffer.alloc(75);
        const len = VOTE_RECORD_V1_LAYOUT.encode(account, buffer);
        return buffer.slice(0, len);
      }
      case "governanceV2": {
        const buffer = Buffer.alloc(236);
        const len = GOVERNANCE_V2_LAYOUT.encode(account, buffer);
        return buffer.slice(0, len);
      }
      case "voteRecordV2": {
        const buffer = Buffer.alloc(83);
        const len = VOTE_RECORD_V2_LAYOUT.encode(account, buffer);
        return buffer.slice(0, len);
      }
      case "tokenOwnerRecordV2": {
        const buffer = Buffer.alloc(10485760); // Space is variable
        const len = TOKEN_OWNER_RECORD_V2_LAYOUT.encode(account, buffer);
        return buffer.slice(0, len);
      }
      case "realmConfigAccount": {
        const buffer = Buffer.alloc(10485760); // Space is variable
        const len = REALM_CONFIG_ACCOUNT_LAYOUT.encode(account, buffer);
        return buffer.slice(0, len);
      }
      case "proposalTransactionV2": {
        const buffer = Buffer.alloc(10485760); // Space is variable
        const len = PROPOSAL_TRANSACTION_V2_LAYOUT.encode(account, buffer);
        return buffer.slice(0, len);
      }
      default: {
        throw new Error(`Invalid account name: ${accountName}`);
      }
    }
  }

  public decode<T = any>(accountName: A, ix: Buffer): T {
    return this.decodeUnchecked(accountName, ix);
  }

  public decodeUnchecked<T = any>(accountName: A, ix: Buffer): T {
    switch (accountName) {
      case "realmV2": {
        return decodeRealmV2Account(ix);
      }
      case "proposalV2": {
        return decodeProposalV2Account(ix);
      }
      case "programMetadata": {
        return decodeProgramMetadataAccount(ix);
      }
      case "signatoryRecordV2": {
        return decodeSignatoryRecordV2Account(ix);
      }
      case "realmV1": {
        return decodeRealmV1Account(ix);
      }
      case "tokenOwnerRecordV1": {
        return decodeTokenOwnerRecordV1Account(ix);
      }
      case "governanceV1": {
        return decodeGovernanceV1Account(ix);
      }
      case "proposalV1": {
        return decodeProposalV1Account(ix);
      }
      case "signatoryRecordV1": {
        return decodeSignatoryRecordV1Account(ix);
      }
      case "voteRecordV1": {
        return decodeVoteRecordV1Account(ix);
      }
      case "governanceV2": {
        return decodeGovernanceV2Account(ix);
      }
      case "voteRecordV2": {
        return decodeVoteRecordV2Account(ix);
      }
      case "tokenOwnerRecordV2": {
        return decodeTokenOwnerRecordV2Account(ix);
      }
      case "realmConfigAccount": {
        return decodeRealmConfigAccountAccount(ix);
      }
      case "proposalTransactionV2": {
        return decodeProposalTransactionV2Account(ix);
      }
      default: {
        throw new Error(`Invalid account name: ${accountName}`);
      }
    }
  }

  public memcmp(
    accountName: A,
    _appendData?: Buffer
  ): { dataSize?: number; offset?: number; bytes?: string } {
    switch (accountName) {
      case "realmV2": {
        return {
          // Space is variable
        };
      }
      case "proposalV2": {
        return {
          // Space is variable
        };
      }
      case "programMetadata": {
        return {
          // Space is variable
        };
      }
      case "signatoryRecordV2": {
        return {
          dataSize: 74,
        };
      }
      case "realmV1": {
        return {
          // Space is variable
        };
      }
      case "tokenOwnerRecordV1": {
        return {
          // Space is variable
        };
      }
      case "governanceV1": {
        return {
          dataSize: 108,
        };
      }
      case "proposalV1": {
        return {
          // Space is variable
        };
      }
      case "signatoryRecordV1": {
        return {
          dataSize: 66,
        };
      }
      case "voteRecordV1": {
        return {
          dataSize: 75,
        };
      }
      case "governanceV2": {
        return {
          dataSize: 236,
        };
      }
      case "voteRecordV2": {
        return {
          dataSize: 83,
        };
      }
      case "tokenOwnerRecordV2": {
        return {
          // Space is variable
        };
      }
      case "realmConfigAccount": {
        return {
          // Space is variable
        };
      }
      case "proposalTransactionV2": {
        return {
          // Space is variable
        };
      }
      default: {
        throw new Error(`Invalid account name: ${accountName}`);
      }
    }
  }

  public size(idlAccount: IdlTypeDef): number {
    switch (idlAccount.name) {
      case "realmV2": {
        return 0; // Space is variable;
      }
      case "proposalV2": {
        return 0; // Space is variable;
      }
      case "programMetadata": {
        return 0; // Space is variable;
      }
      case "signatoryRecordV2": {
        return 74;
      }
      case "realmV1": {
        return 0; // Space is variable;
      }
      case "tokenOwnerRecordV1": {
        return 0; // Space is variable;
      }
      case "governanceV1": {
        return 108;
      }
      case "proposalV1": {
        return 0; // Space is variable;
      }
      case "signatoryRecordV1": {
        return 66;
      }
      case "voteRecordV1": {
        return 75;
      }
      case "governanceV2": {
        return 236;
      }
      case "voteRecordV2": {
        return 83;
      }
      case "tokenOwnerRecordV2": {
        return 0; // Space is variable;
      }
      case "realmConfigAccount": {
        return 0; // Space is variable;
      }
      case "proposalTransactionV2": {
        return 0; // Space is variable;
      }
      default: {
        throw new Error(`Invalid account name: ${idlAccount.name}`);
      }
    }
  }
}

function decodeRealmV2Account<T = any>(ix: Buffer): T {
  return REALM_V2_LAYOUT.decode(ix) as T;
}
function decodeProposalV2Account<T = any>(ix: Buffer): T {
  return PROPOSAL_V2_LAYOUT.decode(ix) as T;
}
function decodeProgramMetadataAccount<T = any>(ix: Buffer): T {
  return PROGRAM_METADATA_LAYOUT.decode(ix) as T;
}
function decodeSignatoryRecordV2Account<T = any>(ix: Buffer): T {
  return SIGNATORY_RECORD_V2_LAYOUT.decode(ix) as T;
}
function decodeRealmV1Account<T = any>(ix: Buffer): T {
  return REALM_V1_LAYOUT.decode(ix) as T;
}
function decodeTokenOwnerRecordV1Account<T = any>(ix: Buffer): T {
  return TOKEN_OWNER_RECORD_V1_LAYOUT.decode(ix) as T;
}
function decodeGovernanceV1Account<T = any>(ix: Buffer): T {
  return GOVERNANCE_V1_LAYOUT.decode(ix) as T;
}
function decodeProposalV1Account<T = any>(ix: Buffer): T {
  return PROPOSAL_V1_LAYOUT.decode(ix) as T;
}
function decodeSignatoryRecordV1Account<T = any>(ix: Buffer): T {
  return SIGNATORY_RECORD_V1_LAYOUT.decode(ix) as T;
}
function decodeVoteRecordV1Account<T = any>(ix: Buffer): T {
  return VOTE_RECORD_V1_LAYOUT.decode(ix) as T;
}
function decodeGovernanceV2Account<T = any>(ix: Buffer): T {
  return GOVERNANCE_V2_LAYOUT.decode(ix) as T;
}
function decodeVoteRecordV2Account<T = any>(ix: Buffer): T {
  return VOTE_RECORD_V2_LAYOUT.decode(ix) as T;
}
function decodeTokenOwnerRecordV2Account<T = any>(ix: Buffer): T {
  return TOKEN_OWNER_RECORD_V2_LAYOUT.decode(ix) as T;
}
function decodeRealmConfigAccountAccount<T = any>(ix: Buffer): T {
  return REALM_CONFIG_ACCOUNT_LAYOUT.decode(ix) as T;
}
function decodeProposalTransactionV2Account<T = any>(ix: Buffer): T {
  return PROPOSAL_TRANSACTION_V2_LAYOUT.decode(ix) as T;
}

const REALM_V2_LAYOUT: any = B.struct([
  ((p: string) => {
    const U = B.union(B.u8("discriminator"), null, p);
    U.addVariant(0, B.struct([]), "uninitialized");
    U.addVariant(1, B.struct([]), "realmV1");
    U.addVariant(2, B.struct([]), "tokenOwnerRecordV1");
    U.addVariant(3, B.struct([]), "governanceV1");
    U.addVariant(4, B.struct([]), "programGovernanceV1");
    U.addVariant(5, B.struct([]), "proposalV1");
    U.addVariant(6, B.struct([]), "signatoryRecordV1");
    U.addVariant(7, B.struct([]), "voteRecordV1");
    U.addVariant(8, B.struct([]), "proposalInstructionV1");
    U.addVariant(9, B.struct([]), "mintGovernanceV1");
    U.addVariant(10, B.struct([]), "tokenGovernanceV1");
    U.addVariant(11, B.struct([]), "realmConfig");
    U.addVariant(12, B.struct([]), "voteRecordV2");
    U.addVariant(13, B.struct([]), "proposalTransactionV2");
    U.addVariant(14, B.struct([]), "proposalV2");
    U.addVariant(15, B.struct([]), "programMetadata");
    U.addVariant(16, B.struct([]), "realmV2");
    U.addVariant(17, B.struct([]), "tokenOwnerRecordV2");
    U.addVariant(18, B.struct([]), "governanceV2");
    U.addVariant(19, B.struct([]), "programGovernanceV2");
    U.addVariant(20, B.struct([]), "mintGovernanceV2");
    U.addVariant(21, B.struct([]), "tokenGovernanceV2");
    U.addVariant(22, B.struct([]), "signatoryRecordV2");
    return U;
  })("accountType"),
  B.publicKey("communityMint"),
  B.struct(
    [
      B.bool("useCommunityVoterWeightAddin"),
      B.bool("useMaxCommunityVoterWeightAddin"),
      B.seq(B.u8(), 6, "reserved"),
      B.u64("minCommunityWeightToCreateGovernance"),
      ((p: string) => {
        const U = B.union(B.u8("discriminator"), null, p);
        U.addVariant(0, B.u64(), "supplyFraction");
        U.addVariant(1, B.u64(), "absolute");
        return U;
      })("communityMintMaxVoteWeightSource"),
      B.option(B.publicKey(), "councilMint"),
    ],
    "config"
  ),
  B.seq(B.u8(), 6, "reserved"),
  B.u16("votingProposalCount"),
  B.option(B.publicKey(), "authority"),
  B.utf8Str("name"),
  B.seq(B.u8(), 128, "reservedV2"),
]);

const PROPOSAL_V2_LAYOUT: any = B.struct([
  ((p: string) => {
    const U = B.union(B.u8("discriminator"), null, p);
    U.addVariant(0, B.struct([]), "uninitialized");
    U.addVariant(1, B.struct([]), "realmV1");
    U.addVariant(2, B.struct([]), "tokenOwnerRecordV1");
    U.addVariant(3, B.struct([]), "governanceV1");
    U.addVariant(4, B.struct([]), "programGovernanceV1");
    U.addVariant(5, B.struct([]), "proposalV1");
    U.addVariant(6, B.struct([]), "signatoryRecordV1");
    U.addVariant(7, B.struct([]), "voteRecordV1");
    U.addVariant(8, B.struct([]), "proposalInstructionV1");
    U.addVariant(9, B.struct([]), "mintGovernanceV1");
    U.addVariant(10, B.struct([]), "tokenGovernanceV1");
    U.addVariant(11, B.struct([]), "realmConfig");
    U.addVariant(12, B.struct([]), "voteRecordV2");
    U.addVariant(13, B.struct([]), "proposalTransactionV2");
    U.addVariant(14, B.struct([]), "proposalV2");
    U.addVariant(15, B.struct([]), "programMetadata");
    U.addVariant(16, B.struct([]), "realmV2");
    U.addVariant(17, B.struct([]), "tokenOwnerRecordV2");
    U.addVariant(18, B.struct([]), "governanceV2");
    U.addVariant(19, B.struct([]), "programGovernanceV2");
    U.addVariant(20, B.struct([]), "mintGovernanceV2");
    U.addVariant(21, B.struct([]), "tokenGovernanceV2");
    U.addVariant(22, B.struct([]), "signatoryRecordV2");
    return U;
  })("accountType"),
  B.publicKey("governance"),
  B.publicKey("governingTokenMint"),
  ((p: string) => {
    const U = B.union(B.u8("discriminator"), null, p);
    U.addVariant(0, B.struct([]), "draft");
    U.addVariant(1, B.struct([]), "signingOff");
    U.addVariant(2, B.struct([]), "voting");
    U.addVariant(3, B.struct([]), "succeeded");
    U.addVariant(4, B.struct([]), "executing");
    U.addVariant(5, B.struct([]), "completed");
    U.addVariant(6, B.struct([]), "cancelled");
    U.addVariant(7, B.struct([]), "defeated");
    U.addVariant(8, B.struct([]), "executingWithErrors");
    U.addVariant(9, B.struct([]), "vetoed");
    return U;
  })("state"),
  B.publicKey("tokenOwnerRecord"),
  B.u8("signatoriesCount"),
  B.u8("signatoriesSignedOffCount"),
  ((p: string) => {
    const U = B.union(B.u8("discriminator"), null, p);
    U.addVariant(0, B.struct([]), "singleChoice");
    U.addVariant(
      1,
      B.struct([B.u8("maxVoterOptions"), B.u8("maxWinningOptions")]),
      "multiChoice"
    );
    return U;
  })("voteType"),
  B.vec(
    B.struct([
      B.utf8Str("label"),
      B.u64("voteWeight"),
      ((p: string) => {
        const U = B.union(B.u8("discriminator"), null, p);
        U.addVariant(0, B.struct([]), "none");
        U.addVariant(1, B.struct([]), "succeeded");
        U.addVariant(2, B.struct([]), "defeated");
        return U;
      })("voteResult"),
      B.u16("transactionsExecutedCount"),
      B.u16("transactionsCount"),
      B.u16("transactionsNextIndex"),
    ]),
    "options"
  ),
  B.option(B.u64(), "denyVoteWeight"),
  B.u8("reserved1"),
  B.option(B.u64(), "abstainVoteWeight"),
  B.option(B.i64(), "startVotingAt"),
  B.i64("draftAt"),
  B.option(B.i64(), "signingOffAt"),
  B.option(B.i64(), "votingAt"),
  B.option(B.u64(), "votingAtSlot"),
  B.option(B.i64(), "votingCompletedAt"),
  B.option(B.i64(), "executingAt"),
  B.option(B.i64(), "closedAt"),
  ((p: string) => {
    const U = B.union(B.u8("discriminator"), null, p);
    U.addVariant(0, B.struct([]), "none");
    U.addVariant(1, B.struct([]), "ordered");
    U.addVariant(2, B.struct([]), "useTransaction");
    return U;
  })("executionFlags"),
  B.option(B.u64(), "maxVoteWeight"),
  B.option(B.u32(), "maxVotingTime"),
  B.option(
    ((p: string) => {
      const U = B.union(B.u8("discriminator"), null, p);
      U.addVariant(0, B.u8(), "yesVotePercentage");
      U.addVariant(1, B.u8(), "quorumPercentage");
      U.addVariant(2, B.struct([]), "disabled");
      return U;
    })(),
    "voteThreshold"
  ),
  B.seq(B.u8(), 64, "reserved"),
  B.utf8Str("name"),
  B.utf8Str("descriptionLink"),
  B.u64("vetoVoteWeight"),
]);

const PROGRAM_METADATA_LAYOUT: any = B.struct([
  ((p: string) => {
    const U = B.union(B.u8("discriminator"), null, p);
    U.addVariant(0, B.struct([]), "uninitialized");
    U.addVariant(1, B.struct([]), "realmV1");
    U.addVariant(2, B.struct([]), "tokenOwnerRecordV1");
    U.addVariant(3, B.struct([]), "governanceV1");
    U.addVariant(4, B.struct([]), "programGovernanceV1");
    U.addVariant(5, B.struct([]), "proposalV1");
    U.addVariant(6, B.struct([]), "signatoryRecordV1");
    U.addVariant(7, B.struct([]), "voteRecordV1");
    U.addVariant(8, B.struct([]), "proposalInstructionV1");
    U.addVariant(9, B.struct([]), "mintGovernanceV1");
    U.addVariant(10, B.struct([]), "tokenGovernanceV1");
    U.addVariant(11, B.struct([]), "realmConfig");
    U.addVariant(12, B.struct([]), "voteRecordV2");
    U.addVariant(13, B.struct([]), "proposalTransactionV2");
    U.addVariant(14, B.struct([]), "proposalV2");
    U.addVariant(15, B.struct([]), "programMetadata");
    U.addVariant(16, B.struct([]), "realmV2");
    U.addVariant(17, B.struct([]), "tokenOwnerRecordV2");
    U.addVariant(18, B.struct([]), "governanceV2");
    U.addVariant(19, B.struct([]), "programGovernanceV2");
    U.addVariant(20, B.struct([]), "mintGovernanceV2");
    U.addVariant(21, B.struct([]), "tokenGovernanceV2");
    U.addVariant(22, B.struct([]), "signatoryRecordV2");
    return U;
  })("accountType"),
  B.u64("updatedAt"),
  B.utf8Str("version"),
  B.seq(B.u8(), 64, "reserved"),
]);

const SIGNATORY_RECORD_V2_LAYOUT: any = B.struct([
  ((p: string) => {
    const U = B.union(B.u8("discriminator"), null, p);
    U.addVariant(0, B.struct([]), "uninitialized");
    U.addVariant(1, B.struct([]), "realmV1");
    U.addVariant(2, B.struct([]), "tokenOwnerRecordV1");
    U.addVariant(3, B.struct([]), "governanceV1");
    U.addVariant(4, B.struct([]), "programGovernanceV1");
    U.addVariant(5, B.struct([]), "proposalV1");
    U.addVariant(6, B.struct([]), "signatoryRecordV1");
    U.addVariant(7, B.struct([]), "voteRecordV1");
    U.addVariant(8, B.struct([]), "proposalInstructionV1");
    U.addVariant(9, B.struct([]), "mintGovernanceV1");
    U.addVariant(10, B.struct([]), "tokenGovernanceV1");
    U.addVariant(11, B.struct([]), "realmConfig");
    U.addVariant(12, B.struct([]), "voteRecordV2");
    U.addVariant(13, B.struct([]), "proposalTransactionV2");
    U.addVariant(14, B.struct([]), "proposalV2");
    U.addVariant(15, B.struct([]), "programMetadata");
    U.addVariant(16, B.struct([]), "realmV2");
    U.addVariant(17, B.struct([]), "tokenOwnerRecordV2");
    U.addVariant(18, B.struct([]), "governanceV2");
    U.addVariant(19, B.struct([]), "programGovernanceV2");
    U.addVariant(20, B.struct([]), "mintGovernanceV2");
    U.addVariant(21, B.struct([]), "tokenGovernanceV2");
    U.addVariant(22, B.struct([]), "signatoryRecordV2");
    return U;
  })("accountType"),
  B.publicKey("proposal"),
  B.publicKey("signatory"),
  B.bool("signedOff"),
  B.seq(B.u8(), 8, "reservedV2"),
]);

const REALM_V1_LAYOUT: any = B.struct([
  ((p: string) => {
    const U = B.union(B.u8("discriminator"), null, p);
    U.addVariant(0, B.struct([]), "uninitialized");
    U.addVariant(1, B.struct([]), "realmV1");
    U.addVariant(2, B.struct([]), "tokenOwnerRecordV1");
    U.addVariant(3, B.struct([]), "governanceV1");
    U.addVariant(4, B.struct([]), "programGovernanceV1");
    U.addVariant(5, B.struct([]), "proposalV1");
    U.addVariant(6, B.struct([]), "signatoryRecordV1");
    U.addVariant(7, B.struct([]), "voteRecordV1");
    U.addVariant(8, B.struct([]), "proposalInstructionV1");
    U.addVariant(9, B.struct([]), "mintGovernanceV1");
    U.addVariant(10, B.struct([]), "tokenGovernanceV1");
    U.addVariant(11, B.struct([]), "realmConfig");
    U.addVariant(12, B.struct([]), "voteRecordV2");
    U.addVariant(13, B.struct([]), "proposalTransactionV2");
    U.addVariant(14, B.struct([]), "proposalV2");
    U.addVariant(15, B.struct([]), "programMetadata");
    U.addVariant(16, B.struct([]), "realmV2");
    U.addVariant(17, B.struct([]), "tokenOwnerRecordV2");
    U.addVariant(18, B.struct([]), "governanceV2");
    U.addVariant(19, B.struct([]), "programGovernanceV2");
    U.addVariant(20, B.struct([]), "mintGovernanceV2");
    U.addVariant(21, B.struct([]), "tokenGovernanceV2");
    U.addVariant(22, B.struct([]), "signatoryRecordV2");
    return U;
  })("accountType"),
  B.publicKey("communityMint"),
  B.struct(
    [
      B.bool("useCommunityVoterWeightAddin"),
      B.bool("useMaxCommunityVoterWeightAddin"),
      B.seq(B.u8(), 6, "reserved"),
      B.u64("minCommunityWeightToCreateGovernance"),
      ((p: string) => {
        const U = B.union(B.u8("discriminator"), null, p);
        U.addVariant(0, B.u64(), "supplyFraction");
        U.addVariant(1, B.u64(), "absolute");
        return U;
      })("communityMintMaxVoteWeightSource"),
      B.option(B.publicKey(), "councilMint"),
    ],
    "config"
  ),
  B.seq(B.u8(), 6, "reserved"),
  B.u16("votingProposalCount"),
  B.option(B.publicKey(), "authority"),
  B.utf8Str("name"),
]);

const TOKEN_OWNER_RECORD_V1_LAYOUT: any = B.struct([
  ((p: string) => {
    const U = B.union(B.u8("discriminator"), null, p);
    U.addVariant(0, B.struct([]), "uninitialized");
    U.addVariant(1, B.struct([]), "realmV1");
    U.addVariant(2, B.struct([]), "tokenOwnerRecordV1");
    U.addVariant(3, B.struct([]), "governanceV1");
    U.addVariant(4, B.struct([]), "programGovernanceV1");
    U.addVariant(5, B.struct([]), "proposalV1");
    U.addVariant(6, B.struct([]), "signatoryRecordV1");
    U.addVariant(7, B.struct([]), "voteRecordV1");
    U.addVariant(8, B.struct([]), "proposalInstructionV1");
    U.addVariant(9, B.struct([]), "mintGovernanceV1");
    U.addVariant(10, B.struct([]), "tokenGovernanceV1");
    U.addVariant(11, B.struct([]), "realmConfig");
    U.addVariant(12, B.struct([]), "voteRecordV2");
    U.addVariant(13, B.struct([]), "proposalTransactionV2");
    U.addVariant(14, B.struct([]), "proposalV2");
    U.addVariant(15, B.struct([]), "programMetadata");
    U.addVariant(16, B.struct([]), "realmV2");
    U.addVariant(17, B.struct([]), "tokenOwnerRecordV2");
    U.addVariant(18, B.struct([]), "governanceV2");
    U.addVariant(19, B.struct([]), "programGovernanceV2");
    U.addVariant(20, B.struct([]), "mintGovernanceV2");
    U.addVariant(21, B.struct([]), "tokenGovernanceV2");
    U.addVariant(22, B.struct([]), "signatoryRecordV2");
    return U;
  })("accountType"),
  B.publicKey("realm"),
  B.publicKey("governingTokenMint"),
  B.publicKey("governingTokenOwner"),
  B.u64("governingTokenDepositAmount"),
  B.u32("unrelinquishedVotesCount"),
  B.u32("totalVotesCount"),
  B.u8("outstandingProposalCount"),
  B.seq(B.u8(), 7, "reserved"),
  B.option(B.publicKey(), "governanceDelegate"),
]);

const GOVERNANCE_V1_LAYOUT: any = B.struct([
  ((p: string) => {
    const U = B.union(B.u8("discriminator"), null, p);
    U.addVariant(0, B.struct([]), "uninitialized");
    U.addVariant(1, B.struct([]), "realmV1");
    U.addVariant(2, B.struct([]), "tokenOwnerRecordV1");
    U.addVariant(3, B.struct([]), "governanceV1");
    U.addVariant(4, B.struct([]), "programGovernanceV1");
    U.addVariant(5, B.struct([]), "proposalV1");
    U.addVariant(6, B.struct([]), "signatoryRecordV1");
    U.addVariant(7, B.struct([]), "voteRecordV1");
    U.addVariant(8, B.struct([]), "proposalInstructionV1");
    U.addVariant(9, B.struct([]), "mintGovernanceV1");
    U.addVariant(10, B.struct([]), "tokenGovernanceV1");
    U.addVariant(11, B.struct([]), "realmConfig");
    U.addVariant(12, B.struct([]), "voteRecordV2");
    U.addVariant(13, B.struct([]), "proposalTransactionV2");
    U.addVariant(14, B.struct([]), "proposalV2");
    U.addVariant(15, B.struct([]), "programMetadata");
    U.addVariant(16, B.struct([]), "realmV2");
    U.addVariant(17, B.struct([]), "tokenOwnerRecordV2");
    U.addVariant(18, B.struct([]), "governanceV2");
    U.addVariant(19, B.struct([]), "programGovernanceV2");
    U.addVariant(20, B.struct([]), "mintGovernanceV2");
    U.addVariant(21, B.struct([]), "tokenGovernanceV2");
    U.addVariant(22, B.struct([]), "signatoryRecordV2");
    return U;
  })("accountType"),
  B.publicKey("realm"),
  B.publicKey("governedAccount"),
  B.u32("proposalsCount"),
  B.struct(
    [
      ((p: string) => {
        const U = B.union(B.u8("discriminator"), null, p);
        U.addVariant(0, B.u8(), "yesVotePercentage");
        U.addVariant(1, B.u8(), "quorumPercentage");
        U.addVariant(2, B.struct([]), "disabled");
        return U;
      })("communityVoteThreshold"),
      B.u64("minCommunityWeightToCreateProposal"),
      B.u32("minTransactionHoldUpTime"),
      B.u32("maxVotingTime"),
      ((p: string) => {
        const U = B.union(B.u8("discriminator"), null, p);
        U.addVariant(0, B.struct([]), "strict");
        U.addVariant(1, B.struct([]), "early");
        U.addVariant(2, B.struct([]), "disabled");
        return U;
      })("voteTipping"),
      ((p: string) => {
        const U = B.union(B.u8("discriminator"), null, p);
        U.addVariant(0, B.u8(), "yesVotePercentage");
        U.addVariant(1, B.u8(), "quorumPercentage");
        U.addVariant(2, B.struct([]), "disabled");
        return U;
      })("councilVoteThreshold"),
      ((p: string) => {
        const U = B.union(B.u8("discriminator"), null, p);
        U.addVariant(0, B.u8(), "yesVotePercentage");
        U.addVariant(1, B.u8(), "quorumPercentage");
        U.addVariant(2, B.struct([]), "disabled");
        return U;
      })("councilVetoVoteThreshold"),
      B.u64("minCouncilWeightToCreateProposal"),
    ],
    "config"
  ),
  B.seq(B.u8(), 6, "reserved"),
  B.u16("votingProposalCount"),
]);

const PROPOSAL_V1_LAYOUT: any = B.struct([
  ((p: string) => {
    const U = B.union(B.u8("discriminator"), null, p);
    U.addVariant(0, B.struct([]), "uninitialized");
    U.addVariant(1, B.struct([]), "realmV1");
    U.addVariant(2, B.struct([]), "tokenOwnerRecordV1");
    U.addVariant(3, B.struct([]), "governanceV1");
    U.addVariant(4, B.struct([]), "programGovernanceV1");
    U.addVariant(5, B.struct([]), "proposalV1");
    U.addVariant(6, B.struct([]), "signatoryRecordV1");
    U.addVariant(7, B.struct([]), "voteRecordV1");
    U.addVariant(8, B.struct([]), "proposalInstructionV1");
    U.addVariant(9, B.struct([]), "mintGovernanceV1");
    U.addVariant(10, B.struct([]), "tokenGovernanceV1");
    U.addVariant(11, B.struct([]), "realmConfig");
    U.addVariant(12, B.struct([]), "voteRecordV2");
    U.addVariant(13, B.struct([]), "proposalTransactionV2");
    U.addVariant(14, B.struct([]), "proposalV2");
    U.addVariant(15, B.struct([]), "programMetadata");
    U.addVariant(16, B.struct([]), "realmV2");
    U.addVariant(17, B.struct([]), "tokenOwnerRecordV2");
    U.addVariant(18, B.struct([]), "governanceV2");
    U.addVariant(19, B.struct([]), "programGovernanceV2");
    U.addVariant(20, B.struct([]), "mintGovernanceV2");
    U.addVariant(21, B.struct([]), "tokenGovernanceV2");
    U.addVariant(22, B.struct([]), "signatoryRecordV2");
    return U;
  })("accountType"),
  B.publicKey("governance"),
  B.publicKey("governingTokenMint"),
  ((p: string) => {
    const U = B.union(B.u8("discriminator"), null, p);
    U.addVariant(0, B.struct([]), "draft");
    U.addVariant(1, B.struct([]), "signingOff");
    U.addVariant(2, B.struct([]), "voting");
    U.addVariant(3, B.struct([]), "succeeded");
    U.addVariant(4, B.struct([]), "executing");
    U.addVariant(5, B.struct([]), "completed");
    U.addVariant(6, B.struct([]), "cancelled");
    U.addVariant(7, B.struct([]), "defeated");
    U.addVariant(8, B.struct([]), "executingWithErrors");
    U.addVariant(9, B.struct([]), "vetoed");
    return U;
  })("state"),
  B.publicKey("tokenOwnerRecord"),
  B.u8("signatoriesCount"),
  B.u8("signatoriesSignedOffCount"),
  B.u64("yesVotesCount"),
  B.u64("noVotesCount"),
  B.u16("instructionsExecutedCount"),
  B.u16("instructionsCount"),
  B.u16("instructionsNextIndex"),
  B.i64("draftAt"),
  B.option(B.i64(), "signingOffAt"),
  B.option(B.i64(), "votingAt"),
  B.option(B.u64(), "votingAtSlot"),
  B.option(B.i64(), "votingCompletedAt"),
  B.option(B.i64(), "executingAt"),
  B.option(B.i64(), "closedAt"),
  ((p: string) => {
    const U = B.union(B.u8("discriminator"), null, p);
    U.addVariant(0, B.struct([]), "none");
    U.addVariant(1, B.struct([]), "ordered");
    U.addVariant(2, B.struct([]), "useTransaction");
    return U;
  })("executionFlags"),
  B.option(B.u64(), "maxVoteWeight"),
  B.option(
    ((p: string) => {
      const U = B.union(B.u8("discriminator"), null, p);
      U.addVariant(0, B.u8(), "yesVotePercentage");
      U.addVariant(1, B.u8(), "quorumPercentage");
      U.addVariant(2, B.struct([]), "disabled");
      return U;
    })(),
    "voteThreshold"
  ),
  B.utf8Str("name"),
  B.utf8Str("descriptionLink"),
]);

const SIGNATORY_RECORD_V1_LAYOUT: any = B.struct([
  ((p: string) => {
    const U = B.union(B.u8("discriminator"), null, p);
    U.addVariant(0, B.struct([]), "uninitialized");
    U.addVariant(1, B.struct([]), "realmV1");
    U.addVariant(2, B.struct([]), "tokenOwnerRecordV1");
    U.addVariant(3, B.struct([]), "governanceV1");
    U.addVariant(4, B.struct([]), "programGovernanceV1");
    U.addVariant(5, B.struct([]), "proposalV1");
    U.addVariant(6, B.struct([]), "signatoryRecordV1");
    U.addVariant(7, B.struct([]), "voteRecordV1");
    U.addVariant(8, B.struct([]), "proposalInstructionV1");
    U.addVariant(9, B.struct([]), "mintGovernanceV1");
    U.addVariant(10, B.struct([]), "tokenGovernanceV1");
    U.addVariant(11, B.struct([]), "realmConfig");
    U.addVariant(12, B.struct([]), "voteRecordV2");
    U.addVariant(13, B.struct([]), "proposalTransactionV2");
    U.addVariant(14, B.struct([]), "proposalV2");
    U.addVariant(15, B.struct([]), "programMetadata");
    U.addVariant(16, B.struct([]), "realmV2");
    U.addVariant(17, B.struct([]), "tokenOwnerRecordV2");
    U.addVariant(18, B.struct([]), "governanceV2");
    U.addVariant(19, B.struct([]), "programGovernanceV2");
    U.addVariant(20, B.struct([]), "mintGovernanceV2");
    U.addVariant(21, B.struct([]), "tokenGovernanceV2");
    U.addVariant(22, B.struct([]), "signatoryRecordV2");
    return U;
  })("accountType"),
  B.publicKey("proposal"),
  B.publicKey("signatory"),
  B.bool("signedOff"),
]);

const VOTE_RECORD_V1_LAYOUT: any = B.struct([
  ((p: string) => {
    const U = B.union(B.u8("discriminator"), null, p);
    U.addVariant(0, B.struct([]), "uninitialized");
    U.addVariant(1, B.struct([]), "realmV1");
    U.addVariant(2, B.struct([]), "tokenOwnerRecordV1");
    U.addVariant(3, B.struct([]), "governanceV1");
    U.addVariant(4, B.struct([]), "programGovernanceV1");
    U.addVariant(5, B.struct([]), "proposalV1");
    U.addVariant(6, B.struct([]), "signatoryRecordV1");
    U.addVariant(7, B.struct([]), "voteRecordV1");
    U.addVariant(8, B.struct([]), "proposalInstructionV1");
    U.addVariant(9, B.struct([]), "mintGovernanceV1");
    U.addVariant(10, B.struct([]), "tokenGovernanceV1");
    U.addVariant(11, B.struct([]), "realmConfig");
    U.addVariant(12, B.struct([]), "voteRecordV2");
    U.addVariant(13, B.struct([]), "proposalTransactionV2");
    U.addVariant(14, B.struct([]), "proposalV2");
    U.addVariant(15, B.struct([]), "programMetadata");
    U.addVariant(16, B.struct([]), "realmV2");
    U.addVariant(17, B.struct([]), "tokenOwnerRecordV2");
    U.addVariant(18, B.struct([]), "governanceV2");
    U.addVariant(19, B.struct([]), "programGovernanceV2");
    U.addVariant(20, B.struct([]), "mintGovernanceV2");
    U.addVariant(21, B.struct([]), "tokenGovernanceV2");
    U.addVariant(22, B.struct([]), "signatoryRecordV2");
    return U;
  })("accountType"),
  B.publicKey("proposal"),
  B.publicKey("governingTokenOwner"),
  B.bool("isRelinquished"),
  ((p: string) => {
    const U = B.union(B.u8("discriminator"), null, p);
    U.addVariant(0, B.u64(), "yes");
    U.addVariant(1, B.u64(), "no");
    return U;
  })("voteWeight"),
]);

const GOVERNANCE_V2_LAYOUT: any = B.struct([
  ((p: string) => {
    const U = B.union(B.u8("discriminator"), null, p);
    U.addVariant(0, B.struct([]), "uninitialized");
    U.addVariant(1, B.struct([]), "realmV1");
    U.addVariant(2, B.struct([]), "tokenOwnerRecordV1");
    U.addVariant(3, B.struct([]), "governanceV1");
    U.addVariant(4, B.struct([]), "programGovernanceV1");
    U.addVariant(5, B.struct([]), "proposalV1");
    U.addVariant(6, B.struct([]), "signatoryRecordV1");
    U.addVariant(7, B.struct([]), "voteRecordV1");
    U.addVariant(8, B.struct([]), "proposalInstructionV1");
    U.addVariant(9, B.struct([]), "mintGovernanceV1");
    U.addVariant(10, B.struct([]), "tokenGovernanceV1");
    U.addVariant(11, B.struct([]), "realmConfig");
    U.addVariant(12, B.struct([]), "voteRecordV2");
    U.addVariant(13, B.struct([]), "proposalTransactionV2");
    U.addVariant(14, B.struct([]), "proposalV2");
    U.addVariant(15, B.struct([]), "programMetadata");
    U.addVariant(16, B.struct([]), "realmV2");
    U.addVariant(17, B.struct([]), "tokenOwnerRecordV2");
    U.addVariant(18, B.struct([]), "governanceV2");
    U.addVariant(19, B.struct([]), "programGovernanceV2");
    U.addVariant(20, B.struct([]), "mintGovernanceV2");
    U.addVariant(21, B.struct([]), "tokenGovernanceV2");
    U.addVariant(22, B.struct([]), "signatoryRecordV2");
    return U;
  })("accountType"),
  B.publicKey("realm"),
  B.publicKey("governedAccount"),
  B.u32("proposalsCount"),
  B.struct(
    [
      ((p: string) => {
        const U = B.union(B.u8("discriminator"), null, p);
        U.addVariant(0, B.u8(), "yesVotePercentage");
        U.addVariant(1, B.u8(), "quorumPercentage");
        U.addVariant(2, B.struct([]), "disabled");
        return U;
      })("communityVoteThreshold"),
      B.u64("minCommunityWeightToCreateProposal"),
      B.u32("minTransactionHoldUpTime"),
      B.u32("maxVotingTime"),
      ((p: string) => {
        const U = B.union(B.u8("discriminator"), null, p);
        U.addVariant(0, B.struct([]), "strict");
        U.addVariant(1, B.struct([]), "early");
        U.addVariant(2, B.struct([]), "disabled");
        return U;
      })("voteTipping"),
      ((p: string) => {
        const U = B.union(B.u8("discriminator"), null, p);
        U.addVariant(0, B.u8(), "yesVotePercentage");
        U.addVariant(1, B.u8(), "quorumPercentage");
        U.addVariant(2, B.struct([]), "disabled");
        return U;
      })("councilVoteThreshold"),
      ((p: string) => {
        const U = B.union(B.u8("discriminator"), null, p);
        U.addVariant(0, B.u8(), "yesVotePercentage");
        U.addVariant(1, B.u8(), "quorumPercentage");
        U.addVariant(2, B.struct([]), "disabled");
        return U;
      })("councilVetoVoteThreshold"),
      B.u64("minCouncilWeightToCreateProposal"),
    ],
    "config"
  ),
  B.seq(B.u8(), 6, "reserved"),
  B.u16("votingProposalCount"),
  B.seq(B.u8(), 128, "reservedV2"),
]);

const VOTE_RECORD_V2_LAYOUT: any = B.struct([
  ((p: string) => {
    const U = B.union(B.u8("discriminator"), null, p);
    U.addVariant(0, B.struct([]), "uninitialized");
    U.addVariant(1, B.struct([]), "realmV1");
    U.addVariant(2, B.struct([]), "tokenOwnerRecordV1");
    U.addVariant(3, B.struct([]), "governanceV1");
    U.addVariant(4, B.struct([]), "programGovernanceV1");
    U.addVariant(5, B.struct([]), "proposalV1");
    U.addVariant(6, B.struct([]), "signatoryRecordV1");
    U.addVariant(7, B.struct([]), "voteRecordV1");
    U.addVariant(8, B.struct([]), "proposalInstructionV1");
    U.addVariant(9, B.struct([]), "mintGovernanceV1");
    U.addVariant(10, B.struct([]), "tokenGovernanceV1");
    U.addVariant(11, B.struct([]), "realmConfig");
    U.addVariant(12, B.struct([]), "voteRecordV2");
    U.addVariant(13, B.struct([]), "proposalTransactionV2");
    U.addVariant(14, B.struct([]), "proposalV2");
    U.addVariant(15, B.struct([]), "programMetadata");
    U.addVariant(16, B.struct([]), "realmV2");
    U.addVariant(17, B.struct([]), "tokenOwnerRecordV2");
    U.addVariant(18, B.struct([]), "governanceV2");
    U.addVariant(19, B.struct([]), "programGovernanceV2");
    U.addVariant(20, B.struct([]), "mintGovernanceV2");
    U.addVariant(21, B.struct([]), "tokenGovernanceV2");
    U.addVariant(22, B.struct([]), "signatoryRecordV2");
    return U;
  })("accountType"),
  B.publicKey("proposal"),
  B.publicKey("governingTokenOwner"),
  B.bool("isRelinquished"),
  B.u64("voterWeight"),
  ((p: string) => {
    const U = B.union(B.u8("discriminator"), null, p);
    U.addVariant(
      0,
      B.vec(B.struct([B.u8("rank"), B.u8("weightPercentage")])),
      "approve"
    );
    U.addVariant(1, B.struct([]), "deny");
    U.addVariant(2, B.struct([]), "abstain");
    U.addVariant(3, B.struct([]), "veto");
    return U;
  })("vote"),
  B.seq(B.u8(), 8, "reservedV2"),
]);

const TOKEN_OWNER_RECORD_V2_LAYOUT: any = B.struct([
  ((p: string) => {
    const U = B.union(B.u8("discriminator"), null, p);
    U.addVariant(0, B.struct([]), "uninitialized");
    U.addVariant(1, B.struct([]), "realmV1");
    U.addVariant(2, B.struct([]), "tokenOwnerRecordV1");
    U.addVariant(3, B.struct([]), "governanceV1");
    U.addVariant(4, B.struct([]), "programGovernanceV1");
    U.addVariant(5, B.struct([]), "proposalV1");
    U.addVariant(6, B.struct([]), "signatoryRecordV1");
    U.addVariant(7, B.struct([]), "voteRecordV1");
    U.addVariant(8, B.struct([]), "proposalInstructionV1");
    U.addVariant(9, B.struct([]), "mintGovernanceV1");
    U.addVariant(10, B.struct([]), "tokenGovernanceV1");
    U.addVariant(11, B.struct([]), "realmConfig");
    U.addVariant(12, B.struct([]), "voteRecordV2");
    U.addVariant(13, B.struct([]), "proposalTransactionV2");
    U.addVariant(14, B.struct([]), "proposalV2");
    U.addVariant(15, B.struct([]), "programMetadata");
    U.addVariant(16, B.struct([]), "realmV2");
    U.addVariant(17, B.struct([]), "tokenOwnerRecordV2");
    U.addVariant(18, B.struct([]), "governanceV2");
    U.addVariant(19, B.struct([]), "programGovernanceV2");
    U.addVariant(20, B.struct([]), "mintGovernanceV2");
    U.addVariant(21, B.struct([]), "tokenGovernanceV2");
    U.addVariant(22, B.struct([]), "signatoryRecordV2");
    return U;
  })("accountType"),
  B.publicKey("realm"),
  B.publicKey("governingTokenMint"),
  B.publicKey("governingTokenOwner"),
  B.u64("governingTokenDepositAmount"),
  B.u32("unrelinquishedVotesCount"),
  B.u32("totalVotesCount"),
  B.u8("outstandingProposalCount"),
  B.seq(B.u8(), 7, "reserved"),
  B.option(B.publicKey(), "governanceDelegate"),
  B.seq(B.u8(), 128, "reservedV2"),
]);

const REALM_CONFIG_ACCOUNT_LAYOUT: any = B.struct([
  ((p: string) => {
    const U = B.union(B.u8("discriminator"), null, p);
    U.addVariant(0, B.struct([]), "uninitialized");
    U.addVariant(1, B.struct([]), "realmV1");
    U.addVariant(2, B.struct([]), "tokenOwnerRecordV1");
    U.addVariant(3, B.struct([]), "governanceV1");
    U.addVariant(4, B.struct([]), "programGovernanceV1");
    U.addVariant(5, B.struct([]), "proposalV1");
    U.addVariant(6, B.struct([]), "signatoryRecordV1");
    U.addVariant(7, B.struct([]), "voteRecordV1");
    U.addVariant(8, B.struct([]), "proposalInstructionV1");
    U.addVariant(9, B.struct([]), "mintGovernanceV1");
    U.addVariant(10, B.struct([]), "tokenGovernanceV1");
    U.addVariant(11, B.struct([]), "realmConfig");
    U.addVariant(12, B.struct([]), "voteRecordV2");
    U.addVariant(13, B.struct([]), "proposalTransactionV2");
    U.addVariant(14, B.struct([]), "proposalV2");
    U.addVariant(15, B.struct([]), "programMetadata");
    U.addVariant(16, B.struct([]), "realmV2");
    U.addVariant(17, B.struct([]), "tokenOwnerRecordV2");
    U.addVariant(18, B.struct([]), "governanceV2");
    U.addVariant(19, B.struct([]), "programGovernanceV2");
    U.addVariant(20, B.struct([]), "mintGovernanceV2");
    U.addVariant(21, B.struct([]), "tokenGovernanceV2");
    U.addVariant(22, B.struct([]), "signatoryRecordV2");
    return U;
  })("accountType"),
  B.publicKey("realm"),
  B.option(B.publicKey(), "communityVoterWeightAddin"),
  B.option(B.publicKey(), "maxCommunityVoterWeightAddin"),
  B.option(B.publicKey(), "councilVoterWeightAddin"),
  B.option(B.publicKey(), "councilMaxVoteWeightAddin"),
  B.seq(B.u8(), 128, "reserved"),
]);

const PROPOSAL_TRANSACTION_V2_LAYOUT: any = B.struct([
  ((p: string) => {
    const U = B.union(B.u8("discriminator"), null, p);
    U.addVariant(0, B.struct([]), "uninitialized");
    U.addVariant(1, B.struct([]), "realmV1");
    U.addVariant(2, B.struct([]), "tokenOwnerRecordV1");
    U.addVariant(3, B.struct([]), "governanceV1");
    U.addVariant(4, B.struct([]), "programGovernanceV1");
    U.addVariant(5, B.struct([]), "proposalV1");
    U.addVariant(6, B.struct([]), "signatoryRecordV1");
    U.addVariant(7, B.struct([]), "voteRecordV1");
    U.addVariant(8, B.struct([]), "proposalInstructionV1");
    U.addVariant(9, B.struct([]), "mintGovernanceV1");
    U.addVariant(10, B.struct([]), "tokenGovernanceV1");
    U.addVariant(11, B.struct([]), "realmConfig");
    U.addVariant(12, B.struct([]), "voteRecordV2");
    U.addVariant(13, B.struct([]), "proposalTransactionV2");
    U.addVariant(14, B.struct([]), "proposalV2");
    U.addVariant(15, B.struct([]), "programMetadata");
    U.addVariant(16, B.struct([]), "realmV2");
    U.addVariant(17, B.struct([]), "tokenOwnerRecordV2");
    U.addVariant(18, B.struct([]), "governanceV2");
    U.addVariant(19, B.struct([]), "programGovernanceV2");
    U.addVariant(20, B.struct([]), "mintGovernanceV2");
    U.addVariant(21, B.struct([]), "tokenGovernanceV2");
    U.addVariant(22, B.struct([]), "signatoryRecordV2");
    return U;
  })("accountType"),
  B.publicKey("proposal"),
  B.u8("optionIndex"),
  B.u16("transactionIndex"),
  B.u32("holdUpTime"),
  B.vec(
    B.struct([
      B.publicKey("programId"),
      B.vec(
        B.struct([
          B.publicKey("pubkey"),
          B.bool("isSigner"),
          B.bool("isWritable"),
        ]),
        "accounts"
      ),
      B.bytes("data"),
    ]),
    "instructions"
  ),
  B.option(B.i64(), "executedAt"),
  ((p: string) => {
    const U = B.union(B.u8("discriminator"), null, p);
    U.addVariant(0, B.struct([]), "none");
    U.addVariant(1, B.struct([]), "success");
    U.addVariant(2, B.struct([]), "error");
    return U;
  })("executionStatus"),
  B.seq(B.u8(), 8, "reservedV2"),
]);
