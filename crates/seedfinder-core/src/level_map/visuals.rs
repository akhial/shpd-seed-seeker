//! Flat `DungeonTerrainTilemap` selection, with revealed secret doors. Sprite
//! indices follow the pinned `DungeonTileSheet` (see `assets::SOURCE_REVISION`).

use super::{MapDraw, MapKind, MapLayer, MapScene, MapSprite, TILE_SIZE};
use crate::geometry::terrain as t;
use crate::level::{Level, TrapKind};
use crate::rng::{RandomStack, seed_for_depth};
use crate::seed::DungeonSeed;

pub(super) fn scene(seed: DungeonSeed, level: &Level, kind: MapKind) -> MapScene {
    let region = ((level.depth - 1) / 5) as usize;
    let tiles = match kind {
        MapKind::BlacksmithCrystal => "tiles_caves_crystal.png",
        MapKind::BlacksmithGnoll => "tiles_caves_gnoll.png",
        _ => super::assets::ASSETS[region].id,
    };
    let water = super::assets::ASSETS[region + 5].id;
    let mut scene = MapScene {
        tile_size: TILE_SIZE,
        sprites: Vec::new(),
        layers: Vec::new(),
    };
    let mut water_layer = layer("water", level.len());
    let mut terrain_layer = layer("terrain", level.len());
    let mut features = layer("features", level.len());
    let mut effects = layer("effects", level.len());
    let water_sprites: Vec<_> = (0..4)
        .map(|phase| intern(&mut scene, water_sprite(water, phase % 2, phase / 2)))
        .collect();
    let mut terrain_sprites = [None; 256];
    // GameScene.setupVariance seeds its own child generator with seedCurDepth.
    let root = seed_for_depth(
        i64::try_from(seed.value()).expect("seed fits i64"),
        level.depth,
        u32::from(kind.branch()),
    );
    let mut random = RandomStack::with_base_seed(0);
    random.push(root);
    for (cell, &tile) in level.map.cells.iter().enumerate() {
        let variance = random.int_bound(100);
        // The game draws water beneath the entire terrain layer, including
        // transparent portions of regional decorations such as sewer barrels.
        let point = level.map.cell_to_point(cell);
        let phase =
            usize::try_from(point.x % 2 + 2 * (point.y % 2)).expect("water phase fits usize");
        water_layer.cells[cell] = Some(water_sprites[phase]);
        let visual = if kind == MapKind::BlacksmithGnoll
            && tile == t::EMPTY_DECO
            && [(0, -1), (1, 0), (0, 1), (-1, 0)]
                .iter()
                .any(|&(dx, dy)| neighbour(level, cell, dx, dy) == t::MINE_BOULDER)
        {
            if variance < 50 { 11 } else { 5 }
        } else if matches!(kind, MapKind::BlacksmithCrystal | MapKind::BlacksmithGnoll)
            && matches!(tile, t::MINE_CRYSTAL | t::MINE_BOULDER)
        {
            match variance {
                0..=33 => 78,
                34..=66 => 77,
                _ => 76,
            }
        } else {
            flat_visual(level, cell, tile, variance)
        };
        terrain_layer.cells[cell] = Some(
            *terrain_sprites[usize::from(visual)]
                .get_or_insert_with(|| intern(&mut scene, tile_sprite(tiles, visual))),
        );
        if region == 0 && tile == t::WALL_DECO {
            effects.cells[cell] = Some(intern(&mut scene, pipe_drips(variance)));
        }
    }
    for plant in &level.plants {
        features.cells[plant.cell] = Some(intern(
            &mut scene,
            tile_sprite("terrain_features.png", 112 + plant_image(plant.seed)),
        ));
    }
    for trap in &level.traps {
        features.cells[trap.cell] = Some(intern(
            &mut scene,
            tile_sprite(
                "terrain_features.png",
                trap_image(trap.spec.kind, trap.active),
            ),
        ));
    }
    let structures = branch_structures(&mut scene, level, kind);
    scene.layers = vec![water_layer, terrain_layer, structures, features, effects];
    scene
}

fn branch_structures(scene: &mut MapScene, level: &Level, kind: MapKind) -> MapLayer {
    let mut structures = layer("structures", level.len());
    let (asset, source_x) = match kind {
        MapKind::BlacksmithCrystal | MapKind::BlacksmithGnoll => ("caves_quest.png", 0),
        MapKind::ImpVault => ("city_quest.png", 128),
        MapKind::Regular => return structures,
    };
    if let Some(entrance) = level.entrance() {
        let origin = level.map.cell_to_point(entrance);
        for y in 0..3_u16 {
            for x in 0..3_u16 {
                let cell = level.map.point_to_cell(crate::geometry::Point::new(
                    origin.x + i32::from(x) - 1,
                    origin.y + i32::from(y) - 1,
                ));
                structures.cells[cell] = Some(intern(
                    scene,
                    MapSprite {
                        frame_duration_ms: 1,
                        frames: vec![vec![MapDraw::Blit {
                            asset,
                            source: [source_x + x * 16, (y + 1) * 16, 16, 16],
                            destination: [0, 0, 16, 16],
                        }]],
                    },
                ));
            }
        }
    }
    structures
}

fn layer(name: &'static str, length: usize) -> MapLayer {
    MapLayer {
        name,
        cells: vec![None; length],
    }
}

fn intern(scene: &mut MapScene, sprite: MapSprite) -> usize {
    if let Some(index) = scene
        .sprites
        .iter()
        .position(|existing| *existing == sprite)
    {
        index
    } else {
        let index = scene.sprites.len();
        scene.sprites.push(sprite);
        index
    }
}

fn tile_sprite(asset: &'static str, tile: u16) -> MapSprite {
    MapSprite {
        frame_duration_ms: 1,
        frames: vec![vec![MapDraw::Blit {
            asset,
            source: [(tile % 16) * 16, (tile / 16) * 16, 16, 16],
            destination: [0, 0, 16, 16],
        }]],
    }
}

// GameScene scrolls the 32x32 water texture at -5 pixels/sec. Split wrapped
// source rectangles in the engine so frontends need neither UV wrapping nor
// clipping. Thirty-two 200ms frames preserve pixel art at a 6.4-second period.
fn water_sprite(asset: &'static str, x: i32, y: i32) -> MapSprite {
    let sx = u16::try_from(x.rem_euclid(2) * 16).expect("water x fits u16");
    let frames = (0..32)
        .map(|step| {
            let sy = u16::try_from((y * 16 - step).rem_euclid(32)).expect("water y fits u16");
            let first = 16.min(32 - sy);
            let mut draws = vec![MapDraw::Blit {
                asset,
                source: [sx, sy, 16, first],
                destination: [0, 0, 16, first],
            }];
            if first < 16 {
                draws.push(MapDraw::Blit {
                    asset,
                    source: [sx, 0, 16, 16 - first],
                    destination: [0, first, 16, 16 - first],
                });
            }
            draws
        })
        .collect();
    MapSprite {
        frame_duration_ms: 200,
        frames,
    }
}

// Sewer sinks use 2px, half-alpha green pixel particles with a 0.4s life,
// emitted every 0.1s. A deterministic loop approximates their unseeded motion.
// The flat projection keeps the drips inside the pipe's cell.
fn pipe_drips(variance: i32) -> MapSprite {
    let phase = u16::try_from(variance.rem_euclid(4)).expect("phase fits u16");
    let frames = (0..8_u16)
        .map(|frame| {
            (0..4_u16)
                .map(|drop| {
                    let age = (frame + drop * 2 + phase) % 8;
                    MapDraw::Fill {
                        destination: [6 + (drop + phase) % 4, 10 + age * age / 16, 2, 2],
                        rgba: [93, 143, 117, 128],
                    }
                })
                .collect()
        })
        .collect();
    MapSprite {
        frame_duration_ms: 50,
        frames,
    }
}

fn neighbour(level: &Level, cell: usize, dx: i32, dy: i32) -> i32 {
    let point = level.map.cell_to_point(cell);
    let (x, y) = (point.x + dx, point.y + dy);
    if x < 0 || y < 0 || x >= level.width() || y >= level.height() {
        return -1;
    }
    level.map.cells[usize::try_from(x + y * level.width()).expect("in-bounds cell")]
}

fn flat_visual(level: &Level, cell: usize, tile: i32, variance: i32) -> u16 {
    let visual = match tile {
        t::EMPTY
        | t::SECRET_TRAP
        | t::TRAP
        | t::INACTIVE_TRAP
        | t::CUSTOM_DECO
        | t::CUSTOM_DECO_EMPTY => 0,
        t::GRASS => 2,
        t::EMPTY_WELL => 19,
        t::WALL => 48,
        t::DOOR | t::SECRET_DOOR => 56,
        t::OPEN_DOOR => 57,
        t::ENTRANCE => 16,
        t::EXIT => 17,
        t::EMBERS => 3,
        t::LOCKED_DOOR | t::HERO_LKD_DR => 58,
        t::PEDESTAL => 20,
        t::WALL_DECO => 49,
        t::BARRICADE => 65,
        t::EMPTY_SP => 4,
        t::HIGH_GRASS => 66,
        t::EMPTY_DECO => 1,
        t::LOCKED_EXIT => 61,
        t::UNLOCKED_EXIT => 60,
        t::WELL => 18,
        t::STATUE => 72,
        t::STATUE_SP => 73,
        t::BOOKSHELF => 50,
        t::ALCHEMY => 64,
        t::FURROWED_GRASS => 67,
        t::CRYSTAL_DOOR => 59,
        t::REGION_DECO => 74,
        t::REGION_DECO_ALT => 75,
        t::MINE_CRYSTAL | t::MINE_BOULDER => 76,
        t::ENTRANCE_SP => 22,
        t::CUSTOM_DECO_WTR => 32,
        t::WATER => {
            let mut visual = 32;
            for (dx, dy, mask) in [(0, -1, 1), (1, 0, 2), (0, 1, 4), (-1, 0, 8)] {
                if water_stitchable(neighbour(level, cell, dx, dy), level.depth) {
                    visual += mask;
                }
            }
            visual
        }
        t::CHASM => chasm_visual(neighbour(level, cell, 0, -1), level.depth),
        _ => 24,
    };
    alternate(visual, variance)
}

fn alternate(visual: u16, variance: i32) -> u16 {
    match (visual, variance) {
        (0, 0..=4) => 12,
        (0, 5..=52) => 6,
        (1..=4, 0..=49) => visual + 6,
        (48..=50, 0..=49) => visual + 4,
        (66..=67, 0..=49) => visual + 3,
        _ => visual,
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

fn trap_image(kind: TrapKind, active: bool) -> u16 {
    use TrapKind as K;
    let (color, shape) = match kind {
        K::Burning => (1, 0),
        K::Explosive => (1, 4),
        K::WornDart => (7, 5),
        K::PoisonDart => (3, 5),
        K::Gripping => (7, 0),
        K::Geyser => (4, 4),
        K::Chilling => (6, 0),
        K::Shocking => (2, 0),
        K::Toxic => (3, 2),
        K::Alarm | K::VaultFlame => (0, 0),
        K::Ooze => (3, 0),
        K::Confusion => (4, 2),
        K::Flock => (6, 1),
        K::Summoning => (4, 1),
        K::Teleportation => (4, 0),
        K::Gateway => (4, 5),
        K::Frost => (6, 3),
        K::Storm => (2, 3),
        K::Corrosion => (7, 2),
        K::Blazing => (1, 3),
        K::Disintegration => (5, 5),
        K::Rockfall | K::GnollRockfall => (7, 4),
        K::Flashing => (7, 3),
        K::Guardian => (0, 3),
        K::Weakening => (3, 1),
        K::Disarming => (0, 6),
        K::Warping => (4, 3),
        K::Cursing => (5, 1),
        K::Pitfall => (0, 4),
        K::Distortion => (4, 6),
        K::Grim => (7, 6),
    };
    (if active { color } else { 8 }) + 16 * shape
}

fn plant_image(seed: crate::generator::SeedKind) -> u16 {
    use crate::generator::SeedKind as S;
    match seed {
        S::Rotberry => 0,
        S::Firebloom => 1,
        S::Swiftthistle => 2,
        S::Sungrass => 3,
        S::Icecap => 4,
        S::Stormvine => 5,
        S::Sorrowmoss => 6,
        S::Fadeleaf => 10,
        S::Earthroot => 8,
        S::Starflower => 9,
        S::Blindweed => 11,
        S::Mageroyal => 7,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::level::{Feeling, PlacedTrap, TrapSpec};

    #[test]
    fn hidden_doors_and_traps_are_revealed_without_mutating_the_map() {
        let mut level = Level::new(2, Feeling::None);
        level.set_size(3, 3);
        level.map.cells[4] = t::SECRET_DOOR;
        assert_eq!(flat_visual(&level, 4, t::SECRET_DOOR, 0), 56);
        level.map.cells[4] = t::SECRET_TRAP;
        level.set_trap(PlacedTrap {
            spec: TrapSpec::new(TrapKind::PoisonDart),
            cell: 4,
            visible: false,
            active: true,
        });
        let before = level.clone();
        let scene = scene(DungeonSeed::MIN, &level, MapKind::Regular);
        assert_eq!(level, before);
        let features = scene
            .layers
            .iter()
            .find(|layer| layer.name == "features")
            .unwrap();
        let sprite = &scene.sprites[features.cells[4].unwrap()];
        assert_eq!(
            sprite.frames[0],
            vec![MapDraw::Blit {
                asset: "terrain_features.png",
                source: [48, 80, 16, 16],
                destination: [0, 0, 16, 16]
            }]
        );
        assert_eq!(trap_image(TrapKind::PoisonDart, false), 88);
    }

    #[test]
    fn water_stitches_cardinal_edges_and_handles_map_boundaries() {
        let mut level = Level::new(1, Feeling::None);
        level.set_size(3, 3);
        level.map.cells.fill(t::WATER);
        assert_eq!(flat_visual(&level, 4, t::WATER, 0), 32);
        level.map.cells[1] = t::EMPTY;
        level.map.cells[5] = t::GRASS;
        assert_eq!(flat_visual(&level, 4, t::WATER, 0), 35);
        level.map.cells[7] = t::DOOR;
        level.map.cells[3] = t::WELL;
        assert_eq!(flat_visual(&level, 4, t::WATER, 0), 47);
        assert_eq!(flat_visual(&level, 0, t::WATER, 0), 38);
        assert_eq!(chasm_visual(t::WATER, 1), 28);
        assert_eq!(chasm_visual(t::REGION_DECO_ALT, 7), 24);
        assert_eq!(chasm_visual(t::REGION_DECO_ALT, 17), 26);
    }

    #[test]
    fn animated_water_wraps_sources_and_remains_seamless_between_cells() {
        for step in 0..32_u64 {
            let sprite = water_sprite("water0.png", 0, 0);
            let frame = sprite.frame(step * 200);
            let mut pixels = [[false; 16]; 16];
            for draw in frame {
                let MapDraw::Blit {
                    source: [sx, sy, w, h],
                    destination: [dx, dy, dw, dh],
                    ..
                } = draw
                else {
                    panic!()
                };
                assert_eq!((w, h), (dw, dh));
                for y in 0..*h {
                    for x in 0..*w {
                        let covered = &mut pixels[usize::from(dy + y)][usize::from(dx + x)];
                        assert!(!*covered);
                        *covered = true;
                        assert_eq!(sx + x, dx + x);
                        assert_eq!(u64::from(sy + y), (u64::from(dy + y) + 32 - step) % 32);
                    }
                }
            }
            assert!(pixels.into_iter().flatten().all(|covered| covered));
            let below = water_sprite("water0.png", 0, 1);
            let MapDraw::Blit { source, .. } = &below.frame(step * 200)[0] else {
                panic!()
            };
            assert_eq!(u64::from(source[1]), (48 - step) % 32);
        }
        let water = water_sprite("water0.png", 0, 0);
        assert_eq!(water.frame(0), water.frame(6400));
        assert_ne!(water.frame(0), water.frame(200));
        assert_eq!(
            water.frame(u64::MAX),
            water.frame((u64::MAX / 200 % 32) * 200)
        );
        let pipe = pipe_drips(0);
        assert_eq!(pipe.frame(0), pipe.frame(400));
        assert_ne!(pipe.frame(0), pipe.frame(100));
    }
}
