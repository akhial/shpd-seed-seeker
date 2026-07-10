#!/usr/bin/env bash
set -euo pipefail

ORACLE_DIR=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
WORKSPACE=$(cd "$ORACLE_DIR/../.." && pwd)
PIN_TAG=v3.3.8
PIN_COMMIT=7b8b845a76fe76c6b7c031ae9e570852411f56db
OFFICIAL_URL=https://github.com/00-Evan/shattered-pixel-dungeon.git
PATCH_FILE="$ORACLE_DIR/patches/v3.3.8-oracle.patch"
ORACLE_SOURCE="$ORACLE_DIR/src/com/shatteredpixel/shatteredpixeldungeon/ParityOracle.java"

PATCH_HASH=$(git hash-object "$PATCH_FILE")
SOURCE_HASH=$(git hash-object "$ORACLE_SOURCE")
BUILD_HASH=$(git hash-object "$ORACLE_DIR/build.sh")
REVISION=${PATCH_HASH:0:12}-${SOURCE_HASH:0:12}-${BUILD_HASH:0:12}
STAGE="$ORACLE_DIR/.work/$PIN_TAG-$REVISION"
LOCAL_SOURCE=${SHPD_SOURCE:-"$WORKSPACE/upstream/shattered-pixel-dungeon"}
CACHE_SOURCE="$ORACLE_DIR/.work/official-$PIN_TAG"

if [[ ! -d "$LOCAL_SOURCE/.git" ]]; then
    mkdir -p "$ORACLE_DIR/.work"
    if [[ ! -d "$CACHE_SOURCE/.git" ]]; then
        git clone --branch "$PIN_TAG" --depth 1 "$OFFICIAL_URL" "$CACHE_SOURCE"
    fi
    LOCAL_SOURCE=$CACHE_SOURCE
fi

ACTUAL_COMMIT=$(git -C "$LOCAL_SOURCE" rev-parse "$PIN_TAG^{commit}")
if [[ "$ACTUAL_COMMIT" != "$PIN_COMMIT" ]]; then
    echo "parity-oracle: $PIN_TAG resolved to $ACTUAL_COMMIT, expected $PIN_COMMIT" >&2
    exit 1
fi

if [[ ! -f "$STAGE/core/src/main/java/com/shatteredpixel/shatteredpixeldungeon/ParityOracle.java" ]]; then
    mkdir -p "$STAGE"
    git -C "$LOCAL_SOURCE" archive "$PIN_COMMIT" | tar -x -C "$STAGE"
    patch -d "$STAGE" -p1 -i "$PATCH_FILE"
    mkdir -p "$STAGE/core/src/main/java/com/shatteredpixel/shatteredpixeldungeon"
    cp "$ORACLE_SOURCE" "$STAGE/core/src/main/java/com/shatteredpixel/shatteredpixeldungeon/ParityOracle.java"
    chmod +x "$STAGE/gradlew"
fi

GRADLE_ARGS=()
if [[ "${ORACLE_OFFLINE:-0}" == "1" ]]; then
    GRADLE_ARGS+=(--offline)
fi

cd "$STAGE"
./gradlew "${GRADLE_ARGS[@]}" :core:classes

echo "$STAGE"
