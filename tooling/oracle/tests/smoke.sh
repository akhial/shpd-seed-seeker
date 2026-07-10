#!/usr/bin/env bash
set -euo pipefail

ORACLE_DIR=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
FIRST=$(mktemp "${TMPDIR:-/tmp}/shpd-oracle-first.XXXXXX")
SECOND=$(mktemp "${TMPDIR:-/tmp}/shpd-oracle-second.XXXXXX")
JSON_OUTPUT=$(mktemp "${TMPDIR:-/tmp}/shpd-oracle-json.XXXXXX")
trap 'rm -f "$FIRST" "$SECOND" "$JSON_OUTPUT"' EXIT

"$ORACLE_DIR/run.sh" --seed AAA-AAA-AAA --floors 1 --format ndjson --no-phases >"$FIRST"
"$ORACLE_DIR/run.sh" --seed AAA-AAA-AAA --floors 1 --format ndjson --no-phases >"$SECOND"
cmp "$FIRST" "$SECOND"

"$ORACLE_DIR/run.sh" --seed AAA-AAA-AAA --floors 1 --format json --no-phases >"$JSON_OUTPUT"
python3 "$ORACLE_DIR/tests/assert_smoke.py" \
    "$ORACLE_DIR/tests/aaa-aaa-aaa-floor1.expected.json" \
    "$FIRST" "$JSON_OUTPUT"

echo "AAA-AAA-AAA floor 1 oracle smoke test passed"

