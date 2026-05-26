import {
  blob,
  Layout as LayoutCls,
  offset,
  seq,
  struct,
  u32,
  u8,
  union,
} from "buffer-layout";
import { PublicKey } from "@solana/web3.js";
import BN from "bn.js";

export {
  u8,
  s8 as i8,
  u16,
  s16 as i16,
  u32,
  s32 as i32,
  f32,
  f64,
  struct,
} from "buffer-layout";

export interface Layout<T> {
  span: number;
  property?: string;

  decode(b: Buffer, offset?: number): T;

  encode(src: T, b: Buffer, offset?: number): number;

  getSpan(b: Buffer, offset?: number): number;

  replicate(name: string): this;
}

class BNLayout extends LayoutCls<BN> {
  blob: Layout<Buffer>;
  signed: boolean;

  constructor(span: number, signed: boolean, property?: string) {
    super(span, property);
    this.blob = blob(span);
    this.signed = signed;
  }

  decode(b: Buffer, offset = 0) {
    const num = new BN(this.blob.decode(b, offset), 10, "le");
    if (this.signed) {
      return num.fromTwos(this.span * 8).clone();
    }
    return num;
  }

  encode(src: BN, b: Buffer, offset = 0) {
    if (this.signed) {
      src = src.toTwos(this.span * 8);
    }
    return this.blob.encode(
      src.toArrayLike(Buffer, "le", this.span),
      b,
      offset
    );
  }
}

export function u64(property?: string): Layout<BN> {
  return new BNLayout(8, false, property);
}

export function i64(property?: string): Layout<BN> {
  return new BNLayout(8, true, property);
}

export function u128(property?: string): Layout<BN> {
  return new BNLayout(16, false, property);
}

export function i128(property?: string): Layout<BN> {
  return new BNLayout(16, true, property);
}

export function u256(property?: string): Layout<BN> {
  return new BNLayout(32, false, property);
}

export function i256(property?: string): Layout<BN> {
  return new BNLayout(32, true, property);
}

class WrappedLayout<T, U> extends LayoutCls<U> {
  layout: Layout<T>;
  decoder: (data: T) => U;
  encoder: (src: U) => T;

  constructor(
    layout: Layout<T>,
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

export function publicKey(property?: string): Layout<PublicKey> {
  return new WrappedLayout(
    blob(32),
    (b: Buffer) => new PublicKey(b),
    (key: PublicKey) => key.toBuffer(),
    property
  );
}

const OPTION_SOME: unique symbol = Symbol.for(
  "@anchor-lang/borsh.option.some"
) as never;

export type Some<T> = {
  readonly [OPTION_SOME]: true;
  readonly value: T;
};

export type Option<T> = Some<T> | T | null;

export function some<T>(value: T): Some<T> {
  return {
    [OPTION_SOME]: true,
    value,
  };
}

export function isSome<T>(src: unknown): src is Some<T> {
  return typeof src === "object" && src !== null && OPTION_SOME in src;
}

class OptionLayout<T> extends LayoutCls<Option<T>> {
  layout: Layout<T>;
  discriminator: Layout<number>;

  constructor(layout: Layout<T>, property?: string) {
    super(-1, property);
    this.layout = layout;
    this.discriminator = u8();
  }

  encode(src: Option<T> | undefined, b: Buffer, offset = 0): number {
    if (isSome(src)) {
      this.discriminator.encode(1, b, offset);
      return this.layout.encode(src.value, b, offset + 1) + 1;
    }
    if (src === null || src === undefined) {
      return this.discriminator.encode(0, b, offset);
    }
    this.discriminator.encode(1, b, offset);
    return this.layout.encode(src, b, offset + 1) + 1;
  }

  decode(b: Buffer, offset = 0): Some<T> | null {
    const discriminator = this.discriminator.decode(b, offset);
    if (discriminator === 0) {
      return null;
    } else if (discriminator === 1) {
      return some(this.layout.decode(b, offset + 1));
    }
    throw new Error("Invalid option " + this.property);
  }

  getSpan(b: Buffer, offset = 0): number {
    const discriminator = this.discriminator.decode(b, offset);
    if (discriminator === 0) {
      return 1;
    } else if (discriminator === 1) {
      return this.layout.getSpan(b, offset + 1) + 1;
    }
    throw new Error("Invalid option " + this.property);
  }
}

export function option<T>(
  layout: Layout<T>,
  property?: string
): Layout<Option<T>> {
  return new OptionLayout<T>(layout, property);
}

// C-style Option: 4-byte LE u32 tag (0 = None, 1 = Some) followed by the
// payload. Used by native Solana account layouts (e.g. SPL Mint/Account)
// and by programs that want wire compatibility with them.
class COptionLayout<T> extends LayoutCls<T | null> {
  layout: Layout<T>;
  discriminator: Layout<number>;

  constructor(layout: Layout<T>, property?: string) {
    super(-1, property);
    this.layout = layout;
    this.discriminator = u32();
  }

  encode(src: T | null, b: Buffer, offset = 0): number {
    if (src === null || src === undefined) {
      this.discriminator.encode(0, b, offset);
      // Still reserve the payload slot for fixed-width inners so downstream
      // offsets line up; variable-width inners (span < 0) contribute 0.
      const payloadSpan = this.layout.span >= 0 ? this.layout.span : 0;
      return 4 + payloadSpan;
    }
    this.discriminator.encode(1, b, offset);
    return this.layout.encode(src, b, offset + 4) + 4;
  }

  decode(b: Buffer, offset = 0): T | null {
    const discriminator = this.discriminator.decode(b, offset);
    if (discriminator === 0) {
      return null;
    } else if (discriminator === 1) {
      return this.layout.decode(b, offset + 4);
    }
    throw new Error("Invalid coption " + this.property);
  }

  getSpan(b: Buffer, offset = 0): number {
    const discriminator = this.discriminator.decode(b, offset);
    if (discriminator === 0) {
      return 4;
    } else if (discriminator === 1) {
      return this.layout.getSpan(b, offset + 4) + 4;
    }
    throw new Error("Invalid coption " + this.property);
  }
}

export function coption<T>(
  layout: Layout<T>,
  property?: string
): Layout<T | null> {
  return new COptionLayout<T>(layout, property);
}

export function bool(property?: string): Layout<boolean> {
  return new WrappedLayout(u8(), decodeBool, encodeBool, property);
}

function decodeBool(value: number): boolean {
  if (value === 0) {
    return false;
  } else if (value === 1) {
    return true;
  }
  throw new Error("Invalid bool: " + value);
}

function encodeBool(value: boolean): number {
  return value ? 1 : 0;
}

export function vec<T>(
  elementLayout: Layout<T>,
  property?: string
): Layout<T[]> {
  const length = u32("length");
  const layout: Layout<{ values: T[] }> = struct([
    length,
    seq(elementLayout, offset(length, -length.span), "values"),
  ]);
  return new WrappedLayout(
    layout,
    ({ values }) => values,
    (values) => ({ values }),
    property
  );
}

export function tagged<T>(
  tag: BN,
  layout: Layout<T>,
  property?: string
): Layout<T> {
  const wrappedLayout: Layout<{ tag: BN; data: T }> = struct([
    u64("tag"),
    layout.replicate("data"),
  ]);

  function decodeTag({ tag: receivedTag, data }: { tag: BN; data: T }) {
    if (!receivedTag.eq(tag)) {
      throw new Error(
        "Invalid tag, expected: " +
          tag.toString("hex") +
          ", got: " +
          receivedTag.toString("hex")
      );
    }
    return data;
  }

  return new WrappedLayout(
    wrappedLayout,
    decodeTag,
    (data) => ({ tag, data }),
    property
  );
}

export function vecU8(property?: string): Layout<Buffer> {
  const length = u32("length");
  const layout: Layout<{ data: Buffer }> = struct([
    length,
    blob(offset(length, -length.span), "data"),
  ]);
  return new WrappedLayout(
    layout,
    ({ data }) => data,
    (data) => ({ data }),
    property
  );
}

export function str(property?: string): Layout<string> {
  return new WrappedLayout(
    vecU8(),
    (data) => data.toString("utf-8"),
    (s) => Buffer.from(s, "utf-8"),
    property
  );
}

export interface EnumLayout<T> extends Layout<T> {
  registry: Record<string, Layout<any>>;
}

export function rustEnum<T>(
  variants: Layout<any>[],
  property?: string,
  discriminant?: Layout<any>
): EnumLayout<T> {
  const unionLayout = union(discriminant ?? u8(), property);
  variants.forEach((variant, index) =>
    unionLayout.addVariant(index, variant, variant.property)
  );
  return unionLayout;
}

export function array<T>(
  elementLayout: Layout<T>,
  length: number,
  property?: string
): Layout<T[]> {
  const layout: Layout<{ values: T[] }> = struct([
    seq(elementLayout, length, "values"),
  ]);
  return new WrappedLayout(
    layout,
    ({ values }) => values,
    (values) => ({ values }),
    property
  );
}

class MapEntryLayout<K, V> extends LayoutCls<[K, V]> {
  keyLayout: Layout<K>;
  valueLayout: Layout<V>;

  constructor(keyLayout: Layout<K>, valueLayout: Layout<V>, property?: string) {
    super(keyLayout.span + valueLayout.span, property);
    this.keyLayout = keyLayout;
    this.valueLayout = valueLayout;
  }

  decode(b: Buffer, offset?: number): [K, V] {
    offset = offset || 0;
    const key = this.keyLayout.decode(b, offset);
    const value = this.valueLayout.decode(
      b,
      offset + this.keyLayout.getSpan(b, offset)
    );
    return [key, value];
  }

  encode(src: [K, V], b: Buffer, offset?: number): number {
    offset = offset || 0;
    const keyBytes = this.keyLayout.encode(src[0], b, offset);
    const valueBytes = this.valueLayout.encode(src[1], b, offset + keyBytes);
    return keyBytes + valueBytes;
  }

  getSpan(b: Buffer, offset?: number): number {
    return (
      this.keyLayout.getSpan(b, offset) + this.valueLayout.getSpan(b, offset)
    );
  }
}

export function map<K, V>(
  keyLayout: Layout<K>,
  valueLayout: Layout<V>,
  property?: string
): Layout<Map<K, V>> {
  const length = u32("length");
  const layout: Layout<{ values: [K, V][] }> = struct([
    length,
    seq(
      new MapEntryLayout(keyLayout, valueLayout),
      offset(length, -length.span),
      "values"
    ),
  ]);
  return new WrappedLayout(
    layout,
    ({ values }) => new Map(values),
    (values) => ({ values: Array.from(values.entries()) }),
    property
  );
}
