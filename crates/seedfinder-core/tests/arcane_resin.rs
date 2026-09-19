use shpd_seedfinder_core::catalog::ItemId;
use shpd_seedfinder_core::feasibility::QueryPlan;
use shpd_seedfinder_core::main_world::{CanonicalMainWorldGenerator, generate_main_world};
use shpd_seedfinder_core::model::{Accessibility, GeneratedWorld, ItemSource, WorldItem};
use shpd_seedfinder_core::query::{ArcaneResinFilter, SearchQuery, scout_matches};
use shpd_seedfinder_core::quests::QuestSummary;
use shpd_seedfinder_core::run::RingGems;
use shpd_seedfinder_core::search::{FloorGate, WorldGenerator};
use shpd_seedfinder_core::seed::DungeonSeed;
use shpd_seedfinder_core::{deep_link, json_query, results_export};

fn query(amount: u16, requirements: &str) -> SearchQuery {
    json_query::decode(&format!(
        r#"{{"arcane_resin":{amount},"requirements":{requirements}}}"#
    ))
    .unwrap()
}

fn wand(upgrade: u8) -> WorldItem {
    WorldItem {
        item: ItemId::WandLightning,
        upgrade,
        effect: None,
        cursed: false,
        depth: 3,
        source: ItemSource::Heap,
        accessibility: Accessibility::Independent,
        secret: false,
    }
}

fn world(items: Vec<WorldItem>) -> GeneratedWorld {
    GeneratedWorld {
        feelings: Vec::new(),
        seed: DungeonSeed::MIN,
        items,
        quests: QuestSummary::default(),
        ring_gems: RingGems::UNSHUFFLED,
    }
}

#[test]
fn resin_totals_accept_overpayment_and_reserve_required_wands() {
    let resin = query(6, "[]");
    for upgrades in [vec![0, 0, 0], vec![0, 1], vec![2], vec![3]] {
        assert!(resin.matches(&world(upgrades.into_iter().map(wand).collect())));
    }
    assert!(!resin.matches(&world(vec![wand(0), wand(0)])));
    let lightning = query(3, r#"[{"item":"wand_lightning","upgrade":2}]"#);
    assert!(!lightning.matches(&world(vec![wand(2)])));
    assert!(!lightning.matches(&world(vec![wand(2), wand(0)])));
    for extras in [vec![wand(0), wand(0)], vec![wand(1)]] {
        let mut items = vec![wand(2)];
        items.extend(extras);
        let world = world(items);
        assert!(lightning.matches(&world));
        let marks = scout_matches(&world, &lightning);
        assert_eq!(marks.total_requirements, 2);
        assert_eq!(marks.matched_requirements, 2);
        assert!(marks.matched.iter().all(|&matched| matched));
    }
}

#[test]
fn ordinary_assignment_backtracks_to_leave_enough_resin() {
    let query = query(6, r#"[{"kind":"wand"}]"#);
    // Reserving the first wand fails; reserving the second succeeds.
    let world = world(vec![wand(2), wand(0)]);
    assert!(query.matches(&world));
    assert_eq!(scout_matches(&world, &query).matched_requirements, 2);
}

#[test]
fn surplus_wands_must_be_uncursed_in_scope_and_from_allowed_sources() {
    let mut query = query(2, "[]");
    let mut candidate = wand(0);
    candidate.cursed = true;
    assert!(!query.matches(&world(vec![candidate.clone()])));
    candidate.cursed = false;
    query.max_depth = 2;
    assert!(!query.matches(&world(vec![candidate.clone()])));
    query.max_depth = 3;
    assert!(query.matches(&world(vec![candidate.clone()])));
    candidate.source = ItemSource::BlacksmithReward;
    query.exclude_blacksmith_rewards = true;
    assert!(!query.matches(&world(vec![candidate])));
}

#[test]
fn resin_respects_reward_choices_and_overlapping_scenarios() {
    let mut first = wand(1);
    first.accessibility = Accessibility::Choice {
        group: 1,
        option: 0,
    };
    let mut second = wand(1);
    second.accessibility = Accessibility::Choice {
        group: 1,
        option: 1,
    };
    assert!(!query(6, "[]").matches(&world(vec![first.clone(), second.clone()])));
    assert!(query(4, "[]").matches(&world(vec![first.clone(), second.clone()])));

    let mut reserved = wand(2);
    reserved.accessibility = first.accessibility;
    let query = query(4, r#"[{"item":"wand_lightning","upgrade":2}]"#);
    assert!(!query.matches(&world(vec![reserved.clone(), second.clone()])));
    assert!(query.matches(&world(vec![reserved, first.clone()])));

    first.accessibility = Accessibility::Scenarios {
        group: 1,
        mask: 0b011,
    };
    second.accessibility = Accessibility::Scenarios {
        group: 1,
        mask: 0b110,
    };
    let mut third = wand(0);
    third.accessibility = Accessibility::Scenarios {
        group: 1,
        mask: 0b101,
    };
    let world = world(vec![first, second, third]);
    let resin = self::query(8, "[]");
    assert!(resin.matches(&world));
    assert!(!self::query(9, "[]").matches(&world));
    assert_eq!(scout_matches(&world, &resin).matched_indices(), vec![0, 1]);
}

#[test]
fn resin_filters_apply_only_to_surplus_wands() {
    let mut resin = query(4, r#"[{"item":"wand_lightning","upgrade":2}]"#);
    resin.arcane_resin_filter = ArcaneResinFilter {
        uncursed: false,
        max_depth: Some(4),
        source: Some(ItemSource::Chest),
    };
    let mut reserved = wand(2);
    reserved.depth = 9;
    let mut surplus = wand(1);
    surplus.cursed = true;
    surplus.source = ItemSource::Chest;
    let candidate = world(vec![reserved.clone(), surplus.clone()]);
    assert!(resin.matches(&candidate));
    assert_eq!(
        scout_matches(&candidate, &resin).matched_indices(),
        vec![0, 1]
    );
    resin.arcane_resin_filter.uncursed = true;
    assert!(!resin.matches(&candidate));
    resin.arcane_resin_filter.uncursed = false;
    surplus.depth = 5;
    assert!(!resin.matches(&world(vec![reserved.clone(), surplus.clone()])));
    surplus.depth = 4;
    surplus.source = ItemSource::Heap;
    assert!(!resin.matches(&world(vec![reserved, surplus])));
    // A local limit never extends the global search scope.
    resin.requirements.clear();
    resin.arcane_resin_filter.source = None;
    resin.max_depth = 2;
    assert!(!resin.matches(&world(vec![wand(1)])));
}

#[test]
fn resin_filter_refinement_never_reuses_a_wider_supply() {
    let mut base = query(4, "[]");
    base.arcane_resin_filter.uncursed = false;
    let mut candidate = base.clone();
    candidate.arcane_resin_filter.uncursed = true;
    assert!(candidate.continues(&base));
    assert!(!base.continues(&candidate));
    base = candidate.clone();
    candidate.arcane_resin_filter.max_depth = Some(4);
    assert!(candidate.continues(&base));
    assert!(!base.continues(&candidate));
    base = candidate.clone();
    candidate.arcane_resin_filter.source = Some(ItemSource::Chest);
    assert!(candidate.continues(&base));
    assert!(!base.continues(&candidate));
    base = candidate.clone();
    candidate.arcane_resin_filter.source = Some(ItemSource::Heap);
    assert!(!candidate.continues(&base));
    assert!(!base.continues(&candidate));
}

#[test]
fn resin_filters_round_trip_and_reject_invalid_values() {
    for uncursed in [false, true] {
        for max_depth in [None, Some(1), Some(24)] {
            for source in [
                None,
                Some(ItemSource::Heap),
                Some(ItemSource::VaultTreasure),
            ] {
                let mut resin = query(6, "[]");
                resin.arcane_resin_filter = ArcaneResinFilter {
                    uncursed,
                    max_depth,
                    source,
                };
                assert_eq!(
                    json_query::decode(&json_query::encode(&resin).to_string()).unwrap(),
                    resin
                );
                assert_eq!(
                    deep_link::decode(&deep_link::encode(&resin).unwrap()).unwrap(),
                    resin
                );
                let exported = results_export::encode(&resin, &[DungeonSeed::MIN], "test");
                assert_eq!(results_export::decode(&exported).unwrap().query, resin);
            }
        }
    }
    for invalid in [
        r#"{"uncursed":"yes"}"#,
        r#"{"max_depth":0}"#,
        r#"{"max_depth":25}"#,
        r#"{"source":"unknown"}"#,
        "null",
    ] {
        assert!(
            json_query::decode(&format!(
                r#"{{"arcane_resin":2,"arcane_resin_filter":{invalid},"requirements":[]}}"#
            ))
            .is_err()
        );
    }
}

#[test]
fn resin_continuation_and_portable_formats_preserve_the_minimum() {
    let base = query(3, r#"[{"kind":"wand"}]"#);
    assert!(query(6, r#"[{"item":"wand_lightning"}]"#).continues(&base));
    assert!(!query(2, r#"[{"kind":"wand"}]"#).continues(&base));
    assert!(!query(0, r#"[{"kind":"wand"}]"#).continues(&base));
    assert!(base.shares_item(&query(6, "[]")));
    assert!(query(6, "[]").shares_item(&query(0, r#"[{"kind":"wand"}]"#)));
    assert!(!query(6, "[]").shares_item(&query(0, r#"[{"kind":"ring"}]"#)));
    for amount in [1, 3, 6, u16::MAX] {
        for automatic in [false, true] {
            for requirements in ["[]", r#"[{"item":"wand_lightning","upgrade":2}]"#] {
                let mut query = query(amount, requirements);
                query.auto_apply_trinket = automatic;
                assert_eq!(
                    json_query::decode(&json_query::encode(&query).to_string()).unwrap(),
                    query
                );
                assert_eq!(
                    deep_link::decode(&deep_link::encode(&query).unwrap()).unwrap(),
                    query
                );
                let exported = results_export::encode(&query, &[DungeonSeed::MIN], "test");
                assert_eq!(results_export::decode(&exported).unwrap().query, query);
            }
        }
    }
    for invalid in ["-1", "1.5", "65536", "true", "null", "\"6\""] {
        assert!(
            json_query::decode(&format!(
                r#"{{"arcane_resin":{invalid},"requirements":[]}}"#
            ))
            .is_err()
        );
    }
    assert!(json_query::decode(r#"{"requirements":[]}"#).is_err());
    // Freeze version 7 so future format changes must still read these links.
    let frozen = query(3, r#"[{"item":"wand_lightning","upgrade":2}]"#);
    assert_eq!(deep_link::encode(&frozen).unwrap(), "cAAAwZbCgAA");
    assert_eq!(deep_link::decode("cAAAwZbCgAA").unwrap(), frozen);
    let selected = query(
        6,
        r#"[{"item":"mimic_tooth","select_trinket":true},{"any_of":[{"item":"wand_lightning"},{"item":"wand_frost"}]}]"#,
    );
    assert_eq!(
        deep_link::decode(&deep_link::encode(&selected).unwrap()).unwrap(),
        selected
    );
    let old = query(0, r#"[{"kind":"wand"}]"#);
    assert!(json_query::encode(&old).get("arcane_resin").is_none());
    assert_eq!(
        deep_link::decode(&deep_link::encode(&old).unwrap()).unwrap(),
        old
    );
}

#[test]
fn resin_plans_keep_later_wands_and_vault_supply() {
    let query = query(
        6,
        r#"[{"item":"wand_lightning","source":"wandmaker_reward"}]"#,
    );
    let plan = QueryPlan::analyze(&query);
    assert_eq!(plan.generation_depth(), 24);
    assert!(plan.wants_vault_treasure());
    assert!(!plan.is_unsatisfiable());
    assert!(shpd_seedfinder_core::probability::estimate_match_probability(&query).is_nan());

    // Compare optimized search with complete generation across both sides
    // of the Wandmaker and Imp windows, including resin-only searches.
    let seeds: Vec<_> = (0..12)
        .map(|seed| DungeonSeed::new(seed).unwrap())
        .collect();
    for depth in [4, 9, 19, 24] {
        let worlds: Vec<_> = seeds
            .iter()
            .map(|&seed| generate_main_world(seed, depth).unwrap())
            .collect();
        for requirements in [
            "[]",
            r#"[{"kind":"wand"}]"#,
            r#"[{"kind":"wand","source":"wandmaker_reward"}]"#,
        ] {
            let mut query = self::query(6, requirements);
            query.max_depth = depth;
            let plan = QueryPlan::analyze(&query);
            let generated = CanonicalMainWorldGenerator.generate_batch_gated(
                &seeds,
                plan.generation_depth(),
                &plan,
            );
            for (full, gated) in worlds.iter().zip(generated) {
                assert_eq!(
                    gated.is_some_and(|world| query.matches(&world)),
                    query.matches(full),
                    "seed {:?}, query {query:?}",
                    full.seed
                );
            }
        }
    }
}
