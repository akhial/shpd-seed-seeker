#!/bin/sh
# SPDX-License-Identifier: GPL-3.0-or-later
#
# Builds the JNI library for the *host* so the Android JVM unit tests can load
# the real engine for query validation, recipe filtering, and bridge tests.
# Nothing here goes into an APK — scripts/build-android-native.sh does that.
set -eu

ROOT=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
OUTPUT=${1:-"$ROOT/android/app/build/generated/hostJni"}

case "$(uname -s)" in
    Darwin) LIBRARY=libshpd_seedfinder.dylib ;;
    Linux) LIBRARY=libshpd_seedfinder.so ;;
    *) echo "Unsupported host for the JNI unit-test library" >&2; exit 1 ;;
esac

cd "$ROOT"
# Use the dev profile for host-side unit and bridge tests.
cargo build --locked -p shpd-seedfinder-jni

TARGET_DIR=${CARGO_TARGET_DIR:-$ROOT/target}
mkdir -p "$OUTPUT"
cp "$TARGET_DIR/debug/$LIBRARY" "$OUTPUT/$LIBRARY"
