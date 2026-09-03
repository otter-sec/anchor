// @ts-nocheck
import * as B from "@native-to-anchor/buffer-layout";
import { AccountsCoder, Idl } from "@anchor-lang/core";
import { IdlTypeDef } from "@anchor-lang/core/dist/cjs/idl";

export class SplTokenSwapAccountsCoder<A extends string = string>
  implements AccountsCoder
{
  constructor(_idl: Idl) {}

  public async encode<T = any>(accountName: A, account: T): Promise<Buffer> {
    switch (accountName) {
      case "swap": {
        const buffer = Buffer.alloc(324);
        const len = SWAP_LAYOUT.encode(account, buffer);
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
      case "swap": {
        return decodeSwapAccount(ix);
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
      case "swap": {
        return {
          dataSize: 324,
        };
      }
      default: {
        throw new Error(`Invalid account name: ${accountName}`);
      }
    }
  }

  public size(idlAccount: IdlTypeDef): number {
    switch (idlAccount.name) {
      case "swap": {
        return 324;
      }
      default: {
        throw new Error(`Invalid account name: ${idlAccount.name}`);
      }
    }
  }
}

function decodeSwapAccount<T = any>(ix: Buffer): T {
  return SWAP_LAYOUT.decode(ix) as T;
}

const SWAP_LAYOUT: any = B.struct([
  B.u8("version"),
  B.bool("isInitialized"),
  B.u8("bumpSeed"),
  B.publicKey("tokenProgramId"),
  B.publicKey("tokenA"),
  B.publicKey("tokenB"),
  B.publicKey("poolMint"),
  B.publicKey("tokenAMint"),
  B.publicKey("tokenBMint"),
  B.publicKey("poolFeeAccount"),
  B.struct(
    [
      B.u64("tradeFeeNumerator"),
      B.u64("tradeFeeDenominator"),
      B.u64("ownerTradeFeeNumerator"),
      B.u64("ownerTradeFeeDenominator"),
      B.u64("ownerWithdrawFeeNumerator"),
      B.u64("ownerWithdrawFeeDenominator"),
      B.u64("hostFeeNumerator"),
      B.u64("hostFeeDenominator"),
    ],
    "fees"
  ),
  B.struct(
    [
      ((p: string) => {
        const U = B.union(B.u8("discriminator"), null, p);
        U.addVariant(0, B.struct([]), "constantProduct");
        U.addVariant(1, B.struct([]), "constantPrice");
        U.addVariant(2, B.struct([]), "stable");
        U.addVariant(3, B.struct([]), "offset");
        return U;
      })("curveType"),
      B.seq(B.u8(), 32, "calculator"),
    ],
    "swapCurve"
  ),
]);
