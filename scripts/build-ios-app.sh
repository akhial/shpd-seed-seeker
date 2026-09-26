#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
PLATFORM="${1:-simulator}"
case "$PLATFORM" in
    simulator) SDK=iphonesimulator; DESTINATION='generic/platform=iOS Simulator' ;;
    device) SDK=iphoneos; DESTINATION='generic/platform=iOS' ;;
    *) echo "Usage: $0 [simulator|device] [xcodebuild options...]" >&2; exit 2 ;;
esac
if [ "$#" -gt 0 ]; then shift; fi

DERIVED_DATA="${IOS_DERIVED_DATA_PATH:-$ROOT/target/ios/build}"
CONFIGURATION=Release
BUILD_OPTIONS=()
while [ "$#" -gt 0 ]; do
    case "$1" in
        -derivedDataPath|-configuration)
            if [ "$#" -lt 2 ]; then
                echo "error: $1 requires a value" >&2
                exit 2
            fi
            if [ "$1" = -derivedDataPath ]; then
                DERIVED_DATA="$2"
            else
                CONFIGURATION="$2"
            fi
            shift 2
            ;;
        *) BUILD_OPTIONS+=("$1"); shift ;;
    esac
done

# The project's native build phase also runs when building directly in Xcode.
# Signing is unnecessary on a simulator. Device builds can set a team through
# IOS_DEVELOPMENT_TEAM or pass their own xcodebuild signing options.
SIGNING=()
if [ "$PLATFORM" = simulator ]; then
    SIGNING+=(CODE_SIGNING_ALLOWED=NO)
elif [ -n "${IOS_DEVELOPMENT_TEAM:-}" ]; then
    SIGNING+=("DEVELOPMENT_TEAM=$IOS_DEVELOPMENT_TEAM")
else
    SIGNING+=(CODE_SIGNING_ALLOWED=NO)
fi
xcodebuild -project "$ROOT/ios/SeedSeeker.xcodeproj" -scheme SeedSeeker \
    -configuration "$CONFIGURATION" -sdk "$SDK" -destination "$DESTINATION" \
    -derivedDataPath "$DERIVED_DATA" "${SIGNING[@]}" ${BUILD_OPTIONS[@]+"${BUILD_OPTIONS[@]}"} build

APP="$DERIVED_DATA/Build/Products/$CONFIGURATION-$SDK/SeedSeeker.app"
if otool -L "$APP/SeedSeeker" | grep -q shpd_seedfinder_ffi; then
    echo "error: SeedSeeker dynamically links the Rust engine" >&2
    exit 1
fi
OUTPUT="$ROOT/dist/ios/$PLATFORM/Seed Seeker.app"
mkdir -p "$(dirname "$OUTPUT")"
rm -rf "$OUTPUT"
ditto "$APP" "$OUTPUT"
echo "$OUTPUT"
