// @ts-nocheck
import * as B from "@native-to-anchor/buffer-layout";
import { AccountsCoder, Idl } from "@anchor-lang/core";
import { IdlTypeDef } from "@anchor-lang/core/dist/cjs/idl";

export class SplTokenAccountsCoder<A extends string = string>
  implements AccountsCoder
{
  constructor(_idl: Idl) {}

  public async encode<T = any>(accountName: A, account: T): Promise<Buffer> {
    switch (accountName) {
      case "mint": {
        const buffer = Buffer.alloc(82);
        const len = MINT_LAYOUT.encode(account, buffer);
        return buffer.slice(0, len);
      }
      case "account": {
        const buffer = Buffer.alloc(165);
        const len = ACCOUNT_LAYOUT.encode(account, buffer);
        return buffer.slice(0, len);
      }
      case "multisig": {
        const buffer = Buffer.alloc(355);
        const len = MULTISIG_LAYOUT.encode(account, buffer);
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
      case "mint": {
        return decodeMintAccount(ix);
      }
      case "account": {
        return decodeAccountAccount(ix);
      }
      case "multisig": {
        return decodeMultisigAccount(ix);
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
      case "mint": {
        return {
          dataSize: 82,
        };
      }
      case "account": {
        return {
          dataSize: 165,
        };
      }
      case "multisig": {
        return {
          dataSize: 355,
        };
      }
      default: {
        throw new Error(`Invalid account name: ${accountName}`);
      }
    }
  }

  public size(accountName: A): number {
    switch (accountName) {
      case "mint": {
        return 82;
      }
      case "account": {
        return 165;
      }
      case "multisig": {
        return 355;
      }
      default: {
        throw new Error(`Invalid account name: ${accountName}`);
      }
    }
  }
}

function decodeMintAccount<T = any>(ix: Buffer): T {
  return MINT_LAYOUT.decode(ix) as T;
}
function decodeAccountAccount<T = any>(ix: Buffer): T {
  return ACCOUNT_LAYOUT.decode(ix) as T;
}
function decodeMultisigAccount<T = any>(ix: Buffer): T {
  return MULTISIG_LAYOUT.decode(ix) as T;
}

const MINT_LAYOUT: any = B.struct([
  B.coption(B.publicKey(), "mintAuthority"),
  B.u64("supply"),
  B.u8("decimals"),
  B.bool("isInitialized"),
  B.coption(B.publicKey(), "freezeAuthority"),
]);

const ACCOUNT_LAYOUT: any = B.struct([
  B.publicKey("mint"),
  B.publicKey("owner"),
  B.u64("amount"),
  B.coption(B.publicKey(), "delegate"),
  ((p: string) => {
    const U = B.union(B.u8("discriminator"), null, p);
    U.addVariant(0, B.struct([]), "uninitialized");
    U.addVariant(1, B.struct([]), "initialized");
    U.addVariant(2, B.struct([]), "frozen");
    return U;
  })("state"),
  B.coption(B.u64(), "isNative"),
  B.u64("delegatedAmount"),
  B.coption(B.publicKey(), "closeAuthority"),
]);

const MULTISIG_LAYOUT: any = B.struct([
  B.u8("m"),
  B.u8("n"),
  B.bool("isInitialized"),
  B.seq(B.publicKey(), 11, "signers"),
]);
