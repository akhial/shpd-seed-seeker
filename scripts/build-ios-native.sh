#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
PLATFORM="${1:-${PLATFORM_NAME:-iphonesimulator}}"
case "$PLATFORM" in
    simulator|iphonesimulator) RUST_TARGET=aarch64-apple-ios-sim ;;
    device|iphoneos) RUST_TARGET=aarch64-apple-ios ;;
    *) echo "Usage: $0 [simulator|device]" >&2; exit 2 ;;
esac

# Xcode's build-service environment does not inherit interactive shell paths.
export PATH="$HOME/.cargo/bin:/opt/homebrew/bin:$PATH"
export IPHONEOS_DEPLOYMENT_TARGET=27.0
if ! rustup target list --installed | grep -qx "$RUST_TARGET"; then
    echo "Install the iOS Rust target first: rustup target add $RUST_TARGET" >&2
    exit 1
fi

# The macOS PGO profile contains platform-specific symbols. Build iOS with the
# workspace's release LTO/optimization settings and its real C ABI engine.
cargo build --locked --release --target "$RUST_TARGET" \
    -p shpd-seedfinder-ffi --manifest-path "$ROOT/Cargo.toml"

# The shared C shim gives Xcode a changed source dependency when Rust changes.
# Keep generated headers separate from SwiftPM's macOS build products.
GENERATED="$ROOT/target/ios/generated/$RUST_TARGET"
mkdir -p "$GENERATED"
ENGINE_HASH=$(shasum -a 256 "$ROOT/target/$RUST_TARGET/release/libshpd_seedfinder_ffi.a" | cut -d ' ' -f 1)
printf '#define SEEDSEEKER_ENGINE_ARCHIVE_SHA256 "%s"\n' "$ENGINE_HASH" > "$GENERATED/ios_engine_revision.h.tmp"
if ! cmp -s "$GENERATED/ios_engine_revision.h.tmp" "$GENERATED/ios_engine_revision.h"; then
    mv "$GENERATED/ios_engine_revision.h.tmp" "$GENERATED/ios_engine_revision.h"
else
    rm "$GENERATED/ios_engine_revision.h.tmp"
fi
