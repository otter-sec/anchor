// @ts-nocheck
import * as B from "@native-to-anchor/buffer-layout";
import { AccountsCoder, Idl } from "@anchor-lang/core";
import { IdlTypeDef } from "@anchor-lang/core/dist/cjs/idl";

export class SplStakePoolAccountsCoder<A extends string = string>
  implements AccountsCoder
{
  constructor(_idl: Idl) {}

  public async encode<T = any>(accountName: A, account: T): Promise<Buffer> {
    switch (accountName) {
      case "stakePool": {
        const buffer = Buffer.alloc(10485760); // Space is variable
        const len = STAKE_POOL_LAYOUT.encode(account, buffer);
        return buffer.slice(0, len);
      }
      case "validatorStakeInfo": {
        const buffer = Buffer.alloc(73);
        const len = VALIDATOR_STAKE_INFO_LAYOUT.encode(account, buffer);
        return buffer.slice(0, len);
      }
      case "validatorList": {
        const buffer = Buffer.alloc(10485760); // Space is variable
        const len = VALIDATOR_LIST_LAYOUT.encode(account, buffer);
        return buffer.slice(0, len);
      }
      default: {
        throw new Error(`Invalid account name: ${accountName}`);
      }
    }
  }

  public decode<T = any>(accountName: A, ix: Buffer): T {
    return this.decodeUnchecked(accountName, ix);
  }

  public decodeUnchecked<T = any>(accountName: A, ix: Buffer): T {
    switch (accountName) {
      case "stakePool": {
        return decodeStakePoolAccount(ix);
      }
      case "validatorStakeInfo": {
        return decodeValidatorStakeInfoAccount(ix);
      }
      case "validatorList": {
        return decodeValidatorListAccount(ix);
      }
      default: {
        throw new Error(`Invalid account name: ${accountName}`);
      }
    }
  }

  public memcmp(
    accountName: A,
    _appendData?: Buffer
  ): { dataSize?: number; offset?: number; bytes?: string } {
    switch (accountName) {
      case "stakePool": {
        return {
          // Space is variable
        };
      }
      case "validatorStakeInfo": {
        return {
          dataSize: 73,
        };
      }
      case "validatorList": {
        return {
          // Space is variable
        };
      }
      default: {
        throw new Error(`Invalid account name: ${accountName}`);
      }
    }
  }

  public size(idlAccount: IdlTypeDef): number {
    switch (idlAccount.name) {
      case "stakePool": {
        return 0; // Space is variable;
      }
      case "validatorStakeInfo": {
        return 73;
      }
      case "validatorList": {
        return 0; // Space is variable;
      }
      default: {
        throw new Error(`Invalid account name: ${idlAccount.name}`);
      }
    }
  }
}

function decodeStakePoolAccount<T = any>(ix: Buffer): T {
  return STAKE_POOL_LAYOUT.decode(ix) as T;
}
function decodeValidatorStakeInfoAccount<T = any>(ix: Buffer): T {
  return VALIDATOR_STAKE_INFO_LAYOUT.decode(ix) as T;
}
function decodeValidatorListAccount<T = any>(ix: Buffer): T {
  return VALIDATOR_LIST_LAYOUT.decode(ix) as T;
}

const STAKE_POOL_LAYOUT: any = B.struct([
  ((p: string) => {
    const U = B.union(B.u8("discriminator"), null, p);
    U.addVariant(0, B.struct([]), "uninitialized");
    U.addVariant(1, B.struct([]), "stakePool");
    U.addVariant(2, B.struct([]), "validatorList");
    return U;
  })("accountType"),
  B.publicKey("manager"),
  B.publicKey("staker"),
  B.publicKey("stakeDepositAuthority"),
  B.u8("stakeWithdrawBumpSeed"),
  B.publicKey("validatorList"),
  B.publicKey("reserveStake"),
  B.publicKey("poolMint"),
  B.publicKey("managerFeeAccount"),
  B.publicKey("tokenProgramId"),
  B.u64("totalLamports"),
  B.u64("poolTokenSupply"),
  B.u64("lastUpdateEpoch"),
  B.u8("lockup"),
  B.struct([B.u64("denominator"), B.u64("numerator")], "epochFee"),
  B.option(
    B.struct([B.u64("denominator"), B.u64("numerator")]),
    "nextEpochFee"
  ),
  B.option(B.publicKey(), "preferredDepositValidatorVoteAddress"),
  B.option(B.publicKey(), "preferredWithdrawValidatorVoteAddress"),
  B.struct([B.u64("denominator"), B.u64("numerator")], "stakeDepositFee"),
  B.struct([B.u64("denominator"), B.u64("numerator")], "stakeWithdrawalFee"),
  B.option(
    B.struct([B.u64("denominator"), B.u64("numerator")]),
    "nextStakeWithdrawalFee"
  ),
  B.u8("stakeReferralFee"),
  B.option(B.publicKey(), "solDepositAuthority"),
  B.struct([B.u64("denominator"), B.u64("numerator")], "solDepositFee"),
  B.u8("solReferralFee"),
  B.option(B.publicKey(), "solWithdrawAuthority"),
  B.struct([B.u64("denominator"), B.u64("numerator")], "solWithdrawalFee"),
  B.option(
    B.struct([B.u64("denominator"), B.u64("numerator")]),
    "nextSolWithdrawalFee"
  ),
  B.u64("lastEpochPoolTokenSupply"),
  B.u64("lastEpochTotalLamports"),
]);

const VALIDATOR_STAKE_INFO_LAYOUT: any = B.struct([
  B.u64("activeStakeLamports"),
  B.u64("transientStakeLamports"),
  B.u64("lastUpdateEpoch"),
  B.u64("transientSeedSuffixStart"),
  B.u64("transientSeedSuffixEnd"),
  ((p: string) => {
    const U = B.union(B.u8("discriminator"), null, p);
    U.addVariant(0, B.struct([]), "active");
    U.addVariant(1, B.struct([]), "deactivatingTransient");
    U.addVariant(2, B.struct([]), "readyForRemoval");
    return U;
  })("status"),
  B.publicKey("voteAccountAddress"),
]);

const VALIDATOR_LIST_LAYOUT: any = B.struct([
  B.struct(
    [
      ((p: string) => {
        const U = B.union(B.u8("discriminator"), null, p);
        U.addVariant(0, B.struct([]), "uninitialized");
        U.addVariant(1, B.struct([]), "stakePool");
        U.addVariant(2, B.struct([]), "validatorList");
        return U;
      })("accountType"),
      B.u32("maxValidators"),
    ],
    "header"
  ),
  B.vec(
    B.struct([
      B.u64("activeStakeLamports"),
      B.u64("transientStakeLamports"),
      B.u64("lastUpdateEpoch"),
      B.u64("transientSeedSuffixStart"),
      B.u64("transientSeedSuffixEnd"),
      ((p: string) => {
        const U = B.union(B.u8("discriminator"), null, p);
        U.addVariant(0, B.struct([]), "active");
        U.addVariant(1, B.struct([]), "deactivatingTransient");
        U.addVariant(2, B.struct([]), "readyForRemoval");
        return U;
      })("status"),
      B.publicKey("voteAccountAddress"),
    ]),
    "validators"
  ),
]);
