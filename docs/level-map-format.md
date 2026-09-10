# Engine level maps (version 1)

The engine returns a **sprite scene as JSON**, with embedded game PNGs available
through an asset endpoint. It resolves terrain selection, water/chasm stitching,
visual variants, trap/plant sprites and animation frames. A platform only draws
rectangles. There is no graphics dependency, image encoder or runtime asset
network request in the engine, and no UI is introduced here.

This is an on-demand endpoint separate from item scouting. Search worlds and
SSC5 packets do not retain maps or carry sprite data. One request replays the
canonical generation prefix through the requested floor and captures that
floor after mob/item placement has modified terrain. It uses the same selected
trinket, +3 activation after the first brewing opportunity, challenges, persistent
item decks and quest state as loot scouting. Visual randomness uses a separate
RNG, seeded with the floor root, and cannot advance generation state.

## Coverage

Version 1 supports regular main-branch floors **1–4, 6–9, 11–14, 16–19, 21–24**.
It also supports **branch 1** at the Blacksmith quest floor (12–14, Crystal or
Gnoll mine) and the Imp quest floor (17–19, vault). The branch must exist at the
requested depth in the selected run; its variant is inferred from that run.
`engine_info.levelMaps` publishes supported main/branch depths, kinds, schema
version, tile size, projection and asset revision. Unsupported locations fail;
there is no substitution of the preceding floor. Boss floors 5/10/15 have no
terrain generator in this engine, and depth 20 only generates Imp shop stock.
Boss arenas and the final levels need separate map generators.

Maps use the game's **flat** 16×16 tile sprites to make the complete layout
readable without wall occlusion or fog. Secret doors are drawn as ordinary
closed doors; recorded traps are drawn even when hidden. Original terrain
codes, hidden trap flags, secret-door cells and secret-room bounds remain in
the document so a future UI can distinguish secrets. Dark feelings do not hide
terrain in this scouting view.

This is an initial **terrain overview**, not a screenshot of a running game.
It includes recorded plants and traps, mine crystals/boulders/gold, the branch
return stairs, and the vault’s flame trap markers. Items, heaps, monsters, NPCs, blobs,
lighting, room-specific custom tilemaps (such as decorative paintings and quest
props other than the branch return stairs), ripples and other ambient effects are not drawn. Custom terrain uses
its base game tile; the underlying layout remains present. Inventory-dependent
changes and events during play are outside the canonical scout profile.
Water uses the game texture and scrolling rate; pipe particles use a repeating
approximation of the game's unseeded particle motion. The JSON does not promise
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

- Missing or `null`: resolve selection from the query's selected trinket slots.
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

The envelope identifies `format: "seed-seeker-level-map"`, `schemaVersion: 1`,
`shpdVersion`, `shpdCommit`, `profile: "canonical-scout-flat-v1"`, `assetRevision`,
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
contains `{cell, kind, hidden, active}`, using snake_case trap identities.

`assets` lists the required PNGs with `{id, width, height, sha256}`. Load
and cache them using the asset endpoint; PNG bytes are omitted from JSON.
The approximately 170 KiB of original textures are embedded once in the engine.
Map documents reuse a sprite palette.

`scene` contains `tileSize: 16`, `sprites` and `layers`. Every layer has `name`
and `cells`, another row-major array of `width * height` entries. `null` means
no sprite; an integer indexes `sprites`. Draw layers in array order (water,
terrain, structures, features, effects), then cells in row-major order.

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

- `blit`: copy the source PNG rectangle to the destination rectangle.
- `fill`: fill `destination` with `rgba: [red, green, blue, alpha]`, each 0–255.

Both rectangle types are `[x, y, width, height]` in pixels, with a top-left
origin and downward-positive y. Destination coordinates are relative to the
cell's origin. Use nearest-neighbour sampling and ordinary source-over alpha
on an opaque black canvas. Assets contain straight alpha. Apply viewport
translation/zoom after computing map pixel coordinates; an integer scale keeps
pixel art crisp. Version 1 commands stay inside their cell's 16×16 rectangle.

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
samples are already split into valid rectangles. Pipe drips have eight 50 ms
frames. Consumers never need to implement water stitching, UV wrapping or a
particle system, and must not regenerate the map on each animation tick.

Cache scenes by generation version, schema/asset revision, seed, depth, branch,
challenge mask and resolved trinket. Changing the selection must use a fresh
map document. No UI state, zoom, wall-clock time or platform identifier affects
the generated map.

## Source and validation

Generation uses the repository's pinned RC1 engine and saved Java parity
fixtures. Terrain hashes, dimensions and transitions are tested against those
fixtures across all five regions. Tests also cover query/override selection,
pre-brewing behaviour, repeated requests, secret revelation, source/destination
bounds, animation wrapping and native/wasm document equivalence.

The RC1 release URL recorded in `tooling/oracle-4.0/build.sh` returned 404 while
this feature was implemented. Sprite assets and their selection tables are
therefore explicitly pinned to source commit
`2bb34a4e91d29c8785a9363cad6ddfe5122b1d4f`, separately from the RC1 JAR digest.
See [asset attribution](../crates/seedfinder-core/assets/level-map/ATTRIBUTION.md)
for the original files and rules. Mining terrain additionally matches 36 fixtures generated from the unmodified
v4.0.0 release JAR (both quest types, depths 12–14, three seeds, challenge masks
0/104). Its separate JAR digest and reproduction instructions are recorded in
[mining-map fixtures](../tooling/oracle-4.0/tests/mining-maps.md).
Vault terrain uses the existing RC1-validated generator. No fresh Java
rendering-parity run is claimed.
