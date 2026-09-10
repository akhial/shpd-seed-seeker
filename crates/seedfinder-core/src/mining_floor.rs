//! Blacksmith Crystal/Gnoll mining branch terrain, from `MiningLevel` and
//! its five room painters. Branch generation uses its own depth root and
//! never mutates the main run's item decks or quest state.
#![allow(clippy::missing_panics_doc, clippy::too_many_lines)]

use crate::builder::{Builder, FigureEightBuilder};
use crate::caves_rooms::{CavesRoomDispatcher, decorate_caves_without_gold};
use crate::challenges::Challenges;
use crate::geometry::{PathFinder, Point, Rect, cast_shadow, painter as draw, terrain as t};
use crate::level::{Feeling, Level, PlacedTrap, TransitionKind, TrapKind, TrapSpec};
use crate::level_flags::LevelFlags;
use crate::painter::{
    self, RegularPainter, RoomPaintDispatch, fill_room, fill_room_margin, set_shared_door_type,
};
use crate::quests::BlacksmithQuestType;
use crate::rng::{RandomStack, seed_for_depth};
use crate::room::{
    DoorType, Room, RoomKind, SecretRoomKind, StandardRoomKind as K, place_doors_in_order,
};
use crate::run::RunState;
use crate::sewer_rooms::GeneratorRoomContent;
use crate::trinkets::TrinketEffects;

#[derive(Clone, Debug, PartialEq)]
pub struct GeneratedMine {
    pub level: Level,
    pub rooms: Vec<Room>,
    pub variant: BlacksmithQuestType,
    pub gold_in_chests: u32,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MiningError {
    InvalidDepth(u8),
    Paint(painter::PaintError),
    NoDropCell,
}
impl std::fmt::Display for MiningError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidDepth(depth) => {
                write!(f, "blacksmith mine depth must be 12..=14, got {depth}")
            }
            Self::Paint(error) => error.fmt(f),
            Self::NoDropCell => f.write_str("mine has no valid item drop cell"),
        }
    }
}
impl std::error::Error for MiningError {}

/// Generate a mining branch for a scheduled Blacksmith quest. The map API
/// validates that the quest exists at this depth before calling this primitive.
///
/// # Errors
/// Rejects invalid depths and reports structural painting/placement failures.
pub(crate) fn generate_mine(
    seed: i64,
    depth: u8,
    variant: BlacksmithQuestType,
    challenges: Challenges,
    trinket: &TrinketEffects,
) -> Result<GeneratedMine, MiningError> {
    if !(12..=14).contains(&depth) {
        return Err(MiningError::InvalidDepth(depth));
    }
    let mut rng = RandomStack::with_base_seed(0);
    rng.push(seed_for_depth(seed, u32::from(depth), 1));
    rng.trinket = trinket.clone();
    let mut builder = FigureEightBuilder::default()
        .set_path_length(0.8, &[1.0])
        .set_tunnel_length(&[1.0], &[1.0]);
    let mut entrance = Room::standard(K::MineEntrance, &mut rng);
    entrance.kind = RoomKind::Entrance(K::MineEntrance);
    let mut initial = vec![entrance];
    for kind in [K::MineGiant, K::MineLarge, K::MineLarge, K::MineLarge] {
        let mut room = Room::standard(kind, &mut rng);
        room.set_size_category(0, 2, &mut rng);
        initial.push(room);
    }
    for _ in 0..rng.normal_int_range(6, 8) {
        let mut room = Room::standard(K::MineSmall, &mut rng);
        room.set_size_category(0, 2, &mut rng);
        initial.push(room);
    }
    initial.extend([
        Room::secret(SecretRoomKind::Mine),
        Room::secret(SecretRoomKind::Mine),
    ]);
    rng.shuffle_list(&mut initial);
    let mut rooms = loop {
        let mut candidate = initial.clone();
        if builder.build(&mut candidate, u32::from(depth), &mut rng) {
            break candidate;
        }
    };
    let mut level = Level::new(u32::from(depth), Feeling::None);
    level.plants_enabled = !challenges.contains(Challenges::NO_HERBALISM);
    let gold_target = rng.normal_int_range(45, 47);
    painter::normalize_rooms_with_padding(&mut level, &mut rooms, 3);
    let mut order: Vec<_> = (0..rooms.len()).collect();
    rng.shuffle_list(&mut order);
    // Mine rooms don't draw ordinary items, but the shared cave/connection
    // dispatcher expects a content provider. This state is branch-local.
    let mut run = RunState::with_challenges(seed, challenges);
    let mut queue = Vec::new();
    let mut caves = CavesRoomDispatcher::new(GeneratorRoomContent {
        depth: i32::from(depth),
        generator: &mut run.generator,
        items_to_spawn: &mut queue,
    });
    let mut chest_gold = 0;
    for &room in &order {
        place_doors_in_order(&mut rooms, &[room], &mut rng).map_err(|(room, neighbour)| {
            MiningError::Paint(painter::PaintError::NoDoorCandidate { room, neighbour })
        })?;
        match rooms[room].kind {
            RoomKind::Secret(SecretRoomKind::Mine) => paint_secret(
                &mut level,
                &mut rooms,
                room,
                variant,
                &mut chest_gold,
                &mut rng,
            ),
            RoomKind::Standard(kind) | RoomKind::Entrance(kind) => {
                let fill = match kind {
                    K::MineSmall => 0.40,
                    K::MineLarge => 0.55,
                    K::MineGiant => 0.70,
                    _ => {
                        #[allow(clippy::cast_precision_loss)]
                        let scale = (rooms[room].width() * rooms[room].height()).min(324) as f32;
                        0.30 + scale / 1024.0
                    }
                };
                caves.paint_cave_with_fill(&mut level, &mut rooms, room, fill, &mut rng);
                paint_mine_room(&mut level, &mut rooms, room, kind, variant, &mut rng);
            }
            _ => caves.paint_room(&mut level, &mut rooms, room, &mut rng),
        }
        painter::synchronize_room_doors(&mut rooms, room);
    }
    paint_doors(&mut level, &mut rooms, &order, &mut rng);
    let child = rng.long();
    rng.push(child);
    let painter = RegularPainter::default()
        .set_water(0.35, 6)
        .set_grass(0.10, 3);
    painter.paint_water(&mut level, &rooms, &order, &MineDispatch, &mut rng);
    painter.paint_grass(&mut level, &rooms, &order, &MineDispatch, &mut rng);
    decorate_caves_without_gold(&mut level, &rooms, &order, &mut rng);
    generate_gold(
        &mut level,
        &rooms,
        &mut order,
        gold_target - chest_gold,
        &mut rng,
    );
    for tile in &mut level.map.cells {
        if *tile == t::CHASM {
            *tile = t::EMPTY;
        }
    }
    rng.pop();
    level.room_order = order;
    populate(&mut level, &rooms, variant, challenges, &mut rng)?;
    Ok(GeneratedMine {
        level,
        rooms,
        variant,
        gold_in_chests: u32::try_from(chest_gold).unwrap_or_default(),
    })
}

struct MineDispatch;
impl RoomPaintDispatch for MineDispatch {
    fn paint_room(&mut self, _: &mut Level, _: &mut [Room], _: usize, _: &mut RandomStack) {
        unreachable!()
    }
    fn can_merge(
        &self,
        level: &Level,
        rooms: &[Room],
        room: usize,
        _: usize,
        point: Point,
        _: i32,
    ) -> bool {
        rooms[room].is_standard() && painter::standard_can_merge(level, rooms, room, point)
    }
}

fn random_point(room: &Room, margin: i32, rng: &mut RandomStack) -> Point {
    Point::new(
        rng.int_range(room.bounds.left + margin, room.bounds.right - margin),
        rng.int_range(room.bounds.top + margin, room.bounds.bottom - margin),
    )
}
fn ellipse(level: &mut Level, room: &Room, margin: i32, tile: i32) {
    draw::fill_ellipse(
        &mut level.map,
        room.bounds.left + margin,
        room.bounds.top + margin,
        room.width() - 2 * margin,
        room.height() - 2 * margin,
        tile,
    );
}
fn offset(cell: usize, delta: i32) -> usize {
    usize::try_from(i32::try_from(cell).expect("cell fits i32") + delta).expect("padded neighbour")
}
fn distance(level: &Level, a: usize, b: usize) -> i32 {
    let (a, b) = (level.map.cell_to_point(a), level.map.cell_to_point(b));
    (a.x - b.x).abs().max((a.y - b.y).abs())
}

fn paint_secret(
    level: &mut Level,
    rooms: &mut [Room],
    room: usize,
    variant: BlacksmithQuestType,
    chest_gold: &mut i32,
    rng: &mut RandomStack,
) {
    fill_room(level, &rooms[room], t::WALL);
    set_shared_door_type(rooms, room, rooms[room].connected[0].room, DoorType::Hidden);
    if variant == BlacksmithQuestType::Gnoll {
        fill_room_margin(level, &rooms[room], 1, t::EMPTY_SP);
        *chest_gold += rng.normal_int_range(4, 5);
        let pos = level.point_to_cell(rooms[room].center(rng));
        level.mark_heap(pos);
    } else {
        fill_room_margin(level, &rooms[room], 1, t::MINE_CRYSTAL);
        for _ in 0..rng.normal_int_range(4, 5) {
            // Upstream tests one random cell, then paints a second random cell.
            loop {
                let pos = level.point_to_cell(random_point(&rooms[room], 1, rng));
                if level.map.cells[pos] != t::WALL_DECO {
                    break;
                }
            }
            let pos = level.point_to_cell(random_point(&rooms[room], 1, rng));
            level.map.cells[pos] = t::WALL_DECO;
        }
    }
}

fn paint_mine_room(
    level: &mut Level,
    rooms: &mut [Room],
    room: usize,
    kind: K,
    variant: BlacksmithQuestType,
    rng: &mut RandomStack,
) {
    let mut protected = Vec::new();
    if kind == K::MineEntrance {
        let paths = PathFinder::new(level.width(), level.height());
        let pos = loop {
            let pos = level.point_to_cell(random_point(&rooms[room], 3, rng));
            let valid = paths
                .neighbours9
                .iter()
                .any(|&d| level.map.cells[offset(pos, d)] != t::WALL)
                || (rooms[room].width() == 7 && rooms[room].height() == 7);
            if !level.mob_cells[pos] && valid {
                break pos;
            }
        };
        level.map.cells[pos] = t::ENTRANCE_SP;
        for d in paths.neighbours8 {
            level.map.cells[offset(pos, d)] = t::EMPTY_SP;
        }
        // The map endpoint identifies this as a return to the parent branch.
        level.add_transition(pos, TransitionKind::RegularEntrance);
        protected.push(pos);
    }
    if variant == BlacksmithQuestType::Crystal {
        if kind == K::MineLarge {
            ellipse(level, &rooms[room], 3, t::MINE_CRYSTAL);
            ellipse(level, &rooms[room], 4, t::EMPTY);
            let pos = level.point_to_cell(random_point(&rooms[room], 5, rng));
            let finder = PathFinder::new(level.width(), level.height());
            let mut internal = Vec::new();
            find_internal(level, pos, &finder.neighbours4, &mut internal);
            for &cell in &internal {
                if finder.circle8.iter().any(|&d| {
                    !internal.contains(&offset(cell, d))
                        && level.map.cells[offset(cell, d)] != t::MINE_CRYSTAL
                }) {
                    level.map.cells[cell] = t::MINE_CRYSTAL;
                }
            }
            protected.push(pos);
        } else if kind == K::MineGiant {
            ellipse(level, &rooms[room], 3, t::EMPTY);
        }
        let divisor = match kind {
            K::MineSmall => 3,
            K::MineLarge => 4,
            _ => 2,
        };
        for _ in 0..rooms[room].width() * rooms[room].height() / divisor {
            let pos = level.point_to_cell(random_point(&rooms[room], 1, rng));
            if level.map.cells[pos] != t::WALL
                && (kind != K::MineEntrance || distance(level, pos, protected[0]) > 1)
            {
                level.map.cells[pos] = t::MINE_CRYSTAL;
            }
        }
        if kind == K::MineGiant {
            protected.push(level.point_to_cell(rooms[room].center(rng)));
        }
        if matches!(kind, K::MineGiant | K::MineLarge) {
            rng.int_bound(3); // CrystalGuardian/CrystalSpire sprite colour.
            let pos = protected[0];
            level.mark_mob(pos);
            level.map.cells[pos] = t::EMPTY;
        }
        return;
    }
    if matches!(kind, K::MineLarge | K::MineGiant) {
        ellipse(level, &rooms[room], 3, t::EMPTY);
    }
    let neighbours: Vec<_> = rooms[room].connected.iter().map(|c| c.room).collect();
    for neighbour in neighbours {
        if !rooms[neighbour].is_secret()
            && rooms[room]
                .connection_to(neighbour)
                .unwrap()
                .door
                .unwrap()
                .door_type
                == DoorType::Regular
        {
            let tile = if rng.int_bound(10) == 0 {
                DoorType::Empty
            } else {
                DoorType::Wall
            };
            set_shared_door_type(rooms, room, neighbour, tile);
            rooms[room]
                .connection_to_mut(neighbour)
                .unwrap()
                .door
                .as_mut()
                .unwrap()
                .type_locked = true;
            rooms[neighbour]
                .connection_to_mut(room)
                .unwrap()
                .door
                .as_mut()
                .unwrap()
                .type_locked = true;
        }
    }
    let doors: Vec<_> = rooms[room]
        .connected
        .iter()
        .filter_map(|c| c.door)
        .filter(|d| d.door_type == DoorType::Wall)
        .map(|d| d.point)
        .collect();
    if kind == K::MineLarge {
        gnoll_camp(level, &rooms[room], &mut protected, rng);
    }
    for p in rooms[room].bounds.points() {
        let pos = level.point_to_cell(p);
        if level.map.cells[pos] != t::EMPTY
            || protected.contains(&pos)
            || (kind == K::MineEntrance && distance(level, pos, protected[0]) <= 1)
        {
            continue;
        }
        let dist = doors
            .iter()
            .map(|&d| Point::distance(p, d))
            .fold(1000.0_f32, f32::min);
        let dist = (dist - if kind == K::MineSmall { 0.0 } else { 0.5 }).clamp(
            1.0,
            match kind {
                K::MineGiant => 3.1,
                K::MineLarge => 4.0,
                _ => 5.0,
            },
        );
        let val = rng.float() * dist.powi(2);
        if val <= 0.75 || dist <= 1.0 {
            level.map.cells[pos] = t::MINE_BOULDER;
        } else if val <= 5.0 && dist <= if kind == K::MineSmall { 2.0 } else { 3.0 } {
            level.map.cells[pos] = t::EMPTY_DECO;
        }
    }
    if kind == K::MineGiant {
        let center = rooms[room].center(rng);
        let rect = Rect::new(center.x - 2, center.y - 2, center.x + 3, center.y + 3);
        draw::fill_ellipse_rect(&mut level.map, rect, t::MINE_BOULDER);
        draw::fill_rect_margin(&mut level.map, rect, 2, t::EMPTY_DECO);
        rng.normal_int_range(3, 5); // GnollGeomancer's ability cooldown initializer.
        level.mark_mob(level.point_to_cell(center));
    }
}

fn find_internal(level: &Level, cell: usize, offsets: &[i32; 4], result: &mut Vec<usize>) {
    for &d in offsets {
        let n = offset(cell, d);
        if !result.contains(&n) && level.map.cells[n] != t::MINE_CRYSTAL {
            result.push(n);
            find_internal(level, n, offsets, result);
        }
    }
}

fn gnoll_camp(level: &mut Level, room: &Room, protected: &mut Vec<usize>, rng: &mut RandomStack) {
    let sapper = level.point_to_cell(random_point(room, 5, rng));
    rng.normal_int_range(4, 6); // GnollSapper ability cooldown initializer.
    level.mark_mob(sapper);
    let offsets = PathFinder::new(level.width(), level.height()).neighbours8;
    let guard = loop {
        let pos = offset(sapper, offsets[usize::try_from(rng.int_bound(8)).unwrap()]);
        if level.map.cells[pos] == t::EMPTY {
            break pos;
        }
    };
    level.mark_mob(guard);
    protected.extend([sapper, guard]);
    for _ in 0..if rng.int_bound(2) == 0 { 2 } else { 1 } {
        loop {
            let pos = offset(sapper, offsets[usize::try_from(rng.int_bound(8)).unwrap()]);
            if level.map.cells[pos] == t::EMPTY && pos != guard {
                level.map.cells[pos] = t::BARRICADE;
                break;
            }
        }
    }
    for _ in 0..if room.width() * room.height() > 150 {
        3
    } else {
        2
    } {
        let pos = loop {
            let pos = level.point_to_cell(random_point(room, 2, rng));
            if level.map.cells[pos] == t::EMPTY && !protected.contains(&pos) {
                break pos;
            }
        };
        level.map.cells[pos] = t::TRAP;
        level.set_trap(PlacedTrap {
            cell: pos,
            spec: TrapSpec::new(TrapKind::GnollRockfall),
            visible: true,
            active: true,
        });
    }
}

fn paint_doors(level: &mut Level, rooms: &mut [Room], order: &[usize], rng: &mut RandomStack) {
    let mut merges = vec![None; rooms.len()];
    for &room in order {
        let neighbours: Vec<_> = rooms[room].connected.iter().map(|c| c.room).collect();
        for neighbour in neighbours {
            let door = rooms[room].connection_to(neighbour).unwrap().door.unwrap();
            let pos = level.point_to_cell(door.point);
            if matches!(door.door_type, DoorType::Wall | DoorType::Hidden) {
                level.map.cells[pos] = t::WALL;
            } else {
                let hidden = rng.float() < 0.90;
                if hidden {
                    painter::force_shared_door_type(rooms, room, neighbour, DoorType::Hidden);
                    painter::build_room_distance_map(rooms, room);
                }
                if hidden && rooms[neighbour].distance != i32::MAX {
                    level.map.cells[pos] = t::WALL;
                } else {
                    level.map.cells[pos] = t::EMPTY;
                    painter::force_shared_door_type(rooms, room, neighbour, DoorType::Empty);
                }
            }
            if level.map.cells[pos] == t::EMPTY
                && merges[room] != Some(neighbour)
                && merges[neighbour] != Some(room)
                && painter::merge_rooms(
                    level,
                    rooms,
                    room,
                    neighbour,
                    Some(door.point),
                    t::EMPTY,
                    &mut MineDispatch,
                    rng,
                )
            {
                merges[room] = Some(neighbour);
                merges[neighbour] = Some(room);
            }
        }
    }
}

fn generate_gold(
    level: &mut Level,
    rooms: &[Room],
    order: &mut [usize],
    target: i32,
    rng: &mut RandomStack,
) {
    let mut remaining = target
        - i32::try_from(
            level
                .map
                .cells
                .iter()
                .filter(|&&tile| tile == t::WALL_DECO)
                .count(),
        )
        .unwrap();
    let neighbours = PathFinder::new(level.width(), level.height()).neighbours4;
    loop {
        rng.shuffle_list(order);
        for &room in order.iter() {
            if rooms[room].is_secret() {
                continue;
            }
            let candidates: Vec<_> = rooms[room]
                .bounds
                .points()
                .map(|p| level.point_to_cell(p))
                .filter(|&pos| {
                    remaining > 0
                        && level.map.cells[pos] == t::WALL
                        && neighbours.iter().any(|&d| {
                            let n = offset(pos, d);
                            level.map.cells[n] != t::WALL
                                && rooms[room].inside(level.map.cell_to_point(n))
                        })
                })
                .collect();
            if remaining > 0 && !candidates.is_empty() {
                let pos = candidates[usize::try_from(
                    rng.int_bound(i32::try_from(candidates.len()).unwrap()),
                )
                .unwrap()];
                level.map.cells[pos] = t::WALL_DECO;
                remaining -= 1;
                if remaining > 0 {
                    let n = offset(pos, neighbours[usize::try_from(rng.int_bound(4)).unwrap()]);
                    if level.map.cells[n] == t::WALL {
                        level.map.cells[n] = t::WALL_DECO;
                        remaining -= 1;
                    }
                    if rng.int_bound(2) == 0 {
                        let n = offset(pos, neighbours[usize::try_from(rng.int_bound(4)).unwrap()]);
                        if level.map.cells[n] == t::WALL {
                            level.map.cells[n] = t::WALL_DECO;
                            remaining -= 1;
                        }
                    }
                }
            }
        }
        if remaining <= 0 {
            break;
        }
    }
}

fn populate(
    level: &mut Level,
    rooms: &[Room],
    variant: BlacksmithQuestType,
    challenges: Challenges,
    rng: &mut RandomStack,
) -> Result<(), MiningError> {
    let mut flags = LevelFlags::build_for_generation(&level.map);
    let entrance = level.entrance().expect("mine entrance");
    let entrance_room = rooms.iter().position(Room::is_entrance).unwrap();
    let mut count = crate::caves_mobs::caves_mob_limit(level.depth, false, rng) - 1;
    let mut candidates: Vec<_> = level
        .room_order
        .iter()
        .copied()
        .filter(|&r| rooms[r].is_standard())
        .collect();
    rng.shuffle_list(&mut candidates);
    let point = level.map.cell_to_point(entrance);
    let mut fov = vec![false; level.len()];
    cast_shadow(
        point.x,
        point.y,
        level.width(),
        &mut fov,
        &flags.los_blocking,
        8,
    );
    let mut walkable: Vec<_> = flags.solid.iter().map(|&s| !s).collect();
    for p in rooms[entrance_room]
        .bounds
        .points()
        .filter(|&p| rooms[entrance_room].inside(p))
    {
        let pos = level.point_to_cell(p);
        if flags.passable[pos] {
            walkable[pos] = true;
        }
    }
    let mut finder = PathFinder::new(level.width(), level.height());
    finder.build_distance_map_limited(entrance, &walkable, 8);
    let mut room_index = 0;
    let mut pending = false;
    while count > 0 {
        if !pending && variant == BlacksmithQuestType::Crystal {
            rng.int_bound(3);
        }
        let room = candidates[room_index % candidates.len()];
        room_index += 1;
        pending = true;
        if place_mob(level, &flags, &rooms[room], &fov, &finder.distance, rng) {
            count -= 1;
            pending = false;
            if count > 0 && rng.int_bound(4) == 0 {
                if variant == BlacksmithQuestType::Crystal {
                    rng.int_bound(3);
                }
                pending = true;
                if place_mob(level, &flags, &rooms[room], &fov, &finder.distance, rng) {
                    count -= 1;
                    pending = false;
                }
            }
        }
    }
    for pos in 0..level.len() {
        if level.mob_cells[pos] {
            trample(level, &mut flags, pos);
        }
    }
    rng.long(); // isolated Bones generator, no bones in the scout profile.
    for _ in 0..if variant == BlacksmithQuestType::Gnoll {
        2
    } else {
        1
    } {
        let pos = drop_cell(level, &flags, rooms, rng)?;
        trample(level, &mut flags, pos);
        crate::vault_loot::random_food_using_defaults(rng);
        level.mark_heap(pos);
    }
    if challenges.contains(Challenges::DARKNESS) {
        let pos = drop_cell(level, &flags, rooms, rng)?;
        trample(level, &mut flags, pos);
        level.mark_heap(pos);
    }
    Ok(())
}
fn trample(level: &mut Level, flags: &mut LevelFlags, pos: usize) {
    if matches!(level.map.cells[pos], t::HIGH_GRASS | t::FURROWED_GRASS) {
        level.map.cells[pos] = t::GRASS;
        flags.los_blocking[pos] = false;
    }
}
fn place_mob(
    level: &mut Level,
    flags: &LevelFlags,
    room: &Room,
    fov: &[bool],
    distance: &[i32],
    rng: &mut RandomStack,
) -> bool {
    for attempt in 0..31 {
        let pos = level.point_to_cell(random_point(room, 1, rng));
        if attempt < 30
            && !level.mob_cells[pos]
            && !fov[pos]
            && distance[pos] == i32::MAX
            && flags.passable[pos]
            && !flags.solid[pos]
            && !level.traps.iter().any(|t| t.cell == pos)
            && !level.plants.iter().any(|p| p.cell == pos)
        {
            level.mark_mob(pos);
            return true;
        }
    }
    false
}
fn drop_cell(
    level: &mut Level,
    flags: &LevelFlags,
    rooms: &[Room],
    rng: &mut RandomStack,
) -> Result<usize, MiningError> {
    for _ in 0..100 {
        rng.shuffle_list(&mut level.room_order);
        let room = level
            .room_order
            .iter()
            .find(|&&r| rooms[r].kind == RoomKind::Standard(K::MineSmall))
            .copied()
            .ok_or(MiningError::NoDropCell)?;
        let pos = level.point_to_cell(random_point(&rooms[room], 1, rng));
        if flags.passable[pos]
            && !flags.solid[pos]
            && !level.heap_cells[pos]
            && !level.mob_cells[pos]
        {
            return Ok(pos);
        }
    }
    Err(MiningError::NoDropCell)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{Value, json};

    #[test]
    fn mining_maps_match_unmodified_java_crystal_gnoll_and_challenges() {
        let fixture: Value = serde_json::from_str(include_str!(
            "../../../tooling/oracle-4.0/tests/mining-maps.expected.json"
        ))
        .unwrap();
        for row in fixture["levels"].as_array().unwrap() {
            let seed = crate::seed::DungeonSeed::from_code(row["seed"].as_str().unwrap()).unwrap();
            let depth = u8::try_from(row["depth"].as_u64().unwrap()).unwrap();
            let variant = if row["variant"] == 1 {
                BlacksmithQuestType::Crystal
            } else {
                BlacksmithQuestType::Gnoll
            };
            let challenges =
                Challenges::new(u16::try_from(row["challenges"].as_u64().unwrap()).unwrap())
                    .unwrap();
            let mine = generate_mine(
                i64::try_from(seed.value()).unwrap(),
                depth,
                variant,
                challenges,
                &TrinketEffects::default(),
            )
            .unwrap();
            let hash = mine
                .level
                .map
                .cells
                .iter()
                .fold(1_i32, |h, &v| h.wrapping_mul(31).wrapping_add(v));
            assert_eq!(
                json!(hash),
                row["hash"],
                "{} depth {depth} {variant:?} {challenges:?}",
                seed.to_code()
            );
            assert_eq!(json!(mine.level.width()), row["width"]);
            assert_eq!(json!(mine.level.height()), row["height"]);
            assert_eq!(json!(mine.level.entrance()), row["entrance"]);
            let mut traps: Vec<_> = mine.level.traps.iter().map(|t| t.cell).collect();
            traps.sort_unstable();
            assert_eq!(json!(traps), row["traps"]);
            let secrets: Vec<_> = mine
                .level
                .room_order
                .iter()
                .map(|&i| &mine.rooms[i])
                .filter(|r| r.is_secret())
                .map(|r| [r.bounds.left, r.bounds.top, r.bounds.right, r.bounds.bottom])
                .collect();
            assert_eq!(json!(secrets), row["secretRooms"]);
        }
    }
}
