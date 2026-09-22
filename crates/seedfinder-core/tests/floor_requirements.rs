use shpd_seedfinder_core::{
    auto_trinkets::{self, SeedRecipe},
    catalog::ItemId,
    challenges::Challenges,
    deep_link,
    feasibility::QueryPlan,
    floor_filters::{CompiledFloorRequirement, FloorRequirement, FloorRooms, RoomSet, RoomType},
    json_query,
    level_prelude::Feeling,
    main_world::{CanonicalMainWorldGenerator, generate_main_world_with_trinket},
    model::{FloorFeeling, WorldItem},
    probability::estimate_match_probability,
    query::{SearchQuery, scout_matches},
    quests::QuestSummary,
    search::{FloorGate, WorldGenerator},
    seed::DungeonSeed,
};

fn farm(depth: u8) -> FloorRequirement {
    FloorRequirement {
        depth,
        feeling: Some(Feeling::Dark),
        rooms: vec![],
        any_rooms: vec![RoomType::SpecialGarden, RoomType::SecretGarden],
    }
}
fn query(floors: Vec<FloorRequirement>) -> SearchQuery {
    let mut q = json_query::decode_unvalidated(r#"{"requirements":[]}"#).unwrap();
    q.floor_requirements = floors;
    q
}

#[test]
fn floor_only_queries_validate_and_round_trip_all_portable_formats() {
    let q = query(vec![farm(7), farm(17), farm(22)]);
    q.validate().unwrap();
    assert_eq!(
        json_query::decode(&json_query::encode(&q).to_string()).unwrap(),
        q
    );
    assert_eq!(
        deep_link::decode(&deep_link::encode(&q).unwrap()).unwrap(),
        q
    );
    let document = shpd_seedfinder_core::results_export::encode(&q, &[], "test");
    // Results codec signatures are exercised in the web test too.
    assert!(document.contains("floor_requirements"));
    for auto in [false, true] {
        let mut q = q.clone();
        q.auto_apply_trinket = auto;
        q.arcane_resin_auto = true;
        assert_eq!(
            deep_link::decode(&deep_link::encode(&q).unwrap()).unwrap(),
            q
        );
    }
    let all = query(vec![FloorRequirement {
        depth: 7,
        feeling: Some(Feeling::None),
        rooms: RoomType::ALL.to_vec(),
        any_rooms: RoomType::ALL.to_vec(),
    }]);
    assert_eq!(
        deep_link::decode(&deep_link::encode(&all).unwrap()).unwrap(),
        all
    );
}

#[test]
fn malformed_filters_fail_before_search_and_impossible_feelings_are_planned_out() {
    for depth in [0, 5, 10, 15, 20, 25] {
        assert!(query(vec![farm(depth)]).validate().is_err());
    }
    assert!(query(vec![farm(7), farm(7)]).validate().is_err());
    let mut q = query(vec![farm(17)]);
    q.max_depth = 16;
    assert!(q.validate().is_err());
    assert!(
        json_query::decode(r#"{"requirements":[],"floor_requirements":[{"depth":7}]}"#).is_err()
    );
    for filter in [
        r#"{"depth":7,"rooms":["typo"]}"#,
        r#"{"depth":7,"feeling":"typo"}"#,
        r#"{"depth":7,"feelng":"dark"}"#,
    ] {
        assert!(
            json_query::decode(&format!(
                r#"{{"requirements":[],"floor_requirements":[{filter}]}}"#
            ))
            .is_err()
        );
    }
    let q = query(vec![farm(1)]);
    assert!(QueryPlan::analyze(&q).is_unsatisfiable());
    assert!(estimate_match_probability(&q).abs() < f64::EPSILON);
}

#[test]
fn same_floor_garden_alternatives_and_all_selected_floors_are_required() {
    let mut world = CanonicalMainWorldGenerator.generate(DungeonSeed::MIN, 22);
    world.feelings = vec![
        FloorFeeling {
            depth: 7,
            feeling: Feeling::Dark,
        },
        FloorFeeling {
            depth: 17,
            feeling: Feeling::Dark,
        },
    ];
    world.floor_rooms = vec![
        FloorRooms {
            depth: 7,
            rooms: RoomSet::from_types([RoomType::SpecialGarden]),
        },
        FloorRooms {
            depth: 17,
            rooms: RoomSet::from_types([RoomType::SecretGarden]),
        },
    ];
    let q = query(vec![farm(7), farm(17)]);
    assert!(q.matches(&world));
    let marks = scout_matches(&world, &q);
    assert_eq!(
        (marks.matched_requirements, marks.total_requirements),
        (2, 2)
    );
    world.feelings[1].feeling = Feeling::Water;
    assert!(!q.matches(&world));
    assert_eq!(scout_matches(&world, &q).matched_requirements, 1);
    world.feelings[1].feeling = Feeling::Dark;
    world.floor_rooms[1].depth = 18;
    assert!(!q.matches(&world));
    world.floor_rooms.clear();
    assert!(!q.matches(&world)); // Older scouts must never imply a garden match.
}

struct SelectedGate<'a> {
    plan: &'a QueryPlan,
    selected: Option<ItemId>,
}
impl FloorGate for SelectedGate<'_> {
    fn floor_requirement(&self, depth: u8) -> Option<&CompiledFloorRequirement> {
        self.plan.floor_requirement(depth)
    }
    fn selected_trinket(&self, _: DungeonSeed) -> Option<ItemId> {
        self.selected
    }
    fn continue_after_floor(&self, depth: u8, items: &[WorldItem], quests: &QuestSummary) -> bool {
        self.plan.continue_after_floor(depth, items, quests)
    }
    fn wants_vault_treasure(&self) -> bool {
        self.plan.wants_vault_treasure()
    }
}

#[test]
fn pruning_agrees_with_full_generation_at_terminal_and_earlier_floors() {
    let mut accepted = 0;
    let mut rejected = 0;
    for challenges in [Challenges::NONE, Challenges::LEVEL_GENERATION] {
        let generator = CanonicalMainWorldGenerator::with_challenges(challenges);
        for selected in [
            None,
            Some(ItemId::MossyClump),
            Some(ItemId::TrapMechanism),
            Some(ItemId::MimicTooth),
        ] {
            for value in [0, 1, 91, 502, 92_113, 20_013_266] {
                let seed = DungeonSeed::new(value).unwrap();
                let full =
                    generate_main_world_with_trinket(seed, 24, challenges, selected).unwrap();
                for depth in [1, 4, 7, 12, 17, 22, 24] {
                    let feeling = full
                        .feelings
                        .iter()
                        .find(|floor| floor.depth == depth)
                        .unwrap()
                        .feeling;
                    let rooms = full
                        .floor_rooms
                        .iter()
                        .find(|floor| floor.depth == depth)
                        .unwrap()
                        .rooms;
                    let exact = FloorRequirement {
                        depth,
                        feeling: Some(feeling),
                        rooms: rooms.iter().collect(),
                        any_rooms: vec![],
                    };
                    for floor in [exact, farm(depth)] {
                        let mut q = query(vec![floor]);
                        q.challenges = challenges;
                        let plan = QueryPlan::analyze(&q);
                        assert_eq!(plan.generation_depth(), depth);
                        let gate = SelectedGate {
                            plan: &plan,
                            selected,
                        };
                        let result = generator
                            .generate_batch_gated(&[seed], depth, &gate)
                            .pop()
                            .unwrap();
                        let expected = q.matches(&full);
                        assert_eq!(
                            result.as_ref().is_some_and(|world| q.matches(world)),
                            expected,
                            "seed={seed} depth={depth} challenges={challenges:?} selected={selected:?}"
                        );
                        if let Some(world) = result {
                            let prefix =
                                generate_main_world_with_trinket(seed, depth, challenges, selected)
                                    .unwrap();
                            assert_eq!(world.feelings, prefix.feelings);
                            assert_eq!(world.floor_rooms, prefix.floor_rooms);
                            accepted += usize::from(expected);
                        } else {
                            rejected += 1;
                        }
                    }
                }
            }
        }
    }
    assert!(accepted > 0 && rejected > 0);
}

#[test]
fn saved_recipes_and_item_requirements_keep_floor_rules_on_replay() {
    let generator = CanonicalMainWorldGenerator;
    for value in [0, 1, 502, 92_113] {
        let seed = DungeonSeed::new(value).unwrap();
        let selected = shpd_seedfinder_core::trinkets::trinket_order(seed)[0];
        let full =
            generate_main_world_with_trinket(seed, 24, Challenges::NONE, Some(selected)).unwrap();
        let depth = 17;
        let floor = FloorRequirement {
            depth,
            feeling: Some(
                full.feelings
                    .iter()
                    .find(|f| f.depth == depth)
                    .unwrap()
                    .feeling,
            ),
            rooms: vec![],
            any_rooms: vec![],
        };
        let mut q = query(vec![floor]);
        q.auto_apply_trinket = true;
        let recipes = [SeedRecipe {
            seed,
            trinket: Some(selected),
        }];
        let plan = QueryPlan::analyze(&q);
        assert!(auto_trinkets::filter_batch(&generator, &q, &plan, &recipes)[0].is_some());
        q.floor_requirements[0].feeling =
            Some(if q.floor_requirements[0].feeling == Some(Feeling::Dark) {
                Feeling::Water
            } else {
                Feeling::Dark
            });
        let plan = QueryPlan::analyze(&q);
        assert!(auto_trinkets::filter_batch(&generator, &q, &plan, &recipes)[0].is_none());
    }
}

#[test]
fn measured_probability_accounts_for_each_floor_and_combines_once_with_items_and_resin() {
    let a = estimate_match_probability(&query(vec![farm(7)]));
    let b = estimate_match_probability(&query(vec![farm(17)]));
    let ab = estimate_match_probability(&query(vec![farm(7), farm(17)]));
    assert!(a > 0.0 && b > 0.0 && ab < a.min(b));
    assert!((ab - a * b).abs() < 1e-12);
    for document in [
        r#"{"requirements":[{"item":"ring_wealth"}]}"#,
        r#"{"requirements":[{"item":"wand_frost"}],"arcane_resin":4}"#,
        r#"{"requirements":[{"kind":"wand"},{"kind":"wand","uncursed":true,"blanket":true}]}"#,
    ] {
        let mut q = json_query::decode(document).unwrap();
        let baseline = estimate_match_probability(&q);
        q.floor_requirements.push(farm(7));
        assert!((estimate_match_probability(&q) - baseline * a).abs() < 1e-12);
    }
}

#[test]
fn mixed_item_queries_extend_the_horizon_and_keep_deferred_vault_checks() {
    let seeds: Vec<_> = (0..24)
        .map(|n| DungeonSeed::new(3_715 + n * 973).unwrap())
        .collect();
    for document in [
        r#"{"requirements":[{"kind":"ring","max_depth":16}],"floor_requirements":[{"depth":17,"feeling":"dark","any_rooms":["garden","secret_garden"]}]}"#,
        r#"{"requirements":[{"kind":"weapon"},{"kind":"ring","source":"imp_reward"}],"floor_requirements":[{"depth":17,"feeling":"dark"}]}"#,
    ] {
        let mut q = json_query::decode(document).unwrap();
        for depth in [7, 17, 22] {
            q.floor_requirements[0].depth = depth;
            let plan = QueryPlan::analyze(&q);
            assert!(plan.generation_depth() >= depth);
            for seed in &seeds {
                let full = CanonicalMainWorldGenerator.generate(*seed, 24);
                let expected = q.matches(&full);
                let found =
                    auto_trinkets::search_batch(&CanonicalMainWorldGenerator, &q, &plan, &[*seed]);
                assert_eq!(found[0].is_some(), expected);
            }
        }
    }
}

#[test]
fn selected_and_automatic_trinket_probabilities_include_floor_rules() {
    let mut only_floor = query(vec![farm(17)]);
    let baseline = estimate_match_probability(&only_floor);
    only_floor.auto_apply_trinket = true;
    assert!((estimate_match_probability(&only_floor) - baseline).abs() < 1e-12);
    for id in ["mimic_tooth", "mossy_clump", "trap_mechanism"] {
        let mut selected = json_query::decode(&format!(
            r#"{{"requirements":[{{"item":"{id}","select_trinket":true}}]}}"#
        ))
        .unwrap();
        let offer = estimate_match_probability(&selected);
        selected.floor_requirements.push(farm(17));
        let with_floor = estimate_match_probability(&selected);
        assert!(with_floor.is_finite() && with_floor > 0.0 && with_floor < offer * 0.05);
    }
}
