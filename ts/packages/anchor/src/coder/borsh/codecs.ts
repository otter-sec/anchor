import {
  Address,
  assertNumberIsBetweenForCodec,
  Codec,
  Endian,
  FixedSizeCodec,
  getAddressCodec,
  getDiscriminatedUnionCodec,
  getI128Codec,
  getOptionCodec,
  getTupleCodec,
  getU8Codec,
  getU32Codec,
  getU128Codec,
  isFixedSize,
  NumberCodec,
  NumberCodecConfig,
  OptionOrNullable,
  transformCodec,
  unwrapOption,
} from "@solana/kit";
import type { PublicKey } from "@solana/web3.js";

/**
 * A codec for a value whose shape is only known at runtime, e.g. because it
 * is described by an IDL. This is the type returned by the `IdlCoder`
 * compiler; any precisely typed codec is assignable to it.
 */
export type IdlCodec = Codec<unknown, unknown>;

/**
 * Public key codec. Decodes to a Kit `Address` (base58 string); encodes from
 * an `Address` or a web3.js `PublicKey`.
 */
export function getPublicKeyCodec(): FixedSizeCodec<
  Address | PublicKey,
  Address,
  32
> {
  return transformCodec(getAddressCodec(), (value: Address | PublicKey) =>
    typeof value === "string" ? value : (value.toBase58() as Address)
  );
}

/**
 * Boolean codec: a single byte holding 0 or 1. Unlike Kit's boolean codec,
 * decoding any other byte throws, matching Rust borsh deserialization.
 */
export function getBoolCodec(): FixedSizeCodec<boolean, boolean, 1> {
  return transformCodec(
    getU8Codec(),
    (value: boolean) => (value ? 1 : 0),
    (byte) => {
      if (byte !== 0 && byte !== 1) {
        throw new Error(`Invalid bool: ${byte}`);
      }
      return byte === 1;
    }
  );
}

/**
 * Borsh `Option<T>`: u8 tag (0 = None, 1 = Some) followed by the payload.
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
    getOptionCodec(inner),
    (value: OptionOrNullable<TFrom> | undefined) => value ?? null,
    (option) => unwrapOption(option)
  );
}

/**
 * C-style `COption<T>`: u32 LE tag (0 = None, 1 = Some) followed by the
 * payload. Used by native Solana account layouts (e.g. SPL Mint/Account)
 * and by programs that want wire compatibility with them. For fixed-size
 * inners, `None` values still occupy the payload slot (zero-filled) so
 * downstream offsets line up.
 *
 * Inputs and outputs behave like {@link getAnchorOptionCodec}.
 */
export function getCOptionCodec<TFrom, TTo extends TFrom = TFrom>(
  inner: Codec<TFrom, TTo>
): Codec<OptionOrNullable<TFrom> | undefined, TTo | null> {
  const prefix = getU32Codec();
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
 * This is Kit's discriminated union codec with the `__kind` discriminator
 * property mapped to and from the single-key object shape.
 *
 * @param variants     Ordered `[variantName, fieldsCodec]` pairs.
 * @param discriminant Codec for the variant index. Defaults to u8 (borsh);
 *                     some native layouts use u32.
 */
export function getRustEnumCodec(
  variants: [string, IdlCodec][],
  discriminant?: NumberCodec
): IdlCodec {
  const variantNames = new Set(variants.map(([name]) => name));
  const union = getDiscriminatedUnionCodec(
    variants,
    discriminant ? { size: discriminant } : {}
  );

  return transformCodec(
    union,
    (value: unknown): { __kind: string } => {
      if (typeof value === "object" && value !== null) {
        const record = value as Record<string, object | undefined>;
        for (const key of Object.keys(record)) {
          if (variantNames.has(key)) {
            return { __kind: key, ...(record[key] ?? {}) };
          }
        }
      }
      throw new Error(`Invalid enum variant: ${JSON.stringify(value)}`);
    },
    (value) => {
      const { __kind, ...fields } = value as { __kind: string } & Record<
        string,
        unknown
      >;
      return { [__kind]: fields };
    }
  );
}

// TODO(kit): upstream 256-bit codecs to @solana/codecs-numbers and remove
// these local implementations.

const U128_MASK = (1n << 128n) - 1n;

/**
 * 256-bit unsigned integer codec, as two 128-bit chunks.
 */
export function getU256Codec(
  config: NumberCodecConfig = {}
): FixedSizeCodec<bigint | number, bigint, 32> {
  return get256BitCodec({ config, name: "u256", signed: false });
}

/**
 * 256-bit signed (two's complement) integer codec, as two 128-bit chunks.
 */
export function getI256Codec(
  config: NumberCodecConfig = {}
): FixedSizeCodec<bigint | number, bigint, 32> {
  return get256BitCodec({ config, name: "i256", signed: true });
}

function get256BitCodec({
  config,
  name,
  signed,
}: {
  config: NumberCodecConfig;
  name: string;
  signed: boolean;
}): FixedSizeCodec<bigint | number, bigint, 32> {
  const min = signed ? -(1n << 255n) : 0n;
  const max = signed ? (1n << 255n) - 1n : (1n << 256n) - 1n;
  const le = config.endian !== Endian.Big;

  // The most significant chunk carries the sign for signed values.
  const lowCodec = getU128Codec(config);
  const highCodec = signed ? getI128Codec(config) : getU128Codec(config);

  return transformCodec(
    getTupleCodec(le ? [lowCodec, highCodec] : [highCodec, lowCodec]),
    (value: bigint | number): [bigint, bigint] => {
      assertNumberIsBetweenForCodec(name, min, max, value);
      const v = BigInt(value);
      const [low, high] = [v & U128_MASK, v >> 128n];
      return le ? [low, high] : [high, low];
    },
    (chunks) => {
      const [low, high] = le ? chunks : [chunks[1], chunks[0]];
      return (high << 128n) + low;
    }
  ) as FixedSizeCodec<bigint | number, bigint, 32>;
}
