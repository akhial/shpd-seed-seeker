# Seed Seeker for Linux

This directory contains the native Linux application. It uses GTK 4 through gtk-rs and
libadwaita-rs, follows the GNOME Human Interface Guidelines and the GNOME application-ID and
resource conventions, and is part of the root Cargo workspace. The binary is named
`seed-seeker-gtk` because the CLI already claims `seed-seeker` in the shared workspace target
directory.

## Interface

The window is an adaptive triple-pane layout built from two nested `AdwNavigationSplitView`s,
the libadwaita pattern for triple-pane navigation. Breakpoints collapse the panes into
push/pop navigation as the window narrows, down to a fully single-pane phone-sized layout.

- **Query** (sidebar) builds the search declaratively: a board of requirement chips, plus
  floor limit, Wandmaker quest, and blacksmith scope controls. Dropping one chip
  on another makes an either/or cluster; dragging a member back onto the board pulls it out
  again; a chip's badges say how many items of its kind it asks for and what combined upgrade
  level they reach. A chip opens in a dialog with category, item, tier, upgrade,
  enchantment/glyph, source, total item count, and per-item floor limit predicates.
  Artifacts require a named selection, support per-item floor limits, and
  show the Imp vault reward at +5 when scouting. Artifact match probability uses the engine's measured supply model.
  Trinkets use a named selection without equipment details. Both can join either/or clusters
  and have no item-count control. Included and user-saved presets are available from the
  sidebar and main menu; user presets can be updated or deleted. Challenges live in a
  preferences dialog, and the whole query persists across sessions in the user configuration
  directory. A Performance group sets how many search threads to spawn — a device-local
  preference saved beside the query rather than in it, and hidden on a single-core host.
- **Results** streams matching seed codes from a full-seed-space production session running
  on the chosen number of cores with a rotated start, with live match probability,
  seeds-per-second, time-to-match, and a seeds-searched count. Impossible queries, empty completions, and
  worker failures each get a dedicated status page. Sessions are cancellable and cap
  accepted results at 1,024, like the other app frontends.
- **Seed** scouts one seed — typed in, or selected from the results — and lists every
  searchable item through depth 24, grouped by floor with region names, upgrade and
  enchantment tags, cursed state, source, and choice constraints. Items that jointly
  satisfy the current requirements are highlighted. Special floor feelings appear as
  game sprites beside the floor heading; normal floors have no icon. The magical catalyst appears on its
  generated floor with its source, four square trinket choices, and a single smaller row
  preserving the remaining deck order. Matched choices use a flat green fill and border.

The engine is linked in-process through `shpd-seedfinder-session`. The shell also provides
the application lifecycle, app actions and shortcuts (with a shortcuts dialog), an About
dialog, embedded resources including per-category symbolic icons, a desktop entry, and
AppStream metadata.

Floor headings disclose an on-demand map, including empty floors and supported
boss floors within the scouted run. Maps use the engine's version 3 raised sprite
scene and embedded textures: initial items, actors, animated glows and continuous
effects retain their seeded schedules, wall occlusion and secret visibility.
Generation runs off the GTK thread with a bounded scene cache; hidden maps stop
painting, and disabling system animations samples the complete scene at time zero.

Drag to pan, scroll or pinch to zoom, and use the fit button (or `0`) to reset.
The adaptive expanded dialog includes previous/next floor controls (`K`/`J`) and
trinket shortcuts. Switching trinkets refreshes the map and loot while keeping the
dialog's floor, zoom, pan and selected area when it remains available. Native branch tabs open the
Blacksmith mine or Imp vault; the Secrets toggle reveals the alternative scene.
Load failures offer Retry. Choice-group letter chips sit beside match badges;
only alternatives conflicting with a matched choice are dimmed.

## Requirements

- Rust 1.97 or newer
- GTK 4.22 or newer
- libadwaita 1.9 or newer
- `pkg-config` and `glib-compile-resources`

On Fedora 44:

```sh
sudo dnf install gcc gtk4-devel libadwaita-devel pkgconf-pkg-config
```

## Run

From the repository root:

```sh
cargo run -p shpd-seedfinder-gtk
```

GTK Inspector can be enabled while developing with `GTK_DEBUG=interactive`.

## AppImage

The AppImage builder packages the release binary, GTK, libadwaita, the Adwaita icon theme,
GSettings schemas, desktop integration metadata, and license notices. It supports native x86_64
and arm64 builds on Fedora 44:

```sh
sudo dnf install gcc curl file gtk4-devel libadwaita-devel pkgconf-pkg-config
APPIMAGE_VERSION=dev bash scripts/build-linux-appimage.sh
./dist/seed-seeker-dev-"$(uname -m)".AppImage
```

Opening `seedseeker://` share links requires the bundled desktop entry to be installed, so
AppImage users need desktop integration (for example through AppImageLauncher or `--install`
tooling) before such links launch the app.

The Release workflow runs the same builder for both architectures when a `v*` tag is pushed and
publishes the AppImages with the other GitHub Release assets. Release builds first compile the
pinned GTK and libadwaita stack against Ubuntu 24.04 so they retain an older glibc baseline;
`build-linux-appimage.sh` can use that stack locally through `APPIMAGE_GTK_PREFIX` as well.

## Validate

```sh
cargo check -p shpd-seedfinder-gtk
cargo clippy -p shpd-seedfinder-gtk --all-targets -- -D warnings
cargo test -p shpd-seedfinder-gtk
desktop-file-validate linux/data/dev.seedseeker.SeedSeeker.desktop
appstreamcli validate --no-net linux/data/dev.seedseeker.SeedSeeker.metainfo.xml
```

## Seed information

The info button beside the scouted seed opens all potion colors, scroll runes,
and ring gems directly from the engine. Each category has six columns and two
rows of sprites, with identity glyphs at the top right. The shared journal frame
metadata keeps artwork aligned with the other apps. Click a sprite to reveal its
name and appearance; tooltips and accessible labels carry the same information.

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
choose **Auto** to find enough resin to upgrade every reserved wand to +3.
Auto counts each reserved wand once. Donor wands are consumed for resin, including
those witnessing blankets, and add no upgrade cost. Wands already at +3 or higher
need no resin. The requirement chip shows **Auto**.
Optionally limit donor wands by curse status, floor, or source. Donors must
be uncursed by default. Each surplus wand contributes `2 × (upgrade + 1)` resin;
wands reserved for ordinary requirements cannot also become resin. Resin is one
separate requirement and can be searched on its own.

The requirement can be edited or removed from the board. Saved drafts, presets,
share links, result exports, and search refinement retain its mode, amount, and filters.
Auto uses version 10 share links; existing fixed-amount links remain compatible.
Scout highlights the contributing wands, and the engine supplies the match estimate.
