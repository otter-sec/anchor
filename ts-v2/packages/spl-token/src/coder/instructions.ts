// @ts-nocheck
import * as B from "@native-to-anchor/buffer-layout";
import { Idl, InstructionCoder } from "@anchor-lang/core";

export class SplTokenInstructionCoder implements InstructionCoder {
  constructor(_idl: Idl) {}

  encode(ixName: string, ix: any): Buffer {
    switch (ixName) {
      case "initializeMint": {
        return encodeInitializeMint(ix);
      }
      case "initializeAccount": {
        return encodeInitializeAccount(ix);
      }
      case "initializeMultisig": {
        return encodeInitializeMultisig(ix);
      }
      case "transfer": {
        return encodeTransfer(ix);
      }
      case "approve": {
        return encodeApprove(ix);
      }
      case "revoke": {
        return encodeRevoke(ix);
      }
      case "setAuthority": {
        return encodeSetAuthority(ix);
      }
      case "mintTo": {
        return encodeMintTo(ix);
      }
      case "burn": {
        return encodeBurn(ix);
      }
      case "closeAccount": {
        return encodeCloseAccount(ix);
      }
      case "freezeAccount": {
        return encodeFreezeAccount(ix);
      }
      case "thawAccount": {
        return encodeThawAccount(ix);
      }
      case "transferChecked": {
        return encodeTransferChecked(ix);
      }
      case "approveChecked": {
        return encodeApproveChecked(ix);
      }
      case "mintToChecked": {
        return encodeMintToChecked(ix);
      }
      case "burnChecked": {
        return encodeBurnChecked(ix);
      }
      case "initializeAccount2": {
        return encodeInitializeAccount2(ix);
      }
      case "syncNative": {
        return encodeSyncNative(ix);
      }
      case "initializeAccount3": {
        return encodeInitializeAccount3(ix);
      }
      case "initializeMultisig2": {
        return encodeInitializeMultisig2(ix);
      }
      case "initializeMint2": {
        return encodeInitializeMint2(ix);
      }
      case "getAccountDataSize": {
        return encodeGetAccountDataSize(ix);
      }
      case "initializeImmutableOwner": {
        return encodeInitializeImmutableOwner(ix);
      }
      case "amountToUiAmount": {
        return encodeAmountToUiAmount(ix);
      }
      case "uiAmountToAmount": {
        return encodeUiAmountToAmount(ix);
      }

      default: {
        throw new Error(`Invalid instruction: ${ixName}`);
      }
    }
  }

  encodeState(_ixName: string, _ix: any): Buffer {
    throw new Error("SplToken does not have state");
  }
}

function encodeInitializeMint({
  decimals,
  mintAuthority,
  freezeAuthority,
}: any): Buffer {
  return encodeData(
    { initializeMint: { decimals, mintAuthority, freezeAuthority } },
    1 + 1 + 32 + 4 + 32
  );
}

function encodeInitializeAccount({}: any): Buffer {
  return encodeData({ initializeAccount: {} }, 1);
}

function encodeInitializeMultisig({ m }: any): Buffer {
  return encodeData({ initializeMultisig: { m } }, 1 + 1);
}

function encodeTransfer({ amount }: any): Buffer {
  return encodeData({ transfer: { amount } }, 1 + 8);
}

function encodeApprove({ amount }: any): Buffer {
  return encodeData({ approve: { amount } }, 1 + 8);
}

function encodeRevoke({}: any): Buffer {
  return encodeData({ revoke: {} }, 1);
}

function encodeSetAuthority({ authorityType, newAuthority }: any): Buffer {
  return encodeData(
    { setAuthority: { authorityType, newAuthority } },
    1 +
      (() => {
        switch (Object.keys(authorityType)[0]) {
          case "mintTokens":
            return 1;
          case "freezeAccount":
            return 1;
          case "accountOwner":
            return 1;
          case "closeAccount":
            return 1;
        }
      })() +
      4 +
      32
  );
}

function encodeMintTo({ amount }: any): Buffer {
  return encodeData({ mintTo: { amount } }, 1 + 8);
}

function encodeBurn({ amount }: any): Buffer {
  return encodeData({ burn: { amount } }, 1 + 8);
}

function encodeCloseAccount({}: any): Buffer {
  return encodeData({ closeAccount: {} }, 1);
}

function encodeFreezeAccount({}: any): Buffer {
  return encodeData({ freezeAccount: {} }, 1);
}

function encodeThawAccount({}: any): Buffer {
  return encodeData({ thawAccount: {} }, 1);
}

function encodeTransferChecked({ amount, decimals }: any): Buffer {
  return encodeData({ transferChecked: { amount, decimals } }, 1 + 8 + 1);
}

function encodeApproveChecked({ amount, decimals }: any): Buffer {
  return encodeData({ approveChecked: { amount, decimals } }, 1 + 8 + 1);
}

function encodeMintToChecked({ amount, decimals }: any): Buffer {
  return encodeData({ mintToChecked: { amount, decimals } }, 1 + 8 + 1);
}

function encodeBurnChecked({ amount, decimals }: any): Buffer {
  return encodeData({ burnChecked: { amount, decimals } }, 1 + 8 + 1);
}

function encodeInitializeAccount2({ owner }: any): Buffer {
  return encodeData({ initializeAccount2: { owner } }, 1 + 32);
}

function encodeSyncNative({}: any): Buffer {
  return encodeData({ syncNative: {} }, 1);
}

function encodeInitializeAccount3({ owner }: any): Buffer {
  return encodeData({ initializeAccount3: { owner } }, 1 + 32);
}

function encodeInitializeMultisig2({ m }: any): Buffer {
  return encodeData({ initializeMultisig2: { m } }, 1 + 1);
}

function encodeInitializeMint2({
  decimals,
  mintAuthority,
  freezeAuthority,
}: any): Buffer {
  return encodeData(
    { initializeMint2: { decimals, mintAuthority, freezeAuthority } },
    1 + 1 + 32 + 4 + 32
  );
}

function encodeGetAccountDataSize({}: any): Buffer {
  return encodeData({ getAccountDataSize: {} }, 1);
}

function encodeInitializeImmutableOwner({}: any): Buffer {
  return encodeData({ initializeImmutableOwner: {} }, 1);
}

function encodeAmountToUiAmount({ amount }: any): Buffer {
  return encodeData({ amountToUiAmount: { amount } }, 1 + 8);
}

function encodeUiAmountToAmount({ uiAmount }: any): Buffer {
  return encodeData({ uiAmountToAmount: { uiAmount } }, 1);
}

const LAYOUT = B.union(B.u8("instruction"));
LAYOUT.addVariant(
  0,
  B.struct([
    B.u8("decimals"),
    B.publicKey("mintAuthority"),
    B.option(B.publicKey(), "freezeAuthority"),
  ]),
  "initializeMint"
);
LAYOUT.addVariant(1, B.struct([]), "initializeAccount");
LAYOUT.addVariant(2, B.struct([B.u8("m")]), "initializeMultisig");
LAYOUT.addVariant(3, B.struct([B.u64("amount")]), "transfer");
LAYOUT.addVariant(4, B.struct([B.u64("amount")]), "approve");
LAYOUT.addVariant(5, B.struct([]), "revoke");
LAYOUT.addVariant(
  6,
  B.struct([
    ((p: string) => {
      const U = B.union(B.u8("discriminator"), null, p);
      U.addVariant(0, B.struct([]), "mintTokens");
      U.addVariant(1, B.struct([]), "freezeAccount");
      U.addVariant(2, B.struct([]), "accountOwner");
      U.addVariant(3, B.struct([]), "closeAccount");
      return U;
    })("authorityType"),
    B.option(B.publicKey(), "newAuthority"),
  ]),
  "setAuthority"
);
LAYOUT.addVariant(7, B.struct([B.u64("amount")]), "mintTo");
LAYOUT.addVariant(8, B.struct([B.u64("amount")]), "burn");
LAYOUT.addVariant(9, B.struct([]), "closeAccount");
LAYOUT.addVariant(10, B.struct([]), "freezeAccount");
LAYOUT.addVariant(11, B.struct([]), "thawAccount");
LAYOUT.addVariant(
  12,
  B.struct([B.u64("amount"), B.u8("decimals")]),
  "transferChecked"
);
LAYOUT.addVariant(
  13,
  B.struct([B.u64("amount"), B.u8("decimals")]),
  "approveChecked"
);
LAYOUT.addVariant(
  14,
  B.struct([B.u64("amount"), B.u8("decimals")]),
  "mintToChecked"
);
LAYOUT.addVariant(
  15,
  B.struct([B.u64("amount"), B.u8("decimals")]),
  "burnChecked"
);
LAYOUT.addVariant(16, B.struct([B.publicKey("owner")]), "initializeAccount2");
LAYOUT.addVariant(17, B.struct([]), "syncNative");
LAYOUT.addVariant(18, B.struct([B.publicKey("owner")]), "initializeAccount3");
LAYOUT.addVariant(19, B.struct([B.u8("m")]), "initializeMultisig2");
LAYOUT.addVariant(
  20,
  B.struct([
    B.u8("decimals"),
    B.publicKey("mintAuthority"),
    B.option(B.publicKey(), "freezeAuthority"),
  ]),
  "initializeMint2"
);
LAYOUT.addVariant(21, B.struct([]), "getAccountDataSize");
LAYOUT.addVariant(22, B.struct([]), "initializeImmutableOwner");
LAYOUT.addVariant(23, B.struct([B.u64("amount")]), "amountToUiAmount");
LAYOUT.addVariant(24, B.struct([B.utf8Str("uiAmount")]), "uiAmountToAmount");

function encodeData(ix: any, span: number): Buffer {
  const b = Buffer.alloc(span);
  LAYOUT.encode(ix, b);
  return b;
}
