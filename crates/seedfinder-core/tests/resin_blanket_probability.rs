#![allow(clippy::float_cmp)] // Exact zero is required for impossible allocations.

use shpd_seedfinder_core::{
    json_query,
    model::ItemSource,
    probability::estimate_match_probability,
    query::{SearchQuery, UpgradeRequirement},
};

fn query(resin: &str, ordinary: &str, blankets: &str) -> SearchQuery {
    json_query::decode(&format!(
        r#"{{"arcane_resin":{resin},"requirements":[{ordinary},{blankets}]}}"#
    ))
    .unwrap()
}

fn chance(query: &SearchQuery) -> f64 {
    let estimate = estimate_match_probability(query);
    assert!((0.0..=1.0).contains(&estimate), "{query:?}: {estimate}");
    estimate
}

fn assert_close(actual: f64, expected: f64) {
    assert!(
        (actual - expected).abs() < 1e-10 * expected.max(1e-8),
        "{actual} != {expected}"
    );
}

#[test]
fn reported_query_has_numeric_estimates_when_the_blanket_is_tightened() {
    let mut query = json_query::decode(include_str!("fixtures/resin-blanket-floor.json")).unwrap();
    for selected in [false, true] {
        if selected {
            query.auto_apply_trinket = false;
            query.requirements.insert(
                0,
                json_query::decode(
                    r#"{"requirements":[{"item":"mimic_tooth","select_trinket":true}]}"#,
                )
                .unwrap()
                .requirements[0],
            );
        }
        let mut previous = 1.0;
        for cap in [None, Some(4), Some(2)] {
            query.requirements.last_mut().unwrap().max_depth = cap;
            let estimate = chance(&query);
            assert!(
                estimate > 0.0 && estimate <= previous + 1e-15,
                "{cap:?}: {estimate} > {previous}"
            );
            previous = estimate;
        }
    }
}

#[test]
fn sufficient_donor_is_the_same_supply_as_an_explicit_extra_wand() {
    let ordinary = r#"{"item":"wand_lightning","upgrade":1}"#;
    let blanket =
        r#"{"item":"wand_corrosion","upgrade":2,"uncursed":true,"max_depth":4,"blanket":true}"#;
    let explicit = query(
        "0",
        ordinary,
        &blanket.replace("\"blanket\":true", "\"blanket\":false"),
    );
    let expected = chance(&explicit);
    assert!(expected > 0.0);
    for resin in ["2", "6", "\"auto\""] {
        let query = query(resin, ordinary, blanket);
        assert_close(chance(&query), expected);
    }
}

#[test]
fn auto_with_no_upgrade_cost_cannot_use_donors_as_blanket_witnesses() {
    let blanket = r#"{"item":"wand_corrosion","blanket":true}"#;
    for ordinary in [
        r#"{"item":"wand_lightning","upgrade":3}"#,
        r#"{"kind":"ring"}"#,
    ] {
        assert_eq!(chance(&query("\"auto\"", ordinary, blanket)), 0.0);
        assert!(chance(&query("2", ordinary, blanket)) > 0.0);
    }
    let query = query(
        "\"auto\"",
        r#"{"item":"wand_lightning","upgrade":{"at_least":2}}"#,
        blanket,
    );
    assert!(chance(&query) > 0.0);
}

#[test]
fn shared_donor_witnesses_count_their_resin_once() {
    let mut query = query(
        "4",
        r#"{"item":"wand_lightning","upgrade":1,"source":"heap"}"#,
        r#"
        {"item":"wand_corrosion","upgrade":1,"source":"wandmaker_reward","blanket":true},
        {"kind":"wand","source":"wandmaker_reward","max_depth":9,"blanket":true}
    "#,
    );
    query.arcane_resin_filter.source = Some(ItemSource::WandmakerReward);
    let shared = chance(&query);
    assert!(shared > 0.0);
    let mut single = query.clone();
    single.requirements.pop();
    assert_close(chance(&single), shared);
    query.arcane_resin = 5;
    // The only donor is a +1 Wandmaker reward, yielding four resin even
    // though it witnesses both blankets.
    assert_eq!(chance(&query), 0.0);
    query.arcane_resin_auto = true;
    assert_eq!(chance(&query), 0.0);
    query.requirements[0].upgrade = UpgradeRequirement::Exact(2);
    assert!(chance(&query) > 0.0);
}

#[test]
fn incompatible_donor_witnesses_cannot_share_a_reward() {
    let mut query = query(
        "8",
        r#"{"kind":"armor"}"#,
        r#"
        {"item":"wand_corrosion","blanket":true},
        {"item":"wand_frost","blanket":true}
    "#,
    );
    query.arcane_resin_filter.source = Some(ItemSource::WandmakerReward);
    assert_eq!(chance(&query), 0.0);
    query.arcane_resin_filter.source = Some(ItemSource::Heap);
    assert!(chance(&query) > 0.0);
}

#[test]
fn donor_filters_intersect_blanket_filters_without_becoming_global_filters() {
    let mut query = query(
        "2",
        r#"{"item":"wand_lightning"}"#,
        r#"
        {"item":"wand_corrosion","upgrade":2,"max_depth":4,"source":"chest","blanket":true}
    "#,
    );
    let broad = chance(&query);
    assert!(broad > 0.0);
    query.arcane_resin_filter.max_depth = Some(2);
    assert!(chance(&query) <= broad);
    query.arcane_resin_filter.max_depth = None;
    query.arcane_resin_filter.source = Some(ItemSource::Heap);
    assert_eq!(chance(&query), 0.0);
    query.arcane_resin_filter.source = Some(ItemSource::Chest);
    assert_close(chance(&query), broad);
    query.arcane_resin_filter.uncursed = false;
    assert!(chance(&query) >= broad);
}

#[test]
fn donor_alternatives_duplicates_and_cache_keys_preserve_their_meaning() {
    let ordinary = r#"{"item":"wand_lightning","upgrade":1}"#;
    let first = r#"{"item":"wand_corrosion","upgrade":2,"max_depth":4,"blanket":true}"#;
    let second = r#"{"item":"wand_frost","upgrade":2,"max_depth":4,"blanket":true}"#;
    let base = query("\"auto\"", ordinary, first);
    let alternative = query(
        "\"auto\"",
        ordinary,
        &format!(r#"{{"any_of":[{first},{second}]}}"#),
    );
    let duplicated = query("\"auto\"", ordinary, &format!("{first},{first}"));
    let mut ordinary_only = base.clone();
    ordinary_only.requirements.pop();
    let before = chance(&base);
    assert!(chance(&alternative) >= before);
    assert!(chance(&alternative) <= chance(&ordinary_only));
    assert_close(chance(&duplicated), before);
    let mut variants = vec![base.clone(), alternative, duplicated];
    for cap in [2, 9, 24] {
        let mut edited = base.clone();
        edited.requirements[1].max_depth = Some(cap);
        variants.push(edited);
    }
    for variant in variants.iter().chain(variants.iter().rev()) {
        let fresh = variant.clone();
        let expected = std::thread::spawn(move || chance(&fresh)).join().unwrap();
        assert_close(chance(variant), expected);
    }
}
