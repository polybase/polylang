#!/usr/bin/env bash

set -uo pipefail

cargo build --release --bin compile 2>/dev/null || { echo "Failed to build compile" && exit 1; }
# cargo build --release --bin miden-run 2>/dev/null || { echo "Failed to build miden-run" && exit 1; }

declare -A specific_error_counter=()
success_count=0
failure_count=0

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
                output=$(./target/release/compile collection:"$current_collection" function:"$name" <<<"$code" 2>&1)
                if [ $? -ne 0 ]; then
                    if [ -z "${specific_error_counter["$output"]+x}" ]; then
                        specific_error_counter["$output"]=0
                    fi
                    specific_error_counter["$output"]=$((specific_error_counter["$output"] + 1))
                    failure_count=$((failure_count + 1))
                else
                    success_count=$((success_count + 1))
                fi
                ;;
        esac
    done
done

echo "Successes: $success_count"
echo "Failures:  $failure_count"

echo "Top errors:"
for error in "${!specific_error_counter[@]}"; do
    echo "  ${specific_error_counter["$error"]} - $error"
done | sort -nr
