#!/bin/bash

set -e

echo "Test 1: Running FAIL-ARGS-COUNT case (expects compilation error)..."
cd fail-args-count
if cargo build 2>&1 | grep -q "expects MORE args"; then
    echo "PASS: FAIL-ARGS-COUNT case correctly caught parameter mismatch at compile time"
else
    echo "FAIL: Expected compilation error but build succeeded or wrong error"
    echo "Build output: $BUILD_OUTPUT"
    exit 1
fi
cd ..

echo "Test 2: Running PASS-ARGS-COUNT case (expects successful compilation)..."
cd pass-args-count
if cargo build 2>&1 | grep -q "Finished"; then
    echo "PASS: PASS-ARGS-COUNT case compiled successfully"
else
    echo "FAIL: Expected successful compilation but build failed"
    exit 1
fi
cd ..

echo "Test 3: Running FAIL-TYPE case (expects compilation error)..."
cd fail-type
if cargo build 2>&1 | grep -q "IsSameType"; then
    echo "PASS: Fail-TYPE case correctly caught type mismatch at compile time"
else
    echo "FAIL: Expected compilation error but build succeeded or wrong error"
    echo "Build output: $BUILD_OUTPUT"
    exit 1
fi
cd ..

echo "Test 4: Running PASS-TYPE case (expects successful compilation)..."
cd pass-type
if cargo build 2>&1 | grep -q "Finished"; then
    echo "PASS: PASS-TYPE case compiled successfully"
else
    echo "FAIL: Expected successful compilation but build failed"
    exit 1
fi
cd ..