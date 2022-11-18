#!/usr/bin/env bash

set -uo pipefail

cargo build --bin compile 2>/dev/null || { echo "Failed to build compile" && exit 1; }
cargo build --release --bin miden-run 2>/dev/null || { echo "Failed to build miden-run" && exit 1; }

code="$(cat)"
test_functions="$(echo "$code" | grep -E '^function test')"
test_functions="$(echo "$test_functions" | sed -E 's/function ([a-zA-Z0-9_]+).*/\1/')"

failures=0

for f in $test_functions; do
    echo "$(tput bold)Running $f$(tput sgr0)"
    miden="$(./target/debug/compile function:"$f" <<<"$code")"
    ./target/release/miden-run <<<"$miden"
    exit_code="$?"
    if [[ "$f" == *"ShouldError"* ]] && [[ $exit_code -eq 0 ]]; then
        echo "$(tput bold)$(tput setaf 1)Test $f should have errored but didn't$(tput sgr0)"
        failures=$((failures + 1))
    elif [[ "$f" != *"ShouldError"* ]] && [[ $exit_code -ne 0 ]]; then
        echo "$(tput bold)$(tput setaf 1)Test $f didn't pass$(tput sgr0)"
        failures=$((failures + 1))
    else
        echo "$(tput bold)$(tput setaf 2)Test $f passed$(tput sgr0)"
    fi
    echo
done

if [[ $failures -eq 0 ]]; then
    echo "$(tput bold)$(tput setaf 2)All tests passed$(tput sgr0)"
else
    echo "$(tput bold)$(tput setaf 1)$failures tests failed$(tput sgr0)"
fi
