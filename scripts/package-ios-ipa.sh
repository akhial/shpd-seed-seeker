#!/usr/bin/env bash
# Package the unsigned device build for SideStore / AltStore to re-sign.
# Usage: package-ios-ipa.sh [output.ipa]
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
APP="$ROOT/dist/ios/device/Seed Seeker.app"
IPA="${1:-$ROOT/dist/ios/SeedSeeker.ipa}"

if [ "$#" -gt 1 ]; then
    echo "Usage: $0 [output.ipa]" >&2
    exit 2
fi
if [ ! -d "$APP" ]; then
    echo "error: $APP not found; run scripts/build-ios-app.sh device CODE_SIGNING_ALLOWED=NO first" >&2
    exit 1
fi

# Both simulator and device builds are arm64. Check the platform as well so
# a simulator bundle cannot accidentally become a public device download.
PLATFORM=$(/usr/libexec/PlistBuddy -c 'Print :CFBundleSupportedPlatforms:0' "$APP/Info.plist")
EXECUTABLE=$(/usr/libexec/PlistBuddy -c 'Print :CFBundleExecutable' "$APP/Info.plist")
if [ "$PLATFORM" != iPhoneOS ] || [ "$(lipo -archs "$APP/$EXECUTABLE")" != arm64 ]; then
    echo "error: packaging requires an arm64 iPhoneOS device build" >&2
    exit 1
fi
if [ ! -x "$APP/$EXECUTABLE" ]; then
    echo "error: the app executable is missing or not executable" >&2
    exit 1
fi
if [ -e "$APP/embedded.mobileprovision" ] || [ -d "$APP/_CodeSignature" ] || codesign -d "$APP" 2>/dev/null; then
    echo "error: packaging requires an unsigned app; rebuild with CODE_SIGNING_ALLOWED=NO" >&2
    exit 1
fi

STAGING=$(mktemp -d "${TMPDIR:-/tmp}/seed-seeker-ipa.XXXXXX")
trap 'rm -rf "$STAGING"' EXIT
mkdir -p "$STAGING/Payload" "$(dirname "$IPA")"
ditto --norsrc --noextattr --noqtn "$APP" "$STAGING/Payload/Seed Seeker.app"
# An IPA is a ZIP with Payload at its root. Do not include macOS resource
# forks or Finder metadata; preserve the executable's Unix permissions.
COPYFILE_DISABLE=1 ditto -c -k --norsrc --keepParent "$STAGING/Payload" "$STAGING/SeedSeeker.ipa"
unzip -tq "$STAGING/SeedSeeker.ipa"
mv -f "$STAGING/SeedSeeker.ipa" "$IPA"
echo "$IPA"
