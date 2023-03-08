#!/usr/bin/env bash

set -euo pipefail

mkdir -p test-collections

collections="$(curl -L 'https://testnet.polybase.xyz/v0/collections/Collection/records?limit=1000' | jq -r '.data | .[].data | @base64')"

for collection in $collections; do
    collection="$(echo "$collection" | base64 -d)"
    id="$(echo "$collection" | jq -r '.id')"
    code="$(echo "$collection" | jq -r '.code')"

    echo "$code" > "test-collections/$(sed -e 's/[]\/$*.^[]/-/g' <<< "$id")"
done
