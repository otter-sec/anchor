#!/bin/bash

# This test script verifies that running 'anchor' commands
# correctly proxies to the installed Anchor CLI binary (e.g., shows Anchor "something")
# instead of launching AVM itself.

# Exit on first error
set -e

# --- Helper for timing and logging ---
function step_start {
    echo ""
    echo "============================================================"
    echo "🚀 Starting Step: $1"
    echo "============================================================"
    STEP_START_TIME=$(date +%s)
}

function step_end {
    local end_time=$(date +%s)
    local duration=$((end_time - STEP_START_TIME))
    echo "✅ Step completed in ${duration}s."
}


trap 'echo ""; echo "🧹 Cleaning up..."' INT TERM EXIT

step_start "Building local avm"
(cd ../.. && cargo build --package avm)
step_end

step_start "Installing local anchor-cli via avm (force to ensure fresh install)"
../../target/debug/avm install --path ../.. --force
step_end

# --- Set a Specific Version ---
step_start "Setting AVM to use a known version (e.g., latest)"
../../target/debug/avm use latest
step_end

# --- Test 'avm' Command (Should Show AVM Help) ---
step_start "Testing 'avm --help' (should show AVM usage)"
AVM_OUTPUT=$(~/.avm/bin/avm --help 2>&1) || true
if echo "$AVM_OUTPUT" | grep -q "Anchor version manager"; then
  echo "✅ 'avm --help' shows AVM usage as expected."
else
  echo "❌ Test failed: 'avm --help' did not show expected AVM output."
  echo "$AVM_OUTPUT"
  exit 1
fi
step_end

# --- Test 'anchor' Command (Should Proxy to Anchor CLI) ---
step_start "Testing 'anchor --version' (should show Anchor CLI version, not AVM)"
ANCHOR_OUTPUT=$(~/.avm/bin/anchor --version 2>&1) || true
if echo "$ANCHOR_OUTPUT" | grep -q "anchor-cli"; then
  echo "✅ 'anchor --version' proxies to Anchor CLI successfully."
elif echo "$ANCHOR_OUTPUT" | grep -q "Anchor version manager"; then
  echo "❌ Test failed: 'anchor --version' is still launching AVM instead of proxying."
  echo "$ANCHOR_OUTPUT"
  exit 1
else
  echo "❌ Test failed: Unexpected output from 'anchor --version'."
  echo "$ANCHOR_OUTPUT"
  exit 1
fi
step_end

echo ""
echo "============================================================"
echo "🎉 All tests passed successfully! 🎉"
echo "============================================================"