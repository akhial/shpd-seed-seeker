//! Shared, generation-only item inspection data for every map client.
use super::item_text::ITEM_TEXT;
use super::{LevelMap, MapItem};

#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "json-query", derive(serde::Serialize))]
#[cfg_attr(feature = "json-query", serde(rename_all = "camelCase"))]
pub struct MapItemTooltip {
    pub cell: usize,
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
    pub quantity: i32,
    pub deterministic: bool,
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
            quantity: item.quantity,
            deterministic: item.deterministic,
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
                (heap.cell, label, &heap.items)
            })
            .chain(
                self.contents
                    .mobs
                    .iter()
                    .filter(|mob| !mob.items.is_empty())
                    .map(|mob| (mob.cell, words(&mob.kind), &mob.items)),
            );
        for (cell, label, items) in sources {
            if items.is_empty() {
                continue;
            }
            if let Some(tip) = tips.iter_mut().find(|tip| tip.cell == cell) {
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
}
