use shpd_seedfinder_core::{
    catalog::ItemId,
    challenges::Challenges,
    feasibility::QueryPlan,
    json_query,
    main_world::generate_main_world_with_trinket,
    model::{Accessibility, GeneratedWorld, ItemSource, WorldItem},
    query::{SearchQuery, scout_matches},
    quests::QuestSummary,
    run::RingGems,
    seed::DungeonSeed,
};

fn query(resin: &str, blankets: &str) -> SearchQuery {
    json_query::decode(&format!(
        r#"{{"arcane_resin":{resin},"requirements":[
        {{"item":"wand_lightning"}},{blankets}]}}"#
    ))
    .unwrap()
}

fn wand(item: ItemId, upgrade: u8, depth: u8) -> WorldItem {
    WorldItem {
        item,
        upgrade,
        depth,
        effect: None,
        cursed: false,
        source: ItemSource::Heap,
        accessibility: Accessibility::Independent,
        secret: false,
    }
}

fn world(items: Vec<WorldItem>) -> GeneratedWorld {
    GeneratedWorld {
        items,
        seed: DungeonSeed::MIN,
        quests: QuestSummary::default(),
        ring_gems: RingGems::UNSHUFFLED,
        floor_rooms: Vec::new(),
        artifact_decks: Vec::new(),
        feelings: Vec::new(),
    }
}

fn assert_matches(query: &SearchQuery, world: &GeneratedWorld, expected: bool) {
    assert_eq!(query.matches(world), expected);
    let marks = scout_matches(world, query);
    assert_eq!(
        marks.matched_requirements == marks.total_requirements,
        expected
    );
    if expected {
        let plan = QueryPlan::analyze(query);
        assert!(!plan.is_unsatisfiable());
        for depth in 1..=query.max_depth {
            let prefix: Vec<_> = world
                .items
                .iter()
                .filter(|item| item.depth <= depth)
                .cloned()
                .collect();
            assert!(
                plan.viable_after_floor(depth, &prefix, &world.quests),
                "floor {depth}"
            );
        }
    }
}

#[test]
fn reported_corrosion_seed_matches_the_early_blanket() {
    let mut query = json_query::decode(include_str!("fixtures/resin-blanket-floor.json")).unwrap();
    let world = generate_main_world_with_trinket(
        DungeonSeed::from_code("AAA-CAJ-QCN").unwrap(),
        22,
        Challenges::NONE,
        Some(ItemId::MimicTooth),
    )
    .unwrap();
    let corrosion = world
        .items
        .iter()
        .position(|item| item.item == ItemId::WandCorrosion && item.depth == 2 && item.upgrade == 2)
        .unwrap();
    for cap in [None, Some(4), Some(2)] {
        query.requirements.last_mut().unwrap().max_depth = cap;
        assert_matches(&query, &world, true);
        assert!(scout_matches(&world, &query).matched[corrosion]);
    }
    query.requirements.last_mut().unwrap().max_depth = Some(1);
    assert_matches(&query, &world, false);
}

#[test]
fn donor_witnesses_obey_both_blanket_and_donor_filters() {
    for resin in ["6", "\"auto\""] {
        let mut query = query(
            resin,
            r#"{"kind":"wand","upgrade":2,"max_depth":4,"blanket":true}"#,
        );
        let mut world = world(vec![
            wand(ItemId::WandLightning, 0, 6),
            wand(ItemId::WandCorrosion, 2, 2),
        ]);
        assert_matches(&query, &world, true);
        world.items[1].depth = 5;
        assert_matches(&query, &world, false);
        world.items[1].depth = 2;
        world.items[1].cursed = true;
        assert_matches(&query, &world, false);
        query.arcane_resin_filter.uncursed = false;
        assert_matches(&query, &world, true);
        query.arcane_resin_filter.max_depth = Some(1);
        assert_matches(&query, &world, false);
        query.arcane_resin_filter.max_depth = None;
        query.arcane_resin_filter.source = Some(ItemSource::Chest);
        assert_matches(&query, &world, false);
        world.items[1].source = ItemSource::Chest;
        assert_matches(&query, &world, true);
        query.arcane_resin = 0;
        query.arcane_resin_auto = false;
        assert_matches(&query, &world, false);
    }
}

#[test]
fn donors_backtrack_over_reward_choices_and_greedy_overpayment() {
    let query = query(
        "\"auto\"",
        r#"{"kind":"wand","upgrade":2,"max_depth":4,"blanket":true}"#,
    );
    let mut world = world(vec![
        wand(ItemId::WandLightning, 0, 6),
        wand(ItemId::WandFrost, 3, 2),
        wand(ItemId::WandCorrosion, 2, 2),
    ]);
    // The first donor already pays the whole cost; the later one is needed
    // for the blanket and must still be considered.
    assert_matches(&query, &world, true);
    assert!(scout_matches(&world, &query).matched[2]);
    world.items[1].accessibility = Accessibility::Choice {
        group: 1,
        option: 0,
    };
    world.items[2].accessibility = Accessibility::Choice {
        group: 1,
        option: 1,
    };
    assert_matches(&query, &world, true);
    assert_eq!(scout_matches(&world, &query).matched_indices(), vec![0, 2]);
    // The blanket's choice must leave enough resin and agree with reservations.
    let mut more_resin = query.clone();
    more_resin.arcane_resin_auto = false;
    more_resin.arcane_resin = 8;
    assert_matches(&more_resin, &world, false);
    world.items[0].accessibility = Accessibility::Choice {
        group: 1,
        option: 0,
    };
    assert_matches(&query, &world, false);
}

#[test]
fn blankets_share_donors_but_cannot_combine_incompatible_donors() {
    let query = query(
        "\"auto\"",
        r#"
        {"kind":"wand","upgrade":2,"blanket":true},
        {"item":"wand_corrosion","max_depth":4,"blanket":true}
    "#,
    );
    let mut world = world(vec![
        wand(ItemId::WandLightning, 0, 6),
        wand(ItemId::WandCorrosion, 2, 2),
    ]);
    assert_matches(&query, &world, true);
    assert_eq!(scout_matches(&world, &query).matched_indices(), vec![0, 1]);
    world.items[1].accessibility = Accessibility::Scenarios {
        group: 1,
        mask: 0b011,
    };
    world.items[1].upgrade = 3;
    world.items.push(WorldItem {
        accessibility: Accessibility::Scenarios {
            group: 1,
            mask: 0b110,
        },
        ..wand(ItemId::WandFrost, 2, 3)
    });
    assert_matches(&query, &world, true);
    world.items[2].accessibility = Accessibility::Scenarios {
        group: 1,
        mask: 0b100,
    };
    assert_matches(&query, &world, false);
    assert_eq!(scout_matches(&world, &query).matched_requirements, 3);
}

#[test]
fn alternatives_and_zero_cost_auto_do_not_invent_donor_matches() {
    let query = query(
        "\"auto\"",
        r#"{"any_of":[
        {"item":"wand_frost","blanket":true},
        {"item":"wand_corrosion","blanket":true}
    ]}"#,
    );
    let mut world = world(vec![
        wand(ItemId::WandLightning, 0, 6),
        wand(ItemId::WandCorrosion, 2, 2),
    ]);
    assert_matches(&query, &world, true);
    world.items[0].upgrade = 3;
    assert_matches(&query, &world, false);
    assert_eq!(scout_matches(&world, &query).matched_indices(), vec![0]);
}

#[test]
fn donor_blankets_agree_with_exhaustive_subsets() {
    let mut random = 17_u64;
    for case in 0..512 {
        let query = query(
            if case % 2 == 0 { "6" } else { "\"auto\"" },
            r#"
            {"kind":"wand","upgrade":2,"max_depth":4,"blanket":true},
            {"item":"wand_frost","blanket":true}
        "#,
        );
        let mut items = vec![wand(ItemId::WandLightning, 1, 6)];
        for _ in 0..4 {
            random = random
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1);
            let bits = random.to_le_bytes();
            items.push(WorldItem {
                cursed: bits[0] % 4 == 0,
                accessibility: Accessibility::Scenarios {
                    group: 1,
                    mask: u64::from(1 + bits[1] % 7),
                },
                ..wand(
                    if bits[2] % 2 == 0 {
                        ItemId::WandFrost
                    } else {
                        ItemId::WandCorrosion
                    },
                    bits[3] % 4,
                    if bits[4] % 2 == 0 { 2 } else { 5 },
                )
            });
        }
        let world = world(items);
        // Enumerate every subset directly, independent of the production
        // algorithm's blanket-first search and maximum-yield scenario choice.
        let expected = (0..16).any(|subset| {
            let donors: Vec<_> = world.items[1..]
                .iter()
                .enumerate()
                .filter(|(index, _)| subset & (1 << index) != 0)
                .map(|(_, item)| item)
                .collect();
            if donors.iter().any(|item| item.cursed) {
                return false;
            }
            let mask = donors.iter().fold(u64::MAX, |mask, item| {
                mask & item.accessibility.scenario_constraint().unwrap().1
            });
            let supplied: u16 = donors
                .iter()
                .map(|item| 2 * (u16::from(item.upgrade) + 1))
                .sum();
            mask != 0
                && supplied >= if query.arcane_resin_auto { 5 } else { 6 }
                && query
                    .requirements
                    .iter()
                    .filter(|r| r.blanket)
                    .all(|blanket| {
                        donors.iter().any(|item| {
                            blanket.matches(item)
                                && item.depth <= blanket.max_depth.unwrap_or(query.max_depth)
                        })
                    })
        });
        assert_eq!(
            query.matches(&world),
            expected,
            "case {case}: {:?}",
            world.items
        );
        let marks = scout_matches(&world, &query);
        assert_eq!(
            marks.matched_requirements == marks.total_requirements,
            expected,
            "Scout case {case}"
        );
    }
}
