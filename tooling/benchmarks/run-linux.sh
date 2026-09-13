#!/usr/bin/env bash
# Build and run the Linux benchmark with fresh PGO.
set -euo pipefail
cd "$(dirname "$0")/../.."
rustup component add llvm-tools
export CARGO_TARGET_DIR="$PWD/target/benchmark"
mkdir -p "$CARGO_TARGET_DIR"
BENCH_RAW=$(mktemp -d "$CARGO_TARGET_DIR/profile-raw.XXXXXX")
BENCH_PROFILE="$CARGO_TARGET_DIR/benchmark.profdata"
BENCH_PROFDATA="$(rustc --print target-libdir)/../bin/llvm-profdata"
BENCH_BUILD=(cargo build --locked --release -p shpd-seedfinder-ffi --example match_benchmark)
RUSTFLAGS="-Cprofile-generate=$BENCH_RAW" "${BENCH_BUILD[@]}"
python3 tooling/benchmarks/effective_matches.py --train --workers 1 \
  --binary target/benchmark/release/examples/match_benchmark
"$BENCH_PROFDATA" merge -o "$BENCH_PROFILE" "$BENCH_RAW"/*.profraw
RUSTFLAGS="-Cprofile-use=$BENCH_PROFILE" "${BENCH_BUILD[@]}"
bash tooling/java-finder/build.sh
exec python3 tooling/benchmarks/effective_matches.py \
  --binary target/benchmark/release/examples/match_benchmark "$@"
