import { Buffer } from "buffer";
import { getBase58Codec } from "@solana/kit";

export function encode(data: Buffer | number[] | Uint8Array): string {
  return getBase58Codec().decode(Uint8Array.from(data));
}

export function decode(data: string): Buffer {
  return Buffer.from(getBase58Codec().encode(data));
}
