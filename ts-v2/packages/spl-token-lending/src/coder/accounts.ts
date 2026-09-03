// @ts-nocheck
import * as B from "@native-to-anchor/buffer-layout";
import { AccountsCoder, Idl } from "@anchor-lang/core";
import { IdlTypeDef } from "@anchor-lang/core/dist/cjs/idl";

export class SplTokenLendingAccountsCoder<A extends string = string>
  implements AccountsCoder
{
  constructor(_idl: Idl) {}

  public async encode<T = any>(accountName: A, account: T): Promise<Buffer> {
    switch (accountName) {
      case "obligation": {
        const buffer = Buffer.alloc(916);
        const len = OBLIGATION_LAYOUT.encode(account, buffer);
        return buffer.slice(0, len);
      }
      case "lendingMarket": {
        const buffer = Buffer.alloc(258);
        const len = LENDING_MARKET_LAYOUT.encode(account, buffer);
        return buffer.slice(0, len);
      }
      case "reserve": {
        const buffer = Buffer.alloc(571);
        const len = RESERVE_LAYOUT.encode(account, buffer);
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
      case "obligation": {
        return decodeObligationAccount(ix);
      }
      case "lendingMarket": {
        return decodeLendingMarketAccount(ix);
      }
      case "reserve": {
        return decodeReserveAccount(ix);
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
      case "obligation": {
        return {
          dataSize: 916,
        };
      }
      case "lendingMarket": {
        return {
          dataSize: 258,
        };
      }
      case "reserve": {
        return {
          dataSize: 571,
        };
      }
      default: {
        throw new Error(`Invalid account name: ${accountName}`);
      }
    }
  }

  public size(idlAccount: IdlTypeDef): number {
    switch (idlAccount.name) {
      case "obligation": {
        return 916;
      }
      case "lendingMarket": {
        return 258;
      }
      case "reserve": {
        return 571;
      }
      default: {
        throw new Error(`Invalid account name: ${idlAccount.name}`);
      }
    }
  }
}

function decodeObligationAccount<T = any>(ix: Buffer): T {
  return OBLIGATION_LAYOUT.decode(ix) as T;
}
function decodeLendingMarketAccount<T = any>(ix: Buffer): T {
  return LENDING_MARKET_LAYOUT.decode(ix) as T;
}
function decodeReserveAccount<T = any>(ix: Buffer): T {
  return RESERVE_LAYOUT.decode(ix) as T;
}

const OBLIGATION_LAYOUT: any = B.struct([
  B.u8("version"),
  B.struct([B.u64("slot"), B.bool("stale")], "lastUpdate"),
  B.publicKey("lendingMarket"),
  B.publicKey("owner"),
  B.vec(
    B.struct([
      B.publicKey("depositReserve"),
      B.u64("depositedAmount"),
      B.decimal("marketValue"),
    ]),
    "deposits"
  ),
  B.vec(
    B.struct([
      B.publicKey("borrowReserve"),
      B.decimal("cumulativeBorrowRateWads"),
      B.decimal("borrowedAmountWads"),
      B.decimal("marketValue"),
    ]),
    "borrows"
  ),
  B.decimal("depositedValue"),
  B.decimal("borrowedValue"),
  B.decimal("allowedBorrowValue"),
  B.decimal("unhealthyBorrowValue"),
]);

const LENDING_MARKET_LAYOUT: any = B.struct([
  B.u8("version"),
  B.u8("bumpSeed"),
  B.publicKey("owner"),
  B.seq(B.u8(), 32, "quoteCurrency"),
  B.publicKey("tokenProgramId"),
  B.publicKey("oracleProgramId"),
  B.blob(128, "padding"),
]);

const RESERVE_LAYOUT: any = B.struct([
  B.u8("version"),
  B.struct([B.u64("slot"), B.bool("stale")], "lastUpdate"),
  B.publicKey("lendingMarket"),
  B.struct(
    [
      B.publicKey("mintPubkey"),
      B.u8("mintDecimals"),
      B.publicKey("supplyPubkey"),
      B.publicKey("feeReceiver"),
      B.publicKey("oraclePubkey"),
      B.u64("availableAmount"),
      B.decimal("borrowedAmountWads"),
      B.decimal("cumulativeBorrowRateWads"),
      B.decimal("marketPrice"),
    ],
    "liquidity"
  ),
  B.struct(
    [
      B.publicKey("mintPubkey"),
      B.u64("mintTotalSupply"),
      B.publicKey("supplyPubkey"),
    ],
    "collateral"
  ),
  B.struct(
    [
      B.u8("optimalUtilizationRate"),
      B.u8("loanToValueRatio"),
      B.u8("liquidationBonus"),
      B.u8("liquidationThreshold"),
      B.u8("minBorrowRate"),
      B.u8("optimalBorrowRate"),
      B.u8("maxBorrowRate"),
      B.struct(
        [
          B.u64("borrowFeeWad"),
          B.u64("flashLoanFeeWad"),
          B.u8("hostFeePercentage"),
        ],
        "fees"
      ),
    ],
    "config"
  ),
  B.blob(248, "padding"),
]);
