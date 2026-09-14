#!/bin/sh
# SPDX-License-Identifier: GPL-3.0-or-later
# Reject stale native libraries before packaging an APK.
set -eu
ROOT=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
NM=$1
LIBRARY=$2
SYMBOLS=$("$NM" --dynamic --defined-only --extern-only --format=posix "$LIBRARY" | awk '{print $1}')
METHODS=$(sed -n 's/.*external fun \([a-zA-Z0-9_]*\)(.*/\1/p' \
    "$ROOT/android/app/src/main/java/dev/seedseeker/app/engine/NativeSeedFinder.kt")
for METHOD in $METHODS; do
    SYMBOL="Java_dev_seedseeker_app_engine_JniBindings_$METHOD"
    if ! printf '%s\n' "$SYMBOLS" | grep -Fxq "$SYMBOL"; then
        echo "Missing JNI export $SYMBOL in $LIBRARY" >&2
        exit 1
    fi
done
