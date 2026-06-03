#!/usr/bin/env bash

set -euo pipefail

source scripts/common.sh

DEX_PID="srmqPvymJeFKQ4zGQed1GFppgkRHL9kaELCbyksJtPX"
PAYER_FILEPATH="$HOME/.config/solana/id.json"
CRANK="/home/armaniferrante/Documents/code/src/github.com/project-serum/serum-dex/target/debug/crank"
VALIDATOR_OUT="./validator-stdout.txt"
CRANK_LOGS="crank-logs.txt"
CRANK_STDOUT="crank-stdout.txt"
TRADE_BOT_STDOUT="trade-bot-stdout.txt"
FEES_STDOUT="fees.txt"

main () {
		echo "Cleaning old output files..."
		rm -rf test-ledger
		rm -f $TRADE_BOT_STDOUT
		rm -f $FEES_STDOUT
		rm -f $VALIDATOR_OUT
		rm -f $CRANK_LOGS && touch $CRANK_LOGS

		echo "Starting local network..."
		solana-test-validator \
				--bpf-program srmqPvymJeFKQ4zGQed1GFppgkRHL9kaELCbyksJtPX ./deps/openbook-dex/dex/target/deploy/serum_dex.so \
				--bpf-program 22Y43yTVxuUkoRKdm9thyRhQ3SdgQS7c7kB6UNCiaczD ./deps/swap/target/deploy/swap.so \
				--bpf-program GrAkKfEpTKQuVHG2Y97Y2FF4i7y7Q5AHLK94JBy7Y5yv ./deps/stake/target/deploy/registry.so \
				--bpf-program 6ebQNeTPZ1j7k3TtkCCtEPRvG7GQsucQrZ7sSEDQi9Ks ./deps/stake/target/deploy/lockup.so \
				--bpf-program 5CHQcwNhkFiFXXM8HakHi8cB7AKP3M3GPdEBDeRJBWQq ./target/deploy/cfo.so > $VALIDATOR_OUT &
		sleep 2

		echo "Listing market..."
		market=$(./scripts/list-market.js | jq -r .market)
		sleep 2
		echo "Market listed $market"

		echo "Running crank..."
		$CRANK localnet consume-events \
					-c $market \
					-d $DEX_PID -e 5 \
					--log-directory $CRANK_LOGS \
					--market $market \
					--num-workers 1 \
					--payer $PAYER_FILEPATH \
					--pc-wallet $market > $CRANK_STDOUT &
		echo "Running trade bot..."
		./scripts/trade-bot.js $market > $TRADE_BOT_STDOUT &

		echo "Running fees listener..."
		./scripts/fees.js $market > $FEES_STDOUT &

		echo "Localnet running..."
		echo "Ctl-c to exit."
		wait
}

main
