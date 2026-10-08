#!/bin/bash

set -euo pipefail

# FIXME: For some reason, in CI, executing with NPX results in `sh: program-metadata: not found`
# Installing this globally fixes this in CI, but this should be investigated and fixed properly
# Implementing IDL fetching via Rust client will make this redundant
npm install --global @solana-program/program-metadata@0.5.1
DEPLOYER_KEYPAIR="keypairs/deployer-keypair.json"
PROGRAM_ONE="2uA3amp95zsEHUpo8qnLMhcFAUsiKVEcKHXS1JetFjU5"
CUSTOM_AUTHORITY="target/deploy/security-metadata-authority.json"
FETCHED_SECURITY="target/security-metadata.json"

cleanup() {
  if [ -n "${VALIDATOR_PID:-}" ]; then
    kill "$VALIDATOR_PID" 2>/dev/null || true
    wait "$VALIDATOR_PID" 2>/dev/null || true
  fi
  rm -f security.json "$FETCHED_SECURITY" target/security-metadata-valid.json
}
trap cleanup EXIT

write_security_metadata() {
  local version="$1"
  cat > security.json <<EOF
{
  "name": "idl_commands_one",
  "description": "Validator integration test revision ${version}",
  "contacts": ["email:security@example.com"],
  "version": "${version}"
}
EOF
}

fetch_security_metadata() {
  local expected_version="$1"
  program-metadata --rpc http://localhost:8899 \
    fetch security "$PROGRAM_ONE" --output "$FETCHED_SECURITY"
  jq -e --arg version "$expected_version" '.version == $version' "$FETCHED_SECURITY" >/dev/null
}

last_deployed_slot() {
  solana program show "$PROGRAM_ONE" --url http://localhost:8899 |
    awk '/Last Deployed In Slot:/ { print $5 }'
}

assert_deploy_preflight_failure() {
  local before_slot after_slot
  before_slot=$(last_deployed_slot)
  if anchor program deploy --program-name idl_commands_one --security-metadata --no-idl --use-rpc; then
    echo "Expected security metadata preflight to fail"
    exit 1
  fi
  after_slot=$(last_deployed_slot)
  test "$before_slot" = "$after_slot"
}

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
VALIDATOR_PID=$!

sleep 10

echo "Running tests"

anchor test --skip-deploy --skip-local-validator

echo "Testing security metadata deployment"

# Create canonical security metadata with the default wallet acting as both
# upgrade authority and payer, then update the same PDA in place.
write_security_metadata "0.1.0"
anchor program deploy --program-name idl_commands_one --security-metadata --no-idl --use-rpc
fetch_security_metadata "0.1.0"

write_security_metadata "0.2.0"
anchor program deploy --program-name idl_commands_one --security-metadata --no-idl --use-rpc
fetch_security_metadata "0.2.0"

# Missing and malformed metadata must fail before the program is upgraded.
mv security.json target/security-metadata-valid.json
assert_deploy_preflight_failure
printf '{"name":' > security.json
assert_deploy_preflight_failure
mv target/security-metadata-valid.json security.json

# A separate upgrade authority must sign buffer writes while the configured
# wallet continues to pay deployment and metadata account fees.
solana-keygen new --no-bip39-passphrase --silent --force --outfile "$CUSTOM_AUTHORITY"
solana program set-upgrade-authority "$PROGRAM_ONE" \
  --new-upgrade-authority "$CUSTOM_AUTHORITY" \
  --keypair "$DEPLOYER_KEYPAIR" \
  --url http://localhost:8899

write_security_metadata "0.3.0"
anchor program deploy --program-name idl_commands_one \
  --upgrade-authority "$CUSTOM_AUTHORITY" \
  --security-metadata --no-idl --use-rpc
fetch_security_metadata "0.3.0"

# Security metadata is uploaded before finalization removes the program's
# upgrade authority.
write_security_metadata "1.0.0"
anchor program deploy --program-name idl_commands_one \
  --upgrade-authority "$CUSTOM_AUTHORITY" \
  --security-metadata --no-idl --use-rpc --final
fetch_security_metadata "1.0.0"
solana program show "$PROGRAM_ONE" --url http://localhost:8899 | grep -q 'Authority: none'
