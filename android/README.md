# Seed Seeker Android

Seed Seeker is an independent, unofficial seed-search interface for Shattered Pixel Dungeon. It uses an original Jetpack Compose UI and does not include or reuse the game's UI components.

The debug build deliberately uses `DemoNativeSeedFinder` as its search engine, so UI states can be exercised with deterministic sample seeds. Interactive scouting uses the real engine in all build types, including trinket effects and match highlights. Release and dev builds select `JniNativeSeedFinder`, whose compact wire contract is documented in `NativeSeedFinder.kt`. All build types package `libshpd_seedfinder.so` for `arm64-v8a` and `x86_64` (built through `scripts/build-android-native.sh`): wire codecs such as the share-link format always run the canonical Rust implementation through `dev.seedseeker.app.engine.JniBindings`, even in debug APKs.

Build with:

```shell
./gradlew :app:assembleDebug
./gradlew :app:assembleDev
./gradlew :app:assembleRelease
```

`assembleDev` builds the real engine with release optimizations, but uses the
application ID `dev.seedseeker.unofficial.dev` and launcher name **Seed Seeker
Canary** so it can be installed alongside the production app. Its yellow icon
distinguishes it from production's blue icon. It is signed with the standard
local Android debug key and has the version-name suffix `-dev`.

The APK is written to `app/build/outputs/apk/dev/app-dev.apk`. Keep the same
local debug keystore (`~/.android/debug.keystore`) when building updates for an
existing Canary installation. Canary has its own app data, separate from
production and the debug demo.

The build uses Gradle 9.4 and AGP 9.1. Run Gradle on JDK 21 with Android SDK 36
and NDK `28.2.13676358` installed; app bytecode remains Java 11 compatible. The
native build also needs these Rust targets:

```shell
rustup target add aarch64-linux-android x86_64-linux-android
```

If a newer JDK is your shell default, set `JAVA_HOME` to JDK 21 before invoking
the wrapper. `ANDROID_HOME` or `android/local.properties` must identify the SDK.

The app targets API 36, supports API 23+, opts into edge-to-edge drawing, and uses AndroidX's predictive-back handler for in-app navigation. It uses Internet access for update checks, foreground-service and wake-lock permissions for background searches, and requests notification permission on Android 13+ when the first search starts. Declining notification permission does not prevent searching; Android still lists the service in its active-apps controls.

## Background searches and recovery

**Search** checks saved seeds against the current query and then looks for
more matches, filling the list toward 1,024 unique seeds. Imported results
have no scan history, so searching them starts a fresh scan after filtering.
Edited queries reuse prior scan progress only when the engine proves that it
is safe; otherwise they recheck saved seeds and scan afresh. The original
saved set remains available until Clear or a new import. Refinement keeps the
original automatic trinket selection rules, including reapplying a choice that
was previously removed as unnecessary, before testing the edited query.

**Filter loaded seeds**, shown beneath Search when saved seeds are available,
checks only that saved set. It also accepts unrelated queries: load results
for A, then filter for B to find seeds in that list satisfying B. Filtering
does not scan or discard the original set, so loosening a filter can bring
seeds back.

Searches belong to the application and run under a foreground service, so switching
apps, recreating the activity, removing its recent-apps task, or locking the screen
does not cancel the native engine. The ongoing **Seed searches** notification opens
the app and provides a **Stop** action. A partial CPU wake lock is held only while
the service is running and released when the search finishes, stops, or is interrupted.

Android's deep Doze mode and manufacturer battery restrictions can still suspend
searching. **Settings → Background search → Battery settings** opens Android's
battery controls; exempt Seed Seeker from optimization for uninterrupted locked-screen
searching. This increases battery use. The app does not request an exemption automatically.
See Android's [Doze documentation](https://developer.android.com/training/monitoring-device-state/doze-standby).

Every 15 seconds, the controller cooperatively stops the native workers, drains all
matches, atomically saves the exact remaining traversal and full result collection,
then resumes. The engine's running cursor is never used as a checkpoint. Queries,
trinket recipes, worker count, Target and detached/refine history survive process
death. A kill may replay work since the last checkpoint; overlapping results are
deduplicated. A kill before the first checkpoint restarts the fresh traversal or
filter. Completed and explicitly stopped searches stay stopped when restored.

Android may restart an interrupted service; otherwise reopening the app resumes
the saved search. Force-stop and reboot require reopening the app. Recovery files
are private and excluded from backup. An app/engine version change restores results
but discards old traversal coverage so searches cannot silently mix engine versions.
The foreground service uses `specialUse` for user-initiated offline computation;
its use case is declared in the manifest.

Device verification (use the real-engine Canary build):

1. Start a long search, wait at least 20 seconds, then lock the phone. Unlock and
   verify progress increased; repeat with battery optimization disabled for a Doze test.
2. Rotate the phone, switch apps, and remove Seed Seeker from recents. Reopen from
   the notification and verify the query/results and active search are retained.
3. Use `adb shell am force-stop dev.seedseeker.unofficial.dev`, reopen, and verify
   the search resumes with saved results. Repeat during refinement and after a reboot.
4. Stop through the notification, reopen, and verify no search restarts. Repeat
   after pressing Clear, and with notification permission denied.

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

## Search drafts

The Finder automatically saves the current query, including requirements, floor
limits, quest filters, challenges, and trinket options, and restores it on the
next launch. Empty drafts are saved too. Fireblast +3 is only the default when
there is no readable saved draft. Named presets remain separate; search results
and running searches are not restored.

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

Scout requests use SSQ4 (little-endian challenge mask and length-prefixed seed
and override, followed by canonical query JSON). SSC7 responses extend SSC6
with all 36 [item mappings](../docs/item-mappings.md). The info button beside
the scouted seed opens potion, scroll, and ring grids with six columns and two
full rows per category, in game class order. The seed's actual appearance is
centered in each tile, with its identity glyph at the top right, using the game's
journal frame geometry. Tap a tile to show its name and appearance; screen
readers announce the same description. The dialog scrolls independently of the
Scout floors. Older packets omit this button.

SSC6 responses extend SSC5
with a UTF-8 selected ID and big-endian unsigned 16-bit length; an empty ID means
no selection. The decoder also accepts SSC3, SSC4, SSC5, and SSC6.

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
or double-tap to zoom, and drag to pan; double-tap again to return to fit zoom.
The rounded previous/next control at the bottom and horizontal swipes at fit zoom
browse floors. The dialog shares the item list’s floor heading, quest badge, and
compact trinket shortcuts, retaining the current floor, zoom, pan and available quest area
when a selection changes. Inline and expanded maps keep independent viewports. Quest-area
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

## Blanket requirements

The collapsed **Blanket Requirements** section adds conditions on items already
chosen to fulfill ordinary requirements. Expand it to add or edit filters,
including either/or alternatives. Each blanket applies all its filters to one
chosen item; separate blankets may use the same or different chosen items.
Blankets do not request extra copies, combined levels, or trinket selection.
At least one ordinary requirement is required to search. Saved queries, results
files, and version 9 share links preserve blankets.

## Arcane Resin

Choose **Arcane Resin** when adding a Wand requirement. Set a minimum amount or
choose **Auto** to find enough resin to upgrade every matched wand to +3.
Auto counts each reserved wand once, including blanket witnesses; wands already
at +3 or higher need no resin. The requirement chip shows **Auto**.
Optionally limit donor wands by curse status, floor, or source. Donors must
be uncursed by default. Each surplus wand contributes `2 × (upgrade + 1)` resin;
wands reserved for ordinary requirements cannot also become resin. Resin is one
separate requirement and can be searched on its own.

The requirement can be edited or removed from the board. Saved drafts, presets,
share links, result exports, and search refinement retain its mode, amount, and filters.
Auto uses version 10 share links; existing fixed-amount links remain compatible.
Scout highlights the contributing wands, and the engine supplies the match estimate.
