#!/bin/bash
set -e

echo "Safety Checks Test Suite"

#
# UncheckedAccount with duplicate field names
#

echo "[TEST 1] UncheckedAccount - Duplicate field names with struct context tracking"
pushd programs/unchecked-account/ > /dev/null
output=$(anchor build 2>&1 || true)
if [[ $output =~ "Struct \"FuncTwo\" field \"unchecked\" is unsafe" ]]; then
   echo "✓ PASS: Error message includes struct name (FuncTwo)"
else
   echo "✗ FAIL: Error message should include 'Struct \"FuncTwo\" field \"unchecked\" is unsafe'"
   echo "Actual output:"
   echo "$output"
   exit 1
fi
popd > /dev/null
echo ""

#
# AccountInfo field safety check
#

echo "[TEST 2] AccountInfo - Struct name validation"
pushd programs/account-info/ > /dev/null
output=$(anchor build 2>&1 || true)
if [[ $output =~ "Struct \"Initialize\" field \"unchecked\" is unsafe" ]]; then
   echo "✓ PASS: Error message includes struct name (Initialize)"
else
   echo "✗ FAIL: Error message should include 'Struct \"Initialize\" field \"unchecked\" is unsafe'"
   echo "Actual output:"
   echo "$output"
   exit 1
fi
popd > /dev/null
echo ""

#
# Non-Account structs should be ignored
#

echo "[TEST 3] Non-Account structs - Safety checks should be ignored"
pushd programs/ignore-non-accounts/ > /dev/null
if anchor build > /dev/null 2>&1 ; then
   echo "✓ PASS: Build succeeded (non-account structs properly ignored)"
else
   echo "✗ FAIL: Build should succeed for non-account structs"
   exit 1
fi
popd > /dev/null
echo ""

echo "All Tests Passed"
echo "✓ Struct names correctly included in error messages"
echo "✓ Duplicate field names are properly distinguished"
echo "✓ Non-account structs do not trigger false positives"
