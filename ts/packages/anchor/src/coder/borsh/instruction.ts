import {
  AccountMeta,
  getBase16Encoder,
  getBase58Encoder,
  getStructCodec,
  ReadonlyUint8Array,
} from "@solana/kit";
import {
  handleDefinedFields,
  Idl,
  IdlField,
  IdlType,
  IdlTypeDef,
  IdlAccount,
  IdlInstructionAccountItem,
  IdlTypeVec,
  IdlInstructionAccounts,
} from "../../idl.js";
import { IdlCoder } from "./idl.js";
import {
  DiscriminatedIdlCodec,
  getDiscriminatedIdlCodec,
  IdlCodec,
} from "./codecs.js";
import { InstructionCoder } from "../index.js";

/**
 * Encodes and decodes program instructions.
 */
export class BorshInstructionCoder implements InstructionCoder {
  // Instruction args codec, prefixed by the instruction discriminator.
  private ixCodecs: Map<string, DiscriminatedIdlCodec>;

  public constructor(private idl: Idl) {
    const ixCodecs = idl.instructions.map((ix) => {
      const fieldCodecs = ix.args.map((arg): [string, IdlCodec] => [
        arg.name,
        IdlCoder.fieldCodec(arg, idl.types),
      ]);
      return [
        ix.name,
        getDiscriminatedIdlCodec(ix.discriminator, getStructCodec(fieldCodecs)),
      ] as const;
    });
    this.ixCodecs = new Map(ixCodecs);
  }

  /**
   * Encodes a program instruction.
   */
  public encode(ixName: string, ix: any): ReadonlyUint8Array {
    const codec = this.ixCodecs.get(ixName);
    if (!codec) {
      throw new Error(`Unknown method: ${ixName}`);
    }

    return codec.encode(ix);
  }

  /**
   * Decodes a program instruction.
   */
  public decode(
    ix: ReadonlyUint8Array | string,
    encoding: "hex" | "base58" = "hex"
  ): Instruction | null {
    const bytes =
      typeof ix === "string"
        ? (encoding === "hex" ? getBase16Encoder() : getBase58Encoder()).encode(
            ix
          )
        : ix;

    for (const [name, codec] of this.ixCodecs) {
      if (codec.matches(bytes)) {
        return { name, data: codec.decode(bytes) as Object };
      }
    }

    return null;
  }

  /**
   * Returns a formatted table of all the fields in the given instruction data.
   */
  public format(
    ix: Instruction,
    accountMetas: readonly AccountMeta[]
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
  /** The instruction's account metas, named after the IDL where known. */
  accounts: (AccountMeta & { name?: string })[];
};

class InstructionFormatter {
  public static format(
    ix: Instruction,
    accountMetas: readonly AccountMeta[],
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
