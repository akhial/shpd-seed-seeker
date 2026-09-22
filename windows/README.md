# Seed Seeker for Windows

The Windows client is a native WinUI 3 desktop app using Fluent Design 2 and the same Rust engine and wire formats as the macOS and Android clients.

## Requirements

- Windows 10 1809 or newer (Windows 11 recommended)
- Visual Studio with **WinUI application development**, **.NET desktop development**, and ARM64 tools
- Rust MSVC ARM64 target: `rustup target add aarch64-pc-windows-msvc`

Open `SeedSeeker.slnx` in Visual Studio and select ARM64, or run `.\scripts\build-windows-app.ps1` from the repository root.

The app is unpackaged and framework-dependent. The Windows App SDK 1.8 runtime must be installed; Visual Studio installs it on development machines.

On first launch the app registers the `seedseeker://` link scheme for the current user (per-user `HKCU` entries, refreshed on every launch), so shared query links open in the app — in the already-running instance when there is one.

## Item artwork

Scouted items and requirements render the real Shattered Pixel Dungeon sprites, pulsing with the game's enchantment and curse glow colours. The atlas geometry and the glow table mirror `web/src/lib/sprites.ts` and `web/src/lib/glow.ts`; the Fluent palette itself is unchanged, and the only new colours are the per-enchantment glows, which are item data from the game rather than app chrome.

Requirements with several selected effects show their count inside a stationary ring, with the effect colours evenly spaced and smoothly blended around its circumference.

`items.png`, `item_icons.png`, `LICENSE.txt` and `ATTRIBUTION.md` are linked into `Assets\` from `android/app/src/main/assets/third_party/shattered-pixel-dungeon/` rather than duplicated. The artwork is GPL-3.0-or-later, so the app ships those notices and surfaces them: the sidebar footer's upstream-version link — whose text is the engine's own `shpdVersion` — opens an About dialog with the attribution and the full license text.

## Trinkets

The Trinket category selects a named trinket and supports the board’s existing OR groups. Its editor offers “Choose matching trinket at +3”, preserved in queries and share links. The engine applies a unique offered match after the first brewing opportunity; ambiguous matches apply no trinket. Scout displays the Magical Catalyst at its source and floor, with four square toggle cards and the remaining deck in one row. Click a card to apply it, or click the applied card again to deselect it. The selected card carries an “Applied +3” badge and matching cards retain a success border. SpriteView renders both sizes with nearest-neighbor scaling, and long card names shrink on one line. SSQ5 requests carry the query and optional override; the SSC8 decoder reads the selected identity, item mappings, and floor room summaries and retains SSC3–SSC7 response compatibility.

## Artifacts

The Artifact category requires a named artifact and supports floor limits, source and curse filters, and OR groups. Artifacts are single items rather than stacks. Scout uses the shared catalog and engine upgrades, including +5 Imp vault rewards, and keeps artifact matches aligned with the full manifest. Artifact queries show the measured match probability and estimated search time.

## Floor maps

Each supported Scout floor heading is a native map disclosure, retaining its feeling, region and quest labels. Floors without notable loot, including supported boss floors 5 and 15, remain available within the scouted prefix. Opening a map loads the engine's v3 scene and embedded PNGs on demand. Main/Mine/Vault area buttons follow the generated parent map's branches. Secrets start concealed and the toggle reveals rooms, doors and traps without regeneration.

Use the mouse wheel or pinch to zoom, drag to pan, and **Fit** to reset. The expanded Fluent dialog includes floor and trinket selectors, Previous/Next buttons, J/K floor navigation, and swipe navigation while fitted. Arrow keys pan, +/− zoom and 0 fits the map. Changing trinkets preserves the expanded dialog, floor, available quest area, zoom and pan while regenerating the map and Scout manifest together. Choice-group letter chips include the complete choice explanation as an accessible tooltip; alternatives conflicting with a matched choice dim to 45% opacity.

`LevelMapRenderer.cs` draws every engine layer in order, including initial contents, actor animation, tint, opacity, item glow and additive layers. Continuous particles use the engine's curves, acceleration, delayed hazard schedules, rotation, vertical scale, wall masks and chasm clipping. Geometric darkness applies last. Scenery redraws only changed cells, textures and a bounded set of documents are cached, and collapsed/offscreen/hidden views stop rendering. Windows' animation preference selects time zero. Native PNG decoding and viewport rasterization retain nearest-neighbour sampling at the display scale; no browser or asset network request is involved. See [the shared map contract](../docs/level-map-format.md) and the shipped `Assets/LEVEL-MAP-ATTRIBUTION.md` notice.

The host-compatible tests include the shipping C ABI, schema rejection, native boss/branch maps and asset hashes, rendering/animation math, concealment, additive wall masking and choice conflicts. Run `dotnet test windows/SeedSeeker.Tests` from the repository root with the Rust toolchain available. The WinUI application itself requires Windows to build and run.

## Seed information

The info button beside the scouted seed opens all potion colors, scroll runes,
and ring gems in six-column grids. Sprites use the shared journal frame metadata,
with each identity glyph flush at the top right. Click a sprite for its name and
appearance, also available through tooltips and accessible labels. Older scout
packets omit the control. The packaged `item-mapping-art.json` is shared with
Android, web, Linux, and macOS.

## Blanket requirements

The collapsed **Blanket Requirements** section adds conditions on items already
chosen to fulfill ordinary requirements or supply Arcane Resin. Expand it to
add or edit filters, including either/or alternatives. Each blanket applies all
its filters to one chosen item; separate blankets may use the same or different chosen items.
Blankets do not request extra copies, combined levels, or trinket selection.
At least one ordinary requirement is required to search. Saved queries, results
files, and version 9 share links preserve blankets.

## Arcane Resin

Choose **Arcane Resin** when adding a Wand requirement. Set a minimum amount or
choose **Auto** to find enough resin to upgrade each kept wand to +3.
Extra stack copies are reserved for Blacksmith reforging: they need no resin
upgrades and cannot also be consumed as resin donors.
In a wand’s editor, **Exclude from Auto resin** keeps that wand reserved without
budgeting resin upgrades, for example when imbuing it into the staff.
**Include Mage’s starting wand** adds 2 resin from the Magic Missile wand
recovered with Wand Preservation. The preserved wand is +0 regardless of the
staff’s level; incoming resin upgrades do not transfer when imbuing.

Auto counts each kept wand once. Donor wands are consumed for resin, including
those witnessing blankets, and add no upgrade cost. Wands already at +3 or higher
need no resin. The requirement chip shows **Auto**.
Optionally limit donor wands by curse status, floor, or source. Donors must
be uncursed by default. Each surplus wand contributes `2 × (upgrade + 1)` resin;
wands reserved for ordinary requirements cannot also become resin. Resin is one
separate requirement and can be searched on its own.

The requirement can be edited or removed from the board. Saved drafts, presets,
share links, result exports, and search refinement retain its mode, amount, and filters.
Auto uses version 10 share links; the Mage credit and per-wand exclusion use
version 12, including floor requirements. Existing links remain compatible.
The chip shows **Mage +2** for the starting credit and **No resin** on excluded wands.
Scout highlights the contributing wands, and the engine supplies the match estimate.
