# Seed Seeker Android

Seed Seeker is an independent, unofficial seed-search interface for Shattered Pixel Dungeon. It uses an original Jetpack Compose UI and does not include or reuse the game's UI components.

The debug build deliberately uses `DemoNativeSeedFinder` as its search engine, so UI states can be exercised with deterministic sample seeds. Interactive scouting uses the real engine in both build types, including trinket effects and match highlights. Release builds select `JniNativeSeedFinder`, whose compact wire contract is documented in `NativeSeedFinder.kt`. Both build types package `libshpd_seedfinder.so` for `arm64-v8a` and `x86_64` (built through `scripts/build-android-native.sh`): wire codecs such as the share-link format always run the canonical Rust implementation through `dev.seedseeker.app.engine.JniBindings`, even in debug APKs.

Build with:

```shell
./gradlew :app:assembleDebug
./gradlew :app:assembleRelease
```

The build uses Gradle 9.4 and AGP 9.1. Run Gradle on JDK 21 with Android SDK 36
and NDK `28.2.13676358` installed; app bytecode remains Java 11 compatible. The
native build also needs these Rust targets:

```shell
rustup target add aarch64-linux-android x86_64-linux-android
```

If a newer JDK is your shell default, set `JAVA_HOME` to JDK 21 before invoking
the wrapper. `ANDROID_HOME` or `android/local.properties` must identify the SDK.

The app requests no Android permissions. It targets API 36, supports API 23+, opts into edge-to-edge drawing, and uses AndroidX's predictive-back handler for in-app navigation.

## Licensing

This project is licensed under GPL-3.0-or-later. The unmodified `items.png` atlas is redistributed from Shattered Pixel Dungeon v3.3.8 under that license; details and integrity metadata live under `app/src/main/assets/third_party/shattered-pixel-dungeon/`.

Shattered Pixel Dungeon is copyright © 2014–2026 Evan Debenham. Pixel Dungeon is copyright © 2012–2015 Oleg Dolya. Seed Seeker is not affiliated with or endorsed by Shattered Pixel Dungeon or its authors.

## Scout scrolling

The seed form scrolls out beneath the app bar. During the last part of that
scroll, the summary reduces to one line: counts move right and fade, the seed
becomes slightly smaller, and the requirements capsule wipes its label from
the right while shrinking into a status circle. Complete matches show a check;
partial matches keep an info indicator and the full count for accessibility.
Pagination and trinket controls stay beneath the summary, above sticky floor
headings. Scrolling back to the top reverses the transition.

`./gradlew :app:testDebugUnitTest :app:lintDebug` runs the Android checks without
an emulator. The Scout Compose tests use Robolectric's native renderer and save
review screenshots under `app/build/outputs/scout-ui/`.

## Trinkets

The requirement picker has a named Trinket category, including either/or groups.
Trinkets use a second details page for the matching-trinket switch and the
shared save and trash controls, without equipment filters. Scout shows
four square choices beneath the catalyst on its actual floor, highlights matching
choices with a flat green fill, and keeps the remaining thirteen icons
in one row. Sprite drawing uses nearest-neighbor filtering.

The requirement editor can choose matching trinkets at +3. The engine applies
that selection after the first brewing opportunity, with ambiguous offered OR
alternatives leaving no trinket selected. Presets, exports, and share links keep
this choice. Probability estimates use the selected generation effects.

Tap any of the four offered scout cards to apply it, or tap the selected card
again to deselect it. The selected card carries an “Applied +3” badge. Each
rescout keeps the query and explicit override together so match highlights use
the same generated world.

Scout requests use SSQ3 (little-endian challenge mask and length-prefixed seed
and override, followed by canonical query JSON). SSC6 responses extend SSC5
with a UTF-8 selected ID and big-endian unsigned 16-bit length; an empty ID means
no selection. The decoder also accepts SSC3, SSC4, and SSC5.

SSC5 contains the SSC3 layout followed by a 17-entry trinket deck: a one-byte
count and UTF-8 IDs with unsigned 16-bit lengths. After the deck, SSC5 carries a one-byte
feeling count (0..20), followed by depth/feeling byte pairs in strictly ascending
regular-floor order (1..24, excluding boss floors). Feeling IDs are none=0,
chasm=1, water=2, grass=3, dark=4, large=5, traps=6, secrets=7.
The deck ordering is independent of item sorting. Scout floor headings show the
feeling sprite without visible text; normal floors have no icon. The unmodified
`dungeon-icons.png` atlas and its upstream attribution are packaged beside the
item artwork.

## Floor maps and reward choices

Tap a supported Scout floor heading to open its map above the loot, or tap again
to close it. Maps start closed, with one floor open at a time; empty floors and
boss floors 5 and 15 are included. Tap **Expand map** for a full-screen Material dialog. Pinch
or double-tap to zoom, drag to pan, and use **Fit** to reset. Previous/next controls
and horizontal swipes at fit zoom browse floors. The dialog offers the initial
four trinkets and retains the current floor when a selection changes. Quest-area
chips open the Blacksmith Mine or Imp Vault; **Secrets** switches the complete
concealed/revealed scene without generating another map.

Map requests use the completed scout's challenges and resolved trinket. JNI map
generation and PNG decoding run off the UI thread, with bounded document and
atlas caches. The native Canvas renderer implements the engine's version 3
[sprite scene](../docs/level-map-format.md), including initial items, containers,
actors, sprite animations, item glows, continuous particles, seeded Vault hazards,
additive blending, wall/darkness masks and chasm clipping. It redraws only changed
scenery tiles, renders particles at the display rate, stops animation while hidden,
and uses time zero when Android's animator duration scale is disabled. The embedded
map artwork's [attribution](../crates/seedfinder-core/assets/level-map/ATTRIBUTION.md)
is separate from the older item-picker atlas.

Mutually exclusive rewards carry compact lettered choice chips with accessible
option descriptions. When the engine marks a reward as a requirement match,
conflicting alternatives in that group dim; compatible options and unrelated
groups retain their normal appearance. Requirement effect-count badges use a
stationary ring with evenly spaced, smoothly blended effect colors.

`LevelMapTest` exercises the real host JNI, generates map review images under
`app/build/outputs/level-maps/`, and checks particle scheduling, glows, secret
stacks, additive blending and occlusion. `ScoutChoicesTest` covers choice conflicts
and supported empty/boss-floor navigation.
