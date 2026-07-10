#!/usr/bin/env bash
set -euo pipefail

ORACLE_DIR=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
PATCH_FILE="$ORACLE_DIR/patches/v3.3.8-oracle.patch"
ORACLE_SOURCE="$ORACLE_DIR/src/com/shatteredpixel/shatteredpixeldungeon/ParityOracle.java"
PATCH_HASH=$(git hash-object "$PATCH_FILE")
SOURCE_HASH=$(git hash-object "$ORACLE_SOURCE")
BUILD_HASH=$(git hash-object "$ORACLE_DIR/build.sh")
STAGE="$ORACLE_DIR/.work/v3.3.8-${PATCH_HASH:0:12}-${SOURCE_HASH:0:12}-${BUILD_HASH:0:12}"

if [[ ! -f "$STAGE/core/src/main/java/com/shatteredpixel/shatteredpixeldungeon/ParityOracle.java" ]]; then
    "$ORACLE_DIR/build.sh" >&2
fi

GRADLE_ARGS=(--quiet)
if [[ "${ORACLE_OFFLINE:-0}" == "1" ]]; then
    GRADLE_ARGS+=(--offline)
fi

cd "$STAGE"
exec ./gradlew "${GRADLE_ARGS[@]}" :core:parityOracle --args="$*"
