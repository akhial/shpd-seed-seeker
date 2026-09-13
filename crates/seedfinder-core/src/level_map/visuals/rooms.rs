//! Generation-time `CustomTilemaps` from the pinned room painters.
use super::{Level, MapDraw, MapLayer, MapScene, MapSprite, intern, layer, objects, tile_sprite};
use crate::geometry::terrain as t;
use crate::level_map::MapContents;
use crate::room::{QuestRoomKind, Room, RoomKind};

#[allow(clippy::too_many_lines)] // Pinned custom-tile layouts in rendering order.
pub(super) fn layers(
    scene: &mut MapScene,
    level: &Level,
    rooms: &[Room],
    contents: &MapContents,
) -> [MapLayer; 3] {
    let mut floor = layer("room_floor", level.len());
    let mut terrain = layer("room_terrain", level.len());
    let mut walls = layer("room_walls", level.len());
    for room in rooms {
        super::carpets::statues(scene, &mut floor, level, room);
        if room.kind == RoomKind::Quest(QuestRoomKind::Blacksmith) {
            let b = room.bounds;
            let w = room.width() - 4;
            let h = room.height() - 4;
            for y in 0..h {
                for x in 0..w {
                    let cell = level
                        .map
                        .point_to_cell(crate::geometry::Point::new(b.left + 2 + x, b.top + 2 + y));
                    let tile = if y == 0 && x < 3 {
                        [7, 16, 17][usize::try_from(x).unwrap()]
                    } else if y == 1 && x == 2 {
                        18
                    } else if level.map.cells[cell] == t::PEDESTAL {
                        if y == h - 1 { 20 } else { 21 }
                    } else if level.map.cells[cell] == t::EMPTY_SP {
                        if y == h - 1 {
                            if x == 0 {
                                12
                            } else if x == w - 1 {
                                14
                            } else {
                                13
                            }
                        } else if x == 0 {
                            8
                        } else if x == w - 1 {
                            10
                        } else {
                            -1
                        }
                    } else {
                        -1
                    };
                    put(scene, &mut floor, cell, "caves_quest.png", tile);
                }
            }
            let cell = level
                .map
                .point_to_cell(crate::geometry::Point::new(b.left + 2, b.top + 1));
            put(scene, &mut walls, cell, "caves_quest.png", 3);
        }
    }
    for feature in &contents.features {
        let p = level.map.cell_to_point(feature.cell);
        match feature.kind.as_str() {
            "ImpEntrance" => {
                // Two rectangular carpets under the torn 5x5 entrance artwork.
                for (ox, oy, w, h) in [(-1, 1, 7, 3), (1, -1, 3, 7)] {
                    for y in 0..h {
                        for x in 0..w {
                            let cell = level.map.point_to_cell(crate::geometry::Point::new(
                                p.x + ox + x,
                                p.y + oy + y,
                            ));
                            let tile = 48
                                + i32::from(y == 0)
                                + 2 * i32::from(x == w - 1)
                                + 4 * i32::from(y == h - 1)
                                + 8 * i32::from(x == 0);
                            put(scene, &mut floor, cell, "carpet.png", tile);
                        }
                    }
                }
                for y in 0..5 {
                    for x in 0..5 {
                        let cell = level
                            .map
                            .point_to_cell(crate::geometry::Point::new(p.x + x, p.y + y));
                        append(
                            scene,
                            &mut floor,
                            cell,
                            tile_sprite("city_quest.png", u16::try_from(x + 16 * y).unwrap()),
                        );
                    }
                }
                // Custom terrain hangs down the wall face, skipping doorways.
                for (x, y) in [(1, 0), (3, 0), (5, 0), (0, 1), (6, 1)] {
                    let cell = level
                        .map
                        .point_to_cell(crate::geometry::Point::new(p.x - 1 + x, p.y - 2 + y));
                    if !level.passable[cell] {
                        // Banner variant is cosmetic runtime randomness in-game.
                        put(
                            scene,
                            &mut terrain,
                            cell,
                            "city_quest.png",
                            80 + i32::try_from(cell % 2).unwrap(),
                        );
                        let below = cell + usize::try_from(level.width()).unwrap();
                        if x != 3 || level.map.cells[below] != t::PEDESTAL {
                            put(scene, &mut terrain, below, "city_quest.png", 82);
                        }
                    }
                }
            }
            "ImpBarrier" => {
                for y in 0..3 {
                    for x in 0..3 {
                        let cell = level
                            .map
                            .point_to_cell(crate::geometry::Point::new(p.x + x, p.y + y));
                        let sprite = tile_sprite(
                            "city_quest.png",
                            u16::try_from(5 + x + 16 * (1 + y)).unwrap(),
                        );
                        // EntranceBarrier.update(): alpha = .3 + .3*sin(time).
                        // This is a floor tile animation, below heaps and actors.
                        let frames = (0..126)
                            .map(|frame| {
                                let mut draws = sprite.frames[0].clone();
                                for draw in &mut draws {
                                    if let MapDraw::Blit { opacity, .. } = draw {
                                        #[allow(
                                            clippy::cast_possible_truncation,
                                            clippy::cast_sign_loss
                                        )]
                                        {
                                            *opacity = ((0.3
                                                + 0.3
                                                    * (f64::from(frame) * std::f64::consts::TAU
                                                        / 126.0)
                                                        .sin())
                                                * 255.0)
                                                .round()
                                                as u8;
                                        }
                                    }
                                }
                                draws
                            })
                            .collect();
                        append(
                            scene,
                            &mut floor,
                            cell,
                            MapSprite {
                                frame_duration_ms: 50,
                                frames,
                            },
                        );
                    }
                }
            }
            "HiddenWell" => {
                if objects::visible(level, feature.cell) {
                    put(
                        scene,
                        &mut floor,
                        feature.cell,
                        "weak_floor.png",
                        i32::try_from(level.depth / 5).unwrap(),
                    );
                }
            }
            "DemonSpawnerFloor" => {
                for y in 0..feature.height {
                    for x in 0..feature.width {
                        let cell = level
                            .map
                            .point_to_cell(crate::geometry::Point::new(p.x + x, p.y + y));
                        let image = if let Some(mob) = contents
                            .mobs
                            .iter()
                            .find(|m| m.kind == "DemonSpawner" && m.cell.abs_diff(cell) <= 1)
                        {
                            38 + i32::try_from(cell).unwrap() - i32::try_from(mob.cell).unwrap()
                        } else if level.map.cells[cell] == t::EMPTY_DECO {
                            27
                        } else {
                            19
                        };
                        put(scene, &mut floor, cell, "halls_special.png", image);
                    }
                }
            }
            "MassGraveBones" => {
                const MASK: [&str; 9] = [
                    "000111000",
                    "001111100",
                    "111111111",
                    "111111111",
                    "111111111",
                    "111111111",
                    "111111111",
                    "111000111",
                    "110000011",
                ];
                for (y, row) in MASK.iter().enumerate() {
                    for (x, draw) in row.bytes().enumerate() {
                        if draw == b'1' {
                            let cell =
                                feature.cell + x + y * usize::try_from(level.width()).unwrap();
                            put(
                                scene,
                                &mut floor,
                                cell,
                                "prison_quest.png",
                                i32::try_from(5 + x + 16 * y).unwrap(),
                            );
                        }
                    }
                }
                for x in [2, 6] {
                    let cell = level
                        .map
                        .point_to_cell(crate::geometry::Point::new(p.x + x, p.y + 2));
                    put(scene, &mut walls, cell, "prison_quest.png", 4);
                }
            }
            "RitualMarker" | "RitualTable" => {
                let start = if feature.kind == "RitualMarker" {
                    32
                } else {
                    0
                };
                for y in 0..feature.height {
                    for x in 0..feature.width {
                        let cell = level
                            .map
                            .point_to_cell(crate::geometry::Point::new(p.x + x, p.y + y));
                        if objects::visible(level, cell) {
                            put(
                                scene,
                                &mut floor,
                                cell,
                                "prison_quest.png",
                                start + x + 16 * y,
                            );
                        }
                    }
                }
            }
            _ => {}
        }
    }
    [floor, terrain, walls]
}

fn append(scene: &mut MapScene, target: &mut MapLayer, cell: usize, mut sprite: MapSprite) {
    if let Some(index) = target.cells[cell] {
        let background = &scene.sprites[index].frames[0];
        for frame in &mut sprite.frames {
            frame.splice(0..0, background.iter().cloned());
        }
    }
    target.cells[cell] = Some(intern(scene, sprite));
}
fn put(scene: &mut MapScene, target: &mut MapLayer, cell: usize, asset: &'static str, tile: i32) {
    if let Ok(tile) = u16::try_from(tile) {
        target.cells[cell] = Some(intern(scene, tile_sprite(asset, tile)));
    }
}
