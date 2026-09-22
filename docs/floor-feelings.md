# Floor feelings

`GeneratedWorld.feelings` stores the final feeling of each generated regular
floor, in ascending depth order. The regional, batch, and gated generators
carry this data alongside items and quests. Boss floors have no feeling entry;
prefix generation only includes floors within the requested effective depth.
There is no observer, callback, extra generation pass, or independent RNG roll.

All scout panes display the game sprite immediately after the floor label.
Normal floors and old packets without feeling metadata display no extra icon.
Feeling names are available to accessibility APIs without visible text.
The web UI reads the same world data through the WASM `feelings` array:
`[{ "depth": 2, "feeling": "chasm" }]`.

## Native wire format

The current encoder emits `SSC5`. Its prefix is the existing `SSC4` layout:
seed, ring gems, quests, items, and the 17-entry trinket deck. Immediately after
the deck it appends:

```text
feeling_count:u8
repeated feeling_count times { depth:u8, feeling:u8 }
```

| ID | Feeling |
| --- | --- |
| 0 | None |
| 1 | Chasm |
| 2 | Water |
| 3 | Grass |
| 4 | Dark |
| 5 | Large |
| 6 | Traps |
| 7 | Secrets |

Counts are 0..20. Depths must be unique and strictly ascending in 1..24,
excluding boss depths 5, 10, 15, and 20. Unknown feeling IDs, invalid counts,
depths or ordering, truncated fields, and trailing bytes are rejected. A full
canonical scout carries all 20 regular floors, including normal floors (ID 0).

Decoders continue to accept `SSC3` (no deck or feelings) and `SSC4` (deck, no
feelings), returning an empty feelings collection. Legacy scout requests emit `SSC5`; `SSC3` and `SSC4` are supported only
for decoding.

The sprite atlas is Shattered Pixel Dungeon's `interfaces/icons.png`, pinned
with provenance in the asset attribution. The approved large feeling frames
are 15 by 16 pixels at y=64, x=16 times the feeling ID.

Selected-trinket requests (`SSQ3`) return `SSC6`, which preserves the entire
`SSC5` layout and appends the selected stable ID as a big-endian u16-length
UTF-8 string (empty means none). All native decoders accept both versions.

## Floor and room search (web)

The web Requirements pane includes **RoW farming floors**, with independent
7, 17, and 22 toggles. Every selected floor must have the Dark feeling and
at least one Garden or Secret Garden. No Ring of Wealth is implicitly required;
combine the floor toggles with an item requirement to constrain its upgrade,
source, or availability. Selecting a floor raises the global floor limit if
needed; subsequently lowering the limit produces a validation error. Farming
floors are marked **RoW farm** in the scout.

The shared engine supports all feelings and 95 room classes, exposed as stable
IDs in `engine_info.roomTypes`. A query may contain no item requirements when
it has a floor requirement:

```json
{
  "requirements": [],
  "floor_requirements": [
    { "depth": 7, "feeling": "dark", "any_rooms": ["garden", "secret_garden"] },
    { "depth": 17, "feeling": "dark", "rooms": ["garden"] }
  ]
}
```

Depths must be unique, regular floors within the global limit. Omitted feeling
means any; `"none"` explicitly requires a normal floor. `rooms` requires all
listed room types; nonempty `any_rooms` requires at least one listed type on
that same floor. Both predicates apply if supplied together. These are presence
checks, independent of room count and size. Empty conditions, unknown fields,
unknown IDs, duplicate depths, and boss floors are rejected.

`GeneratedWorld.floor_rooms` retains one 128-bit presence mask per regular
floor, alongside feelings. WASM scouts expose `floorRooms: [{depth, rooms}]`.
Native scout packets keep their existing format and omit room summaries.
Portable JSON preserves floor requirements in presets and result exports.
Share links use version 11 when floor requirements are present; older queries
retain their existing link bytes. Room enum ordering is append-only because
links and calibration tables use those indices.

The planner compiles depth-indexed masks once per query and extends generation
to the deepest floor condition. Feeling mismatches stop after preparation;
room mismatches stop after successful graph construction, before painting,
mobs, loot, or a deferred vault. Checks also run on the terminal floor and
when replaying saved trinket recipes. Final matching independently checks the
retained summaries. Earlier floors retain all normal state transitions.
AutoTrinket selection remains item-based; floor conditions are evaluated under
the selected setup. Query edits continue to start fresh traversal coverage
while rechecking the retained seed pool.

## Probability calibration and performance

The merged overnight bake measures 1,048,560 deterministic, dispersed seeds
for each of the eight trinket profiles (8,388,480 worlds). The 3.73 MiB
`probability_tables/floors.bin` contains packed little-endian counts and offsets,
read directly without allocation or decompression.

The table includes room pairs, repeated rooms across floors, and cross-floor
feeling pairs. Room scheduling only distinguishes Large, Secrets, and ordinary
feelings, so ordinary feelings share a room distribution to reduce sampling
noise. Normal-profile feelings use their exact generator probabilities. Mossy
Clump and Trap Mechanism use measured feeling distributions and correlations,
plus separate room rows for each exact feeling. Those rows use another 524,288
worlds/profile because brewing timing correlates feelings with laboratories,
adding a 1.61 MiB table with the same direct-read format.

Two-room conjunctions and unions use measured intersections. Wider combinations
use a tree of pairwise dependencies. Cross-floor estimates account for repeated
room types (including either-garden) and alternating trinket feelings; other
cross-floor dependencies and item/floor correlations remain approximate.
Challenges use canonical measurements, matching the existing item model.
These estimates never reject seeds; only generation and exact matching do.

See [probability calibration](probability-calibration.md) for the held-out query
sweeps, remaining limitations, performance measurements, and regeneration commands.

`examples/benchmark_floors.rs` compares identical queries with and without the
new floor gates, retaining existing item pruning on both sides. A local run
on 4,096 separate dispersed seeds produced identical matches in every case:

| Query | Matches | Speedup from floor gates |
| --- | ---: | ---: |
| Darkness on 17 | 292 | 1.11× |
| Hidden garden on 7 | 213 | 1.13× |
| Farming floor 7 | 42 | 1.34× |
| Farming floor 17 | 45 | 1.08× |
| Farming floor 22 | 53 | 1.09× |
| RoW by 16 and farming floor 17 | 12 | 1.01× |
| RoW by 16 only (control) | 1,038 | 1.00× |

Deep-floor gains are limited by the necessary generation of earlier floors.
Current farming estimates and their held-out observations are recorded in the
[calibration report](probability-calibration.md).
