#!/usr/bin/env bash

set -euo pipefail

mkdir -p test-collections

limit=1000
cursor_after=""
while true; do
    echo "After: $cursor_after"
    result="$(curl -L "https://testnet.polybase.xyz/v0/collections/Collection/records?limit=$limit&after=$cursor_after")"
    cursor_after="$(printf "%s" "$result" | jq -r '.cursor.after')"
    # URL encode the cursor for the next request
    cursor_after="$(python3 -c "import urllib.parse; print(urllib.parse.quote('''$cursor_after'''))")"
    collections="$(printf "%s" "$result" | jq -r '.data | .[].data | @base64')"
    collections_lines="$(printf "%s" "$collections" | wc -l)"

    for collection in $collections; do
        collection="$(echo "$collection" | base64 -d)"
        id="$(echo "$collection" | jq -r '.id')"
        code="$(echo "$collection" | jq -r '.code')"
        filename="$(sed -e 's/[]\/$*.^[]/-/g' <<< "$id")"

        # skip file names that are too long
        if [ "${#filename}" -gt 255 ]; then
            continue
        fi

        echo "$code" > "test-collections/$filename"
    done

    if [ "$collections_lines" -eq 0 ]; then
        break
    fi
done
