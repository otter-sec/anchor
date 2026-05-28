# Contributing to Anchor

Thank you for your interest in contributing to Anchor! All contributions are welcome no
matter how big or small. This includes (but is not limited to) filing issues,
adding documentation, fixing bugs, creating examples, and implementing features.

## Finding issues to work on

If you're looking to get started,
check out [good first issues](https://github.com/otter-sec/anchor/issues?q=is%3Aissue+is%3Aopen+label%3A%22good+first+issue%22)
or issues where [help is wanted](https://github.com/otter-sec/anchor/issues?q=is%3Aissue+is%3Aopen+label%3A%22help+wanted%22).
PRs that only correct typos or make minor wording adjustments will be rejected. Fixing typos alongside other non-trivial engineering work is welcome.

If you're considering larger changes or self motivated features, please file an issue
and engage with the maintainers in [Discord](https://discord.gg/NHHGSXAnXk).

## Choosing an issue

If you'd like to contribute, please claim an issue by commenting, forking, and
opening a pull request, even if empty. This allows the maintainers to track who
is working on what issue as to not overlap work.

## Branch Targeting

Pull requests should usually target `master`.

If your change is breaking, open the pull request against `anchor-next`
instead of `master`.

## Issue Guidelines

Please follow these guidelines:

Before coding:

- choose a branch name that describes the issue you're working on
- enable [commit signing](https://docs.github.com/en/authentication/managing-commit-signature-verification/signing-commits)

While coding:

- Submit a draft PR asap
- Only change code directly relevant to your PR. Sometimes you might find some code that could really need some refactoring. However, if it's not relevant to your PR, do not touch it. File an issue instead. This allows the reviewer to focus on a single problem at a time.
- If you write comments, do not exceed 80 chars per line. This allows contributors who work with multiple open windows to still read the comments without horizontally scrolling.
- Write adversarial tests. For example, if you're adding a new account type, do not only write tests where the instruction succeeds. Also write tests that test whether the instruction fails, if a check inside the new type is violated.

After coding:

- If you've moved code around, build the docs with `cargo doc --open` and adjust broken links
- Adjust the cli templates if necessary
- If you've added a new folder to the `tests` directory, add it to the [CI](./.github/workflows/tests.yaml).
- Before opening a PR, build, test, and run formatting and lints locally; see the sections below for commands.

## Building and Testing Locally

Use the commands below to validate changes before opening a PR.

### Build

```sh
# Rust workspace
cargo build

# TypeScript packages (run in the package/workspace you changed)
yarn build
```

### Test Categories

#### 1) Rust unit tests

```sh
cargo test
```

#### 2) TypeScript unit tests

```sh
yarn test
```

Run `yarn test` in the relevant package/workspace (for example
`ts/packages/anchor`).

#### 3) Anchor integration tests

```sh
anchor test
```

These are the integration tests under the root `tests/` folder.
Contributors should run the tests that cover their changes and/or add new
test cases.

Integration and other non-Rust tests depend on local TS package linking.
You can set this up in either of these ways:

- Recommended: run `./setup-tests.sh`
- Manual: use `yarn link` to link local Anchor TS packages

For `anchor test`, ensure the `anchor` command points to your local CLI
build (for example via `./setup-tests.sh`, `cargo install --path cli`, or
running the binary from `target` directly).

### AVM and local CLI installs

`cargo install --path cli` writes `anchor` into `~/.cargo/bin/anchor`.
AVM also manages `anchor` in that same location via symlink, so installing
from Cargo overrides AVM version selection until reverted.

To restore AVM-managed behavior:

- uninstall/remove the Cargo-installed binary (`cargo uninstall anchor-cli`
  or `rm ~/.cargo/bin/anchor`)
- re-run AVM so it rewrites the symlink

## Formatting and Linting

If you have edited any Rust or TypeScript/JavaScript code, ensure it is correctly formatted and compatible with our lint suite; this will ensure CI passes and help us quickly review your contribution.

### Rust

In the root workspace, run:

```sh
# Ensure you have the latest nightly version with `rustup update nightly`
cargo +nightly fmt
cargo clippy --all-targets -- -D warnings
```

### TypeScript/JavaScript

Enter the relevant JS package (e.g. `tests/`, or `ts/packages/anchor`) and run:

```sh
yarn
yarn lint:fix
```
