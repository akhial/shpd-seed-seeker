//! Exact-floor feelings and room-presence requirements.
//! Room IDs are portable; masks are compiled once and generation retains only presence.
use crate::level_prelude::Feeling;
use crate::model::GeneratedWorld;
use crate::room::{
    ConnectionRoomKind, QuestRoomKind, Room, RoomKind, SecretRoomKind, SpecialRoomKind,
    StandardRoomKind,
};

// Append only: discriminants are used by share links and probability tables.
macro_rules! room_types {
    ($($variant:ident => $name:literal, $kind:pat,)*) => {
        #[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
        #[cfg_attr(feature = "json-query", derive(serde::Serialize, serde::Deserialize))]
        #[repr(u8)]
        pub enum RoomType {
            $(#[cfg_attr(feature = "json-query", serde(rename = $name))] $variant,)*
        }
        impl RoomType {
            pub const ALL: &'static [Self] = &[$(Self::$variant,)*];
            #[must_use]
            pub const fn stable_id(self) -> &'static str {
                match self { $(Self::$variant => $name,)* }
            }
            #[must_use]
            pub const fn from_kind(kind: RoomKind) -> Self {
                match kind { $($kind => Self::$variant,)* }
            }
        }
    }
}
room_types! {
    StandardSewerPipe => "standard_sewer_pipe", RoomKind::Standard(StandardRoomKind::SewerPipe),
    StandardRing => "standard_ring", RoomKind::Standard(StandardRoomKind::Ring),
    StandardWaterBridge => "standard_water_bridge", RoomKind::Standard(StandardRoomKind::WaterBridge),
    StandardRegionDecoPatch => "standard_region_deco_patch", RoomKind::Standard(StandardRoomKind::RegionDecoPatch),
    StandardCircleBasin => "standard_circle_basin", RoomKind::Standard(StandardRoomKind::CircleBasin),
    StandardRegionDecoLine => "standard_region_deco_line", RoomKind::Standard(StandardRoomKind::RegionDecoLine),
    StandardSegmented => "standard_segmented", RoomKind::Standard(StandardRoomKind::Segmented),
    StandardPillars => "standard_pillars", RoomKind::Standard(StandardRoomKind::Pillars),
    StandardChasmBridge => "standard_chasm_bridge", RoomKind::Standard(StandardRoomKind::ChasmBridge),
    StandardCellBlock => "standard_cell_block", RoomKind::Standard(StandardRoomKind::CellBlock),
    StandardCave => "standard_cave", RoomKind::Standard(StandardRoomKind::Cave),
    StandardRegionDecoBridge => "standard_region_deco_bridge", RoomKind::Standard(StandardRoomKind::RegionDecoBridge),
    StandardCavesFissure => "standard_caves_fissure", RoomKind::Standard(StandardRoomKind::CavesFissure),
    StandardCirclePit => "standard_circle_pit", RoomKind::Standard(StandardRoomKind::CirclePit),
    StandardCircleWall => "standard_circle_wall", RoomKind::Standard(StandardRoomKind::CircleWall),
    StandardHallway => "standard_hallway", RoomKind::Standard(StandardRoomKind::Hallway),
    StandardLibraryHall => "standard_library_hall", RoomKind::Standard(StandardRoomKind::LibraryHall),
    StandardLibraryRing => "standard_library_ring", RoomKind::Standard(StandardRoomKind::LibraryRing),
    StandardStatues => "standard_statues", RoomKind::Standard(StandardRoomKind::Statues),
    StandardSegmentedLibrary => "standard_segmented_library", RoomKind::Standard(StandardRoomKind::SegmentedLibrary),
    StandardRuins => "standard_ruins", RoomKind::Standard(StandardRoomKind::Ruins),
    StandardChasm => "standard_chasm", RoomKind::Standard(StandardRoomKind::Chasm),
    StandardSkulls => "standard_skulls", RoomKind::Standard(StandardRoomKind::Skulls),
    StandardRitual => "standard_ritual", RoomKind::Standard(StandardRoomKind::Ritual),
    StandardPlants => "standard_plants", RoomKind::Standard(StandardRoomKind::Plants),
    StandardAquarium => "standard_aquarium", RoomKind::Standard(StandardRoomKind::Aquarium),
    StandardPlatform => "standard_platform", RoomKind::Standard(StandardRoomKind::Platform),
    StandardBurned => "standard_burned", RoomKind::Standard(StandardRoomKind::Burned),
    StandardFissure => "standard_fissure", RoomKind::Standard(StandardRoomKind::Fissure),
    StandardGrassyGrave => "standard_grassy_grave", RoomKind::Standard(StandardRoomKind::GrassyGrave),
    StandardStriped => "standard_striped", RoomKind::Standard(StandardRoomKind::Striped),
    StandardStudy => "standard_study", RoomKind::Standard(StandardRoomKind::Study),
    StandardSuspiciousChest => "standard_suspicious_chest", RoomKind::Standard(StandardRoomKind::SuspiciousChest),
    StandardMinefield => "standard_minefield", RoomKind::Standard(StandardRoomKind::Minefield),
    StandardMineEntrance => "standard_mine_entrance", RoomKind::Standard(StandardRoomKind::MineEntrance),
    StandardMineSmall => "standard_mine_small", RoomKind::Standard(StandardRoomKind::MineSmall),
    StandardMineLarge => "standard_mine_large", RoomKind::Standard(StandardRoomKind::MineLarge),
    StandardMineGiant => "standard_mine_giant", RoomKind::Standard(StandardRoomKind::MineGiant),
    StandardSewerBossEntrance => "standard_sewer_boss_entrance", RoomKind::Standard(StandardRoomKind::SewerBossEntrance),
    StandardSewerBossExit => "standard_sewer_boss_exit", RoomKind::Standard(StandardRoomKind::SewerBossExit),
    StandardDiamondGoo => "standard_diamond_goo", RoomKind::Standard(StandardRoomKind::DiamondGoo),
    StandardWalledGoo => "standard_walled_goo", RoomKind::Standard(StandardRoomKind::WalledGoo),
    StandardThinPillarsGoo => "standard_thin_pillars_goo", RoomKind::Standard(StandardRoomKind::ThinPillarsGoo),
    StandardThickPillarsGoo => "standard_thick_pillars_goo", RoomKind::Standard(StandardRoomKind::ThickPillarsGoo),
    ConnectionTunnel => "connection_tunnel", RoomKind::Connection(ConnectionRoomKind::Tunnel),
    ConnectionBridge => "connection_bridge", RoomKind::Connection(ConnectionRoomKind::Bridge),
    ConnectionPerimeter => "connection_perimeter", RoomKind::Connection(ConnectionRoomKind::Perimeter),
    ConnectionWalkway => "connection_walkway", RoomKind::Connection(ConnectionRoomKind::Walkway),
    ConnectionRingTunnel => "connection_ring_tunnel", RoomKind::Connection(ConnectionRoomKind::RingTunnel),
    ConnectionRingBridge => "connection_ring_bridge", RoomKind::Connection(ConnectionRoomKind::RingBridge),
    ConnectionMaze => "connection_maze", RoomKind::Connection(ConnectionRoomKind::Maze),
    SpecialWeakFloor => "weak_floor", RoomKind::Special(SpecialRoomKind::WeakFloor),
    SpecialCrypt => "crypt", RoomKind::Special(SpecialRoomKind::Crypt),
    SpecialPool => "pool", RoomKind::Special(SpecialRoomKind::Pool),
    SpecialArmory => "armory", RoomKind::Special(SpecialRoomKind::Armory),
    SpecialSentry => "sentry", RoomKind::Special(SpecialRoomKind::Sentry),
    SpecialStatue => "statue", RoomKind::Special(SpecialRoomKind::Statue),
    SpecialCrystalVault => "crystal_vault", RoomKind::Special(SpecialRoomKind::CrystalVault),
    SpecialCrystalChoice => "crystal_choice", RoomKind::Special(SpecialRoomKind::CrystalChoice),
    SpecialSacrifice => "sacrifice", RoomKind::Special(SpecialRoomKind::Sacrifice),
    SpecialRunestone => "runestone", RoomKind::Special(SpecialRoomKind::Runestone),
    SpecialGarden => "garden", RoomKind::Special(SpecialRoomKind::Garden),
    SpecialLibrary => "library", RoomKind::Special(SpecialRoomKind::Library),
    SpecialStorage => "storage", RoomKind::Special(SpecialRoomKind::Storage),
    SpecialTreasury => "treasury", RoomKind::Special(SpecialRoomKind::Treasury),
    SpecialMagicWell => "magic_well", RoomKind::Special(SpecialRoomKind::MagicWell),
    SpecialToxicGas => "toxic_gas", RoomKind::Special(SpecialRoomKind::ToxicGas),
    SpecialMagicalFire => "magical_fire", RoomKind::Special(SpecialRoomKind::MagicalFire),
    SpecialTraps => "traps", RoomKind::Special(SpecialRoomKind::Traps),
    SpecialCrystalPath => "crystal_path", RoomKind::Special(SpecialRoomKind::CrystalPath),
    SpecialLaboratory => "laboratory", RoomKind::Special(SpecialRoomKind::Laboratory),
    SpecialPit => "pit", RoomKind::Special(SpecialRoomKind::Pit),
    SpecialShop => "shop", RoomKind::Special(SpecialRoomKind::Shop),
    SpecialDemonSpawner => "demon_spawner", RoomKind::Special(SpecialRoomKind::DemonSpawner),
    SecretGarden => "secret_garden", RoomKind::Secret(SecretRoomKind::Garden),
    SecretLaboratory => "secret_laboratory", RoomKind::Secret(SecretRoomKind::Laboratory),
    SecretLibrary => "secret_library", RoomKind::Secret(SecretRoomKind::Library),
    SecretLarder => "secret_larder", RoomKind::Secret(SecretRoomKind::Larder),
    SecretWell => "secret_well", RoomKind::Secret(SecretRoomKind::Well),
    SecretRunestone => "secret_runestone", RoomKind::Secret(SecretRoomKind::Runestone),
    SecretArtillery => "secret_artillery", RoomKind::Secret(SecretRoomKind::Artillery),
    SecretChestChasm => "secret_chest_chasm", RoomKind::Secret(SecretRoomKind::ChestChasm),
    SecretHoneypot => "secret_honeypot", RoomKind::Secret(SecretRoomKind::Honeypot),
    SecretHoard => "secret_hoard", RoomKind::Secret(SecretRoomKind::Hoard),
    SecretMaze => "secret_maze", RoomKind::Secret(SecretRoomKind::Maze),
    SecretSummoning => "secret_summoning", RoomKind::Secret(SecretRoomKind::Summoning),
    SecretMine => "secret_mine", RoomKind::Secret(SecretRoomKind::Mine),
    SecretRatKing => "secret_rat_king", RoomKind::Secret(SecretRoomKind::RatKing),
    QuestMassGrave => "quest_mass_grave", RoomKind::Quest(QuestRoomKind::MassGrave),
    QuestRitualSite => "quest_ritual_site", RoomKind::Quest(QuestRoomKind::RitualSite),
    QuestRotGarden => "quest_rot_garden", RoomKind::Quest(QuestRoomKind::RotGarden),
    QuestBlacksmith => "quest_blacksmith", RoomKind::Quest(QuestRoomKind::Blacksmith),
    QuestAmbitiousImp => "quest_ambitious_imp", RoomKind::Quest(QuestRoomKind::AmbitiousImp),
    Entrance => "entrance", RoomKind::Entrance(_),
    Exit => "exit", RoomKind::Exit(_),
}

/// Compact set of room classes, independent of room order, size, and count.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct RoomSet(pub u128);
impl RoomSet {
    #[must_use]
    pub fn from_rooms(rooms: &[Room]) -> Self {
        Self::from_types(rooms.iter().map(|room| RoomType::from_kind(room.kind)))
    }
    #[must_use]
    pub fn from_types(rooms: impl IntoIterator<Item = RoomType>) -> Self {
        Self(
            rooms
                .into_iter()
                .fold(0, |mask, room| mask | (1 << room as u8)),
        )
    }
    #[must_use]
    pub const fn contains(self, room: RoomType) -> bool {
        self.0 & (1 << room as u8) != 0
    }
    pub fn iter(self) -> impl Iterator<Item = RoomType> {
        RoomType::ALL
            .iter()
            .copied()
            .filter(move |room| self.contains(*room))
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FloorRooms {
    pub depth: u8,
    pub rooms: RoomSet,
}

/// All entries in a query must pass. `rooms` requires every listed class;
/// `any_rooms` requires at least one listed class on this same floor.
#[derive(Clone, Debug, Eq, PartialEq)]
#[cfg_attr(feature = "json-query", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "json-query", serde(deny_unknown_fields))]
pub struct FloorRequirement {
    pub depth: u8,
    #[cfg_attr(
        feature = "json-query",
        serde(default, skip_serializing_if = "Option::is_none")
    )]
    pub feeling: Option<Feeling>,
    #[cfg_attr(
        feature = "json-query",
        serde(default, skip_serializing_if = "Vec::is_empty")
    )]
    pub rooms: Vec<RoomType>,
    #[cfg_attr(
        feature = "json-query",
        serde(default, skip_serializing_if = "Vec::is_empty")
    )]
    pub any_rooms: Vec<RoomType>,
}
impl FloorRequirement {
    #[must_use]
    pub fn matches(&self, world: &GeneratedWorld) -> bool {
        world.feelings.iter().any(|floor| {
            floor.depth == self.depth && self.feeling.is_none_or(|wanted| floor.feeling == wanted)
        }) && (self.rooms.is_empty() && self.any_rooms.is_empty()
            || world.floor_rooms.iter().any(|floor| {
                floor.depth == self.depth
                    && self.rooms.iter().all(|room| floor.rooms.contains(*room))
                    && (self.any_rooms.is_empty()
                        || self
                            .any_rooms
                            .iter()
                            .any(|room| floor.rooms.contains(*room)))
            }))
    }
    #[must_use]
    pub fn compile(&self) -> CompiledFloorRequirement {
        CompiledFloorRequirement {
            feeling: self.feeling,
            rooms: RoomSet::from_types(self.rooms.iter().copied()),
            any_rooms: RoomSet::from_types(self.any_rooms.iter().copied()),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CompiledFloorRequirement {
    pub feeling: Option<Feeling>,
    pub rooms: RoomSet,
    pub any_rooms: RoomSet,
}
impl CompiledFloorRequirement {
    #[must_use]
    pub fn matches_feeling(&self, feeling: Feeling) -> bool {
        self.feeling.is_none_or(|wanted| wanted == feeling)
    }
    #[must_use]
    pub const fn matches_rooms(&self, rooms: RoomSet) -> bool {
        rooms.0 & self.rooms.0 == self.rooms.0
            && (self.any_rooms.0 == 0 || rooms.0 & self.any_rooms.0 != 0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        level_prelude::LimitedDrops,
        regular_level::{build_sewer_room_graph, prepare_regular_floor},
        rng::{RandomStack, seed_for_depth},
        run::RunState,
        sewer_floor::paint_sewer_floor_filtered,
    };

    #[test]
    fn rejection_stops_exactly_after_preparation_or_graph_construction() {
        for check_rooms in [false, true] {
            let mut run = RunState::new(0);
            let mut limited = LimitedDrops::default();
            let mut random = RandomStack::with_base_seed(0);
            random.push(seed_for_depth(0, 1, 0));
            let (mut expected_run, mut expected_limited, mut expected_random) =
                (run.clone(), limited, random.clone());
            let prepared = prepare_regular_floor(
                &mut expected_run,
                &mut expected_limited,
                1,
                &mut expected_random,
            )
            .unwrap();
            if check_rooms {
                let _ = build_sewer_room_graph(
                    &mut expected_run,
                    &mut expected_limited,
                    1,
                    prepared.feeling,
                    &mut expected_random,
                );
            }
            let filter = CompiledFloorRequirement {
                feeling: if check_rooms {
                    None
                } else {
                    Some(Feeling::Dark)
                },
                rooms: if check_rooms {
                    RoomSet::from_types(RoomType::ALL.iter().copied())
                } else {
                    RoomSet::default()
                },
                any_rooms: RoomSet::default(),
            };
            assert!(
                paint_sewer_floor_filtered(&mut run, &mut limited, 1, &mut random, Some(&filter))
                    .unwrap()
                    .is_none()
            );
            assert_eq!(run, expected_run);
            assert_eq!(limited, expected_limited);
            assert_eq!(random.long(), expected_random.long());
        }
    }
}
