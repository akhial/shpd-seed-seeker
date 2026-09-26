# Seed Seeker for iOS

Native SwiftUI app targeting iOS 27 on iPhone and iPad. The app uses the same
Rust search, scout, share-link, result-file, and level-map engine as Android.
The Xcode project compiles the existing Apple `SeedSeekerKit` sources directly;
there is no second implementation of the wire formats or query models.

See the [iPhone screenshot gallery](../docs/ios/screenshots/README.md) for the
Finder, requirement editors, search results, and Scout workflows.

## Requirements

- Xcode 27 with the iOS 27 SDK
- An Apple Silicon Mac
- Rust with both Apple mobile targets:

  ```sh
  rustup target add aarch64-apple-ios-sim aarch64-apple-ios
  ```

## Simulator build

From the repository root:

```sh
bash scripts/build-ios-app.sh simulator
```

This builds the real Rust engine with release optimizations, LTO, and the same
allocator as the other native frontends. It then builds the SwiftUI app for an
arm64 iOS simulator and writes `dist/ios/simulator/Seed Seeker.app`.

Open `ios/SeedSeeker.xcodeproj`, select the **SeedSeeker** scheme and an iOS 27
simulator such as **iPhone 18 Pro**, then Run. Building directly from Xcode also
builds the Rust archive and bundles the shared artwork automatically.

## Device build

```sh
bash scripts/build-ios-app.sh device
```

The unsigned device bundle is written to `dist/ios/device/Seed Seeker.app`.
For a signed build, select a development team in Xcode or supply it to the script:

```sh
IOS_DEVELOPMENT_TEAM=YOUR_TEAM_ID bash scripts/build-ios-app.sh device -allowProvisioningUpdates
```

Use `-derivedDataPath target/ios/device-build` to build a device app alongside
simulator tests without sharing Xcode's build database. The script also accepts
`-configuration Debug`; the Rust engine retains release optimizations.

The bundle identifier is `dev.seedseeker.ios`. A device must run iOS 27 or later.
The app registers `seedseeker://` query links and JSON result documents. Background
search uses iOS continued-processing tasks; availability and execution remain
subject to the system's scheduling decisions.

When continued processing is unavailable (including Simulator), the app saves
a drained native search checkpoint before suspension and resumes on return.
An explicit Cancel remains cancelled. Drafts, presets, retained seed recipes,
and completed results also survive relaunches. The compact-chip and worker
preferences stay on the device and are excluded from shared queries.

Android's battery-optimization opt-out and APK downloader are platform-specific:
iOS uses continued processing and the chosen signing/distribution channel.
The HTTPS query-link codec is shared with every other frontend; automatic
universal-link routing would additionally require a signed Associated Domains
entitlement and a hosted Apple association file for that signing team. The
registered `seedseeker://q/CODE` form works in local simulator builds.

## Tests

The iOS test target runs the shared Apple engine/model tests inside the app so
catalog and sprite resources resolve from the real bundle:

```sh
xcodebuild -project ios/SeedSeeker.xcodeproj -scheme SeedSeeker \
  -destination 'platform=iOS Simulator,name=iPhone 18 Pro' \
  -derivedDataPath target/ios/build CODE_SIGNING_ALLOWED=NO test
```

Rust is always built in release mode, including when the Swift configuration is
Debug. Shared randomized test samples retain the repository's CI test budget.

## Project layout

- `SeedSeeker/`: iOS SwiftUI interface and platform integrations. Xcode uses a
  synchronized source group, so new Swift files are included automatically.
- `../macos/SeedSeeker/Sources/SeedSeekerKit/`: shared models, persistence, codecs,
  engine bridge, search controller, and map scene decoder; built as a static
  framework within the iOS project.
- `CSeedFinder/module.modulemap`: exposes the existing shared C ABI header.
- `../scripts/build-ios-native.sh`: builds the target-specific Rust static
  archive in `target/aarch64-apple-ios[-sim]/release/` and generates the engine
  revision header under `target/ios/generated/`.
- `Assets.xcassets/`: app icon and About preview rendered from the existing
  `assets/icon/seed-seeker-square.svg`. `scripts/generate-icons.sh` regenerates
  them alongside the other platform icons.

The project bundles the canonical Android catalog, item atlases, metadata, and
Shattered Pixel Dungeon license from
`android/app/src/main/assets/third_party/shattered-pixel-dungeon/`. Dungeon icons
come from the web frontend's shared assets. Both artwork attributions and the
full license ship in the application bundle. No Sparkle framework is included.
