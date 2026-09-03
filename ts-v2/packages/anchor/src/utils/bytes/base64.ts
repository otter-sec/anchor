import { Buffer } from "buffer";

export function encode(data: Buffer): string {
  return data.toString("base64");
}

export function decode(data: string): Buffer {
  return Buffer.from(data, "base64");
}
