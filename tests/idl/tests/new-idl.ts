import * as anchor from "@anchor-lang/core";
import { assert } from "chai";

import type { NewIdl } from "../target/types/new_idl";

describe("New IDL", () => {
  anchor.setProvider(anchor.AnchorProvider.env());
  const program = anchor.workspace.newIdl as anchor.Program<NewIdl>;

  describe("Case conversion", () => {
    const caseConversionAccountKp = anchor.web3.Keypair.generate();
    const FIELD_NAME = 5;

    it("Works when instructions have no case conversion in IDL", async () => {
      const ixName = "no_case_conversion";
      const ix = program.rawIdl.instructions.find((ix) => ix.name === ixName);
      if (!ix) throw new Error(`Instruction \`${ixName}\` not found`);

      await program.methods
        .noCaseConversion(FIELD_NAME)
        .accounts({ caseConversionAccount: caseConversionAccountKp.publicKey })
        .signers([caseConversionAccountKp])
        .rpc();
    });

    it("Works when accounts have no case conversion in IDL", async () => {
      const accName = "SimpleAccount";
      const acc = program.rawIdl.accounts!.find((acc) => acc.name === accName);
      if (!acc) throw new Error(`Account \`${accName}\` not found`);

      const caseConversionAccount = await program.account.simpleAccount.fetch(
        caseConversionAccountKp.publicKey
      );
      assert.strictEqual(caseConversionAccount.fieldName, FIELD_NAME);
    });

    it("Works when events have no case conversion in IDL", async () => {
      const eventName = "SimpleEvent";
      const event = program.rawIdl.events!.find((ev) => ev.name === eventName);
      if (!event) throw new Error(`Event \`${eventName}\` not found`);

      await new Promise<void>(async (res) => {
        const id = program.addEventListener("simpleEvent", (ev) => {
          program.removeEventListener(id);
          assert.strictEqual(ev.fieldName, FIELD_NAME);
          res();
        });

        const caseConversionAccountKp = anchor.web3.Keypair.generate();
        await program.methods
          .noCaseConversion(FIELD_NAME)
          .accounts({
            caseConversionAccount: caseConversionAccountKp.publicKey,
          })
          .signers([caseConversionAccountKp])
          .rpc();
      });
    });
  });

  describe("Client interaction", () => {
    it("Can send empty ix(no arg, no account)", async () => {
      await program.methods.empty().rpc();
    });

    it("Can use primitive types", async () => {
      const kp = anchor.web3.Keypair.generate();

      const bool = true;

      const i8 = -3;
      const i16 = 1;
      const i32 = -5555551;
      const i64 = new anchor.BN("384535471");
      const i128 = new anchor.BN(-8342491);

      const u8 = 123;
      const u16 = 7888;
      const u32 = 5555551;
      const u64 = new anchor.BN("384535471");
      const u128 = new anchor.BN(8888888);

      const f32 = 1.0;
      const f64 = 0.618;

      const pubkey = anchor.web3.PublicKey.default;

      await program.methods
        .primitiveTypes(
          bool,
          i8,
          i16,
          i32,
          i64,
          i128,
          u8,
          u16,
          u32,
          u64,
          u128,
          f32,
          f64,
          pubkey
        )
        .accounts({ account: kp.publicKey })
        .signers([kp])
        .preInstructions([
          await program.account.primitiveAccount.createInstruction(kp),
        ])
        .rpc();

      const account = await program.account.primitiveAccount.fetch(
        kp.publicKey
      );
      assert.strictEqual(account.bool, bool);

      assert.strictEqual(account.i8, i8);
      assert.strictEqual(account.i16, i16);
      assert.strictEqual(account.i32, i32);
      assert(account.i64.eq(i64));
      assert(account.i128.eq(i128));

      assert.strictEqual(account.u8, u8);
      assert.strictEqual(account.u16, u16);
      assert.strictEqual(account.u32, u32);
      assert(account.u64.eq(u64));
      assert(account.u128.eq(u128));

      assert.strictEqual(account.f32, f32);
      assert.strictEqual(account.f64, f64);

      assert(account.pubkey.equals(pubkey));
    });

    it("Can use unsized types", async () => {
      const kp = anchor.web3.Keypair.generate();

      const string = "anchor";
      const bytes = Buffer.from([1, 2, 3, 4]);
      await program.methods
        .unsizedTypes(string, bytes)
        .accounts({ account: kp.publicKey })
        .signers([kp])
        .preInstructions([
          await program.account.primitiveAccount.createInstruction(kp),
        ])
        .rpc();

      const account = await program.account.unsizedAccount.fetch(kp.publicKey);
      assert.strictEqual(account.string, string);
      assert(account.bytes.equals(bytes));
    });

    it("Can use struct", async () => {
      const unitStructArg = {} as const;
      const namedStructArg = {
        u8: 1,
        u16: 11,
        u32: 111,
        u64: new anchor.BN(1111),
      } as const;
      const tupleStructArg = [new anchor.BN(23), "tuple"] as const;

      const kp = anchor.web3.Keypair.generate();
      await program.methods
        .strct(unitStructArg, namedStructArg, tupleStructArg)
        .accounts({ account: kp.publicKey })
        .signers([kp])
        .preInstructions([
          await program.account.structAccount.createInstruction(kp, 1024),
        ])
        .rpc();

      const struct = await program.account.structAccount.fetch(kp.publicKey);

      // Unit
      assert.deepEqual(struct.unit, unitStructArg);

      // Named
      assert.strictEqual(struct.named.u8, namedStructArg.u8);
      assert.strictEqual(struct.named.u16, namedStructArg.u16);
      assert.strictEqual(struct.named.u32, namedStructArg.u32);
      assert(struct.named.u64.eq(namedStructArg.u64));

      // Tuple
      assert(struct.tuple[0].eq(tupleStructArg[0]));
      assert.strictEqual(struct.tuple[1], tupleStructArg[1]);
    });

    it("Can use enum", async () => {
      const testAccountEnum = async (
        ...args: Parameters<typeof program["methods"]["enm"]>
      ) => {
        const kp = anchor.web3.Keypair.generate();
        await program.methods
          .enm(...(args as any))
          .accounts({ account: kp.publicKey })
          .signers([kp])
          .preInstructions([
            await program.account.enumAccount.createInstruction(kp),
          ])
          .rpc();
        return await program.account.enumAccount.fetch(kp.publicKey);
      };

      // Unit
      const unit = await testAccountEnum({ unit: {} });
      assert.deepEqual(unit.fullEnum.unit, {});

      // Named
      const pointX = new anchor.BN(1);
      const pointY = new anchor.BN(2);
      const named = await testAccountEnum({ named: { pointX, pointY } });
      if (!named.fullEnum.named) throw new Error("Named not crated");
      assert(named.fullEnum.named.pointX.eq(pointX));
      assert(named.fullEnum.named.pointY.eq(pointY));

      // Unnamed
      const tupleArg = [1, 2, 3, 4] as const;
      const unnamed = await testAccountEnum({ unnamed: tupleArg });
      if (!unnamed.fullEnum.unnamed) throw new Error("Unnamed not crated");
      assert(
        Object.entries(unnamed.fullEnum.unnamed).every(
          ([key, value]) => value === tupleArg[key as keyof typeof tupleArg]
        )
      );

      // Unnamed struct
      const tupleStructArg = [
        { u8: 1, u16: 11, u32: 111, u64: new anchor.BN(1111) },
      ] as const;
      const unnamedStruct = await testAccountEnum({
        unnamedStruct: tupleStructArg,
      });
      if (!unnamedStruct.fullEnum.unnamedStruct) {
        throw new Error("Unnamed struct not crated");
      }
      assert.strictEqual(
        unnamedStruct.fullEnum.unnamedStruct[0].u8,
        tupleStructArg[0].u8
      );
      assert.strictEqual(
        unnamedStruct.fullEnum.unnamedStruct[0].u16,
        tupleStructArg[0].u16
      );
      assert.strictEqual(
        unnamedStruct.fullEnum.unnamedStruct[0].u32,
        tupleStructArg[0].u32
      );
      assert(
        unnamedStruct.fullEnum.unnamedStruct[0].u64.eq(tupleStructArg[0].u64)
      );
    });

    it("Can use type aliases", async () => {
      const kp = anchor.web3.Keypair.generate();

      const aliasU8 = 42;
      const aliasU8Array = [1, 2, 3, 4, 5, 6, 7, 8];
      const aliasStruct = {
        u8: 1,
        u16: 2,
        u32: 3,
        u64: new anchor.BN(4),
      };
      const aliasVecString = ["first", "second"];
      const aliasOptionVecPubkey = [anchor.web3.Keypair.generate().publicKey];
      const aliasGenericConst = [1, 23045, 32, 4];
      const aliasMultipleGenericsMixed = [
        [true, false],
        [false, true],
      ];
      const aliasExternal = new anchor.BN(1708705033);

      await program.methods
        .typeAlias(
          aliasU8,
          aliasU8Array,
          aliasStruct,
          aliasVecString,
          aliasOptionVecPubkey,
          aliasGenericConst,
          aliasMultipleGenericsMixed,
          aliasExternal
        )
        .accounts({ account: kp.publicKey })
        .signers([kp])
        .preInstructions([
          await program.account.aliasAccount.createInstruction(kp, 1024),
        ])
        .rpc();

      const account = await program.account.aliasAccount.fetch(kp.publicKey);

      assert.strictEqual(account.aliasU8, aliasU8);
      assert.deepEqual(account.aliasU8Array, aliasU8Array);
      assert.strictEqual(account.aliasStruct.u8, aliasStruct.u8);
      assert.strictEqual(account.aliasStruct.u16, aliasStruct.u16);
      assert.strictEqual(account.aliasStruct.u32, aliasStruct.u32);
      assert(account.aliasStruct.u64.eq(aliasStruct.u64));
      assert.deepEqual(account.aliasVecString, aliasVecString);
      assert.deepEqual(account.aliasOptionVecPubkey, aliasOptionVecPubkey);
      assert.deepEqual(account.aliasGenericConst, aliasGenericConst);
      assert.deepEqual(
        account.aliasMultipleGenericsMixed,
        aliasMultipleGenericsMixed
      );
      assert(account.aliasExternal.eq(aliasExternal));
    });

    it("Can use accounts and events as arguments and fields", async () => {
      const kp = anchor.web3.Keypair.generate();

      const accountArg = {
        simpleAccount: { fieldName: 2 },
        simpleEvent: { fieldName: 4 },
      };
      await program.methods
        .accountAndEventArgAndField(accountArg)
        .accounts({ account: kp.publicKey })
        .signers([kp])
        .preInstructions([
          await program.account.accountAndEventFieldAccount.createInstruction(
            kp
          ),
        ])
        .rpc();

      const account = await program.account.accountAndEventFieldAccount.fetch(
        kp.publicKey
      );
      assert.deepEqual(account, accountArg);
    });

    it("Can use generics", async () => {
      const arg = {
        arr: [6, 123, 2, 3],
        subField: {
          subArr: new Array(8).fill(null).map((_, i) => i + 33),
          another: [30211, 65050, 21, 441],
        },
      };
      const { pubkeys } = await program.methods.generic(arg).rpcAndKeys();
      const myAccount = await program.account.genericAccount.fetch(
        pubkeys.myAccount
      );
      assert.deepEqual(myAccount.field, arg);
    });

    it("Can use generics populated with custom struct", async () => {
      const arg = {
        arr: [{ field: 1 }, { field: 2 }, { field: 3 }, { field: 4 }],
        subField: {
          subArr: new Array(8).fill(null).map((_, i) => ({ field: i })),
          another: [
            { field: 42 },
            { field: 420 },
            { field: 4_200 },
            { field: 42_000 },
          ],
        },
      };
      const { pubkeys } = await program.methods
        .genericCustomStruct(arg)
        .rpcAndKeys();
      const myAccount = await program.account.genericAccountCustomStruct.fetch(
        pubkeys.myAccount
      );
      assert.deepEqual(myAccount.field, arg);
    });

    it("Can use full module path types", async () => {
      const kp = anchor.web3.Keypair.generate();

      const namedStructArg = { u8: 1, u16: 2, u32: 3, u64: new anchor.BN(4) };
      const someModuleNamedStructArg = { data: 5 };

      await program.methods
        .fullPath(namedStructArg, someModuleNamedStructArg)
        .accounts({ account: kp.publicKey })
        .preInstructions([
          await program.account.fullPathAccount.createInstruction(kp),
        ])
        .signers([kp])
        .rpc();

      const fullPathAccount = await program.account.fullPathAccount.fetch(
        kp.publicKey
      );
      assert.strictEqual(fullPathAccount.namedStruct.u8, namedStructArg.u8);
      assert.strictEqual(fullPathAccount.namedStruct.u16, namedStructArg.u16);
      assert.strictEqual(fullPathAccount.namedStruct.u32, namedStructArg.u32);
      assert(fullPathAccount.namedStruct.u64.eq(namedStructArg.u64));
      assert.deepEqual(
        fullPathAccount.someModuleNamedStruct,
        someModuleNamedStructArg
      );
    });

    it("Can use external types", async () => {
      const externalArg = { someField: 5 };

      const kp = anchor.web3.Keypair.generate();
      await program.methods
        .external(externalArg)
        .accounts({ account: kp.publicKey })
        .signers([kp])
        .preInstructions([
          await program.account.accountWithExternalField.createInstruction(kp),
        ])
        .rpc();

      const account = await program.account.accountWithExternalField.fetch(
        kp.publicKey
      );

      assert.deepEqual(account.myStruct, externalArg);
    });

    it("Can use non-Anchor external types", async () => {
      const feature = { activatedAt: new anchor.BN(42) };

      const kp = anchor.web3.Keypair.generate();
      await program.methods
        .externalNonAnchor(feature)
        .accounts({ account: kp.publicKey })
        .signers([kp])
        .preInstructions([
          await program.account.accountWithNonAnchorExternalField.createInstruction(
            kp
          ),
        ])
        .rpc();

      const account =
        await program.account.accountWithNonAnchorExternalField.fetch(
          kp.publicKey
        );

      assert(account.feature.activatedAt?.eq(feature.activatedAt));
    });
  });

  describe("Format", () => {
    const ixCoder = new anchor.BorshInstructionCoder(program.idl);

    const formatEnum = async (argName: string, data: any, expected: string) => {
      const typeName = Object.keys(data)[0];
      const arg = data[typeName];
      const ix = await program.methods
        .enm(arg)
        .accounts({ account: anchor.web3.PublicKey.default })
        .instruction();

      const formattedIx = ixCoder.format({ name: "enm", data }, ix.keys);
      if (!formattedIx) throw new Error("Failed to format");

      assert.deepEqual(formattedIx.args[0], {
        name: argName,
        type: typeName,
        data: expected,
      });
    };

    it("Can format unit enum", async () => {
      await formatEnum("fullEnum", { fullEnum: { unit: {} } }, "unit");
    });

    it("Can format named enum", async () => {
      await formatEnum(
        "fullEnum",
        {
          fullEnum: {
            named: { pointX: new anchor.BN(1), pointY: new anchor.BN(2) },
          },
        },
        "named { pointX: 1, pointY: 2 }"
      );
    });

    it("Can format tuple enum", async () => {
      await formatEnum(
        "fullEnum",
        { fullEnum: { unnamed: [2, 10, 200, 49] } },
        "unnamed { 0: 2, 1: 10, 2: 200, 3: 49 }"
      );
    });
  });
});
