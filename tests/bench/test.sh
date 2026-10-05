#!/bin/sh
set -e

ANCHOR_BUILD_SBF_ARCH=v3 \
  ANCHOR_TEST_VALIDATOR=surfpool \
  RUSTC_BOOTSTRAP=1 \
  CARGO_TARGET_SBF_SOLANA_SOLANA_RUSTFLAGS="-Z emit-stack-sizes" \
  CARGO_TARGET_SBPF_SOLANA_SOLANA_RUSTFLAGS="-Z emit-stack-sizes" \
  CARGO_TARGET_SBPFV1_SOLANA_SOLANA_RUSTFLAGS="-Z emit-stack-sizes" \
  CARGO_TARGET_SBPFV2_SOLANA_SOLANA_RUSTFLAGS="-Z emit-stack-sizes" \
  CARGO_TARGET_SBPFV3_SOLANA_SOLANA_RUSTFLAGS="-Z emit-stack-sizes" \
  anchor test --skip-lint --validator surfpool
