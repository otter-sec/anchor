# anchor-lang

v2 is a drop-in speedup for Anchor v1, but up to **94% smaller** and **3× faster or more** per instruction in the example programs below.

**Compatibility is a priority.** Most v1 programs port with the renames in [Migrating from v1](#migrating-from-v1). A `compat` feature restores v1-shaped helpers to ease migrations for larger programs. v2 also comes with first-class tooling like `anchor debugger` to help give you full visibility into your program.

**Built for extensibility.** As active contributors and auditors of Anchor v1, we've felt the struggles of working inside a large macro-based framework — v2 is our answer. The core derive is around 3× smaller, moving most of the logic from the macro to behind traits. This makes it much easier to audit and maintain, and more excitingly also lets you write your own Anchor extensions (see [Extensibility](#extensibility))!

**And most importantly, v2 is secure by default for users.** Safer APIs steer you away from common v1 footguns, promoting whole classes of runtime bugs into compile errors. [Fuzzing](https://github.com/asymmetric-research/crucible-harnesses), static analysis, and formal verification are builtin with first-class support.

> [!WARNING]
> **Alpha.** Not audited, not on crates.io. APIs may break between commits. Depend via git on the [`anchor-next`](https://github.com/otter-sec/anchor/tree/anchor-next) branch.

## Examples

Worked programs live under [`bench/programs/`](../bench/programs/), paired with v1 for a head-to-head comparison. Binary sizes and CU numbers are from the v2 variant.

| Program | Description | Binary | CU range | Bin ↓ | CU ↓ |
|---|---|---|---|---|---|
| [helloworld](../bench/programs/helloworld/anchor-v2) | Single-instruction counter | 6.9 KB | 1,383 | 18× | 4.2× |
| [prop-amm](../bench/programs/prop-amm/anchor-v2) | Oracle feed with asm fast-path | 9.2 KB | 26–1,383 | 15× | 3.1–50× |
| [vault](../bench/programs/vault/anchor-v2) | Single-depositor SOL vault | 5.9 KB | 403–1,910 | 18× | 3.0–6.1× |
| [nested](../bench/programs/nested/anchor-v2) | Shared-validation via `Nested<T>` | 13 KB | 476–2,748 | 12× | 7.2–10× |
| [multisig](../bench/programs/multisig/anchor-v2) | Four-instruction SOL multisig | 31 KB | 477–2,363 | 5.3× | 3.0–9.1× |

## Getting started

```bash
$ cargo install --git https://github.com/otter-sec/anchor.git --branch anchor-next anchor-cli --force
$ anchor init --no-install counter && cd counter
$ anchor build && anchor test
$ anchor debugger                    # optional: step through the SBF trace in a TUI
```

> [!WARNING]
> `cargo install` overwrites the `anchor` binary on your `PATH`, including one from [`avm`](https://www.anchor-lang.com/docs/installation#anchor-version-manager-avm). To keep v1 alongside, run the debug binary directly from a source checkout.

> [!NOTE]
> On macOS you may hit `ld: could not parse bitcode object file … Unknown attribute kind` during the final link. Turn LTO off for the install:
>
> ```bash
> $ CARGO_PROFILE_RELEASE_LTO=off cargo install --git https://github.com/otter-sec/anchor.git --branch anchor-next anchor-cli --force
> ```

### Building from source

Same codebase, but built from a local checkout — useful if you want to tweak v2 internals or try `anchor debugger` on one of the bench programs without leaving the repo:

```bash
$ git clone https://github.com/otter-sec/anchor.git && cd anchor && git checkout anchor-next
$ cargo install --path cli --force   # prepend `CARGO_PROFILE_RELEASE_LTO=off` on macOS
$ cd bench/programs/prop-amm/anchor-v2   # or: vault, multisig, nested, helloworld
$ anchor debugger
```

## Migrating from v1

Most v1 programs port with the below.

| v1 | v2 |
|---|---|
| `solana_program` | `pinocchio` |
| `std` required | `#![no_std]` (`alloc` is a default feature) |
| `<'info>` everywhere | removed — pinocchio's account model is static-scoped |
| `Context<T>` in handlers | `&mut Context<T>` (constraints + exit hooks mutate context state) |
| `Pubkey` | `Address` (drop-in replacement — same 32-byte type) |
| `Account<T>` | `BorshAccount<T>` |

### `compat` feature

To help improve the migration for users, we've added an additional `compat` flag to more easily bridge the gap between v1 and v2 programs.

```toml
[dependencies]
anchor-lang = { git = "...", branch = "anchor-next", features = ["compat"] }
```

## Optimizations

Here are some examples of optimizations present in Anchor v2.

- **PDA bumps precomputed at macro time.** If your seeds are all literals, the derive runs the PDA search during compilation and bakes the canonical bump in as a `const`. This lets us skip the runtime PDA search entirely.
- **Skip the on-curve check for program-owned PDAs.** If the program already owns the account, it had to be created via signed CPI — which did the curve check at the time. Verification can just hash-and-compare. Saves ~1,000 CU per verify.
- **Wincode events by default.** `#[event]` automatically derives Anchor's Wincode-backed serialization, so programs do not need a direct Wincode dependency. It still handles `Vec` / `String` / `Option` / enums and is 3–10× cheaper than borsh on SBF.
- **`#[event(bytemuck)]` for fixed-size events.** The struct's `repr(C)` Pod layout already matches the wire format, so emitting is just disc + one memcpy of the body. No per-field encoding, with compile-time rejection of padded layouts.
- **Alignment-1 Pod wrappers** (`PodU64`, `PodI128`, `PodBool`, ...). Integers stored as `[u8; N]` so the whole `#[account]` struct casts directly from the account's raw bytes. Zero deserialization.
- **`PodVec<T, MAX>`**: fixed-capacity vec with a `u16` length, stored inline in the account. Variable-length data without heap allocation.
- **Typed `CpiHandle` lets us use pinocchio's unchecked CPI.** The unchecked path would be UB under stale-borrow aliasing, but the Rust borrow checker rules that out at compile time — so `CpiContext::invoke()` takes it. Turns UB into a compile error and drops one runtime check per CPI.
- **`const-rent` feature** bakes the rent formula into the binary so `create_account` skips the `Rent::get()` sysvar. Saves ~85 CU per `create_account`.
- **Guardrails compile away** when you drop the feature. `check_program_id` / the `is_writable` check in `load_mut` just aren't emitted. Smaller binary in prod.
- **`remaining_accounts()` is lazy-cached.** First call walks the account cursor; subsequent calls return a clone of the cached vec. Zero overhead for instructions that don't touch it.

### Isolating a large instruction stack frame

Anchor inlines instruction wrappers by default to minimize compute units and
binary size. A program with several large or deeply nested `Accounts` structs
can instead give selected instructions their own SBF stack frames:

```rust
#[program]
pub mod my_program {
    use super::*;

    #[inline(never)]
    pub fn large_instruction(ctx: &mut Context<LargeAccounts>) -> Result<()> {
        // ...
        Ok(())
    }
}
```

Anchor mirrors the handler's inline policy onto its generated wrapper. Without
an explicit policy, the wrapper keeps its existing `#[inline(always)]` default.
Use `#[inline(never)]` only where the SBF compiler reports an oversized
dispatcher frame. It also applies to the user handler, so the additional call
boundaries can cost compute units and increase binary size.

The policy can be conditional without changing the default in other builds:

```rust
#[cfg_attr(feature = "isolate-large-handler", inline(never))]
pub fn large_instruction(ctx: &mut Context<LargeAccounts>) -> Result<()> {
    // ...
    Ok(())
}
```

## Account types

**Why `Account<T>` is zero-copy by default.** Stored bytes match the struct layout, so load is a pointer cast and exit is a no-op — no (de)serialization and no heap.

None of these carry an `'info` lifetime — pinocchio's account model is static-scoped, so you can pass them around without the v1 lifetime gymnastics.

| Type | Usage |
|---|---|
| `Account<T>` | **Default for your program's data.** Zero-copy, requires `T: Pod` (enforced by `#[account]`). Layout: `[8-byte disc][repr(C) T]`. |
| `BorshAccount<T>` | Data with `Vec` / `String` / enums. Deserializes on load, serializes on exit. |
| `Slab<H, Item>` | Header + dynamic item tail. Zero-copy ledger / event-log accounts. `Account<T>` is `Slab<T, HeaderOnly>` under the hood. |
| `Option<Account<T>>` | Optional account slot. Client sends program-ID as sentinel when absent; bumps become `Option<u8>`. |
| `Nested<T>` | Compose `#[derive(Accounts)]` structs. Inline expansion, access via `ctx.accounts.inner.field`. |
| `Signer` | Transaction signer. Validates `is_signer`. (v1 compat) |
| `Program<T>` | CPI targets (`Program<System>`, `Program<Token>`, …). Validates executable + program ID via `T: Id`. (v1 compat) |
| `SystemAccount` | System-owned account. Owner check only. (v1 compat) |
| `UncheckedAccount` | Escape hatch. No validation. No generic `close` support. (v1 compat) |
| `Sysvar<T>` | `Sysvar<Clock>`, `Sysvar<Rent>`. Prefer `Clock::get()` / `Rent::get()` syscalls where possible. (v1 compat) |

## CPI Semantics

Same `CpiContext` shape as v1. The big caller-side win is the generated **`Resolved`** struct: alongside the full accounts struct for each handler, the derive emits a variant with only the fields a caller actually has to provide. The standard system and token programs auto-fill when you build the instruction metas, and PDAs derive in topological order so dependent seeds still work.

For example, the [multisig bench's `Create`](../bench/programs/multisig/anchor-v2/src/instructions/create.rs) takes `creator` (signer), `config` (PDA from `[b"multisig", creator]`), and `system_program`. The caller only passes `creator`:

```rust
// multisig_v2::accounts::Create { creator, config, system_program }   // full — all three
// multisig_v2::accounts::CreateResolved { creator }                    // resolved — just the input

let metas = multisig_v2::accounts::CreateResolved { creator: creator.pubkey() }
    .to_account_metas(None);   // auto-derives `config` PDA, auto-fills `system_program`
```

In v1, the caller built the `AccountMeta` vector by hand on every call — deriving the PDA, wiring up `system_program`, and keeping the order in sync with the handler's `#[derive(Accounts)]`.

## Extensibility

An important implication of our trait-based framework is: **you can write your own Anchor extensions.**

In v1, anything the macro didn't already support meant forking the derive. v2 moves much of the account behavior behind traits, so anyone can ship new behavior from a separate crate — no fork, no upstream PR.

For example, [anchor-dynamic-account](https://github.com/chen-robert/anchor-dynamic-account) adds a brand-new primitive — zero-copy accounts with a `Vec<T>` / `String` tail that auto-reallocates to fit — behind a single `#[wrapped_account]` macro, with no changes to `anchor-lang`:

```rust
#[wrapped_account]
pub struct Post {
    pub author: Address,
    pub body:   Vec<u8>,     // tail, auto-reallocs to fit
}

#[derive(Accounts)]
pub struct Edit {
    #[account(mut)]
    pub author: Signer,
    #[account(mut, dynamic_account::payer = author)]
    pub post: DynamicAccount<Post>,
}
```

At the call site, `DynamicAccount<T>` is a cosmetic alias that reads parallel to v2's `Account<T>` / `BorshAccount<T>`. Under the hood, the macro plugs it into v2 by implementing `AnchorAccount`:

```rust
impl AnchorAccount for DynamicAccount<Post> {
    type Data = PostFixed;
    fn load(view, pid)   -> Result<Self>     { /* parse disc + tail */ }
    fn exit(&mut self)   -> ProgramResult    { /* realloc to fit, persist */ }
    // load_mut, load_mut_after_init, account
}
```

## Contributing

File issues at [otter-sec/anchor](https://github.com/otter-sec/anchor/issues), tagged with `v2` where applicable. Working branch: `anchor-next`. See the top-level [CONTRIBUTING.md](../CONTRIBUTING.md).
