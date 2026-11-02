#!/bin/bash

active_version=$(solana -V | awk '{print $2}')
if [ "$active_version" != "4.1.2" ]; then
  agave-install init 4.1.2
fi

git submodule update --init --recursive --depth 1
cd ts/packages/borsh && yarn --frozen-lockfile && yarn build && yarn link --force && cd ../../../
cd ts/packages/anchor-errors && yarn --frozen-lockfile && yarn build && yarn link --force && cd ../../../
cd ts/packages/anchor && yarn --frozen-lockfile && yarn build:node && yarn link && cd ../../../
cd ts/packages/spl-associated-token-account && yarn --frozen-lockfile && yarn build:node && yarn link && cd ../../../
cd ts/packages/spl-token && yarn --frozen-lockfile && yarn build:node && yarn link && cd ../../../
cd examples/tutorial && yarn link @anchor-lang/core @anchor-lang/borsh && yarn --frozen-lockfile && cd ../../
cd tests && yarn link @anchor-lang/core @anchor-lang/borsh @anchor-lang/spl-associated-token-account @anchor-lang/spl-token && yarn --frozen-lockfile && cd ..
cargo install --path cli anchor-cli --locked --force --debug
