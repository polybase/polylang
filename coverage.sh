#!/usr/bin/env bash

set -uo pipefail

cargo build --release --bin compile 2>/dev/null || { echo "Failed to build compile" && exit 1; }
# cargo build --release --bin miden-run 2>/dev/null || { echo "Failed to build miden-run" && exit 1; }

for file in ./test-collections/*; do
    echo "Processing file: $file" >&2
    code=$(cat "$file")

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
                ./target/release/compile collection:"$current_collection" function:"$name" <<<"$code"
                if [ $? -ne 0 ]; then
                    echo "Failure"
                else
                    echo "Success"
                fi
                ;;
        esac
    done
done | {
    success_count=0
    failure_count=0
    while read -r line; do
        case "$line" in
            Success)
                success_count=$((success_count + 1))
                ;;
            Failure)
                failure_count=$((failure_count + 1))
                ;;
        esac

        echo "Successes: $success_count"
        echo "Failures:  $failure_count"
    done
}
