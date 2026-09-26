//! Shared, generation-only item inspection data for every map client.
use super::item_text::{ITEM_ICONS, ITEM_TEXT};
use super::{LevelMap, MapItem};

#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "json-query", derive(serde::Serialize))]
#[cfg_attr(feature = "json-query", serde(rename_all = "camelCase"))]
pub struct MapItemTooltip {
    pub cell: usize,
    /// Sprite bounds relative to its tile, in map pixels: x, y, width, height.
    pub bounds: [i32; 4],
    /// Empty for loose items; otherwise the container or inventory owner.
    pub label: String,
    pub hidden: bool,
    pub items: Vec<MapTooltipItem>,
}

#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "json-query", derive(serde::Serialize))]
pub struct MapTooltipItem {
    pub name: String,
    pub description: &'static str,
    pub image: u16,
    /// Type glyph source rectangle in `item_icons.png`, independent of appearance.
    pub icon: Option<[u16; 4]>,
    pub quantity: i32,
    pub deterministic: bool,
    /// Identified in-game level. None for items without generated equipment properties.
    pub upgrade: Option<u8>,
    pub cursed: bool,
    /// Weapon enchantment or armor glyph, using the game's display name.
    pub enchantment: Option<&'static str>,
    /// Named weapon/armor curse, separate from the item's cursed flag.
    pub curse: Option<&'static str>,
    /// The same color and half-cycle duration used by the map sprite.
    pub glow: Option<super::MapGlow>,
}

fn words(kind: &str) -> String {
    let mut result = String::new();
    for ch in kind.chars() {
        if ch == '_' || ch == '$' {
            result.push(' ');
        } else {
            if ch.is_uppercase() && !result.is_empty() && !result.ends_with(' ') {
                result.push(' ');
            }
            result.push(ch);
        }
    }
    if let Some(first) = result.get_mut(..1) {
        first.make_ascii_uppercase();
    }
    result
}

impl MapTooltipItem {
    fn from_item(item: &MapItem) -> Self {
        let key: String = item
            .kind
            .chars()
            .filter(|c| c.is_ascii_alphanumeric() || *c == '$')
            .map(|c| c.to_ascii_lowercase())
            .collect();
        let text = ITEM_TEXT
            .binary_search_by_key(&key.as_str(), |entry| entry.0)
            .ok()
            .map(|index| ITEM_TEXT[index]);
        Self {
            name: text.map_or_else(
                || match item.kind.as_str() {
                    "RuntimeVaultConsumable" => "Random consumable".into(),
                    "ImpRewardOption" => "Imp reward".into(),
                    _ => words(&item.kind),
                },
                |entry| entry.1.to_owned(),
            ),
            description: text.map_or("", |entry| entry.2),
            image: item.image,
            icon: ITEM_ICONS
                .binary_search_by_key(&key.as_str(), |entry| entry.0)
                .ok()
                .map(|index| ITEM_ICONS[index].1),
            quantity: item.quantity,
            deterministic: item.deterministic,
            upgrade: item.roll.map(|roll| {
                crate::catalog::item_by_stable_id(&item.kind).map_or(roll.upgrade, |definition| {
                    crate::model::displayed_upgrade(definition.id, roll.upgrade)
                })
            }),
            cursed: item.roll.is_some_and(|roll| roll.cursed),
            enchantment: item
                .roll
                .and_then(|roll| roll.effect)
                .filter(|effect| !effect.is_curse())
                .map(crate::catalog::Effect::wire_name),
            curse: item
                .roll
                .and_then(|roll| roll.effect)
                .filter(|effect| effect.is_curse())
                .map(crate::catalog::Effect::wire_name),
            glow: item.glow,
        }
    }
}

impl LevelMap {
    /// Metadata only: never advances RNG or changes the rendered scene.
    ///
    /// # Panics
    /// Panics if a manually constructed map contains a cell outside the i32
    /// range or has zero width. Generated maps always have valid dimensions.
    #[must_use]
    pub fn item_tooltips(&self) -> Vec<MapItemTooltip> {
        let mut tips: Vec<MapItemTooltip> = Vec::new();
        let sources = self
            .contents
            .heaps
            .iter()
            .map(|heap| {
                let label = match heap.kind.as_str() {
                    "Heap" => String::new(),
                    "ForSale" => "For sale".into(),
                    other => words(other),
                };
                let label = if heap.phantom {
                    format!(
                        "Phantom{sep}{label}",
                        sep = if label.is_empty() { "" } else { " · " }
                    )
                } else {
                    label
                };
                (
                    heap.cell,
                    label,
                    &heap.items,
                    super::visuals::heap_bounds(heap),
                )
            })
            .chain(
                self.contents
                    .mobs
                    .iter()
                    .filter(|mob| !mob.items.is_empty())
                    .map(|mob| {
                        (
                            mob.cell,
                            words(&mob.kind),
                            &mob.items,
                            super::visuals::mob_bounds(&mob.kind),
                        )
                    }),
            );
        for (cell, label, items, bounds) in sources {
            if items.is_empty() {
                continue;
            }
            if let Some(tip) = tips.iter_mut().find(|tip| tip.cell == cell) {
                let [x, y, w, h] = tip.bounds;
                let [bx, by, bw, bh] = bounds;
                let left = x.min(bx);
                let top = y.min(by);
                tip.bounds = [
                    left,
                    top,
                    (x + w).max(bx + bw) - left,
                    (y + h).max(by + bh) - top,
                ];
                tip.items
                    .extend(items.iter().map(MapTooltipItem::from_item));
                if !label.is_empty() {
                    if !tip.label.is_empty() {
                        tip.label.push_str(" · ");
                    }
                    tip.label.push_str(&label);
                }
                continue;
            }
            let cell = i32::try_from(cell).expect("map cell fits i32");
            let x = cell % self.width;
            let y = cell / self.width;
            tips.push(MapItemTooltip {
                cell: usize::try_from(cell).expect("map cell is nonnegative"),
                label,
                bounds,
                hidden: self.secret_rooms.iter().any(|&[left, top, right, bottom]| {
                    x > left && x < right && y > top && y < bottom
                }),
                items: items.iter().map(MapTooltipItem::from_item).collect(),
            });
        }
        tips.sort_by_key(|tip| tip.cell);
        tips
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::catalog::ITEMS;

    #[test]
    fn inspection_keeps_stack_order_quantities_and_secret_visibility_without_changing_scene() {
        use crate::level_map::{MapHeap, generate_level_map};
        use crate::{challenges::Challenges, seed::DungeonSeed};
        let mut map = generate_level_map(
            DungeonSeed::from_code("AAA-AAA-AAA").unwrap(),
            1,
            Challenges::NONE,
            None,
        )
        .unwrap();
        let scene = map.scene.clone();
        let cell = usize::try_from(map.width + 1).unwrap();
        map.secret_rooms = vec![[0, 0, 3, 3]];
        map.contents.heaps = vec![MapHeap {
            cell,
            kind: "LockedChest".into(),
            haunted: false,
            phantom: false,
            items: vec![
                MapItem::new("PotionOfHealing", 352, 2),
                MapItem::new("Gold", 18, 40),
            ],
        }];
        map.contents.mobs.clear();
        let tips = map.item_tooltips();
        assert_eq!(tips.len(), 1);
        assert_eq!(tips[0].label, "Locked Chest");
        assert_eq!(tips[0].bounds, [0, -3, 16, 14]);
        assert!(tips[0].hidden);
        assert_eq!(tips[0].items[0].name, "Potion of healing");
        assert_eq!(tips[0].items[0].quantity, 2);
        assert_eq!(tips[0].items[1].quantity, 40);
        assert!(tips[0].items[0].description.starts_with("This elixir"));
        assert_eq!(map.scene, scene);
        let document = crate::level_map::json::document(&map);
        assert_eq!(
            document["itemTooltips"][0]["items"][0]["name"],
            "Potion of healing"
        );
    }

    #[test]
    fn all_catalog_items_have_original_descriptions() {
        for item in ITEMS {
            let tip = MapTooltipItem::from_item(&MapItem::new(item.stable_id, 0, 1));
            assert!(!tip.description.is_empty(), "{}", item.stable_id);
            assert!(!tip.description.contains("%s"));
            assert!(!tip.description.contains("%d"));
        }
    }

    #[test]
    fn descriptions_resolve_java_aliases_and_seeded_exotic_identities() {
        for (kind, name) in [
            ("Ration", "Ration of food"),
            ("ExoticPotionOfStrength", "Potion of mastery"),
            ("ExoticScrollOfIdentify", "Scroll of divination"),
            ("FirebloomSeed", "Seed of firebloom"),
            ("HourglassSandBag", "Bag of magic sand"),
        ] {
            let tip = MapTooltipItem::from_item(&MapItem::new(kind, 0, 3));
            assert_eq!(tip.name, name);
            assert!(!tip.description.is_empty());
            assert_eq!(tip.quantity, 3);
        }
        let tip = MapTooltipItem::from_item(&MapItem::new("FutureItem", 0, 1));
        assert_eq!(tip.name, "Future Item");
        assert!(tip.description.is_empty());
    }

    #[test]
    fn identity_glyphs_follow_type_including_exotics_not_randomized_appearance() {
        for (kind, icon) in [
            ("ring_might", [64, 0, 7, 7]),
            ("PotionOfFrost", [24, 40, 7, 7]),
            ("ExoticPotionOfFrost", [24, 48, 7, 7]),
            ("PotionOfSnapFreeze", [24, 48, 7, 7]),
            ("ScrollOfIdentify", [8, 16, 4, 7]),
            ("ExoticScrollOfIdentify", [8, 24, 7, 6]),
            ("ScrollOfDivination", [8, 24, 7, 6]),
        ] {
            for image in [224, 235, 304, 315, 352, 363] {
                let tip = MapTooltipItem::from_item(&MapItem::new(kind, image, 1));
                assert_eq!(tip.icon, Some(icon), "{kind}");
                assert_eq!(tip.image, image);
            }
        }
        assert!(
            MapTooltipItem::from_item(&MapItem::new("Gold", 18, 1))
                .icon
                .is_none()
        );
        let unique: std::collections::HashSet<_> = ITEM_ICONS.iter().map(|entry| entry.1).collect();
        assert_eq!(
            unique.len(),
            60,
            "12 rings and 24 each of potions and scrolls"
        );
    }
    #[test]
    fn inspection_reports_identified_levels_effects_curses_and_shared_glows() {
        use crate::catalog::{ArmorEffect, Effect, ItemId, WeaponEffect, item};
        use crate::equipment::EquipmentRoll;
        for (id, effect, cursed, enchantment, curse, period) in [
            (
                ItemId::Shortsword,
                Some(Effect::Weapon(WeaponEffect::Shocking)),
                true,
                Some("Shocking"),
                None,
                Some(500),
            ),
            (
                ItemId::LeatherArmor,
                Some(Effect::Armor(ArmorEffect::Potential)),
                false,
                Some("Potential"),
                None,
                Some(600),
            ),
            (
                ItemId::Shortsword,
                Some(Effect::Weapon(WeaponEffect::Wayward)),
                true,
                None,
                Some("Wayward"),
                Some(1000),
            ),
            (ItemId::RingMight, None, true, None, None, Some(1000)),
            (ItemId::WandFireblast, None, true, None, None, None),
        ] {
            let definition = item(id);
            let mut source = MapItem::new(definition.stable_id, definition.sprite_index, 1);
            source.roll = Some(EquipmentRoll {
                upgrade: 2,
                effect,
                cursed,
            });
            source.glow = super::super::MapGlow::for_item(definition.kind, effect, cursed);
            let tip = MapTooltipItem::from_item(&source);
            assert_eq!(tip.upgrade, Some(2));
            assert_eq!(tip.cursed, cursed);
            assert_eq!(tip.enchantment, enchantment);
            assert_eq!(tip.curse, curse);
            assert_eq!(tip.glow, source.glow);
            assert_eq!(tip.glow.map(|glow| glow.period_ms), period);
        }
        for (id, expected) in [(ItemId::SandalsOfNature, 7), (ItemId::EtherealChains, 6)] {
            let definition = item(id);
            let mut source = MapItem::new(definition.stable_id, definition.sprite_index, 1);
            source.roll = Some(EquipmentRoll {
                upgrade: 5,
                effect: None,
                cursed: false,
            });
            assert_eq!(MapTooltipItem::from_item(&source).upgrade, Some(expected));
        }
        let unknown = MapTooltipItem::from_item(&MapItem {
            deterministic: false,
            ..MapItem::new("RuntimeVaultConsumable", 0, 1)
        });
        assert_eq!(unknown.upgrade, None);
        assert!(!unknown.cursed);
        assert_eq!(unknown.enchantment, None);
        assert_eq!(unknown.curse, None);
        assert_eq!(unknown.glow, None);
    }
}
