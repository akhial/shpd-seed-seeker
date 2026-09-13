use super::{Level, MapDraw, MapLayer, MapScene, MapSprite, intern, layer, t, tile_sprite};

/// Boss custom tiles are kept in the portable scene so all renderers agree.
#[allow(clippy::too_many_lines)] // Pinned custom-tile layouts, in game layer order.
pub(super) fn layers(
    scene: &mut MapScene,
    level: &Level,
    rooms: &[crate::room::Room],
    reveal: bool,
) -> [MapLayer; 4] {
    use crate::room::{RoomKind, SecretRoomKind, StandardRoomKind as S};
    let mut floor = layer("boss_floor", level.len());
    let mut terrain = layer("boss_terrain", level.len());
    let mut walls = layer("boss_walls", level.len());
    let mut actors = layer("boss_actors", level.len());
    if level.depth == 15 {
        const ENTRY: [i32; 55] = [
            -1, 7, 7, 7, -1, -1, 1, 2, 3, -1, 8, 1, 2, 3, 12, 16, 9, 10, 11, 20, 16, 16, 22, 20,
            20, 16, 17, 18, 19, 20, 16, 16, 18, 20, 20, 16, 17, 18, 19, 20, 16, 16, 18, 20, 20, 16,
            17, 18, 19, 20, 24, 25, 26, 27, 28,
        ];
        const ENTRY_TERRAIN: [i32; 55] = [
            -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, 8, -1, -1, -1, 12, 30, -1, -1, -1, 31, -1, -1,
            -1, -1, -1, -1, 17, -1, 19, -1, -1, -1, -1, -1, -1, -1, 17, -1, 19, -1, -1, -1, -1, -1,
            -1, -1, 17, -1, 19, -1, -1, -1, -1, -1, -1,
        ];
        const OVERHANG: [i32; 55] = [
            0, 7, 7, 7, 4, 0, 15, 15, 15, 4, -1, 23, 23, 23, -1, -1, -1, -1, -1, -1, -1, 6, -1, 14,
            -1, -1, -1, -1, -1, -1, -1, 6, -1, 14, -1, -1, -1, -1, -1, -1, -1, 6, -1, 14, -1, -1,
            -1, -1, -1, -1, -1, -1, -1, -1, -1,
        ];
        for y in 0..11 {
            for x in 0..33 {
                let tile = if (14..19).contains(&x) {
                    ENTRY[usize::try_from(x - 14 + y * 5).expect("entry tile")]
                } else if y == 2 {
                    13
                } else if y == 3 && x != 9 && x != 23 {
                    21
                } else {
                    -1
                };
                boss_tile(scene, &mut floor, level, x, y, "caves_boss.png", tile);
            }
        }
        for y in 0..11 {
            for x in 0..5 {
                boss_tile(
                    scene,
                    &mut terrain,
                    level,
                    x + 14,
                    y,
                    "caves_boss.png",
                    ENTRY_TERRAIN[usize::try_from(x + y * 5).expect("entry tile")],
                );
                boss_tile(
                    scene,
                    &mut walls,
                    level,
                    x + 14,
                    y,
                    "caves_boss.png",
                    OVERHANG[usize::try_from(x + y * 5).expect("entry tile")],
                );
            }
        }
        for (cell, &tile) in level.map.cells.iter().enumerate() {
            let p = level.map.cell_to_point(cell);
            if tile == t::INACTIVE_TRAP {
                boss_tile(scene, &mut floor, level, p.x, p.y, "caves_boss.png", 37);
            }
            if p.y == 13 && (14..19).contains(&p.x) {
                boss_tile(
                    scene,
                    &mut floor,
                    level,
                    p.x,
                    p.y,
                    "caves_boss.png",
                    40 + p.x - 14,
                );
            }
        }
        for cell in crate::boss_floor::PYLONS {
            let p = level.map.cell_to_point(cell);
            for dy in -1..=1 {
                for dx in -1..=1 {
                    if dx != 0 || dy != 0 {
                        boss_tile(
                            scene,
                            &mut floor,
                            level,
                            p.x + dx,
                            p.y + dy,
                            "caves_boss.png",
                            54 + dx + 8 * dy,
                        );
                    }
                }
            }
            boss_actor(scene, &mut actors, level, cell, "pylon.png", [10, 20, 5]);
        }
    } else {
        for room in rooms {
            let b = room.bounds;
            if room.kind == RoomKind::Secret(SecretRoomKind::RatKing) && reveal {
                for y in 0..3 {
                    for x in 0..3 {
                        let tile = i32::from(y == 0)
                            + 2 * i32::from(x == 2)
                            + 4 * i32::from(y == 2)
                            + 8 * i32::from(x == 0);
                        boss_tile(
                            scene,
                            &mut floor,
                            level,
                            b.left + 2 + x,
                            b.top + 2 + y,
                            "carpet.png",
                            tile,
                        );
                    }
                }
                for y in b.top + 1..b.bottom {
                    for x in b.left + 1..b.right {
                        let cell = level.map.point_to_cell(crate::geometry::Point::new(x, y));
                        if level.map.cells[cell] == t::CUSTOM_DECO {
                            boss_tile(scene, &mut floor, level, x, y, "rat_king_room.png", 0);
                            boss_tile(scene, &mut terrain, level, x, y, "rat_king_room.png", 1);
                            boss_tile(scene, &mut walls, level, x, y - 1, "rat_king_room.png", 2);
                        }
                    }
                }
                boss_tile(
                    scene,
                    &mut floor,
                    level,
                    b.left + 3,
                    b.top + 3,
                    "rat_king_room.png",
                    3,
                );
                let cell = level
                    .map
                    .point_to_cell(crate::geometry::Point::new(b.left + 3, b.top + 3));
                boss_actor(scene, &mut actors, level, cell, "ratking.png", [16, 16, 6]);
            } else if matches!(
                room.kind,
                RoomKind::Standard(
                    S::DiamondGoo | S::WalledGoo | S::ThinPillarsGoo | S::ThickPillarsGoo
                )
            ) {
                let w = 4 + room.width() % 2;
                let h = 4 + room.height() % 2;
                for y in 0..h {
                    for x in 0..w {
                        let tile = if (x == 0 || x == w - 1) && (y == 0 || y == h - 1) {
                            -1
                        } else if (x == 1 && y == 0) || (x == 0 && y == 1) {
                            0
                        } else if (x == w - 2 && y == 0) || (x == w - 1 && y == 1) {
                            1
                        } else if (x == 1 && y == h - 1) || (x == 0 && y == h - 2) {
                            2
                        } else if (x == w - 2 && y == h - 1) || (x == w - 1 && y == h - 2) {
                            3
                        } else if x == 0 {
                            4
                        } else if y == 0 {
                            5
                        } else if x == w - 1 {
                            6
                        } else if y == h - 1 {
                            7
                        } else {
                            8
                        };
                        boss_tile(
                            scene,
                            &mut floor,
                            level,
                            b.left + room.width() / 2 - 2 + x,
                            b.top + room.height() / 2 - 2 + y,
                            "sewer_boss.png",
                            tile,
                        );
                    }
                }
            }
        }
        if let Some(exit) = level.exit() {
            let p = level.map.cell_to_point(exit);
            for (i, tile) in [21, -1, 22, 23, 23, 23, 24, 24, 24].into_iter().enumerate() {
                boss_tile(
                    scene,
                    &mut floor,
                    level,
                    p.x - 1 + i32::try_from(i % 3).expect("exit column"),
                    p.y + i32::try_from(i / 3).expect("exit row"),
                    "sewer_boss.png",
                    tile,
                );
            }
            for (i, tile) in [16, 17, 18, 19, -1, 20].into_iter().enumerate() {
                boss_tile(
                    scene,
                    &mut walls,
                    level,
                    p.x - 1 + i32::try_from(i % 3).expect("exit column"),
                    p.y - 2 + i32::try_from(i / 3).expect("exit row"),
                    "sewer_boss.png",
                    tile,
                );
            }
        }
    }
    [floor, terrain, walls, actors]
}

fn boss_tile(
    scene: &mut MapScene,
    layer: &mut MapLayer,
    level: &Level,
    x: i32,
    y: i32,
    asset: &'static str,
    tile: i32,
) {
    if tile < 0 {
        return;
    }
    let cell = level.map.point_to_cell(crate::geometry::Point::new(x, y));
    layer.cells[cell] = Some(intern(
        scene,
        tile_sprite(asset, u16::try_from(tile).expect("boss atlas index")),
    ));
}

/// Split raised actor art across cells; the scene contract uses unsigned rectangles.
fn boss_actor(
    scene: &mut MapScene,
    layer: &mut MapLayer,
    level: &Level,
    cell: usize,
    asset: &'static str,
    [width, height, raise]: [u16; 3],
) {
    let above = height + raise - 16;
    for (cell, sy, dy, h) in [
        (
            cell - usize::try_from(level.width()).expect("positive map width"),
            0,
            16 - above,
            above,
        ),
        (cell, above, 0, height - above),
    ] {
        layer.cells[cell] = Some(intern(
            scene,
            MapSprite {
                frame_duration_ms: 1,
                frames: vec![vec![MapDraw::Blit {
                    asset,
                    source: [0, sy, width, h],
                    destination: [(16 - width) / 2, dy, width, h],
                }]],
            },
        ));
    }
}
