import BN from "bn.js";
import * as BufferLayout from "buffer-layout";
import { Layout } from "buffer-layout";
import { PublicKey } from "@solana/web3.js";

function formatProperty(property?: string): string {
  return property ? ` for "${property}"` : "";
}

// The fixed-width reads below go through `BufferLayout.blob`, which slices
// with `Buffer.subarray` and so clamps at the end of the buffer instead of
// failing. Check the bytes are there first. Copied from the guard in
// `@anchor-lang/borsh`, which this package does not depend on, so the two
// report a short buffer the same way.
function assertReadableBytes(
  b: Buffer,
  offset: number,
  size: number,
  kind: string,
  property?: string
) {
  const remaining = Math.max(0, b.length - offset);
  if (offset < 0 || size < 0 || remaining < size) {
    throw new RangeError(
      `Invalid ${kind}${formatProperty(
        property
      )}: need ${size} bytes, only ${remaining} remaining bytes`
    );
  }
}

function uint64(property?: string): Layout<u64 | null> {
  return new WrappedLayout(
    BufferLayout.blob(8),
    (b: Buffer) => u64.fromBuffer(b),
    (n: u64) => n.toBuffer(),
    "integer",
    property
  );
}

function publicKey(property?: string): Layout<PublicKey> {
  return new WrappedLayout(
    BufferLayout.blob(32),
    (b: Buffer) => new PublicKey(b),
    (key: PublicKey) => key.toBuffer(),
    "pubkey",
    property
  );
}

function coption<T>(layout: Layout<T>, property?: string): Layout<T | null> {
  return new COptionLayout<T>(layout, property);
}

class WrappedLayout<T, U> extends Layout<U> {
  layout: Layout<T>;
  decoder: (data: T) => U;
  encoder: (src: U) => T;
  kind: string;

  constructor(
    layout: Layout<T>,
    decoder: (data: T) => U,
    encoder: (src: U) => T,
    kind: string,
    property?: string
  ) {
    super(layout.span, property);
    this.layout = layout;
    this.decoder = decoder;
    this.encoder = encoder;
    this.kind = kind;
  }

  decode(b: Buffer, offset = 0): U {
    assertReadableBytes(b, offset, this.layout.span, this.kind, this.property);
    return this.decoder(this.layout.decode(b, offset));
  }

  encode(src: U, b: Buffer, offset?: number): number {
    return this.layout.encode(this.encoder(src), b, offset);
  }

  getSpan(b: Buffer, offset?: number): number {
    return this.layout.getSpan(b, offset);
  }
}

// `BufferLayout.union` reads its discriminator with `u8`, which does check the
// bounds but reports a short buffer as Node's own `ERR_OUT_OF_RANGE`, naming
// neither the field nor the shortfall. Guard it like the reads above.
class GuardedLayout<T> extends Layout<T> {
  layout: Layout<T>;
  size: number;
  kind: string;

  constructor(
    layout: Layout<T>,
    size: number,
    kind: string,
    property?: string
  ) {
    super(layout.span, property);
    this.layout = layout;
    this.size = size;
    this.kind = kind;
  }

  decode(b: Buffer, offset = 0): T {
    assertReadableBytes(b, offset, this.size, this.kind, this.property);
    return this.layout.decode(b, offset);
  }

  encode(src: T, b: Buffer, offset?: number): number {
    return this.layout.encode(src, b, offset);
  }

  getSpan(b: Buffer, offset?: number): number {
    return this.layout.getSpan(b, offset);
  }
}

class COptionLayout<T> extends Layout<T | null> {
  layout: Layout<T>;
  discriminator: Layout<number>;

  constructor(layout: Layout<T>, property?: string) {
    super(-1, property);
    this.layout = layout;
    this.discriminator = BufferLayout.u32();
  }

  encode(src: T | null, b: Buffer, offset = 0): number {
    if (src === null || src === undefined) {
      return this.layout.span + this.discriminator.encode(0, b, offset);
    }
    this.discriminator.encode(1, b, offset);
    return this.layout.encode(src, b, offset + 4) + 4;
  }

  decode(b: Buffer, offset = 0): T | null {
    assertReadableBytes(
      b,
      offset,
      this.discriminator.span,
      "coption",
      this.property
    );
    const discriminator = this.discriminator.decode(b, offset);
    if (discriminator === 0) {
      // A `None` still occupies the value bytes on chain, so a buffer that
      // stops short of them is truncated even though nothing reads them.
      assertReadableBytes(
        b,
        offset + this.discriminator.span,
        this.layout.span,
        "coption",
        this.property
      );
      return null;
    } else if (discriminator === 1) {
      return this.layout.decode(b, offset + 4);
    }
    throw new Error("Invalid coption " + this.layout.property);
  }

  getSpan(b: Buffer, offset = 0): number {
    return this.layout.getSpan(b, offset + 4) + 4;
  }
}

class u64 extends BN {
  /**
   * Convert to Buffer representation
   */
  toBuffer(): Buffer {
    const a = super.toArray().reverse();
    const b = Buffer.from(a);
    if (b.length === 8) {
      return b;
    }
    if (b.length >= 8) {
      throw new Error("u64 too large");
    }

    const zeroPad = Buffer.alloc(8);
    b.copy(zeroPad);
    return zeroPad;
  }

  /**
   * Construct a u64 from Buffer representation
   */
  static fromBuffer(buffer: Buffer): u64 {
    if (buffer.length !== 8) {
      throw new Error(`Invalid buffer length: ${buffer.length}`);
    }
    return new u64(
      [...buffer]
        .reverse()
        .map((i) => `00${i.toString(16)}`.slice(-2))
        .join(""),
      16
    );
  }
}

// The base layout is 165 bytes. The guards above are per field rather than a
// check on the whole buffer, so a Token-2022 account that carries extension
// bytes after the base layout still decodes.
const TOKEN_ACCOUNT_LAYOUT = BufferLayout.struct([
  publicKey("mint"),
  publicKey("owner"),
  uint64("amount"),
  coption(publicKey("delegate"), "delegate"),
  ((p: string) => {
    const U = BufferLayout.union(BufferLayout.u8("discriminator"), null, p);
    U.addVariant(0, BufferLayout.struct([]), "uninitialized");
    U.addVariant(1, BufferLayout.struct([]), "initialized");
    U.addVariant(2, BufferLayout.struct([]), "frozen");
    return new GuardedLayout(U, BufferLayout.u8().span, "enum", p);
  })("state"),
  coption(uint64("isNative"), "isNative"),
  uint64("delegatedAmount"),
  coption(publicKey("closeAuthority"), "closeAuthority"),
]);

export function decodeTokenAccount(b: Buffer) {
  return TOKEN_ACCOUNT_LAYOUT.decode(b);
}
