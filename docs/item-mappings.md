# Scouted item mappings

The engine's `item_mappings(seed)` exposes every scroll rune, potion color, and
ring gem assignment, including items that do not appear on the scouted floors.
It uses canonical run initialization in an independent RNG state. Challenges,
trinkets, and scouting depth do not change these assignments.

Each category contains twelve entries in the game's item class order. Entries
provide `name`, `appearance`, and `spriteIndex` (Rust: `sprite_index`). Sprite
indices identify the unidentified appearance in `items.png`; do not resolve
these cells through the ring gem table a second time.

Web scouting includes this data as `itemMappings: { scrolls, potions, rings }`.

Native clients opt in with `SSQ4`, whose request body is identical to `SSQ3`.
The response is `SSC7`: the entire `SSC6` body followed by three blocks in scroll,
potion, ring order. Each block contains exactly twelve entries, each encoded as
`name:utf8_u16, appearance:utf8_u16, sprite_index:u16`. Lengths and sprite indices
are unsigned and big-endian. The native decoder validates the extension against
the seed. `SSQ3` still returns `SSC6`, and older requests still return `SSC5`.

Clients reading older results should omit the mappings control when metadata is
absent, rather than displaying an unshuffled or guessed mapping.

## Journal grid artwork

Web and Android share `item-mapping-art.json` in the Android artwork directory.
This is display metadata from the pinned v4.0.0 game's
[`ItemSpriteSheet`](https://github.com/00-Evan/shattered-pixel-dungeon/blob/v4.0.0/core/src/main/java/com/shatteredpixel/shatteredpixeldungeon/sprites/ItemSpriteSheet.java) and
[`ScrollingGridPane`](https://github.com/00-Evan/shattered-pixel-dungeon/blob/v4.0.0/core/src/main/java/com/shatteredpixel/shatteredpixeldungeon/ui/ScrollingGridPane.java).
The unmodified `items.png` and `item_icons.png` atlases remain the artwork source.

Each category's identity icons use the same class order as the engine mappings:
scrolls start at icon 32, potions at 80, and rings at 0. Keep that identity index
attached to the entry if it is reordered. The grid uses game class order, with
stacked category headings and six tiles per row (two full rows per category).

Journal slots are 17×17 game pixels with a one-pixel gap. Center the full item
frame (scroll 15×14, potion 12×14, ring 8×10) within the slot. Place an identity
icon of size `w×h` at `(17-w, 0)`, flush with the top-right corner. Scale both
layers together with nearest-neighbor filtering. The seed's actual appearances
replace the journal's generic outlines. Names remain accessible through tile
labels and on-demand details.
