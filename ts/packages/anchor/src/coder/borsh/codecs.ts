import {
  Address,
  Codec,
  combineCodec,
  containsBytes,
  FixedSizeCodec,
  getAddressCodec,
  getBooleanDecoder,
  getBooleanEncoder,
  getConstantCodec,
  getOptionCodec,
  getHiddenPrefixCodec,
  getTupleCodec,
  getU8Codec,
  getU32Codec,
  getUnionCodec,
  isFixedSize,
  NumberCodec,
  OptionOrNullable,
  ReadonlyUint8Array,
  tapDecoder,
  tapDecoderBytes,
  transformCodec,
  unwrapOption,
} from "@solana/kit";
import type { AddressInput } from "../../program/common.js";

/**
 * A codec for a value whose shape is only known at runtime, e.g. because it
 * is described by an IDL. This is the type returned by the `IdlCoder`
 * compiler; any precisely typed codec is assignable to it.
 */
export type IdlCodec = Codec<unknown, unknown>;

/**
 * Address codec. Decodes to a Kit `Address`; encodes from an `Address` or any
 * object exposing one through `toBase58()`, e.g. a web3.js public key.
 */
export function getAnchorAddressCodec(): FixedSizeCodec<
  AddressInput,
  Address,
  32
> {
  return transformCodec(getAddressCodec(), (value: AddressInput) =>
    typeof value === "string" ? value : (value.toBase58() as Address)
  );
}

/**
 * Boolean codec: a single byte holding 0 or 1. Unlike Kit's boolean codec,
 * decoding any other byte throws, matching Rust borsh deserialization.
 */
export function getBoolCodec(): FixedSizeCodec<boolean, boolean, 1> {
  return combineCodec(
    getBooleanEncoder(),
    tapDecoderBytes(getBooleanDecoder(), (bytes, offset) => {
      // A missing byte falls through to Kit's own bounds check.
      if (bytes[offset] > 1) {
        throw new Error(`Invalid bool: ${bytes[offset]}`);
      }
    })
  );
}

/**
 * Option tag codec: the given number codec, except that decoding a tag other
 * than 0 or 1 throws. Kit's option codec treats any tag other than 1 as
 * `None`; borsh deserialization (and v1) reject it.
 */
function getOptionPrefixCodec<TSize extends number>(
  prefix: FixedSizeCodec<bigint | number, number, TSize>
): FixedSizeCodec<bigint | number, number, TSize> {
  return tapDecoder(prefix, (tag) => {
    if (tag !== 0 && tag !== 1) {
      throw new Error(`Invalid option tag: ${tag}`);
    }
  });
}

/**
 * Borsh `Option<T>`: u8 tag (0 = None, 1 = Some) followed by the payload.
 * Decoding any other tag throws, matching Rust borsh deserialization.
 *
 * This is Kit's option codec with the decoded `Option<T>` unwrapped to
 * `T | null`, which collapses nested options on decode. Encoding accepts
 * `null` or `undefined` (e.g. an omitted field) for `None` and Kit's
 * `some()` and `none()` wrappers to disambiguate nested options (e.g.
 * `some(null)` encodes `Some(None)`).
 */
export function getAnchorOptionCodec<TFrom, TTo extends TFrom = TFrom>(
  inner: Codec<TFrom, TTo>
): Codec<OptionOrNullable<TFrom> | undefined, TTo | null> {
  return transformCodec(
    getOptionCodec(inner, { prefix: getOptionPrefixCodec(getU8Codec()) }),
    (value: OptionOrNullable<TFrom> | undefined) => value ?? null,
    (option) => unwrapOption(option)
  );
}

/**
 * C-style `COption<T>`: u32 LE tag (0 = None, 1 = Some) followed by the
 * payload. Used by native Solana account layouts (e.g. SPL Mint/Account)
 * and by programs that want wire compatibility with them. For fixed-size
 * inners, `None` values still occupy the payload slot (zero-filled) so
 * downstream offsets line up. Decoding any other tag throws.
 *
 * Inputs and outputs behave like {@link getAnchorOptionCodec}.
 */
export function getCOptionCodec<TFrom, TTo extends TFrom = TFrom>(
  inner: Codec<TFrom, TTo>
): Codec<OptionOrNullable<TFrom> | undefined, TTo | null> {
  const prefix = getOptionPrefixCodec(getU32Codec());
  return transformCodec(
    isFixedSize(inner)
      ? getOptionCodec(inner, { noneValue: "zeroes", prefix })
      : getOptionCodec(inner, { prefix }),
    (value: OptionOrNullable<TFrom> | undefined) => value ?? null,
    (option) => unwrapOption(option)
  );
}

/**
 * Borsh enum, preserving the Anchor JS shape: values are single-key objects
 * (`{ variantName: fields }`), with unit variants represented as
 * `{ variantName: {} }`.
 *
 * Each variant is encoded as its index followed by its fields. The fields
 * object is handed to the variant codec as is, so field names never clash
 * with a discriminator property.
 *
 * @param variants     Ordered `[variantName, fieldsCodec]` pairs.
 * @param discriminant Codec for the variant index. Defaults to u8 (borsh);
 *                     some native layouts use u32.
 */
export function getRustEnumCodec(
  variants: [string, IdlCodec][],
  discriminant: NumberCodec = getU8Codec()
): IdlCodec {
  const names = variants.map(([name]) => name);
  return getUnionCodec(
    variants.map(([name, fields], index) =>
      transformCodec(
        getTupleCodec([discriminant, fields]),
        (value: Record<string, unknown>): [number, unknown] => [
          index,
          value[name] ?? {},
        ],
        ([, decoded]) => ({ [name]: decoded })
      )
    ),
    (value: unknown) => {
      if (typeof value === "object" && value !== null) {
        const index = names.findIndex((name) => Object.hasOwn(value, name));
        if (index >= 0) return index;
      }
      throw new Error(`Invalid enum variant: ${JSON.stringify(value)}`);
    },
    (bytes, offset) => Number(discriminant.read(bytes, offset)[0])
  );
}

/**
 * An IDL codec for a value prefixed by its discriminator, e.g. an account or
 * an instruction. Encoding prepends the discriminator and decoding checks it,
 * as Kit's hidden prefix codec does; the discriminator bytes and the codec of
 * the value that follows are exposed for callers that need to identify or
 * decode values without the check.
 */
export type DiscriminatedIdlCodec = IdlCodec & {
  /** The discriminator prefixing encoded values. */
  readonly discriminator: ReadonlyUint8Array;
  /** The codec of the value that follows the discriminator. */
  readonly item: IdlCodec;
  /** Whether the given bytes start with the discriminator. */
  matches(bytes: ReadonlyUint8Array): boolean;
};

export function getDiscriminatedIdlCodec(
  discriminator: readonly number[],
  item: IdlCodec
): DiscriminatedIdlCodec {
  const prefix: ReadonlyUint8Array = new Uint8Array(discriminator);
  return Object.freeze({
    ...getHiddenPrefixCodec(item, [getConstantCodec(prefix)]),
    discriminator: prefix,
    item,
    matches: (bytes: ReadonlyUint8Array) => containsBytes(bytes, prefix, 0),
  });
}
