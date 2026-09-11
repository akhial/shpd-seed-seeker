#!/usr/bin/env bash
# Build the macOS arm64 benchmark with the native allocator, fat LTO and fresh PGO.
set -euo pipefail
cd "$(dirname "$0")/../.."
ROOT="$PWD"
export CARGO_TARGET_DIR="$ROOT/target/benchmark"
RAW="$CARGO_TARGET_DIR/profile-raw"
PROFILE="$CARGO_TARGET_DIR/benchmark.profdata"
PROFDATA="$(rustc --print target-libdir)/../bin/llvm-profdata"
if [[ ! -x "$PROFDATA" ]]; then
    echo 'Install the matching LLVM tools: rustup component add llvm-tools' >&2
    exit 1
fi
mkdir -p "$RAW"
rm -f "$RAW"/*.profraw
BUILD=(cargo build --locked --release --target aarch64-apple-darwin -p shpd-seedfinder-ffi --example match_benchmark)
RUSTFLAGS="-Cprofile-generate=$RAW" "${BUILD[@]}"
python3 tooling/benchmarks/effective_matches.py --train --workers 1
"$PROFDATA" merge -o "$PROFILE" "$RAW"/*.profraw
RUSTFLAGS="-Cprofile-use=$PROFILE" "${BUILD[@]}"
bash tooling/java-finder/build.sh
exec python3 tooling/benchmarks/effective_matches.py "$@"
