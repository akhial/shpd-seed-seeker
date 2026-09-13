//! Initial, generation-time objects. Collection never advances the generation RNG.
//! Future drops, rewards and respawns are deliberately not floor heaps.
mod items;
mod sources;

use crate::level::{Level, PaintMob};
use crate::run::ItemAppearanceState;
pub(crate) use sources::FloorSource;

#[derive(Clone, Debug, Default, Eq, PartialEq)]
#[cfg_attr(feature = "json-query", derive(serde::Serialize))]
pub struct MapContents {
    pub heaps: Vec<MapHeap>,
    pub mobs: Vec<MapMob>,
    pub plants: Vec<MapPlant>,
    pub effects: Vec<MapEffect>,
    pub features: Vec<MapFeature>,
    pub traps: Vec<super::MapTrap>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
#[cfg_attr(feature = "json-query", derive(serde::Serialize))]
pub struct MapItem {
    pub kind: String,
    pub image: u16,
    pub quantity: i32,
    /// False means the identity depends on play or unseeded runtime state.
    pub deterministic: bool,
    #[cfg_attr(feature = "json-query", serde(skip_serializing_if = "Option::is_none"))]
    pub glow: Option<super::MapGlow>,
}
impl MapItem {
    pub(crate) fn new(kind: impl Into<String>, image: u16, quantity: i32) -> Self {
        Self {
            kind: kind.into(),
            image,
            quantity,
            deterministic: true,
            glow: None,
        }
    }
    fn unknown(kind: &str, image: u16) -> Self {
        Self {
            deterministic: false,
            ..Self::new(kind, image, 1)
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
#[cfg_attr(feature = "json-query", derive(serde::Serialize))]
pub struct MapHeap {
    pub cell: usize,
    pub kind: String,
    pub haunted: bool,
    /// Top item first, matching `Heap.peek()`. Containers keep their contents.
    pub items: Vec<MapItem>,
}
#[derive(Clone, Debug, Eq, PartialEq)]
#[cfg_attr(feature = "json-query", derive(serde::Serialize))]
pub struct MapMob {
    /// Generation-time disguise; Ebony mimics always use advanced hiding.
    pub stealthy: bool,
    pub sleeping: bool,
    /// Runtime-created partner: this adjacent position is a scouting approximation.
    pub approximate: bool,
    pub cell: usize,
    pub kind: String,
    /// Only inventory rolled during generation; never prospective death loot.
    pub items: Vec<MapItem>,
}
#[derive(Clone, Debug, Eq, PartialEq)]
#[cfg_attr(feature = "json-query", derive(serde::Serialize))]
pub struct MapPlant {
    pub cell: usize,
    pub kind: String,
    pub image: u16,
}
#[derive(Clone, Debug, Eq, PartialEq)]
#[cfg_attr(feature = "json-query", derive(serde::Serialize))]
pub struct MapEffect {
    pub cell: usize,
    pub kind: String,
}
#[derive(Clone, Debug, Eq, PartialEq)]
#[cfg_attr(feature = "json-query", derive(serde::Serialize))]
pub struct MapFeature {
    pub cell: usize,
    pub width: i32,
    pub height: i32,
    pub kind: String,
}

pub(super) fn name(value: impl std::fmt::Debug) -> String {
    format!("{value:?}")
        .split([' ', '{', '('])
        .next()
        .unwrap()
        .to_owned()
}

impl MapContents {
    pub(crate) fn from_level(level: &Level, appearance: &ItemAppearanceState) -> Self {
        let mut result = Self::default();
        for heap in &level.heaps {
            for item in &heap.items {
                result.drop(
                    heap.cell,
                    &name(heap.kind),
                    false,
                    items::paint(*item, appearance),
                );
            }
        }
        for mob in &level.mobs {
            match mob {
                PaintMob::Initial { cell, kind } => result.mob(*cell, *kind, vec![]),
                PaintMob::Piranha { cell, phantom } => result.mob(
                    *cell,
                    if *phantom {
                        "PhantomPiranha"
                    } else {
                        "Piranha"
                    },
                    vec![],
                ),
                PaintMob::Mimic { cell, items } => result.mob(
                    *cell,
                    "Mimic",
                    items.iter().map(|i| items::paint(*i, appearance)).collect(),
                ),
            }
        }
        for plant in &level.plants {
            result.plant(
                plant.cell,
                name(plant.seed),
                super::visuals::plant_image(plant.seed),
            );
        }
        result
    }

    fn drop(&mut self, cell: usize, kind: &str, haunted: bool, item: MapItem) {
        let index = self
            .heaps
            .iter()
            .position(|heap| heap.cell == cell)
            .unwrap_or_else(|| {
                self.heaps.push(MapHeap {
                    cell,
                    kind: kind.to_owned(),
                    haunted,
                    items: vec![],
                });
                self.heaps.len() - 1
            });
        let heap = &mut self.heaps[index];
        if kind != "Heap" {
            kind.clone_into(&mut heap.kind);
        }
        heap.haunted |= haunted;
        // Heap.drop merges stackable items, removes the old entry, then puts
        // the resulting stack on top (except explicit dropsDownHeap items).
        let stackable = matches!(
            item.kind.as_str(),
            "Gold"
                | "DarkGold"
                | "EnergyCrystal"
                | "Dewdrop"
                | "DwarfToken"
                | "Torch"
                | "IronKey"
                | "GoldenKey"
                | "CrystalKey"
                | "CeremonialCandle"
                | "AlchemyPage"
                | "Alchemize"
                | "Dart"
                | "Ration"
                | "Pasty"
                | "MysteryMeat"
                | "SmallRation"
                | "ChargrilledMeat"
        ) || item.kind.contains("PotionOf")
            || item.kind.contains("ScrollOf")
            || item.kind.starts_with("StoneOf")
            || item.kind.ends_with("Seed")
            || crate::catalog::item_by_stable_id(&item.kind).is_some_and(|d| {
                d.weapon_category() == Some(crate::catalog::WeaponCategory::Thrown)
            });
        let mut item = item;
        if stackable
            && kind != "ForSale"
            && let Some(index) = heap
                .items
                .iter()
                .position(|i| i.kind == item.kind && i.image == item.image)
        {
            item.quantity += heap.items.remove(index).quantity;
        }
        if matches!(item.kind.as_str(), "Dewdrop" | "EnergyCrystal") && kind != "ForSale" {
            heap.items.push(item);
        } else {
            heap.items.insert(0, item);
        }
    }

    pub(crate) fn mob(&mut self, cell: usize, kind: impl Into<String>, items: Vec<MapItem>) {
        self.mobs.retain(|mob| mob.cell != cell);
        let kind = kind.into();
        self.mobs.push(MapMob {
            stealthy: kind == "EbonyMimic",
            sleeping: !kind.ends_with("Mimic")
                && !matches!(
                    kind.as_str(),
                    "Bee"
                        | "GnollExile"
                        | "FungalSentry"
                        | "VaultSentry"
                        | "VaultLaser"
                        | "Ghost"
                        | "Wandmaker"
                        | "Blacksmith"
                        | "Shopkeeper"
                        | "Imp"
                        | "Statue"
                        | "ArmoredStatue"
                        | "Sentry"
                        | "Pylon"
                        | "RotHeart"
                        | "RotLasher"
                        | "DemonSpawner"
                        | "VaultMirror"
                        | "VaultTokenDoor"
                        | "FetidRat"
                        | "Wraith"
                        | "CrystalSpire"
                        | "BlueCrystalSpire"
                        | "GreenCrystalSpire"
                        | "RedCrystalSpire"
                ),
            approximate: false,
            cell,
            kind,
            items,
        });
    }
    fn trap(&mut self, cell: usize, kind: String, visible: bool, active: bool) {
        self.traps.retain(|t| t.cell != cell);
        self.traps.push(super::MapTrap {
            cell,
            kind,
            hidden: !visible,
            active,
        });
    }
    fn plant(&mut self, cell: usize, kind: impl Into<String>, image: u16) {
        self.plants.retain(|plant| plant.cell != cell);
        self.plants.push(MapPlant {
            cell,
            kind: kind.into(),
            image,
        });
    }
    fn effect(&mut self, cell: usize, kind: impl Into<String>) {
        let effect = MapEffect {
            cell,
            kind: kind.into(),
        };
        if !self.effects.contains(&effect) {
            self.effects.push(effect);
        }
    }
    pub(crate) fn add_ghoul_partners(&mut self, level: &Level) {
        // Ghoul.act creates its partner with runtime RNG. Use a stable adjacent
        // tile solely for scouting, preferring clear ground. VaultGhoul has no partner.
        let parents: Vec<_> = self
            .mobs
            .iter()
            .filter(|m| m.kind == "Ghoul")
            .cloned()
            .collect();
        for parent in parents {
            let point = level.map.cell_to_point(parent.cell);
            let cell = [(1, 0), (-1, 0), (0, 1), (0, -1)]
                .into_iter()
                .filter_map(|(x, y)| {
                    let p = crate::geometry::Point::new(point.x + x, point.y + y);
                    if p.x < 0 || p.y < 0 || p.x >= level.width() || p.y >= level.height() {
                        return None;
                    }
                    let cell = level.map.point_to_cell(p);
                    (level.passable[cell] && !self.mobs.iter().any(|m| m.cell == cell))
                        .then_some(cell)
                })
                .min_by_key(|cell| {
                    (
                        self.heaps.iter().any(|h| h.cell == *cell),
                        level.map.cells[*cell] == crate::geometry::terrain::HIGH_GRASS,
                    )
                });
            if let Some(cell) = cell {
                self.mobs.push(MapMob {
                    cell,
                    approximate: true,
                    ..parent
                });
            }
        }
    }
    pub(crate) fn normalize(&mut self) {
        self.heaps.sort_by_key(|heap| heap.cell);
        self.mobs.sort_by_key(|mob| mob.cell);
        self.plants.sort_by_key(|plant| plant.cell);
        self.effects
            .sort_by(|a, b| (a.cell, &a.kind).cmp(&(b.cell, &b.kind)));
        self.features.sort_by_key(|feature| feature.cell);
    }
}
