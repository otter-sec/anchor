#!/bin/sh
set -e

RUSTC_BOOTSTRAP=1 RUSTFLAGS="-Z emit-stack-sizes" anchor test --skip-lint
