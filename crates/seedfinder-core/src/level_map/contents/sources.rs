use super::{MapContents, MapFeature, MapHeap, MapItem, items, name};
use crate::quest_rooms::QuestPaintEvent as Q;
use crate::run::ItemAppearanceState;
use crate::secret_rooms::{SecretPaintEvent as S, SecretPlantKind};
use crate::special_consumable::{
    ConsumableBlobKind as B, ConsumablePaintEvent as C, ConsumablePlantKind, WellWaterKind,
};
use crate::special_equipment::{SpecialMobKind, SpecialPaintEvent as E};
use crate::special_forced::ForcedPaintEvent as F;

/// Borrowed only at the observation boundary: search allocates no map objects.
pub(crate) enum FloorSource<'a> {
    Sewer(&'a crate::sewer_floor::GeneratedSewerFloor),
    Prison(&'a crate::prison_floor::GeneratedPrisonFloor),
    Caves(&'a crate::caves_floor::GeneratedCavesFloor),
    City(&'a crate::city_floor::GeneratedCityFloor),
    Halls(&'a crate::halls_floor::GeneratedHallsFloor),
}

impl FloorSource<'_> {
    pub(crate) fn collect(&self, a: &ItemAppearanceState, stealthy: bool) -> MapContents {
        macro_rules! collect {
            ($floor:expr, $quest:expr) => {{
                let floor = $floor;
                let p = &floor.painted;
                let mut out = MapContents::from_level(&p.level, a);
                out.events(
                    &p.equipment_events,
                    &p.consumable_events,
                    &p.forced_events,
                    &p.secret_events,
                    $quest,
                    &p.level,
                    a,
                );
                for m in &floor.mobs.mobs {
                    out.mob(m.cell, name(m.mob.kind), vec![]);
                }
                let order: Vec<_> = p
                    .level
                    .paint_events
                    .iter()
                    .filter_map(|e| {
                        if let crate::level::PaintEvent::RoomPaint(id) = e {
                            Some(*id)
                        } else {
                            None
                        }
                    })
                    .collect();
                out.heaps.sort_by_key(|heap| {
                    order
                        .iter()
                        .position(|&id| p.rooms[id].inside(p.level.map.cell_to_point(heap.cell)))
                });
                out.regular(&floor.regular_items.placements, a);
                out.ebony_position(&p.level, &p.rooms, &floor.regular_items.isolated_streams);
                out.mask_runtime_room_items(&p.level, &p.rooms);
                for mob in &mut out.mobs {
                    if mob.kind.ends_with("Mimic") {
                        mob.stealthy |= stealthy;
                    }
                }
                out
            }};
        }
        match self {
            Self::Sewer(f) => {
                let mut out = collect!(f, &[]);
                if let Some(cell) = f.mobs.ghost_cell {
                    out.mob(cell, "Ghost", vec![]);
                }
                out
            }
            Self::Prison(f) => {
                let mut out = collect!(f, &f.painted.quest_events);
                if let Some(cell) = f.mobs.wandmaker_cell {
                    out.mob(cell, "Wandmaker", vec![]);
                }
                out
            }
            Self::Caves(f) => collect!(f, &f.painted.quest_events),
            Self::City(f) => collect!(f, &f.painted.quest_events),
            Self::Halls(f) => collect!(f, &f.painted.quest_events),
        }
    }
}

impl MapContents {
    #[allow(clippy::too_many_arguments, clippy::too_many_lines)] // Exhaustive adapters for the independent generation event logs.
    fn events(
        &mut self,
        equipment: &[E],
        consumables: &[C],
        forced: &[F],
        secret: &[S],
        quest: &[Q],
        level: &crate::level::Level,
        a: &ItemAppearanceState,
    ) {
        for e in equipment {
            match e {
                E::Drop { cell, heap, reward } => {
                    self.drop(*cell, &name(heap), false, items::special(reward.item, a));
                }
                E::Mob {
                    cell,
                    kind,
                    carried,
                } => self.mob(
                    *cell,
                    match kind {
                        SpecialMobKind::Sentry { .. } => "Sentry".into(),
                        SpecialMobKind::CrystalMimic { .. } => "CrystalMimic".into(),
                        k => name(k),
                    },
                    carried.iter().map(|r| items::special(r.item, a)).collect(),
                ),
                E::Blob { cell, kind, .. } => self.effect(*cell, name(kind)),
                E::Feature { point, kind } => self.features.push(MapFeature {
                    cycle: None,
                    cell: level.map.point_to_cell(*point),
                    width: 1,
                    height: 1,
                    kind: name(kind),
                }),
                E::SpawnItem(_) => {} // Later placement is in RegularItemsResult.
            }
        }
        for e in consumables {
            match e {
                C::Drop {
                    cell, heap, reward, ..
                } => self.drop(
                    *cell,
                    &heap.map_or_else(|| "Heap".into(), name),
                    false,
                    items::consumable(reward.item, a),
                ),
                C::Mimic { cell, carried } => self.mob(
                    *cell,
                    "Mimic",
                    carried
                        .iter()
                        .map(|r| items::consumable(r.item, a))
                        .collect(),
                ),
                C::Plant { cell, kind } if level.plants_enabled => self.plant(
                    *cell,
                    name(kind),
                    match kind {
                        ConsumablePlantKind::Sungrass => 3,
                        ConsumablePlantKind::Blandfruit => 12,
                    },
                ),
                C::Blob { cell, kind, .. } => self.effect(
                    *cell,
                    match kind {
                        B::WellWater(WellWaterKind::Health) => "WaterOfHealth".into(),
                        B::WellWater(WellWaterKind::Awareness) => "WaterOfAwareness".into(),
                        k => name(k),
                    },
                ),
                C::Trap {
                    cell,
                    kind,
                    visible,
                    active,
                } => self.trap(*cell, name(kind), *visible, *active),
                C::SpawnItem(_) | C::Plant { .. } => {}
            }
        }
        for e in forced {
            match e {
                F::Drop {
                    cell,
                    heap,
                    haunted,
                    reward,
                } => self.drop(*cell, &name(heap), *haunted, items::forced(&reward.item, a)),
                F::Mob { cell, kind } => self.mob(*cell, name(kind), vec![]),
                F::Blob { cell, kind, .. } => self.effect(*cell, name(kind)),
                F::SpawnItem(_) | F::PitFallCandidates(_) => {}
            }
        }
        for e in secret {
            match e {
                S::Drop {
                    cell,
                    heap,
                    haunted,
                    reward,
                } => self.drop(*cell, &name(heap), *haunted, items::secret(reward.item, a)),
                S::Mob { cell, kind } => self.mob(*cell, name(kind), vec![]),
                S::Plant { cell, kind } if level.plants_enabled => self.plant(
                    *cell,
                    name(kind),
                    match kind {
                        SecretPlantKind::Starflower => 9,
                        SecretPlantKind::Seedpod => 14,
                        SecretPlantKind::Dewcatcher => 13,
                        SecretPlantKind::BlandfruitBush => 12,
                    },
                ),
                S::Blob { cell, kind, .. } => self.effect(*cell, name(kind)),
                S::Trap {
                    cell,
                    kind,
                    visible,
                    active,
                } => self.trap(*cell, name(kind), *visible, *active),
                S::SpawnItem(_) | S::Plant { .. } => {}
            }
        }
        for e in quest {
            match e {
                Q::Drop {
                    cell,
                    heap,
                    haunted,
                    item,
                } => self.drop(*cell, &name(heap), *haunted, items::regular(*item, a)),
                Q::Mob { cell, kind } => self.mob(*cell, name(kind), vec![]),
                Q::Feature {
                    point,
                    width,
                    height,
                    kind,
                } => self.features.push(MapFeature {
                    cycle: None,
                    cell: level.map.point_to_cell(*point),
                    width: *width,
                    height: *height,
                    kind: name(kind),
                }),
                Q::Trap {
                    cell,
                    kind,
                    visible,
                    active,
                } => self.trap(*cell, name(kind), *visible, *active),
                Q::SpawnItem(_) | Q::Transition(_) => {}
            }
        }
    }
    fn regular(
        &mut self,
        placements: &[crate::regular_items::RegularItemPlacementRecord],
        a: &ItemAppearanceState,
    ) {
        use crate::regular_items::RegularItemDestination as D;
        for p in placements {
            let Ok(cell) = usize::try_from(p.cell) else {
                continue;
            };
            match p.destination {
                D::Heap(kind) => {
                    for i in &p.items {
                        self.drop(cell, &name(kind), false, items::regular(*i, a));
                    }
                }
                D::Mimic(kind) => self.mob(
                    cell,
                    name(kind),
                    p.items.iter().map(|i| items::regular(*i, a)).collect(),
                ),
                D::NonPrimaryDrop => {} // E.g. a chasm drop; not a heap on this floor.
            }
        }
    }
    fn ebony_position(
        &mut self,
        level: &crate::level::Level,
        rooms: &[crate::room::Room],
        streams: &[crate::regular_items::IsolatedItemStream],
    ) {
        use crate::regular_items::IsolatedItemStreamKind;
        let Some(mimic) = self.mobs.iter().position(|m| m.kind == "EbonyMimic") else {
            return;
        };
        let seed = streams
            .iter()
            .find(|s| s.kind == IsolatedItemStreamKind::EbonyMimic)
            .unwrap()
            .seed;
        let mut random = crate::rng::RandomStack::with_base_seed(0);
        random.push(seed);
        random.float(); // The spawn-chance roll, already known to have succeeded.
        if random.int_bound(2) != 0 {
            return;
        } // Door/exit placement is already exact.
        let candidates: Vec<_> =
            crate::grid_builder::sparse_array_order(self.heaps.iter().map(|h| h.cell))
                .into_iter()
                .filter(|&cell| {
                    self.heaps
                        .iter()
                        .any(|h| h.cell == cell && h.kind == "Heap")
                        && !self
                            .mobs
                            .iter()
                            .enumerate()
                            .any(|(i, m)| i != mimic && m.cell == cell)
                        && !rooms.iter().any(|r| {
                            r.inside(level.map.cell_to_point(cell))
                                && matches!(
                                    r.kind,
                                    crate::room::RoomKind::Special(_)
                                        | crate::room::RoomKind::Secret(_)
                                        | crate::room::RoomKind::Quest(
                                            crate::room::QuestRoomKind::MassGrave
                                                | crate::room::QuestRoomKind::RotGarden
                                                | crate::room::QuestRoomKind::AmbitiousImp
                                        )
                                )
                        })
                })
                .collect();
        if !candidates.is_empty() {
            self.mobs[mimic].cell = candidates[usize::try_from(
                random.int_bound(i32::try_from(candidates.len()).unwrap()),
            )
            .unwrap()];
        }
    }

    fn mask_runtime_room_items(
        &mut self,
        level: &crate::level::Level,
        rooms: &[crate::room::Room],
    ) {
        use crate::room::{RoomKind, SecretRoomKind};
        for heap in &mut self.heaps {
            if let Some(room) = rooms
                .iter()
                .find(|room| room.inside(level.map.cell_to_point(heap.cell)))
            {
                let placeholder = match room.kind {
                    RoomKind::Secret(SecretRoomKind::Laboratory) => {
                        Some(("RuntimeLaboratoryPotion", 10))
                    }
                    RoomKind::Secret(SecretRoomKind::Library) => Some(("RuntimeLibraryScroll", 12)),
                    _ => None,
                };
                if let Some((kind, image)) = placeholder {
                    for item in &mut heap.items {
                        if (304..336).contains(&item.image) || (352..384).contains(&item.image) {
                            *item = MapItem::unknown(kind, image);
                        }
                    }
                }
            }
        }
    }
    pub(crate) fn from_vault(
        vault: &crate::vault_floor::GeneratedVault,
        a: &ItemAppearanceState,
        rewards: &[crate::quests::ImpRewardOption],
    ) -> Self {
        let mut out = Self::from_level(&vault.level, a);
        // Flame-path groups overlap at corners; setupTrap overwrites the prior
        // cooldown arrays there, so the final assignment wins.
        let cycles: std::collections::BTreeMap<_, _> =
            vault.flame_cycles.iter().flatten().copied().collect();
        for (cell, cycle) in cycles {
            // Preserve the exact setupTrap turn schedule without advancing the game.
            out.features.push(MapFeature {
                cycle: Some(cycle),
                cell,
                kind: "VaultFlameTrap".to_owned(),
                width: 1,
                height: 1,
            });
        }
        for heap in &vault.heaps {
            out.heaps.push(MapHeap {
                cell: heap.cell,
                kind: name(heap.kind),
                haunted: false,
                items: heap
                    .items
                    .iter()
                    .map(|i| {
                        if let crate::vault_loot::VaultItem::ImpRewardOption(index) = i {
                            items::imp(rewards[usize::from(*index)], a)
                        } else {
                            items::vault(*i, a)
                        }
                    })
                    .collect(),
            });
        }
        for mob in &vault.mobs {
            out.mob(
                mob.cell,
                match mob.kind {
                    crate::vault_mobs::VaultMobKind::FireElemental
                    | crate::vault_mobs::VaultMobKind::FrostElemental
                    | crate::vault_mobs::VaultMobKind::ShockElemental => name(mob.kind),
                    k => format!("Vault{k:?}"),
                },
                vec![],
            );
        }
        for mob in &mut out.mobs {
            use crate::vault_rooms::VaultRoomKind as K;
            if vault.room_at(mob.cell).is_some_and(|id| {
                matches!(
                    vault.rooms[id].kind,
                    K::Ring
                        | K::Rings
                        | K::EnemyCenter
                        | K::Hallway
                        | K::LongRings
                        | K::Quadrants
                        | K::Tokens
                )
            }) {
                mob.sleeping = false;
            }
        }
        out
    }
}
