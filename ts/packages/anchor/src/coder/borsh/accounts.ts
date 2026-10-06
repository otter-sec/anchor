import {
  getBase58Decoder,
  getBytesEncoder,
  getConstantEncoder,
  getHiddenPrefixEncoder,
  ReadonlyUint8Array,
} from "@solana/kit";
import { Idl } from "../../idl.js";
import { IdlCoder } from "./idl.js";
import { DiscriminatedIdlCodec, getDiscriminatedIdlCodec } from "./codecs.js";
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
  private accountCodecs: Map<A, DiscriminatedIdlCodec>;

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
        getDiscriminatedIdlCodec(
          acc.discriminator,
          IdlCoder.typeDefCodec({ typeDef, types })
        ),
      ] as const;
    });

    this.accountCodecs = new Map(codecs);
  }

  public async encode<T = any>(
    accountName: A,
    account: T
  ): Promise<ReadonlyUint8Array> {
    return this.codec(accountName).encode(account);
  }

  public decode<T = any>(accountName: A, data: ReadonlyUint8Array): T {
    // Assert the account discriminator is correct.
    const codec = this.codec(accountName);
    if (!codec.matches(data)) {
      throw new Error("Invalid account discriminator");
    }
    return codec.decode(data) as T;
  }

  public decodeAny<T = any>(data: ReadonlyUint8Array): T {
    for (const codec of this.accountCodecs.values()) {
      if (codec.matches(data)) {
        return codec.decode(data) as T;
      }
    }

    throw new Error("Account not found");
  }

  public decodeUnchecked<T = any>(accountName: A, data: ReadonlyUint8Array): T {
    const codec = this.codec(accountName);
    return codec.item.decode(data, codec.discriminator.length) as T;
  }

  public memcmp(accountName: A, appendData?: ReadonlyUint8Array): any {
    const discriminator = this.accountDiscriminator(accountName);
    const bytes = appendData
      ? getHiddenPrefixEncoder(getBytesEncoder(), [
          getConstantEncoder(discriminator),
        ]).encode(appendData)
      : discriminator;
    return { offset: 0, bytes: getBase58Decoder().decode(bytes) };
  }

  public size(accountName: A): number {
    return (
      this.accountDiscriminator(accountName).length +
      IdlCoder.typeSize({ defined: { name: accountName } }, this.idl)
    );
  }

  /**
   * Get the unique discriminator prepended to all anchor accounts, as a
   * fresh copy: the coder keeps using its own.
   *
   * @param name The name of the account to get the discriminator of.
   */
  public accountDiscriminator(name: string): ReadonlyUint8Array {
    return new Uint8Array(this.codec(name as A).discriminator);
  }

  private codec(accountName: A): DiscriminatedIdlCodec {
    const codec = this.accountCodecs.get(accountName);
    if (!codec) {
      throw new Error(`Account not found: ${accountName}`);
    }
    return codec;
  }
}
