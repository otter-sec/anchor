import { AccountsCoder } from "../index.js";
import { Idl } from "../../idl.js";
import * as BufferLayout from "buffer-layout";
import { NONCE_ACCOUNT_LENGTH, PublicKey } from "@solana/web3.js";
import { IdlCoder } from "../borsh/idl.js";

export class SystemAccountsCoder<A extends string = string>
  implements AccountsCoder
{
  constructor(private idl: Idl) {}

  public async encode<T = any>(accountName: A, account: T): Promise<Buffer> {
    switch (accountName) {
      case "nonce": {
        const buffer = Buffer.alloc(NONCE_ACCOUNT_LENGTH);
        const len = NONCE_ACCOUNT_LAYOUT.encode(account, buffer);
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
      case "nonce": {
        return decodeNonceAccount(ix);
      }
      default: {
        throw new Error(`Invalid account name: ${accountName}`);
      }
    }
  }

  // TODO: this won't use the appendData.
  public memcmp(accountName: A, _appendData?: Buffer): any {
    switch (accountName) {
      case "nonce": {
        return {
          dataSize: NONCE_ACCOUNT_LENGTH,
        };
      }
      default: {
        throw new Error(`Invalid account name: ${accountName}`);
      }
    }
  }

  public size(accountName: A): number {
    return IdlCoder.typeSize({ defined: { name: accountName } }, this.idl);
  }
}

function decodeNonceAccount<T = any>(ix: Buffer): T {
  return NONCE_ACCOUNT_LAYOUT.decode(ix) as T;
}

class WrappedLayout<T, U> extends BufferLayout.Layout<U> {
  layout: BufferLayout.Layout<T>;
  decoder: (data: T) => U;
  encoder: (src: U) => T;

  constructor(
    layout: BufferLayout.Layout<T>,
    decoder: (data: T) => U,
    encoder: (src: U) => T,
    property?: string
  ) {
    super(layout.span, property);
    this.layout = layout;
    this.decoder = decoder;
    this.encoder = encoder;
  }

  decode(b: Buffer, offset?: number): U {
    return this.decoder(this.layout.decode(b, offset));
  }

  encode(src: U, b: Buffer, offset?: number): number {
    return this.layout.encode(this.encoder(src), b, offset);
  }

  getSpan(b: Buffer, offset?: number): number {
    return this.layout.getSpan(b, offset);
  }
}

function publicKey(property?: string): BufferLayout.Layout<PublicKey> {
  return new WrappedLayout(
    BufferLayout.blob(32),
    (b: Buffer) => new PublicKey(b),
    (key: PublicKey) => key.toBuffer(),
    property
  );
}

const NONCE_ACCOUNT_LAYOUT = BufferLayout.struct([
  BufferLayout.u32("version"),
  BufferLayout.u32("state"),
  publicKey("authorizedPubkey"),
  publicKey("nonce"),
  BufferLayout.struct(
    [BufferLayout.nu64("lamportsPerSignature")],
    "feeCalculator"
  ),
]);
