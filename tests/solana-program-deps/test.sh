#!/bin/bash

set -euo pipefail

assert_build_warns() {
  local workspace_dir="$1"
  local expected_warning="$2"
  local output

  pushd "$workspace_dir" > /dev/null
  if ! output=$(anchor build --ignore-keys 2>&1 > /dev/null); then
    echo "Error: anchor build failed in $workspace_dir"
    echo "$output"
    exit 1
  fi
  popd > /dev/null

  if [[ "$output" != *"$expected_warning"* ]]; then
    echo "Error: expected warning in $workspace_dir"
    echo "$output"
    exit 1
  fi
}

assert_build_succeeds() {
  local workspace_dir="$1"
  local unexpected_warning="$2"
  local output

  pushd "$workspace_dir" > /dev/null
  if ! output=$(anchor build --ignore-keys 2>&1 > /dev/null); then
    echo "Error: anchor build failed in $workspace_dir when it should have succeeded"
    echo "$output"
    exit 1
  fi
  popd > /dev/null

  if [[ -n "$unexpected_warning" && "$output" == *"$unexpected_warning"* ]]; then
    echo "Error: unexpected warning in $workspace_dir"
    echo "$output"
    exit 1
  fi
}

warning='Adding `solana-program` as a separate dependency might cause conflicts.'

echo "Checking mismatched solana-program dependency"
assert_build_warns mismatched "$warning"

echo "Checking matching solana-program dependency"
assert_build_succeeds matching "$warning"

echo "Success. Solana dependency checks matched expectations."
