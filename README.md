<div align="center">
  <img height="170x" src="https://pbs.twimg.com/media/FVUVaO9XEAAulvK?format=png&name=small" />

  <h1>Anchor</h1>

  <p>
    <strong>Solana Program Framework</strong>
  </p>

  <p>
    <a href="https://github.com/otter-sec/anchor/actions"><img alt="Build Status" src="https://github.com/otter-sec/anchor/actions/workflows/tests.yaml/badge.svg" /></a>
    <a href="https://anchor-lang.com"><img alt="Tutorials" src="https://img.shields.io/badge/docs-tutorials-blueviolet" /></a>
    <a href="https://discord.gg/NHHGSXAnXk"><img alt="Discord Chat" src="https://img.shields.io/discord/889577356681945098?color=blueviolet" /></a>
    <a href="https://opensource.org/licenses/Apache-2.0"><img alt="License" src="https://img.shields.io/github/license/otter-sec/anchor?color=blueviolet" /></a>
  </p>
</div>

[Anchor](https://www.anchor-lang.com/) is a framework for Solana programs: a Rust eDSL, an [IDL](https://en.wikipedia.org/wiki/Interface_description_language) spec, a TypeScript client generated from that IDL, and a CLI + workspace tool for driving the whole loop.

## v2 in progress

[`anchor-lang-v2`](./lang-v2/) is the next-generation runtime, built on [pinocchio](https://github.com/anza-xyz/pinocchio) and `#![no_std]` by default. It produces an order of magnitude smaller binaries and fewer CU per instruction than v1. Alpha — see [`lang-v2/README.md`](./lang-v2/README.md) for quick-start, bench numbers, and caveats. The v1 (`lang/`) code in this repo remains the stable, published path.

## Getting started

See the [Anchor book](https://book.anchor-lang.com), the [docs site](https://anchor-lang.com), and the [examples](https://github.com/otter-sec/anchor/tree/master/examples). Rust API on [docs.rs](https://docs.rs/anchor-lang), TypeScript API in the [typedoc](https://www.anchor-lang.com/docs/clients/typescript).

## Packages

| Package                 | Description                                              | Version                                                                                                                          | Docs                                                                                                            |
| :---------------------- | :------------------------------------------------------- | :------------------------------------------------------------------------------------------------------------------------------- | :-------------------------------------------------------------------------------------------------------------- |
| `anchor-lang`           | Rust primitives for writing programs on Solana           | [![Crates.io](https://img.shields.io/crates/v/anchor-lang?color=blue)](https://crates.io/crates/anchor-lang)                     | [![Docs.rs](https://docs.rs/anchor-lang/badge.svg)](https://docs.rs/anchor-lang)                                |
| `anchor-spl`            | CPI clients for SPL programs on Solana                   | [![crates](https://img.shields.io/crates/v/anchor-spl?color=blue)](https://crates.io/crates/anchor-spl)                          | [![Docs.rs](https://docs.rs/anchor-spl/badge.svg)](https://docs.rs/anchor-spl)                                  |
| `anchor-client`         | Rust client for Anchor programs                          | [![crates](https://img.shields.io/crates/v/anchor-client?color=blue)](https://crates.io/crates/anchor-client)                    | [![Docs.rs](https://docs.rs/anchor-client/badge.svg)](https://docs.rs/anchor-client)                            |
| `@anchor-lang/core`     | TypeScript client for Anchor programs                    | [![npm](https://img.shields.io/npm/v/@anchor-lang/core.svg?color=blue)](https://www.npmjs.com/package/@anchor-lang/core)         | [![Docs](https://img.shields.io/badge/docs-typedoc-blue)](https://otter-sec.github.io/anchor/ts/index.html)     |
| `@anchor-lang/cli`      | CLI for building and managing an Anchor workspace        | [![npm](https://img.shields.io/npm/v/@anchor-lang/cli.svg?color=blue)](https://www.npmjs.com/package/@anchor-lang/cli)           | [![Docs](https://img.shields.io/badge/docs-typedoc-blue)](https://www.anchor-lang.com/docs/references/cli)      |

## Notes

- **APIs are subject to change** — Anchor is under active development.

## License

Anchor is licensed under [Apache 2.0](./LICENSE). Contributions are accepted under the same license unless you explicitly state otherwise.

See [CONTRIBUTING.md](./CONTRIBUTING.md) for contribution guidelines.

<div align="center">
  <a href="https://github.com/otter-sec/anchor/graphs/contributors">
    <img src="https://contrib.rocks/image?repo=otter-sec/anchor" width="100%" />
  </a>
</div>
