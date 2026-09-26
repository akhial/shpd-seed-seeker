//! Public probability regressions; the larger held-out audit is an opt-in example.
use serde_json::{Value, json};
use shpd_seedfinder_core::{
    auto_trinkets::AutoTrinketPolicy,
    challenges::Challenges,
    json_query,
    main_world::generate_main_world_with_trinket,
    probability::estimate_match_probability,
    query::SearchQuery,
    seed::{DungeonSeed, TOTAL_SEEDS},
};

#[allow(clippy::needless_pass_by_value)] // Own temporary JSON query documents.
fn query(requirements: Value, depth: u8, auto: bool) -> SearchQuery {
    json_query::decode(
        &json!({
            "requirements": requirements, "max_depth": depth, "auto_apply_trinket": auto
        })
        .to_string(),
    )
    .unwrap()
}
fn artifact(count: u8) -> Value {
    json!({"item":"skeleton_key", "artifact_transmutations":count})
}

#[test]
fn screenshot_query_is_finite_with_and_without_auto_trinket() {
    for auto in [false, true] {
        let q = query(
            json!([artifact(2), {"item":"ring_energy", "upgrade":4}]),
            24,
            auto,
        );
        let p = estimate_match_probability(&q);
        // Independent 65,536-seed audits put both paths near one in sixty.
        assert!((0.01..0.04).contains(&p), "auto={auto}: {p}");
    }
}

#[test]
fn more_transmutations_never_reduce_probability_and_imp_prizes_compete() {
    for depth in [1, 9, 19, 24] {
        let mut previous = 0.0;
        for count in 0..=10 {
            let p = estimate_match_probability(&query(json!([artifact(count)]), depth, false));
            assert!(
                p.is_finite() && p >= previous && p <= 1.0,
                "{depth}/{count}: {p}"
            );
            previous = p;
        }
    }
    for auto in [false, true] {
        let competing = query(
            json!([
                {"item":"skeleton_key", "artifact_transmutations":2,"source":"imp_reward"},
                {"item":"ring_energy", "upgrade":4,"source":"imp_reward"}
            ]),
            24,
            auto,
        );
        assert!(estimate_match_probability(&competing).abs() < f64::EPSILON);
        let duplicate = query(json!([artifact(2), artifact(10)]), 24, auto);
        assert!(estimate_match_probability(&duplicate).abs() < f64::EPSILON);
    }
}

#[test]
fn deck_estimates_track_a_shared_held_out_sample() {
    // Keep CI at 1,024 seeds. Large calibration sweeps are opt-in --release examples.
    let mut queries: Vec<_> = [1, 9, 24]
        .into_iter()
        .flat_map(|depth| [1, 2, 10].map(|count| query(json!([artifact(count)]), depth, false)))
        .collect();
    queries.extend([
        query(json!([artifact(2), {"item":"ethereal_chains", "artifact_transmutations":4}]),24,false),
        query(json!([artifact(2), {"item":"ethereal_chains", "artifact_transmutations":4,"uncursed":true}]),24,false),
        query(json!([artifact(2), {"item":"ring_energy", "upgrade":4}]),24,true),
    ]);
    let policies: Vec<_> = queries.iter().map(AutoTrinketPolicy::prepare).collect();
    let mut hits = vec![0u32; queries.len()];
    for index in 0..1024u64 {
        let seed =
            DungeonSeed::new((4_271_828_182 + index * 3_355_211_884_971) % TOTAL_SEEDS).unwrap();
        let baseline = generate_main_world_with_trinket(seed, 24, Challenges::NONE, None).unwrap();
        let chosen = policies
            .last()
            .unwrap()
            .as_ref()
            .unwrap()
            .selected_trinket(seed);
        let automatic = chosen.map(|id| {
            generate_main_world_with_trinket(seed, 24, Challenges::NONE, Some(id)).unwrap()
        });
        for ((query, policy), hits) in queries.iter().zip(&policies).zip(&mut hits) {
            let world = if policy.is_some() {
                automatic.as_ref().unwrap_or(&baseline)
            } else {
                &baseline
            };
            *hits += u32::from(query.matches(world));
        }
    }
    for (query, hits) in queries.iter().zip(hits) {
        let predicted = estimate_match_probability(query);
        let observed = f64::from(hits) / 1024.0;
        let tolerance = 4.0 * (predicted * (1.0 - predicted) / 1024.0).sqrt() + 0.015;
        assert!(
            (predicted - observed).abs() <= tolerance,
            "{query:?}: predicted {predicted}, observed {observed}, tolerance {tolerance}"
        );
    }
}
