#!/usr/bin/env bash
set -euo pipefail

ORACLE_DIR=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
EXPECTED="$ORACLE_DIR/tests/prison-floors.expected.json"

if [[ -n "${JAVA_21_HOME:-}" ]]; then
    export JAVA_HOME="$JAVA_21_HOME"
elif [[ -x /usr/libexec/java_home ]]; then
    export JAVA_HOME=$(/usr/libexec/java_home -v 21)
elif [[ -z "${JAVA_HOME:-}" ]]; then
    echo "prison fixture requires Temurin 21.0.11; set JAVA_21_HOME" >&2
    exit 1
fi

AAA=$(mktemp "${TMPDIR:-/tmp}/shpd-prison-aaa.XXXXXX")
ONE=$(mktemp "${TMPDIR:-/tmp}/shpd-prison-one.XXXXXX")
ABC=$(mktemp "${TMPDIR:-/tmp}/shpd-prison-abc.XXXXXX")
MAX=$(mktemp "${TMPDIR:-/tmp}/shpd-prison-max.XXXXXX")
trap 'rm -f "$AAA" "$ONE" "$ABC" "$MAX"' EXIT

"$ORACLE_DIR/run.sh" --seed AAA-AAA-AAA --floors 6-9 \
    --format json --run-checkpoints >"$AAA"
"$ORACLE_DIR/run.sh" --seed AAA-AAA-AAB --floors 6 \
    --format json --no-phases --run-checkpoints >"$ONE"
"$ORACLE_DIR/run.sh" --seed ABC-DEF-GHI --floors 6 \
    --format json --no-phases --run-checkpoints >"$ABC"
"$ORACLE_DIR/run.sh" --seed ZZZ-ZZZ-ZZZ --floors 6 \
    --format json --no-phases --run-checkpoints >"$MAX"

MODE="$EXPECTED"
if [[ "${1:-}" == "--print" ]]; then
    MODE=--print
fi

python3 "$ORACLE_DIR/tests/assert_prison.py" "$MODE" \
    "AAA-AAA-AAA=$AAA" \
    "AAA-AAA-AAB=$ONE" \
    "ABC-DEF-GHI=$ABC" \
    "ZZZ-ZZZ-ZZZ=$MAX"

if [[ "$MODE" != "--print" ]]; then
    echo "Prison floors 6-9 official parity fixtures passed"
fi
