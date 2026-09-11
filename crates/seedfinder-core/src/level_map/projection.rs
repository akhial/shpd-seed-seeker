//! Shattered's raised terrain, upper walls and 4.0 occlusion selection.
//! Ported from the revision recorded in `assets::SOURCE_REVISION`. Keep the
//! lower/upper layers separate: an overhang belongs to the cell ABOVE an object.

use super::{MapDraw, MapKind};
use crate::{geometry::terrain as t, level::Level};

pub(super) struct Projection<'a> {
    pub level: &'a Level,
    pub kind: MapKind,
    pub variance: &'a [i32],
}

pub(super) fn wall(tile: i32) -> bool {
    matches!(
        tile,
        t::WALL
            | t::WALL_DECO
            | t::SECRET_DOOR
            | t::LOCKED_EXIT
            | t::UNLOCKED_EXIT
            | t::BOOKSHELF
            | -1
    )
}
fn door(tile: i32) -> bool {
    matches!(
        tile,
        t::DOOR | t::OPEN_DOOR | t::LOCKED_DOOR | t::HERO_LKD_DR | t::CRYSTAL_DOOR
    )
}

impl Projection<'_> {
    pub fn at(&self, cell: usize, dx: i32, dy: i32) -> i32 {
        let point = self.level.map.cell_to_point(cell);
        let x = point.x + dx;
        let y = point.y + dy;
        if x < 0 || y < 0 || x >= self.level.width() || y >= self.level.height() {
            return -1;
        }
        self.level.map.cells[usize::try_from(x + y * self.level.width()).expect("in-bounds cell")]
    }
    pub fn alternate(&self, visual: u16, cell: usize) -> u16 {
        let v = self.variance[cell];
        match (visual, v) {
            (0, 0..=4) => 12,
            (0, 5..=52) => 6,
            (1..=4, 0..=49) => visual + 6,
            (80 | 84 | 92, 0..=49) => visual + 16,
            (122..=123 | 234..=235, 0..=49) => visual + 3,
            (5, 0..=49) if self.kind == MapKind::BlacksmithGnoll => 11,
            (132, _) if self.kind == MapKind::BlacksmithCrystal => {
                132 + match v {
                    0..=16 => 5,
                    17..=33 => 4,
                    34..=49 => 3,
                    50..=66 => 2,
                    67..=83 => 1,
                    _ => 0,
                }
            }
            (132 | 244, _)
                if self.kind == MapKind::BlacksmithGnoll
                    || (visual == 244 && self.kind == MapKind::BlacksmithCrystal) =>
            {
                visual
                    + match v {
                        0..=33 => 2,
                        34..=66 => 1,
                        _ => 0,
                    }
            }
            _ => visual,
        }
    }
    pub fn terrain(&self, cell: usize) -> Option<u16> {
        let tile = self.at(cell, 0, 0);
        let direct = match tile {
            t::EMPTY
            | t::SECRET_TRAP
            | t::TRAP
            | t::INACTIVE_TRAP
            | t::CUSTOM_DECO
            | t::CUSTOM_DECO_EMPTY => Some(0),
            t::GRASS => Some(2),
            t::EMPTY_WELL => Some(19),
            t::ENTRANCE => Some(16),
            t::EXIT => Some(17),
            t::EMBERS => Some(3),
            t::PEDESTAL => Some(20),
            t::EMPTY_SP => Some(4),
            t::ENTRANCE_SP => Some(22),
            t::CUSTOM_DECO_WTR => Some(32),
            t::EMPTY_DECO => Some(
                if self.kind == MapKind::BlacksmithGnoll
                    && [(0, -1), (1, 0), (0, 1), (-1, 0)]
                        .iter()
                        .any(|&(x, y)| self.at(cell, x, y) == t::MINE_BOULDER)
                {
                    5
                } else {
                    1
                },
            ),
            t::LOCKED_EXIT => Some(61),
            t::UNLOCKED_EXIT => Some(60),
            t::WELL => Some(18),
            _ => None,
        };
        if let Some(visual) = direct {
            return Some(self.alternate(visual, cell));
        }
        if tile == t::WATER {
            return Some(
                32 + [(0, -1, 1), (1, 0, 2), (0, 1, 4), (-1, 0, 8)]
                    .iter()
                    .filter_map(|&(x, y, bit)| {
                        water_stitchable(self.at(cell, x, y), self.level.depth).then_some(bit)
                    })
                    .sum::<u16>(),
            );
        }
        if tile == t::CHASM {
            return Some(chasm_visual(self.at(cell, 0, -1), self.level.depth));
        }
        if door(tile) {
            // The upstream helper's argument is named "below", but the terrain
            // tilemap passes the cell ABOVE the door. This chooses orientation.
            return Some(if wall(self.at(cell, 0, -1)) {
                116
            } else {
                match tile {
                    t::DOOR => 112,
                    t::OPEN_DOOR => 113,
                    t::CRYSTAL_DOOR => 115,
                    _ => 114,
                }
            });
        }
        if wall(tile) {
            let below = self.at(cell, 0, 1);
            if wall(below) {
                return None;
            }
            let base = if door(below) {
                88
            } else {
                match tile {
                    t::WALL | t::SECRET_DOOR => 80,
                    t::WALL_DECO => 84,
                    t::BOOKSHELF => 92,
                    _ => return None,
                }
            };
            return Some(
                self.alternate(base, cell)
                    + u16::from(!wall(self.at(cell, 1, 0)))
                    + 2 * u16::from(!wall(self.at(cell, -1, 0))),
            );
        }
        let base = match tile {
            t::STATUE => 128,
            t::STATUE_SP => 129,
            t::REGION_DECO => 130,
            t::REGION_DECO_ALT => 131,
            t::MINE_CRYSTAL | t::MINE_BOULDER => 132,
            t::ALCHEMY => 120,
            t::BARRICADE => 121,
            t::HIGH_GRASS => 122,
            t::FURROWED_GRASS => 123,
            _ => return None,
        };
        Some(self.alternate(base, cell))
    }
    #[allow(clippy::if_not_else)] // Preserve the upstream decision order for review.
    pub fn walls(&self, cell: usize) -> Option<u16> {
        let tile = self.at(cell, 0, 0);
        let below = self.at(cell, 0, 1);
        if wall(tile) {
            if !wall(below) {
                match below {
                    t::DOOR => return Some(227),
                    t::LOCKED_DOOR | t::HERO_LKD_DR => return Some(228),
                    t::CRYSTAL_DOOR => return Some(229),
                    t::OPEN_DOOR => return None,
                    _ => {}
                }
            } else {
                let base = if tile == t::BOOKSHELF || below == t::BOOKSHELF {
                    176
                } else if self.kind.branch() == 1 && tile == t::WALL_DECO {
                    160
                } else {
                    144
                };
                return Some(
                    base + [(1, 0, 1), (1, 1, 2), (-1, 1, 4), (-1, 0, 8)]
                        .iter()
                        .filter_map(|&(x, y, bit)| (!wall(self.at(cell, x, y))).then_some(bit))
                        .sum::<u16>(),
                );
            }
        }
        if matches!(tile, t::LOCKED_EXIT | t::UNLOCKED_EXIT) {
            return Some(230);
        }
        if below == -1 {
            return None;
        }
        if wall(below) {
            let base = match tile {
                t::OPEN_DOOR => 208,
                t::DOOR => 212,
                t::LOCKED_DOOR | t::HERO_LKD_DR => 216,
                t::CRYSTAL_DOOR => 220,
                _ => {
                    if self.kind.branch() == 1 && below == t::WALL_DECO {
                        196
                    } else if below == t::BOOKSHELF {
                        200
                    } else {
                        192
                    }
                }
            };
            return Some(
                base + u16::from(!wall(self.at(cell, 1, 1)))
                    + 2 * u16::from(!wall(self.at(cell, -1, 1))),
            );
        }
        let base = match below {
            t::DOOR | t::LOCKED_DOOR | t::HERO_LKD_DR => 224,
            t::OPEN_DOOR => 225,
            t::CRYSTAL_DOOR => 226,
            t::STATUE => 240,
            t::STATUE_SP => 241,
            t::REGION_DECO => 242,
            t::REGION_DECO_ALT => 243,
            t::MINE_CRYSTAL | t::MINE_BOULDER => 244,
            t::ALCHEMY => 232,
            t::BARRICADE => 233,
            t::HIGH_GRASS => 234,
            t::FURROWED_GRASS => 235,
            _ => return None,
        };
        Some(self.alternate(
            base,
            cell + usize::try_from(self.level.width()).expect("positive map width"),
        ))
    }
    pub fn discoverable(&self, cell: usize) -> bool {
        // Level.cleanWalls: only walls adjacent to terrain can be seen.
        (-1..=1)
            .any(|y| (-1..=1).any(|x| !matches!(self.at(cell, x, y), -1 | t::WALL | t::WALL_DECO)))
    }
    pub fn shadows(&self, cell: usize) -> Option<u16> {
        if self.at(cell, -1, 0) == -1
            || self.at(cell, 1, 0) == -1
            || self.at(cell, 0, -1) == -1
            || self.at(cell, 0, 1) == -1
            || !self.discoverable(cell)
        {
            return None;
        }
        let tile = self.at(cell, 0, 0);
        if door(tile) {
            return Some(if wall(self.at(cell, -1, 0)) && wall(self.at(cell, 1, 0)) {
                7
            } else {
                6
            });
        }
        if wall(tile) || tile == t::CHASM {
            return None;
        }
        let above = wall(self.at(cell, 0, -1)) && tile != t::ALCHEMY;
        let mut visual = if above { 40 } else { 0 };
        for (side, weight) in [(-1, 1), (1, 8)] {
            visual += weight
                * if wall(self.at(cell, side, 0)) {
                    2
                } else if wall(self.at(cell, side, 1)) {
                    if !above && wall(self.at(cell, side, -1)) {
                        4
                    } else {
                        1
                    }
                } else if !above && wall(self.at(cell, side, -1)) {
                    3
                } else {
                    0
                };
        }
        (visual != 0).then_some(visual)
    }
    pub fn features(&self, cell: usize) -> Option<u16> {
        let region = u16::try_from((self.level.depth - 1) / 5).expect("supported region");
        let alt = u16::from(self.variance[cell] < 50);
        Some(match self.at(cell, 0, 0) {
            t::HIGH_GRASS => 128 + 16 * region + alt,
            t::FURROWED_GRASS => 130 + 16 * region + alt,
            t::GRASS => 132 + 16 * region + alt,
            t::BARRICADE => 134 + 16 * region,
            t::ALCHEMY => 135 + 16 * region,
            t::STATUE | t::STATUE_SP => 136 + 16 * region,
            t::REGION_DECO => 137 + 16 * region,
            t::REGION_DECO_ALT => 138 + 16 * region,
            t::EMBERS => 208 + alt,
            t::MINE_CRYSTAL => 210 + self.alternate(132, cell) - 132,
            t::MINE_BOULDER => 216 + self.alternate(132, cell) - 132,
            _ => return None,
        })
    }
    pub fn raised(&self, cell: usize) -> Option<u16> {
        let base = u16::try_from((self.level.depth - 1) / 5).expect("supported region") * 4
            + 2 * u16::from(self.variance[cell] < 50);
        match self.at(cell, 0, 0) {
            t::HIGH_GRASS => Some(base),
            t::FURROWED_GRASS => Some(base + 1),
            _ => None,
        }
    }
    pub fn darkness(&self, cell: usize) -> Vec<MapDraw> {
        let fill = |x, w| MapDraw::Fill {
            destination: [x, 0, w, 16],
            rgba: [0, 0, 0, 255],
        };
        if !self.discoverable(cell) {
            return vec![fill(0, 16)];
        }
        if wall(self.at(cell, 0, 0)) {
            if self.at(cell, 0, 1) == -1 {
                return vec![fill(0, 16)];
            }
            if wall(self.at(cell, 0, 1)) {
                // FogOfWar's geometric half-wall masks apply even when the
                // entire floor is visible: solid wall interiors stay black.
                return [(-1, 0), (1, 8)]
                    .into_iter()
                    .filter_map(|(side, x)| {
                        (wall(self.at(cell, side, 0)) && wall(self.at(cell, side, 1)))
                            .then_some(fill(x, 8))
                    })
                    .collect();
            }
        }
        Vec::new()
    }
}

fn water_stitchable(tile: i32, depth: u32) -> bool {
    matches!(
        tile,
        t::EMPTY
            | t::GRASS
            | t::EMPTY_WELL
            | t::ENTRANCE
            | t::EXIT
            | t::EMBERS
            | t::BARRICADE
            | t::HIGH_GRASS
            | t::FURROWED_GRASS
            | t::SECRET_TRAP
            | t::TRAP
            | t::INACTIVE_TRAP
            | t::EMPTY_DECO
            | t::CUSTOM_DECO
            | t::WELL
            | t::STATUE
            | t::REGION_DECO
            | t::ALCHEMY
            | t::CUSTOM_DECO_EMPTY
            | t::MINE_CRYSTAL
            | t::MINE_BOULDER
            | t::DOOR
            | t::OPEN_DOOR
            | t::LOCKED_DOOR
            | t::HERO_LKD_DR
            | t::CRYSTAL_DOOR
    ) || (tile == t::REGION_DECO_ALT && depth > 20)
}
fn chasm_visual(above: i32, depth: u32) -> u16 {
    match above {
        t::REGION_DECO_ALT => match depth {
            1..=5 | 11..=20 => 26,
            6..=10 => 24,
            _ => 25,
        },
        t::EMPTY_SP | t::STATUE_SP => 26,
        t::WALL
        | t::DOOR
        | t::OPEN_DOOR
        | t::LOCKED_DOOR
        | t::HERO_LKD_DR
        | t::SECRET_DOOR
        | t::WALL_DECO => 27,
        t::WATER => 28,
        t::EMPTY
        | t::GRASS
        | t::EMBERS
        | t::EMPTY_WELL
        | t::HIGH_GRASS
        | t::FURROWED_GRASS
        | t::EMPTY_DECO
        | t::CUSTOM_DECO
        | t::WELL
        | t::STATUE
        | t::REGION_DECO
        | t::SECRET_TRAP
        | t::INACTIVE_TRAP
        | t::TRAP
        | t::BOOKSHELF
        | t::BARRICADE
        | t::PEDESTAL
        | t::CUSTOM_DECO_EMPTY
        | t::MINE_BOULDER
        | t::MINE_CRYSTAL => 25,
        _ => 24,
    }
}

#[cfg(all(test, feature = "json-query"))]
mod tests {
    use super::*;
    use crate::{
        challenges::Challenges,
        level::Feeling,
        level_map::generate_level_map_in_branch,
        rng::{RandomStack, seed_for_depth},
        seed::DungeonSeed,
    };
    #[test]
    fn raised_layers_match_the_unmodified_v4_game_selectors() {
        let rows: serde_json::Value = serde_json::from_str(include_str!(
            "../../../../tooling/oracle-4.0/tests/raised-map-visuals.expected.json"
        ))
        .unwrap();
        for row in rows.as_array().unwrap() {
            let seed = DungeonSeed::from_code(row["seed"].as_str().unwrap()).unwrap();
            let depth = u8::try_from(row["depth"].as_u64().unwrap()).unwrap();
            let branch = u8::try_from(row["branch"].as_u64().unwrap()).unwrap();
            let map =
                generate_level_map_in_branch(seed, depth, branch, Challenges::NONE, None).unwrap();
            let mut level = Level::new(u32::from(depth), Feeling::None);
            level.set_size(map.width, map.height);
            level.map.cells = map.terrain;
            let hash = |values: Vec<i32>| {
                values
                    .iter()
                    .fold(1_i32, |h, &v| h.wrapping_mul(31).wrapping_add(v))
            };
            assert_eq!(
                serde_json::json!(hash(level.map.cells.clone())),
                row["terrainHash"]
            );
            let mut random = RandomStack::with_base_seed(0);
            random.push(seed_for_depth(
                i64::try_from(seed.value()).unwrap(),
                u32::from(depth),
                u32::from(branch),
            ));
            let variance: Vec<_> = (0..level.len()).map(|_| random.int_bound(100)).collect();
            let projection = Projection {
                level: &level,
                kind: map.kind,
                variance: &variance,
            };
            for name in ["terrain", "shadows", "features", "raised", "walls"] {
                let values = (0..level.len())
                    .map(|cell| {
                        match name {
                            "terrain" => projection.terrain(cell),
                            "shadows" => projection.shadows(cell),
                            "features" => projection.features(cell),
                            "raised" => projection.raised(cell),
                            _ => projection.walls(cell),
                        }
                        .map_or(-1, i32::from)
                    })
                    .collect();
                assert_eq!(
                    serde_json::json!(hash(values)),
                    row["layers"][name],
                    "{} depth {depth} branch {branch} {name}",
                    seed.to_code()
                );
            }
        }
    }
    #[test]
    fn undiscoverable_rock_and_internal_wall_halves_are_black() {
        let mut level = Level::new(1, Feeling::None);
        level.set_size(5, 5);
        level.map.cells.fill(t::WALL);
        let variance = vec![99; 25];
        let opaque = MapDraw::Fill {
            destination: [0, 0, 16, 16],
            rgba: [0, 0, 0, 255],
        };
        assert_eq!(
            Projection {
                level: &level,
                kind: MapKind::Regular,
                variance: &variance
            }
            .darkness(12),
            vec![opaque]
        );
        level.map.cells[11] = t::EMPTY;
        let p = Projection {
            level: &level,
            kind: MapKind::Regular,
            variance: &variance,
        };
        assert_eq!(
            p.darkness(12),
            vec![MapDraw::Fill {
                destination: [8, 0, 8, 16],
                rgba: [0, 0, 0, 255]
            }]
        );
        assert_eq!(p.walls(12), Some(152));
        assert_eq!(p.terrain(12), None);
        assert_eq!(p.terrain(6), Some(80));
    }
}
