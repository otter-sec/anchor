import { bs58 } from "../src/utils/bytes";

describe("utils/bytes/bs58", () => {
  it("encodes bytes to base58", () => {
    expect(bs58.encode(Buffer.from([0, 1, 2]))).toBe("15T");
    expect(bs58.encode([0, 1, 2])).toBe("15T");
    expect(bs58.encode(new Uint8Array([0, 1, 2]))).toBe("15T");
  });

  it("decodes base58 to a Buffer", () => {
    const decoded = bs58.decode("15T");
    expect(Buffer.isBuffer(decoded)).toBe(true);
    expect([...decoded]).toStrictEqual([0, 1, 2]);
  });

  it("round-trips 32-byte addresses", () => {
    const address = "J2XMGdW2qQLx7rAdwWtSZpTXDgAQ988BLP9QTgUZvm54";
    expect(bs58.encode(bs58.decode(address))).toBe(address);
  });
});
