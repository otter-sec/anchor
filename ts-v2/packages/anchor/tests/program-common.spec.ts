import BN from "bn.js";
import bs58 from "bs58";
import { PublicKey } from "@solana/web3.js";

import NodeWallet from "../src/nodewallet";
import { translateAddress } from "../src/program/common";

describe("program/common", () => {
  describe("translateAddress", () => {
    it("should accept a valid string address", () => {
      const address = "11111111111111111111111111111111";

      const func = () => translateAddress(address);
      expect(func).not.toThrow();

      const output = func();
      expect(output).toBeInstanceOf(PublicKey);
      expect(new PublicKey(address).equals(output)).toBeTruthy();
    });

    it("should accept a PublicKey address", () => {
      const publicKey = new PublicKey("11111111111111111111111111111111");

      const func = () => translateAddress(publicKey);
      expect(func).not.toThrow();

      const output = func();
      expect(output).toBeInstanceOf(PublicKey);
      expect(new PublicKey(publicKey).equals(output)).toBe(true);
    });

    it("should accept an object with a PublicKey shape { _bn }", () => {
      const obj = {
        _bn: new BN(bs58.decode("11111111111111111111111111111111")),
      } as any as PublicKey;
      const func = () => translateAddress(obj);

      expect(func).not.toThrow();
      const output = func();

      expect(output).toBeInstanceOf(PublicKey);
      expect(new PublicKey(obj).equals(output)).toBe(true);
    });

    it("should not accept an invalid string address", () => {
      const invalid = "invalid";
      const func = () => translateAddress(invalid);
      expect(func).toThrow();
    });

    it("should not accept an invalid object", () => {
      const invalid = {} as PublicKey;
      const func = () => translateAddress(invalid);
      expect(func).toThrow();
    });
  });

  describe("NodeWallet", () => {
    it("should throw an error when ANCHOR_WALLET is unset", () => {
      const oldValue = process.env.ANCHOR_WALLET;
      delete process.env.ANCHOR_WALLET;

      expect(() => NodeWallet.local()).toThrowError(
        "expected environment variable `ANCHOR_WALLET` is not set."
      );

      process.env.ANCHOR_WALLET = oldValue;
    });
  });
});
