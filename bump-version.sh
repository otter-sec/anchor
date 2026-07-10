#!/bin/bash

set -ex

if [ $# -eq 0 ]; then
    echo "Usage $0 VERSION"
    exit 1
fi

old_version=$(cat VERSION)
old_version_regex=$(printf '%s\n' "$old_version" | sed 's/[.[\*^$()+?{}|\\]/\\&/g') # escape .
version=$1

if [[ "$version" == v* ]]; then
    echo "The version number must not contain the v[...] prefix"
    exit 1
fi

is_prerelease=0
if [[ "$version" =~ ^[0-9]+\.[0-9]+\.[0-9]+-.+ ]]; then
    is_prerelease=1
fi

echo "Bumping versions to $version (is_prerelease=$is_prerelease)"

# GNU/BSD compat
sedi=(-i)
case "$(uname)" in
  # For macOS, use two parameters
  Darwin*) sedi=(-i "")
esac

# Bump all rust crates that have `publish` enabled (excluding crates that are
# versioned separately)
cargo release version $version \
    --workspace \
    --exclude anchor-lang-idl \
    --exclude anchor-lang-idl-spec \
    $(cargo metadata --no-deps --format-version 1 | jq -r '.packages[] | select(.publish == []) | "--exclude " + .name') \
    --no-confirm \
    --execute

# Only replace version with the following globs
allow_globs="**/Makefile client/src/lib.rs lang/attribute/program/src/lib.rs"
git grep -l "$old_version" -- $allow_globs |
    xargs sed "${sedi[@]}" \
    -e "s/$old_version_regex/$version/g"

# Avoid updating the docs for pre-release builds
if [[ "$is_prerelease" -eq 0 ]]; then
    latest_stable_version=$(
        git tag --sort=-version:refname | \
            grep -E '^v[0-9]+\.[0-9]+\.[0-9]+$' | \
            head -n1 | \
            sed 's/^v//'
    )
    latest_stable_version_regex=$(printf '%s\n' "$latest_stable_version" | sed 's/[.[\*^$()+?{}|\\]/\\&/g')
    echo "Latest stable version for documentation was $latest_stable_version..."

    # Separately handle docs because blindly replacing the old version with the new
    # might break certain examples/links
    pushd docs/content/docs
    git grep -l "$latest_stable_version" -- "./*.md*" | \
        xargs sed "${sedi[@]}" \
        -e "s/\"$latest_stable_version_regex\"/\"$version\"/g"
    allow_globs="installation.mdx quickstart/local.mdx references/verifiable-builds.mdx"
    git grep -l "$latest_stable_version" -- $allow_globs |
        xargs sed "${sedi[@]}" \
        -e "s/$latest_stable_version_regex/$version/g"
    # Replace `solana_version` with the current version
    solana_version=$(solana --version | awk '{print $2;}')
    sed "${sedi[@]}" "s/solana_version.*\"/solana_version = \"$solana_version\"/g" references/anchor-toml.mdx
    # Keep release notes and changelog the same
    git restore updates
    popd

    # If the release notes are missing from the website, we should at the minimum create a placeholder linking to the github release notes
    VERSION_URL=$(echo $version | sed 's/\./-/g')
    VERSION_CHANGELOG=$(echo $version | sed 's/\.//g')
    RELEASE_NOTE_PATH="docs/content/docs/updates/release-notes/${VERSION_URL}.mdx"
    RELEASE_NOTE_META_PATH="docs/content/docs/updates/release-notes/meta.json"
    CHANGELOG_TEXT="See the full [CHANGELOG](https://github.com/otter-sec/anchor/blob/v${version}/CHANGELOG.md#${VERSION_CHANGELOG}---$(date '+%Y-%m-%d'))."
    if [[ ! -f "$RELEASE_NOTE_PATH" ]]; then
        cat <<EOF > "$RELEASE_NOTE_PATH"
---
title: $version
description: Anchor - Release Notes $version
---

$CHANGELOG_TEXT
EOF

        # Insert the version into release notes meta, and sort the versions so the order is correct
        tmp=$(mktemp)
        jq --arg v "$VERSION_URL" '
            .pages |= (
                . + [$v]
                | unique
                | sort_by(split("-") | map(tonumber))
                | reverse
            )
        ' "$RELEASE_NOTE_META_PATH" > "$tmp"
        mv "$tmp" "$RELEASE_NOTE_META_PATH"
    fi

    # Additionally, add to the changelog the version if it's missing
    CHANGELOG_PATH="docs/content/docs/updates/changelog.mdx"
    if ! grep -qF "## [$version]" "$CHANGELOG_PATH"; then
        PREVIOUS_VERSION_URL=$(
            jq -r --arg v "$VERSION_URL" '
                .pages
                | map(select(. != $v))
                | sort_by(split("-") | map(tonumber))
                | reverse
                | .[0]
            ' "$RELEASE_NOTE_META_PATH"
        )
        PREVIOUS_VERSION=$(echo "$PREVIOUS_VERSION_URL" | sed 's/-/./g')
        CHANGELOG_ENTRY=$(cat <<EOF
## [$version]

$CHANGELOG_TEXT
EOF
)

        tmp=$(mktemp)
        awk -v entry="$CHANGELOG_ENTRY" -v prev="## [$PREVIOUS_VERSION]" '
            $0 == prev {
                print entry
                print ""
            }
            { print }
        ' "$CHANGELOG_PATH" > "$tmp" && mv "$tmp" "$CHANGELOG_PATH"
    fi
fi

# Potential for collisions in `package.json` files, handle those separately
# Replace only matching "version": "x.xx.x" and "@anchor-lang/core": "x.xx.x"
git grep -l "$old_version" -- "**/package.json" | \
    xargs sed -E "${sedi[@]}" \
    -e "s/\"version\": \"$old_version_regex\"/\"version\": \"$version\"/g" \
    -e "s/@anchor-lang\/(.*)\": \"(.*)$old_version_regex\"/@anchor-lang\/\1\": \"\2$version\"/g"

# Insert version number into CHANGELOG
sed "${sedi[@]}" -e \
    "s/## \[Unreleased\]/## [Unreleased]\n\n### Features\n\n### Fixes\n\n### Breaking\n\n## [$version] - $(date '+%Y-%m-%d')/g" \
    CHANGELOG.md

# Update lock files
# Cannot use --frozen-lockfile: package.json versions were just bumped, so refresh the lockfiles.
# Only workspace versions changed above; if lockfile diffs look like broad third-party churn, investigate before tagging.
pushd ts
yarn install  # locked-in: ignore[yarn-frozen-lockfile]
popd

pushd tests
yarn install  # locked-in: ignore[yarn-frozen-lockfile]
popd

pushd examples/tutorial
yarn install  # locked-in: ignore[yarn-frozen-lockfile]
popd

# Avoid updating the benchmarks for pre-release builds
if [[ "$is_prerelease" -eq 0 ]]; then
    # Bump benchmark files
    pushd tests/bench
    anchor run bump-version -- --anchor-version $version
    popd
fi

echo $version > VERSION

echo "$(git diff --stat | tail -n1) files modified"

echo "$version changeset generated, commit and tag"
