/**
 * Convert an identifier to `lowerCamelCase`, matching the Rust-side
 * `heck::to_lower_camel_case` used during IDL generation. Unlike the npm
 * `camelcase` package, digit-letter transitions don't introduce a new word,
 * so `a1bReceive` stays `a1bReceive` instead of becoming `a1BReceive`.
 * See https://github.com/otter-sec/anchor/issues/3043.
 */
export function toCamelCase(input: string): string {
  if (!input) return input;

  const spaced = input
    .replace(/[^A-Za-z0-9]+/g, " ")
    .replace(/([a-z][\d]*)([A-Z])/g, "$1 $2")
    .replace(/([A-Z][A-Z\d]*)([A-Z][a-z])/g, "$1 $2")
    .trim();

  if (!spaced) return "";

  return spaced
    .split(/\s+/)
    .map((word, i) =>
      i === 0
        ? word.toLowerCase()
        : word.charAt(0).toUpperCase() + word.slice(1).toLowerCase()
    )
    .join("");
}
