# Seed Seeker for iOS

Native SwiftUI app targeting iOS 27 on iPhone and iPad. The app uses the same
Rust search, scout, share-link, result-file, and level-map engine as Android.
The Xcode project compiles the existing Apple `SeedSeekerKit` sources directly;
there is no second implementation of the wire formats or query models.

See the [iPhone screenshot gallery](../docs/ios/screenshots/README.md) for the
Finder, requirement editors, search results, and Scout workflows.

Artifact requirements support natural finds or up to ten transmutations, using
the remaining deck at the requirement's floor limit. Scout shows the compact
starting artifact deck, highlights matching targets, and dims artifacts available
in the dungeon. See [artifact search](../docs/artifact-search.md) for the rules.
Tap an item on an inline or expanded map to inspect its original game description,
seeded appearance, upgrades, effects, and container. Map inspection also supports
pointer hover and VoiceOver actions.

## Installation

Download `seed-seeker-<tag>-ios-arm64.ipa` from
[GitHub Releases](https://github.com/akhial/shpd-seed-seeker/releases).
It requires **iOS 27 or iPadOS 27 or later**. The IPA contains an unsigned device
app: SideStore or AltStore Classic signs it with your own Apple Account before
installing it. Opening the download directly in Files does not install the app.

The app has been verified in the iOS 27 simulator. Installation through these
tools on a physical device has not yet been verified; check their current
compatibility and setup instructions for your OS version.

### Set up a sideloading tool

Choose one:

- **SideStore:** follow its [prerequisites](https://docs.sidestore.io/docs/installation/prerequisites)
  and [installation guide](https://docs.sidestore.io/docs/installation/install).
  Initial setup needs a computer. Afterward, installation and refresh use Wi-Fi
  with LocalDevVPN connected on your device.
- **AltStore Classic:** follow the [macOS](https://faq.altstore.io/altstore-classic/how-to-install-altstore-macos)
  or [Windows](https://faq.altstore.io/altstore-classic/how-to-install-altstore-windows)
  setup guide. Keep [AltServer](https://faq.altstore.io/altstore-classic/altserver)
  running on your computer, with the device on the same Wi-Fi network or
  connected by USB when installing or refreshing apps.

Both setup guides cover signing in with your Apple Account, trusting its
developer profile, and enabling Developer Mode in iOS Settings.

### Install Seed Seeker

1. Save the release IPA to Files on your iPhone or iPad.
2. Open SideStore or AltStore Classic and go to **My Apps**.
3. Tap **+**, select the downloaded IPA, and wait for signing and installation
   to finish.
4. Open **Seed Seeker** from the Home Screen or App Library.

### Refresh and update

With a free Apple Account, sideloaded apps expire after **7 days**. Refresh
both Seed Seeker and your sideloading tool before their timers expire. In
SideStore, tap the remaining-days counter in **My Apps** while connected to
Wi-Fi and LocalDevVPN. In AltStore Classic, use **Refresh All** while AltServer
is reachable. Background refresh can help, but check the expiry timers yourself.
See [SideStore's refresh steps](https://docs.sidestore.io/docs/installation/install)
and [AltStore's refresh guide](https://faq.altstore.io/altstore-classic/your-altstore).

To update Seed Seeker, download the newer release IPA and import it through
**My Apps → +** again. Keep the existing app installed and use the same
sideloading tool and Apple Account to preserve its local data. Refreshing renews
the signature; it does not download a new Seed Seeker release. Deleting the app
removes its local data.

Free accounts normally allow three active sideloaded apps, including the
sideloading tool. See [SideStore's account limits](https://docs.sidestore.io/docs/faq#what-limitations-does-sidestore-have)
for details.

## Build requirements

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

### Package an IPA

From the repository root:

```sh
bash scripts/build-ios-app.sh device CODE_SIGNING_ALLOWED=NO
bash scripts/package-ios-ipa.sh
```

The packaging script reads `dist/ios/device/Seed Seeker.app` and writes
`dist/ios/SeedSeeker.ipa`. It rejects simulator and signed bundles. Pass an
output path as its first argument to choose a different filename:

```sh
bash scripts/package-ios-ipa.sh dist/ios/seed-seeker-dev-ios-arm64.ipa
```

The [release workflow](../.github/workflows/release.yml) builds this device IPA
and publishes `seed-seeker-<tag>-ios-arm64.ipa` alongside the other downloads
when a `v*` tag is pushed. A manual workflow run builds downloadable Actions
artifacts without publishing a GitHub release. CI needs no Apple signing
certificate, provisioning profile, or signing secrets; users sign the IPA
with their own accounts when installing it. The IPA is included in the release's
`SHA256SUMS.txt` alongside the other assets.

### Device behavior

The bundle identifier is `dev.seedseeker.ios`. A device must run iOS 27 or later.
The app registers `seedseeker://` query links and JSON result documents. Background
search uses iOS continued-processing tasks; availability and execution remain
subject to the system's scheduling decisions.

Background processing support can also depend on the sideloading tool's handling
of the app's background-task registration. When continued processing is
unavailable (including Simulator), the app saves
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

The [CI workflow](../.github/workflows/ci.yml) builds the unsigned ARM64 device
app and packages and validates its IPA on every pull request and push to `main`.
The `iOS` job uses Xcode 27 and needs no signing secrets. Its `ios-ipa` artifact
contains `SeedSeeker.ipa` and is retained for seven days.

The iOS test target runs the shared Apple engine/model tests and iOS map interaction
tests inside the app so catalog and sprite resources resolve from the real bundle:

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
- `SeedSeekerTests/`: iOS viewport and map inspection regressions, run alongside
  the shared `SeedSeekerKitTests` sources.
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
