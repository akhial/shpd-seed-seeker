//! On-demand floor and quest-branch maps with optional secret revelation.
//!
//! Generation replays the same sequential prefix and trinket activation as
//! loot scouting. Rendering uses a separate RNG, never the generation stream.
//! See `docs/level-map-format.md` for the platform-neutral sprite contract.

pub mod assets;
#[cfg(feature = "json-query")]
pub mod json;
mod projection;
mod visuals;

use crate::catalog::ItemId;
use crate::challenges::Challenges;
use crate::level::{Feeling, Level, TrapKind};
use crate::main_world::{MainWorldError, generate_main_world_observed};
use crate::room::{Room, RoomKind};
use crate::search::FloorGate;
use crate::seed::DungeonSeed;

/// Main floors whose complete initial terrain is implemented. Boss/shop-only
/// floors are excluded. Quest branches use `SUPPORTED_BRANCH_DEPTHS`.
pub const SUPPORTED_DEPTHS: [u8; 20] = [
    1, 2, 3, 4, 6, 7, 8, 9, 11, 12, 13, 14, 16, 17, 18, 19, 21, 22, 23, 24,
];
pub const SUPPORTED_BRANCH_DEPTHS: [u8; 6] = [12, 13, 14, 17, 18, 19];

pub const SCHEMA_VERSION: u8 = 2;
pub const TILE_SIZE: u16 = 16;

#[derive(Clone, Debug, PartialEq)]
pub enum MapError {
    UnsupportedDepth(u8),
    UnsupportedBranch { depth: u8, branch: u8 },
    MissingQuestBranch,
    Mining(crate::mining_floor::MiningError),
    InvalidTrinket,
    Generation(MainWorldError),
    MissingFloor,
}

impl std::fmt::Display for MapError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnsupportedDepth(depth) => write!(
                f,
                "level maps support regular main-branch floors 1..=24, got {depth}"
            ),
            Self::UnsupportedBranch { depth, branch } => {
                write!(f, "no map generator for depth {depth}, branch {branch}")
            }
            Self::MissingQuestBranch => f.write_str(
                "this floor does not host an accessible quest branch in the selected run",
            ),
            Self::Mining(error) => error.fmt(f),
            Self::InvalidTrinket => {
                f.write_str("selected trinket must be one of the four initial catalyst offers")
            }
            Self::Generation(error) => error.fmt(f),
            Self::MissingFloor => f.write_str("generation did not produce the requested floor"),
        }
    }
}
impl std::error::Error for MapError {}

/// The complete initial terrain and a portable raised drawing plan.
/// Both secret visibility modes retain the original terrain codes. The geometric
/// wall masks are included; player-specific exploration fog is not.
#[derive(Clone, Debug, PartialEq)]
pub struct LevelMap {
    pub seed: DungeonSeed,
    pub depth: u8,
    pub kind: MapKind,
    pub branches: Vec<MapBranch>,
    pub challenges: Challenges,
    pub selected_trinket: Option<ItemId>,
    pub feeling: Feeling,
    pub width: i32,
    pub height: i32,
    pub terrain: Vec<i32>,
    pub entrance: Option<usize>,
    pub exit: Option<usize>,
    /// Inclusive [left, top, right, bottom] room bounds.
    pub secret_rooms: Vec<[i32; 4]>,
    pub traps: Vec<MapTrap>,
    pub scene: MapScene,
}

/// Selects the branch's terrain atlas; both quest branches use game branch 1.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[cfg_attr(feature = "json-query", derive(serde::Serialize))]
#[cfg_attr(feature = "json-query", serde(rename_all = "snake_case"))]
pub enum MapKind {
    Regular,
    BlacksmithCrystal,
    BlacksmithGnoll,
    ImpVault,
}
impl MapKind {
    #[must_use]
    pub const fn branch(self) -> u8 {
        if matches!(self, Self::Regular) { 0 } else { 1 }
    }
    fn blacksmith(variant: crate::quests::BlacksmithQuestType) -> Self {
        match variant {
            crate::quests::BlacksmithQuestType::Crystal => Self::BlacksmithCrystal,
            crate::quests::BlacksmithQuestType::Gnoll => Self::BlacksmithGnoll,
        }
    }
}

/// An accessible quest branch on a regular floor; entrance is a cell on that
/// parent floor. Request the same seed/profile with this depth and branch.
#[derive(Clone, Debug, Eq, PartialEq)]
#[cfg_attr(feature = "json-query", derive(serde::Serialize))]
pub struct MapBranch {
    pub depth: u8,
    pub branch: u8,
    pub kind: MapKind,
    pub entrance: usize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
#[cfg_attr(feature = "json-query", derive(serde::Serialize))]
pub struct MapTrap {
    pub cell: usize,
    pub kind: TrapKind,
    pub hidden: bool,
    pub active: bool,
}

/// Layers are drawn in order over an opaque black background. Each cell is
/// either empty (`None`) or a sprite palette index, positioned at (x*16,y*16).
#[derive(Clone, Debug, Eq, PartialEq)]
#[cfg_attr(feature = "json-query", derive(serde::Serialize))]
#[cfg_attr(feature = "json-query", serde(rename_all = "camelCase"))]
pub struct MapScene {
    pub tile_size: u16,
    pub sprites: Vec<MapSprite>,
    pub layers: Vec<MapLayer>,
    /// Complete alternative layer stack with undiscovered secrets concealed.
    pub concealed_layers: Vec<MapLayer>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
#[cfg_attr(feature = "json-query", derive(serde::Serialize))]
pub struct MapLayer {
    pub name: &'static str,
    pub cells: Vec<Option<usize>>,
}

/// A static sprite has one frame. Animated frames loop without interpolation;
/// frame zero is a complete, usable reduced-motion/static fallback.
#[derive(Clone, Debug, Eq, PartialEq)]
#[cfg_attr(feature = "json-query", derive(serde::Serialize))]
#[cfg_attr(feature = "json-query", serde(rename_all = "camelCase"))]
pub struct MapSprite {
    pub frame_duration_ms: u16,
    pub frames: Vec<Vec<MapDraw>>,
}

impl MapSprite {
    /// Samples a sprite at elapsed milliseconds, including arbitrarily long
    /// sessions. Empty user-constructed sprites safely produce no commands.
    #[must_use]
    pub fn frame(&self, elapsed_ms: u64) -> &[MapDraw] {
        if self.frames.is_empty() {
            return &[];
        }
        let index =
            (elapsed_ms / u64::from(self.frame_duration_ms.max(1))) % self.frames.len() as u64;
        &self.frames[usize::try_from(index).unwrap_or_default()]
    }
}

/// Pixel rectangles are [x,y,width,height]. Destinations are relative to the
/// cell origin. Blits use nearest-neighbour sampling and ordinary source-over
/// alpha. Fills reproduce the game's pixel particles without a particle engine.
#[derive(Clone, Debug, Eq, PartialEq)]
#[cfg_attr(feature = "json-query", derive(serde::Serialize))]
#[cfg_attr(feature = "json-query", serde(tag = "kind", rename_all = "snake_case"))]
pub enum MapDraw {
    Blit {
        asset: &'static str,
        source: [u16; 4],
        destination: [u16; 4],
    },
    Fill {
        destination: [u16; 4],
        rgba: [u8; 4],
    },
}

struct MapGate(Option<ItemId>);
impl FloorGate for MapGate {
    fn selected_trinket(&self, _: DungeonSeed) -> Option<ItemId> {
        self.0
    }
    fn continue_after_floor(
        &self,
        _: u8,
        _: &[crate::model::WorldItem],
        _: &crate::quests::QuestSummary,
    ) -> bool {
        true
    }
}

/// Generates one map using the canonical scout profile. `selected_trinket`
/// must be an initial offer; it is equipped at +3 after the first brewing
/// opportunity, exactly as in loot scouting. Earlier floors are replayed to
/// preserve deck, quest and limited-drop state; only the requested map is kept.
///
/// # Errors
/// Rejects unsupported floors/unoffered trinkets and reports generation errors.
pub fn generate_level_map(
    seed: DungeonSeed,
    depth: u8,
    challenges: Challenges,
    selected_trinket: Option<ItemId>,
) -> Result<LevelMap, MapError> {
    generate_level_map_in_branch(seed, depth, 0, challenges, selected_trinket)
}

pub(super) fn validate_location(depth: u8, branch: u8) -> Result<(), MapError> {
    match branch {
        0 if SUPPORTED_DEPTHS.contains(&depth) => Ok(()),
        0 => Err(MapError::UnsupportedDepth(depth)),
        1 if SUPPORTED_BRANCH_DEPTHS.contains(&depth) => Ok(()),
        _ => Err(MapError::UnsupportedBranch { depth, branch }),
    }
}

/// Generates a regular floor (`branch = 0`) or its Blacksmith/Imp quest level
/// (`branch = 1`). Branches are available only at their actual quest depth in
/// the selected run. The mine variant and active trinket are inferred from it.
///
/// # Errors
/// Rejects unsupported locations, absent quest branches and unoffered trinkets;
/// propagates generation failures.
///
/// # Panics
/// Panics if an internal generation invariant fails; native bridges contain
/// these panics at their API boundary.
pub fn generate_level_map_in_branch(
    seed: DungeonSeed,
    depth: u8,
    branch: u8,
    challenges: Challenges,
    selected_trinket: Option<ItemId>,
) -> Result<LevelMap, MapError> {
    validate_location(depth, branch)?;
    if selected_trinket.is_some_and(|id| !crate::trinkets::trinket_order(seed)[..4].contains(&id)) {
        return Err(MapError::InvalidTrinket);
    }
    let mut result = None;
    generate_main_world_observed(
        seed,
        depth,
        challenges,
        &MapGate(selected_trinket),
        &mut |floor| {
            if floor.level.depth != u32::from(depth) {
                return;
            }
            let blacksmith = floor.quests.blacksmith.filter(|quest| quest.depth == depth);
            let imp = floor.quests.imp.filter(|quest| quest.depth == depth);
            if branch == 0 {
                let mut map = snapshot(
                    seed,
                    depth,
                    challenges,
                    selected_trinket,
                    floor.level,
                    floor.rooms,
                    MapKind::Regular,
                );
                let kind = blacksmith
                    .map(|q| MapKind::blacksmith(q.variant))
                    .or(imp.map(|_| MapKind::ImpVault));
                map.branches = kind
                    .and_then(|kind| branch_link(depth, kind, floor.level, floor.rooms))
                    .into_iter()
                    .collect();
                result = Some(Ok(map));
            } else if let Some(quest) = blacksmith {
                result = Some(
                    crate::mining_floor::generate_mine(
                        i64::try_from(seed.value()).expect("seed fits i64"),
                        depth,
                        quest.variant,
                        challenges,
                        floor.trinket,
                    )
                    .map(|mine| {
                        snapshot(
                            seed,
                            depth,
                            challenges,
                            selected_trinket,
                            &mine.level,
                            &mine.rooms,
                            MapKind::blacksmith(quest.variant),
                        )
                    })
                    .map_err(MapError::Mining),
                );
            } else if imp.is_some() {
                if let Some(vault) = floor.vault {
                    let mut level = vault.level.clone();
                    level.add_transition(
                        vault.entrance_cell,
                        crate::level::TransitionKind::RegularEntrance,
                    );
                    for &cell in &vault.flame_traps {
                        level.set_trap(crate::level::PlacedTrap {
                            cell,
                            spec: crate::level::TrapSpec::new(TrapKind::VaultFlame),
                            visible: true,
                            active: false,
                        });
                    }
                    let map = snapshot(
                        seed,
                        depth,
                        challenges,
                        selected_trinket,
                        &level,
                        &[],
                        MapKind::ImpVault,
                    );
                    result = Some(Ok(map));
                }
            }
        },
    )
    .map_err(MapError::Generation)?;
    result.unwrap_or(Err(if branch == 0 {
        MapError::MissingFloor
    } else {
        MapError::MissingQuestBranch
    }))
}

fn branch_link(depth: u8, kind: MapKind, level: &Level, rooms: &[Room]) -> Option<MapBranch> {
    let quest = match kind {
        MapKind::BlacksmithCrystal | MapKind::BlacksmithGnoll => {
            crate::room::QuestRoomKind::Blacksmith
        }
        MapKind::ImpVault => crate::room::QuestRoomKind::AmbitiousImp,
        MapKind::Regular => return None,
    };
    let room = rooms
        .iter()
        .find(|room| room.kind == RoomKind::Quest(quest))?;
    let entrance = level
        .map
        .cells
        .iter()
        .enumerate()
        .find_map(|(cell, &tile)| {
            (tile == crate::geometry::terrain::EXIT && room.inside(level.map.cell_to_point(cell)))
                .then_some(cell)
        })?;
    Some(MapBranch {
        depth,
        branch: 1,
        kind,
        entrance,
    })
}

fn snapshot(
    seed: DungeonSeed,
    depth: u8,
    challenges: Challenges,
    selected_trinket: Option<ItemId>,
    level: &Level,
    rooms: &[Room],
    kind: MapKind,
) -> LevelMap {
    LevelMap {
        seed,
        depth,
        kind,
        branches: Vec::new(),
        challenges,
        selected_trinket,
        feeling: level.feeling,
        width: level.width(),
        height: level.height(),
        terrain: level.map.cells.clone(),
        entrance: level.entrance(),
        exit: level.exit(),
        secret_rooms: rooms
            .iter()
            .filter(|room| matches!(room.kind, RoomKind::Secret(_)))
            .map(|room| {
                [
                    room.bounds.left,
                    room.bounds.top,
                    room.bounds.right,
                    room.bounds.bottom,
                ]
            })
            .collect(),
        traps: level
            .traps
            .iter()
            .map(|trap| MapTrap {
                cell: trap.cell,
                kind: trap.spec.kind,
                hidden: !trap.visible,
                active: trap.active,
            })
            .collect(),
        scene: visuals::scene(seed, level, rooms, kind),
    }
}
