import { address, FetchAccountConfig } from "@solana/kit";
import { PublicKey } from "@solana/web3.js";

import { createLocalWallet } from "../src/wallet";
import { toAddress } from "../src/program/common";
import { withProviderDefaults } from "../src/utils/common";

describe("program/common", () => {
  describe("toAddress", () => {
    it("accepts a Kit address", () => {
      const input = address("11111111111111111111111111111111");
      expect(toAddress(input)).toBe(input);
    });

    it("accepts an object exposing toBase58, e.g. a web3.js PublicKey", () => {
      const publicKey = new PublicKey("11111111111111111111111111111111");
      expect(toAddress(publicKey)).toBe("11111111111111111111111111111111");
      expect(
        toAddress({ toBase58: () => "11111111111111111111111111111111" })
      ).toBe("11111111111111111111111111111111");
    });

    it("rejects invalid addresses", () => {
      expect(() => toAddress("invalid" as any)).toThrow();
      expect(() => toAddress({ toBase58: () => "invalid" })).toThrow();
      expect(() => toAddress({} as any)).toThrow();
    });
  });

  describe("withProviderDefaults", () => {
    const provider = { opts: { commitment: "processed" as const } };

    it("fills the commitment from the provider", () => {
      const config: FetchAccountConfig = { minContextSlot: 5n };
      expect(withProviderDefaults(provider, config)).toEqual({
        commitment: "processed",
        minContextSlot: 5n,
      });
    });

    it("lets the caller's commitment win", () => {
      expect(
        withProviderDefaults(provider, { commitment: "finalized" })
      ).toEqual({ commitment: "finalized" });
    });

    it("never emits an undefined commitment", () => {
      // Kit would strip it without applying its own default.
      const config = withProviderDefaults({}, { commitment: undefined });
      expect("commitment" in config).toBe(false);
      expect(withProviderDefaults({ opts: {} })).toEqual({});
    });
  });

  describe("createLocalWallet", () => {
    it("should throw an error when ANCHOR_WALLET is unset", () => {
      const oldValue = process.env.ANCHOR_WALLET;
      delete process.env.ANCHOR_WALLET;

      expect(() => createLocalWallet()).toThrow(
        "expected environment variable `ANCHOR_WALLET` is not set."
      );

      process.env.ANCHOR_WALLET = oldValue;
    });
  });
});
