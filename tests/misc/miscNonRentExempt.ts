import * as anchor from "@anchor-lang/core";
import { Program, AnchorError } from "@anchor-lang/core";
import {
  PublicKey,
  Keypair,
  SystemProgram,
  SYSVAR_RENT_PUBKEY,
} from "@solana/web3.js";
// @ts-expect-error
import { Misc } from "../../target/types/misc";
const { assert } = require("chai");

describe("miscNonRentExempt", () => {
  // Configure the client to use the local cluster.
  const provider = anchor.AnchorProvider.env();
  anchor.setProvider(provider);
  const program = anchor.workspace.Misc as Program<Misc>;

  it("init_if_needed checks rent_exemption if init is not needed", async () => {
    const data = Keypair.generate();
    await program.rpc.initDecreaseLamports({
      accounts: {
        data: data.publicKey,
        user: provider.wallet.publicKey,
        systemProgram: SystemProgram.programId,
      },
      signers: [data],
    });

    try {
      await program.rpc.initIfNeededChecksRentExemption({
        accounts: {
          data: data.publicKey,
          user: provider.wallet.publicKey,
          systemProgram: SystemProgram.programId,
        },
        signers: [data],
      });
      assert.ok(false);
    } catch (_err) {
      assert.isTrue(_err instanceof AnchorError);
      const err: AnchorError = _err;
      assert.strictEqual(err.error.errorCode.number, 2005);
    }
  });

  it("allows non-rent exempt accounts", async () => {
    const data = Keypair.generate();
    await program.rpc.initializeNoRentExempt({
      accounts: {
        data: data.publicKey,
        rent: SYSVAR_RENT_PUBKEY,
      },
      signers: [data],
      instructions: [
        SystemProgram.createAccount({
          programId: program.programId,
          space: 8 + 16 + 16,
          lamports:
            await program.provider.connection.getMinimumBalanceForRentExemption(
              39
            ),
          fromPubkey: provider.wallet.publicKey,
          newAccountPubkey: data.publicKey,
        }),
      ],
    });
    await program.rpc.testNoRentExempt({
      accounts: {
        data: data.publicKey,
      },
    });
  });

  it("allows rent exemption to be skipped", async () => {
    const data = anchor.web3.Keypair.generate();
    await program.rpc.initializeSkipRentExempt({
      accounts: {
        data: data.publicKey,
        rent: SYSVAR_RENT_PUBKEY,
      },
      signers: [data],
      instructions: [
        SystemProgram.createAccount({
          programId: program.programId,
          space: 8 + 16 + 16,
          lamports:
            await program.provider.connection.getMinimumBalanceForRentExemption(
              39
            ),
          fromPubkey: provider.wallet.publicKey,
          newAccountPubkey: data.publicKey,
        }),
      ],
    });
  });

  it("can use rent_exempt to enforce rent exemption", async () => {
    const data = Keypair.generate();
    await program.rpc.initializeSkipRentExempt({
      accounts: {
        data: data.publicKey,
        rent: SYSVAR_RENT_PUBKEY,
      },
      signers: [data],
      instructions: [
        SystemProgram.createAccount({
          programId: program.programId,
          space: 8 + 16 + 16,
          lamports:
            await program.provider.connection.getMinimumBalanceForRentExemption(
              39
            ),
          fromPubkey: provider.wallet.publicKey,
          newAccountPubkey: data.publicKey,
        }),
      ],
    });

    try {
      await program.rpc.testEnforceRentExempt({
        accounts: {
          data: data.publicKey,
        },
      });
      assert.ok(false);
    } catch (_err) {
      assert.isTrue(_err instanceof AnchorError);
      const err: AnchorError = _err;
      assert.strictEqual(err.error.errorCode.number, 2005);
      assert.strictEqual(
        "A rent exemption constraint was violated",
        err.error.errorMessage
      );
    }
  });
});
