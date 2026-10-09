#!/usr/bin/env bash
# Builds every tests-v2 program with the upstream BPF toolchain in a scratch
# workspace, then (optionally) runs the tests-v2 LiteSVM suites against them.
#
#   ./stress.sh [scratch_dir]          # build only
#   RUN_TESTS=1 ./stress.sh            # build + run tests-v2 suites
#
# A scratch copy is needed because the programs must be patched for upstream
# BPF (no_std shim, `crate-type = ["lib"]`), which can't be done in place.
set -euo pipefail
HERE=$(cd "$(dirname "$0")" && pwd)
REPO=$(cd "$HERE/../.." && pwd)
S=${1:-/tmp/anchor-upstream-bpf-stress}
rm -rf "$S"; mkdir -p "$S/logs"
cp -r "$REPO/tests-v2/programs" "$S/programs"
find "$S/programs" -name target -type d -prune -exec rm -rf {} +
find "$S/programs" -name Cargo.lock -delete
cp -r "$HERE/hello-world/.cargo" "$HERE/hello-world/rust-toolchain.toml" "$S/"

python3 - "$S" "$REPO/tests-v2/programs" <<'EOF'
import glob, os, re, sys
S, ORIG = sys.argv[1], sys.argv[2]
# User-code no_std shims: crate attr + alloc prelude (Box/Vec/String/format!/vec!).
SHIM = '''#![cfg_attr(target_arch = "bpf", no_std)]
#[cfg(target_arch = "bpf")]
extern crate alloc as __bpf_alloc;
#[cfg(target_arch = "bpf")]
#[allow(unused_imports)]
use __bpf_alloc::{borrow::ToOwned, boxed::*, string::*, vec::*, *};
'''
members, cdylibs = [], []
for m in glob.glob(S + '/programs/**/Cargo.toml', recursive=True):
    d = os.path.dirname(m); od = os.path.join(ORIG, os.path.relpath(d, S + '/programs'))
    s = open(m).read()
    def fix(mm):
        t = os.path.normpath(os.path.join(od, mm.group(2)))
        return mm.group(0) if t.startswith(ORIG + '/') else mm.group(1) + t + mm.group(3)
    s = re.sub(r'(path\s*=\s*")([^"]+)(")', fix, s)
    s = re.sub(r'(?ms)^\[workspace\].*?(?=^\[|\Z)', '', s)
    # cdylib deps (e.g. `callee` with `cpi`) can't link without std; build the
    # cdylib explicitly with `cargo rustc --crate-type cdylib` instead.
    is_cdylib = 'crate-type = ["cdylib", "lib"]' in s
    s = s.replace('crate-type = ["cdylib", "lib"]', 'crate-type = ["lib"]')
    if is_cdylib:
        cdylibs.append(re.search(r'(?m)^name\s*=\s*"([^"]+)"', s).group(1))
    if '[package]' in s:
        s = re.sub(r'(?m)^\[dependencies\]\n', '[dependencies]\nsolana-compiler-builtins = "0.1.0"\n', s, count=1)
        members.append(os.path.relpath(d, S))
    open(m, 'w').write(s)
for f in glob.glob(S + '/programs/**/src/lib.rs', recursive=True):
    lines = open(f).read().split('\n'); i = 0
    while i < len(lines) and (lines[i].startswith(('#![', '//!')) or not lines[i].strip()): i += 1
    lines.insert(i, SHIM); open(f, 'w').write('\n'.join(lines))
open(S + '/Cargo.toml', 'w').write('[workspace]\nresolver = "2"\nmembers = [\n'
    + ''.join(f'  "{x}",\n' for x in sorted(members))
    + ']\n\n[profile.release]\noverflow-checks = true\ncodegen-units = 1\n')
open(S + '/cdylibs.txt', 'w').write('\n'.join(cdylibs) + '\n')
EOF

cd "$S"
for p in $(cargo metadata --no-deps --format-version 1 | python3 -c 'import json,sys;[print(p["name"]) for p in json.load(sys.stdin)["packages"]]'); do
  extra=(); grep -qx "$p" cdylibs.txt && extra=(--crate-type cdylib)
  if cargo rustc -p "$p" --release --target bpfel-unknown-none -Zbuild-std=core,alloc "${extra[@]}" >"logs/$p.log" 2>&1; then
    echo "OK   $p"
  else
    echo "FAIL $p  ($(grep -m1 -E "is required by|^error" "logs/$p.log" | sed 's/^ *= note: //'))"
  fi
done | tee results.txt

if [[ "${RUN_TESTS:-}" == 1 ]]; then
  cd "$REPO"
  export ANCHOR_V2_PREBUILT_SO_DIR="$S/target/deploy"
  for t in accounts account_meta_signer_overrides borsh_realloc client_builders constraints \
           custom_constraints cpi declare_program derives dispatch_remaining dup_mut event_cpi \
           init_space_usability ix_macro min_ix_data_len optional_accounts pda_payer seeds space_annotation; do
    echo "$t: $(cargo test -p tests-v2 --test $t 2>&1 | grep -E '^test result' | tail -1)"
  done
fi
