#!/usr/bin/env bash
# Fresh query-specific PGO, AVX2, then the warmed Java/native comparison.
set -euo pipefail
cd "$(dirname "$0")/../.."
export CARGO_TARGET_DIR="$PWD/target/benchmark"
mkdir -p "$CARGO_TARGET_DIR"
RAW=$(mktemp -d "$CARGO_TARGET_DIR/four-trinkets-profile-raw.XXXXXX")
PROFILE="$CARGO_TARGET_DIR/four-trinkets.profdata"
PROFDATA="$(rustc --print target-libdir)/../bin/llvm-profdata"
if [[ ! -x "$PROFDATA" ]]; then
    echo 'Install the matching LLVM tools: rustup component add llvm-tools' >&2
    exit 1
fi
BUILD=(cargo build --locked --release -p shpd-seedfinder-ffi --example match_benchmark)
RUSTFLAGS="-C target-feature=+avx2 -C profile-generate=$RAW" "${BUILD[@]}"
python3 tooling/benchmarks/four_trinkets.py --train --output "$RAW"
"$PROFDATA" merge -o "$PROFILE" "$RAW"/*.profraw
RUSTFLAGS="-C target-feature=+avx2 -C profile-use=$PROFILE" "${BUILD[@]}"
bash tooling/java-finder/build.sh
exec python3 tooling/benchmarks/four_trinkets.py "$@"
