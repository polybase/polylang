#!/usr/bin/env bash

set -uo pipefail

cargo build --bin compile 2>/dev/null || { echo "Failed to build compile" && exit 1; }
# cargo build --release --bin miden-run 2>/dev/null || { echo "Failed to build miden-run" && exit 1; }

code="$(cat)"

collection_or_function=$(printf '%s' "$code" | grep -E '^ *(collection|function)' | awk '{ print $1" "$2 }' | sed 's/[(|{|}|)]/ /g' | awk '{ print $1","$2 }')
current_collection=""
for part in $collection_or_function; do
    type=$(printf '%s' "$part" | cut -d',' -f1)
    name=$(printf '%s' "$part" | cut -d',' -f2)

    case "$type" in
        collection)
            current_collection="$name"
            ;;
        function)
            echo "$(tput bold)Compiling $name$(tput sgr0)"
            miden="$(./target/debug/compile collection:"$current_collection" function:"$name" <<<"$code")"
            ;;
    esac
done
