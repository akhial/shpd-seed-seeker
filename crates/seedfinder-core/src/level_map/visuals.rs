//! Raised `DungeonTerrainTilemap` composition with concealed/revealed secrets. Sprite
//! indices follow the pinned `DungeonTileSheet` (see `assets::SOURCE_REVISION`).

mod actors;
mod ambient;
mod boss;
mod carpets;
mod chasms;
mod item_rects;
mod objects;
mod particles;
mod rooms;
mod sentries;

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
    contents: &super::MapContents,
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
        emitters: Vec::new(),
        concealed_emitters: Vec::new(),
    };
    let mut revealed = level.clone();
    for tile in &mut revealed.map.cells {
        if *tile == t::SECRET_DOOR {
            *tile = t::DOOR;
        }
    }
    scene.layers = build_layers(
        &mut scene, &revealed, kind, &variance, true, rooms, contents,
    );
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
    scene.concealed_layers = build_layers(
        &mut scene, &concealed, kind, &variance, false, rooms, contents,
    );
    scene.emitters = particles::emitters(&revealed, contents);
    scene.concealed_emitters = particles::emitters(&concealed, contents);
    scene
        .emitters
        .extend(chasms::emitters(&revealed, rooms, kind));
    scene
        .concealed_emitters
        .extend(chasms::emitters(&concealed, rooms, kind));
    scene
}

#[allow(clippy::too_many_lines)] // Compose layers in the game’s drawing order.
fn build_layers(
    scene: &mut MapScene,
    level: &Level,
    kind: MapKind,
    variance: &[i32],
    reveal: bool,
    rooms: &[crate::room::Room],
    contents: &super::MapContents,
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
    let effects = layer("effects", level.len());
    let mut darkness = layer("darkness", level.len());
    let water_sprites: Vec<_> = (0..4)
        .map(|phase| intern(scene, water_sprite(water, phase % 2, phase / 2)))
        .collect();
    for cell in 0..level.len() {
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
    }
    for plant in &contents.plants {
        if !super::projection::wall(level.map.cells[plant.cell]) {
            features.cells[plant.cell] = Some(intern(
                scene,
                tile_sprite("terrain_features.png", 112 + plant.image),
            ));
        }
    }
    for trap in &contents.traps {
        if objects::visible(level, trap.cell) && (reveal || !trap.hidden) {
            features.cells[trap.cell] = Some(intern(
                scene,
                tile_sprite(
                    "terrain_features.png",
                    trap_name_image(&trap.kind, trap.active),
                ),
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
    let [room_floor, room_terrain, room_walls] = rooms::layers(scene, level, rooms, contents);
    let [boss_floor, boss_terrain, boss_walls] =
        if kind == MapKind::Regular && matches!(level.depth, 5 | 15) {
            boss::layers(scene, level, rooms, reveal)
        } else {
            [
                layer("boss_floor", level.len()),
                layer("boss_terrain", level.len()),
                layer("boss_walls", level.len()),
            ]
        };
    // GameScene: custom floors, occlusion shadows, plants/traps, custom
    // terrain, heaps, mobs, raised terrain, walls, custom walls, emitters.
    // Foreground wall lips must occlude the bottom of actors and heaps.
    let mut layers = vec![
        water_layer,
        terrain,
        structures,
        room_floor,
        boss_floor,
        shadows,
        features,
        room_terrain,
        boss_terrain,
    ];
    layers.extend(objects::layers(scene, level, contents, &mut walls));
    layers.extend([raised, walls, room_walls, boss_walls, effects]);
    layers.push(darkness);
    layers
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
                            opacity: 255,
                            tint: None,
                            glow: None,
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
        blend: None,
        name,
        cells: vec![None; length],
    }
}

#[cfg(test)]
mod occlusion_tests {
    use super::*;
    use crate::{
        level::Feeling,
        level_map::{MapContents, MapMob},
    };

    #[test]
    fn tall_spire_occludes_background_crystal_but_not_foreground_or_structural_walls() {
        let mut level = Level::new(13, Feeling::None);
        level.set_size(7, 8);
        level.map.cells.fill(t::EMPTY);
        level.map.cells[4 * 7 + 3] = t::MINE_CRYSTAL;
        level.map.cells[5 * 7 + 4] = t::MINE_CRYSTAL;
        level.map.cells[4 * 7 + 2] = t::WALL;
        let contents = MapContents {
            mobs: vec![MapMob {
                cell: 5 * 7 + 3,
                kind: "GreenCrystalSpire".into(),
                stealthy: false,
                sleeping: false,
                approximate: false,
                items: vec![],
            }],
            ..MapContents::default()
        };
        let scene = scene(
            DungeonSeed::MIN,
            &level,
            &[],
            MapKind::BlacksmithCrystal,
            &contents,
        );
        for layers in [&scene.layers, &scene.concealed_layers] {
            for (cell, behind) in [(3 * 7 + 3, true), (4 * 7 + 4, false), (3 * 7 + 2, false)] {
                let wall = layers
                    .iter()
                    .position(|layer| layer.name == "walls" && layer.cells[cell].is_some())
                    .unwrap();
                let actor = layers
                    .iter()
                    .position(|layer| layer.name == "actors" && layer.cells[cell].is_some())
                    .unwrap();
                assert_eq!(wall < actor, behind, "occlusion at cell {cell}");
            }
        }
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
            opacity: 255,
            tint: None,
            glow: None,
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
                opacity: 255,
                tint: None,
                glow: None,
                asset,
                source: [sx, sy, 16, first],
                destination: [0, 0, 16, first],
            }];
            if first < 16 {
                draws.push(MapDraw::Blit {
                    opacity: 255,
                    tint: None,
                    glow: None,
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

fn trap_image(kind: TrapKind, active: bool) -> u16 {
    trap_name_image(&format!("{kind:?}"), active)
}

fn trap_name_image(kind: &str, active: bool) -> u16 {
    let (color, shape) = match kind {
        "Burning" => (1, 0),
        "Explosive" => (1, 4),
        "WornDart" => (7, 5),
        "PoisonDart" => (3, 5),
        "Gripping" => (7, 0),
        "Geyser" => (4, 4),
        "Chilling" => (6, 0),
        "Shocking" => (2, 0),
        "Toxic" => (3, 2),
        "Alarm" | "VaultFlame" => (0, 0),
        "Ooze" => (3, 0),
        "Confusion" => (4, 2),
        "Flock" => (6, 1),
        "Summoning" => (4, 1),
        "Teleportation" => (4, 0),
        "Gateway" => (4, 5),
        "Frost" => (6, 3),
        "Storm" => (2, 3),
        "Corrosion" => (7, 2),
        "Blazing" => (1, 3),
        "Disintegration" => (5, 5),
        "Rockfall" | "GnollRockfall" => (7, 4),
        "Flashing" => (7, 3),
        "Guardian" => (0, 3),
        "Weakening" => (3, 1),
        "Disarming" => (0, 6),
        "Warping" => (4, 3),
        "Cursing" => (5, 1),
        "Pitfall" => (0, 4),
        "Distortion" => (4, 6),
        "Grim" => (7, 6),
        "ToxicVent" => (8, 2),
        _ => unreachable!("unregistered map trap {kind}"),
    };
    (if active { color } else { 8 }) + 16 * shape
}

pub(super) fn plant_image(seed: crate::generator::SeedKind) -> u16 {
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
    fn rotberry_clouds_follow_the_heart() {
        use crate::level_map::{MapContents, MapMob};
        let mut level = Level::new(8, Feeling::None);
        level.set_size(5, 5);
        level.map.cells[12] = t::EMPTY;
        let contents = MapContents {
            mobs: vec![MapMob {
                cell: 12,
                kind: "RotHeart".into(),
                sleeping: false,
                stealthy: false,
                approximate: false,
                items: vec![],
            }],
            ..MapContents::default()
        };
        let map = scene(DungeonSeed::MIN, &level, &[], MapKind::Regular, &contents);
        for emitters in [&map.emitters, &map.concealed_emitters] {
            assert_eq!(emitters.len(), 1);
            let cloud = &emitters[0];
            assert_eq!(cloud.cell, 12);
            assert!(cloud.wall_mask);
            assert_eq!(cloud.blend, None);
            assert!(matches!(
                cloud.image,
                MapDraw::Blit {
                    asset: "specks.png",
                    tint: Some([80, 255, 96]),
                    ..
                }
            ));
            assert_eq!(cloud.angular_speed, 30);
            let mut births: Vec<_> = cloud.particles.iter().map(|p| p.birth_ms).collect();
            births.sort_unstable();
            assert_eq!(births.len(), 5);
            births.push(births[0] + cloud.loop_ms);
            assert!(births.windows(2).all(|pair| pair[1] - pair[0] == 700));
            assert!(
                cloud
                    .particles
                    .iter()
                    .all(|p| (1000..3000).contains(&p.lifespan_ms)
                        && (0..16000).contains(&p.position[0])
                        && (-3000..13000).contains(&p.position[1]))
            );
        }
        // Concealed/solid cells must not reveal an actor through its cloud.
        level.map.cells[12] = t::WALL;
        let hidden = scene(DungeonSeed::MIN, &level, &[], MapKind::Regular, &contents);
        assert!(hidden.emitters.is_empty());
        assert!(hidden.concealed_emitters.is_empty());
    }

    #[test]
    fn spyglass_opacity_covers_raised_items_glows_and_shadows_in_both_scenes() {
        use crate::level_map::{MapContents, MapGlow, MapHeap, MapItem};
        let mut level = Level::new(6, Feeling::None);
        level.set_size(5, 5);
        level.map.cells[12] = t::EMPTY;
        for phantom in [false, true] {
            let contents = MapContents {
                heaps: vec![MapHeap {
                    cell: 12,
                    kind: "Heap".into(),
                    haunted: false,
                    phantom,
                    items: vec![MapItem {
                        glow: Some(MapGlow {
                            color: [255, 0, 0],
                            period_ms: 1000,
                        }),
                        ..MapItem::new("test_weapon", 160, 1)
                    }],
                }],
                ..MapContents::default()
            };
            let scene = scene(DungeonSeed::MIN, &level, &[], MapKind::Regular, &contents);
            for layers in [&scene.layers, &scene.concealed_layers] {
                let mut fragments = 0;
                let mut shadows = 0;
                for sprite in layers
                    .iter()
                    .filter(|l| l.name == "heaps")
                    .flat_map(|l| l.cells.iter().flatten())
                    .map(|&s| &scene.sprites[s])
                {
                    fragments += 1;
                    for draw in sprite.frames.iter().flatten() {
                        if let MapDraw::Blit {
                            opacity,
                            tint,
                            glow,
                            ..
                        } = draw
                        {
                            let expected = if phantom { 102 } else { 255 };
                            if tint.is_some() {
                                shadows += 1;
                                assert_eq!(u16::from(*opacity), expected * 153 / 255);
                                assert!(glow.is_none());
                            } else {
                                assert_eq!(u16::from(*opacity), expected);
                                assert!(glow.is_some());
                            }
                        }
                    }
                }
                assert!(fragments > 1, "raised item crosses cell boundaries");
                assert!(shadows > 0);
            }
        }
    }

    #[test]
    fn mimic_disguises_preserve_stealth_and_ebony_opacity_covers_raised_fragments() {
        use crate::level_map::{MapContents, MapMob};
        let mut level = Level::new(6, Feeling::None);
        level.set_size(5, 5);
        level.map.cells[12] = t::EMPTY;
        for kind in ["Mimic", "GoldenMimic", "EbonyMimic"] {
            for stealthy in [false, true] {
                let contents = MapContents {
                    mobs: vec![MapMob {
                        cell: 12,
                        kind: kind.into(),
                        stealthy,
                        sleeping: true,
                        approximate: false,
                        items: vec![],
                    }],
                    ..MapContents::default()
                };
                let scene = scene(DungeonSeed::MIN, &level, &[], MapKind::Regular, &contents);
                let mut fragments = 0;
                for sprite in scene
                    .layers
                    .iter()
                    .filter(|l| l.name == "actors")
                    .flat_map(|l| l.cells.iter().flatten())
                    .map(|&s| &scene.sprites[s])
                {
                    fragments += 1;
                    if stealthy {
                        assert_eq!(sprite.frames.len(), 1);
                    } else {
                        assert_eq!(sprite.frames.len(), 6);
                        assert_eq!(sprite.frame_duration_ms, 1000);
                    }
                    for draw in sprite.frames.iter().flatten() {
                        if let MapDraw::Blit { opacity, tint, .. } = draw {
                            let expected = match (kind == "EbonyMimic", tint.is_some()) {
                                (true, false) | (false, true) => 153,
                                (true, true) => 91,
                                (false, false) => 255,
                            };
                            assert_eq!(*opacity, expected);
                        }
                    }
                }
                assert_eq!(fragments, 2, "raised actor must span both cells");
            }
        }
    }

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
        let scene = scene(
            DungeonSeed::MIN,
            &level,
            &[],
            MapKind::Regular,
            &crate::level_map::MapContents::default(),
        );
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
                opacity: 255,
                tint: None,
                glow: None,
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
        let map = scene(
            DungeonSeed::MIN,
            &level,
            &[room],
            MapKind::Regular,
            &crate::level_map::MapContents::default(),
        );
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
                opacity: 255,
                tint: None,
                glow: None,
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
    }
}
