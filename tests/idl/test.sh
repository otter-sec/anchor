#!/usr/bin/env bash
set -e

# Generate temp directory
tmp_dir=$(mktemp -d)

# Fix external type resolution not working in CI due to missing `anchor-lang`
# crates.io entry in runner machine.
pushd $tmp_dir
cargo new external-ci
pushd external-ci
cargo add anchor-lang
cargo b
popd
popd

# Run anchor test
anchor test --skip-lint

# Verify workspace.idl copies the generated JSON files out of `target/idl`.
for idl in target/idl/*.json; do
    filename=$(basename "$idl")
    diff -u "$idl" "workspace-idls/$filename"
done

# Verify workspace.types copies the generated TypeScript files out of `target/types`.
for types in target/types/*.ts; do
    filename=$(basename "$types")
    diff -u "$types" "workspace-types/$filename"
done

# Generate IDLs
./generate.sh $tmp_dir

# Exit status
ret=0

# Compare IDLs. `$ret` will be non-zero in the case of a mismatch.
compare() {
    echo "----------------------------------------------------"
    echo "IDL $1 before > after changes"
    echo "----------------------------------------------------"
    diff -y --color=always --suppress-common-lines idls/$1.json $tmp_dir/$1.json
    ret=$(($ret+$?))

    if [ "$ret" = "0" ]; then
        echo "No changes"
    fi

    echo ""
}

compare "idl"
compare "generics"
compare "relations"

exit $ret
