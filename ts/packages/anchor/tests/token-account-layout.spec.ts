import { PublicKey } from "@solana/web3.js";
import BN from "bn.js";

import { decodeTokenAccount } from "../src/program/token-account-layout";

// Field offsets of the 165-byte SPL token account base layout. A Token-2022
// account carries extension bytes after it, so a longer buffer is valid.
const MINT_OFFSET = 0;
const OWNER_OFFSET = 32;
const AMOUNT_OFFSET = 64;
const DELEGATE_OFFSET = 72;
const STATE_OFFSET = 108;
const IS_NATIVE_OFFSET = 109;
const DELEGATED_AMOUNT_OFFSET = 121;
const CLOSE_AUTHORITY_OFFSET = 129;
const TOKEN_ACCOUNT_LEN = 165;

const MINT = new PublicKey("EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v");
const OWNER = new PublicKey("7VHUFJHWu2CuExkJcJrzhQPJ2oygupTWkL2A2For4BmE");
const CLOSE_AUTHORITY = new PublicKey(
  "AgmLJBMDCqWynYnQiPCuj9ewsNNsBJXyzoUhD9LJzN51"
);
const AMOUNT = new BN("1835153240887458");

// A real mainnet Token-2022 token account,
// 4VZGjkc1HMFHa5v4vxFg5daAD7oGhC3Wr7iU8UQXG99e, read at slot 470623358: the
// 165-byte base layout, an account type byte, then three extensions
// (immutable owner, pausable, transfer hook), 179 bytes in total.
const TOKEN_2022_ACCOUNT = Buffer.from(
  "B+gucFoqWjnZiom0AoKLQ1UqLEyLkG5zxCCE1atBTRMCI1UWqRcTTGdYjEk4IK4V" +
    "XulmZVd6wbcY2kfdzyoFDmxl0QIAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA" +
    "AAAAAAAAAAAAAAAAAQAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA" +
    "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAgcAAAAbAAAADwABAAA=",
  "base64"
);
const TOKEN_2022_MINT = "XsCPL9dNWBMvFtTmwcCA5v3xWPSMEBCszbQdiLLq6aN";
const TOKEN_2022_OWNER = "9M4giFFMxmFGXtc3feFzRai56WbBqehoSeRE5GK7gf7";
const TOKEN_2022_AMOUNT = "47277420";

// `BufferLayout.struct` is untyped, so name the fields this test reads.
type TokenAccount = {
  mint: PublicKey;
  owner: PublicKey;
  amount: BN;
  delegate: PublicKey | null;
  state: Record<string, unknown>;
  closeAuthority: PublicKey | null;
};

function decode(data: Buffer): TokenAccount {
  return decodeTokenAccount(data) as TokenAccount;
}

function tokenAccount({ closeAuthority }: { closeAuthority: boolean }): Buffer {
  const b = Buffer.alloc(TOKEN_ACCOUNT_LEN);
  MINT.toBuffer().copy(b, MINT_OFFSET);
  OWNER.toBuffer().copy(b, OWNER_OFFSET);
  AMOUNT.toArrayLike(Buffer, "le", 8).copy(b, AMOUNT_OFFSET);
  b.writeUInt32LE(0, DELEGATE_OFFSET); // delegate: None
  b.writeUInt8(1, STATE_OFFSET); // state: initialized
  b.writeUInt32LE(0, IS_NATIVE_OFFSET); // isNative: None
  b.writeUInt32LE(0, DELEGATED_AMOUNT_OFFSET); // delegatedAmount: 0
  b.writeUInt32LE(closeAuthority ? 1 : 0, CLOSE_AUTHORITY_OFFSET);
  if (closeAuthority) {
    CLOSE_AUTHORITY.toBuffer().copy(b, CLOSE_AUTHORITY_OFFSET + 4);
  }
  return b;
}

describe("token account layout", () => {
  test("Decodes a complete token account", () => {
    const account = decode(tokenAccount({ closeAuthority: true }));
    expect(account.mint.toBase58()).toEqual(MINT.toBase58());
    expect(account.owner.toBase58()).toEqual(OWNER.toBase58());
    expect(account.amount.toString()).toEqual(AMOUNT.toString());
    expect(account.delegate).toBeNull();
    expect(Object.keys(account.state)).toEqual(["initialized"]);
    expect(account.closeAuthority?.toBase58()).toEqual(
      CLOSE_AUTHORITY.toBase58()
    );
  });

  test("Decodes a Token-2022 account with extension bytes", () => {
    expect(TOKEN_2022_ACCOUNT.length).toEqual(179);
    const account = decode(TOKEN_2022_ACCOUNT);
    expect(account.mint.toBase58()).toEqual(TOKEN_2022_MINT);
    expect(account.owner.toBase58()).toEqual(TOKEN_2022_OWNER);
    expect(account.amount.toString()).toEqual(TOKEN_2022_AMOUNT);
    expect(account.delegate).toBeNull();
    expect(Object.keys(account.state)).toEqual(["initialized"]);
    expect(account.closeAuthority).toBeNull();

    // The length checks are per field, not on the whole buffer, so the same
    // account truncated inside its base layout is still rejected.
    expect(() =>
      decodeTokenAccount(TOKEN_2022_ACCOUNT.subarray(0, 164))
    ).toThrow(
      /Invalid coption for "closeAuthority": need 32 bytes, only 31 remaining bytes/
    );
  });

  test("Rejects a token account truncated inside the close authority", () => {
    const data = tokenAccount({ closeAuthority: true });

    // One byte short, `blob(32)` used to clamp and hand `PublicKey` 31 bytes,
    // which is a different, valid-looking address.
    expect(() => decodeTokenAccount(data.subarray(0, 164))).toThrow(
      /Invalid pubkey for "closeAuthority": need 32 bytes, only 31 remaining bytes/
    );
    // The key missing entirely used to decode as the default address.
    expect(() => decodeTokenAccount(data.subarray(0, 133))).toThrow(
      /Invalid pubkey for "closeAuthority": need 32 bytes, only 0 remaining bytes/
    );
  });

  test("Rejects a truncated token account whose close authority is None", () => {
    const data = tokenAccount({ closeAuthority: false });

    // A `None` still occupies its 32 value bytes on chain.
    expect(() => decodeTokenAccount(data.subarray(0, 164))).toThrow(
      /Invalid coption for "closeAuthority": need 32 bytes, only 31 remaining bytes/
    );
    expect(() => decodeTokenAccount(data.subarray(0, 129))).toThrow(
      /Invalid coption for "closeAuthority": need 4 bytes, only 0 remaining bytes/
    );
  });

  test("Rejects every truncation of a token account", () => {
    // Every rejection names the field and the shortfall, with no buffer-layout
    // or Node message leaking through.
    const message =
      /^Invalid (pubkey|integer|coption|enum) for "[a-zA-Z]+": need \d+ bytes, only \d+ remaining bytes$/;
    for (const closeAuthority of [true, false]) {
      const data = tokenAccount({ closeAuthority });
      expect(decode(data).mint.toBase58()).toEqual(MINT.toBase58());
      for (let len = 0; len < TOKEN_ACCOUNT_LEN; len++) {
        expect(() => decodeTokenAccount(data.subarray(0, len))).toThrow(
          message
        );
      }
    }
  });

  test("Names the truncated field", () => {
    const data = tokenAccount({ closeAuthority: true });
    const cases: [number, RegExp][] = [
      [0, /Invalid pubkey for "mint": need 32 bytes, only 0 remaining bytes/],
      [32, /Invalid pubkey for "owner": need 32 bytes, only 0 remaining bytes/],
      [
        64,
        /Invalid integer for "amount": need 8 bytes, only 0 remaining bytes/,
      ],
      [
        72,
        /Invalid coption for "delegate": need 4 bytes, only 0 remaining bytes/,
      ],
      [
        76,
        /Invalid coption for "delegate": need 32 bytes, only 0 remaining bytes/,
      ],
      [108, /Invalid enum for "state": need 1 bytes, only 0 remaining bytes/],
      [
        109,
        /Invalid coption for "isNative": need 4 bytes, only 0 remaining bytes/,
      ],
      [
        113,
        /Invalid coption for "isNative": need 8 bytes, only 0 remaining bytes/,
      ],
      [
        121,
        /Invalid integer for "delegatedAmount": need 8 bytes, only 0 remaining bytes/,
      ],
    ];
    for (const [len, message] of cases) {
      expect(() => decodeTokenAccount(data.subarray(0, len))).toThrow(message);
    }
  });
});
