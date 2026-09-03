import { Buffer } from "buffer";

export function encode(data: Buffer): string {
  return data.reduce(
    (str, byte) => str + byte.toString(16).padStart(2, "0"),
    "0x"
  );
}

export function decode(data: string): Buffer {
  if (data.indexOf("0x") === 0) {
    data = data.substr(2);
  }
  if (data.length % 2 === 1) {
    data = "0" + data;
  }

  let key = data.match(/.{2}/g);

  if (key === null) {
    return Buffer.from([]);
  }

  return Buffer.from(key.map((byte) => parseInt(byte, 16)));
}
