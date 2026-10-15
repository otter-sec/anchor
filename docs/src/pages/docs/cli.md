---
title: CLI
description: Anchor - CLI
---

A CLI is provided to support building and managing an Anchor workspace.
For a comprehensive list of commands and options, run `anchor -h` on any
of the following subcommands.

---

```shell
anchor-cli

USAGE:
    anchor <SUBCOMMAND>

FLAGS:
    -h, --help       Prints help information
    -V, --version    Prints version information

SUBCOMMANDS:
    account    Fetch and deserialize an account using the IDL provided
    build      Builds the workspace
    cluster    Cluster commands
    deploy     Deploys each program in the workspace
    expand     Expands the macros of a program or the workspace
    help       Prints this message or the help of the given subcommand(s)
    idl        Commands for interacting with interface definitions
    init       Initializes a workspace
    migrate    Runs the deploy migration script
    new        Creates a new program
    shell      Starts a node shell with an Anchor client setup according to the local config
    test       Runs integration tests against a localnetwork
    upgrade    Upgrades a single program. The configured wallet must be the upgrade authority
    verify     Verifies the on-chain bytecode matches the locally compiled artifact. Run this
               command inside a program subdirectory, i.e., in the dir containing the program's
               Cargo.toml
```

## Account

```
anchor account <program-name>.<AccountTypeName> <account_pubkey>
```

Fetches an account with the given public key and deserializes the data to JSON using the type name provided. If this command is run from within a workspace, the workspace's IDL files will be used to get the data types. Otherwise, the path to the IDL file must be provided.

The `program-name` is the name of the program where the account struct resides, usually under `programs/<program-name>`. `program-name` should be provided in a case-sensitive manner exactly as the folder name, usually in kebab-case.

The `AccountTypeName` is the name of the account struct, usually in PascalCase.

The `account_pubkey` refers to the Pubkey of the account to deserialise, in Base58.

Example Usage: `anchor account anchor-escrow.EscrowAccount 3PNkzWKXCsbjijbasnx55NEpJe8DFXvEEbJKdRKpDcfK`, deserializes an account in the given pubkey with the account struct `EscrowAccount` defined in the `anchor-escrow` program.

```
anchor account <program-name>.<AccountTypeName> <account_pubkey> --idl <path/to/idl.json>
```

Deserializes the account with the data types provided in the given IDL file even if inside a workspace.

## Build

```shell
anchor build
```

Builds programs in the workspace targeting Solana's BPF runtime and emitting IDLs in the `target/idl` directory.

Builds default to SBPF architecture `v3` and platform-tools `v1.42.1`, the
v3 backport selected by AVM's map for Anchor 0.30. Override either explicitly:

```shell
anchor build --tools-version v1.42.1 --arch v3
```

These options also apply to verifiable builds. `anchor build --arch` selects
the SBPF version (`v0`, `v1`, `v2`, or `v3`), replacing the old
`bpf`/`sbf` backend selector. Use a `cargo-build-sbf` version that supports v3.

```shell
anchor build --verifiable
```

Runs the build inside a docker image so that the output binary is deterministic (assuming a Cargo.lock file is used). This command must be run from within a single crate subdirectory within the workspace. For example, `programs/<my-program>/`.

## Cluster

### Cluster list

```shell
anchor cluster list
```

This lists cluster endpoints:

```shell
Cluster Endpoints:

* Mainnet - https://api.mainnet-beta.solana.com
* Devnet  - https://api.devnet.solana.com
* Testnet - https://api.testnet.solana.com
```

## Deploy

```shell
anchor deploy
```

Deploys all programs in the workspace to the configured cluster.

{% callout title="Tip" %}
This is different from the `solana program deploy` command, because every time it's run
it will generate a _new_ program address.
{% /callout %}

## Expand

```shell
anchor expand
```

If run inside a program folder, expands the macros of the program.

If run in the workspace but outside a program folder, expands the macros of the workspace.

If run with the `--program-name` option, expand only the given program.

## Idl

The `idl` subcommand provides commands for interacting with interface definition files.
It's recommended to use these commands to store an IDL on chain, at a deterministic
address, as a function of nothing but the program's ID. This
allows us to generate clients for a program using nothing but the program ID.

### Idl Init

```shell
anchor idl init -f <target/idl/program.json> <program-id>
```

Creates an idl account, writing the given `<target/idl/program.json>` file into a program owned account. By default, the size of the account is double the size of the IDL,
allowing room for growth in case the idl needs to be upgraded in the future.

### Idl Fetch

```shell
anchor idl fetch -o <out-file.json> <program-id>
```

Fetches an IDL from the configured blockchain. For example, make sure
your `Anchor.toml` is pointing to the `mainnet` cluster and run

```shell
anchor idl fetch GrAkKfEpTKQuVHG2Y97Y2FF4i7y7Q5AHLK94JBy7Y5yv
```

### Idl Authority

```shell
anchor idl authority <program-id>
```

Outputs the IDL account's authority. This is the wallet that has the ability to
update the IDL.

### Idl Erase Authority

```shell
anchor idl erase-authority -p <program-id>
```

Erases the IDL account's authority so that upgrades can no longer occur. The
configured wallet must be the current authority.

### Idl Upgrade

```shell
anchor idl upgrade <program-id> -f <target/idl/program.json>
```

Upgrades the IDL file on chain to the new `target/idl/program.json` idl.
The configured wallet must be the current authority.

```shell
anchor idl set-authority -n <new-authority> -p <program-id>
```

Sets a new authority on the IDL account. Both the `new-authority` and `program-id`
must be encoded in base 58.

## Init

```shell
anchor init
```

Initializes a project workspace with the following structure.

- `Anchor.toml`: Anchor configuration file.
- `Cargo.toml`: Rust workspace configuration file.
- `package.json`: JavaScript dependencies file.
- `programs/`: Directory for Solana program crates.
- `app/`: Directory for your application frontend.
- `tests/`: Directory for JavaScript integration tests.
- `migrations/deploy.js`: Deploy script.

## Migrate

```shell
anchor migrate
```

Runs the deploy script located at `migrations/deploy.js`, injecting a provider configured
from the workspace's `Anchor.toml`. For example,

```javascript
// File: migrations/deploys.js

const anchor = require('@anchor-lang/core')

module.exports = async function (provider) {
  anchor.setProvider(provider)

  // Add your deploy script here.
}
```

Migrations are a new feature
and only support this simple deploy script at the moment.

## New

```shell
anchor new <program-name>
```

Creates a new program in the workspace's `programs/` directory initialized with boilerplate.

## Shell

```shell
anchor shell
```

Starts a node js shell with an Anchor client setup according to the local config. This client can be used to interact with deployed Solana programs in the workspace.

## Test

```shell
anchor test
```

Run an integration test suit against the configured cluster, deploying new versions
of all workspace programs before running them.

If the configured network is a localnet, then automatically starts the localnetwork and runs
the test.

{% callout title="Note" %}
Be sure to shutdown any other local validators, otherwise `anchor test` will fail to run.

If you'd prefer to run the program against your local validator use `anchor test --skip-local-validator`.
{% /callout %}

When running tests we stream program logs to `.anchor/program-logs/<address>.<program-name>.log`

{% callout title="Note" %}
The Anchor workflow [recommends](https://www.parity.io/paritys-checklist-for-secure-smart-contract-development/) to test your program using integration tests in a language other than Rust to make sure that bugs related to syntax misunderstandings are coverable with tests and not just replicated in tests.
{% /callout %}

## Upgrade

```shell
anchor upgrade <target/deploy/program.so> --program-id <program-id>
```

Uses Solana's upgradeable BPF loader to upgrade the on chain program code.

## Verify

```shell
anchor verify <program-id>
```

Verifies the on-chain bytecode matches the locally compiled artifact.
