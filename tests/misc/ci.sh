#!/bin/sh

# this script ensures that the Misc test does not
# test the miscNonRentExempt.ts during its test in the ci
# because the misc test uses a newer solana version
# than the miscNonRentExempt one. The latter needs to be on
# a validator with a version < 1.9, so it can test
# whether anchor's rent-exemption checks work for
# legacy accounts which don't have to be rent-exempt
rm ./tests/misc/misc.ts
mv miscNonRentExempt.ts ./tests/misc/miscNonRentExempt.ts
