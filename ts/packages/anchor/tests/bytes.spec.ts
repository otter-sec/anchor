import { decode } from "../src/utils/bytes/hex";

describe("hex bytes", () => {
  it.each(["gg", "0x1g"])("rejects invalid hex input %s", (input) => {
    expect(() => decode(input)).toThrow("Invalid hex string");
  });

  it("continues to left-pad odd-length input", () => {
    expect(decode("abc")).toEqual(Buffer.from([0x0a, 0xbc]));
  });
});
