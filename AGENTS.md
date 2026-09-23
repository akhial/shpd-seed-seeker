# Agent guidance

## CI test budget

Cap seed equivalence and randomized matcher differential tests at 1,024 seeds/cases.
Keep smaller samples small, retain known positive and negative cases, and reuse
generated worlds across query combinations where possible. Large calibration
and exhaustive sweeps must stay opt-in (`#[ignore]`, run with `--release`).

The test profile optimizes `shpd-seedfinder-core` while retaining debug assertions
and overflow checks. Keep that setting: complex query tests can take minutes in
an unoptimized build even when they generate only one seed.

## Android Canary builds

When asked to build the Canary app, use the `dev` build variant. From the
repository root, run:

```sh
cd android
./gradlew :app:assembleDev
```

The resulting APK is `android/app/build/outputs/apk/dev/app-dev.apk` (relative
to the repository root).

Canary uses the real JNI engine with release optimizations and is signed with
the local Android debug key. Keep `~/.android/debug.keystore` stable when
building updates for an existing Canary installation. See `android/README.md`
for the required JDK, Android SDK/NDK, and Rust targets.
