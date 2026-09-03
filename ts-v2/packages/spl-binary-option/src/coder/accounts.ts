// @ts-nocheck
import * as B from "@native-to-anchor/buffer-layout";
import { AccountsCoder, Idl } from "@anchor-lang/core";
import { IdlTypeDef } from "@anchor-lang/core/dist/cjs/idl";

export class SplBinaryOptionAccountsCoder<A extends string = string>
  implements AccountsCoder
{
  constructor(_idl: Idl) {}

  public async encode<T = any>(accountName: A, account: T): Promise<Buffer> {
    switch (accountName) {
      case "binaryOption": {
        const buffer = Buffer.alloc(202);
        const len = BINARY_OPTION_LAYOUT.encode(account, buffer);
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
      case "binaryOption": {
        return decodeBinaryOptionAccount(ix);
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
      case "binaryOption": {
        return {
          dataSize: 202,
        };
      }
      default: {
        throw new Error(`Invalid account name: ${accountName}`);
      }
    }
  }

  public size(idlAccount: IdlTypeDef): number {
    switch (idlAccount.name) {
      case "binaryOption": {
        return 202;
      }
      default: {
        throw new Error(`Invalid account name: ${idlAccount.name}`);
      }
    }
  }
}

function decodeBinaryOptionAccount<T = any>(ix: Buffer): T {
  return BINARY_OPTION_LAYOUT.decode(ix) as T;
}

const BINARY_OPTION_LAYOUT: any = B.struct([
  B.u8("decimals"),
  B.u64("circulation"),
  B.bool("settled"),
  B.publicKey("escrowMintAccountPubkey"),
  B.publicKey("escrowAccountPubkey"),
  B.publicKey("longMintAccountPubkey"),
  B.publicKey("shortMintAccountPubkey"),
  B.publicKey("owner"),
  B.publicKey("winningSidePubkey"),
]);
