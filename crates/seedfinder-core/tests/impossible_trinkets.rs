use serde_json::{Value, json};
use shpd_seedfinder_core::feasibility::QueryPlan;
use shpd_seedfinder_core::json_query;
use shpd_seedfinder_core::main_world::CanonicalMainWorldGenerator;
use shpd_seedfinder_core::query::SearchQuery;
use shpd_seedfinder_core::search::{FloorGate, SearchOptions, SearchProgress, search_parallel};
use shpd_seedfinder_core::seed::DungeonSeed;
use shpd_seedfinder_core::trinkets::trinket_order;

fn requirements(count: usize, limit: u8) -> Vec<Value> {
    trinket_order(DungeonSeed::MIN)[..count]
        .iter()
        .map(|&id| {
            json!({
                "item": shpd_seedfinder_core::catalog::item(id).stable_id,
                "trinket_transmutations": limit,
            })
        })
        .collect()
}

fn query(requirements: &[Value]) -> SearchQuery {
    json_query::decode(&json!({"requirements": requirements}).to_string()).unwrap()
}

#[test]
fn every_trinket_prefix_has_a_shared_capacity_and_an_exact_reason() {
    for limit in 0..=12 {
        let capacity = 4 + usize::from(limit);
        let mut rows = requirements(capacity + 1, limit);
        for row in &mut rows[..4] {
            row["trinket_transmutations"] = json!(0);
        }
        let impossible = query(&rows);
        let plan = QueryPlan::analyze(&impossible);
        let expected = if limit == 0 {
            "Requires 5 initial trinket offers, but each seed offers only 4.".to_owned()
        } else {
            let noun = if limit == 1 {
                "transmutation"
            } else {
                "transmutations"
            };
            format!(
                "{} trinket requirements allow at most {limit} {noun}; only {capacity} distinct trinkets are reachable.",
                capacity + 1
            )
        };
        assert_eq!(plan.unsatisfiable_reason(), Some(expected.as_str()));
        assert_eq!(QueryPlan::check_impossibility(&impossible), Some(expected));
        assert!(!plan.continue_after_run_init(&shpd_seedfinder_core::run::RunState::new(0)));
        rows.pop();
        assert!(!QueryPlan::analyze(&query(&rows)).is_unsatisfiable());
        assert_eq!(QueryPlan::check_impossibility(&query(&rows)), None);
    }
    assert!(!QueryPlan::analyze(&query(&requirements(17, 13))).is_unsatisfiable());
}

#[test]
fn screenshot_query_never_scans_and_counts_only_initial_offers() {
    let mut rows = requirements(6, 0);
    rows[5]["trinket_transmutations"] = json!(3);
    rows.push(json!({"item": "wand_lightning", "upgrade": 3}));
    let query = query(&rows);
    let plan = QueryPlan::analyze(&query);
    assert_eq!(
        plan.unsatisfiable_reason(),
        Some("Requires 5 initial trinket offers, but each seed offers only 4.")
    );
    let options = SearchOptions {
        start_seed: 0,
        end_seed_exclusive: 16,
        workers: std::num::NonZeroUsize::MIN,
        chunk_size: std::num::NonZeroUsize::MIN,
        max_results: std::num::NonZeroUsize::MIN,
    };
    let result = search_parallel(
        &CanonicalMainWorldGenerator,
        &query,
        options,
        &SearchProgress::default(),
    )
    .unwrap();
    assert_eq!(result.tested, 0);
    assert!(result.worlds.is_empty());
}

#[test]
fn alternatives_blankets_and_mixed_limits_do_not_create_false_impossibility() {
    let mut rows = requirements(4, 0);
    let tail = requirements(7, 1);
    rows.push(json!({"any_of": [tail[4], tail[5]]}));
    assert!(!QueryPlan::analyze(&query(&rows)).is_unsatisfiable());
    let mut later = tail[6].clone();
    later["trinket_transmutations"] = json!(2);
    rows.push(later);
    assert!(!QueryPlan::analyze(&query(&rows)).is_unsatisfiable());
    let mut blanket = rows[0].clone();
    blanket["blanket"] = json!(true);
    rows.push(blanket);
    assert!(!QueryPlan::analyze(&query(&rows)).is_unsatisfiable());
    rows.push(json!({"any_of": [tail[4], {"kind":"wand"}]}));
    assert!(!QueryPlan::analyze(&query(&rows)).is_unsatisfiable());
    // The loosest OR deadline is allowed, regardless of member order.
    let mut rows = requirements(4, 0);
    rows.push(json!({"any_of": [requirements(5, 0)[4], tail[4]]}));
    assert!(!QueryPlan::analyze(&query(&rows)).is_unsatisfiable());
}

#[test]
fn repeated_identities_are_impossible_but_distinct_or_assignments_are_possible() {
    for limit in [0, 1, 13] {
        let rat = json!({"item":"rat_skull", "trinket_transmutations":limit});
        let plan = QueryPlan::analyze(&query(&[rat.clone(), rat]));
        assert_eq!(
            plan.unsatisfiable_reason(),
            Some(
                "Rat Skull is required more than once, but each trinket appears only once in the deck."
            )
        );
    }
    let either = json!({"any_of":[{"item":"rat_skull"},{"item":"mimic_tooth"}]});
    assert!(!QueryPlan::analyze(&query(&[either.clone(), either.clone()])).is_unsatisfiable());
    let plan = QueryPlan::analyze(&query(&[either.clone(), either.clone(), either]));
    assert_eq!(
        plan.unsatisfiable_reason(),
        Some(
            "3 trinket requirements share only 2 distinct choices; each trinket appears once in the deck."
        )
    );
}

#[test]
fn existing_impossibility_reasons_do_not_borrow_unrelated_floor_prose() {
    for (document, expected) in [
        (
            json!({"max_depth":6,"requirements":[{"item":"wand_lightning","upgrade":3}]}),
            "Requirement 1 (Wand of lightning) needs floor 7 or later, beyond its floor limit.",
        ),
        (
            json!({"requirements":[{"kind":"ring","upgrade":4},{"kind":"wand","upgrade":4}]}),
            "2 requirements need separate rewards from Imp, but only 1 reward can be taken.",
        ),
        (
            json!({"requirements":[{"kind":"wand","upgrade":2},{"kind":"wand","upgrade":3,"blanket":true}]}),
            "Blanket requirement 2 has no compatible required item or resin donor.",
        ),
    ] {
        let query = json_query::decode(&document.to_string()).unwrap();
        assert_eq!(
            QueryPlan::analyze(&query).unsatisfiable_reason(),
            Some(expected)
        );
        assert_eq!(
            QueryPlan::check_impossibility(&query).as_deref(),
            Some(expected)
        );
    }
}
