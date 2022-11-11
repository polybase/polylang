#!/usr/bin/env bash

set -euo pipefail

cargo build --bin compile 2>/dev/null
cargo build --bin miden-run 2>/dev/null

code="$(cat)"
test_functions="$(echo "$code" | grep -E '^function test')"
test_functions="$(echo "$test_functions" | sed -E 's/function ([a-zA-Z0-9_]+).*/\1/')"

for f in $test_functions; do
    echo "$(tput bold)Running $f$(tput sgr0)"
    miden="$(./target/debug/compile function:"$f" <<<"$code")"
    ./target/debug/miden-run <<<"$miden"
    echo
done

