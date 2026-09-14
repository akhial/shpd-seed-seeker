//! WindParticle.Wind at the pinned game revision; coverage is a scouting heuristic.
#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::cast_precision_loss
)]
use super::particles::{curve, sample};
use crate::{
    geometry::terrain as t,
    level::Level,
    level_flags::LevelFlags,
    level_map::{MapDraw, MapEmitter, MapKind, MapParticle},
    room::Room,
};

pub(super) fn emitters(level: &Level, rooms: &[Room], kind: MapKind) -> Vec<MapEmitter> {
    if !level.map.cells.contains(&t::CHASM) {
        return Vec::new();
    }
    coverage(level, rooms, kind)
        .into_iter()
        .enumerate()
        .filter(|&(_, show)| show)
        .map(|(cell, _)| wind(cell))
        .collect()
}

fn neighbours(cell: usize, width: usize, len: usize) -> impl Iterator<Item = usize> {
    [
        cell.checked_sub(width),
        (cell % width > 0).then(|| cell - 1),
        (cell % width + 1 < width).then_some(cell + 1),
        (cell + width < len).then_some(cell + width),
    ]
    .into_iter()
    .flatten()
}

fn coverage(level: &Level, rooms: &[Room], kind: MapKind) -> Vec<bool> {
    let flags = LevelFlags::build(&level.map, false);
    // CavesBossLevel explicitly paints the bridge's abyss, even at map edges.
    // Its surrounding rock is WALL, so no distance or border heuristic is needed.
    if kind == MapKind::Regular && level.depth == 15 {
        return flags.pit;
    }
    let width = level.width() as usize;
    let mut visible = vec![false; level.len()];
    for room in rooms {
        for y in room.bounds.top + 1..room.bounds.bottom {
            for x in room.bounds.left + 1..room.bounds.right {
                let cell = (x + y * level.width()) as usize;
                visible[cell] = flags.pit[cell];
            }
        }
    }
    let ground: Vec<_> = (0..level.len())
        .map(|cell| !flags.pit[cell] && (flags.passable[cell] || flags.avoid[cell]))
        .collect();
    // Keep enclosed pits next to playable ground, including quest rooms without
    // regular Room metadata. Do not mistake sealed voids between walls for rooms.
    let mut visited = vec![false; level.len()];
    for cell in 0..level.len() {
        if !flags.pit[cell] || visited[cell] {
            continue;
        }
        let mut component = vec![cell];
        visited[cell] = true;
        let (mut edge, mut near_ground) = (false, false);
        let mut next = 0;
        while next < component.len() {
            let cell = component[next];
            next += 1;
            edge |= cell < width
                || cell >= level.len() - width
                || cell % width == 0
                || cell % width == width - 1;
            for neighbour in neighbours(cell, width, level.len()) {
                near_ground |= ground[neighbour];
                if flags.pit[neighbour] && !visited[neighbour] {
                    visited[neighbour] = true;
                    component.push(neighbour);
                }
            }
        }
        if !edge && near_ground {
            for cell in component {
                visible[cell] = true;
            }
        }
    }
    visible
}

fn wind(cell: usize) -> MapEmitter {
    // One reused particle per cell: 2.5s emission interval exceeds its 1–2s life.
    // Runtime wind RNG is visual only; use a coherent deterministic breeze here.
    let size = sample(cell, 0, 31) * 3.0;
    let life = 1000 + (sample(cell, 0, 32) * 1000.0) as u16;
    let angle = -0.65 + sample(cell, 0, 33) * 0.2;
    let velocity = [
        (angle.cos() * 5.0 * size).round() as i16,
        (angle.sin() * 5.0 * size).round() as i16,
    ];
    MapEmitter {
        cell,
        start_ms: None,
        wall_mask: true,
        clip_to_chasm: true,
        loop_ms: 2500,
        blend: None,
        image: MapDraw::Fill {
            rgba: [255; 4],
            destination: [0, 0, 1, 1],
        },
        velocity,
        acceleration: [0, 0],
        angular_speed: 0,
        alpha: curve(&[[0, 0], [500, (size * 100.0).round() as u16], [1000, 0]]),
        scale: curve(&[[0, 1000], [1000, 1000]]),
        scale_x: None,
        scale_y: None,
        particles: vec![MapParticle {
            birth_ms: (sample(cell, 0, 34) * 2500.0) as u16,
            lifespan_ms: life,
            position: [0, 1].map(|axis| {
                // PixelParticle's half-pixel origin, minus half the drift.
                500 + (sample(cell, 0, 35 + axis as u32) * 16000.0) as i32
                    - i32::from(velocity[axis]) * i32::from(life) / 2
            }),
            scale: (size * 1000.0).round() as u16,
            angle: 0,
        }],
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{geometry::painter as draw, level::Feeling};

    #[test]
    fn enclosed_pit_animates_but_rock_and_exterior_void_stay_clear() {
        let mut level = Level::new(18, Feeling::Chasm);
        level.set_size(31, 31);
        draw::fill(&mut level.map, 5, 5, 21, 21, t::WALL);
        draw::fill(&mut level.map, 6, 6, 19, 19, t::CHASM);
        draw::fill(&mut level.map, 6, 6, 19, 1, t::EMPTY);
        let winds = emitters(&level, &[], MapKind::Regular);
        // Even the far side of a large walled pit is included, beyond FOV range.
        assert_eq!(winds.len(), 19 * 18);
        assert!(winds.iter().any(|e| e.cell == 15 + 24 * 31));
        assert!(winds.iter().all(|e| level.map.cells[e.cell] == t::CHASM));
        assert!(!winds.iter().any(|e| e.cell == 15 + 26 * 31));
        assert!(!winds.iter().any(|e| e.cell == 0));
        // Concealing the room as solid walls also removes its effect.
        draw::fill(&mut level.map, 6, 6, 19, 19, t::WALL);
        assert!(emitters(&level, &[], MapKind::Regular).is_empty());
    }

    #[test]
    fn open_room_chasms_animate_without_following_the_connected_outer_void() {
        use crate::{geometry::Rect, rng::RandomStack, room::StandardRoomKind};
        let mut level = Level::new(18, Feeling::Chasm);
        level.set_size(31, 31);
        draw::fill(&mut level.map, 14, 10, 3, 11, t::EMPTY);
        let mut room = Room::standard(
            StandardRoomKind::ChasmBridge,
            &mut RandomStack::with_base_seed(0),
        );
        room.bounds = Rect::new(10, 10, 20, 20);
        let winds = emitters(&level, &[room], MapKind::Regular);
        assert!(winds.iter().any(|e| e.cell == 17 + 15 * 31));
        assert!(
            winds
                .iter()
                .all(|e| (11..20).contains(&(e.cell % 31)) && (11..20).contains(&(e.cell / 31)))
        );
        assert!(!winds.iter().any(|e| e.cell == 21 + 15 * 31));
        assert!(winds.iter().all(|e| e.clip_to_chasm && e.wall_mask));
        assert!(emitters(&level, &[], MapKind::Regular).is_empty());
    }
}
