# Anchor benchmark harness

Benchmarks Anchor account validation and deserialization across releases:

- compute units;
- deployable binary size;
- `Accounts::try_accounts` stack usage.

## Usage

Run the current checkout:

```sh
./bench/bench bench
```

Run all supported versions or select specific versions:

```sh
./bench/bench bench all
./bench/bench bench 0.32.1 1.2.0
```

`all` runs the unreleased checkout first.

Each run prints `Benchmarking <version>` and either `No change` or a field-level
diff. `--verbose` also prints commands and JSON results. `--check` does not write
results and returns nonzero if a measurement changes by more than 1%.

The repository release script uses `./bench/bench bump-version <version>` to
move the unreleased results to a stable release and regenerate its fixture
lockfile.

## Reproducibility

`results.json` defines the versions and pins their `solanaVersion`; a version is
runnable when it has a lockfile in `locks/`. The fixture depends directly on the
selected Anchor crates, while AVM installs the matching Solana and
platform-tools versions. Builds before Anchor 1.2.0 use SBPF v0; newer builds
use v3.

Compute units are measured in-process with the workspace-pinned LiteSVM. A
successful run updates `results.json` and regenerates the benchmark Markdown
files in this directory.
