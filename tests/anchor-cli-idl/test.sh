#!/bin/bash

set -euo pipefail

# FIXME: For some reason, in CI, executing with NPX results in `sh: program-metadata: not found`
# Installing this globally fixes this in CI, but this should be investigated and fixed properly
# Implementing IDL fetching via Rust client will make this redundant
npm install --global @solana-program/program-metadata@0.5.1
DEPLOYER_KEYPAIR="keypairs/deployer-keypair.json"
PROGRAM_ONE="2uA3amp95zsEHUpo8qnLMhcFAUsiKVEcKHXS1JetFjU5"

# Write a keypair for program deploy
mkdir -p target/deploy
cp keypairs/idl_commands_one-keypair.json target/deploy
# Generate over 20kb bytes of random data (base64 encoded), surround it with quotes, and store it in a variable
RANDOM_DATA=$(openssl rand -base64 $((10*1680)) | sed 's/.*/"&",/')

# Create the JSON object with the "docs" field containing random data
echo '{
  "address": "2uA3amp95zsEHUpo8qnLMhcFAUsiKVEcKHXS1JetFjU5",
  "metadata": {
    "name": "idl_commands_one",
    "version": "0.1.0",
    "spec": "0.1.0"
  },
  "instructions": [
    {
      "name": "initialize",
      "docs" : [
        '"$RANDOM_DATA"'
        "trailing comma begone"
      ],
      "discriminator": [],
      "accounts": [],
      "args": []
    }
  ]
}' > testLargeIdl.json

# Dump the Program Metadata Program from mainnet for local testing
PMP_SO="target/deploy/program_metadata.so"
if [ ! -f "$PMP_SO" ]; then
  echo "Dumping Program Metadata Program from mainnet"
  solana program dump ProgM6JCCvbYkfKqJYHePx4xxSUSqJp7rh8Lyv7nk7S "$PMP_SO" --url https://api.mainnet-beta.solana.com
fi

echo "Building programs"

anchor build --ignore-keys

echo "Starting local validator for test"

solana-test-validator --reset \
  -q \
  --mint tgyXxAhCkpgtKCEi4W6xWJSzqwVGs3uk2RodbZP2J49 \
  --bpf-program ProgM6JCCvbYkfKqJYHePx4xxSUSqJp7rh8Lyv7nk7S "$PMP_SO" \
  --upgradeable-program 2uA3amp95zsEHUpo8qnLMhcFAUsiKVEcKHXS1JetFjU5 target/deploy/idl_commands_one.so tgyXxAhCkpgtKCEi4W6xWJSzqwVGs3uk2RodbZP2J49 \
  --upgradeable-program DE4UbHnAcT6Kfh1fVTPRPwpiA3vipmQ4xR3gcLwX3wwS target/deploy/idl_commands_two.so tgyXxAhCkpgtKCEi4W6xWJSzqwVGs3uk2RodbZP2J49 \
  &

sleep 10

echo "Running tests"

anchor test --skip-deploy --skip-local-validator
kill $(jobs -p)
