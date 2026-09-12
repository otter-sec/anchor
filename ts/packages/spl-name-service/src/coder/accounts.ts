// @ts-nocheck
import * as B from "@native-to-anchor/buffer-layout";
import { AccountsCoder, Idl } from "@anchor-lang/core";
import { IdlTypeDef } from "@anchor-lang/core/dist/cjs/idl";

export class SplNameServiceAccountsCoder<A extends string = string>
  implements AccountsCoder
{
  constructor(_idl: Idl) {}

  public async encode<T = any>(accountName: A, account: T): Promise<Buffer> {
    switch (accountName) {
      case "nameRecordHeader": {
        const buffer = Buffer.alloc(96);
        const len = NAME_RECORD_HEADER_LAYOUT.encode(account, buffer);
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
      case "nameRecordHeader": {
        return decodeNameRecordHeaderAccount(ix);
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
      case "nameRecordHeader": {
        return {
          dataSize: 96,
        };
      }
      default: {
        throw new Error(`Invalid account name: ${accountName}`);
      }
    }
  }

  public size(idlAccount: IdlTypeDef): number {
    switch (idlAccount.name) {
      case "nameRecordHeader": {
        return 96;
      }
      default: {
        throw new Error(`Invalid account name: ${idlAccount.name}`);
      }
    }
  }
}

function decodeNameRecordHeaderAccount<T = any>(ix: Buffer): T {
  return NAME_RECORD_HEADER_LAYOUT.decode(ix) as T;
}

const NAME_RECORD_HEADER_LAYOUT: any = B.struct([
  B.publicKey("parentName"),
  B.publicKey("owner"),
  B.publicKey("class"),
]);
