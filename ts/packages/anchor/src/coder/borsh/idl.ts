import { Layout } from "buffer-layout";
import * as borsh from "@anchor-lang/borsh";
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
  public static fieldLayout(
    field: PartialField,
    types: IdlTypeDef[] = [],
    genericArgs?: IdlGenericArg[] | null
  ): Layout {
    const fieldName = field.name;
    switch (field.type) {
      case "bool": {
        return borsh.bool(fieldName);
      }
      case "u8": {
        return borsh.u8(fieldName);
      }
      case "i8": {
        return borsh.i8(fieldName);
      }
      case "u16": {
        return borsh.u16(fieldName);
      }
      case "i16": {
        return borsh.i16(fieldName);
      }
      case "u32": {
        return borsh.u32(fieldName);
      }
      case "i32": {
        return borsh.i32(fieldName);
      }
      case "f32": {
        return borsh.f32(fieldName);
      }
      case "u64": {
        return borsh.u64(fieldName);
      }
      case "i64": {
        return borsh.i64(fieldName);
      }
      case "f64": {
        return borsh.f64(fieldName);
      }
      case "u128": {
        return borsh.u128(fieldName);
      }
      case "i128": {
        return borsh.i128(fieldName);
      }
      case "u256": {
        return borsh.u256(fieldName);
      }
      case "i256": {
        return borsh.i256(fieldName);
      }
      case "bytes": {
        return borsh.vecU8(fieldName);
      }
      case "string": {
        return borsh.str(fieldName);
      }
      case "pubkey": {
        return borsh.publicKey(fieldName);
      }
      default: {
        if ("option" in field.type) {
          return borsh.option(
            IdlCoder.fieldLayout(
              { type: field.type.option },
              types,
              genericArgs
            ),
            fieldName
          );
        }
        if ("vec" in field.type) {
          return borsh.vec(
            IdlCoder.fieldLayout({ type: field.type.vec }, types, genericArgs),
            fieldName
          );
        }
        if ("array" in field.type) {
          let [type, len] = field.type.array;
          len = IdlCoder.resolveArrayLen(len, genericArgs);

          return borsh.array(
            IdlCoder.fieldLayout({ type }, types, genericArgs),
            len,
            fieldName
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

          return IdlCoder.typeDefLayout({
            typeDef,
            types,
            genericArgs: genericArgs ?? field.type.defined.generics,
            name: fieldName,
          });
        }
        if ("generic" in field.type) {
          const genericArg = genericArgs?.at(0);
          if (genericArg?.kind !== "type") {
            throw new IdlError(`Invalid generic field: ${field.name}`);
          }

          return IdlCoder.fieldLayout(
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
   * Get the type layout of the given defined type(struct or enum).
   */
  public static typeDefLayout({
    typeDef,
    types,
    name,
    genericArgs,
  }: {
    typeDef: IdlTypeDef;
    types: IdlTypeDef[];
    genericArgs?: IdlGenericArg[] | null;
    name?: string;
  }): Layout {
    switch (typeDef.type.kind) {
      case "struct": {
        const fieldLayouts = handleDefinedFields(
          typeDef.type.fields,
          () => [],
          (fields) =>
            fields.map((f) => {
              const genArgs = genericArgs
                ? IdlCoder.resolveGenericArgs({
                    type: f.type,
                    typeDef,
                    genericArgs,
                  })
                : genericArgs;
              return IdlCoder.fieldLayout(f, types, genArgs);
            }),
          (fields) =>
            fields.map((f, i) => {
              const genArgs = genericArgs
                ? IdlCoder.resolveGenericArgs({
                    type: f,
                    typeDef,
                    genericArgs,
                  })
                : genericArgs;
              return IdlCoder.fieldLayout(
                { name: i.toString(), type: f },
                types,
                genArgs
              );
            })
        );

        return borsh.struct(fieldLayouts, name);
      }

      case "enum": {
        const variants = typeDef.type.variants.map((variant) => {
          const fieldLayouts = handleDefinedFields(
            variant.fields,
            () => [],
            (fields) =>
              fields.map((f) => {
                const genArgs = genericArgs
                  ? IdlCoder.resolveGenericArgs({
                      type: f.type,
                      typeDef,
                      genericArgs,
                    })
                  : genericArgs;
                return IdlCoder.fieldLayout(f, types, genArgs);
              }),
            (fields) =>
              fields.map((f, i) => {
                const genArgs = genericArgs
                  ? IdlCoder.resolveGenericArgs({
                      type: f,
                      typeDef,
                      genericArgs,
                    })
                  : genericArgs;
                return IdlCoder.fieldLayout(
                  { name: i.toString(), type: f },
                  types,
                  genArgs
                );
              })
          );

          return borsh.struct(fieldLayouts, variant.name);
        });

        if (name !== undefined) {
          // Buffer-layout lib requires the name to be null (on construction)
          // when used as a field.
          return borsh.rustEnum(variants).replicate(name);
        }

        return borsh.rustEnum(variants, name);
      }

      case "type": {
        return IdlCoder.fieldLayout({ type: typeDef.type.alias, name }, types);
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
