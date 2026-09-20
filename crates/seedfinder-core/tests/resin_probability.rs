//! Resin estimates must include the surplus allocation and its donor filters.
#![allow(clippy::float_cmp)] // Exact zeros and equivalent allocations are intentional.
use shpd_seedfinder_core::{
    json_query, model::ItemSource, probability::estimate_match_probability, query::SearchQuery,
};

fn query(amount: u16, requirements: &str) -> SearchQuery {
    json_query::decode(&format!(
        r#"{{"arcane_resin":{amount},"requirements":{requirements}}}"#
    ))
    .unwrap()
}

#[test]
fn resin_estimates_are_finite_and_decrease_with_the_amount() {
    for depth in [4, 9, 24] {
        let mut previous = 1.0;
        for amount in [1, 2, 3, 4, 6, 8, 12, 20, 40, u16::MAX] {
            let mut query = query(amount, "[]");
            query.max_depth = depth;
            let before = query.clone();
            let started = std::time::Instant::now();
            let estimate = estimate_match_probability(&query);
            eprintln!(
                "depth {depth}, resin {amount}: {estimate:.6} ({:?})",
                started.elapsed()
            );
            assert!((0.0..=1.0).contains(&estimate));
            assert!(
                estimate <= previous + 1e-10,
                "{before:?}: {estimate} > {previous}"
            );
            assert_eq!(query, before);
            previous = estimate;
        }
        assert_eq!(previous, 0.0);
    }
}

#[test]
fn two_resin_is_one_additional_eligible_wand() {
    for requirements in ["[]", r#"[{"item":"wand_lightning","upgrade":2}]"#] {
        let resin = query(2, requirements);
        let mut ordinary = resin.clone();
        ordinary.arcane_resin = 0;
        ordinary
            .requirements
            .push(query(0, r#"[{"kind":"wand","uncursed":true}]"#).requirements[0]);
        assert_eq!(
            estimate_match_probability(&resin),
            estimate_match_probability(&ordinary)
        );
    }
}

#[test]
fn resin_competes_with_reserved_wands_and_single_choice_prizes() {
    let mut resin = query(3, "[]");
    resin.arcane_resin_filter.source = Some(ItemSource::WandmakerReward);
    assert!(estimate_match_probability(&resin) > 0.0);
    resin.requirements = query(0, r#"[{"kind":"wand","source":"wandmaker_reward"}]"#).requirements;
    assert_eq!(estimate_match_probability(&resin), 0.0);

    let baseline = query(0, r#"[{"item":"wand_lightning","upgrade":2}]"#);
    let mut with_resin = baseline.clone();
    with_resin.arcane_resin = 12;
    let estimate = estimate_match_probability(&with_resin);
    assert!(estimate > 0.0 && estimate < estimate_match_probability(&baseline));
}

#[test]
fn resin_respects_donor_curse_floor_and_source_filters() {
    let mut resin = query(6, "[]");
    resin.max_depth = 9;
    let broad = estimate_match_probability(&resin);
    resin.arcane_resin_filter.max_depth = Some(4);
    let early = estimate_match_probability(&resin);
    assert!(early > 0.0 && early < broad);
    resin.arcane_resin_filter.uncursed = false;
    assert!(estimate_match_probability(&resin) > early);
    resin.arcane_resin_filter.source = Some(ItemSource::WandmakerReward);
    assert_eq!(estimate_match_probability(&resin), 0.0);
    resin.arcane_resin_filter.max_depth = None;
    assert!(estimate_match_probability(&resin) > 0.0);
    resin.arcane_resin_filter.source = Some(ItemSource::GhostReward);
    assert_eq!(estimate_match_probability(&resin), 0.0);
}

#[test]
fn resin_works_with_selected_and_automatic_trinkets_and_linked_wands() {
    for requirements in [
        r#"[{"item":"mimic_tooth","select_trinket":true}]"#,
        r#"[{"kind":"wand","identity_group":1},{"kind":"wand","identity_group":1}]"#,
        r#"[{"any_of":[{"item":"wand_lightning"},{"item":"wand_frost"}]}]"#,
    ] {
        let resin = query(3, requirements);
        let estimate = estimate_match_probability(&resin);
        assert!(estimate > 0.0 && estimate <= 1.0, "{resin:?}: {estimate}");
    }
    let mut resin = query(3, "[]");
    resin.auto_apply_trinket = true;
    let estimate = estimate_match_probability(&resin);
    assert!(estimate > 0.0 && estimate <= 1.0);
    resin.arcane_resin = u16::MAX;
    assert_eq!(estimate_match_probability(&resin), 0.0);
}

#[test]
#[ignore = "held-out calibration; run with --release --ignored --nocapture"]
#[allow(clippy::cast_precision_loss)]
fn resin_estimates_track_generated_matches() {
    use shpd_seedfinder_core::{
        main_world::generate_main_world,
        seed::{DungeonSeed, TOTAL_SEEDS},
    };
    const WORLDS: u64 = 2048;
    let mut queries = Vec::new();
    for depth in [4, 9, 24] {
        for amount in [2, 3, 6, 12, 20] {
            for requirements in ["[]", r#"[{"item":"wand_lightning"}]"#] {
                let mut resin = query(amount, requirements);
                resin.max_depth = depth;
                queries.push(resin);
            }
        }
    }
    let mut hits = vec![0_u64; queries.len()];
    for index in 0..WORLDS {
        let seed =
            DungeonSeed::new((index * (TOTAL_SEEDS / WORLDS) + 123_457) % TOTAL_SEEDS).unwrap();
        let world = generate_main_world(seed, 24).unwrap();
        for (query, hits) in queries.iter().zip(&mut hits) {
            *hits += u64::from(query.matches(&world));
        }
    }
    for (query, hits) in queries.iter().zip(hits) {
        let estimate = estimate_match_probability(query);
        let observed = hits as f64 / WORLDS as f64;
        eprintln!(
            "depth {}, resin {}, {} wands: {estimate:.5} vs {observed:.5}",
            query.max_depth,
            query.arcane_resin,
            query.requirements.len()
        );
        let noise =
            3.0 * ((observed.max(1.0 / WORLDS as f64)) * (1.0 - observed) / WORLDS as f64).sqrt();
        assert!(
            (estimate - observed).abs() <= 0.5 * observed + noise,
            "{query:?}: estimate {estimate}, observed {observed}"
        );
    }
}
