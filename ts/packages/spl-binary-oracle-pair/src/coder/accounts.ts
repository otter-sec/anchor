// @ts-nocheck
import * as B from "@native-to-anchor/buffer-layout";
import { AccountsCoder, Idl } from "@anchor-lang/core";
import { IdlTypeDef } from "@anchor-lang/core/dist/cjs/idl";

export class SplBinaryOraclePairAccountsCoder<A extends string = string>
  implements AccountsCoder
{
  constructor(_idl: Idl) {}

  public async encode<T = any>(accountName: A, account: T): Promise<Buffer> {
    switch (accountName) {
      case "pool": {
        const buffer = Buffer.alloc(179);
        const len = POOL_LAYOUT.encode(account, buffer);
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
      case "pool": {
        return decodePoolAccount(ix);
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
      case "pool": {
        return {
          dataSize: 179,
        };
      }
      default: {
        throw new Error(`Invalid account name: ${accountName}`);
      }
    }
  }

  public size(idlAccount: IdlTypeDef): number {
    switch (idlAccount.name) {
      case "pool": {
        return 179;
      }
      default: {
        throw new Error(`Invalid account name: ${idlAccount.name}`);
      }
    }
  }
}

function decodePoolAccount<T = any>(ix: Buffer): T {
  return POOL_LAYOUT.decode(ix) as T;
}

const POOL_LAYOUT: any = B.struct([
  B.u8("version"),
  B.u8("bumpSeed"),
  B.publicKey("tokenProgramId"),
  B.publicKey("depositAccount"),
  B.publicKey("tokenPassMint"),
  B.publicKey("tokenFailMint"),
  B.publicKey("decider"),
  B.u64("mintEndSlot"),
  B.u64("decideEndSlot"),
  ((p: string) => {
    const U = B.union(B.u8("discriminator"), null, p);
    U.addVariant(0, B.struct([]), "undecided");
    U.addVariant(1, B.struct([]), "pass");
    U.addVariant(2, B.struct([]), "fail");
    return U;
  })("decision"),
]);
