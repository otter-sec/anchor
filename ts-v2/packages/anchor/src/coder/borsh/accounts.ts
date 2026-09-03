import bs58 from "bs58";
import { Buffer } from "buffer";
import { Layout } from "buffer-layout";
import { Idl, IdlDiscriminator } from "../../idl.js";
import { IdlCoder } from "./idl.js";
import { AccountsCoder } from "../index.js";

/**
 * Encodes and decodes account objects.
 */
export class BorshAccountsCoder<A extends string = string>
  implements AccountsCoder
{
  /**
   * Maps account type identifier to a layout.
   */
  private accountLayouts: Map<
    A,
    { discriminator: IdlDiscriminator; layout: Layout }
  >;

  public constructor(private idl: Idl) {
    if (!idl.accounts) {
      this.accountLayouts = new Map();
      return;
    }

    const types = idl.types;
    if (!types) {
      throw new Error("Accounts require `idl.types`");
    }

    const layouts = idl.accounts.map((acc) => {
      const typeDef = types.find((ty) => ty.name === acc.name);
      if (!typeDef) {
        throw new Error(`Account not found: ${acc.name}`);
      }
      return [
        acc.name as A,
        {
          discriminator: acc.discriminator,
          layout: IdlCoder.typeDefLayout({ typeDef, types }),
        },
      ] as const;
    });

    this.accountLayouts = new Map(layouts);
  }

  public async encode<T = any>(accountName: A, account: T): Promise<Buffer> {
    const buffer = Buffer.alloc(1000); // TODO: use a tighter buffer.
    const layout = this.accountLayouts.get(accountName);
    if (!layout) {
      throw new Error(`Unknown account: ${accountName}`);
    }
    const len = layout.layout.encode(account, buffer);
    const accountData = buffer.slice(0, len);
    const discriminator = this.accountDiscriminator(accountName);
    return Buffer.concat([discriminator, accountData]);
  }

  public decode<T = any>(accountName: A, data: Buffer): T {
    // Assert the account discriminator is correct.
    const discriminator = this.accountDiscriminator(accountName);
    if (discriminator.compare(data.slice(0, discriminator.length))) {
      throw new Error("Invalid account discriminator");
    }
    return this.decodeUnchecked(accountName, data);
  }

  public decodeAny<T = any>(data: Buffer): T {
    for (const [name, layout] of this.accountLayouts) {
      const givenDisc = data.subarray(0, layout.discriminator.length);
      const matches = givenDisc.equals(Buffer.from(layout.discriminator));
      if (matches) return this.decodeUnchecked(name, data);
    }

    throw new Error("Account not found");
  }

  public decodeUnchecked<T = any>(accountName: A, acc: Buffer): T {
    // Chop off the discriminator before decoding.
    const discriminator = this.accountDiscriminator(accountName);
    const data = acc.subarray(discriminator.length);
    const layout = this.accountLayouts.get(accountName);
    if (!layout) {
      throw new Error(`Unknown account: ${accountName}`);
    }
    return layout.layout.decode(data);
  }

  public memcmp(accountName: A, appendData?: Buffer): any {
    const discriminator = this.accountDiscriminator(accountName);
    return {
      offset: 0,
      bytes: bs58.encode(
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
