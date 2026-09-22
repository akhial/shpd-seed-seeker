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

fn auto_query(requirements: &str) -> SearchQuery {
    json_query::decode(&format!(
        r#"{{"arcane_resin":"auto","requirements":{requirements}}}"#
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
        floor_rooms: Vec::new(),
        feelings: Vec::new(),
        seed: DungeonSeed::MIN,
        items,
        quests: QuestSummary::default(),
        ring_gems: RingGems::UNSHUFFLED,
    }
}

#[test]
fn starting_wand_is_one_fixed_credit_not_a_generated_item() {
    let mut resin = query(2, "[]");
    resin.arcane_resin_filter.include_mage_wand = true;
    // Donor filters apply to generated loot, not the explicit starting credit.
    resin.arcane_resin_filter.source = Some(ItemSource::GhostReward);
    resin.arcane_resin_filter.max_depth = Some(1);
    assert!(resin.matches(&world(vec![])));
    let marks = scout_matches(&world(vec![]), &resin);
    assert_eq!(marks.matched_requirements, 1);
    assert!(marks.matched_indices().is_empty());
    resin.arcane_resin = 3;
    assert!(!resin.matches(&world(vec![])));
    resin.arcane_resin = 2;
    resin.requirements = query(0, r#"[{"item":"wand_magic_missile"}]"#).requirements;
    assert!(!resin.matches(&world(vec![])));
}

#[test]
fn mage_credit_and_exclusions_handle_each_generated_upgrade() {
    for (upgrade, cost) in [(0, 6), (1, 5), (2, 3), (3, 0), (4, 0)] {
        for excluded in [false, true] {
            for mage in [false, true] {
                let mut resin = auto_query(&format!(
                    r#"[{{"item":"wand_lightning","upgrade":{{"at_least":{upgrade}}},"exclude_resin":{excluded}}}]"#
                ));
                resin.arcane_resin_filter.include_mage_wand = mage;
                for count in 0..=3 {
                    let mut items = vec![wand(upgrade)];
                    items.extend((0..count).map(|_| WorldItem {
                        item: ItemId::WandFrost,
                        ..wand(0)
                    }));
                    let candidate = world(items);
                    let needed = if excluded { 0 } else { cost };
                    let passes = count * 2 + if mage { 2 } else { 0 } >= needed;
                    assert_eq!(
                        resin.matches(&candidate),
                        passes,
                        "+{upgrade}, excluded={excluded}, mage={mage}, donors={count}"
                    );
                    let marks = scout_matches(&candidate, &resin);
                    assert_eq!(marks.matched_requirements == 2, passes);
                    if passes && excluded {
                        assert_eq!(marks.matched_indices(), vec![0]);
                    }
                }
            }
        }
    }
}

#[test]
fn excluded_wands_stay_reserved_and_cost_follows_the_chosen_alternative() {
    let resin = auto_query(
        r#"[{"item":"wand_lightning","exclude_resin":true},{"item":"wand_frost","upgrade":2}]"#,
    );
    let candidate = world(vec![
        wand(3),
        WorldItem {
            item: ItemId::WandFrost,
            ..wand(2)
        },
    ]);
    // The excluded +3 would supply eight resin if it were incorrectly donated.
    assert!(!resin.matches(&candidate));
    let fixed = query(2, r#"[{"item":"wand_lightning","exclude_resin":true}]"#);
    assert!(!fixed.matches(&world(vec![wand(3)])));
    // Both OR branches match the same item; only the second excludes its cost.
    let alternative =
        auto_query(r#"[{"any_of":[{"kind":"wand"},{"kind":"wand","exclude_resin":true}]}]"#);
    assert!(alternative.matches(&world(vec![wand(0)])));
    assert_eq!(
        scout_matches(&world(vec![wand(0)]), &alternative).matched_requirements,
        2
    );
    // Assignment must backtrack: exclude the low wand and keep the +3 at no cost.
    let duplicates = auto_query(r#"[{"kind":"wand","exclude_resin":true},{"kind":"wand"}]"#);
    assert!(duplicates.matches(&world(vec![wand(3), wand(0)])));
}

#[test]
fn credit_and_excluded_wands_do_not_create_blanket_donors() {
    let mut resin = query(
        2,
        r#"[{"item":"wand_lightning","exclude_resin":true},{"item":"wand_frost","blanket":true}]"#,
    );
    resin.arcane_resin_filter.include_mage_wand = true;
    let candidate = world(vec![
        wand(0),
        WorldItem {
            item: ItemId::WandFrost,
            ..wand(0)
        },
    ]);
    // Credit already covers the amount, so no generated wand is consumed.
    assert!(!resin.matches(&candidate));
    resin.arcane_resin_auto = true;
    assert!(!resin.matches(&candidate));
    resin.requirements[1].item = Some(ItemId::WandLightning);
    assert!(resin.matches(&candidate));
    assert_eq!(scout_matches(&candidate, &resin).matched_indices(), vec![0]);
}

#[test]
fn resin_planning_options_round_trip_and_validate() {
    for auto in [false, true] {
        for mage in [false, true] {
            let mut resin = query(
                7,
                r#"[{"item":"wand_lightning","exclude_resin":true},{"any_of":[{"item":"wand_frost"},{"item":"wand_fireblast","exclude_resin":true}]}]"#,
            );
            resin.arcane_resin_auto = auto;
            if auto {
                resin.arcane_resin = 0;
            }
            resin.arcane_resin_filter.include_mage_wand = mage;
            assert_eq!(
                json_query::decode(&json_query::encode(&resin).to_string()).unwrap(),
                resin
            );
            assert_eq!(
                deep_link::decode(&deep_link::encode(&resin).unwrap()).unwrap(),
                resin
            );
        }
    }
    for requirements in [
        r#"[{"kind":"ring","exclude_resin":true}]"#,
        r#"[{"kind":"wand"},{"kind":"wand","blanket":true,"exclude_resin":true}]"#,
        r#"[{"kind":"wand","exclude_resin":"true"}]"#,
    ] {
        assert!(json_query::decode(&format!(r#"{{"requirements":{requirements}}}"#)).is_err());
    }
}

#[test]
fn auto_upgrades_every_reserved_wand_to_three() {
    let query = auto_query(r#"[{"kind":"wand","max_depth":3},{"kind":"wand","max_depth":3}]"#);
    // Keep donors outside the requested wands' scope so their upgrades
    // cannot change which pair is reserved by the two wildcard slots.
    for (upgrades, needed) in [
        ([1, 0], 11),
        ([2, 3], 3),
        ([0, 0], 12),
        ([1, 1], 10),
        ([3, 3], 0),
        ([4, 3], 0),
    ] {
        let mut items: Vec<_> = upgrades.into_iter().map(wand).collect();
        for supplied in (0..=12).step_by(2) {
            items.truncate(2);
            items.extend((0..supplied / 2).map(|_| WorldItem {
                depth: 4,
                ..wand(0)
            }));
            let world = world(items.clone());
            assert_eq!(
                query.matches(&world),
                supplied >= needed,
                "{upgrades:?}, {supplied}"
            );
            let marks = scout_matches(&world, &query);
            assert_eq!(marks.total_requirements, 3);
            assert_eq!(marks.matched_requirements == 3, supplied >= needed);
            if supplied >= needed {
                assert_eq!(
                    marks.matched_indices().len(),
                    2 + usize::try_from((needed + 1) / 2).unwrap()
                );
            }
        }
    }
}

#[test]
fn auto_backtracks_over_wands_and_alternatives() {
    let query = auto_query(r#"[{"kind":"wand"}]"#);
    // A +0 costs 6 with just 4 available; reserving +1 costs 5 with only 2.
    assert!(!query.matches(&world(vec![wand(0), wand(1)])));
    // Reserving +0 first fails, but +3 needs no donors at all.
    let candidate = world(vec![
        WorldItem {
            accessibility: Accessibility::Choice {
                group: 1,
                option: 0,
            },
            ..wand(0)
        },
        WorldItem {
            accessibility: Accessibility::Choice {
                group: 1,
                option: 1,
            },
            ..wand(3)
        },
    ]);
    assert!(query.matches(&candidate));
    assert_eq!(scout_matches(&candidate, &query).matched_indices(), vec![1]);
    let alternatives =
        auto_query(r#"[{"any_of":[{"item":"wand_lightning"},{"item":"ring_haste"}]}]"#);
    let ring = WorldItem {
        item: ItemId::RingHaste,
        ..wand(0)
    };
    let candidate = world(vec![wand(0), ring]);
    assert!(alternatives.matches(&candidate));
    assert_eq!(
        scout_matches(&candidate, &alternatives).matched_indices(),
        vec![1]
    );
    assert!(auto_query("[]").matches(&world(vec![])));
}

#[test]
fn auto_preserves_donor_filters_and_reward_choices() {
    let mut query = auto_query(r#"[{"item":"wand_lightning","upgrade":2}]"#);
    let mut reserved = wand(2);
    reserved.accessibility = Accessibility::Choice {
        group: 1,
        option: 0,
    };
    let mut donor = WorldItem {
        item: ItemId::WandFrost,
        ..wand(1)
    };
    donor.accessibility = Accessibility::Choice {
        group: 1,
        option: 1,
    };
    assert!(!query.matches(&world(vec![reserved.clone(), donor.clone()])));
    donor.accessibility = Accessibility::Choice {
        group: 1,
        option: 0,
    };
    assert!(query.matches(&world(vec![reserved.clone(), donor.clone()])));
    donor.cursed = true;
    assert!(!query.matches(&world(vec![reserved.clone(), donor.clone()])));
    query.arcane_resin_filter.uncursed = false;
    assert!(query.matches(&world(vec![reserved.clone(), donor.clone()])));
    query.arcane_resin_filter.max_depth = Some(2);
    assert!(!query.matches(&world(vec![reserved.clone(), donor.clone()])));
    query.arcane_resin_filter.max_depth = None;
    query.arcane_resin_filter.source = Some(ItemSource::Chest);
    assert!(!query.matches(&world(vec![reserved.clone(), donor.clone()])));
    donor.source = ItemSource::Chest;
    assert!(query.matches(&world(vec![reserved, donor])));
}

#[test]
fn auto_blankets_share_reserved_wands_and_do_not_add_upgrade_cost() {
    let query =
        auto_query(r#"[{"kind":"wand","max_depth":3},{"kind":"wand","upgrade":1,"blanket":true}]"#);
    let mut candidate = world(vec![
        wand(0),
        wand(1),
        WorldItem {
            depth: 4,
            ..wand(1)
        },
    ]);
    // The blanket forces the +1 reservation: five resin, paid by the other
    // two wands' six resin. The witness must not add another five to the cost.
    assert!(query.matches(&candidate));
    let marks = scout_matches(&candidate, &query);
    assert_eq!(marks.total_requirements, 3);
    assert_eq!(marks.matched_requirements, 3);
    assert_eq!(marks.matched_indices(), vec![0, 1, 2]);
    candidate.items[1].accessibility = Accessibility::Choice {
        group: 1,
        option: 0,
    };
    candidate.items[2].accessibility = Accessibility::Choice {
        group: 1,
        option: 1,
    };
    assert!(!query.matches(&candidate));
}

#[test]
fn auto_donors_can_witness_blankets_and_zero_cost_still_counts() {
    let query =
        auto_query(r#"[{"item":"wand_lightning"},{"kind":"wand","upgrade":3,"blanket":true}]"#);
    let mut candidate = world(vec![
        wand(1),
        WorldItem {
            item: ItemId::WandFrost,
            ..wand(3)
        },
    ]);
    assert!(query.matches(&candidate));
    assert_eq!(scout_matches(&candidate, &query).matched_requirements, 3);
    candidate.items[0].upgrade = 3;
    candidate.items.pop();
    assert!(query.matches(&candidate));
    let marks = scout_matches(&candidate, &query);
    assert_eq!(marks.total_requirements, 3);
    assert_eq!(marks.matched_requirements, 3);
    assert_eq!(marks.matched_indices(), vec![0]);
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
        include_mage_wand: false,
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
                    include_mage_wand: false,
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
fn resin_plans_keep_later_wands_and_vault_supply() {
    let query = query(
        6,
        r#"[{"item":"wand_lightning","source":"wandmaker_reward"}]"#,
    );
    let plan = QueryPlan::analyze(&query);
    assert_eq!(plan.generation_depth(), 24);
    assert!(plan.wants_vault_treasure());
    assert!(!plan.is_unsatisfiable());
    assert!(shpd_seedfinder_core::probability::estimate_match_probability(&query).is_finite());

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
            for auto in [false, true] {
                let mut query = self::query(6, requirements);
                query.arcane_resin_auto = auto;
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
}
