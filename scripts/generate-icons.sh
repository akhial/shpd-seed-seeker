#!/usr/bin/env bash
# Regenerates every checked-in raster icon from the master SVGs in
# assets/icon/. The outputs are committed so CI never needs an SVG rasterizer.
#
# Two masters feed the size ladder, because one chest cannot serve all of it:
#
#   seed-seeker-small.svg   the reduced 14x14 chest. Crisp at 16, 32 and 48px,
#                           where the full 28x27 sprite cannot land on whole
#                           pixels and downsampling mushes the straps.
#   seed-seeker-square.svg  the full sprite, from 64px up.
#
# macOS takes the reduction as far as 64px: the Big Sur plate is only 824 of
# 1024pt, so its content is a further 20% smaller at any given size. The
# handover therefore sits between the 32pt and 128pt entries of the iconset,
# which is a logical-size boundary rather than a break inside one.
#
# Requires: rsvg-convert (brew install librsvg), ImageMagick (magick),
# iconutil and tiffutil (macOS).
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
SRC="$ROOT/assets/icon"
TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT

render() { # render <svg> <size> <out.png>
    rsvg-convert -w "$2" -h "$2" "$1" -o "$3"
}

canary_svg() { # canary_svg <blue source.svg> <yellow output.svg>
    sed \
        -e 's/#0F3272/#FFC107/g' \
        -e 's/#071D48/#F57F17/g' \
        -e 's/#E8F0FF/#FFF8E1/g' \
        -e 's/#0A2149/#4E3400/g' \
        -e 's/#1E4E9E/#A65F00/g' \
        -e 's/#F6F9FF/#FFFDE7/g' \
        -e 's/#2E6AC8/#FFA000/g' \
        -e 's/#4A8AEA/#FFD54F/g' \
        -e 's/#050F2E/#2D1B00/g' \
        -e 's/#B7CDEF/#FFE082/g' \
        "$1" > "$2"
}

# --- macOS AppIcon.icns -----------------------------------------------------
ICONSET="$TMP/AppIcon.iconset"
mkdir -p "$ICONSET"
for entry in 16:1 16:2 32:1 32:2 128:1 128:2 256:1 256:2 512:1 512:2; do
    pt="${entry%%:*}"; scale="${entry##*:}"
    px=$((pt * scale))
    [ "$pt" -le 32 ] && master=small || master=macos
    suffix=""; [ "$scale" = 2 ] && suffix="@2x"
    render "$SRC/seed-seeker-$master.svg" "$px" \
        "$ICONSET/icon_${pt}x${pt}${suffix}.png"
done
mkdir -p "$ROOT/macos/SeedSeeker/Resources"
iconutil -c icns "$ICONSET" -o "$ROOT/macos/SeedSeeker/Resources/AppIcon.icns"

# --- iOS universal app icon and About preview -------------------------------
IOS_ASSETS="$ROOT/ios/Assets.xcassets"
mkdir -p "$IOS_ASSETS/AppIcon.appiconset" "$IOS_ASSETS/AppIconPreview.imageset"
render "$SRC/seed-seeker-square.svg" 1024 "$IOS_ASSETS/AppIcon.appiconset/AppIcon.png"
cp "$IOS_ASSETS/AppIcon.appiconset/AppIcon.png" "$IOS_ASSETS/AppIconPreview.imageset/AppIcon.png"

# --- Windows SeedSeeker.ico -------------------------------------------------
WINPNGS=()
for size in 16 32 48 64 128 256; do
    [ "$size" -le 48 ] && master=small || master=square
    render "$SRC/seed-seeker-$master.svg" "$size" "$TMP/win_$size.png"
    WINPNGS+=("$TMP/win_$size.png")
done
mkdir -p "$ROOT/windows/SeedSeeker/Assets"
magick "${WINPNGS[@]}" "$ROOT/windows/SeedSeeker/Assets/SeedSeeker.ico"

# --- web favicon ------------------------------------------------------------
render "$SRC/seed-seeker-small.svg" 32 "$ROOT/web/public/favicon.png"

# --- Android legacy launcher mipmaps (API < 26) -----------------------------
# Both masters here are the full sprite, so only 96 and 192 land on whole
# pixels. That is deliberate: mixing the reduction into one density and not
# its round twin would look worse than uniform softness, and every Android 8+
# device uses the adaptive vectors in res/drawable instead, which are exact.
RES="$ROOT/android/app/src/main/res"
CANARY_RES="$ROOT/android/app/src/dev/res"
canary_svg "$SRC/seed-seeker-square.svg" "$TMP/seed-seeker-canary-square.svg"
canary_svg "$SRC/seed-seeker-round.svg" "$TMP/seed-seeker-canary-round.svg"
for entry in mdpi:48 hdpi:72 xhdpi:96 xxhdpi:144 xxxhdpi:192; do
    density="${entry%%:*}"; px="${entry##*:}"
    mkdir -p "$RES/mipmap-$density"
    mkdir -p "$CANARY_RES/mipmap-$density"
    render "$SRC/seed-seeker-square.svg" "$px" "$RES/mipmap-$density/ic_launcher.png"
    render "$SRC/seed-seeker-round.svg" "$px" "$RES/mipmap-$density/ic_launcher_round.png"
    render "$TMP/seed-seeker-canary-square.svg" "$px" \
        "$CANARY_RES/mipmap-$density/ic_launcher.png"
    render "$TMP/seed-seeker-canary-round.svg" "$px" \
        "$CANARY_RES/mipmap-$density/ic_launcher_round.png"
done

# --- DMG background (1x + 2x combined into a retina tiff) -------------------
rsvg-convert -w 660 -h 420 "$SRC/dmg-background.svg" -o "$TMP/dmg_1x.png"
rsvg-convert -w 1320 -h 840 "$SRC/dmg-background.svg" -o "$TMP/dmg_2x.png"
mkdir -p "$ROOT/macos/dmg"
tiffutil -cathidpicheck "$TMP/dmg_1x.png" "$TMP/dmg_2x.png" \
    -out "$ROOT/macos/dmg/background.tiff"

echo "regenerated: macos icns, windows ico, web favicon, android mipmaps, dmg background"
