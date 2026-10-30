# @anchor-lang/cli

[![npm](https://img.shields.io/npm/v/@anchor-lang/cli.svg?color=blue)](https://www.npmjs.com/package/@anchor-lang/cli)
[![Docs](https://img.shields.io/badge/docs-anchor--cli-blue)](https://www.anchor-lang.com/docs/references/cli)

The Anchor CLI for building and managing Anchor workspaces.

## Install

<!-- locked-in: ignore[npm-version-pin] -->
```sh
npm install -g @anchor-lang/cli@latest
anchor --version
```

For reproducible installs, we recommend pinning to a specific version
instead of `@latest`, e.g. `npm install -g @anchor-lang/cli@1.0.2`. This
ensures the CLI version is locked across machines and CI runs.

For most users, the recommended installation method is [AVM](https://www.anchor-lang.com/docs/installation).

## Platform Support

This npm package currently bundles the `anchor` binary for `x86_64` Linux only.

On other platforms, the wrapper will try to use a globally installed `anchor`
binary with the same version. If you need a native installation flow, use the
[installation guide](https://www.anchor-lang.com/docs/installation).

## Documentation

- [Installation](https://www.anchor-lang.com/docs/installation)
- [CLI reference](https://www.anchor-lang.com/docs/references/cli)
- [Repository](https://github.com/otter-sec/anchor)
