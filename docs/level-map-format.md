# Engine level maps (version 3)

The engine returns a **sprite scene as JSON**, with embedded game PNGs available
through an asset endpoint. It resolves terrain selection, water/chasm stitching,
visual variants, trap/plant sprites and animation frames. A platform only draws
rectangles. There is no graphics dependency, image encoder or runtime asset
network request in the engine.

This is an on-demand endpoint separate from item scouting. Search worlds and
SSC5 packets do not retain maps or carry sprite data. One request replays the
canonical generation prefix through the requested floor and captures that
floor after mob/item placement has modified terrain. It uses the same selected
trinket, +3 activation after the first brewing opportunity, challenges, persistent
item decks and quest state as loot scouting. Visual randomness uses a separate
RNG, seeded with the floor root, and cannot advance generation state.

## Coverage

Version 3 supports main-branch floors **1–9, 11–19, 21–24**, including boss
floors **5 and 15**.
It also supports **branch 1** at the Blacksmith quest floor (12–14, Crystal or
Gnoll mine) and the Imp quest floor (17–19, vault). The branch must exist at the
requested depth in the selected run; its variant is inferred from that run.
`engine_info.levelMaps` publishes supported main/branch depths, kinds, schema
version, tile size, projection and asset revision. Unsupported locations fail;
there is no substitution of the preceding floor. Boss floors 5 and 15 have
isolated terrain generators, including the secret Rat King room and the pylon,
wire, and water layout. Badder Bosses changes floor 15 generation, including
retries needed to keep every pylon reachable without crossing water or wires.
Depth 10 remains unsupported, and depth 20 only generates Imp shop stock.
Search/scout loot packets still omit the state-neutral floors 5/10/15; map
clients may offer supported boss floors within the scouted prefix.

Maps use Shattered's **raised 16×16 tile layers**: terrain, occlusion shadows,
terrain features, raised grass, and upper walls/overhangs. The original 4.0 shadow
atlas supplies the ambient wall shadows. The game's geometric fog rules mask
undiscoverable rock and the hidden halves of solid wall interiors to opaque black;
there is no player-position visibility radius or explored-floor dimming.

`scene.layers` reveals secret doors, hidden traps, and secret rooms.
`scene.concealedLayers` is a complete alternative layer stack using the same sprite
palette. It conceals room interiors, draws undiscovered doors as walls, and omits
hidden traps. Terrain, water edges, shadows, and overhangs are re-stitched for that
concealed geometry so secret areas do not leak through neighbouring cells. Prefer
this stack until the user enables secrets. Original terrain and secret metadata
remain unchanged. Switching visibility requires no regeneration or extra assets.

This is an overview of the floor immediately after generation. It includes
heaps and their contents, chest/tomb/skeleton/crystal containers, shop stock,
keys, planted bushes, initial mobs/NPCs, closed mimics (including Ebony Mimics),
boss actors and mine actors. Quest custom tiles include the smithy/furnace,
Mass Grave, ritual marker/tables and demon spawner floor. The Rat King room and
all its objects are concealed together until secrets are revealed.

Map generation assumes an obtainable hourglass on an earlier floor is taken,
identified and uncursed, and that its shop sand is purchased. The existing
shop-generation code then includes the appropriate sand stock and shuffle;
the acquisition cannot affect a shop already generated on the same floor.
`pickupAssumptions` records this profile. Ordinary search and loot scouting keep
their existing inventory profile. In particular, the map snapshot, texture
selection and animation allocation are never performed by seed search.

`contents` is a v3 object with `heaps`, `mobs`, `plants`, `effects`,
`features`, and supplemental `traps` arrays. Positions are row-major `cell` indices. Heaps contain
`kind`, `haunted`, `phantom` and top-first `items`; Cracked Spyglass phantom heaps
use 40% opacity (102/255) in the scene, including their glows and proportionally
dimmed shadows. Older documents omit `phantom`, meaning false. Mobs contain `kind` and the inventory
already rolled during generation, `sleeping`, disguise `stealthy`, and `approximate`. Items contain `kind`, `image` (the game's
item atlas index), `quantity`, `deterministic`, and optional `glow` (RGB `color`
and fade-in `periodMs`). Exposed heap/shop items pulse; closed containers and
sprite shadows do not inherit their contents' glow. Plants include their terrain
feature `image`; custom features carry `width` and `height` in cells. Object
kinds identify engine classes; searchable equipment uses catalog stable IDs.
Metadata always describes the complete floor, irrespective of secret visibility.
The scene defines the visible result; clients need not render metadata themselves.
The Imp Vault's two `VaultBeacon` spells pulse white, fading in over one second
and out over one second, matching `VaultBeacon.glowing()`.

Shop bags use the fixed preview sequence Scroll Holder (floor 6), Potion
Bandolier (floor 11), Magical Holster (floor 16), with `deterministic: false`
because purchases and inventory can change the in-game choice. This replaces
all bag offers, including a bag resolved by the canonical inventory profile.
Secret laboratory/library class-map selections and the vault's unseeded consumable
shuffle use explicit placeholder items with `deterministic: false`. Their locations and containers remain visible. Future
mob drops, respawns, quest completion rewards, harvested plant contents, killed
mimic loot beyond its initial inventory, and player remains are not fabricated.
The overview follows the canonical no-remains, no-holiday profile.

Water, well hearts/question marks, alchemy bubbles, sacrificial blue fire,
eternal green fire, city statue flames, blacksmith sparks and gas have repeatable animation loops.
Rotberry hearts emit green toxic specks every 0.7s over their raised sprite,
matching `RotHeartSprite.link()`.
Fire/newborn fire, frost, shock and chaos elementals carry their original flame,
magic, spark and rainbow effects. Gardens (including secret gardens) emit rising
light shafts; hidden secret gardens emit nothing in the concealed scene. The
shopkeeper tosses his yellow pixel coin on each 0.9s idle loop. Non-stealthy mimics
use the game's six-second hiding animation; stealthy mimics (including those
under Mimic Tooth) retain their single-frame disguise. Ebony mimics use 60%
opacity in the overview, with a matching faded shadow.
Vault flame vents retain their seeded `cycle` (`initialCooldown`, `cooldown`,
`triggers`) on each `VaultFlameTrap` feature. The map previews one game turn per
second: checkerboard vents alternate, flame paths follow their seeded offsets,
and treasure-room vents fire continuously. Small green warnings precede each
burst by one turn. These scouting-only schedules do not advance game simulation
or allocate additional data during seed search.
Purple Vault sentries preserve their seeded ray directions, cooldowns, warning
markers and shot groups. The decorative opposite row in laser treasure rooms
never fires. Death rays use the original effects atlas and fade/thin over 0.5s.
Crossing laser warnings share one red reticle per tile: a new warning resets
the existing marker, including while it is fading, without stacking opacity.
Blue sentries preserve the room's rotation (including clockwise/counterclockwise
treasure scans) or inward-facing perimeter pattern. Their blue checked cells
respect the game's Ballistica, ConeAOE and field-of-view geometry, propagate
outward with distance-dependent delays, then shrink and fade over 0.8s. Scouting
does not invent a hero target or advance terrain destruction/combat.
Chasms use the game's sparse white `WindParticle` squares: a 2.5s emission
interval, 1–2s lifetime, size-dependent drift and a triangular fade peaking at
10% opacity per pixel of size. The scouting visibility heuristic includes room
interiors and enclosed pits adjoining playable ground. On floor 15, the explicit
bridge abyss is included through its map-edge and arena-wall boundaries.
Exterior void and sealed rock pockets stay clear; concealed rooms lose their wind.
Visual randomness uses a deterministic breeze and never consumes generation RNG.
The vault entry uses its torn carpets, circular entrance, pulsing barrier and wall banners.
Actor sprites use their original idle/disguise films, flattened sprite shadows,
and sleep indicators. Foreground walls and raised terrain occlude heaps/actors.
Decorative overhangs (including mine crystal tips) are interleaved with actors
by their terrain anchor row, so tall actors cover decorations behind them.
The ordered stack can contain multiple `walls` layers; structural walls remain
in front of actors.
Ordinary ghoul partners receive a suitable adjacent tile with `approximate: true`:
the game creates them on its first turn using runtime RNG. Vault ghouls do not
spawn partners. Exact initial objects retain `approximate: false`.
Particle trajectories sample the game's unseeded visual motion separately from
generation RNG. Additive fire blending reproduces the bright overlapping glow.
Dynamic lighting, combat/movement, plant activation and inventory-dependent
runtime changes other than the stated hourglass assumption are not simulated.
Water uses the game texture and scrolling rate; this contract does not promise
pixel identity with a running game's ambient effects.

## Entry points

| Platform | Map | PNG asset |
| --- | --- | --- |
| Rust | `level_map::generate_level_map_in_branch(seed, depth, branch, challenges, selected_trinket)`; `level_map::json::decode_request(text)?.generate_document()` | `level_map::assets::get(id)` |
| Native Rust/session | `production_level_map_document(request_bytes)` | `level_map::assets::get(id)` |
| C (macOS/Windows) | `seedfinder_level_map(request, len, out, out_len)` | `seedfinder_level_map_asset(id, len, out, out_len)` |
| JNI (Android) | `JniBindings.levelMap(request: ByteArray)` | `JniBindings.levelMapAsset(id: ByteArray)` |
| WebAssembly | `level_map(request_json)` | `level_map_asset(id)` → `Uint8Array` |

JNI and C requests/responses are UTF-8 JSON. PNG endpoints take an exact UTF-8
asset ID and return binary PNG bytes. IDs are an allowlist, never paths or URLs.
C buffers must be freed with `seedfinder_buffer_free`; return codes are 0 on
success, -1 for invalid input/unsupported floors/unknown assets, and -2 for
internal failures. Failed calls clear output pointers/lengths. Native map calls
contain generation panics. JNI uses `IllegalArgumentException` for bad input and
`IllegalStateException` for generation/allocation failures; wasm uses JS errors.
The core JSON APIs require the existing `json-query` feature.

## Request

```json
{
  "seed": "AAA-AAA-AAA",
  "depth": 4,
  "challenges": [],
  "query": {
    "requirements": [{"item": "mimic_tooth", "select_trinket": true}]
  }
}
```

`seed` is a complete game seed code; `depth` is required. `branch` defaults to
`0` (regular floor); use `1` for its Blacksmith/Imp branch. For example,
`{"seed":"AAA-AAA-AAA","depth":13,"branch":1}` requests the branch on floor 13
and fails if that floor does not host the quest. `challenges` defaults
to `[]` and accepts the same stable challenge names as scouting. This field
controls generation even when `query` has its own challenge list, matching the
browser scout request. `query` is optional and uses the normal query schema.
Unknown request fields and invalid queries/challenges are rejected.

The optional `trinket` field follows scouting semantics:

- Missing or `null`: resolve selection using the query's AutoTrinket policy or
  explicit selected trinket slots, matching scouting.
- `"none"`: explicitly disable the trinket, overriding the query.
- A stable trinket ID: override the query with that offer.

Selections must belong to the seed's four initial catalyst offers. The resolved
selection is returned as `selectedTrinket` (a stable ID or `null`), even for a
floor before the trinket becomes active. A request's floor is independent of
the query's item-search depth limit.

Generate a document without any frontend:

```sh
cargo run -p shpd-seedfinder-core --features json-query --example level_map -- \
  '{"seed":"AAA-AAA-AAA","depth":1}' > map.json
```

## Response and rendering

The envelope identifies `format: "seed-seeker-level-map"`, `schemaVersion: 3`,
`shpdVersion`, `shpdCommit`, `profile: "canonical-scout-raised-v3"`, `assetRevision`,
`seed`, `depth`, `branch`, `kind`, `challenges`, `selectedTrinket` and `feeling`.
`kind` is `regular`, `blacksmith_crystal`, `blacksmith_gnoll` or `imp_vault`.
Regular-floor `branches` lists accessible quest branches as
`{depth, branch: 1, kind, entrance}`. This entrance cell belongs to the parent
map. Branch maps have an empty `branches` list, their `entrance` returns to
branch 0 at the same depth, and `exit` is null.
Consumers should reject unsupported schema versions. Generation compatibility
and sprite-source compatibility are recorded separately.

`width` and `height` are in cells. `terrain` is a row-major array of game terrain
codes (`geometry::terrain`), length `width * height`. `entrance` and `exit` are
cell indices or `null`. `secretRooms` contains inclusive `[left, top, right,
bottom]` bounds. `secretDoors` and `secretTraps` contain cell indices. `traps`
contains `{cell, kind, hidden, active}`, using engine trap class identities in v3.

`assets` lists the required PNGs with `{id, width, height, sha256}`. Load
and cache them using the asset endpoint; PNG bytes are omitted from JSON.
The original textures are embedded once in the engine; each document lists only
the assets its scene uses.
Map documents reuse a sprite palette.

`scene` contains `tileSize: 16`, `sprites`, `layers`, `concealedLayers`,
`emitters`, and `concealedEmitters`. Every layer has `name`
and `cells`, another row-major array of `width * height` entries. `null` means
no sprite; an integer indexes `sprites`. Choose exactly one layer stack and draw
its layers in array order, including custom floor art, heaps, actors, custom
overhangs and particles. Do not assume a fixed list of layer names. Draw cells in row-major order within each
layer. The last layer supplies opaque geometric wall masks, not a visibility hint
for the frontend to interpret.

Each sprite contains `frameDurationMs` and a nonempty `frames` array. Each frame
is a list of drawing commands in draw order:

```json
{
  "frameDurationMs": 1,
  "frames": [[{
    "kind": "blit",
    "asset": "tiles_sewers.png",
    "source": [128, 48, 16, 16],
    "destination": [0, 0, 16, 16]
  }]]
}
```

- `blit`: copy the source PNG rectangle to the destination rectangle. Multiply
  alpha by `opacity / 255` (default 255). Optional `tint: [r,g,b]` multiplies
  each source RGB component by that component / 255, preserving alpha. Black
  tinted, flattened sprite images provide item/actor shadows.
  Optional `glow: {color: [r,g,b], periodMs}` blends the sprite RGB toward the
  glow colour, preserving source alpha: `rgb * (1-v) + color * v`, where
  `phase = (elapsedMs / periodMs) % 2` and `v = min(phase, 2-phase) * 0.6`.
  Evaluate the glow every display frame, independently of sprite-frame changes.
- `fill`: fill `destination` with `rgba: [red, green, blue, alpha]`, each 0–255.

Both rectangle types are `[x, y, width, height]` in pixels, with a top-left
origin and downward-positive y. Destination coordinates are relative to the
cell's origin. Use nearest-neighbour sampling and ordinary source-over alpha
on an opaque black canvas; a layer with `blend: "add"` uses additive compositing.
Assets contain straight alpha. Apply viewport
translation/zoom after computing map pixel coordinates; an integer scale keeps
pixel art crisp. Version 3 commands stay inside their cell's 16×16 rectangle.

The complete animation algorithm is:

```text
frameIndex = floor(elapsedMilliseconds / frameDurationMs) % frames.length
cellOrigin = [(cell % width) * 16, floor(cell / width) * 16]
for command in frames[frameIndex]:
    draw(command, offset = cellOrigin)
```

Use a monotonic elapsed time shared by all sprites. There is no interpolation.
Sprites with one frame are static. Frame zero of every sprite gives a complete
static/reduced-motion image. Rust consumers can use `MapSprite::frame(time)`.
Water has 32 frames at 200 ms (5 pixels/second, 6.4-second loop); wrapped texture
samples are already split into valid rectangles. Pipe drips use continuous emitters. Actors retain their individual frame durations. Do not regenerate maps
on animation ticks.

Continuous effects use the selected `emitters` / `concealedEmitters` list at every
display frame, independently of the scenery frame cache (120 Hz on a 120 Hz display).
Each emitter provides an `image` command, `cell`, `loopMs`, `blend`, velocity and
acceleration in pixels/second, and `angularSpeed` in degrees/second. Each particle
provides `birthMs`, `lifespanMs`, a cell-relative `position` in milli-pixels,
initial `scale` in thousandths, and an initial `angle` in degrees.

```text
clockMs = elapsedMs - (startMs or 0)
if startMs is present and clockMs < birthMs: skip
ageMs = positiveModulo(clockMs - birthMs, loopMs)
if ageMs >= lifespanMs: skip
progress = ageMs / lifespanMs
seconds = ageMs / 1000
position = initialPosition / 1000 + velocity * seconds + acceleration * seconds² / 2
scale = initialScale / 1000 * evaluate(emitter.scale, progress)
alpha = evaluate(emitter.alpha, progress)
angle = initialAngle + angularSpeed * seconds
scaleX = scale * (evaluate(emitter.scaleX, progress) if scaleX exists else 1)
scaleY = scale * (evaluate(emitter.scaleY, progress) if scaleY exists else 1)
```

Optional `startMs` delays scheduled hazards and prevents future emissions from
wrapping backward before their first cycle. Ambient emitters omit it and prewarm
their loops. Curves contain `[progress, value]` points in thousandths: linearly interpolate,
then take the square root if `sqrt` is true. Draw the image centered at the
resulting position, scaled/rotated with the resulting alpha.
Optional `scaleX` and `scaleY` multiply the local horizontal and vertical scale
independently. Beams can thin without shortening, and garden shafts can widen
while growing taller. Both default to 1 for existing documents. `contents.sentries` retains each sentry's initial
cooldown, repeated cooldown, trigger count, warning flag and target-cell groups;
optional `scan` stores cone degrees and tile length, both multiplied by 1000.
`blend: "add"` requires the underlying scenery as the blend destination. Emitters with
`wallMask: true` are occluded by the alpha silhouettes of `raised`, `walls`,
`room_walls`, and `boss_walls`. Status icons use `wallMask: false` and appear
above those surfaces. Apply geometric darkness last to both groups.
Emitters with `clipToChasm: true` draw before other world particles and are
clipped to the union of the tile rectangles of all `clipToChasm` emitters in
the selected scene. Keep this clip during movement, so a square can drift
between adjacent chasms but cannot spill onto floors or into excluded void.
Reduced-motion/static views sample
time zero. The browser uses a separate particle canvas, copying only emitter
bounds from cached scenery before compositing; it pauses offscreen/hidden maps.

Cache scenes by generation version, schema/asset revision, seed, depth, branch,
challenge mask and resolved trinket. Changing the selection must use a fresh
map document. No UI state, zoom, wall-clock time or platform identifier affects
the generated map.

## Scout integrations

The web and all four native Scout panes disclose maps from floor headings and
offer an expanded view. Supported empty and boss floors remain navigable within
the scouted prefix. Branch controls come from the loaded parent map, secrets
start concealed, and trinket changes retain the expanded floor while returning
to its main branch. Each request carries the completed scout's challenge mask
and resolved trinket, including explicit `"none"` when nothing is selected.

Android uses a native Canvas inside Compose, macOS uses CoreGraphics in SwiftUI,
Windows uses a native pixel compositor with WinUI controls, and Linux uses Cairo
with GTK/libadwaita. Generation runs outside the UI thread; each client caches
scenes and embedded textures and offers retry on failure. Reduced-motion views
sample time zero and offscreen maps suspend drawing. Platform READMEs describe
their gestures and controls; native regression tests cover scene composition,
particle schedules, secret masks and trinket refresh alongside choice matching.

## Source and validation

Generation uses the repository's pinned v4.0.0 engine and saved Java parity
fixtures. Terrain hashes, dimensions and transitions are tested against those
fixtures across all five regions. Tests also cover query/override selection,
pre-brewing behaviour, repeated requests, secret revelation, source/destination
bounds, animation wrapping and native/wasm document equivalence.

Sprite assets and their selection tables are pinned to the v4.0.0 source commit
`2bb34a4e91d29c8785a9363cad6ddfe5122b1d4f`. The official release JAR and its
SHA-256 are pinned in `tooling/oracle-4.0/build.sh`.
See [asset attribution](../crates/seedfinder-core/assets/level-map/ATTRIBUTION.md)
for the original files and rules. Mining terrain additionally matches 36 fixtures generated from the unmodified
v4.0.0 release JAR (both quest types, depths 12–14, three seeds, challenge masks
0/104). Its separate JAR digest and reproduction instructions are recorded in
[mining-map fixtures](../tooling/oracle-4.0/tests/mining-maps.md).
Vault terrain uses the v4.0.0-validated generator and saved vault fixtures.
Sentry setup and ray/scan coverage match 342 official sentries across nine seeds
in `tests/fixtures/vault-sentries.json`; regenerate with
`python3 tooling/oracle-4.0/generate-vault-sentries.py`.

The raised selectors are checked against the unmodified official v4.0.0 JAR:
65 full-layer hashes across 13 maps cover every region, Crystal and Gnoll mines,
and an Imp vault, including the example seed JHG-HJJ-BKK. The oracle receives
the engine's terrain and independently resolves atlas indices; it validates
rendering selection, not generation parity or dynamic lighting. See
[raised rendering fixtures](../tooling/oracle-4.0/tests/raised-map-visuals.md) for
the JAR digest and reproduction commands. Secret concealment, geometric black
masks, animation and draw bounds have separate engine tests.

Version 3 adds initial contents, tinted/alpha sprites, and continuous emitters to
the raised version 2 scene. Consumers must check
`schemaVersion` and include it with the rendering profile in persistent cache keys.

Initial contents are validated against 142 official-JAR maps, including four seeds,
challenge variants, Mimic Tooth, vaults, and both mines. The fixtures compare full
heaps, generation-time actor inventory, sleeping states, plants, trap placement,
and blob locations. Regenerate with `python3 tooling/oracle-4.0/generate-map-contents.py`.
Item atlas bounds are exported from the evaluated official `ItemSpriteSheet.film`
using `ItemFilmOracle`, including loop assignments and later overrides.

### Item inspection

The optional `itemTooltips` array is shared by web, Android, macOS, Windows, and
GTK. It adds metadata to schema v3 without changing scene rendering. Each entry
has `cell`, `bounds`, `label` (container/inventory owner, empty for loose items), `hidden`
(inside a concealed secret room), and `items`. Each item has `name`,
`description`, `image` (the run's item atlas cell), `icon`, `quantity`, and `deterministic`.
`bounds` is `[x, y, width, height]` relative to the cell in map pixels, using the
same raised sprite placement as scene rendering. Use it for both hit testing and
selection outlines; older entries without bounds fall back to the ground tile.
`icon` is an optional source rectangle in `item_icons.png`, independent of the
seeded appearance. It covers rings, potions, scrolls, and both exotic categories,
using the pinned Java `ItemSpriteSheet.Icons` frames. Render it over the centered
item sprite at the upper right, at the same pixel scale.
`upgrade` is the identified in-game level (including artifact rounding), or null
when no generated equipment roll is available. `cursed` carries the rolled curse
flag; `enchantment` names a beneficial weapon enchantment or armor glyph, and
`curse` names a weapon/armor curse independently of that flag. These are retained
from generation for heaps, shops, carried mob equipment, and vault rewards.
`glow` uses the same RGB color and `periodMs` as the map sprite. Pulse the art
between 0 and 60% tint over twice that period, leave the type glyph solid, and
hold at 30% tint when reduced motion is enabled. Wands never glow.

Clients must suppress hidden entries while Secrets is off, invert the current
map transform when hit testing, and clear inspection on navigation or gestures.
Older documents without this array remain renderable without inspection.

Names and static descriptions come from the original English Java messages at
v4.0.0 commit `2bb34a4e91d29c8785a9363cad6ddfe5122b1d4f`, under GPL-3.0-or-later
(see `NOTICE`). Regenerate with `python3 scripts/generate-item-text.py`.
The generated file records each source SHA-256. Game emphasis markers are
removed; hero/depth-dependent formatted sentences are omitted. No damage
or future-drop stats are inferred. Seeds include the original
planting instruction and plant description, and exotic consumables resolve
through the game's own regular-to-exotic mapping. Unknown future types retain
a readable name, and runtime-dependent items are marked `Varies with play`.
