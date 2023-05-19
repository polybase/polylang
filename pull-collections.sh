#!/usr/bin/env bash

set -euo pipefail

mkdir -p test-collections

collections="$(curl -L 'https://testnet.polybase.xyz/v0/collections/Collection/records?limit=1000' | jq -r '.data | .[].data | @base64')"

if [[ "$OSTYPE" == "darwin"* ]]
then
  DECODE_CMD="base64 --decode"
else
  DECODE_CMD="base64 -d"
fi

for collection in $collections; do
  collection="$(echo "$collection" | ${DECODE_CMD})"
    id="$(echo "$collection" | jq -r '.id')"
    code="$(echo "$collection" | jq -r '.code')"

    echo "$code" > "test-collections/$(sed -e 's/[]\/$*.^[]/-/g' <<< "$id")"
done
