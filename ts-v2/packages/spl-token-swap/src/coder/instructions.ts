// @ts-nocheck
import * as B from "@native-to-anchor/buffer-layout";
import { Idl, InstructionCoder } from "@anchor-lang/core";

export class SplTokenSwapInstructionCoder implements InstructionCoder {
  constructor(_idl: Idl) {}

  encode(ixName: string, ix: any): Buffer {
    switch (ixName) {
      case "initialize": {
        return encodeInitialize(ix);
      }
      case "swap": {
        return encodeSwap(ix);
      }
      case "depositAllTokenTypes": {
        return encodeDepositAllTokenTypes(ix);
      }
      case "withdrawAllTokenTypes": {
        return encodeWithdrawAllTokenTypes(ix);
      }
      case "depositSingleTokenTypeExactAmountIn": {
        return encodeDepositSingleTokenTypeExactAmountIn(ix);
      }
      case "withdrawSingleTokenTypeExactAmountOut": {
        return encodeWithdrawSingleTokenTypeExactAmountOut(ix);
      }

      default: {
        throw new Error(`Invalid instruction: ${ixName}`);
      }
    }
  }

  encodeState(_ixName: string, _ix: any): Buffer {
    throw new Error("SplTokenSwap does not have state");
  }
}

function encodeInitialize({ fees, swapCurve }: any): Buffer {
  return encodeData(
    { initialize: { fees, swapCurve } },
    1 +
      8 +
      8 +
      8 +
      8 +
      8 +
      8 +
      8 +
      8 +
      (() => {
        switch (Object.keys(swapCurve.curveType)[0]) {
          case "constantProduct":
            return 1;
          case "constantPrice":
            return 1;
          case "stable":
            return 1;
          case "offset":
            return 1;
        }
      })() +
      1 * 32
  );
}

function encodeSwap({ amountIn, minimumAmountOut }: any): Buffer {
  return encodeData({ swap: { amountIn, minimumAmountOut } }, 1 + 8 + 8);
}

function encodeDepositAllTokenTypes({
  poolTokenAmount,
  maximumTokenAAmount,
  maximumTokenBAmount,
}: any): Buffer {
  return encodeData(
    {
      depositAllTokenTypes: {
        poolTokenAmount,
        maximumTokenAAmount,
        maximumTokenBAmount,
      },
    },
    1 + 8 + 8 + 8
  );
}

function encodeWithdrawAllTokenTypes({
  poolTokenAmount,
  minimumTokenAAmount,
  minimumTokenBAmount,
}: any): Buffer {
  return encodeData(
    {
      withdrawAllTokenTypes: {
        poolTokenAmount,
        minimumTokenAAmount,
        minimumTokenBAmount,
      },
    },
    1 + 8 + 8 + 8
  );
}

function encodeDepositSingleTokenTypeExactAmountIn({
  sourceTokenAmount,
  minimumPoolTokenAmount,
}: any): Buffer {
  return encodeData(
    {
      depositSingleTokenTypeExactAmountIn: {
        sourceTokenAmount,
        minimumPoolTokenAmount,
      },
    },
    1 + 8 + 8
  );
}

function encodeWithdrawSingleTokenTypeExactAmountOut({
  destinationTokenAmount,
  maximumPoolTokenAmount,
}: any): Buffer {
  return encodeData(
    {
      withdrawSingleTokenTypeExactAmountOut: {
        destinationTokenAmount,
        maximumPoolTokenAmount,
      },
    },
    1 + 8 + 8
  );
}

const LAYOUT = B.union(B.u8("instruction"));
LAYOUT.addVariant(
  0,
  B.struct([
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
  ]),
  "initialize"
);
LAYOUT.addVariant(
  1,
  B.struct([B.u64("amountIn"), B.u64("minimumAmountOut")]),
  "swap"
);
LAYOUT.addVariant(
  2,
  B.struct([
    B.u64("poolTokenAmount"),
    B.u64("maximumTokenAAmount"),
    B.u64("maximumTokenBAmount"),
  ]),
  "depositAllTokenTypes"
);
LAYOUT.addVariant(
  3,
  B.struct([
    B.u64("poolTokenAmount"),
    B.u64("minimumTokenAAmount"),
    B.u64("minimumTokenBAmount"),
  ]),
  "withdrawAllTokenTypes"
);
LAYOUT.addVariant(
  4,
  B.struct([B.u64("sourceTokenAmount"), B.u64("minimumPoolTokenAmount")]),
  "depositSingleTokenTypeExactAmountIn"
);
LAYOUT.addVariant(
  5,
  B.struct([B.u64("destinationTokenAmount"), B.u64("maximumPoolTokenAmount")]),
  "withdrawSingleTokenTypeExactAmountOut"
);

function encodeData(ix: any, span: number): Buffer {
  const b = Buffer.alloc(span);
  LAYOUT.encode(ix, b);
  return b;
}
