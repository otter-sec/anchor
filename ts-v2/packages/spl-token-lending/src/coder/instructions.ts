// @ts-nocheck
import * as B from "@native-to-anchor/buffer-layout";
import { Idl, InstructionCoder } from "@anchor-lang/core";

export class SplTokenLendingInstructionCoder implements InstructionCoder {
  constructor(_idl: Idl) {}

  encode(ixName: string, ix: any): Buffer {
    switch (ixName) {
      case "initLendingMarket": {
        return encodeInitLendingMarket(ix);
      }
      case "setLendingMarketOwner": {
        return encodeSetLendingMarketOwner(ix);
      }
      case "initReserve": {
        return encodeInitReserve(ix);
      }
      case "refreshReserve": {
        return encodeRefreshReserve(ix);
      }
      case "depositReserveLiquidity": {
        return encodeDepositReserveLiquidity(ix);
      }
      case "redeemReserveCollateral": {
        return encodeRedeemReserveCollateral(ix);
      }
      case "initObligation": {
        return encodeInitObligation(ix);
      }
      case "refreshObligation": {
        return encodeRefreshObligation(ix);
      }
      case "depositObligationCollateral": {
        return encodeDepositObligationCollateral(ix);
      }
      case "withdrawObligationCollateral": {
        return encodeWithdrawObligationCollateral(ix);
      }
      case "borrowObligationLiquidity": {
        return encodeBorrowObligationLiquidity(ix);
      }
      case "repayObligationLiquidity": {
        return encodeRepayObligationLiquidity(ix);
      }
      case "liquidateObligation": {
        return encodeLiquidateObligation(ix);
      }
      case "flashLoan": {
        return encodeFlashLoan(ix);
      }

      default: {
        throw new Error(`Invalid instruction: ${ixName}`);
      }
    }
  }

  encodeState(_ixName: string, _ix: any): Buffer {
    throw new Error("SplTokenLending does not have state");
  }
}

function encodeInitLendingMarket({ owner, quoteCurrency }: any): Buffer {
  return encodeData(
    { initLendingMarket: { owner, quoteCurrency } },
    1 + 32 + 1 * 32
  );
}

function encodeSetLendingMarketOwner({ newOwner }: any): Buffer {
  return encodeData({ setLendingMarketOwner: { newOwner } }, 1 + 32);
}

function encodeInitReserve({ liquidityAmount, config }: any): Buffer {
  return encodeData(
    { initReserve: { liquidityAmount, config } },
    1 + 8 + 1 + 1 + 1 + 1 + 1 + 1 + 1 + 8 + 8 + 1
  );
}

function encodeRefreshReserve({}: any): Buffer {
  return encodeData({ refreshReserve: {} }, 1);
}

function encodeDepositReserveLiquidity({ liquidityAmount }: any): Buffer {
  return encodeData({ depositReserveLiquidity: { liquidityAmount } }, 1 + 8);
}

function encodeRedeemReserveCollateral({ collateralAmount }: any): Buffer {
  return encodeData({ redeemReserveCollateral: { collateralAmount } }, 1 + 8);
}

function encodeInitObligation({}: any): Buffer {
  return encodeData({ initObligation: {} }, 1);
}

function encodeRefreshObligation({}: any): Buffer {
  return encodeData({ refreshObligation: {} }, 1);
}

function encodeDepositObligationCollateral({ collateralAmount }: any): Buffer {
  return encodeData(
    { depositObligationCollateral: { collateralAmount } },
    1 + 8
  );
}

function encodeWithdrawObligationCollateral({ collateralAmount }: any): Buffer {
  return encodeData(
    { withdrawObligationCollateral: { collateralAmount } },
    1 + 8
  );
}

function encodeBorrowObligationLiquidity({ liquidityAmount }: any): Buffer {
  return encodeData({ borrowObligationLiquidity: { liquidityAmount } }, 1 + 8);
}

function encodeRepayObligationLiquidity({ liquidityAmount }: any): Buffer {
  return encodeData({ repayObligationLiquidity: { liquidityAmount } }, 1 + 8);
}

function encodeLiquidateObligation({ liquidityAmount }: any): Buffer {
  return encodeData({ liquidateObligation: { liquidityAmount } }, 1 + 8);
}

function encodeFlashLoan({ amount }: any): Buffer {
  return encodeData({ flashLoan: { amount } }, 1 + 8);
}

const LAYOUT = B.union(B.u8("instruction"));
LAYOUT.addVariant(
  0,
  B.struct([B.publicKey("owner"), B.seq(B.u8(), 32, "quoteCurrency")]),
  "initLendingMarket"
);
LAYOUT.addVariant(
  1,
  B.struct([B.publicKey("newOwner")]),
  "setLendingMarketOwner"
);
LAYOUT.addVariant(
  2,
  B.struct([
    B.u64("liquidityAmount"),
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
  ]),
  "initReserve"
);
LAYOUT.addVariant(3, B.struct([]), "refreshReserve");
LAYOUT.addVariant(
  4,
  B.struct([B.u64("liquidityAmount")]),
  "depositReserveLiquidity"
);
LAYOUT.addVariant(
  5,
  B.struct([B.u64("collateralAmount")]),
  "redeemReserveCollateral"
);
LAYOUT.addVariant(6, B.struct([]), "initObligation");
LAYOUT.addVariant(7, B.struct([]), "refreshObligation");
LAYOUT.addVariant(
  8,
  B.struct([B.u64("collateralAmount")]),
  "depositObligationCollateral"
);
LAYOUT.addVariant(
  9,
  B.struct([B.u64("collateralAmount")]),
  "withdrawObligationCollateral"
);
LAYOUT.addVariant(
  10,
  B.struct([B.u64("liquidityAmount")]),
  "borrowObligationLiquidity"
);
LAYOUT.addVariant(
  11,
  B.struct([B.u64("liquidityAmount")]),
  "repayObligationLiquidity"
);
LAYOUT.addVariant(
  12,
  B.struct([B.u64("liquidityAmount")]),
  "liquidateObligation"
);
LAYOUT.addVariant(13, B.struct([B.u64("amount")]), "flashLoan");

function encodeData(ix: any, span: number): Buffer {
  const b = Buffer.alloc(span);
  LAYOUT.encode(ix, b);
  return b;
}
