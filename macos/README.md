# Seed Seeker for macOS

This directory contains the native SwiftUI app for Apple Silicon. The Swift package in
`SeedSeeker/` links the Rust engine statically through a small C shim.

## Requirements

- macOS 14 or newer
- Xcode with Swift 6
- Rust with the Apple Silicon target:

```sh
rustup target add aarch64-apple-darwin
```

## Build

From the repository root:

```sh
bash scripts/build-macos-app.sh
```

The app is written to `dist/Seed Seeker.app`. Local builds are ad-hoc signed. Set
`MACOS_SIGN_IDENTITY` to a Developer ID Application identity for distribution signing.

## Test

Build the Rust static library before running the Swift tests:

```sh
bash scripts/build-macos-native.sh
cd macos/SeedSeeker
swift test
```

See [`SeedSeeker/LINKING.md`](SeedSeeker/LINKING.md) for SwiftPM linking details.

The Trinket category selects one named trinket and supports the board's existing
Either/or grouping. Scout shows the Magical Catalyst on its generated floor,
with four square choices and the remaining deck in order below them. Matched
choices use the app's flat mint highlight. The native scout decoder accepts SSC4
(including the full 17-entry deck) and legacy SSC3 manifests.

The Artifacts category requires a named artifact and supports floor limits,
source and curse filters, and Either/or groups.
Scouting shows artifacts alongside other items with their generated upgrade.
Artifact searches display the engine's measured match probability.

Scout floor headings disclose inline maps, including supported boss floors in
the scouted prefix. Maps start closed; one heading can be open at a time and
the item list stays visible. Expand opens a native sheet with a floor picker, previous/next
buttons (K/J), quest branch segments and trinket shortcuts. Scroll or pinch to
zoom, drag or use arrow keys to pan, and press 0 to fit the map. Secrets start
hidden and can be revealed without regenerating the scene.
The sheet starts with the inline map's area and Secrets setting, then browses
independently. Trinket changes preserve the selected area, zoom and pan.
Opening an inline map starts loading immediately, even before scroll visibility
has updated; visibility only controls animation.

Maps use the engine's version 3 sprite scene and embedded PNG assets, including
initial containers, loot, actors, glow and continuous effects. Generation runs
away from the UI; maps and PNGs are cached, and offscreen maps pause. Reduce Motion
uses the static time-zero scene. Choice-group letter chips explain mutually
exclusive rewards; when a requirement matches, conflicting options are dimmed.
The contract is documented in [`level-map-format.md`](../docs/level-map-format.md).

## Seed information

The info button beside the scouted seed opens all potion colors, scroll runes,
and ring gems in six-column grids. Sprites use shared journal frames, with each
identity glyph flush at the top right. Click a sprite for its name and appearance;
the same description appears in tooltips and VoiceOver. The client requests SSQ5
and decodes SSC8, retaining SSC3–SSC7 compatibility and omitting the control when
legacy packets lack mappings. `build-macos-app.sh` bundles the shared artwork
metadata with the existing game atlases.

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
