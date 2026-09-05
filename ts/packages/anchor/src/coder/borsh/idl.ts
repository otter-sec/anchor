import {
  addCodecSizePrefix,
  getArrayCodec,
  getBytesCodec,
  getF32Codec,
  getF64Codec,
  getI8Codec,
  getI16Codec,
  getI32Codec,
  getI64Codec,
  getI128Codec,
  getStructCodec,
  getU8Codec,
  getU16Codec,
  getU32Codec,
  getU64Codec,
  getU128Codec,
  getUtf8Codec,
} from "@solana/kit";
import {
  getAnchorOptionCodec,
  getBoolCodec,
  getCOptionCodec,
  getI256Codec,
  getPublicKeyCodec,
  getRustEnumCodec,
  getU256Codec,
  IdlCodec,
} from "./codecs.js";
import {
  IdlField,
  IdlTypeDef,
  IdlType,
  IdlGenericArg,
  Idl,
  handleDefinedFields,
  IdlArrayLen,
} from "../../idl.js";
import { IdlError } from "../../error.js";

type PartialField = { name?: string } & Pick<IdlField, "type">;

export class IdlCoder {
  /**
   * Get the codec of the given IDL field type.
   */
  public static fieldCodec(
    field: PartialField,
    types: IdlTypeDef[] = [],
    genericArgs?: IdlGenericArg[] | null
  ): IdlCodec {
    switch (field.type) {
      case "bool": {
        return getBoolCodec();
      }
      case "u8": {
        return getU8Codec();
      }
      case "i8": {
        return getI8Codec();
      }
      case "u16": {
        return getU16Codec();
      }
      case "i16": {
        return getI16Codec();
      }
      case "u32": {
        return getU32Codec();
      }
      case "i32": {
        return getI32Codec();
      }
      case "f32": {
        return getF32Codec();
      }
      case "u64": {
        return getU64Codec();
      }
      case "i64": {
        return getI64Codec();
      }
      case "f64": {
        return getF64Codec();
      }
      case "u128": {
        return getU128Codec();
      }
      case "i128": {
        return getI128Codec();
      }
      case "u256": {
        return getU256Codec();
      }
      case "i256": {
        return getI256Codec();
      }
      case "bytes": {
        return addCodecSizePrefix(getBytesCodec(), getU32Codec());
      }
      case "string": {
        return addCodecSizePrefix(getUtf8Codec(), getU32Codec());
      }
      case "pubkey": {
        return getPublicKeyCodec();
      }
      default: {
        if ("option" in field.type) {
          return getAnchorOptionCodec(
            IdlCoder.fieldCodec({ type: field.type.option }, types, genericArgs)
          );
        }
        if ("coption" in field.type) {
          return getCOptionCodec(
            IdlCoder.fieldCodec(
              { type: field.type.coption },
              types,
              genericArgs
            )
          );
        }
        if ("vec" in field.type) {
          return getArrayCodec(
            IdlCoder.fieldCodec({ type: field.type.vec }, types, genericArgs)
          );
        }
        if ("array" in field.type) {
          let [type, len] = field.type.array;
          len = IdlCoder.resolveArrayLen(len, genericArgs);

          return getArrayCodec(
            IdlCoder.fieldCodec({ type }, types, genericArgs),
            {
              size: len,
            }
          );
        }
        if ("defined" in field.type) {
          if (!types) {
            throw new IdlError("User defined types not provided");
          }

          const definedName = field.type.defined.name;
          const typeDef = types.find((t) => t.name === definedName);
          if (!typeDef) {
            throw new IdlError(`Type not found: ${field.name}`);
          }

          return IdlCoder.typeDefCodec({
            typeDef,
            types,
            genericArgs: genericArgs ?? field.type.defined.generics,
          });
        }
        if ("generic" in field.type) {
          const genericArg = genericArgs?.at(0);
          if (genericArg?.kind !== "type") {
            throw new IdlError(`Invalid generic field: ${field.name}`);
          }

          return IdlCoder.fieldCodec(
            { ...field, type: genericArg.type },
            types
          );
        }

        throw new IdlError(
          `Not yet implemented: ${JSON.stringify(field.type)}`
        );
      }
    }
  }

  /**
   * Get the codec of the given defined type (struct or enum).
   */
  public static typeDefCodec({
    typeDef,
    types,
    genericArgs,
  }: {
    typeDef: IdlTypeDef;
    types: IdlTypeDef[];
    genericArgs?: IdlGenericArg[] | null;
  }): IdlCodec {
    switch (typeDef.type.kind) {
      case "struct": {
        const fieldCodecs = handleDefinedFields(
          typeDef.type.fields,
          (): [string, IdlCodec][] => [],
          (fields) =>
            fields.map((f): [string, IdlCodec] => {
              const genArgs = genericArgs
                ? IdlCoder.resolveGenericArgs({
                    type: f.type,
                    typeDef,
                    genericArgs,
                  })
                : genericArgs;
              return [f.name, IdlCoder.fieldCodec(f, types, genArgs)];
            }),
          (fields) =>
            fields.map((f, i): [string, IdlCodec] => {
              const genArgs = genericArgs
                ? IdlCoder.resolveGenericArgs({
                    type: f,
                    typeDef,
                    genericArgs,
                  })
                : genericArgs;
              return [
                i.toString(),
                IdlCoder.fieldCodec(
                  { name: i.toString(), type: f },
                  types,
                  genArgs
                ),
              ];
            })
        );

        return getStructCodec(fieldCodecs);
      }

      case "enum": {
        const variants = typeDef.type.variants.map(
          (variant): [string, IdlCodec] => {
            const fieldCodecs = handleDefinedFields(
              variant.fields,
              (): [string, IdlCodec][] => [],
              (fields) =>
                fields.map((f): [string, IdlCodec] => {
                  const genArgs = genericArgs
                    ? IdlCoder.resolveGenericArgs({
                        type: f.type,
                        typeDef,
                        genericArgs,
                      })
                    : genericArgs;
                  return [f.name, IdlCoder.fieldCodec(f, types, genArgs)];
                }),
              (fields) =>
                fields.map((f, i): [string, IdlCodec] => {
                  const genArgs = genericArgs
                    ? IdlCoder.resolveGenericArgs({
                        type: f,
                        typeDef,
                        genericArgs,
                      })
                    : genericArgs;
                  return [
                    i.toString(),
                    IdlCoder.fieldCodec(
                      { name: i.toString(), type: f },
                      types,
                      genArgs
                    ),
                  ];
                })
            );

            return [variant.name, getStructCodec(fieldCodecs)];
          }
        );

        return getRustEnumCodec(variants);
      }

      case "type": {
        return IdlCoder.fieldCodec({ type: typeDef.type.alias }, types);
      }
    }
  }

  /**
   * Get the type of the size in bytes. Returns `1` for variable length types.
   */
  public static typeSize(
    ty: IdlType,
    idl: Idl,
    genericArgs?: IdlGenericArg[] | null
  ): number {
    switch (ty) {
      case "bool":
        return 1;
      case "u8":
        return 1;
      case "i8":
        return 1;
      case "i16":
        return 2;
      case "u16":
        return 2;
      case "u32":
        return 4;
      case "i32":
        return 4;
      case "f32":
        return 4;
      case "u64":
        return 8;
      case "i64":
        return 8;
      case "f64":
        return 8;
      case "u128":
        return 16;
      case "i128":
        return 16;
      case "u256":
        return 32;
      case "i256":
        return 32;
      case "bytes":
        return 1;
      case "string":
        return 1;
      case "pubkey":
        return 32;
      default:
        if ("option" in ty) {
          return 1 + IdlCoder.typeSize(ty.option, idl, genericArgs);
        }
        if ("coption" in ty) {
          return 4 + IdlCoder.typeSize(ty.coption, idl, genericArgs);
        }
        if ("vec" in ty) {
          return 1;
        }
        if ("array" in ty) {
          let [type, len] = ty.array;
          len = IdlCoder.resolveArrayLen(len, genericArgs);
          return IdlCoder.typeSize(type, idl, genericArgs) * len;
        }
        if ("defined" in ty) {
          const typeDef = idl.types?.find((t) => t.name === ty.defined.name);
          if (!typeDef) {
            throw new IdlError(`Type not found: ${JSON.stringify(ty)}`);
          }

          const typeSize = (type: IdlType) => {
            const genArgs = genericArgs ?? ty.defined.generics;
            const args = genArgs
              ? IdlCoder.resolveGenericArgs({
                  type,
                  typeDef,
                  genericArgs: genArgs,
                })
              : genArgs;

            return IdlCoder.typeSize(type, idl, args);
          };

          switch (typeDef.type.kind) {
            case "struct": {
              return handleDefinedFields(
                typeDef.type.fields,
                () => [0],
                (fields) => fields.map((f) => typeSize(f.type)),
                (fields) => fields.map((f) => typeSize(f))
              ).reduce((acc, size) => acc + size, 0);
            }

            case "enum": {
              const variantSizes = typeDef.type.variants.map((variant) => {
                return handleDefinedFields(
                  variant.fields,
                  () => [0],
                  (fields) => fields.map((f) => typeSize(f.type)),
                  (fields) => fields.map((f) => typeSize(f))
                ).reduce((acc, size) => acc + size, 0);
              });

              return Math.max(...variantSizes) + 1;
            }

            case "type": {
              return IdlCoder.typeSize(typeDef.type.alias, idl, genericArgs);
            }
          }
        }
        if ("generic" in ty) {
          const genericArg = genericArgs?.at(0);
          if (genericArg?.kind !== "type") {
            throw new IdlError(`Invalid generic: ${ty.generic}`);
          }

          return IdlCoder.typeSize(genericArg.type, idl, genericArgs);
        }

        throw new Error(`Invalid type ${JSON.stringify(ty)}`);
    }
  }

  /**
   * Resolve the generic array length or return the constant-sized array length.
   */
  private static resolveArrayLen(
    len: IdlArrayLen,
    genericArgs?: IdlGenericArg[] | null
  ): number {
    if (typeof len === "number") return len;

    if (genericArgs) {
      const genericLen = genericArgs.find((g) => g.kind === "const");
      if (genericLen?.kind === "const") {
        len = +genericLen.value;
      }
    }

    if (typeof len !== "number") {
      throw new IdlError("Generic array length did not resolve");
    }

    return len;
  }

  /**
   * Recursively resolve generic arguments i.e. replace all generics with the
   * actual type that they hold based on the initial `genericArgs` given.
   */
  private static resolveGenericArgs({
    type,
    typeDef,
    genericArgs,
    isDefined,
  }: {
    type: IdlType;
    typeDef: IdlTypeDef;
    genericArgs: IdlGenericArg[];
    isDefined?: boolean;
  }): IdlGenericArg[] | null {
    if (typeof type !== "object") return null;

    for (const index in typeDef.generics) {
      const defGeneric = typeDef.generics[index];

      if ("generic" in type && defGeneric.name === type.generic) {
        return [genericArgs[index]];
      }

      if ("option" in type) {
        const args = IdlCoder.resolveGenericArgs({
          type: type.option,
          typeDef,
          genericArgs,
          isDefined,
        });
        if (!args || !isDefined) return args;

        if (args[0].kind === "type") {
          return [
            {
              kind: "type",
              type: { option: args[0].type },
            },
          ];
        }
      }

      if ("vec" in type) {
        const args = IdlCoder.resolveGenericArgs({
          type: type.vec,
          typeDef,
          genericArgs,
          isDefined,
        });
        if (!args || !isDefined) return args;

        if (args[0].kind === "type") {
          return [
            {
              kind: "type",
              type: { vec: args[0].type },
            },
          ];
        }
      }

      if ("array" in type) {
        const [elTy, len] = type.array;
        const isGenericLen = typeof len === "object";

        const args =
          IdlCoder.resolveGenericArgs({
            type: elTy,
            typeDef,
            genericArgs,
            isDefined,
          }) || [];

        // Check all generics for matching const generic length
        if (isGenericLen) {
          const matchingGeneric = typeDef.generics.findIndex(
            (g) => g.name === len.generic
          );
          if (matchingGeneric !== -1) {
            args.push(genericArgs[matchingGeneric]);
          }
        }

        if (args.length > 0) {
          if (!isDefined) return args;

          if (args[0].kind === "type" && args[1].kind === "const") {
            return [
              {
                kind: "type",
                type: { array: [args[0].type, +args[1].value] },
              },
            ];
          }
        }

        // Only generic len
        if (isGenericLen && defGeneric.name === len.generic) {
          const arg = genericArgs[index];
          if (!isDefined) return [arg];

          return [
            {
              kind: "type",
              type: { array: [elTy, +arg.value] },
            },
          ];
        }

        // Non-generic
        return null;
      }

      if ("defined" in type) {
        if (!type.defined.generics) return null;

        return type.defined.generics
          .flatMap((g) => {
            switch (g.kind) {
              case "type":
                return IdlCoder.resolveGenericArgs({
                  type: g.type,
                  typeDef,
                  genericArgs,
                  isDefined: true,
                });
              case "const":
                return [g];
            }
          })
          .filter((g) => g !== null) as IdlGenericArg[];
      }
    }

    return null;
  }
}
