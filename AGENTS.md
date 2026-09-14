# Agent guidance

## Android Canary builds

When asked to build the Canary app, use the `dev` build variant. From the
repository root, run:

```sh
cd android
./gradlew :app:assembleDev
```

The resulting APK is `android/app/build/outputs/apk/dev/app-dev.apk` (relative
to the repository root). It must have all three Canary identifiers:

- A **yellow launcher icon**, including the round and adaptive icon variants.
- Application ID **`dev.seedseeker.unofficial.dev`** (the `.dev` suffix is required).
- Launcher name **Seed Seeker Canary**, exactly as written.

Preserve the Canary resource overrides in `android/app/src/dev/res/` and the
`dev` build configuration in `android/app/build.gradle.kts`. Before handing off
or installing a Canary APK, verify its packaged application ID and label, and
check that its launcher icon is yellow. Do not substitute an `assembleDebug`
or `assembleRelease` APK for Canary.

Canary uses the real JNI engine with release optimizations and is signed with
the local Android debug key. Keep `~/.android/debug.keystore` stable when
building updates for an existing Canary installation. See `android/README.md`
for the required JDK, Android SDK/NDK, and Rust targets.
