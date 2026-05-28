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

const MAX_LAZY_LAYOUT_RECURSION_DEPTH = 256;

class LazyDefinedLayout extends Layout {
  private static recursionDepth = 0;
  private resolvedLayout?: Layout;

  constructor(private readonly buildLayout: () => Layout, property?: string) {
    super(-1, property);
  }

  private get layout(): Layout {
    if (!this.resolvedLayout) {
      this.resolvedLayout = this.buildLayout();
    }
    return this.resolvedLayout;
  }

  private withRecursionGuard<T>(cb: () => T): T {
    if (LazyDefinedLayout.recursionDepth >= MAX_LAZY_LAYOUT_RECURSION_DEPTH) {
      throw new IdlError(
        `Recursive IDL layout exceeded maximum depth: ${this.property}`
      );
    }

    LazyDefinedLayout.recursionDepth += 1;
    try {
      return cb();
    } finally {
      LazyDefinedLayout.recursionDepth -= 1;
    }
  }

  decode(b: Buffer, offset?: number) {
    return this.withRecursionGuard(() => this.layout.decode(b, offset));
  }

  encode(src: unknown, b: Buffer, offset?: number): number {
    return this.withRecursionGuard(() => this.layout.encode(src, b, offset));
  }

  getSpan(b: Buffer, offset?: number): number {
    return this.withRecursionGuard(() => this.layout.getSpan(b, offset));
  }

  replicate(name: string): this {
    return new LazyDefinedLayout(this.buildLayout, name) as this;
  }
}

export class IdlCoder {
  public static fieldLayout(
    field: PartialField,
    types: IdlTypeDef[] = [],
    genericArgs?: IdlGenericArg[] | null
  ): Layout {
    return IdlCoder.fieldLayoutWithContext(field, types, genericArgs);
  }

  private static fieldLayoutWithContext(
    field: PartialField,
    types: IdlTypeDef[] = [],
    genericArgs?: IdlGenericArg[] | null,
    definedTypeStack: string[] = [],
    allowRecursive = false
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
            IdlCoder.fieldLayoutWithContext(
              { type: field.type.option },
              types,
              genericArgs,
              definedTypeStack,
              true
            ),
            fieldName
          );
        }
        if ("vec" in field.type) {
          return borsh.vec(
            IdlCoder.fieldLayoutWithContext(
              { type: field.type.vec },
              types,
              genericArgs,
              definedTypeStack,
              true
            ),
            fieldName
          );
        }
        if ("array" in field.type) {
          let [type, len] = field.type.array;
          len = IdlCoder.resolveArrayLen(len, genericArgs);

          return borsh.array(
            IdlCoder.fieldLayoutWithContext(
              { type },
              types,
              genericArgs,
              definedTypeStack,
              allowRecursive
            ),
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

          const fieldGenericArgs = genericArgs ?? field.type.defined.generics;
          const layout = () =>
            IdlCoder.typeDefLayoutWithContext({
              typeDef,
              types,
              genericArgs: fieldGenericArgs,
              name: fieldName,
            });

          if (definedTypeStack.includes(definedName)) {
            if (!allowRecursive) {
              throw new IdlError(
                `Recursive type must be wrapped in an option or vector: ${definedName}`
              );
            }

            return new LazyDefinedLayout(layout, fieldName);
          }

          return IdlCoder.typeDefLayoutWithContext({
            typeDef,
            types,
            genericArgs: fieldGenericArgs,
            name: fieldName,
            definedTypeStack: [...definedTypeStack, definedName],
          });
        }
        if ("generic" in field.type) {
          const genericArg = genericArgs?.at(0);
          if (genericArg?.kind !== "type") {
            throw new IdlError(`Invalid generic field: ${field.name}`);
          }

          return IdlCoder.fieldLayoutWithContext(
            { ...field, type: genericArg.type },
            types,
            undefined,
            definedTypeStack,
            allowRecursive
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
    return IdlCoder.typeDefLayoutWithContext({
      typeDef,
      types,
      name,
      genericArgs,
    });
  }

  private static typeDefLayoutWithContext({
    typeDef,
    types,
    name,
    genericArgs,
    definedTypeStack,
  }: {
    typeDef: IdlTypeDef;
    types: IdlTypeDef[];
    genericArgs?: IdlGenericArg[] | null;
    name?: string;
    definedTypeStack?: string[];
  }): Layout {
    const typeStack = definedTypeStack ?? [typeDef.name];

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
              return IdlCoder.fieldLayoutWithContext(
                f,
                types,
                genArgs,
                typeStack
              );
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
              return IdlCoder.fieldLayoutWithContext(
                { name: i.toString(), type: f },
                types,
                genArgs,
                typeStack
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
                return IdlCoder.fieldLayoutWithContext(
                  f,
                  types,
                  genArgs,
                  typeStack
                );
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
                return IdlCoder.fieldLayoutWithContext(
                  { name: i.toString(), type: f },
                  types,
                  genArgs,
                  typeStack
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
        return IdlCoder.fieldLayoutWithContext(
          { type: typeDef.type.alias, name },
          types,
          genericArgs,
          typeStack
        );
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
    return IdlCoder.typeSizeWithContext(ty, idl, genericArgs);
  }

  private static typeSizeWithContext(
    ty: IdlType,
    idl: Idl,
    genericArgs?: IdlGenericArg[] | null,
    definedTypeStack: string[] = []
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
          return (
            1 +
            IdlCoder.typeSizeWithContext(
              ty.option,
              idl,
              genericArgs,
              definedTypeStack
            )
          );
        }
        if ("coption" in ty) {
          return (
            4 +
            IdlCoder.typeSizeWithContext(
              ty.coption,
              idl,
              genericArgs,
              definedTypeStack
            )
          );
        }
        if ("vec" in ty) {
          IdlCoder.assertNonRecursiveType(
            ty.vec,
            idl,
            genericArgs,
            definedTypeStack
          );
          return 1;
        }
        if ("array" in ty) {
          let [type, len] = ty.array;
          len = IdlCoder.resolveArrayLen(len, genericArgs);
          return (
            IdlCoder.typeSizeWithContext(
              type,
              idl,
              genericArgs,
              definedTypeStack
            ) * len
          );
        }
        if ("defined" in ty) {
          const typeName = ty.defined.name;
          if (definedTypeStack.includes(typeName)) {
            throw new IdlError(
              `Recursive types do not have a static size: ${typeName}`
            );
          }

          const typeDef = idl.types?.find((t) => t.name === typeName);
          if (!typeDef) {
            throw new IdlError(`Type not found: ${JSON.stringify(ty)}`);
          }

          const typeStack = [...definedTypeStack, typeName];
          const typeSize = (type: IdlType) => {
            const genArgs = genericArgs ?? ty.defined.generics;
            const args = genArgs
              ? IdlCoder.resolveGenericArgs({
                  type,
                  typeDef,
                  genericArgs: genArgs,
                })
              : genArgs;

            return IdlCoder.typeSizeWithContext(type, idl, args, typeStack);
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
              return IdlCoder.typeSizeWithContext(
                typeDef.type.alias,
                idl,
                genericArgs,
                typeStack
              );
            }
          }
        }
        if ("generic" in ty) {
          const genericArg = genericArgs?.at(0);
          if (genericArg?.kind !== "type") {
            throw new IdlError(`Invalid generic: ${ty.generic}`);
          }

          return IdlCoder.typeSizeWithContext(
            genericArg.type,
            idl,
            genericArgs,
            definedTypeStack
          );
        }

        throw new Error(`Invalid type ${JSON.stringify(ty)}`);
    }
  }

  private static assertNonRecursiveType(
    ty: IdlType,
    idl: Idl,
    genericArgs?: IdlGenericArg[] | null,
    definedTypeStack: string[] = []
  ): void {
    if (typeof ty === "string") return;

    if ("option" in ty) {
      return IdlCoder.assertNonRecursiveType(
        ty.option,
        idl,
        genericArgs,
        definedTypeStack
      );
    }
    if ("coption" in ty) {
      return IdlCoder.assertNonRecursiveType(
        ty.coption,
        idl,
        genericArgs,
        definedTypeStack
      );
    }
    if ("vec" in ty) {
      return IdlCoder.assertNonRecursiveType(
        ty.vec,
        idl,
        genericArgs,
        definedTypeStack
      );
    }
    if ("array" in ty) {
      return IdlCoder.assertNonRecursiveType(
        ty.array[0],
        idl,
        genericArgs,
        definedTypeStack
      );
    }
    if ("generic" in ty) {
      const genericArg = genericArgs?.at(0);
      if (genericArg?.kind !== "type") {
        return;
      }

      return IdlCoder.assertNonRecursiveType(
        genericArg.type,
        idl,
        genericArgs,
        definedTypeStack
      );
    }
    if ("defined" in ty) {
      const typeName = ty.defined.name;
      if (definedTypeStack.includes(typeName)) {
        throw new IdlError(
          `Recursive types do not have a static size: ${typeName}`
        );
      }

      const typeDef = idl.types?.find((t) => t.name === typeName);
      if (!typeDef) {
        throw new IdlError(`Type not found: ${JSON.stringify(ty)}`);
      }

      const typeStack = [...definedTypeStack, typeName];
      const checkType = (type: IdlType) => {
        const genArgs = genericArgs ?? ty.defined.generics;
        const args = genArgs
          ? IdlCoder.resolveGenericArgs({
              type,
              typeDef,
              genericArgs: genArgs,
            })
          : genArgs;

        return IdlCoder.assertNonRecursiveType(type, idl, args, typeStack);
      };

      switch (typeDef.type.kind) {
        case "struct": {
          return handleDefinedFields(
            typeDef.type.fields,
            () => undefined,
            (fields) => fields.forEach((f) => checkType(f.type)),
            (fields) => fields.forEach((f) => checkType(f))
          );
        }
        case "enum": {
          typeDef.type.variants.forEach((variant) =>
            handleDefinedFields(
              variant.fields,
              () => undefined,
              (fields) => fields.forEach((f) => checkType(f.type)),
              (fields) => fields.forEach((f) => checkType(f))
            )
          );
          return;
        }
        case "type": {
          return checkType(typeDef.type.alias);
        }
      }
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
