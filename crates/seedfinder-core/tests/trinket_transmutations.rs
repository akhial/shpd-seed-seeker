use shpd_seedfinder_core::catalog::{ItemId, ItemKind, item};
use shpd_seedfinder_core::deep_link;
use shpd_seedfinder_core::feasibility::QueryPlan;
use shpd_seedfinder_core::json_query;
use shpd_seedfinder_core::main_world::CanonicalMainWorldGenerator;
use shpd_seedfinder_core::model::{Accessibility, ItemSource};
use shpd_seedfinder_core::probability::estimate_match_probability;
use shpd_seedfinder_core::query::{SearchQuery, scout_matches};
use shpd_seedfinder_core::search::{FloorGate, WorldGenerator};
use shpd_seedfinder_core::seed::DungeonSeed;
use shpd_seedfinder_core::trinkets::{selected_for_query, trinket_order};

fn query(requirements: &serde_json::Value) -> SearchQuery {
    json_query::decode(
        &serde_json::json!({"requirements": requirements, "max_depth": 3}).to_string(),
    )
    .unwrap()
}

fn requirement(id: ItemId, step: u8) -> serde_json::Value {
    serde_json::json!({"item": item(id).stable_id, "trinket_transmutations": step})
}

#[test]
fn trinket_transmutations_match_each_exact_position_in_scalar_scout_and_gated_search() {
    // 4 seeds × 17 identities × 14 positions = 952 cases. Reuse each world.
    let generator = CanonicalMainWorldGenerator;
    for value in [0, 1, 812_345_678_901, 3_355_211_884_971] {
        let seed = DungeonSeed::new(value).unwrap();
        let world = generator.generate(seed, 3);
        let order = trinket_order(seed);
        let run = shpd_seedfinder_core::run::RunState::new(i64::try_from(value).unwrap());
        let before = run.clone();
        for (position, &id) in order.iter().enumerate() {
            for step in 0..=13 {
                let query = query(&serde_json::json!([requirement(id, step)]));
                let expected = if step == 0 {
                    position < 4
                } else {
                    position == usize::from(step) + 3
                };
                assert_eq!(
                    query.matches(&world),
                    expected,
                    "seed {value}, position {position}, step {step}"
                );
                let marks = scout_matches(&world, &query);
                assert_eq!(marks.matched_requirements, usize::from(expected));
                assert_eq!(marks.matched.len(), world.items.len());
                assert_eq!(
                    marks.matched_indices().len(),
                    usize::from(expected && step == 0)
                );
                assert_eq!(
                    marks.transmuted_trinkets.iter().filter(|&&m| m).count(),
                    usize::from(expected && step > 0)
                );
                let plan = QueryPlan::analyze(&query);
                assert_eq!(plan.continue_after_run_init(&run), expected);
                if expected {
                    let result =
                        generator.generate_batch_gated(&[seed], plan.generation_depth(), &plan);
                    assert!(result[0].as_ref().is_some_and(|world| query.matches(world)));
                }
            }
        }
        assert_eq!(run, before, "peeking must not mutate the deck");
    }
}

#[test]
fn trinket_transmutations_keep_distinct_assignment_alternatives_and_catalyst_constraints() {
    let seed = DungeonSeed::MIN;
    let mut world = CanonicalMainWorldGenerator.generate(seed, 3);
    let order = trinket_order(seed);
    let first = requirement(order[4], 1);
    let last = requirement(order[16], 13);
    assert!(query(&serde_json::json!([first, last])).matches(&world));
    assert!(!query(&serde_json::json!([first, first])).matches(&world));
    assert!(!query(&serde_json::json!([first, requirement(order[4], 0)])).matches(&world));
    let either = query(&serde_json::json!([{"any_of": [requirement(order[4], 0), first]}]));
    assert!(either.matches(&world));
    assert!(scout_matches(&world, &either).transmuted_trinkets[0]);
    let all: Vec<_> = order
        .iter()
        .enumerate()
        .map(|(index, &id)| requirement(id, u8::try_from(index.saturating_sub(3)).unwrap()))
        .collect();
    assert!(query(&serde_json::json!(all)).matches(&world));
    let blanket = query(
        &serde_json::json!([first, {"item": item(order[4]).stable_id, "trinket_transmutations": 1, "blanket": true}]),
    );
    assert!(blanket.matches(&world));
    let plan = QueryPlan::analyze(&blanket);
    assert!(plan.viable_after_floor(3, &world.items, &world.quests));
    assert_eq!(scout_matches(&world, &blanket).matched_requirements, 2);

    // A transmutation inherits the catalyst's placement and acquisition choice.
    for entry in world
        .items
        .iter_mut()
        .filter(|entry| item(entry.item).kind == ItemKind::Trinket)
    {
        entry.depth = 3;
        entry.source = ItemSource::LockedChest;
        entry.accessibility = Accessibility::Choice {
            group: 999,
            option: 0,
        };
    }
    let mut limited = query(&serde_json::json!([first]));
    limited.requirements[0].max_depth = Some(2);
    assert!(!limited.matches(&world));
    limited.requirements[0].max_depth = None;
    limited.requirements[0].source = Some(ItemSource::Heap);
    assert!(!limited.matches(&world));
    limited.requirements[0].source = Some(ItemSource::LockedChest);
    assert!(limited.matches(&world));
    let equipment = world
        .items
        .iter_mut()
        .find(|entry| item(entry.item).kind == ItemKind::Weapon)
        .unwrap();
    equipment.accessibility = Accessibility::Choice {
        group: 999,
        option: 1,
    };
    let equipment_id = item(equipment.item).stable_id;
    let equipment_only = query(&serde_json::json!([{"item": equipment_id}]));
    // Remove any duplicate weapon witnesses to make the conflicting choice explicit.
    let chosen = equipment.clone();
    world
        .items
        .retain(|entry| item(entry.item).kind == ItemKind::Trinket);
    world.items.push(chosen);
    assert!(equipment_only.matches(&world));
    assert!(!query(&serde_json::json!([first, {"item": equipment_id}])).matches(&world));
}

#[test]
fn trinket_transmutation_alternatives_never_select_an_initial_offer() {
    let query = query(&serde_json::json!([{"any_of": [
        {"item": "mimic_tooth", "select_trinket": true},
        {"item": "parchment_scrap", "trinket_transmutations": 1}
    ]}]));
    assert_eq!(
        selected_for_query(DungeonSeed::MIN, &query),
        Some(ItemId::MimicTooth)
    );
}

#[test]
fn trinket_transmutations_round_trip_codecs_and_reject_invalid_settings() {
    for step in [1, 13] {
        let query = query(
            &serde_json::json!([{ "any_of": [requirement(ItemId::RatSkull, step), requirement(ItemId::MimicTooth, 0)] }]),
        );
        assert_eq!(
            json_query::decode(&json_query::encode(&query).to_string()).unwrap(),
            query
        );
        assert_eq!(
            deep_link::decode(&deep_link::encode(&query).unwrap()).unwrap(),
            query
        );
    }
    for bad in [
        serde_json::json!({"item": "rat_skull", "trinket_transmutations": -1}),
        serde_json::json!({"item": "rat_skull", "trinket_transmutations": 14}),
        serde_json::json!({"item": "rat_skull", "trinket_transmutations": 1.5}),
        serde_json::json!({"item": "rat_skull", "trinket_transmutations": "1"}),
        serde_json::json!({"kind": "trinket", "trinket_transmutations": 1}),
        serde_json::json!({"item": "sword", "trinket_transmutations": 1}),
        serde_json::json!({"item": "rat_skull", "trinket_transmutations": 1, "select_trinket": true}),
    ] {
        assert!(
            json_query::decode(&serde_json::json!({"requirements": [bad]}).to_string()).is_err()
        );
    }
    let legacy = query(&serde_json::json!([{"item": "rat_skull"}]));
    let zero = query(&serde_json::json!([requirement(ItemId::RatSkull, 0)]));
    assert_eq!(
        deep_link::encode(&legacy).unwrap(),
        deep_link::encode(&zero).unwrap()
    );
    assert!(
        json_query::encode(&zero)["requirements"][0]
            .get("trinket_transmutations")
            .is_none()
    );
}

#[test]
fn trinket_transmutation_probabilities_use_ordered_without_replacement_draws() {
    let first = requirement(ItemId::RatSkull, 1);
    let last = requirement(ItemId::MimicTooth, 13);
    let cases = [
        (serde_json::json!([first]), 1.0 / 17.0),
        (serde_json::json!([first, last]), 1.0 / (17.0 * 16.0)),
        (
            serde_json::json!([first, requirement(ItemId::MimicTooth, 0)]),
            4.0 / (17.0 * 16.0),
        ),
        (serde_json::json!([first, first]), 0.0),
        (
            serde_json::json!([requirement(ItemId::TrinketCatalyst, 1)]),
            0.0,
        ),
        (
            serde_json::json!([first, requirement(ItemId::TrinketCatalyst, 0)]),
            0.0,
        ),
        (
            serde_json::json!([first, requirement(ItemId::RatSkull, 0)]),
            0.0,
        ),
        (
            serde_json::json!([first, requirement(ItemId::MimicTooth, 1)]),
            0.0,
        ),
    ];
    for (requirements, expected) in cases {
        assert!((estimate_match_probability(&query(&requirements)) - expected).abs() < 1e-12);
    }
    let complex = query(&serde_json::json!([{"any_of": [first, last]}]));
    assert!(estimate_match_probability(&complex).is_nan());
}
