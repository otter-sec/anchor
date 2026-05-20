#!/bin/bash

################################################################################
#
# A script to run the example as an integration test. It starts up a localnet
# and executes the current directory's rust binary.
#
# Usage:
#
# ./run.sh
#
# Run this script from within the `example/` directory in which it is located.
# The anchor cli must be installed.
#
# cargo install --git https://github.com/otter-sec/anchor anchor-cli --locked
#
################################################################################

set -euox pipefail

main() {
    #
    # Build programs.
    #
    local composite_pid="EHthziFziNoac9LBGxEaVN47Y3uUiRoXvqAiR6oes4iU"
    local basic_2_pid="Fg6PaFpoGXkYsidMpWTK6W2BeZ7FEfcYkg476zPFsLnS"
    local basic_4_pid="CwrqeMj2U8tFr1Rhkgwc84tpAsqbt9pTt2a4taoTADPr"
    local events_pid="2dhGsWUzy5YKUsjZdLHLmkNpUDAXkNa9MYWsPc4Ziqzy"
    local optional_pid="FNqz6pqLAwvMSds2FYjR4nKV3moVpPNtvkfGFrqLKrgG"

    cd ../../tests/composite && anchor build --skip-lint --ignore-keys && cd -
    [ $? -ne 0 ] && exit 1
    cd ../../examples/tutorial/basic-2 && anchor build --skip-lint --ignore-keys && cd -
    [ $? -ne 0 ] && exit 1
    cd ../../examples/tutorial/basic-4 && anchor build --skip-lint --ignore-keys && cd -
    [ $? -ne 0 ] && exit 1
    cd ../../tests/events && anchor build --skip-lint --ignore-keys && cd -
    [ $? -ne 0 ] && exit 1
    cd ../../tests/optional && anchor build --skip-lint --ignore-keys && cd -
    [ $? -ne 0 ] && exit 1

    #
    # Bootup validator.
    #
    surfpool_pid=$(start_surfpool)

    #
    # Run single threaded test.
    #
    cargo run -- \
        --composite-pid $composite_pid \
        --basic-2-pid $basic_2_pid \
        --basic-4-pid $basic_4_pid \
        --events-pid $events_pid \
        --optional-pid $optional_pid

    #
    # Restart validator for multithreaded test
    #
    cleanup "$surfpool_pid"
    surfpool_pid=$(start_surfpool)

    #
    # Run multi threaded test.
    #
    cargo run -- \
        --composite-pid $composite_pid \
        --basic-2-pid $basic_2_pid \
        --basic-4-pid $basic_4_pid \
        --events-pid $events_pid \
        --optional-pid $optional_pid \
        --multithreaded

    #
    # Restart validator for async test
    #
    cleanup "$surfpool_pid"
    surfpool_pid=$(start_surfpool)

    #
    # Run async test.
    #
    cargo run --features async -- \
        --composite-pid $composite_pid \
        --basic-2-pid $basic_2_pid \
        --basic-4-pid $basic_4_pid \
        --events-pid $events_pid \
        --optional-pid $optional_pid \
        --multithreaded

}

cleanup() {
    local pid="${1:-}"

    if [ -n "$pid" ]; then
        echo "Attempting to kill tracked surfpool process PID: $pid"
        kill "$pid" 2>/dev/null || true
        sleep 1
        kill -9 "$pid" 2>/dev/null || true
    fi

    echo "Ensuring all surfpool processes are stopped..."
    pkill -9 -f 'surfpool' 2>/dev/null || true

    # Clean up any sub-processes from this script
    pkill -P $$ 2>/dev/null || true
    wait || true
}


trap_add() {
    trap_add_cmd=$1; shift || fatal "${FUNCNAME} usage error"
    for trap_add_name in "$@"; do
        trap -- "$(
            extract_trap_cmd() { printf '%s\n' "${3:-}"; }
            eval "extract_trap_cmd $(trap -p "${trap_add_name}")"
            printf '%s\n' "${trap_add_cmd}"
        )" "${trap_add_name}" \
            || fatal "unable to add to trap ${trap_add_name}"
    done
}

check_surfpool() {
    local pid=$1
    echo "Checking surfpool with PID: $pid"
    exit_state=$(kill -0 "$pid" && echo 'living' || echo 'exited')
    if [ "$exit_state" == 'exited' ]; then
        echo "Cannot start surfpool"
        exit 1
    fi
}

start_surfpool() {
    surfpool start --ci --offline --daemon &>/dev/null &
    local surfpool_pid=$!

    sleep 3

    # Send setup output to stderr, not stdout
    surfpool run setup -u \
        --input composite_pid=$composite_pid \
        --input basic_2_pid=$basic_2_pid \
        --input basic_4_pid=$basic_4_pid \
        --input events_pid=$events_pid \
        --input optional_pid=$optional_pid \
        >&2

    sleep 3

    echo "Surfpool PID: $surfpool_pid" >&2

    echo "$surfpool_pid"
}

declare -f -t trap_add
trap 'cleanup' EXIT
main
