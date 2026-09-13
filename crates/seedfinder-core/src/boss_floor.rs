//! Initial boss terrain from the pinned v4.0.0 SewerBossLevel/CavesBossLevel.
//! These floors have isolated depth RNGs and do not advance searchable run state.

use crate::builder::{Builder, FigureEightBuilder};
use crate::challenges::Challenges;
use crate::geometry::{PathFinder, Point, painter as draw, terrain as t};
use crate::level::{Feeling, HeapKind, Level, PaintItem, TransitionKind};
use crate::painter::{
    RegularPainter, RoomPaintDispatch, decorate_sewers, fill_room, fill_room_margin,
    generate_patch, set_shared_door_type,
};
use crate::rng::{RandomStack, seed_for_depth};
use crate::room::{
    DoorType, Room, RoomKind, SecretRoomKind, StandardRoomKind as S, create_standard_room,
};
use crate::run::RunState;
use crate::sewer_rooms::{GeneratorRoomContent, SewerRoomDispatcher, paint_perimeter};

pub(crate) fn generate(seed: i64, depth: u8, challenges: Challenges) -> (Level, Vec<Room>) {
    let mut rng = RandomStack::with_base_seed(0);
    rng.push(seed_for_depth(seed, u32::from(depth), 0));
    if depth == 15 {
        return (caves(&mut rng, challenges), Vec::new());
    }
    assert_eq!(depth, 5);
    sewers(seed, &mut rng, challenges)
}

fn sewers(seed: i64, rng: &mut RandomStack, challenges: Challenges) -> (Level, Vec<Room>) {
    let mut builder = FigureEightBuilder::default()
        .set_loop_shape(2, rng.float_between(0.3, 0.8), 0.0)
        .set_path_length(1.0, &[1.0])
        .set_tunnel_length(&[1.0, 2.0], &[1.0]);
    let mut initial = vec![
        Room::entrance(S::SewerBossEntrance, rng),
        Room::exit(S::SewerBossExit, rng),
    ];
    for _ in 0..3 {
        let mut room = create_standard_room(5, rng);
        room.set_size_category(0, 0, rng);
        initial.push(room);
    }
    let goo = [
        S::DiamondGoo,
        S::WalledGoo,
        S::ThinPillarsGoo,
        S::ThickPillarsGoo,
    ][usize::try_from(rng.int_bound(4)).expect("nonnegative room variant")];
    initial.push(Room::standard(goo, rng));
    initial.push(Room::secret(SecretRoomKind::RatKing));
    rng.shuffle_list(&mut initial);
    builder = builder.set_landmark_room(
        initial
            .iter()
            .position(|r| r.kind == RoomKind::Standard(goo))
            .unwrap(),
    );
    let mut rooms = loop {
        let mut rooms = initial.clone();
        if builder.build(&mut rooms, 5, rng) {
            break rooms;
        }
        // Failed attempts retain room dimensions and positions in Java.
        let count = initial.len();
        initial.clone_from_slice(&rooms[..count]);
        for r in &mut initial {
            r.connected.clear();
            r.neighbours.clear();
        }
    };
    let mut level = Level::new(5, Feeling::None);
    let mut run = RunState::with_challenges(seed, challenges);
    let mut items = Vec::new();
    let mut dispatch = BossDispatch {
        common: SewerRoomDispatcher::new(GeneratorRoomContent {
            depth: 5,
            generator: &mut run.generator,
            items_to_spawn: &mut items,
        }),
    };
    RegularPainter::default()
        .set_water(0.5, 5)
        .set_grass(0.2, 4)
        .paint_with_decorator(&mut level, &mut rooms, &mut dispatch, rng, decorate_sewers)
        .expect("valid boss graph");
    (level, rooms)
}

struct BossDispatch<'a> {
    common: SewerRoomDispatcher<GeneratorRoomContent<'a>>,
}
fn is_goo(kind: RoomKind) -> bool {
    matches!(
        kind,
        RoomKind::Standard(S::DiamondGoo | S::WalledGoo | S::ThinPillarsGoo | S::ThickPillarsGoo)
    )
}
fn random_point(room: &Room, margin: i32, rng: &mut RandomStack) -> Point {
    Point::new(
        rng.int_range(room.bounds.left + margin, room.bounds.right - margin),
        rng.int_range(room.bounds.top + margin, room.bounds.bottom - margin),
    )
}
impl RoomPaintDispatch for BossDispatch<'_> {
    #[allow(clippy::too_many_lines)] // Keep upstream room painter draw order auditable.
    fn paint_room(
        &mut self,
        level: &mut Level,
        rooms: &mut [Room],
        id: usize,
        rng: &mut RandomStack,
    ) {
        let room = rooms[id].clone();
        let b = room.bounds;
        let w = room.width();
        let h = room.height();
        let special = is_goo(room.kind) || room.is_entrance() || room.is_exit() || room.is_secret();
        if !special {
            self.common.paint_room(level, rooms, id, rng);
            return;
        }
        fill_room(level, &room, t::WALL);
        fill_room_margin(level, &room, 1, t::EMPTY);
        for c in &room.connected {
            set_shared_door_type(
                rooms,
                id,
                c.room,
                if room.is_secret() {
                    DoorType::Hidden
                } else {
                    DoorType::Regular
                },
            );
        }
        match room.kind {
            RoomKind::Entrance(_) => {
                draw::fill(
                    &mut level.map,
                    b.left + 1,
                    b.top + 1,
                    w - 2,
                    1,
                    t::WALL_DECO,
                );
                draw::fill(&mut level.map, b.left + 1, b.top + 2, w - 2, 1, t::WATER);
                let cell = level.point_to_cell(random_point(&room, 3, rng));
                level.map.cells[cell] = t::ENTRANCE;
                level.add_transition(cell, TransitionKind::RegularEntrance);
                for c in &room.connected {
                    let p = c.door.unwrap().point;
                    if p.y == b.top || p.y == b.top + 1 {
                        let cell = level.point_to_cell(room.point_inside(p, 1));
                        level.map.cells[cell] = t::WATER;
                    }
                }
            }
            RoomKind::Exit(_) => {
                let c = room.center(rng);
                draw::fill(&mut level.map, c.x - 1, c.y - 1, 3, 2, t::WALL);
                draw::fill(&mut level.map, c.x - 1, c.y + 1, 3, 1, t::EMPTY_SP);
                let cell = level.point_to_cell(c);
                level.map.cells[cell] = t::LOCKED_EXIT;
                level.add_transition(cell, TransitionKind::RegularExit);
            }
            RoomKind::Secret(_) => {
                fill_room_margin(level, &room, 2, t::EMPTY_SP);
                let door = room.connected[0].door.unwrap().point;
                let center = room.center(rng);
                for (dx, dy) in [
                    (-2, -2),
                    (0, -2),
                    (2, -2),
                    (2, 0),
                    (2, 2),
                    (0, 2),
                    (-2, 2),
                    (-2, 0),
                ] {
                    let p = Point::new(center.x + dx, center.y + dy);
                    if (p.x - door.x).abs().max((p.y - door.y).abs()) >= 2 {
                        let cell = level.point_to_cell(p);
                        level.map.cells[cell] = t::CUSTOM_DECO;
                    }
                }
                let center = level.point_to_cell(center);
                level.mark_mob(center);
                for y in b.top..=b.bottom {
                    for x in b.left..=b.right {
                        let cell = level.point_to_cell(Point::new(x, y));
                        if cell != center && matches!(level.map.cells[cell], t::EMPTY | t::EMPTY_SP)
                        {
                            let quantity = rng.int_range(5, 20);
                            level.drop_item(
                                PaintItem::Generated(crate::generator::GeneratedItem::Gold {
                                    quantity,
                                }),
                                cell,
                                HeapKind::Heap,
                            );
                        }
                    }
                }
            }
            RoomKind::Standard(kind) => {
                match kind {
                    S::DiamondGoo => {
                        fill_room(level, &room, t::WALL);
                        draw::fill_diamond(
                            &mut level.map,
                            b.left + 1,
                            b.top + 1,
                            w - 2,
                            h - 2,
                            t::EMPTY,
                        );
                        for c in &room.connected {
                            let mut p = c.door.unwrap().point;
                            let dir = if p.x == b.left {
                                Point::new(1, 0)
                            } else if p.y == b.top {
                                Point::new(0, 1)
                            } else if p.x == b.right {
                                Point::new(-1, 0)
                            } else {
                                Point::new(0, -1)
                            };
                            loop {
                                let cell = level.point_to_cell(p);
                                level.map.cells[cell] = t::EMPTY_SP;
                                p.x += dir.x;
                                p.y += dir.y;
                                if level.map.cells[level.point_to_cell(p)] != t::WALL {
                                    break;
                                }
                            }
                        }
                    }
                    S::WalledGoo => {
                        fill_room_margin(level, &room, 1, t::EMPTY_SP);
                        fill_room_margin(level, &room, 2, t::EMPTY);
                        let pw = (w - 6) / 2;
                        let ph = (h - 6) / 2;
                        for (x, y, ww, hh) in [
                            (b.left + 2, b.top + 2, pw, 1),
                            (b.left + 2, b.top + 2, 1, ph),
                            (b.left + 2, b.bottom - 2, pw, 1),
                            (b.left + 2, b.bottom - 1 - ph, 1, ph),
                            (b.right - 1 - pw, b.top + 2, pw, 1),
                            (b.right - 2, b.top + 2, 1, ph),
                            (b.right - 1 - pw, b.bottom - 2, pw, 1),
                            (b.right - 2, b.bottom - 1 - ph, 1, ph),
                        ] {
                            draw::fill(&mut level.map, x, y, ww, hh, t::WALL);
                        }
                    }
                    S::ThinPillarsGoo => {
                        fill_room_margin(level, &room, 1, t::WATER);
                        let pw = if w == 14 { 4 } else { 2 } + w % 2;
                        let ph = if h == 14 { 4 } else { 2 } + h % 2;
                        let dy = if h < 12 { 2 } else { 3 };
                        let dx = if w < 12 { 2 } else { 3 };
                        for y in [b.top + dy, b.bottom - dy] {
                            draw::fill(&mut level.map, b.left + (w - pw) / 2, y, pw, 1, t::WALL);
                        }
                        for x in [b.left + dx, b.right - dx] {
                            draw::fill(&mut level.map, x, b.top + (h - ph) / 2, 1, ph, t::WALL);
                        }
                        paint_perimeter(level, rooms, id, t::EMPTY_SP);
                    }
                    S::ThickPillarsGoo => {
                        fill_room_margin(level, &room, 1, t::WATER);
                        let pw = (w - 8) / 2;
                        let ph = (h - 8) / 2;
                        for x in [b.left + 2, b.right - 2 - pw] {
                            for y in [b.top + 2, b.bottom - 2 - ph] {
                                draw::fill(&mut level.map, x, y, pw + 1, ph + 1, t::WALL);
                            }
                        }
                        paint_perimeter(level, rooms, id, t::EMPTY_SP);
                    }
                    _ => unreachable!(),
                }
                if matches!(kind, S::DiamondGoo | S::WalledGoo) {
                    draw::fill(
                        &mut level.map,
                        b.left + w / 2 - 1,
                        b.top + h / 2 - 2,
                        2 + w % 2,
                        4 + h % 2,
                        t::WATER,
                    );
                    draw::fill(
                        &mut level.map,
                        b.left + w / 2 - 2,
                        b.top + h / 2 - 1,
                        4 + w % 2,
                        2 + h % 2,
                        t::WATER,
                    );
                }
                let cell = level.point_to_cell(room.center(rng));
                level.mark_mob(cell);
            }
            _ => unreachable!(),
        }
    }
    fn can_merge(
        &self,
        level: &Level,
        rooms: &[Room],
        room: usize,
        other: usize,
        p: Point,
        tile: i32,
    ) -> bool {
        if is_goo(rooms[room].kind) || rooms[room].is_secret() {
            false
        } else {
            self.common.can_merge(level, rooms, room, other, p, tile)
        }
    }
    fn can_place_water(&self, level: &Level, rooms: &[Room], room: usize, p: Point) -> bool {
        if rooms[room].is_secret()
            || matches!(
                rooms[room].kind,
                RoomKind::Standard(S::DiamondGoo | S::WalledGoo)
            )
        {
            false
        } else if is_goo(rooms[room].kind) || rooms[room].is_entrance() || rooms[room].is_exit() {
            true
        } else {
            self.common.can_place_water(level, rooms, room, p)
        }
    }
    fn can_place_grass(&self, level: &Level, rooms: &[Room], room: usize, p: Point) -> bool {
        if rooms[room].is_secret() {
            false
        } else if is_goo(rooms[room].kind) || rooms[room].is_entrance() || rooms[room].is_exit() {
            true
        } else {
            self.common.can_place_grass(level, rooms, room, p)
        }
    }
}

pub(crate) const PYLONS: [usize; 4] = [4 + 13 * 33, 28 + 13 * 33, 4 + 37 * 33, 28 + 37 * 33];
fn caves(rng: &mut RandomStack, challenges: Challenges) -> Level {
    loop {
        let mut level = Level::new(15, Feeling::None);
        level.set_size(33, 42);
        draw::fill(&mut level.map, 14, 13, 5, 1, t::CUSTOM_DECO);
        draw::fill_ellipse(&mut level.map, 5, 14, 23, 23, t::EMPTY);
        let patch = generate_patch(33, 28, 0.15, 2, true, rng);
        for cell in 14 * 33..level.len() {
            if level.map.cells[cell] == t::EMPTY {
                if patch[cell - 14 * 33] {
                    level.map.cells[cell] = t::WATER;
                } else if rng.int_bound(if challenges.contains(Challenges::STRONGER_BOSSES) {
                    4
                } else {
                    8
                }) == 0
                {
                    level.map.cells[cell] = t::INACTIVE_TRAP;
                }
            }
        }
        mirrored(
            &mut level,
            &ENTRANCES[usize::try_from(rng.int_bound(4)).expect("nonnegative room variant")],
            8,
            9,
            18,
            23,
            32,
        );
        level.map.cells[16 + 25 * 33] = t::ENTRANCE;
        level.add_transition(16 + 25 * 33, TransitionKind::RegularEntrance);
        mirrored(
            &mut level,
            &CORNERS[usize::try_from(rng.int_bound(4)).expect("nonnegative room variant")],
            10,
            2,
            11,
            30,
            39,
        );
        let child = rng.long();
        rng.push(child);
        crate::caves_rooms::decorate_caves(&mut level, &[], &[], rng);
        rng.pop();
        for (x, y, w, h, tile) in [
            (0, 3, 33, 4, t::CHASM),
            (6, 7, 21, 1, t::CHASM),
            (9, 3, 1, 6, t::REGION_DECO_ALT),
            (23, 3, 1, 6, t::REGION_DECO_ALT),
            (10, 8, 13, 1, t::CHASM),
            (12, 9, 9, 1, t::CHASM),
            (13, 10, 7, 1, t::CHASM),
            (14, 3, 5, 10, t::EMPTY),
            (15, 2, 3, 3, t::EMPTY_SP),
            (15, 5, 3, 1, t::STATUE),
            (15, 7, 3, 1, t::STATUE),
            (15, 9, 3, 1, t::STATUE),
            (16, 5, 1, 6, t::EMPTY_SP),
            (15, 0, 3, 3, t::EXIT),
        ] {
            draw::fill(&mut level.map, x, y, w, h, tile);
        }
        level.add_transition(16 + 2 * 33, TransitionKind::RegularExit);
        let pass: Vec<_> = level
            .map
            .cells
            .iter()
            .map(|t| matches!(*t, t::EMPTY | t::EMPTY_SP | t::EMPTY_DECO))
            .collect();
        let mut paths = PathFinder::new(33, 42);
        paths.build_distance_map(16 + 25 * 33, &pass);
        if PYLONS.iter().all(|&p| paths.distance[p] != i32::MAX) {
            return level;
        }
    }
}
fn mirrored(
    level: &mut Level,
    tiles: &[i32],
    size: usize,
    left: usize,
    top: usize,
    right: usize,
    bottom: usize,
) {
    for (i, &tile) in tiles.iter().enumerate() {
        if tile < 0 {
            continue;
        }
        let x = i % size;
        let y = i / size;
        for cell in [
            left + x + (top + y) * 33,
            right - x + (top + y) * 33,
            right - x + (bottom - y) * 33,
            left + x + (bottom - y) * 33,
        ] {
            level.map.cells[cell] = tile;
        }
    }
}

// Preserve the upstream row layout so the mirrored templates are reviewable.
#[rustfmt::skip]
const ENTRANCES: [[i32; 64]; 4] = {
    use t::{WALL as W, EMPTY as E};
    [
        [
            -1, -1, -1, -1, -1, -1, -1, -1,
            -1, -1, -1, -1, -1, -1, -1, -1,
            -1, -1, -1, -1, W, E, W, W,
            -1, -1, -1, W, W, E, W, W,
            -1, -1, W, W, E, E, E, E,
            -1, -1, E, E, E, W, W, E,
            -1, -1, W, W, E, W, E, E,
            -1, -1, W, W, E, E, E, E,
        ],
        [
            -1, -1, -1, -1, -1, -1, -1, -1,
            -1, -1, -1, -1, -1, -1, -1, -1,
            -1, -1, -1, -1, -1, E, E, E,
            -1, -1, -1, W, E, W, W, E,
            -1, -1, -1, E, E, E, E, E,
            -1, -1, E, W, E, W, W, E,
            -1, -1, E, W, E, W, E, E,
            -1, -1, E, E, E, E, E, E,
        ],
        [
            -1, -1, -1, -1, -1, -1, -1, -1,
            -1, -1, -1, -1, -1, -1, -1, -1,
            -1, -1, -1, -1, -1, -1, -1, -1,
            -1, -1, -1, W, W, E, W, W,
            -1, -1, -1, W, W, E, W, W,
            -1, -1, -1, E, E, E, E, E,
            -1, -1, -1, W, W, E, W, E,
            -1, -1, -1, W, W, E, E, E,
        ],
        [
            -1, -1, -1, -1, -1, -1, -1, -1,
            -1, -1, -1, -1, -1, -1, -1, E,
            -1, -1, -1, -1, -1, -1, W, E,
            -1, -1, -1, -1, -1, W, W, E,
            -1, -1, -1, -1, W, W, W, E,
            -1, -1, -1, W, W, W, W, E,
            -1, -1, W, W, W, W, E, E,
            -1, E, E, E, E, E, E, E,
        ],
    ]
};

#[rustfmt::skip]
const CORNERS: [[i32; 100]; 4] = {
    use t::{WALL as W, EMPTY as E, EMPTY_SP as S};
    [
        [
            W, W, W, W, W, W, W, W, W, W,
            W, S, S, S, E, E, E, W, W, W,
            W, S, S, S, W, W, E, E, W, W,
            W, S, S, S, W, W, W, E, E, W,
            W, E, W, W, W, W, W, W, E, -1,
            W, E, W, W, W, W, W, -1, -1, -1,
            W, E, E, W, W, W, -1, -1, -1, -1,
            W, W, E, E, W, -1, -1, -1, -1, -1,
            W, W, W, E, E, -1, -1, -1, -1, -1,
            W, W, W, W, -1, -1, -1, -1, -1, -1,
        ],
        [
            W, W, W, W, W, W, W, W, W, W,
            W, S, S, S, W, W, W, W, W, W,
            W, S, S, S, E, E, E, E, E, W,
            W, S, S, S, W, W, W, W, E, E,
            W, W, E, W, W, W, W, W, W, E,
            W, W, E, W, W, W, W, -1, -1, -1,
            W, W, E, W, W, W, -1, -1, -1, -1,
            W, W, E, W, W, -1, -1, -1, -1, -1,
            W, W, E, E, W, -1, -1, -1, -1, -1,
            W, W, W, E, E, -1, -1, -1, -1, -1,
        ],
        [
            W, W, W, W, W, W, W, W, W, W,
            W, S, S, S, W, W, W, W, W, W,
            W, S, S, S, E, E, E, E, W, W,
            W, S, S, S, W, W, W, E, W, W,
            W, W, E, W, W, W, W, E, W, -1,
            W, W, E, W, W, W, W, E, E, -1,
            W, W, E, W, W, W, -1, -1, -1, -1,
            W, W, E, E, E, E, -1, -1, -1, -1,
            W, W, W, W, W, E, -1, -1, -1, -1,
            W, W, W, W, -1, -1, -1, -1, -1, -1,
        ],
        [
            W, W, W, W, W, W, W, W, W, W,
            W, S, S, S, W, W, W, W, W, W,
            W, S, S, S, E, E, E, W, W, W,
            W, S, S, S, W, W, E, W, W, W,
            W, W, E, W, W, W, E, W, W, -1,
            W, W, E, W, W, W, E, E, -1, -1,
            W, W, E, E, E, E, E, -1, -1, -1,
            W, W, W, W, W, E, -1, -1, -1, -1,
            W, W, W, W, W, -1, -1, -1, -1, -1,
            W, W, W, W, -1, -1, -1, -1, -1, -1,
        ],
    ]
};
