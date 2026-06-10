<div align="center">

  #  <picture><source media="(prefers-color-scheme: dark)" srcset="src/assets/wordmark-dark.svg" /><img alt="Anchor" src="src/assets/wordmark-light.svg" height="60" /></picture>

[![Build Status][Build Status Badge]][Build Status]
[![Tutorials][Tutorials Badge]][Documentation]
[![Discord Chat][Discord Badge]][Discord]
[![License][License Badge]][Apache 2.0 Spec]
</div>

---

Anchor is a framework for writing Solana programs in Rust. Solana's native program model gives you fine-grained control over accounts and instruction bytes, but very little structure. Anchor handles the parts that most programs need to do the same way. It validates accounts, checks ownership, serializes data, and dispatches instructions into your handler functions. You write what is specific to your program and let Anchor handle the rest.

The project ships four pieces that work together:

- `anchor-lang`, the Rust crate you use to write programs.
- An [IDL][IDL] format that describes a program's surface once and generates clients from it.
- `@anchor-lang/core`, a TypeScript package for calling deployed programs from a browser or a script.
- The `anchor` CLI that scaffolds workspaces, builds, runs tests, deploys to a cluster, and manages on-chain IDLs.

> [!IMPORTANT]
> [`anchor-lang-v2`][Anchor Lang v2] is the next-generation runtime. It is built on [pinocchio][Pinocchio] and `#![no_std]` by default, and produces orders-of-magnitude smaller binaries and fewer CU per instruction than v1. See the [v2 docs][Anchor Lang v2 Docs] for the quick-start, bench numbers, and caveats. Note that the v2 path is alpha.

## Getting started

- Read the [documentation][Documentation] for a guided tutorial and full reference.
- Explore the [examples][Examples] and [tests][Tests] directories for runnable code.
- Look up Rust types on [docs.rs][Anchor Lang Docs] and TypeScript types in [TypeDoc][TypeDoc].

## Packages

| Package | Description | Version | Docs |
| :--- | :--- | :--- | :--- |
| `anchor-lang` | Rust primitives for writing programs on Solana | [![Crates.io][Anchor Lang Crates Badge]][Anchor Lang Crates] | [![Docs.rs][Anchor Lang Docs Badge]][Anchor Lang Docs] |
| `anchor-spl` | CPI clients for SPL programs on Solana | [![crates][Anchor SPL Crates Badge]][Anchor SPL Crates] | [![Docs.rs][Anchor SPL Docs Badge]][Anchor SPL Docs] |
| `anchor-client` | Rust client for Anchor programs | [![crates][Anchor Client Crates Badge]][Anchor Client Crates] | [![Docs.rs][Anchor Client Docs Badge]][Anchor Client Docs] |
| `@anchor-lang/core` | TypeScript client for Anchor programs | [![npm][Anchor Core NPM Badge]][Anchor Core NPM] | [![Docs][Anchor Core Docs Badge]][Anchor Core Docs] |
| `@anchor-lang/cli` | CLI for building and managing an Anchor workspace | [![npm][Anchor CLI NPM Badge]][Anchor CLI NPM] | [![Docs][Anchor CLI Docs Badge]][Anchor CLI Docs] |

## Fuzzing

`anchor fuzz` integrates [Crucible](https://github.com/asymmetric-research/crucible) for coverage-guided program fuzzing. See the [fuzzing docs](https://anchor-lang.com/docs/testing/fuzzing) or the [Crucible docs](https://github.com/asymmetric-research/crucible#quick-start).

```sh
# scaffold a fuzz harness
anchor fuzz init program_name

# run a fuzz test
anchor fuzz run program_name test_name --release
```

## License

Anchor is licensed under [Apache 2.0][License]. Contributions are accepted under the same license unless you explicitly state otherwise. See [CONTRIBUTING.md][Contributing] for guidelines.

&nbsp;

<div align="center">
  <a href="https://github.com/solana-foundation/anchor/graphs/contributors">
    <img src="https://contrib.rocks/image?repo=solana-foundation/anchor" width="100%" />
  </a>

</div>

[Build Status]: https://github.com/solana-foundation/anchor/actions
[Build Status Badge]: https://img.shields.io/github/actions/workflow/status/solana-foundation/anchor/tests.yaml?color=6c7086&label=build
[Documentation]: https://anchor-lang.com
[Tutorials Badge]: https://img.shields.io/badge/docs-tutorials-7f849c
[Discord]: https://discord.gg/NHHGSXAnXk
[Discord Badge]: https://img.shields.io/discord/889577356681945098?color=9399b2&label=discord
[Apache 2.0 Spec]: https://opensource.org/licenses/Apache-2.0
[License Badge]: https://img.shields.io/github/license/solana-foundation/anchor?color=a6adc8
[IDL]: https://en.wikipedia.org/wiki/Interface_description_language
[Anchor Lang v2]: https://github.com/otter-sec/anchor/tree/anchor-next/lang-v2
[Pinocchio]: https://github.com/anza-xyz/pinocchio
[Anchor Lang v2 Docs]: https://www.anchor-lang.com/docs/v2/
[Examples]: ../examples/
[Tests]: ../tests/
[Anchor Lang Docs]: https://docs.rs/anchor-lang
[TypeDoc]: https://www.anchor-lang.com/docs/clients/typescript
[Anchor Lang Crates]: https://crates.io/crates/anchor-lang
[Anchor Lang Crates Badge]: https://img.shields.io/crates/v/anchor-lang?color=9399b2
[Anchor Lang Docs Badge]: https://img.shields.io/docsrs/anchor-lang?color=a6adc8&label=docs
[Anchor SPL Crates]: https://crates.io/crates/anchor-spl
[Anchor SPL Crates Badge]: https://img.shields.io/crates/v/anchor-spl?color=9399b2
[Anchor SPL Docs]: https://docs.rs/anchor-spl
[Anchor SPL Docs Badge]: https://img.shields.io/docsrs/anchor-spl?color=a6adc8&label=docs
[Anchor Client Crates]: https://crates.io/crates/anchor-client
[Anchor Client Crates Badge]: https://img.shields.io/crates/v/anchor-client?color=9399b2
[Anchor Client Docs]: https://docs.rs/anchor-client
[Anchor Client Docs Badge]: https://img.shields.io/docsrs/anchor-client?color=a6adc8&label=docs
[Anchor Core NPM]: https://www.npmjs.com/package/@anchor-lang/core
[Anchor Core NPM Badge]: https://img.shields.io/npm/v/@anchor-lang/core.svg?color=9399b2
[Anchor Core Docs]: https://solana-foundation.github.io/anchor/ts/index.html
[Anchor Core Docs Badge]: https://img.shields.io/badge/docs-typedoc-a6adc8
[Anchor CLI NPM]: https://www.npmjs.com/package/@anchor-lang/cli
[Anchor CLI NPM Badge]: https://img.shields.io/npm/v/@anchor-lang/cli.svg?color=9399b2
[Anchor CLI Docs]: https://www.anchor-lang.com/docs/references/cli
[Anchor CLI Docs Badge]: https://img.shields.io/badge/docs-cli-a6adc8
[License]: ../LICENSE
[Contributing]: ../CONTRIBUTING.md
