//! Room and Wandmaker preflight without layouts, painting, or item generation.

use crate::builder::RegularLevelBuilder;
use crate::floor_filters::{CompiledFloorRequirement, RoomSet, RoomType};
use crate::level_prelude::{LimitedDrops, roll_feeling};
use crate::quests::{WandmakerQuest, WandmakerQuestType};
use crate::regular_level::init_base_rooms;
use crate::rng::{RandomStack, seed_for_depth};
use crate::run::{RunState, SecretRoomState, SpecialRoomState};

// Layout adds connections, and regional quest hooks run after base rooms.
// Treat all such types as possible, including in mixed ALL/ANY constraints.
const UNKNOWN_ROOMS: u128 = (1 << RoomType::ConnectionTunnel as u8)
    | (1 << RoomType::ConnectionBridge as u8)
    | (1 << RoomType::ConnectionPerimeter as u8)
    | (1 << RoomType::ConnectionWalkway as u8)
    | (1 << RoomType::ConnectionRingTunnel as u8)
    | (1 << RoomType::ConnectionRingBridge as u8)
    | (1 << RoomType::ConnectionMaze as u8)
    | (1 << RoomType::QuestMassGrave as u8)
    | (1 << RoomType::QuestRitualSite as u8)
    | (1 << RoomType::QuestRotGarden as u8)
    | (1 << RoomType::QuestBlacksmith as u8)
    | (1 << RoomType::QuestAmbitiousImp as u8);

pub(super) fn last_depth(requirements: &[Option<CompiledFloorRequirement>; 25]) -> u8 {
    requirements
        .iter()
        .enumerate()
        .rev()
        .find_map(|(depth, requirement)| {
            // Ignore malformed raw boss/zero-depth filters, as feeling preflight does.
            let requirement = requirement.as_ref()?;
            // Unknown types stay possible; fixed rooms are always present.
            // Neither should pay for a preview that cannot reject this seed.
            let non_rejecting = UNKNOWN_ROOMS
                | (1 << RoomType::Entrance as u8)
                | (1 << RoomType::Exit as u8)
                | if depth >= 21 {
                    1 << RoomType::SpecialDemonSpawner as u8
                } else {
                    0
                };
            (depth % 5 != 0
                && (requirement.rooms.0 & !non_rejecting != 0
                    || (requirement.any_rooms.0 != 0
                        && requirement.any_rooms.0 & non_rejecting == 0)))
                .then(|| u8::try_from(depth).expect("floor requirement index fits u8"))
        })
        .unwrap_or_default()
}

pub(super) fn possible(
    requirements: &[Option<CompiledFloorRequirement>; 25],
    last_room_depth: u8,
    wandmaker: Option<(WandmakerQuestType, u8)>,
    run: &RunState,
) -> bool {
    let last_depth = last_room_depth.max(wandmaker.map_or(0, |(_, depth)| depth));
    if last_depth == 0 {
        return wandmaker.is_none();
    }
    let mut preview = RoomPreview::new(run);
    let mut pending_wandmaker = wandmaker;
    for depth in (1..=last_depth).filter(|depth| depth % 5 != 0) {
        let (rooms, scheduled) = preview.floor(depth, pending_wandmaker.is_some());
        if requirements[usize::from(depth)]
            .is_some_and(|requirement| !requirement.matches_rooms(rooms))
        {
            return false;
        }
        if let Some((wanted, deadline)) = pending_wandmaker {
            match scheduled {
                Some(actual) if actual != wanted => return false,
                Some(_) => pending_wandmaker = None,
                None if depth >= deadline => return false,
                None => {}
            }
        }
        if pending_wandmaker.is_none() && depth >= last_room_depth {
            return true;
        }
    }
    true
}

struct RoomPreview {
    dungeon_seed: i64,
    special_rooms: SpecialRoomState,
    secret_rooms: SecretRoomState,
    limited: LimitedDrops,
    random: RandomStack,
}

impl RoomPreview {
    fn new(run: &RunState) -> Self {
        Self {
            dungeon_seed: run.dungeon_seed,
            special_rooms: run.special_rooms.clone(),
            secret_rooms: run.secret_rooms.clone(),
            limited: LimitedDrops::default(),
            random: RandomStack::with_base_seed(0),
        }
    }

    fn floor(
        &mut self,
        depth: u8,
        predict_wandmaker: bool,
    ) -> (RoomSet, Option<WandmakerQuestType>) {
        self.random
            .push(seed_for_depth(self.dungeon_seed, u32::from(depth), 0));
        // Food uses its private category stream. These are the only prelude
        // draws on the floor stream, even with challenges or a Large floor.
        self.limited
            .roll_for_floor(i32::from(depth), &mut self.random);
        let feeling = roll_feeling(i32::from(depth), &mut self.random);
        // In 4.0.1 both override floats are always drawn. Moss/Trap only change
        // None to Grass/Water/Traps/Chasm using a private stream; base rooms
        // distinguish only Large and Secrets. Saved recipes are safe too,
        // regardless of the query's selected trinket or brewing depth.
        let _builder = RegularLevelBuilder::for_regular_level(&mut self.random);
        let (rooms, _) = init_base_rooms(
            &mut self.special_rooms,
            &mut self.secret_rooms,
            &mut self.limited,
            u32::from(depth),
            feeling,
            &mut self.random,
        );
        // Before the first successful schedule the quest has default state.
        // That first variant survives graph retries and every successful NPC
        // spawn. The caller stops scheduling as soon as it is known, so no
        // reward generation or synthetic spawned state is needed here.
        let wandmaker = if predict_wandmaker && WandmakerQuestType::WINDOW.contains(&depth) {
            WandmakerQuest::default().schedule_room(&mut self.random, depth)
        } else {
            None
        };
        self.random.pop();
        let mut predicted = RoomSet::from_rooms(&rooms).0 | UNKNOWN_ROOMS;
        // Halls adds this fixed room after the common initialization prefix.
        if depth >= 21 {
            predicted |= 1 << RoomType::SpecialDemonSpawner as u8;
        }
        (RoomSet(predicted), wandmaker)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::auto_trinkets::{self, SeedRecipe};
    use crate::catalog::ItemId;
    use crate::challenges::Challenges;
    use crate::feasibility::QueryPlan;
    use crate::floor_filters::FloorRequirement;
    use crate::main_world::{CanonicalMainWorldGenerator, generate_main_world_with_trinket};
    use crate::query::{SearchQuery, scout_matches};
    use crate::search::FloorGate;
    use crate::seed::DungeonSeed;

    fn query(depth: u8, rooms: Vec<RoomType>, any_rooms: Vec<RoomType>) -> SearchQuery {
        let mut query = crate::json_query::decode(
            r#"{"requirements":[{"kind":"ring"}],"max_depth":24,"auto_apply_trinket":false}"#,
        )
        .unwrap();
        query.floor_requirements.push(FloorRequirement {
            depth,
            feeling: None,
            rooms,
            any_rooms,
        });
        query
    }

    #[test]
    fn marker_keeps_unknown_any_alternatives_and_ignores_invalid_depths() {
        use RoomType::{ConnectionTunnel, Entrance, Exit, QuestRotGarden, SpecialGarden};
        for (rooms, any_rooms, expected) in [
            (vec![], vec![], 0),
            (vec![ConnectionTunnel], vec![], 0),
            (vec![], vec![SpecialGarden, QuestRotGarden], 0),
            (vec![SpecialGarden, ConnectionTunnel], vec![], 17),
            (vec![], vec![SpecialGarden], 17),
            (vec![SpecialGarden], vec![QuestRotGarden], 17),
            (vec![Entrance, Exit], vec![], 0),
            (vec![SpecialGarden, Entrance], vec![], 17),
            (vec![], vec![SpecialGarden, Entrance], 0),
            (vec![Entrance], vec![SpecialGarden], 17),
            (vec![SpecialGarden], vec![Entrance], 17),
        ] {
            let mut query = query(17, rooms, any_rooms);
            if query.floor_requirements[0].rooms.is_empty()
                && query.floor_requirements[0].any_rooms.is_empty()
            {
                query.floor_requirements[0].feeling = Some(crate::level_prelude::Feeling::Dark);
            }
            query.validate().unwrap();
            let plan = QueryPlan::analyze(&query);
            assert_eq!(plan.last_room_depth, expected);
        }
        for depth in (1..=24).filter(|depth| depth % 5 != 0) {
            assert_eq!(
                QueryPlan::analyze(&query(depth, vec![Entrance, Exit], vec![])).last_room_depth,
                0
            );
        }
        for (depth, expected) in [(19, 19), (21, 0), (24, 0)] {
            for (all, any) in [
                (vec![RoomType::SpecialDemonSpawner], vec![]),
                (vec![], vec![SpecialGarden, RoomType::SpecialDemonSpawner]),
            ] {
                assert_eq!(
                    QueryPlan::analyze(&query(depth, all, any)).last_room_depth,
                    expected
                );
            }
        }
        let mut guaranteed_later = query(17, vec![SpecialGarden], vec![]);
        guaranteed_later.requirements[0].max_depth = Some(3);
        guaranteed_later.floor_requirements.push(FloorRequirement {
            depth: 22,
            feeling: None,
            rooms: vec![Entrance],
            any_rooms: vec![],
        });
        for _ in 0..2 {
            let plan = QueryPlan::analyze(&guaranteed_later);
            assert_eq!(plan.last_room_depth, 17);
            assert_eq!(plan.generation_depth, 22);
            assert!(plan.floor_requirements[22].is_some());
            guaranteed_later.floor_requirements.reverse();
        }
        let mut query = query(17, vec![SpecialGarden], vec![]);
        for depth in [0, 5, 10, 15, 20, 25, u8::MAX] {
            query.floor_requirements[0].depth = depth;
            assert!(query.validate().is_err());
            assert_eq!(QueryPlan::analyze(&query).last_room_depth, 0);
        }
        query.floor_requirements[0].depth = 17;
        query.floor_requirements.push(FloorRequirement {
            depth: 22,
            feeling: None,
            rooms: vec![RoomType::SecretGarden],
            any_rooms: vec![],
        });
        let plan = QueryPlan::analyze(&query);
        query.floor_requirements.reverse();
        let reversed = QueryPlan::analyze(&query);
        assert_eq!(plan.last_room_depth, 22);
        assert_eq!(reversed.last_room_depth, 22);
        for seed in [0, 1, 91] {
            let run = RunState::new(seed);
            assert_eq!(
                plan.continue_after_run_init(&run),
                reversed.continue_after_run_init(&run)
            );
        }
    }

    #[test]
    fn room_preview_matches_complete_worlds_across_trinkets_and_challenges() {
        let mut overrides = [false; 2];
        // 96 complete worlds, reused for every depth and room class. Include
        // a known dark-garden17 match, boundaries, and dispersed seeds.
        for value in [
            0,
            1,
            91,
            502,
            8_687_205_886,
            203_276_180_871,
            812_345_678_901,
            crate::seed::TOTAL_SEEDS - 1,
        ] {
            let seed = DungeonSeed::new(value).unwrap();
            for challenges in [Challenges::NONE, Challenges::LEVEL_GENERATION] {
                let run = RunState::with_challenges(i64::try_from(value).unwrap(), challenges);
                let before = run.clone();
                let mut preview = RoomPreview::new(&run);
                let predicted: Vec<_> = (1..=24)
                    .filter(|depth| depth % 5 != 0)
                    .map(|depth| (depth, preview.floor(depth, false).0))
                    .collect();
                let mut canonical_feelings = Vec::new();
                for selected in [
                    None,
                    Some(ItemId::MossyClump),
                    Some(ItemId::TrapMechanism),
                    Some(ItemId::MimicTooth),
                    Some(ItemId::CrackedSpyglass),
                    Some(ItemId::WondrousResin),
                ] {
                    let world =
                        generate_main_world_with_trinket(seed, 24, challenges, selected).unwrap();
                    if selected.is_none() {
                        canonical_feelings.clone_from(&world.feelings);
                    } else if let Some(index) = match selected {
                        Some(ItemId::MossyClump) => Some(0),
                        Some(ItemId::TrapMechanism) => Some(1),
                        _ => None,
                    } {
                        overrides[index] |= world.feelings != canonical_feelings;
                    }
                    assert_eq!(world.floor_rooms.len(), predicted.len());
                    for (actual, &(depth, expected)) in world.floor_rooms.iter().zip(&predicted) {
                        assert_eq!(actual.depth, depth);
                        assert_eq!(
                            actual.rooms.0 & !UNKNOWN_ROOMS,
                            expected.0 & !UNKNOWN_ROOMS,
                            "seed={seed}, depth={depth}, challenges={challenges:?}, trinket={selected:?}"
                        );
                    }
                    assert_eq!(run, before, "preflight must not mutate the live run");
                }
            }
        }
        assert!(overrides.into_iter().all(|seen| seen));
    }

    fn reference(plan: &QueryPlan) -> QueryPlan {
        let mut reference = plan.clone();
        reference.last_room_depth = 0;
        // Retain the final query predicate while disabling variant pruning.
        reference.wandmaker_deadline = None;
        reference
    }

    #[test]
    fn preflight_preserves_forced_recipes_worlds_and_witnesses() {
        let mut positives = 0;
        let mut negatives = 0;
        for (value, challenges) in [(0, Challenges::NONE), (91, Challenges::LEVEL_GENERATION)] {
            let seed = DungeonSeed::new(value).unwrap();
            let run = RunState::with_challenges(i64::try_from(value).unwrap(), challenges);
            let generator = CanonicalMainWorldGenerator::with_challenges(challenges);
            for selected in [None, Some(ItemId::MossyClump), Some(ItemId::TrapMechanism)] {
                let world =
                    generate_main_world_with_trinket(seed, 24, challenges, selected).unwrap();
                let rooms = world
                    .floor_rooms
                    .iter()
                    .find(|floor| floor.depth == 22)
                    .unwrap()
                    .rooms;
                let present = rooms
                    .iter()
                    .find(|room| {
                        !matches!(room, RoomType::Entrance | RoomType::Exit)
                            && (1 << *room as u8) & UNKNOWN_ROOMS == 0
                    })
                    .unwrap();
                let absent = [
                    RoomType::SpecialGarden,
                    RoomType::SecretGarden,
                    RoomType::SpecialLibrary,
                ]
                .into_iter()
                .find(|room| !rooms.contains(*room))
                .unwrap();
                for (all, any, preflight) in [
                    (vec![present], vec![], true),
                    (vec![absent], vec![], false),
                    (vec![absent, RoomType::ConnectionTunnel], vec![], false),
                    (vec![], vec![absent, RoomType::ConnectionTunnel], true),
                ] {
                    let mut wanted = query(22, all, any);
                    wanted.challenges = challenges;
                    wanted.validate().unwrap();
                    let plan = QueryPlan::analyze(&wanted);
                    assert_eq!(plan.continue_after_run_init(&run), preflight);
                    assert!(reference(&plan).continue_after_run_init(&run));
                    let recipes = [SeedRecipe {
                        seed,
                        trinket: selected,
                    }];
                    let actual = auto_trinkets::filter_batch(&generator, &wanted, &plan, &recipes);
                    let expected = auto_trinkets::filter_batch(
                        &generator,
                        &wanted,
                        &reference(&plan),
                        &recipes,
                    );
                    assert_eq!(actual[0].is_some(), wanted.matches(&world));
                    assert_eq!(actual[0].is_some(), expected[0].is_some());
                    if let Some(actual) = &actual[0] {
                        let expected = expected[0].as_ref().unwrap();
                        assert_eq!(actual.recipe, recipes[0]);
                        assert_eq!(actual.recipe, expected.recipe);
                        assert_eq!(actual.world, expected.world);
                        let marks = scout_matches(&actual.world, &wanted);
                        assert_eq!(marks, scout_matches(&expected.world, &wanted));
                        assert!(!marks.matched_indices().is_empty());
                        positives += 1;
                    } else {
                        negatives += 1;
                    }
                }
            }
        }
        assert!(positives >= 6);
        assert!(negatives >= 12);
    }

    #[test]
    fn preflight_preserves_automatic_search_and_refinement_cleanup() {
        // This seed offers Mimic Tooth and has Sundial at transmutation #1.
        let seed = DungeonSeed::from_code("AAA-AAA-AAN").unwrap();
        let mut wanted = crate::json_query::decode(
            r#"{"max_depth":24,"auto_apply_trinket":true,"requirements":[{"item":"dimensional_sundial","trinket_transmutations":1},{"kind":"melee_weapon","source":"golden_mimic"}]}"#,
        )
        .unwrap();
        let world =
            generate_main_world_with_trinket(seed, 24, Challenges::NONE, Some(ItemId::MimicTooth))
                .unwrap();
        let present = world
            .floor_rooms
            .iter()
            .find(|floor| floor.depth == 17)
            .unwrap()
            .rooms
            .iter()
            .find(|room| {
                !matches!(room, RoomType::Entrance | RoomType::Exit)
                    && (1 << *room as u8) & UNKNOWN_ROOMS == 0
            })
            .unwrap();
        wanted.floor_requirements.push(FloorRequirement {
            depth: 17,
            feeling: None,
            rooms: vec![present],
            any_rooms: vec![],
        });
        wanted.wandmaker_quest = world.quests.wandmaker.map(|quest| quest.variant);
        let plan = QueryPlan::analyze(&wanted);
        assert_eq!(plan.selected_trinket(seed), Some(ItemId::MimicTooth));
        let actual =
            auto_trinkets::search_batch(&CanonicalMainWorldGenerator, &wanted, &plan, &[seed]);
        let expected = auto_trinkets::search_batch(
            &CanonicalMainWorldGenerator,
            &wanted,
            &reference(&plan),
            &[seed],
        );
        let actual = actual[0].as_ref().unwrap();
        let expected = expected[0].as_ref().unwrap();
        assert_eq!(actual.recipe.trinket, Some(ItemId::MimicTooth));
        assert_eq!(actual.recipe, expected.recipe);
        assert_eq!(actual.world, expected.world);
        assert_eq!(
            scout_matches(&actual.world, &wanted),
            scout_matches(&expected.world, &wanted)
        );

        let mut refined = wanted.clone();
        refined.requirements.truncate(1);
        let refined_plan = QueryPlan::analyze(&refined);
        let recipes = [actual.recipe];
        let actual = auto_trinkets::refine_batch(
            &CanonicalMainWorldGenerator,
            &refined,
            &refined_plan,
            &wanted,
            &recipes,
        );
        let expected = auto_trinkets::refine_batch(
            &CanonicalMainWorldGenerator,
            &refined,
            &reference(&refined_plan),
            &wanted,
            &recipes,
        );
        let actual = actual[0].as_ref().unwrap();
        let expected = expected[0].as_ref().unwrap();
        assert_eq!(
            actual.recipe.trinket, None,
            "unnecessary automatic choice is removed"
        );
        assert_eq!(actual.recipe, expected.recipe);
        assert_eq!(actual.world, expected.world);
        assert_eq!(
            scout_matches(&actual.world, &refined),
            scout_matches(&expected.world, &refined)
        );
    }

    #[test]
    fn wandmaker_preview_matches_variants_and_early_deadlines() {
        let mut positives = 0;
        let mut negatives = 0;
        let mut variants_seen = 0;
        // Thirty complete worlds, reused across every variant and cap.
        // The first three seeds pin scheduling on floors nine, eight, seven.
        for value in [
            0,
            1,
            2,
            DungeonSeed::from_code("BAD-RAT-KNG").unwrap().value(),
            crate::seed::TOTAL_SEEDS - 1,
        ] {
            let seed = DungeonSeed::new(value).unwrap();
            for challenges in [Challenges::NONE, Challenges::LEVEL_GENERATION] {
                let run = RunState::with_challenges(i64::try_from(value).unwrap(), challenges);
                let before = run.clone();
                for selected in [None, Some(ItemId::MossyClump), Some(ItemId::TrapMechanism)] {
                    let world =
                        generate_main_world_with_trinket(seed, 24, challenges, selected).unwrap();
                    let scheduled = world.quests.wandmaker.unwrap();
                    let present = world
                        .floor_rooms
                        .iter()
                        .find(|floor| floor.depth == 17)
                        .unwrap()
                        .rooms
                        .iter()
                        .find(|room| {
                            !matches!(room, RoomType::Entrance | RoomType::Exit)
                                && (1 << *room as u8) & UNKNOWN_ROOMS == 0
                        })
                        .unwrap();
                    variants_seen |= 1 << scheduled.variant.wire_id();
                    if value < 3 && challenges == Challenges::NONE {
                        assert_eq!(scheduled.depth, 9 - u8::try_from(value).unwrap());
                    }
                    for cap in [6, 7, 8, 9, 10, 24] {
                        for wanted in WandmakerQuestType::ALL {
                            let mut query = crate::json_query::decode(
                                r#"{"requirements":[{"kind":"wand"}],"auto_apply_trinket":false}"#,
                            )
                            .unwrap();
                            query.max_depth = cap;
                            query.challenges = challenges;
                            query.wandmaker_quest = Some(wanted);
                            query.validate().unwrap();
                            let expected = scheduled.depth <= cap && scheduled.variant == wanted;
                            let plan = QueryPlan::analyze(&query);
                            assert_eq!(
                                plan.continue_after_run_init(&run),
                                expected,
                                "seed={seed}, cap={cap}, wanted={wanted:?}, trinket={selected:?}"
                            );
                            if expected {
                                positives += 1;
                            } else {
                                negatives += 1;
                            }
                            if cap == 24 {
                                query.floor_requirements.push(FloorRequirement {
                                    depth: 17,
                                    feeling: None,
                                    rooms: vec![present],
                                    any_rooms: vec![],
                                });
                                let plan = QueryPlan::analyze(&query);
                                assert_eq!(plan.last_room_depth, 17);
                                assert_eq!(
                                    plan.continue_after_run_init(&run),
                                    expected,
                                    "later room checks must not postpone a quest rejection"
                                );
                                query.floor_requirements[0].rooms = vec![RoomType::Entrance];
                                let plan = QueryPlan::analyze(&query);
                                assert_eq!(plan.last_room_depth, 0);
                                assert_eq!(
                                    plan.continue_after_run_init(&run),
                                    expected,
                                    "skipping guaranteed rooms must retain the quest preview"
                                );
                            }
                        }
                    }
                    assert_eq!(run, before);
                }
            }
        }
        assert!(positives >= 54);
        assert!(negatives >= 200);
        assert_eq!(variants_seen, 0b1110);
    }

    #[test]
    fn wandmaker_preflight_preserves_forced_recipes_and_room_combinations() {
        let mut positives = 0;
        let mut negatives = 0;
        for value in 0..3 {
            let seed = DungeonSeed::new(value).unwrap();
            for selected in [None, Some(ItemId::MossyClump), Some(ItemId::TrapMechanism)] {
                let world =
                    generate_main_world_with_trinket(seed, 24, Challenges::NONE, selected).unwrap();
                let present = world
                    .floor_rooms
                    .iter()
                    .find(|floor| floor.depth == 17)
                    .unwrap()
                    .rooms
                    .iter()
                    .find(|room| {
                        !matches!(room, RoomType::Entrance | RoomType::Exit)
                            && (1 << *room as u8) & UNKNOWN_ROOMS == 0
                    })
                    .unwrap();
                for wanted in WandmakerQuestType::ALL {
                    let mut query = query(17, vec![present], vec![]);
                    query.requirements = vec![crate::query::Requirement::any(
                        crate::catalog::ItemKind::Wand,
                    )];
                    query.wandmaker_quest = Some(wanted);
                    query.validate().unwrap();
                    let plan = QueryPlan::analyze(&query);
                    let recipes = [SeedRecipe {
                        seed,
                        trinket: selected,
                    }];
                    let actual = auto_trinkets::filter_batch(
                        &CanonicalMainWorldGenerator,
                        &query,
                        &plan,
                        &recipes,
                    );
                    let expected = auto_trinkets::filter_batch(
                        &CanonicalMainWorldGenerator,
                        &query,
                        &reference(&plan),
                        &recipes,
                    );
                    assert_eq!(
                        actual[0].is_some(),
                        world.quests.wandmaker.unwrap().variant == wanted
                    );
                    assert_eq!(actual[0].is_some(), expected[0].is_some());
                    assert_eq!(actual[0].is_some(), query.matches(&world));
                    if let Some(actual) = &actual[0] {
                        let expected = expected[0].as_ref().unwrap();
                        assert_eq!(actual.recipe, recipes[0]);
                        assert_eq!(actual.recipe, expected.recipe);
                        assert_eq!(actual.world, expected.world);
                        assert_eq!(
                            scout_matches(&actual.world, &query),
                            scout_matches(&expected.world, &query)
                        );
                        positives += 1;
                        let rooms = world
                            .floor_rooms
                            .iter()
                            .find(|floor| floor.depth == 17)
                            .unwrap()
                            .rooms;
                        let absent = RoomType::ALL
                            .iter()
                            .copied()
                            .find(|room| {
                                (1 << *room as u8) & UNKNOWN_ROOMS == 0 && !rooms.contains(*room)
                            })
                            .unwrap();
                        query.floor_requirements[0].rooms = vec![absent];
                        assert!(!query.matches(&world));
                        assert!(!QueryPlan::analyze(&query).continue_after_run_init(
                            &RunState::new(i64::try_from(value).unwrap())
                        ));
                    } else {
                        negatives += 1;
                    }
                }
            }
        }
        assert_eq!(positives, 9);
        assert_eq!(negatives, 18);
    }
}
