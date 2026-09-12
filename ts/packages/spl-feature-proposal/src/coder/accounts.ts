// @ts-nocheck
import * as B from "@native-to-anchor/buffer-layout";
import { AccountsCoder, Idl } from "@anchor-lang/core";
import { IdlTypeDef } from "@anchor-lang/core/dist/cjs/idl";

export class SplFeatureProposalAccountsCoder<A extends string = string>
  implements AccountsCoder
{
  constructor(_idl: Idl) {}

  public async encode<T = any>(accountName: A, account: T): Promise<Buffer> {
    switch (accountName) {
      case "featureProposal": {
        const buffer = Buffer.alloc(17);
        const len = FEATURE_PROPOSAL_LAYOUT.encode(account, buffer);
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
      case "featureProposal": {
        return decodeFeatureProposalAccount(ix);
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
      case "featureProposal": {
        return {
          dataSize: 17,
        };
      }
      default: {
        throw new Error(`Invalid account name: ${accountName}`);
      }
    }
  }

  public size(idlAccount: IdlTypeDef): number {
    switch (idlAccount.name) {
      case "featureProposal": {
        return 17;
      }
      default: {
        throw new Error(`Invalid account name: ${idlAccount.name}`);
      }
    }
  }
}

function decodeFeatureProposalAccount<T = any>(ix: Buffer): T {
  return FEATURE_PROPOSAL_LAYOUT.decode(ix) as T;
}

const FEATURE_PROPOSAL_LAYOUT = B.union(B.u8("discriminator"));
FEATURE_PROPOSAL_LAYOUT.addVariant(
  0,
  B.struct([B.u64("tokensRequired"), B.i64("deadline")]),
  "uninitialized"
);
FEATURE_PROPOSAL_LAYOUT.addVariant(1, B.struct([]), "pending");
FEATURE_PROPOSAL_LAYOUT.addVariant(
  2,
  B.struct([B.u64("tokensUponAcceptance")]),
  "accepted"
);
FEATURE_PROPOSAL_LAYOUT.addVariant(3, B.struct([]), "expired");
