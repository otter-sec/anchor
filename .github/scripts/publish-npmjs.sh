#!/usr/bin/env bash

set -xeuo pipefail

publish() {
  local dir="$1"
  pushd "$dir" >/dev/null

  local name version
  name="$(node -p "require('./package.json').name")"
  version="$(node -p "require('./package.json').version")"

  # We still build the package even if we don't publish it, as yarn workspace will
  # use the local version of each package, and if it's unbuilt then any subsequent
  # build will error out due to missing files.
  yarn --frozen-lockfile
  local dirname
  dirname="$(basename "$dir")"
  if [[ "$dirname" == spl-* ]]; then
    yarn build:npm
  else
    yarn build
  fi

  if npm view "${name}@${version}" version >/dev/null 2>&1; then
    echo "The package $dir is already up to date, skipping"
    popd >/dev/null
    return 0
  fi

  local latest major_minor
  latest="$(npm view "$name" dist-tags.latest 2>/dev/null || true)"
  major_minor="$(node -p "require('./package.json').version.split('.').slice(0,2).join('.')")"

  local publish_args=()
  # If version looks like X.Y.Z-<something> (e.g. 1.0.0-rc.2), publish under dist-tag "next"
  if [[ "$version" =~ ^[0-9]+\.[0-9]+\.[0-9]+-.+ ]]; then
    publish_args+=(--tag next)
  # If we are releasing a backport, then we need to explicitly specify --tag as it can't be latest
  elif [[ -n "$latest" ]] && [[ "$version" != "$latest" ]] &&
     [[ "$(printf '%s\n%s\n' "$version" "$latest" | sort -V | tail -n1)" == "$latest" ]]; then
    publish_args+=(--tag "back-$major_minor")
  fi

  if [[ "${DRY_RUN:-false}" == "true" ]]; then
    echo "Publishing $dir (${name}@${version}) as a dry-run"
    npm publish "${publish_args[@]}" --dry-run
  else
    echo "Publishing $dir (${name}@${version})"
    npm publish "${publish_args[@]}" --provenance --access public
  fi

  popd >/dev/null
}

base="ts/packages"

publish "$base/borsh"
publish "$base/anchor-errors"
publish "$base/anchor"
#publish "$base/spl-associated-token-account"
#publish "$base/spl-binary-option"
#publish "$base/spl-binary-oracle-pair"
#publish "$base/spl-feature-proposal"
#publish "$base/spl-governance"
#publish "$base/spl-memo"
#publish "$base/spl-name-service"
#publish "$base/spl-record"
#publish "$base/spl-stake-pool"
#publish "$base/spl-stateless-asks"
publish "$base/spl-token"
#publish "$base/spl-token-lending"
#publish "$base/spl-token-swap"
