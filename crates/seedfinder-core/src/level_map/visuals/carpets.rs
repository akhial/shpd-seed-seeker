//! City statue-room carpets from `StatuesRoom` and its entrance/exit subclasses.
//! Reconstruct visual rectangles from room bounds and the generated transition;
//! this never paints terrain or replays the room's random center selection.
use super::{Level, MapLayer, MapScene, intern, tile_sprite};
use crate::geometry::{Point, Rect, terrain as t};
use crate::room::{Room, RoomKind, StandardRoomKind};

pub(super) fn statues(scene: &mut MapScene, floor: &mut MapLayer, level: &Level, room: &Room) {
    if !(16..=20).contains(&level.depth)
        || !matches!(
            room.kind,
            RoomKind::Standard(StandardRoomKind::Statues)
                | RoomKind::Entrance(StandardRoomKind::Statues)
                | RoomKind::Exit(StandardRoomKind::Statues)
        )
    {
        return;
    }
    let b = room.bounds;
    let width = room.width();
    let height = room.height();
    if (width >= 11 || height >= 11)
        && let Some(cell) = match room.kind {
            RoomKind::Entrance(_) => level.entrance(),
            RoomKind::Exit(_) => level.exit(),
            _ => None,
        }
    {
        let p = level.map.cell_to_point(cell);
        let mut rect = Rect::new(p.x - 1, p.y - 1, p.x + 1, p.y + 1);
        // Even-sized rooms randomly choose one of two center cells. Expand
        // toward the other center, exactly as the game's inclusive rectangle.
        if width % 2 == 0 {
            if 2 * p.x < b.left + b.right {
                rect.right += 1;
            } else {
                rect.left -= 1;
            }
        }
        if height % 2 == 0 {
            if 2 * p.y < b.top + b.bottom {
                rect.bottom += 1;
            } else {
                rect.top -= 1;
            }
        }
        rectangle(scene, floor, level, rect, false);
    }
    let columns = (width + 1) / 6;
    let rows = (height + 1) / 6;
    let w = (width - 4 - (columns - 1)) / columns;
    let h = (height - 4 - (rows - 1)) / rows;
    let x_spacing = if columns % 2 == width % 2 { 2 } else { 1 };
    let y_spacing = if rows % 2 == height % 2 { 2 } else { 1 };
    for x in 0..columns {
        for y in 0..rows {
            let left = b.left + 2 + x * (w + x_spacing);
            let top = b.top + 2 + y * (h + y_spacing);
            rectangle(
                scene,
                floor,
                level,
                Rect::new(left, top, left + w - 1, top + h - 1),
                true,
            );
        }
    }
}

fn rectangle(scene: &mut MapScene, floor: &mut MapLayer, level: &Level, b: Rect, statues: bool) {
    for y in b.top..=b.bottom {
        for x in b.left..=b.right {
            let cell = level.map.point_to_cell(Point::new(x, y));
            let tile = if statues && x == b.left && y == b.top {
                85
            } else if statues && x == b.right && y == b.top {
                83
            } else if statues && x == b.left && y == b.bottom {
                86
            } else if statues && x == b.right && y == b.bottom {
                84
            } else {
                match level.map.cells[cell] {
                    t::ENTRANCE => 82,
                    t::EXIT => continue,
                    t::REGION_DECO if statues => 81,
                    _ => {
                        48 + u16::from(y == b.top)
                            + 2 * u16::from(x == b.right)
                            + 4 * u16::from(y == b.bottom)
                            + 8 * u16::from(x == b.left)
                    }
                }
            };
            let mut sprite = tile_sprite("carpet.png", tile);
            if let Some(index) = floor.cells[cell] {
                sprite.frames[0].splice(0..0, scene.sprites[index].frames[0].iter().cloned());
            }
            floor.cells[cell] = Some(intern(scene, sprite));
        }
    }
}
