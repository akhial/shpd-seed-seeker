# Agent guidance

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
