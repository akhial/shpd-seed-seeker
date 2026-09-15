//! Seed-specific names and artwork for unidentified consumables and rings.

use crate::catalog::{ITEMS, RING_SPRITE_BASE};
use crate::run::RunState;
use crate::seed::DungeonSeed;

/// One unidentified appearance and the item it identifies in this run.
#[derive(Clone, Debug, Eq, PartialEq)]
#[cfg_attr(feature = "json-query", derive(serde::Serialize))]
#[cfg_attr(feature = "json-query", serde(rename_all = "camelCase"))]
pub struct ItemMapping {
    pub name: &'static str,
    pub appearance: &'static str,
    /// The unidentified sprite, without an identified item's type overlay.
    pub sprite_index: u16,
}

/// All twelve entries per category, including items absent from the scouted floors.
/// Entries follow the game's scroll, potion, and ring class order.
#[derive(Clone, Debug, Eq, PartialEq)]
#[cfg_attr(feature = "json-query", derive(serde::Serialize))]
pub struct ItemMappings {
    pub scrolls: [ItemMapping; 12],
    pub potions: [ItemMapping; 12],
    pub rings: [ItemMapping; 12],
}

/// Uses the canonical run initialization. Appearances depend only on the seed,
/// so this never advances a world's RNG or depends on challenges or trinkets.
///
/// # Panics
/// Panics if the seed range exceeds `i64` or the catalog omits a ring class;
/// both are internal invariants of the pinned game version.
#[must_use]
pub fn item_mappings(seed: DungeonSeed) -> ItemMappings {
    let appearances =
        RunState::new(i64::try_from(seed.value()).expect("seed fits i64")).appearances;
    let scrolls = [
        "Scroll of upgrade",
        "Scroll of identify",
        "Scroll of remove curse",
        "Scroll of mirror image",
        "Scroll of recharging",
        "Scroll of teleportation",
        "Scroll of lullaby",
        "Scroll of magic mapping",
        "Scroll of rage",
        "Scroll of retribution",
        "Scroll of terror",
        "Scroll of transmutation",
    ];
    let runes = [
        "KAUNAN", "SOWILO", "LAGUZ", "YNGVI", "GYFU", "RAIDO", "ISAZ", "MANNAZ", "NAUDIZ",
        "BERKANAN", "ODAL", "TIWAZ",
    ];
    let potions = [
        "Potion of strength",
        "Potion of healing",
        "Potion of mind vision",
        "Potion of frost",
        "Potion of liquid flame",
        "Potion of toxic gas",
        "Potion of haste",
        "Potion of invisibility",
        "Potion of levitation",
        "Potion of paralytic gas",
        "Potion of purity",
        "Potion of experience",
    ];
    let colors = [
        "Crimson",
        "Amber",
        "Golden",
        "Jade",
        "Turquoise",
        "Azure",
        "Indigo",
        "Magenta",
        "Bistre",
        "Charcoal",
        "Silver",
        "Ivory",
    ];
    let gems = [
        "Garnet",
        "Ruby",
        "Topaz",
        "Emerald",
        "Onyx",
        "Opal",
        "Tourmaline",
        "Sapphire",
        "Amethyst",
        "Quartz",
        "Agate",
        "Diamond",
    ];
    ItemMappings {
        scrolls: std::array::from_fn(|index| {
            let ordinal = appearances.scroll_labels[index] as u16;
            ItemMapping {
                name: scrolls[index],
                appearance: runes[usize::from(ordinal)],
                sprite_index: 304 + ordinal,
            }
        }),
        potions: std::array::from_fn(|index| {
            let ordinal = appearances.potion_colors[index] as u16;
            ItemMapping {
                name: potions[index],
                appearance: colors[usize::from(ordinal)],
                sprite_index: 352 + ordinal,
            }
        }),
        rings: std::array::from_fn(|index| {
            let definition = ITEMS
                .iter()
                .find(|entry| entry.ring_kind().is_some_and(|kind| kind as usize == index))
                .expect("every ring class has a catalog entry");
            let ordinal = appearances.ring_gems.ordinals()[index];
            ItemMapping {
                name: definition.name,
                appearance: gems[usize::from(ordinal)],
                sprite_index: RING_SPRITE_BASE + u16::from(ordinal),
            }
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mappings_name_all_java_oracle_appearances() {
        // ABC-DEF-GHI, the run initialization fixture in run.rs.
        let mappings = item_mappings(DungeonSeed::new(8_687_205_886).unwrap());
        for (entries, names, ordinals, base) in [
            (
                &mappings.scrolls,
                [
                    "TIWAZ", "YNGVI", "MANNAZ", "ODAL", "BERKANAN", "ISAZ", "RAIDO", "KAUNAN",
                    "SOWILO", "NAUDIZ", "LAGUZ", "GYFU",
                ],
                [11, 3, 7, 10, 9, 6, 5, 0, 1, 8, 2, 4],
                304,
            ),
            (
                &mappings.potions,
                [
                    "Silver",
                    "Charcoal",
                    "Azure",
                    "Crimson",
                    "Turquoise",
                    "Ivory",
                    "Magenta",
                    "Golden",
                    "Bistre",
                    "Jade",
                    "Indigo",
                    "Amber",
                ],
                [10, 9, 5, 0, 4, 11, 7, 2, 8, 3, 6, 1],
                352,
            ),
            (
                &mappings.rings,
                [
                    "Sapphire",
                    "Emerald",
                    "Quartz",
                    "Garnet",
                    "Topaz",
                    "Ruby",
                    "Opal",
                    "Onyx",
                    "Amethyst",
                    "Diamond",
                    "Tourmaline",
                    "Agate",
                ],
                [7, 3, 9, 0, 2, 1, 5, 4, 8, 11, 6, 10],
                224,
            ),
        ] {
            assert_eq!(entries.each_ref().map(|entry| entry.appearance), names);
            assert_eq!(
                entries.each_ref().map(|entry| entry.sprite_index),
                ordinals.map(|ordinal| base + ordinal)
            );
        }
        assert_eq!(mappings.scrolls[0].name, "Scroll of upgrade");
        assert_eq!(mappings.potions[1].name, "Potion of healing");
        assert_ne!(mappings, item_mappings(DungeonSeed::MIN));
    }
}
