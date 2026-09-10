# Level map sprites

These are unmodified Shattered Pixel Dungeon PNG assets:

- Pixel Dungeon © 2012–2015 Oleg Dolya / Watabou.
- Shattered Pixel Dungeon © 2014–2026 Evan Debenham.
- License: GPL-3.0-or-later; see the repository's [COPYING](../../../../COPYING).

Source revision: **2bb34a4e91d29c8785a9363cad6ddfe5122b1d4f**.

Original files are in
[`core/src/main/assets/environment`](https://github.com/00-Evan/shattered-pixel-dungeon/tree/2bb34a4e91d29c8785a9363cad6ddfe5122b1d4f/core/src/main/assets/environment):
`tiles_sewers.png`, `tiles_prison.png`, `tiles_caves.png`, `tiles_city.png`,
`tiles_halls.png`, `tiles_caves_crystal.png`, `tiles_caves_gnoll.png`,
`water0.png` through `water4.png`, `terrain_features.png`, and
`custom_tiles/caves_quest.png` and `custom_tiles/city_quest.png` (stored here
without the `custom_tiles/` prefix).
Every file's SHA-256 and dimensions are recorded in
[`level_map/assets.rs`](../../src/level_map/assets.rs) and returned in map JSON.

Rendering rules are adapted from the same revision's
[`DungeonTileSheet`](https://github.com/00-Evan/shattered-pixel-dungeon/blob/2bb34a4e91d29c8785a9363cad6ddfe5122b1d4f/core/src/main/java/com/shatteredpixel/shatteredpixeldungeon/tiles/DungeonTileSheet.java),
`DungeonTerrainTilemap`, `TerrainFeaturesTilemap`, the trap/plant sprite indices,
`GameScene` water scrolling and `SewerLevel.Sink` pixel particles. The overview
uses flat sprites, reveals secret doors/traps, and makes ambient animation
repeatable. The source revision is distinct from the generation engine's RC1
JAR pin; it is not presented as an RC1 asset extraction.

To update, obtain the original PNGs and rendering tables from one explicit
source revision, update the embedded manifest and bounds/selection tests, and
review the map schema and cache version. Do not silently swap sprite atlases:
their indices are part of the rendering contract.
