#!/usr/bin/env bash
set -euo pipefail

# Every top-level tests-v2 integration target must be either instrumented for
# coverage or deliberately classified as a non-coverage test. This prevents a
# new fixture from silently falling out of anchor-next CI.
repo_root=$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)
makefile="$repo_root/Makefile"

expected=$(mktemp)
classified=$(mktemp)
trap 'rm -f "$expected" "$classified"' EXIT

{
  find "$repo_root/tests-v2/tests" -maxdepth 1 -type f -name '*.rs' -exec basename {} .rs \;
  awk '
    /^\[\[test\]\]/ { in_test = 1; next }
    /^\[/ { in_test = 0 }
    in_test && /^name = / {
      gsub(/^name = "|"$/, "")
      print
    }
  ' "$repo_root/tests-v2/Cargo.toml"
} | sort -u > "$expected"
{
  awk '
    /^TESTS_V2_COVERAGE_TESTS :=/ { in_list = 1; next }
    /^TESTS_V2_COVERAGE_ARGS :=/ { in_list = 0 }
    in_list {
      gsub(/\\/, "")
      gsub(/^[[:space:]]+|[[:space:]]+$/, "")
      if ($0 != "") print $0
    }
  ' "$makefile"
  sed -n 's/^TESTS_V2_NON_COVERAGE_TESTS := //p' "$makefile" | tr ' ' '\n' | sed '/^$/d'
} | sort -u > "$classified"

if ! diff -u "$expected" "$classified"; then
  echo 'Every tests-v2 integration target must be listed in Makefile coverage or non-coverage tests.' >&2
  exit 1
fi
