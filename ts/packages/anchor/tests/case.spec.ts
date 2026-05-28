import { toCamelCase } from "../src/utils/case";
import { convertIdlToCamelCase, Idl } from "../src/idl";

// `structuredClone` ships natively in Node 17+, but jest 27's node test
// environment doesn't expose it on `globalThis`. Anchor's runtime targets
// Node >= 17, so polyfilling here just unblocks the test harness — the
// production code path uses the real built-in.
if (typeof (globalThis as any).structuredClone !== "function") {
  (globalThis as any).structuredClone = (v: unknown) =>
    JSON.parse(JSON.stringify(v));
}

// The npm `camelcase` library treats a digit followed by a letter as a word
// boundary (e.g. `a1bReceive` -> `a1BReceive`), but the Rust-side IDL
// generator uses `heck::to_lower_camel_case`, which does not split on
// digit-letter transitions. The mismatch caused TS-typed lookups for names
// like `a1bReceive` / `myA1bParam` to miss their runtime entries — which
// in the instruction-arg case silently defaulted to the zero pubkey.
// `toCamelCase` matches the Rust-side semantics.
describe("toCamelCase", () => {
  test("converts snake_case to camelCase", () => {
    expect(toCamelCase("foo_bar")).toBe("fooBar");
    expect(toCamelCase("set_pass_threshold_bps")).toBe("setPassThresholdBps");
  });

  test("is idempotent on camelCase input", () => {
    expect(toCamelCase("fooBar")).toBe("fooBar");
    expect(toCamelCase("setPassThresholdBps")).toBe("setPassThresholdBps");
  });

  test("does not split on digit-letter transitions (#3043)", () => {
    expect(toCamelCase("a1b_receive")).toBe("a1bReceive");
    expect(toCamelCase("a1bReceive")).toBe("a1bReceive");
    expect(toCamelCase("my_a1b_param")).toBe("myA1bParam");
    expect(toCamelCase("myA1bParam")).toBe("myA1bParam");
  });

  test("digit-then-uppercase only splits when preceded by lowercase", () => {
    expect(toCamelCase("A1B")).toBe("a1b"); // not "a1B"
    expect(toCamelCase("XML1B")).toBe("xml1b"); // not "xml1B"
    expect(toCamelCase("A1b")).toBe("a1b");
    // Sanity: the lowercase-preceded case still splits.
    expect(toCamelCase("a1B")).toBe("a1B");
  });

  test("acronym + digit + uppercase + lowercase still splits", () => {
    expect(toCamelCase("XML1Http")).toBe("xml1Http"); // not "xml1http"
    expect(toCamelCase("X1Foo")).toBe("x1Foo"); // not "x1foo"
  });

  test("handles leading and trailing digits", () => {
    expect(toCamelCase("foo123")).toBe("foo123");
    expect(toCamelCase("foo_123")).toBe("foo123");
    expect(toCamelCase("123_foo")).toBe("123Foo");
  });

  test("handles acronyms as a single word", () => {
    // `ABCFoo` -> [`ABC`, `Foo`] -> `abcFoo`
    expect(toCamelCase("ABCFoo")).toBe("abcFoo");
    expect(toCamelCase("ABC_foo")).toBe("abcFoo");
  });

  test("accepts hyphen / space / dot separators", () => {
    expect(toCamelCase("foo-bar")).toBe("fooBar");
    expect(toCamelCase("foo bar")).toBe("fooBar");
    expect(toCamelCase("foo.bar")).toBe("fooBar");
  });

  test("handles single word and empty input", () => {
    expect(toCamelCase("initialize")).toBe("initialize");
    expect(toCamelCase("")).toBe("");
  });
});

describe("convertIdlToCamelCase", () => {
  // Regression guard: an earlier iteration of #3043's fix introduced a
  // local helper named `toCamelCase` inside `convertIdlToCamelCase` that
  // shadowed the imported `toCamelCase`, turning its self-reference into
  // infinite recursion. Anchor's full CI matrix (tests/sysvars, escrow,
  // misc, ...) blew the call stack with `at toCamelCase (idl.js:210)`
  // repeated. The helper is now `toCamelCasePath` — this test exercises
  // every code path that fans into it and asserts it terminates.
  test("camelCases dot-separated paths without recursing forever", () => {
    const idl: Idl = {
      address: "Test111111111111111111111111111111111111111",
      metadata: { name: "test", version: "0.0.0", spec: "0.1.0" },
      instructions: [
        {
          name: "do_thing",
          discriminator: [0, 0, 0, 0, 0, 0, 0, 0],
          args: [],
          accounts: [
            {
              name: "my_pda",
              pda: {
                seeds: [{ kind: "account", path: "my_account.field" } as any],
              },
              relations: ["other_account", "another_one"],
            } as any,
          ],
        },
      ],
    };

    const out = convertIdlToCamelCase(idl);

    expect(out.instructions[0].name).toBe("doThing");
    const acct = out.instructions[0].accounts[0] as any;
    expect(acct.name).toBe("myPda");
    // The split-on-`.` wrapper must camelCase each segment in isolation.
    expect(acct.pda.seeds[0].path).toBe("myAccount.field");
    expect(acct.relations).toEqual(["otherAccount", "anotherOne"]);
  });
});
