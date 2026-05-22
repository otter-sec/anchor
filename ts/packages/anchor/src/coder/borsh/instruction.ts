import bs58 from "bs58";
import { Buffer } from "buffer";
import { Layout } from "buffer-layout";
import * as borsh from "@anchor-lang/borsh";
import { AccountMeta, PublicKey } from "@solana/web3.js";
import {
  handleDefinedFields,
  Idl,
  IdlField,
  IdlGenericArg,
  IdlType,
  IdlTypeDef,
  IdlAccount,
  IdlInstructionAccountItem,
  IdlTypeVec,
  IdlInstructionAccounts,
  IdlDiscriminator,
} from "../../idl.js";
import { IdlCoder } from "./idl.js";
import { InstructionCoder } from "../index.js";

/**
 * Encodes and decodes program instructions.
 */
export class BorshInstructionCoder implements InstructionCoder {
  // Instruction args layout. Maps namespaced method
  private ixLayouts: Map<
    string,
    { discriminator: IdlDiscriminator; layout: Layout; args: IdlField[] }
  >;

  public constructor(private idl: Idl) {
    const ixLayouts = idl.instructions.map((ix) => {
      const name = ix.name;
      const fieldLayouts = ix.args.map((arg) =>
        IdlCoder.fieldLayout(arg, idl.types)
      );
      const layout = borsh.struct(fieldLayouts, name);
      return [
        name,
        { discriminator: ix.discriminator, layout, args: ix.args },
      ] as const;
    });
    this.ixLayouts = new Map(ixLayouts);
  }

  /**
   * Encodes a program instruction.
   */
  public encode(ixName: string, ix: any): Buffer {
    const buffer = Buffer.alloc(1000); // TODO: use a tighter buffer.
    const encoder = this.ixLayouts.get(ixName);
    if (!encoder) {
      throw new Error(`Unknown method: ${ixName}`);
    }

    // Validate arg shape so silent zero-encoding of typos / wrong-shape
    // payloads can't slip through. The borsh struct encoder happily writes
    // default-valued bytes for every `undefined` field it sees, which masks
    // mistakes like `encode("foo", 1000)` instead of `encode("foo", { x: 1000 })`
    // or a misspelled field name.
    if (encoder.args.length > 0) {
      if (ix === null || typeof ix !== "object" || Array.isArray(ix)) {
        const expected = encoder.args.map((a) => a.name).join(", ");
        throw new Error(
          `Invalid arguments for instruction "${ixName}": expected an object with fields { ${expected} }, got ${
            ix === null ? "null" : Array.isArray(ix) ? "array" : typeof ix
          }.`
        );
      }
    }

    const requiredMissing = encoder.args
      .filter(
        (arg) => !BorshInstructionCoder.isOption(arg.type, this.idl.types)
      )
      .map((arg) => arg.name)
      .filter((name) => !ix.hasOwnProperty(name));
    if (requiredMissing.length > 0) {
      throw new Error(
        `Invalid arguments for instruction "${ixName}": missing field${
          requiredMissing.length > 1 ? "s" : ""
        } ${requiredMissing.map((m) => `\`${m}\``).join(", ")}.`
      );
    }

    const ixWithDefinedOptions = BorshInstructionCoder.convertUndefinedOptions(
      encoder.args,
      ix,
      this.idl.types
    );
    const len = encoder.layout.encode(ixWithDefinedOptions, buffer);
    const data = buffer.slice(0, len);

    return Buffer.concat([Buffer.from(encoder.discriminator), data]);
  }

  private static isOption(
    idlType: IdlType,
    types: IdlTypeDef[] = [],
    visited = new Set<string>()
  ): boolean {
    if (typeof idlType !== "object") {
      return false;
    }
    if ("option" in idlType) {
      return true;
    }
    if ("defined" in idlType) {
      const definedName = idlType.defined.name;
      if (visited.has(definedName)) {
        return false;
      }

      const typeDef = types.find((t) => t.name === definedName);
      if (typeDef?.type.kind !== "type") {
        return false;
      }

      const alias = BorshInstructionCoder.resolveGenericType(
        typeDef.type.alias,
        typeDef,
        idlType.defined.generics
      );
      return BorshInstructionCoder.isOption(
        alias,
        types,
        new Set([...visited, definedName])
      );
    }

    return false;
  }

  private static convertUndefinedOptions(
    args: IdlField[],
    ix: any,
    types: IdlTypeDef[] = []
  ): any {
    const converted = { ...ix };
    args.forEach((arg) => {
      converted[arg.name] = BorshInstructionCoder.convertUndefinedOption(
        arg.type,
        ix[arg.name],
        types
      );
    });
    return converted;
  }

  private static convertUndefinedOption(
    idlType: IdlType,
    value: any,
    types: IdlTypeDef[]
  ): any {
    if (typeof idlType === "string") {
      return value;
    }

    if ("option" in idlType) {
      if (value === undefined) {
        return null;
      }
      if (value === null) {
        return null;
      }
      return BorshInstructionCoder.convertUndefinedOption(
        idlType.option,
        value,
        types
      );
    }

    if ("vec" in idlType) {
      return Array.isArray(value)
        ? value.map((item) =>
            BorshInstructionCoder.convertUndefinedOption(
              idlType.vec,
              item,
              types
            )
          )
        : value;
    }

    if ("array" in idlType) {
      return Array.isArray(value)
        ? value.map((item) =>
            BorshInstructionCoder.convertUndefinedOption(
              idlType.array[0],
              item,
              types
            )
          )
        : value;
    }

    if ("defined" in idlType) {
      const typeDef = types.find((t) => t.name === idlType.defined.name);
      if (!typeDef) {
        return value;
      }
      const genericArgs = idlType.defined.generics;
      return BorshInstructionCoder.convertUndefinedOptionDefined(
        typeDef,
        value,
        types,
        genericArgs
      );
    }

    return value;
  }

  private static convertUndefinedOptionDefined(
    typeDef: IdlTypeDef,
    value: any,
    types: IdlTypeDef[],
    genericArgs?: IdlGenericArg[]
  ): any {
    if (typeDef.type.kind === "type") {
      const alias = BorshInstructionCoder.resolveGenericType(
        typeDef.type.alias,
        typeDef,
        genericArgs
      );
      return BorshInstructionCoder.convertUndefinedOption(alias, value, types);
    }

    if (value === null || value === undefined || typeof value !== "object") {
      return value;
    }

    switch (typeDef.type.kind) {
      case "struct": {
        return handleDefinedFields(
          typeDef.type.fields,
          () => value,
          (fields) => {
            const converted = { ...value };
            fields.forEach((field) => {
              const fieldType = BorshInstructionCoder.resolveGenericType(
                field.type,
                typeDef,
                genericArgs
              );
              converted[field.name] =
                BorshInstructionCoder.convertUndefinedOption(
                  fieldType,
                  value[field.name],
                  types
                );
            });
            return converted;
          },
          (fields) => {
            const converted = Array.isArray(value) ? [...value] : { ...value };
            fields.forEach((field, index) => {
              const fieldType = BorshInstructionCoder.resolveGenericType(
                field,
                typeDef,
                genericArgs
              );
              converted[index] = BorshInstructionCoder.convertUndefinedOption(
                fieldType,
                value[index],
                types
              );
            });
            return converted;
          }
        );
      }
      case "enum": {
        const variantName = Object.keys(value)[0];
        const variant = typeDef.type.variants.find(
          (v) => v.name === variantName
        );
        if (!variant) {
          return value;
        }

        return {
          ...value,
          [variantName]: handleDefinedFields(
            variant.fields,
            () => value[variantName],
            (fields) => {
              const converted = { ...value[variantName] };
              fields.forEach((field) => {
                const fieldType = BorshInstructionCoder.resolveGenericType(
                  field.type,
                  typeDef,
                  genericArgs
                );
                converted[field.name] =
                  BorshInstructionCoder.convertUndefinedOption(
                    fieldType,
                    value[variantName][field.name],
                    types
                  );
              });
              return converted;
            },
            (fields) => {
              const converted = Array.isArray(value[variantName])
                ? [...value[variantName]]
                : { ...value[variantName] };
              fields.forEach((field, index) => {
                const fieldType = BorshInstructionCoder.resolveGenericType(
                  field,
                  typeDef,
                  genericArgs
                );
                converted[index] = BorshInstructionCoder.convertUndefinedOption(
                  fieldType,
                  value[variantName][index],
                  types
                );
              });
              return converted;
            }
          ),
        };
      }
    }
  }

  private static resolveGenericType(
    idlType: IdlType,
    typeDef: IdlTypeDef,
    genericArgs?: IdlGenericArg[]
  ): IdlType {
    if (!genericArgs || typeof idlType !== "object") {
      return idlType;
    }

    if ("generic" in idlType) {
      const genericIndex = typeDef.generics?.findIndex(
        (generic) => generic.kind === "type" && generic.name === idlType.generic
      );
      if (genericIndex === undefined || genericIndex < 0) {
        return idlType;
      }

      const genericArg = genericArgs[genericIndex];
      return genericArg?.kind === "type" ? genericArg.type : idlType;
    }

    if ("option" in idlType) {
      return {
        option: BorshInstructionCoder.resolveGenericType(
          idlType.option,
          typeDef,
          genericArgs
        ),
      };
    }

    if ("vec" in idlType) {
      return {
        vec: BorshInstructionCoder.resolveGenericType(
          idlType.vec,
          typeDef,
          genericArgs
        ),
      };
    }

    if ("array" in idlType) {
      return {
        array: [
          BorshInstructionCoder.resolveGenericType(
            idlType.array[0],
            typeDef,
            genericArgs
          ),
          idlType.array[1],
        ],
      };
    }

    if ("defined" in idlType) {
      return {
        defined: {
          ...idlType.defined,
          generics: idlType.defined.generics?.map((genericArg) =>
            genericArg.kind === "type"
              ? {
                  ...genericArg,
                  type: BorshInstructionCoder.resolveGenericType(
                    genericArg.type,
                    typeDef,
                    genericArgs
                  ),
                }
              : genericArg
          ),
        },
      };
    }

    return idlType;
  }

  /**
   * Decodes a program instruction.
   */
  public decode(
    ix: Buffer | string,
    encoding: "hex" | "base58" = "hex"
  ): Instruction | null {
    if (typeof ix === "string") {
      ix = encoding === "hex" ? Buffer.from(ix, "hex") : bs58.decode(ix);
    }

    for (const [name, layout] of this.ixLayouts) {
      const givenDisc = ix.subarray(0, layout.discriminator.length);
      const matches = givenDisc.equals(Buffer.from(layout.discriminator));
      if (matches) {
        return {
          name,
          data: layout.layout.decode(ix.subarray(givenDisc.length)),
        };
      }
    }

    return null;
  }

  /**
   * Returns a formatted table of all the fields in the given instruction data.
   */
  public format(
    ix: Instruction,
    accountMetas: AccountMeta[]
  ): InstructionDisplay | null {
    return InstructionFormatter.format(ix, accountMetas, this.idl);
  }
}

export type Instruction = {
  name: string;
  data: Object;
};

export type InstructionDisplay = {
  args: { name: string; type: string; data: string }[];
  accounts: {
    name?: string;
    pubkey: PublicKey;
    isSigner: boolean;
    isWritable: boolean;
  }[];
};

class InstructionFormatter {
  public static format(
    ix: Instruction,
    accountMetas: AccountMeta[],
    idl: Idl
  ): InstructionDisplay | null {
    const idlIx = idl.instructions.find((i) => ix.name === i.name);
    if (!idlIx) {
      console.error("Invalid instruction given");
      return null;
    }

    const args = idlIx.args.map((idlField) => {
      return {
        name: idlField.name,
        type: InstructionFormatter.formatIdlType(idlField.type),
        data: InstructionFormatter.formatIdlData(
          idlField,
          ix.data[idlField.name],
          idl.types
        ),
      };
    });

    const flatIdlAccounts = InstructionFormatter.flattenIdlAccounts(
      idlIx.accounts
    );

    const accounts = accountMetas.map((meta, idx) => {
      if (idx < flatIdlAccounts.length) {
        return {
          name: flatIdlAccounts[idx].name,
          ...meta,
        };
      }
      // "Remaining accounts" are unnamed in Anchor.
      else {
        return {
          name: undefined,
          ...meta,
        };
      }
    });

    return {
      args,
      accounts,
    };
  }

  private static formatIdlType(idlType: IdlType): string {
    if (typeof idlType === "string") {
      return idlType;
    }

    if ("option" in idlType) {
      return `Option<${this.formatIdlType(idlType.option)}>`;
    }
    if ("coption" in idlType) {
      return `COption<${this.formatIdlType(idlType.coption)}>`;
    }
    if ("vec" in idlType) {
      return `Vec<${this.formatIdlType(idlType.vec)}>`;
    }
    if ("array" in idlType) {
      return `Array<${idlType.array[0]}; ${idlType.array[1]}>`;
    }
    if ("defined" in idlType) {
      const name = idlType.defined.name;
      if (idlType.defined.generics) {
        const generics = idlType.defined.generics
          .map((g) => {
            switch (g.kind) {
              case "type":
                return InstructionFormatter.formatIdlType(g.type);
              case "const":
                return g.value;
            }
          })
          .join(", ");

        return `${name}<${generics}>`;
      }

      return name;
    }

    throw new Error(`Unknown IDL type: ${idlType}`);
  }

  private static formatIdlData(
    idlField: IdlField,
    data: Object,
    types?: IdlTypeDef[]
  ): string {
    if (typeof idlField.type === "string") {
      return data.toString();
    }
    if ("vec" in idlField.type) {
      return (
        "[" +
        (<Array<IdlField>>data)
          .map((d) =>
            this.formatIdlData(
              { name: "", type: (<IdlTypeVec>idlField.type).vec },
              d,
              types
            )
          )
          .join(", ") +
        "]"
      );
    }
    if ("option" in idlField.type) {
      return data === null
        ? "null"
        : this.formatIdlData(
            { name: "", type: idlField.type.option },
            data,
            types
          );
    }
    if ("defined" in idlField.type) {
      if (!types) {
        throw new Error("User defined types not provided");
      }

      const definedName = idlField.type.defined.name;
      const typeDef = types.find((t) => t.name === definedName);
      if (!typeDef) {
        throw new Error(`Type not found: ${definedName}`);
      }

      return InstructionFormatter.formatIdlDataDefined(typeDef, data, types);
    }

    return "unknown";
  }

  private static formatIdlDataDefined(
    typeDef: IdlTypeDef,
    data: Object,
    types: IdlTypeDef[]
  ): string {
    switch (typeDef.type.kind) {
      case "struct": {
        return (
          "{ " +
          handleDefinedFields(
            typeDef.type.fields,
            () => "",
            (fields) => {
              return Object.entries(data)
                .map(([key, val]) => {
                  const field = fields.find((f) => f.name === key);
                  if (!field) {
                    throw new Error(`Field not found: ${key}`);
                  }
                  return (
                    key +
                    ": " +
                    InstructionFormatter.formatIdlData(field, val, types)
                  );
                })
                .join(", ");
            },
            (fields) => {
              return Object.entries(data)
                .map(([key, val]) => {
                  return (
                    key +
                    ": " +
                    InstructionFormatter.formatIdlData(
                      { name: "", type: fields[key] },
                      val,
                      types
                    )
                  );
                })
                .join(", ");
            }
          ) +
          " }"
        );
      }

      case "enum": {
        const variantName = Object.keys(data)[0];
        const variant = typeDef.type.variants.find(
          (v) => v.name === variantName
        );
        if (!variant) {
          throw new Error(`Unable to find variant: ${variantName}`);
        }

        const enumValue = data[variantName];
        return handleDefinedFields(
          variant.fields,
          () => variantName,
          (fields) => {
            const namedFields = Object.keys(enumValue)
              .map((f) => {
                const fieldData = enumValue[f];
                const idlField = fields.find((v) => v.name === f);
                if (!idlField) {
                  throw new Error(`Field not found: ${f}`);
                }

                return (
                  f +
                  ": " +
                  InstructionFormatter.formatIdlData(idlField, fieldData, types)
                );
              })
              .join(", ");

            return `${variantName} { ${namedFields} }`;
          },
          (fields) => {
            const tupleFields = Object.entries(enumValue)
              .map(([key, val]) => {
                return (
                  key +
                  ": " +
                  InstructionFormatter.formatIdlData(
                    { name: "", type: fields[key] },
                    val as any,
                    types
                  )
                );
              })
              .join(", ");

            return `${variantName} { ${tupleFields} }`;
          }
        );
      }

      case "type": {
        return InstructionFormatter.formatIdlType(typeDef.type.alias);
      }
    }
  }

  private static flattenIdlAccounts(
    accounts: IdlInstructionAccountItem[],
    prefix?: string
  ): IdlAccount[] {
    return accounts
      .map((account) => {
        const accName = sentenceCase(account.name);
        if (account.hasOwnProperty("accounts")) {
          const newPrefix = prefix ? `${prefix} > ${accName}` : accName;
          return InstructionFormatter.flattenIdlAccounts(
            (<IdlInstructionAccounts>account).accounts,
            newPrefix
          );
        } else {
          return {
            ...(<IdlAccount>account),
            name: prefix ? `${prefix} > ${accName}` : accName,
          };
        }
      })
      .flat();
  }
}

function sentenceCase(field: string): string {
  const result = field.replace(/([A-Z])/g, " $1");
  return result.charAt(0).toUpperCase() + result.slice(1);
}
