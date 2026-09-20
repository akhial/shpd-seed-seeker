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
fn auto_estimates_account_for_upgrade_costs_and_zero_demand() {
    for (upgrades, amount) in [([1, 1], 10), ([2, 3], 3), ([3, 3], 0)] {
        let requirements = format!(
            r#"[{{"kind":"wand","upgrade":{{"exact":{}}}}},{{"kind":"wand","upgrade":{{"exact":{}}}}}]"#,
            upgrades[0], upgrades[1]
        );
        let fixed = query(amount, &requirements);
        let mut auto = fixed.clone();
        auto.arcane_resin_auto = true;
        auto.arcane_resin = 0;
        assert!(
            (estimate_match_probability(&auto) - estimate_match_probability(&fixed)).abs() < 1e-12
        );
    }
    for requirements in [
        r#"[{"kind":"wand"},{"kind":"wand"}]"#,
        r#"[{"kind":"wand","identity_group":1},{"kind":"wand","identity_group":1}]"#,
        r#"[{"any_of":[{"item":"wand_lightning"},{"item":"wand_frost"}]}]"#,
        r#"[{"item":"mimic_tooth","select_trinket":true},{"kind":"wand"}]"#,
    ] {
        let baseline = query(0, requirements);
        let mut auto = baseline.clone();
        auto.arcane_resin_auto = true;
        let estimate = estimate_match_probability(&auto);
        assert!(estimate > 0.0 && estimate <= estimate_match_probability(&baseline));
    }
    // Already +3 wands still have a chance even when no donor source exists.
    let mut auto = query(0, r#"[{"kind":"wand"}]"#);
    auto.arcane_resin_auto = true;
    auto.arcane_resin_filter.source = Some(ItemSource::GhostReward);
    assert!(
        (estimate_match_probability(&auto)
            - estimate_match_probability(&query(
                0,
                r#"[{"kind":"wand","upgrade":{"at_least":3}}]"#
            )))
        .abs()
            < 1e-7
    );
}

#[test]
fn resin_estimates_stay_fast_with_cold_caches() {
    for count in [1, 2, 4, 8] {
        for automatic in [false, true] {
            let mut resin = query(0, r#"[{"kind":"wand"}]"#);
            resin.arcane_resin_auto = true;
            resin.auto_apply_trinket = automatic;
            resin.requirements = vec![resin.requirements[0]; count];
            // Each query gets a fresh thread-local cache, as on first load.
            let (elapsed, estimate) = std::thread::spawn(move || {
                let start = std::time::Instant::now();
                let estimate = estimate_match_probability(&resin);
                (start.elapsed(), estimate)
            })
            .join()
            .unwrap();
            eprintln!("Auto {count} wands, AutoTrinket {automatic}: {elapsed:?}, {estimate}");
            assert!((0.0..=1.0).contains(&estimate));
            let budget = if cfg!(debug_assertions) { 2000 } else { 100 };
            assert!(
                elapsed < std::time::Duration::from_millis(budget),
                "{count} wands, AutoTrinket {automatic}: {elapsed:?}"
            );
        }
    }
}

#[test]
fn cached_resin_estimates_keep_query_filters_and_profiles_separate() {
    let mut base = query(0, r#"[{"kind":"wand"},{"kind":"wand"},{"kind":"wand"}]"#);
    base.arcane_resin_auto = true;
    let mut variants = vec![base.clone()];
    for edit in 0..9 {
        let mut variant = base.clone();
        match edit {
            0 => variant.max_depth = 9,
            1 => variant.arcane_resin_filter.max_depth = Some(4),
            2 => variant.arcane_resin_filter.uncursed = false,
            3 => variant.arcane_resin_filter.source = Some(ItemSource::WandmakerReward),
            4 => {
                variant.arcane_resin_auto = false;
                variant.arcane_resin = 12;
            }
            5 => variant.auto_apply_trinket = true,
            6 => variant.exclude_blacksmith_rewards = true,
            7 => variant.requirements.push(query(0, r#"[{"kind":"wand"},{"kind":"wand","upgrade":3,"blanket":true}]"#).requirements[1]),
            _ => variant.requirements.push(query(0, r#"[{"kind":"wand"},{"kind":"wand","source":"wandmaker_reward","blanket":true}]"#).requirements[1]),
        }
        variants.push(variant);
    }
    let fresh: Vec<_> = variants
        .iter()
        .cloned()
        .map(|query| {
            std::thread::spawn(move || estimate_match_probability(&query))
                .join()
                .unwrap()
        })
        .collect();
    for (query, expected) in variants
        .iter()
        .zip(&fresh)
        .chain(variants.iter().zip(&fresh).rev())
    {
        assert_eq!(estimate_match_probability(query), *expected, "{query:?}");
    }
}

#[test]
#[allow(clippy::cast_precision_loss)] // Small, bounded sample counts.
fn auto_resin_estimates_track_generated_wand_combinations() {
    use shpd_seedfinder_core::{
        main_world::generate_main_world,
        seed::{DungeonSeed, TOTAL_SEEDS},
    };
    const WORLDS: u64 = 512;
    let mut queries: Vec<_> = [2, 3, 4]
        .into_iter()
        .map(|count| {
            let mut resin = query(0, r#"[{"kind":"wand"}]"#);
            resin.arcane_resin_auto = true;
            resin.requirements = vec![resin.requirements[0]; count];
            resin
        })
        .collect();
    for requirements in [
        r#"[{"kind":"wand"},{"kind":"wand"},{"kind":"wand"},{"kind":"wand","upgrade":3,"blanket":true}]"#,
        r#"[{"kind":"wand"},{"kind":"wand"},{"kind":"wand","upgrade":1,"uncursed":true,"blanket":true}]"#,
    ] {
        let mut resin = query(0, requirements);
        resin.arcane_resin_auto = true;
        queries.push(resin);
    }
    let mut hits = vec![0_u64; queries.len()];
    for index in 0..WORLDS {
        let seed =
            DungeonSeed::new((index * (TOTAL_SEEDS / WORLDS) + 823_471) % TOTAL_SEEDS).unwrap();
        let world = generate_main_world(seed, 24).unwrap();
        for (query, hits) in queries.iter().zip(&mut hits) {
            *hits += u64::from(query.matches(&world));
        }
    }
    for (query, hits) in queries.iter().zip(hits) {
        let estimate = estimate_match_probability(query);
        let observed = hits as f64 / WORLDS as f64;
        let noise = 3.0 * (observed * (1.0 - observed) / WORLDS as f64).sqrt();
        assert!(
            (estimate - observed).abs() <= 0.25 * observed + noise,
            "{} wands: {estimate} vs {observed}",
            query.requirements.len()
        );
    }
}

#[test]
#[ignore = "profile calibration; run with --release --ignored --nocapture"]
#[allow(clippy::cast_precision_loss)] // Small, bounded sample counts.
fn auto_resin_profiles_track_generated_matches() {
    use shpd_seedfinder_core::{
        feasibility::QueryPlan,
        main_world::generate_main_world_with_trinket,
        search::FloorGate,
        seed::{DungeonSeed, TOTAL_SEEDS},
    };
    const WORLDS: u64 = 1024;
    let documents = [
        r#"{"arcane_resin":"auto","auto_apply_trinket":true,"requirements":[{"kind":"wand"},{"kind":"wand"},{"kind":"wand"}]}"#,
        r#"{"arcane_resin":"auto","auto_apply_trinket":true,"requirements":[{"kind":"wand"},{"kind":"wand"},{"kind":"wand"},{"kind":"wand"}]}"#,
        r#"{"arcane_resin":"auto","requirements":[{"item":"mimic_tooth","select_trinket":true},{"kind":"wand"},{"kind":"wand"},{"kind":"wand"}]}"#,
        r#"{"arcane_resin":"auto","auto_apply_trinket":true,"requirements":[{"item":"wand_lightning"},{"kind":"wand"}]}"#,
    ];
    let queries: Vec<_> = documents
        .iter()
        .map(|json| json_query::decode(json).unwrap())
        .collect();
    let plans: Vec<_> = queries.iter().map(QueryPlan::analyze).collect();
    let mut hits = vec![0_u64; queries.len()];
    for index in 0..WORLDS {
        let seed =
            DungeonSeed::new((index * (TOTAL_SEEDS / WORLDS) + 597_431) % TOTAL_SEEDS).unwrap();
        let mut worlds = std::collections::BTreeMap::new();
        for ((query, plan), hits) in queries.iter().zip(&plans).zip(&mut hits) {
            let selected = plan.selected_trinket(seed);
            let world = worlds.entry(selected).or_insert_with(|| {
                generate_main_world_with_trinket(seed, 24, query.challenges, selected).unwrap()
            });
            *hits += u64::from(query.matches(world));
        }
    }
    for ((query, document), hits) in queries.iter().zip(documents).zip(hits) {
        let estimate = estimate_match_probability(query);
        let observed = hits as f64 / WORLDS as f64;
        let noise = 3.0 * (observed * (1.0 - observed) / WORLDS as f64).sqrt();
        eprintln!("{document}: {estimate:.5} vs {observed:.5}");
        assert!(
            (estimate - observed).abs() <= 0.25 * observed + noise,
            "{estimate} vs {observed}"
        );
    }
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
fn blanket_and_resin_cannot_both_claim_the_wandmaker_reward() {
    let mut resin = query(
        2,
        r#"[{"kind":"wand"},{"kind":"wand","source":"wandmaker_reward","blanket":true}]"#,
    );
    resin.arcane_resin_filter.source = Some(ItemSource::WandmakerReward);
    for amount in [2, 4, 8] {
        resin.arcane_resin = amount;
        assert_eq!(estimate_match_probability(&resin), 0.0);
    }
    resin.arcane_resin_filter.source = None;
    assert!(estimate_match_probability(&resin) > 0.0);
}

#[test]
fn resin_with_one_blanket_witness_matches_explicit_filters() {
    for (blanket, direct) in [
        (
            r#"{"kind":"wand","source":"wandmaker_reward","blanket":true}"#,
            r#"[{"item":"wand_frost","source":"wandmaker_reward"}]"#,
        ),
        (
            r#"{"kind":"wand","upgrade":3,"blanket":true}"#,
            r#"[{"item":"wand_frost","upgrade":3}]"#,
        ),
        (
            r#"{"kind":"wand","uncursed":true,"max_depth":4,"blanket":true}"#,
            r#"[{"item":"wand_frost","uncursed":true,"max_depth":4}]"#,
        ),
    ] {
        for amount in [0, 2, 6, 12] {
            let mut blanket = query(amount, &format!(r#"[{{"item":"wand_frost"}},{blanket}]"#));
            let mut direct = query(amount, direct);
            blanket.arcane_resin_auto = amount == 0;
            direct.arcane_resin_auto = amount == 0;
            for source in [
                None,
                Some(ItemSource::WandmakerReward),
                Some(ItemSource::Chest),
            ] {
                blanket.arcane_resin_filter.source = source;
                direct.arcane_resin_filter.source = source;
                let estimate = estimate_match_probability(&blanket);
                let expected = estimate_match_probability(&direct);
                assert!(
                    (estimate - expected).abs() < 1e-12,
                    "{blanket:?}: {estimate} != {expected}"
                );
            }
        }
    }
}

#[test]
fn resin_preserves_unknown_for_an_unmodeled_optional_blanket_witness() {
    // The level-sum approximation keeps the first sufficient member, but
    // the blanket needs the other optional member. This is unknown, not zero.
    for (amount, auto) in [(0, false), (2, false), (6, false), (12, false), (0, true)] {
        let mut resin = query(
            amount,
            r#"[
            {"item":"ring_haste","upgrade":1,"level_sum":{"group":1,"at_least":1}},
            {"item":"ring_force","upgrade":1,"level_sum":{"group":1,"at_least":1}},
            {"item":"ring_force","blanket":true}
        ]"#,
        );
        resin.arcane_resin_auto = auto;
        assert!(estimate_match_probability(&resin).is_nan());
    }
}

#[test]
fn mixed_blanket_resin_estimates_preserve_redundancy_and_monotonicity() {
    for requirements in [
        r#"[{"kind":"wand"},{"kind":"wand"},{"kind":"wand","upgrade":3,"blanket":true}]"#,
        r#"[{"any_of":[{"item":"wand_frost"},{"item":"wand_lightning"}]},{"kind":"wand","uncursed":true,"blanket":true}]"#,
        r#"[{"kind":"wand","identity_group":1},{"kind":"wand","identity_group":1},{"kind":"wand","upgrade":{"at_least":1},"blanket":true}]"#,
        r#"[{"item":"mimic_tooth","select_trinket":true},{"kind":"wand"},{"kind":"wand","uncursed":true,"blanket":true}]"#,
    ] {
        let mut previous = 1.0;
        for amount in [0, 2, 6, 12, u16::MAX] {
            let resin = query(amount, requirements);
            let estimate = estimate_match_probability(&resin);
            assert!(
                (0.0..=previous + 1e-12).contains(&estimate),
                "{resin:?}: {estimate} > {previous}"
            );
            previous = estimate;
            let mut repeated = resin.clone();
            repeated
                .requirements
                .push(*resin.requirements.last().unwrap());
            assert!((estimate_match_probability(&repeated) - estimate).abs() < 1e-12);
            let mut ordinary = resin.clone();
            ordinary.requirements.retain(|r| !r.blanket);
            assert!(estimate <= estimate_match_probability(&ordinary) + 1e-12);
        }
    }
}

#[test]
fn auto_blankets_preserve_bounds_redundancy_and_unsupported_estimates() {
    for requirements in [
        r#"[{"kind":"wand"},{"kind":"wand"},{"kind":"wand","upgrade":3,"blanket":true}]"#,
        r#"[{"any_of":[{"item":"wand_frost"},{"item":"wand_lightning"}]},{"item":"wand_lightning","blanket":true}]"#,
        r#"[{"kind":"wand","identity_group":1},{"kind":"wand","identity_group":1},{"kind":"wand","upgrade":1,"blanket":true}]"#,
        r#"[{"item":"mimic_tooth","select_trinket":true},{"kind":"wand"},{"kind":"wand","uncursed":true,"blanket":true}]"#,
    ] {
        for automatic in [false, true] {
            let mut resin = query(0, requirements);
            resin.arcane_resin_auto = true;
            resin.auto_apply_trinket = automatic;
            let estimate = estimate_match_probability(&resin);
            assert!(estimate > 0.0 && estimate <= 1.0, "{resin:?}: {estimate}");
            let mut repeated = resin.clone();
            repeated
                .requirements
                .push(*resin.requirements.last().unwrap());
            assert!((estimate_match_probability(&repeated) - estimate).abs() < 1e-12);
            let mut ordinary = resin.clone();
            ordinary.requirements.retain(|r| !r.blanket);
            // AutoTrinket can choose different profiles for the relaxed query.
            // Bounds compare constraints under the same generation policy.
            if !automatic {
                assert!(estimate <= estimate_match_probability(&ordinary) + 1e-12);
                resin.arcane_resin_auto = false;
                assert!(estimate <= estimate_match_probability(&resin) + 1e-12);
            }
        }
    }
    let mut large = query(0, r#"[{"any_of":[{"kind":"armor"},{"kind":"wand"}]}]"#);
    let pair = large.requirements.clone();
    for group in 2..=8 {
        large.requirements.extend(
            pair.iter()
                .map(|r| shpd_seedfinder_core::query::Requirement {
                    alternative_group: Some(group),
                    ..*r
                }),
        );
    }
    large
        .requirements
        .push(query(0, r#"[{"kind":"armor","blanket":true},{"kind":"armor"}]"#).requirements[0]);
    large.arcane_resin_auto = true;
    assert!(estimate_match_probability(&large).is_nan());
}

#[test]
fn auto_blanket_reward_is_reserved_even_when_donors_share_its_source() {
    let mut resin = query(
        0,
        r#"[{"kind":"wand"},{"kind":"wand","upgrade":2,"source":"wandmaker_reward","blanket":true}]"#,
    );
    resin.arcane_resin_auto = true;
    resin.arcane_resin_filter.source = Some(ItemSource::WandmakerReward);
    assert_eq!(estimate_match_probability(&resin), 0.0);
    resin.requirements[1].upgrade = shpd_seedfinder_core::query::UpgradeRequirement::Exact(3);
    let estimate = estimate_match_probability(&resin);
    assert!(estimate > 0.0);
    resin.arcane_resin_auto = false;
    assert!((estimate_match_probability(&resin) - estimate).abs() < 1e-12);
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
        for count in 1..=4 {
            let mut resin = query(0, r#"[{"kind":"wand"}]"#);
            resin.arcane_resin_auto = true;
            resin.max_depth = depth;
            resin.requirements = vec![resin.requirements[0]; count];
            queries.push(resin);
        }
        for amount in [2, 3, 6, 12, 20] {
            for requirements in [
                "[]",
                r#"[{"item":"wand_lightning"}]"#,
                r#"[{"kind":"wand"},{"kind":"wand","uncursed":true,"blanket":true}]"#,
                r#"[{"kind":"wand"},{"kind":"wand"},{"kind":"wand","upgrade":3,"blanket":true}]"#,
                r#"[{"item":"wand_frost"},{"kind":"wand","source":"wandmaker_reward","blanket":true}]"#,
            ] {
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
