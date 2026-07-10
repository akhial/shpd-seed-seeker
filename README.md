# Seed Seeker for Shattered Pixel Dungeon

An unofficial, offline seed finder for Shattered Pixel Dungeon. The engine is a
Rust reimplementation of the game's deterministic generation path and is called
from a standalone Android UI through JNI.

The compatibility target is intentionally pinned:

- Shattered Pixel Dungeon **v3.3.8**
- upstream commit `7b8b845a76fe76c6b7c031ae9e570852411f56db`
- new custom-seed dungeon, main branch, Warrior, no challenges, no equipped
  trinket, no bones or profile-dependent bonus items
- tutorial/journal progression treated as complete (`intro = false`), matching
  established seed-finder behavior and excluding unseeded Guidebook placement

Seeds use the canonical `XXX-XXX-XXX` base-26 form. Searches support multiple
AND requirements across melee and thrown weapons, armor, and wands, exact `+1`,
`+2`, or `+3` upgrades, and weapon enchantment/curse or armor glyph/curse
constraints.
Mutually exclusive rewards are represented explicitly so impossible reward
combinations cannot satisfy a query.

Seed scouting accepts one canonical seed code and lists the searchable static
equipment generated through depth 24, including its floor, true upgrade,
enchantment or glyph, cursed state, source, and choice constraints. Scouting and
searching use the same version-pinned world generator; normal monster drops and
other play-time loot remain outside the compatibility profile.

## Project layout

- `crates/seedfinder-core`: deterministic Rust engine, query model, matcher,
  multicore scheduler, and Java-parity tests.
- `android`: original Jetpack Compose UI, coarse-grained search sessions, and
  the one-shot JNI seed-scout contract.
- `tooling/oracle`: reproducible, machine-readable whole-floor snapshots from
  an isolated export of the exact pinned game revision.
- `tooling/parity`: focused Java fixture generators for individual RNG and
  reward paths.
- `upstream` (ignored): local read-only clones of the official game and the
  established Java seed-finder used as an independent oracle.

## Local verification

```sh
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings

cd android
JAVA_HOME=/path/to/temurin-21 ./gradlew \
  :app:testDebugUnitTest :app:lintDebug :app:assembleRelease --offline
```

`assembleRelease` automatically cross-compiles the Rust JNI library for
`arm64-v8a` and `x86_64` with Android NDK 28.2. The resulting
`android/app/build/outputs/apk/release/app-release-unsigned.apk` must be signed
before installation. For local testing only, it can be signed with Android's
standard debug key:

```sh
"$ANDROID_HOME/build-tools/36.1.0/apksigner" sign \
  --ks "$HOME/.android/debug.keystore" \
  --ks-pass pass:android --key-pass pass:android \
  --out seed-seeker-release-debug-signed.apk \
  android/app/build/outputs/apk/release/app-release-unsigned.apk
```

A published build should instead use a protected distribution key. Debug builds
deliberately use the deterministic demo adapter and do not package stale release
JNI binaries.

The Java RNG fixture is JDK-only:

```sh
javac -d /tmp tooling/parity/RngOracle.java
java -cp /tmp RngOracle
```

`EquipmentOracle.java` is compiled against the isolated v3.3.8 oracle JAR and
calls the real game equipment classes. See [compatibility notes](docs/COMPATIBILITY.md)
for the parity boundary and known upstream nondeterminism.

## Performance model

Candidate worlds are independent. The scheduler assigns chunks to all available
cores with per-search cancellation and atomic progress. ARM64 builds batch MX3
and Java-LCG depth-root work in NEON lanes. The spatial generator remains scalar
inside each candidate because its rejection loops and room graph are highly
divergent; thread-level parallelism is the effective acceleration there. Android
streams matches as they are found and stops after 1,024 results so an unattended
search cannot grow memory without bound.

Android builds retain fat LTO but use optimization level O2; this is a pinned
correctness requirement for the audited Rust/LLVM Android AArch64 toolchain.
The host release profile remains O3. See the compatibility notes for the
on-device parity gate.

## Licensing and identity

This project is GPL-3.0-or-later. It contains a derived generation
implementation and an unchanged item sprite atlas from Shattered Pixel Dungeon,
which is also GPL-licensed. Copyright notices and the full license are included
with the Android distribution.

Seed Seeker is unofficial and is not endorsed by Shattered Pixel Dungeon or its
developers. It uses a distinct package, name, icon, and UI; no game UI components
are reused.

- Pixel Dungeon © 2012–2015 Oleg Dolya / Watabou
- Shattered Pixel Dungeon © 2014–2026 Evan Debenham
