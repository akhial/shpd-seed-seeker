# Scouted item mappings

The engine's `item_mappings(seed)` exposes every scroll rune, potion color, and
ring gem assignment, including items that do not appear on the scouted floors.
It uses canonical run initialization in an independent RNG state. Challenges,
trinkets, and scouting depth do not change these assignments.

Each category contains twelve entries in the game's item class order. Entries
provide `name`, `appearance`, and `spriteIndex` (Rust: `sprite_index`). Sprite
indices identify the unidentified appearance in `items.png`; do not add a ring
type glyph or resolve these cells through the ring gem table a second time.

Web scouting includes this data as `itemMappings: { scrolls, potions, rings }`.

Native clients opt in with `SSQ4`, whose request body is identical to `SSQ3`.
The response is `SSC7`: the entire `SSC6` body followed by three blocks in scroll,
potion, ring order. Each block contains exactly twelve entries, each encoded as
`name:utf8_u16, appearance:utf8_u16, sprite_index:u16`. Lengths and sprite indices
are unsigned and big-endian. The native decoder validates the extension against
the seed. `SSQ3` still returns `SSC6`, and older requests still return `SSC5`.

Clients reading older results should omit the mappings control when metadata is
absent, rather than displaying an unshuffled or guessed mapping.
