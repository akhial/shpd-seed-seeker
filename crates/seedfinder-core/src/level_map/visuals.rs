//! Raised `DungeonTerrainTilemap` composition with concealed/revealed secrets. Sprite
//! indices follow the pinned `DungeonTileSheet` (see `assets::SOURCE_REVISION`).

use super::{MapDraw, MapKind, MapLayer, MapScene, MapSprite, TILE_SIZE};
use crate::geometry::terrain as t;
use crate::level::{Level, TrapKind};
use crate::rng::{RandomStack, seed_for_depth};
use crate::seed::DungeonSeed;

pub(super) fn scene(
    seed: DungeonSeed,
    level: &Level,
    rooms: &[crate::room::Room],
    kind: MapKind,
) -> MapScene {
    let root = seed_for_depth(
        i64::try_from(seed.value()).expect("seed fits i64"),
        level.depth,
        u32::from(kind.branch()),
    );
    let mut random = RandomStack::with_base_seed(0);
    random.push(root);
    let variance: Vec<_> = (0..level.len()).map(|_| random.int_bound(100)).collect();
    let mut scene = MapScene {
        tile_size: TILE_SIZE,
        sprites: Vec::new(),
        layers: Vec::new(),
        concealed_layers: Vec::new(),
    };
    let mut revealed = level.clone();
    for tile in &mut revealed.map.cells {
        if *tile == t::SECRET_DOOR {
            *tile = t::DOOR;
        }
    }
    scene.layers = build_layers(&mut scene, &revealed, kind, &variance, true);
    let mut concealed = level.clone();
    for room in rooms
        .iter()
        .filter(|room| matches!(room.kind, crate::room::RoomKind::Secret(_)))
    {
        // Preserve shared boundary walls. Re-stitch the whole scene against
        // solid room interiors so overhangs/shadows cannot disclose the room.
        for y in room.bounds.top + 1..room.bounds.bottom {
            for x in room.bounds.left + 1..room.bounds.right {
                let cell = concealed
                    .map
                    .point_to_cell(crate::geometry::Point::new(x, y));
                concealed.map.cells[cell] = t::WALL;
            }
        }
    }
    scene.concealed_layers = build_layers(&mut scene, &concealed, kind, &variance, false);
    scene
}

fn build_layers(
    scene: &mut MapScene,
    level: &Level,
    kind: MapKind,
    variance: &[i32],
    reveal: bool,
) -> Vec<MapLayer> {
    use super::projection::Projection;
    let region = ((level.depth - 1) / 5) as usize;
    let tiles = match kind {
        MapKind::BlacksmithCrystal => "tiles_caves_crystal.png",
        MapKind::BlacksmithGnoll => "tiles_caves_gnoll.png",
        _ => super::assets::ASSETS[region].id,
    };
    let water = super::assets::ASSETS[region + 5].id;
    let projection = Projection {
        level,
        kind,
        variance,
    };
    let mut water_layer = layer("water", level.len());
    let mut terrain = layer("terrain", level.len());
    let mut shadows = layer("shadows", level.len());
    let mut features = layer("features", level.len());
    let mut raised = layer("raised", level.len());
    let mut walls = layer("walls", level.len());
    let mut effects = layer("effects", level.len());
    let mut darkness = layer("darkness", level.len());
    let water_sprites: Vec<_> = (0..4)
        .map(|phase| intern(scene, water_sprite(water, phase % 2, phase / 2)))
        .collect();
    for (cell, &tile) in level.map.cells.iter().enumerate() {
        let point = level.map.cell_to_point(cell);
        let phase =
            usize::try_from(point.x % 2 + 2 * (point.y % 2)).expect("water phase fits usize");
        water_layer.cells[cell] = Some(water_sprites[phase]);
        for (layer, asset, visual) in [
            (&mut terrain, tiles, projection.terrain(cell)),
            (
                &mut shadows,
                "occlusion_shadows.png",
                projection.shadows(cell),
            ),
            (
                &mut features,
                "terrain_features.png",
                projection.features(cell),
            ),
            (&mut raised, "raised_terrain.png", projection.raised(cell)),
            (&mut walls, tiles, projection.walls(cell)),
        ] {
            if let Some(visual) = visual {
                layer.cells[cell] = Some(intern(scene, tile_sprite(asset, visual)));
            }
        }
        let mask = projection.darkness(cell);
        if !mask.is_empty() {
            darkness.cells[cell] = Some(intern(
                scene,
                MapSprite {
                    frame_duration_ms: 1,
                    frames: vec![mask],
                },
            ));
        }
        if region == 0
            && tile == t::WALL_DECO
            && !super::projection::wall(projection.at(cell, 0, 1))
        {
            effects.cells[cell] = Some(intern(scene, pipe_drips(variance[cell])));
        }
    }
    for plant in &level.plants {
        if !super::projection::wall(level.map.cells[plant.cell]) {
            features.cells[plant.cell] = Some(intern(
                scene,
                tile_sprite("terrain_features.png", 112 + plant_image(plant.seed)),
            ));
        }
    }
    for trap in &level.traps {
        if !super::projection::wall(level.map.cells[trap.cell]) {
            features.cells[trap.cell] = if reveal || trap.visible {
                Some(intern(
                    scene,
                    tile_sprite(
                        "terrain_features.png",
                        trap_image(trap.spec.kind, trap.active),
                    ),
                ))
            } else {
                None
            };
        }
    }
    let structures = branch_structures(scene, level, kind);
    vec![
        water_layer,
        terrain,
        structures,
        shadows,
        features,
        raised,
        walls,
        effects,
        darkness,
    ]
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
    let columns = super::assets::get(asset).expect("embedded atlas").width / TILE_SIZE;
    MapSprite {
        frame_duration_ms: 1,
        frames: vec![vec![MapDraw::Blit {
            asset,
            source: [(tile % columns) * 16, (tile / columns) * 16, 16, 16],
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
// The pipe outlet sits near the base of its raised wall face.
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
    fn hidden_traps_have_revealed_and_concealed_layers_without_mutation() {
        let mut level = Level::new(2, Feeling::None);
        level.set_size(3, 3);
        level.map.cells[4] = t::SECRET_TRAP;
        level.set_trap(PlacedTrap {
            spec: TrapSpec::new(TrapKind::PoisonDart),
            cell: 4,
            visible: false,
            active: true,
        });
        let before = level.clone();
        let scene = scene(DungeonSeed::MIN, &level, &[], MapKind::Regular);
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
        assert_eq!(
            scene
                .concealed_layers
                .iter()
                .find(|l| l.name == "features")
                .unwrap()
                .cells[4],
            None
        );
    }

    #[test]
    fn concealing_a_secret_room_restitches_the_door_and_removes_its_interior() {
        use crate::{
            geometry::Rect,
            room::{Room, RoomKind, SecretRoomKind},
        };
        let mut level = Level::new(1, Feeling::None);
        level.set_size(9, 7);
        level.map.cells.fill(t::WALL);
        for y in 2..5 {
            for x in 4..7 {
                level.map.cells[x + y * 9] = t::EMPTY;
            }
        }
        let door = 3 + 3 * 9;
        let center = 5 + 3 * 9;
        level.map.cells[door] = t::SECRET_DOOR;
        level.map.cells[door - 1] = t::EMPTY;
        let mut room = Room::new(RoomKind::Secret(SecretRoomKind::Artillery));
        room.bounds = Rect::new(3, 1, 7, 5);
        let before = level.clone();
        let map = scene(DungeonSeed::MIN, &level, &[room], MapKind::Regular);
        assert_eq!(level, before);
        let revealed = map.layers.iter().find(|l| l.name == "terrain").unwrap();
        let concealed = map
            .concealed_layers
            .iter()
            .find(|l| l.name == "terrain")
            .unwrap();
        assert_eq!(
            map.sprites[revealed.cells[door].unwrap()].frame(0),
            &[MapDraw::Blit {
                asset: "tiles_sewers.png",
                source: [64, 112, 16, 16],
                destination: [0, 0, 16, 16]
            }]
        );
        assert_eq!(concealed.cells[door], None);
        let mask = map
            .concealed_layers
            .iter()
            .find(|l| l.name == "darkness")
            .unwrap();
        assert_eq!(
            map.sprites[mask.cells[center].unwrap()].frame(0),
            &[MapDraw::Fill {
                destination: [0, 0, 16, 16],
                rgba: [0, 0, 0, 255]
            }]
        );
        assert!(
            map.layers
                .iter()
                .find(|l| l.name == "darkness")
                .unwrap()
                .cells[center]
                .is_none()
        );
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
