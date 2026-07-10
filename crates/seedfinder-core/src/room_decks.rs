//! Persistent special- and secret-room queue behavior for regular floors.
//!
//! These queues are initialized under the run seed, then mutated under each
//! floor's generator. Their state therefore links otherwise independent depth
//! roots and must be replayed in ascending floor order.

use crate::level_prelude::LimitedDrops;
use crate::rng::RandomStack;
use crate::run::{
    ConsumableSpecialRoom, EquipmentSpecialRoom, SecretRoomKind, SpecialRoomKind, SpecialRoomState,
};

/// A concrete result of `SpecialRoom.createRoom()`.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum SelectedSpecialRoom {
    Scheduled(SpecialRoomKind),
    Laboratory,
    Pit,
}

/// Mutable copy of `SpecialRoom.floorSpecials` for one regular floor.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FloorSpecialDeck {
    rooms: Vec<SelectedSpecialRoom>,
}

impl FloorSpecialDeck {
    #[must_use]
    pub fn rooms(&self) -> &[SelectedSpecialRoom] {
        &self.rooms
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.rooms.is_empty()
    }
}

/// Mirrors `SpecialRoom.initForFloor`, including the laboratory-needed draw
/// and immediate limited-drop counter mutation.
pub fn begin_special_floor(
    state: &SpecialRoomState,
    limited_drops: &mut LimitedDrops,
    depth: i32,
    random: &mut RandomStack,
) -> FloorSpecialDeck {
    let mut rooms: Vec<_> = state
        .run_specials
        .iter()
        .copied()
        .map(SelectedSpecialRoom::Scheduled)
        .collect();
    if limited_drops.laboratory_needed(depth, random) {
        limited_drops.record_laboratory();
        rooms.insert(0, SelectedSpecialRoom::Laboratory);
    }
    FloorSpecialDeck { rooms }
}

/// Mirrors one call to `SpecialRoom.createRoom` and mutates both the floor and
/// persistent queues.
///
/// # Panics
///
/// Panics only if an invalid caller asks for more special rooms than the
/// version-pinned floor deck can supply.
pub fn select_special_room(
    state: &mut SpecialRoomState,
    floor: &mut FloorSpecialDeck,
    depth: i32,
    random: &mut RandomStack,
) -> SelectedSpecialRoom {
    if state.pit_needed_depth == depth {
        state.pit_needed_depth = -1;
        use_special_type(state, floor, SelectedSpecialRoom::Pit);
        return SelectedSpecialRoom::Pit;
    }

    if floor.rooms.contains(&SelectedSpecialRoom::Laboratory) {
        use_special_type(state, floor, SelectedSpecialRoom::Laboratory);
        return SelectedSpecialRoom::Laboratory;
    }

    if is_boss_depth(depth + 1) {
        floor.rooms.retain(|room| {
            *room
                != SelectedSpecialRoom::Scheduled(SpecialRoomKind::Equipment(
                    EquipmentSpecialRoom::WeakFloor,
                ))
        });
    }

    let mut index = random
        .chances(&[6.0, 3.0, 1.0])
        .expect("the special-room selection weights are positive");
    while index >= floor.rooms.len() {
        index -= 1;
    }
    let selected = floor.rooms[index];
    if selected
        == SelectedSpecialRoom::Scheduled(SpecialRoomKind::Equipment(
            EquipmentSpecialRoom::WeakFloor,
        ))
    {
        state.pit_needed_depth = depth + 1;
    }
    use_special_type(state, floor, selected);
    selected
}

/// Mirrors `SpecialRoom.resetPitRoom`.
pub fn reset_pit_room(state: &mut SpecialRoomState, depth: i32) {
    if state.pit_needed_depth == depth {
        state.pit_needed_depth = -1;
    }
}

fn use_special_type(
    state: &mut SpecialRoomState,
    floor: &mut FloorSpecialDeck,
    selected: SelectedSpecialRoom,
) {
    floor.rooms.retain(|room| *room != selected);
    if uses_crystal_key(selected) {
        floor.rooms.retain(|room| !uses_crystal_key(*room));
    }
    if spawns_potion(selected) {
        floor.rooms.retain(|room| !spawns_potion(*room));
    }

    let SelectedSpecialRoom::Scheduled(kind) = selected else {
        return;
    };
    if let Some(index) = state.run_specials.iter().position(|room| *room == kind) {
        state.run_specials[index..].rotate_left(1);
    }
}

fn uses_crystal_key(room: SelectedSpecialRoom) -> bool {
    matches!(
        room,
        SelectedSpecialRoom::Pit
            | SelectedSpecialRoom::Scheduled(
                SpecialRoomKind::Equipment(
                    EquipmentSpecialRoom::CrystalVault | EquipmentSpecialRoom::CrystalChoice
                ) | SpecialRoomKind::Consumable(ConsumableSpecialRoom::CrystalPath)
            )
    )
}

fn spawns_potion(room: SelectedSpecialRoom) -> bool {
    matches!(
        room,
        SelectedSpecialRoom::Scheduled(
            SpecialRoomKind::Equipment(EquipmentSpecialRoom::Pool | EquipmentSpecialRoom::Sentry)
                | SpecialRoomKind::Consumable(
                    ConsumableSpecialRoom::Storage
                        | ConsumableSpecialRoom::ToxicGas
                        | ConsumableSpecialRoom::MagicalFire
                        | ConsumableSpecialRoom::Traps
                )
        )
    )
}

/// Returns and removes this floor's secret-room budget exactly as
/// `SecretRoom.secretsForFloor` does.
///
/// # Panics
///
/// Panics when called outside the canonical main-dungeon depth range 1..=24.
#[allow(clippy::cast_precision_loss)] // Java performs these divisions in float precision.
pub fn secrets_for_floor(
    region_secrets: &mut [i32; 5],
    depth: i32,
    random: &mut RandomStack,
) -> i32 {
    if depth == 1 {
        return 0;
    }

    let region = usize::try_from(depth / 5).expect("regular depths are non-negative");
    let floor = depth % 5;
    let floors_left = 5 - floor;
    let secrets = if floors_left == 0 {
        region_secrets[region] as f32
    } else {
        #[allow(clippy::cast_precision_loss)]
        let divided = region_secrets[region] as f32 / floors_left as f32;
        if random.float() < divided % 1.0 {
            divided.ceil()
        } else {
            divided.floor()
        }
    };
    #[allow(clippy::cast_possible_truncation)] // Mirrors Java's final float-to-int cast.
    let selected = secrets as i32;
    region_secrets[region] -= selected;
    selected
}

/// Mirrors `SecretRoom.createRoom`, including queue rotation.
pub fn select_secret_room(
    run_secrets: &mut [SecretRoomKind; 12],
    random: &mut RandomStack,
) -> SecretRoomKind {
    let index = random.chances(&[6.0, 3.0, 1.0]).unwrap_or_default();
    let selected = run_secrets[index];
    run_secrets[index..].rotate_left(1);
    selected
}

#[must_use]
pub const fn is_boss_depth(depth: i32) -> bool {
    matches!(depth, 5 | 10 | 15 | 20 | 25)
}

#[cfg(test)]
mod tests {
    use crate::level_prelude::LimitedDrops;
    use crate::rng::RandomStack;
    use crate::run::{
        ConsumableSpecialRoom as C, EquipmentSpecialRoom as E, SecretRoomKind,
        SpecialRoomKind as S, initialize_run,
    };

    use super::{
        SelectedSpecialRoom, begin_special_floor, reset_pit_room, secrets_for_floor,
        select_secret_room, select_special_room,
    };

    #[test]
    fn laboratory_is_forced_before_the_scheduled_queue_and_counted() {
        let mut state = initialize_run(0);
        let mut limited = LimitedDrops::default();
        let mut random = RandomStack::with_base_seed(123);
        let mut floor = begin_special_floor(&state.special_rooms, &mut limited, 4, &mut random);
        assert_eq!(floor.rooms()[0], SelectedSpecialRoom::Laboratory);
        assert_eq!(limited.laboratory_rooms, 1);

        assert_eq!(
            select_special_room(&mut state.special_rooms, &mut floor, 4, &mut random),
            SelectedSpecialRoom::Laboratory
        );
        assert!(!floor.rooms().contains(&SelectedSpecialRoom::Laboratory));
    }

    #[test]
    fn using_one_room_rotates_the_run_queue_and_applies_floor_exclusions() {
        let mut state = initialize_run(0);
        state.special_rooms.run_specials = [
            S::Equipment(E::CrystalVault),
            S::Consumable(C::CrystalPath),
            S::Equipment(E::CrystalChoice),
            S::Equipment(E::Pool),
            S::Equipment(E::Sentry),
            S::Consumable(C::Storage),
            S::Consumable(C::ToxicGas),
            S::Consumable(C::MagicalFire),
            S::Consumable(C::Traps),
            S::Equipment(E::WeakFloor),
            S::Equipment(E::Crypt),
            S::Equipment(E::Armory),
            S::Equipment(E::Statue),
            S::Equipment(E::Sacrifice),
            S::Consumable(C::Runestone),
            S::Consumable(C::Garden),
            S::Consumable(C::Library),
            S::Consumable(C::Treasury),
            S::Consumable(C::MagicWell),
        ];
        let mut limited = LimitedDrops {
            laboratory_rooms: 99,
            ..LimitedDrops::default()
        };
        let mut random = RandomStack::with_base_seed(0);
        let mut floor = begin_special_floor(&state.special_rooms, &mut limited, 2, &mut random);
        assert_eq!(
            select_special_room(&mut state.special_rooms, &mut floor, 2, &mut random),
            SelectedSpecialRoom::Scheduled(S::Consumable(C::CrystalPath))
        );
        assert!(!floor.rooms().iter().any(|room| matches!(
            room,
            SelectedSpecialRoom::Pit
                | SelectedSpecialRoom::Scheduled(
                    S::Equipment(E::CrystalVault | E::CrystalChoice)
                        | S::Consumable(C::CrystalPath)
                )
        )));
        assert_eq!(
            state.special_rooms.run_specials[18],
            S::Consumable(C::CrystalPath)
        );
    }

    #[test]
    fn due_pit_preempts_the_floor_queue_and_can_be_reset() {
        let mut state = initialize_run(0);
        state.special_rooms.pit_needed_depth = 7;
        let mut limited = LimitedDrops {
            laboratory_rooms: 99,
            ..LimitedDrops::default()
        };
        let mut random = RandomStack::with_base_seed(0);
        let mut floor = begin_special_floor(&state.special_rooms, &mut limited, 7, &mut random);
        assert_eq!(
            select_special_room(&mut state.special_rooms, &mut floor, 7, &mut random),
            SelectedSpecialRoom::Pit
        );
        assert_eq!(state.special_rooms.pit_needed_depth, -1);

        state.special_rooms.pit_needed_depth = 8;
        reset_pit_room(&mut state.special_rooms, 8);
        assert_eq!(state.special_rooms.pit_needed_depth, -1);
    }

    #[test]
    fn secret_budget_and_queue_rotation_are_persistent() {
        let mut counts = [2, 2, 3, 2, 3];
        let mut random = RandomStack::with_base_seed(0);
        assert_eq!(secrets_for_floor(&mut counts, 1, &mut random), 0);
        let selected = secrets_for_floor(&mut counts, 2, &mut random);
        assert!((0..=1).contains(&selected));
        assert_eq!(counts[0], 2 - selected);

        let mut queue = [
            SecretRoomKind::Garden,
            SecretRoomKind::Laboratory,
            SecretRoomKind::Library,
            SecretRoomKind::Larder,
            SecretRoomKind::Well,
            SecretRoomKind::Runestone,
            SecretRoomKind::Artillery,
            SecretRoomKind::ChestChasm,
            SecretRoomKind::Honeypot,
            SecretRoomKind::Hoard,
            SecretRoomKind::Maze,
            SecretRoomKind::Summoning,
        ];
        let before = queue;
        let room = select_secret_room(&mut queue, &mut random);
        assert_eq!(queue[11], room);
        assert_ne!(queue, before);
    }
}
