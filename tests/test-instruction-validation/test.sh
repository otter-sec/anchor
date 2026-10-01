#!/bin/bash

set -e

expect_build_failure() {
    expected_error="$1"
    pass_message="$2"

    if BUILD_OUTPUT=$(cargo build 2>&1); then
        echo "FAIL: Expected compilation error but build succeeded"
        echo "Build output: $BUILD_OUTPUT"
        exit 1
    fi

    if echo "$BUILD_OUTPUT" | grep -q "$expected_error"; then
        echo "$pass_message"
    else
        echo "FAIL: Build failed with an unexpected error"
        echo "Build output: $BUILD_OUTPUT"
        exit 1
    fi
}

expect_build_success() {
    if BUILD_OUTPUT=$(cargo build 2>&1); then
        echo "$1"
    else
        echo "FAIL: Expected successful compilation but build failed"
        echo "Build output: $BUILD_OUTPUT"
        exit 1
    fi
}

echo "Test 1: Running FAIL-ARGS-COUNT case (expects compilation error)..."
cd fail-args-count
expect_build_failure \
    "expects MORE args" \
    "PASS: FAIL-ARGS-COUNT case correctly caught parameter mismatch at compile time"
cd ..

echo "Test 2: Running PASS-ARGS-COUNT case (expects successful compilation)..."
cd pass-args-count
expect_build_success "PASS: PASS-ARGS-COUNT case compiled successfully"
cd ..

echo "Test 3: Running FAIL-TYPE case (expects compilation error)..."
cd fail-type
expect_build_failure \
    "error\[E0308\]: mismatched types" \
    "PASS: FAIL-TYPE case correctly caught type mismatch at compile time"
cd ..

echo "Test 4: Running PASS-TYPE case (expects successful compilation)..."
cd pass-type
expect_build_success "PASS: PASS-TYPE case compiled successfully"
cd ..
