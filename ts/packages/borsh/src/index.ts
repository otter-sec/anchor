import {
  blob,
  Layout as LayoutCls,
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

class OptionLayout<T> extends LayoutCls<T | null> {
  layout: Layout<T>;
  discriminator: Layout<number>;

  constructor(layout: Layout<T>, property?: string) {
    super(-1, property);
    this.layout = layout;
    this.discriminator = u8();
  }

  encode(src: T | null, b: Buffer, offset = 0): number {
    if (src === null || src === undefined) {
      return this.discriminator.encode(0, b, offset);
    }
    this.discriminator.encode(1, b, offset);
    return this.layout.encode(src, b, offset + 1) + 1;
  }

  decode(b: Buffer, offset = 0): T | null {
    const discriminator = this.discriminator.decode(b, offset);
    if (discriminator === 0) {
      return null;
    } else if (discriminator === 1) {
      return this.layout.decode(b, offset + 1);
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
): Layout<T | null> {
  return new OptionLayout<T>(layout, property);
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

const U32_SPAN = 4;
const MAX_U32 = 0xffffffff;

function formatProperty(property?: string): string {
  return property ? ` for "${property}"` : "";
}

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

function assertWritableLength(
  length: number,
  kind: string,
  property?: string
): void {
  if (!Number.isSafeInteger(length) || length < 0 || length > MAX_U32) {
    throw new RangeError(
      `Invalid ${kind}${formatProperty(
        property
      )}: length ${length} is outside the supported u32 range`
    );
  }
}

function readU32Length(
  b: Buffer,
  offset: number,
  kind: string,
  property?: string
): number {
  assertReadableBytes(b, offset, U32_SPAN, kind, property);
  return u32().decode(b, offset);
}

function assertCollectionFitsRemaining<T>(
  count: number,
  elementLayout: Layout<T>,
  remainingBytes: number,
  kind: string,
  property?: string
) {
  if (elementLayout.span > 0) {
    const requiredBytes = count * elementLayout.span;
    if (
      !Number.isSafeInteger(requiredBytes) ||
      requiredBytes > remainingBytes
    ) {
      throw new RangeError(
        `Invalid ${kind}${formatProperty(
          property
        )}: length ${count} requires ${requiredBytes} bytes, only ${remainingBytes} remaining bytes`
      );
    }
    return;
  }

  // Dynamic layouts must consume at least one byte per element in all supported
  // Borsh collection element shapes. This rejects impossible counts before any
  // allocation or iteration.
  if (elementLayout.span < 0 && count > remainingBytes) {
    throw new RangeError(
      `Invalid ${kind}${formatProperty(
        property
      )}: length ${count} exceeds ${remainingBytes} remaining bytes`
    );
  }
}

function decodeCollectionValues<T>(
  count: number,
  elementLayout: Layout<T>,
  b: Buffer,
  offset: number,
  kind: string,
  property?: string
): { values: T[]; span: number } {
  const remainingBytes = Math.max(0, b.length - offset);
  assertCollectionFitsRemaining(
    count,
    elementLayout,
    remainingBytes,
    kind,
    property
  );

  const values: T[] = [];
  let cursor = offset;

  for (let i = 0; i < count; i += 1) {
    const value = elementLayout.decode(b, cursor);
    const span = elementLayout.getSpan(b, cursor);
    if (!Number.isSafeInteger(span) || span < 0) {
      throw new RangeError(
        `Invalid ${kind}${formatProperty(
          property
        )}: element ${i} has invalid span ${span}`
      );
    }

    cursor += span;
    if (cursor > b.length) {
      throw new RangeError(
        `Invalid ${kind}${formatProperty(
          property
        )}: decoded past the end of the buffer`
      );
    }

    values.push(value);
  }

  return { values, span: cursor - offset };
}

function measureCollectionSpan<T>(
  count: number,
  elementLayout: Layout<T>,
  b: Buffer,
  offset: number,
  kind: string,
  property?: string
): number {
  const remainingBytes = Math.max(0, b.length - offset);
  assertCollectionFitsRemaining(
    count,
    elementLayout,
    remainingBytes,
    kind,
    property
  );

  if (elementLayout.span > 0) {
    return count * elementLayout.span;
  }

  let cursor = offset;
  for (let i = 0; i < count; i += 1) {
    const span = elementLayout.getSpan(b, cursor);
    if (!Number.isSafeInteger(span) || span < 0) {
      throw new RangeError(
        `Invalid ${kind}${formatProperty(
          property
        )}: element ${i} has invalid span ${span}`
      );
    }

    cursor += span;
    if (cursor > b.length) {
      throw new RangeError(
        `Invalid ${kind}${formatProperty(
          property
        )}: decoded past the end of the buffer`
      );
    }
  }

  return cursor - offset;
}

class VecLayout<T> extends LayoutCls<T[]> {
  elementLayout: Layout<T>;

  constructor(elementLayout: Layout<T>, property?: string) {
    super(-1, property);
    this.elementLayout = elementLayout;
  }

  encode(src: T[], b: Buffer, offset = 0): number {
    assertWritableLength(src.length, "vec", this.property);
    let cursor = offset;
    cursor += u32().encode(src.length, b, cursor);
    for (const value of src) {
      cursor += this.elementLayout.encode(value, b, cursor);
    }
    return cursor - offset;
  }

  decode(b: Buffer, offset = 0): T[] {
    const count = readU32Length(b, offset, "vec", this.property);
    const dataOffset = offset + U32_SPAN;
    return decodeCollectionValues(
      count,
      this.elementLayout,
      b,
      dataOffset,
      "vec",
      this.property
    ).values;
  }

  getSpan(b: Buffer, offset = 0): number {
    const count = readU32Length(b, offset, "vec", this.property);
    return (
      U32_SPAN +
      measureCollectionSpan(
        count,
        this.elementLayout,
        b,
        offset + U32_SPAN,
        "vec",
        this.property
      )
    );
  }
}

class BytesLayout extends LayoutCls<Buffer> {
  constructor(property?: string) {
    super(-1, property);
  }

  encode(src: Buffer, b: Buffer, offset = 0): number {
    assertWritableLength(src.length, "bytes", this.property);
    let cursor = offset;
    cursor += u32().encode(src.length, b, cursor);
    assertReadableBytes(b, cursor, src.length, "bytes", this.property);
    src.copy(b, cursor);
    return U32_SPAN + src.length;
  }

  decode(b: Buffer, offset = 0): Buffer {
    const length = readU32Length(b, offset, "bytes", this.property);
    const dataOffset = offset + U32_SPAN;
    const remainingBytes = Math.max(0, b.length - dataOffset);
    if (length > remainingBytes) {
      throw new RangeError(
        `Invalid bytes${formatProperty(
          this.property
        )}: length ${length} exceeds ${remainingBytes} remaining bytes`
      );
    }
    return b.subarray(dataOffset, dataOffset + length);
  }

  getSpan(b: Buffer, offset = 0): number {
    const length = readU32Length(b, offset, "bytes", this.property);
    const remainingBytes = Math.max(0, b.length - (offset + U32_SPAN));
    if (length > remainingBytes) {
      throw new RangeError(
        `Invalid bytes${formatProperty(
          this.property
        )}: length ${length} exceeds ${remainingBytes} remaining bytes`
      );
    }
    return U32_SPAN + length;
  }
}

class FixedArrayLayout<T> extends LayoutCls<T[]> {
  elementLayout: Layout<T>;
  length: number;

  constructor(elementLayout: Layout<T>, length: number, property?: string) {
    assertWritableLength(length, "array", property);
    const span =
      elementLayout.span > 0 &&
      Number.isSafeInteger(elementLayout.span * length)
        ? elementLayout.span * length
        : -1;
    super(span, property);
    this.elementLayout = elementLayout;
    this.length = length;
  }

  encode(src: T[], b: Buffer, offset = 0): number {
    if (src.length !== this.length) {
      throw new RangeError(
        `Invalid array${formatProperty(this.property)}: expected ${
          this.length
        } items, received ${src.length}`
      );
    }

    let cursor = offset;
    for (const value of src) {
      cursor += this.elementLayout.encode(value, b, cursor);
    }
    return cursor - offset;
  }

  decode(b: Buffer, offset = 0): T[] {
    return decodeCollectionValues(
      this.length,
      this.elementLayout,
      b,
      offset,
      "array",
      this.property
    ).values;
  }

  getSpan(b: Buffer, offset = 0): number {
    return measureCollectionSpan(
      this.length,
      this.elementLayout,
      b,
      offset,
      "array",
      this.property
    );
  }
}

export function vec<T>(
  elementLayout: Layout<T>,
  property?: string
): Layout<T[]> {
  return new VecLayout(elementLayout, property);
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
  return new BytesLayout(property);
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
  return new FixedArrayLayout(elementLayout, length, property);
}

class MapEntryLayout<K, V> extends LayoutCls<[K, V]> {
  keyLayout: Layout<K>;
  valueLayout: Layout<V>;

  constructor(keyLayout: Layout<K>, valueLayout: Layout<V>, property?: string) {
    const span =
      keyLayout.span >= 0 &&
      valueLayout.span >= 0 &&
      Number.isSafeInteger(keyLayout.span + valueLayout.span)
        ? keyLayout.span + valueLayout.span
        : -1;
    super(span, property);
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
    offset = offset || 0;
    const keySpan = this.keyLayout.getSpan(b, offset);
    return keySpan + this.valueLayout.getSpan(b, offset + keySpan);
  }
}

class MapLayout<K, V> extends LayoutCls<Map<K, V>> {
  entryLayout: Layout<[K, V]>;

  constructor(keyLayout: Layout<K>, valueLayout: Layout<V>, property?: string) {
    super(-1, property);
    this.entryLayout = new MapEntryLayout(keyLayout, valueLayout);
  }

  encode(src: Map<K, V>, b: Buffer, offset = 0): number {
    const entries = Array.from(src.entries());
    assertWritableLength(entries.length, "map", this.property);
    let cursor = offset;
    cursor += u32().encode(entries.length, b, cursor);
    for (const entry of entries) {
      cursor += this.entryLayout.encode(entry, b, cursor);
    }
    return cursor - offset;
  }

  decode(b: Buffer, offset = 0): Map<K, V> {
    const count = readU32Length(b, offset, "map", this.property);
    const values = decodeCollectionValues(
      count,
      this.entryLayout,
      b,
      offset + U32_SPAN,
      "map",
      this.property
    ).values;
    return new Map(values);
  }

  getSpan(b: Buffer, offset = 0): number {
    const count = readU32Length(b, offset, "map", this.property);
    return (
      U32_SPAN +
      measureCollectionSpan(
        count,
        this.entryLayout,
        b,
        offset + U32_SPAN,
        "map",
        this.property
      )
    );
  }
}

export function map<K, V>(
  keyLayout: Layout<K>,
  valueLayout: Layout<V>,
  property?: string
): Layout<Map<K, V>> {
  return new MapLayout(keyLayout, valueLayout, property);
}
