import { Buffer } from "buffer";
import { getBase58Decoder } from "@solana/kit";
import { Idl, IdlDiscriminator } from "../../idl.js";
import { IdlCoder } from "./idl.js";
import { IdlCodec } from "./codecs.js";
import { AccountsCoder } from "../index.js";

/**
 * Encodes and decodes account objects.
 */
export class BorshAccountsCoder<A extends string = string>
  implements AccountsCoder
{
  /**
   * Maps account type identifier to a codec.
   */
  private accountCodecs: Map<
    A,
    { discriminator: IdlDiscriminator; codec: IdlCodec }
  >;

  public constructor(private idl: Idl) {
    if (!idl.accounts) {
      this.accountCodecs = new Map();
      return;
    }

    const types = idl.types;
    if (!types) {
      throw new Error("Accounts require `idl.types`");
    }

    const codecs = idl.accounts.map((acc) => {
      const typeDef = types.find((ty) => ty.name === acc.name);
      if (!typeDef) {
        throw new Error(`Account not found: ${acc.name}`);
      }
      return [
        acc.name as A,
        {
          discriminator: acc.discriminator,
          codec: IdlCoder.typeDefCodec({ typeDef, types }),
        },
      ] as const;
    });

    this.accountCodecs = new Map(codecs);
  }

  public async encode<T = any>(accountName: A, account: T): Promise<Buffer> {
    const entry = this.accountCodecs.get(accountName);
    if (!entry) {
      throw new Error(`Unknown account: ${accountName}`);
    }
    const accountData = Buffer.from(entry.codec.encode(account) as Uint8Array);
    const discriminator = this.accountDiscriminator(accountName);
    return Buffer.concat([discriminator, accountData]);
  }

  public decode<T = any>(accountName: A, data: Buffer): T {
    // Assert the account discriminator is correct.
    const discriminator = this.accountDiscriminator(accountName);
    if (discriminator.compare(data.subarray(0, discriminator.length))) {
      throw new Error("Invalid account discriminator");
    }
    return this.decodeUnchecked(accountName, data);
  }

  public decodeAny<T = any>(data: Buffer): T {
    for (const [name, entry] of this.accountCodecs) {
      const givenDisc = data.subarray(0, entry.discriminator.length);
      const matches = givenDisc.equals(Buffer.from(entry.discriminator));
      if (matches) return this.decodeUnchecked(name, data);
    }

    throw new Error("Account not found");
  }

  public decodeUnchecked<T = any>(accountName: A, acc: Buffer): T {
    // Chop off the discriminator before decoding.
    const discriminator = this.accountDiscriminator(accountName);
    const data = acc.subarray(discriminator.length);
    const entry = this.accountCodecs.get(accountName);
    if (!entry) {
      throw new Error(`Unknown account: ${accountName}`);
    }
    return entry.codec.decode(data) as T;
  }

  public memcmp(accountName: A, appendData?: Buffer): any {
    const discriminator = this.accountDiscriminator(accountName);
    return {
      offset: 0,
      bytes: getBase58Decoder().decode(
        appendData ? Buffer.concat([discriminator, appendData]) : discriminator
      ),
    };
  }

  public size(accountName: A): number {
    return (
      this.accountDiscriminator(accountName).length +
      IdlCoder.typeSize({ defined: { name: accountName } }, this.idl)
    );
  }

  /**
   * Get the unique discriminator prepended to all anchor accounts.
   *
   * @param name The name of the account to get the discriminator of.
   */
  public accountDiscriminator(name: string): Buffer {
    const account = this.idl.accounts?.find((acc) => acc.name === name);
    if (!account) {
      throw new Error(`Account not found: ${name}`);
    }

    return Buffer.from(account.discriminator);
  }
}
