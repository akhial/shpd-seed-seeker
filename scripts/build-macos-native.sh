#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
PROFILE="$ROOT/pgo/seed-seeker-aarch64-apple-darwin.profdata"

# The checked-in profile is recorded by scripts/record-pgo-profile.sh and
# improves the seed search on top of the source-level optimizations. Resolve it
# here because rustc evaluates profile-use paths from each dependency's source
# directory, not consistently from the Cargo workspace.
#
# The profile only applies if its mangled symbol names match the ones this
# command produces, and rustc says nothing when they do not, so the shape of
# this invocation — the target, the profile, the package set — is load bearing.
RUSTFLAGS="${RUSTFLAGS:+$RUSTFLAGS }-Cprofile-use=$PROFILE" \
    cargo build --locked --release --target aarch64-apple-darwin \
        -p shpd-seedfinder-ffi --manifest-path "$ROOT/Cargo.toml"

# SwiftPM does not track the linked Rust archive; expose its hash as a C input.
ENGINE_HASH=$(shasum -a 256 "$ROOT/target/aarch64-apple-darwin/release/libshpd_seedfinder_ffi.a" | cut -d ' ' -f 1)
printf '#define SEEDSEEKER_ENGINE_ARCHIVE_SHA256 "%s"\n' "$ENGINE_HASH" \
    > "$ROOT/macos/SeedSeeker/Sources/CSeedFinder/engine_revision.h"
