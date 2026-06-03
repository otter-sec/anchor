# Swap

An example swap program that provides a convenient API to the Serum orderbook
for performing instantly settled token swaps.

## Usage

This example requires building the Serum DEX from source, which is done using
git submodules.

### Install Submodules

Pull the source

```
git submodule init
git submodule update
```

### Build the DEX

Build it

```
cd deps/serum-dex/dex/ && cargo build-bpf && cd ../../../
```

### Run the Test

Run the test

```
anchor test
```
