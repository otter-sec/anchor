# Anchor v2 on upstream BPF (`bpfel-unknown-none` + `sbpf-linker`)

Investigation of building Anchor v2 (rc) programs with the **upstream** rustc
BPF target and [`sbpf-linker`](https://github.com/blueshift-gg/sbpf-linker)
instead of `cargo build-sbf` / platform-tools, following
[solana-upstream-bpf-template](https://github.com/blueshift-gg/solana-upstream-bpf-template).

**TL;DR — yes, it works.** With three small `lang-v2` changes (dependency
swaps to no_std-clean crates + a target-gated panic handler), core Anchor v2
programs (accounts, constraints, PDAs, init/close/realloc, CPI, events, return
data, `declare_program!`) build to sBPF **v3** ELFs and pass the existing
`tests-v2` LiteSVM suites (300/302; the 2 failures are a log-string assertion,
see below). `anchor-spl` (spl-v2) does **not** build yet.

## Layout

| Path | What |
| --- | --- |
| `hello-world/` | Standalone crate: minimal `#[program]` + LiteSVM test |
| `stress.sh` | Builds every `tests-v2/programs/*` with upstream BPF in a scratch workspace; `RUN_TESTS=1` then runs the `tests-v2` suites against those binaries |
| `../src/lib.rs` (`build_program`) | New opt-in `ANCHOR_V2_PREBUILT_SO_DIR`: copy prebuilt `.so`s instead of running `cargo build-sbf` |

## Setup

```sh
rustup toolchain install nightly-2026-10-08 --profile minimal -c rust-src
cargo install sbpf-linker          # or: cargo binstall sbpf-linker
```

`sbpf-linker` 0.2.3 built from source uses the `rust-llvm` feature (dlopens
rustc's `libLLVM`), so it needs a nightly ≥ 2026-08-05 (LLVM 23). It prints a
harmless `unable to open LLVM shared lib .../libLLVM-23-rust-<ver>.so: dlopen
failed` linker warning: `aya-rustc-llvm-proxy` dlopens every `libLLVM*` in
`LD_LIBRARY_PATH` / the toolchain `lib/` until one loads, and that file is a
44-byte GNU ld linker script (`INPUT(libLLVM.so.23.1-rust-<ver>)`), which
`dlopen` can't load. The next candidate is the real LLVM 23.1 that the script
points to, and it loads. If none loaded, the link would panic (`unable to find
LLVM shared lib`), so the warning can't mask a bad build. Caveat: a foreign
`libLLVM` earlier in `LD_LIBRARY_PATH` would be picked first. A
`cargo binstall` build links LLVM statically (untested here).

## Hello world

```sh
cd hello-world
cargo build-bpf     # -> target/deploy/hello_world.so (≈1.3 KiB, sBPF v3)
cargo test          # LiteSVM: logs "Instruction: hello", "Hello, world!" (220 CU)
```

`rust-toolchain.toml` pins the nightly. Unlike the template, `-Zbuild-std` is
in the `build-bpf` alias rather than `[unstable]`, otherwise `cargo test`
tries to rebuild `std` for the host and fails (template bug: its `cargo test`
fails on nightly with duplicate `alloc` lang items).

## `anchor init` projects

New projects are upstream-BPF ready out of the box:

- `anchor init` / `anchor new` write `.cargo/config.toml` (same contents as
  `hello-world/.cargo/config.toml`, plus usage notes). Everything in it is
  scoped to `--target bpfel-unknown-none` or the `build-bpf` alias, so
  `anchor build` / `cargo build-sbf` ignore it. Existing files are not
  overwritten.
- The generated `lib.rs` (single and multiple templates) starts with
  `#![cfg_attr(target_arch = "bpf", no_std)]` (no-op on SBF).
- `anchor-lang` links `solana-compiler-builtins` itself on
  `target_arch = "bpf"`, so programs need no extra dependency.

```sh
rustup toolchain install nightly-2026-10-08 --profile minimal -c rust-src
cargo install sbpf-linker
cargo +nightly-2026-10-08 build-bpf      # -> target/deploy/<program>.so
cargo test                               # template tests load target/deploy/*.so
```

`+nightly` is needed because the generated `rust-toolchain.toml` pins the
stable host toolchain (1.97.1) and cargo aliases can't select a toolchain;
the nightly is `UPSTREAM_BPF_NIGHTLY` in `cli/src/rust_template.rs`.

Verified with the local CLI (patched to the local crates): `multiple` +
`litesvm` and `single` + `mollusk` both build with `cargo +nightly build-bpf`
(5.8 KB sBPF v3) and pass their generated `initialize` test (system CPI +
rent) against the upstream binary; `anchor build` (SBF + IDL) and the same
tests still pass on the normal path.

## Testing: LiteSVM is supported

- LiteSVM **0.18** (now used everywhere: `tests-v2`, `anchor-v2-testing`,
  `bench`, this crate) enables `enable_sbpf_v3_deployment_and_execution` and
  passes the SIMD-0321 `r2` instruction-data pointer that the v2 entrypoint
  relies on. (0.13.1 did too, but pinned `solana-instruction = "=3.2.0"`;
  see roadblock 4.)
- Mollusk 0.16 also works (the vanilla template's test passes); not needed.

## Changes made to Anchor (`lang-v2`)

1. **`wincode` without `std`** — `default-features = false, features = ["alloc", "derive"]`.
   Default `std` pulled `thiserror/std`.
2. **no_std system-interface + rent** (backing the compat re-exports
   `solana_program::{system_instruction, rent}`; same on every target):
   - `solana-system-interface` 2.0 + `bincode` → **`"3.3"` + `wincode`**,
     `default-features = false`. 3.x is `#![no_std]` and its builders
     (`create_account`, `transfer`, …) use wincode. 3.3 needs
     `solana-instruction` ^3.5 (3.1 resolves against 3.2.0 but doesn't
     compile: `solana-instruction` 3.2.0 implements wincode 0.4 while
     `solana-address` 2.9 implements 0.6). That forces the tooling cascade
     below.
   - `solana-sysvar` 3.1 → **`solana-rent = { version = "4.3", features = ["sysvar"] }`**.
     `solana-sysvar` cannot be made no_std even at 5.0: no `#![no_std]`,
     non-optional `lazy_static` (std `Once`) and `solana-slot-history` → `bv`
     (std). Anchor only used its `rent` module, which re-exported
     `solana_rent::Rent`. `solana-rent` ≥ 4.3 is no_std and implements
     `solana_get_sysvar::GetSysvar` (`Rent::get()` via `sol_get_sysvar`); 4.2
     lacks that impl. `4.3` instead of `4.5` because litesvm 0.18 pins
     `~4.4.0`.
   - `solana_program::rent` now re-exports `solana_rent::{*, sysvar::*}`
     (`Rent`, `GetSysvar`, `ID`/`id`/`check_id`). The old
     `solana_sysvar::Sysvar` trait is gone; nothing in the repo used either
     re-export.
   - Verified on upstream BPF under LiteSVM: `<Rent as GetSysvar>::get()` and
     `system_instruction::transfer(..)` work (392 CU).
   - Wart: when `solana-rent/wincode` is enabled (litesvm turns it on in host
     builds), `Rent::get()` is ambiguous with `wincode::SchemaRead::get`
     because `anchor_lang::prelude::*` exports `SchemaRead`. Write
     `<Rent as GetSysvar>::get()`.
   - **Tooling cascade** (all off LiteSVM 0.13.1 / agave 4.0, whose
     `solana-instruction` cap is < 3.4):
     - `tests-v2`, `v2-testing` (`anchor-v2-testing`), `bench` + its v2
       programs: LiteSVM → 0.18, `solana-message`/`solana-transaction`/
       `solana-account` → 4; `v2-testing`'s `profile` agave pins → `=4.3.0`.
     - `anchor init` Mollusk template: `mollusk-svm` 0.13 → 0.16,
       `solana-account` 3 → 4 (0.13 is on agave 4.0 and no longer resolves).
     - LiteSVM 0.18 / Mollusk 0.16 → agave 4.3, which requires **rustc
       1.97.1**. On rustc 1.89 the only option would be agave 4.1.x (4.2
       caps `solana-instruction` < 3.5), which caret requirements don't select
       by default. So the CLI now pins the generated `rust-toolchain.toml` to
       `ANCHOR_HOST_TOOLCHAIN = "1.97.1"`. `rust-version` (`ANCHOR_MSRV`)
       stays `1.89.0`: `cargo build-sbf` checks it against platform-tools'
       rustc (1.89.0-dev in v1.52), and 1.97.1 there fails every SBF build.
3. **`#[program]` emits `nostd_panic_handler!()` on `target_arch = "bpf"`**
   instead of `default_panic_handler!()`. The latter only defines the
   `custom_panic` hook; the real `#[panic_handler]` comes from platform-tools'
   std stub, which doesn't exist upstream.
4. **`solana-compiler-builtins` on `target_arch = "bpf"`**: target-specific
   dependency + `use solana_compiler_builtins as _;` in `lib.rs`, so the
   SVM libcalls are linked without user code mentioning them.

Only changes 3–4 are target-gated. `cfg(target_arch = "bpf")` is the
discriminator: platform-tools is `target_arch = "sbf"`, upstream is `"bpf"`,
and both get `target_os = "solana"` (the template sets it via `--cfg`).

Verified on the normal (`cargo build-sbf`) path after the cascade:
- full `tests-v2` suite: all green except 7 `compile_fail` cases, which fail
  identically on a clean `HEAD`;
- `anchor-lang --features testing`, `anchor-cli` unit tests (the 2
  `debugger_symbol_resolution` failures are environmental: the local
  `cargo-build-sbf` writes `target/sbf-solana-solana/`, the test expects
  `sbpf-solana-solana/`);
- `bench` v2 programs and `v2-testing` (`--features profile` too) type-check
  (the `bench` root crate needs system `fontconfig`, not installed here);
- `anchor init --test-template {litesvm,mollusk}` (patched to the local
  crates): `cargo build-sbf` + `cargo test` pass on the pinned 1.97.1.

## Results of `stress.sh` (all 62 `tests-v2` packages)

Build: **38/62** packages build. All 24 failures depend on `anchor-spl`
(spl, spl-ata, token-interface, token-2022-extensions/*, constraint-values,
account-address-constraints, equivalence spl/metadata v2) or on Anchor v1
(equivalence `*/v1`); nothing else fails.
Running the `tests-v2` LiteSVM suites against the upstream binaries:

| suite | result | | suite | result |
| --- | --- | --- | --- | --- |
| accounts | 51/53 ⚠️ | | dup_mut | 28/28 |
| account_meta_signer_overrides | 3/3 | | event_cpi | 7/7 |
| borsh_realloc | 11/11 | | init_space_usability | 4/4 |
| client_builders | 10/10 | | ix_macro | 2/2 |
| constraints | 41/41 | | min_ix_data_len | 1/1 |
| custom_constraints | 17/17 | | optional_accounts | 14/14 |
| cpi | 9/9 | | pda_payer | 5/5 |
| declare_program | 58/58 | | seeds | 10/10 |
| derives | 12/12 | | space_annotation | 3/3 |
| dispatch_remaining | 14/14 | | | |

The two `accounts` failures (`manual_close_*_then_derive_close_fails_and_rolls_back`)
panic at exactly the right place (`Slab` mutability guard, `slab.rs:309`), but
the test greps the logs for `"Program failed to complete"` / `"panic"` / the
panic message. With `nostd_panic_handler!` the panic is reported via
`sol_panic_` (`SBF program Panicked in …/slab.rs at 309:13`) and the
formatted message is not logged. Behavioural parity; assertion is brittle.

Binary size (same programs, SBF platform-tools v1.52 vs upstream):

| program | sbf | upstream |
| --- | ---: | ---: |
| accounts_test | 213,464 | 196,672 |
| caller | 93,968 | 38,704 |
| callee | 19,000 | 18,448 |
| event_cpi_test | 33,152 | 17,272 |
| seeds | 35,120 | 40,528 |

## Syscalls

No missing syscalls hit. Syscalls are resolved statically by murmur3 hash
(`--cfg target_feature="static-syscalls"`); every `solana-define-syscall`
version in the graph (2.3, 3.0, 4.0, 5.2) honours that cfg, and so does
pinocchio. The passing suites cover logging (`msg!`, `emit!` → `sol_log_data`),
CPI (`sol_invoke_signed_c`, incl. signed PDA CPIs), PDA derivation/verification,
`sha256`, sysvar reads (rent via `init`, clock), `sol_memset_` (close/realloc
zeroing), return data (`declare_program` returns), `sol_panic_` and the bump
heap allocator. Any crate that declares syscalls with a raw `extern "C"`
instead of `solana-define-syscall` would break (none found in the graph).

## Roadblocks

### Anchor / ecosystem

1. **No `std` on `bpfel-unknown-none`.** Platform-tools ships a std stub, so
   the Solana crate ecosystem freely enables `std` on `target_os = "solana"`.
   Upstream only has `core` + `alloc`, so *every* transitive dep must be
   `no_std`. Fixed for `anchor-lang` (changes 1–2 above).
2. **User programs must be `no_std`.** Each program crate needs
   `#![cfg_attr(target_arch = "bpf", no_std)]` (a proc-macro can't add
   crate-level attributes) plus explicit `alloc` imports for `Box`, `Vec`,
   `String`, `format!`, `vec!` — none of the 62 `tests-v2` programs currently
   are. `stress.sh` injects a shim. New `anchor init` projects get the
   attribute; an `anchor_lang::prelude` re-exporting the alloc prelude items
   would cover the rest.
3. **`anchor-spl` (spl-v2) does not build.** It depends on
   `spl-token-2022-interface`, `spl-token-metadata-interface`,
   `spl-token-group-interface`, `spl-pod`, `spl-token-interface` and `borsh`,
   which pull `std` via `num_enum`, `borsh/std`, `base64`, `serde`,
   `thiserror`. Usage is concentrated (Token-2022 extension TLV parsing in
   `extensions.rs`/`token_interface.rs`, metadata/group instruction builders,
   `OptionalNonZeroPubkey`); porting means replacing these with
   `pinocchio-token(-2022)` + hand-rolled layouts, or upstreaming `no_std`
   support to the SPL interface crates. Affects ~20 `tests-v2` programs
   (spl, spl-ata, token-interface, token-2022-extensions/*, equivalence spl).
4. **`solana-instruction` 3.2.0 can't build `no_std` on `target_os = "solana"`**
   (`syscalls.rs` is compiled for `target_os = "solana"` and imports the
   std-gated `AccountMeta`). 3.5.x (a re-export of v4) is fine. **Resolved**:
   `lang-v2` now effectively requires ≥ 3.5 (via `solana-system-interface`
   3.3) and the workspace moved off LiteSVM 0.13.1's `=3.2.0` pin, at the cost
   of the rustc 1.97.1 host toolchain (see change 2). Users still on LiteSVM
   ≤ 0.16 / Mollusk ≤ 0.15 can't resolve against the new `anchor-lang`.
5. **Program crates used as CPI deps (`cdylib` + `lib`).** Cargo builds the
   `cdylib` of a dependency too; with `no-entrypoint` there is no allocator or
   panic handler, and without a std stub that link fails (`no global memory
   allocator found` / `#[panic_handler] function required`). Emitting them in
   `no-entrypoint` mode would clash with the parent program's. Workaround used:
   `crate-type = ["lib"]` + `cargo rustc --crate-type cdylib`. Anchor could
   adopt that in its build command, or users keep CPI interface crates
   separate from the deployable crate.
6. **Panic reporting differs** (`sol_panic_` location only, no message) — see
   the `accounts` failures above.
7. **Nightly-only.** Needs `-Zbuild-std=core,alloc` and a recent nightly
   (LLVM 23) matching the `sbpf-linker` build; `explicit_builtin_cfgs_in_flags`
   must be allowed because the template sets `target_os`/`target_feature` via
   `--cfg`. That also means `target_os` is *both* `"none"` and `"solana"`.
8. **CLI integration.** `anchor init` now scaffolds the config (see above), but
   `anchor build` still drives `cargo build-sbf`; a `--upstream` mode would run
   `cargo +nightly build-bpf`, handle the `cargo rustc --crate-type cdylib`
   case for CPI deps (5), and keep the `target/deploy/*.so` layout (which
   sbpf-linker already writes, relative to the CWD). IDL generation (`cargo test --features idl-build`, host)
   is unaffected.

### `sbpf-linker`

No blocking issues for Anchor v2 — every program above linked, no
`StackArgOverlap`, missing-export or relocation errors (the 2 KiB
`[AccountView; 256]` dispatch frame fits the 4 KiB frame). Observations /
wishlist:

- **`--deploy` writes to `./target/deploy` relative to the process CWD**, not
  cargo's target dir. In a workspace built from the root it lands in
  `<root>/target/deploy`; building from a member dir or with a custom
  `CARGO_TARGET_DIR` scatters outputs. Deriving it from `-o` (…/`<target>`/
  `<triple>`/release → `<target>/deploy`) would be more robust.
- **Only `.text*`, `.rodata*`, `.data.rel.ro*` are handled.** Code that
  references writable `.data`/`.bss` (mutable statics) fails with
  `relocation target is not rodata or text`. Same restriction as SBF in
  practice, and Anchor v2 emits none, but the error could name the offending
  symbol/section kind more clearly.
- **Output is sBPF v3 only by default** (`--arch` is hidden; v0 requires
  `-C target-cpu` ≤ v2). Fine for LiteSVM ≥ 0.13 / Mollusk 0.16 / current
  Agave feature sets with v3 enabled; deploying to a cluster without
  `enable_sbpf_v3_deployment_and_execution` needs v0.
- **Section headers / symbols are stripped**, so `readelf`/`objdump`, the
  `register-tracing` debugger flow and `anchor` coverage tooling can't
  symbolise the binary. An option to keep symbols (or emit a separate debug
  ELF) would help tooling parity with platform-tools.
- `solana-compiler-builtins` is located heuristically from the inputs/build
  dir; programs must still `use solana_compiler_builtins as _;` (or add the
  dep) or the linker warns and libcalls may fail.
- The `dlopen failed` warning on every link (see Setup) is noise: the proxy
  should skip non-ELF files / linker scripts, or only report failures when no
  candidate loads. Silencing it with `-A linker_messages` would also hide real
  linker diagnostics.
