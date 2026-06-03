#!/usr/bin/env bash
set -euo pipefail

# We don't want to build `multiple-errors` because it is expected to error
anchor build -p errors --skip-lint --ignore-keys
anchor test --skip-build