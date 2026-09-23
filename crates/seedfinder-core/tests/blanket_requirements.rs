//! Blanket predicates share the ordinary assignment, including accessibility.
use shpd_seedfinder_core::{
    catalog::ItemId,
    deep_link,
    feasibility::QueryPlan,
    json_query,
    model::{Accessibility, GeneratedWorld, ItemSource, WorldItem},
    probability::estimate_match_probability,
    query::{SearchQuery, scout_matches},
    quests::QuestSummary,
    results_export,
    run::RingGems,
    seed::DungeonSeed,
};

fn parse_query(requirements: &str) -> SearchQuery {
    json_query::decode(&format!(
        r#"{{"max_depth":9,"requirements":[{requirements}]}}"#
    ))
    .unwrap()
}

const WANDS: &str = r#"
    {"item":"wand_lightning","upgrade":{"at_least":2}},
    {"item":"wand_disintegration","upgrade":{"at_least":2}},
    {"item":"wand_frost","upgrade":{"at_least":2}}
"#;
const BLANKET: &str = r#"{"kind":"wand","upgrade":3,"source":"wandmaker_reward","blanket":true}"#;

fn wand(item: ItemId, upgrade: u8, source: ItemSource) -> WorldItem {
    WorldItem {
        item,
        upgrade,
        source,
        depth: 7,
        effect: None,
        cursed: false,
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
        feelings: Vec::new(),
    }
}

#[test]
fn blanket_matches_each_of_the_three_wands_without_a_fourth_item() {
    let query = parse_query(&format!("{WANDS},{BLANKET}"));
    let ids = [
        ItemId::WandLightning,
        ItemId::WandDisintegration,
        ItemId::WandFrost,
    ];
    for chosen in 0..3 {
        let world = world(
            ids.iter()
                .enumerate()
                .map(|(index, &id)| {
                    wand(
                        id,
                        if index == chosen { 3 } else { 2 },
                        if index == chosen {
                            ItemSource::WandmakerReward
                        } else {
                            ItemSource::Heap
                        },
                    )
                })
                .collect(),
        );
        assert!(query.matches(&world));
        let plan = QueryPlan::analyze(&query);
        assert!(!plan.is_unsatisfiable());
        assert!(plan.viable_after_floor(9, &world.items, &world.quests));
        let marks = scout_matches(&world, &query);
        assert_eq!(marks.matched_requirements, 4);
        assert_eq!(marks.total_requirements, 4);
        assert_eq!(marks.matched_indices(), vec![0, 1, 2]);
    }
}

#[test]
fn an_unrelated_or_unassigned_wand_cannot_witness_the_blanket() {
    let query = parse_query(&format!("{WANDS},{BLANKET}"));
    let mut world = world(vec![
        wand(ItemId::WandLightning, 2, ItemSource::Heap),
        wand(ItemId::WandDisintegration, 2, ItemSource::Heap),
        wand(ItemId::WandFrost, 2, ItemSource::Heap),
        wand(ItemId::WandFireblast, 3, ItemSource::WandmakerReward),
    ]);
    assert!(!query.matches(&world));
    assert_eq!(scout_matches(&world, &query).matched_requirements, 3);
    world.items[3].item = ItemId::WandLightning;
    assert!(query.matches(&world));
    assert_eq!(
        scout_matches(&world, &query).matched_indices(),
        vec![1, 2, 3]
    );
    world.items[3].source = ItemSource::Heap;
    assert!(!query.matches(&world));
}

#[test]
fn blankets_share_items_but_not_mutually_exclusive_choices() {
    let query = parse_query(
        r#"
        {"kind":"wand"},
        {"kind":"wand","upgrade":3,"blanket":true},
        {"kind":"wand","source":"wandmaker_reward","blanket":true}
    "#,
    );
    assert!(query.matches(&world(vec![wand(
        ItemId::WandFrost,
        3,
        ItemSource::WandmakerReward
    )])));
    // Both blankets must be witnessed by the single ordinary assignment.
    assert!(!query.matches(&world(vec![
        wand(ItemId::WandFrost, 3, ItemSource::Heap),
        wand(ItemId::WandLightning, 2, ItemSource::WandmakerReward),
    ])));
    let mut exclusive = world(vec![
        wand(ItemId::WandFrost, 3, ItemSource::WandmakerReward),
        wand(ItemId::WandLightning, 2, ItemSource::WandmakerReward),
    ]);
    exclusive.items[0].accessibility = Accessibility::Choice {
        group: 1,
        option: 0,
    };
    exclusive.items[1].accessibility = Accessibility::Choice {
        group: 1,
        option: 1,
    };
    let query = parse_query(
        r#"{"item":"wand_frost"},{"item":"wand_lightning"},{"kind":"wand","upgrade":3,"blanket":true}"#,
    );
    assert!(!query.matches(&exclusive));
}

#[test]
fn blankets_obey_floor_limits_and_do_not_consume_quest_capacity() {
    let mut query = parse_query(
        r#"{"kind":"wand","source":"wandmaker_reward"},{"kind":"wand","source":"wandmaker_reward","upgrade":3,"blanket":true}"#,
    );
    let world = world(vec![wand(
        ItemId::WandFrost,
        3,
        ItemSource::WandmakerReward,
    )]);
    let plan = QueryPlan::analyze(&query);
    assert!(!plan.is_unsatisfiable());
    assert!(plan.viable_after_floor(9, &world.items, &world.quests));
    assert!(query.matches(&world));
    query.requirements[1].max_depth = Some(6);
    assert!(!query.matches(&world));
    assert!(QueryPlan::analyze(&query).is_unsatisfiable());
}

#[test]
fn exact_upgrade_blanket_conflicts_are_impossible() {
    let mut query = parse_query(
        r#"{"item":"wand_lightning","upgrade":2},
           {"item":"wand_disintegration","upgrade":2},
           {"kind":"wand","upgrade":3,"blanket":true}"#,
    );
    for depth in [9, 24] {
        query.max_depth = depth;
        assert!(QueryPlan::analyze(&query).is_unsatisfiable());
    }
    query.requirements[0].upgrade = shpd_seedfinder_core::query::UpgradeRequirement::AtLeast(2);
    assert!(!QueryPlan::analyze(&query).is_unsatisfiable());
}

#[test]
fn blankets_need_a_reachable_item_satisfying_all_filters() {
    for requirements in [
        r#"{"item":"wand_frost"},{"item":"wand_lightning","blanket":true}"#,
        r#"{"kind":"ring"},{"kind":"wand","blanket":true}"#,
        r#"{"kind":"wand","upgrade":{"at_least":3}},{"kind":"wand","upgrade":2,"blanket":true}"#,
        r#"{"kind":"wand","upgrade":2},{"kind":"wand","upgrade":{"at_least":3},"blanket":true}"#,
        r#"{"kind":"wand","source":"chest"},{"kind":"wand","source":"wandmaker_reward","blanket":true}"#,
        r#"{"kind":"wand","max_depth":6},{"kind":"wand","upgrade":3,"blanket":true}"#,
        r#"{"kind":"wand","source":"chest"},{"kind":"wand","upgrade":3,"blanket":true}"#,
        r#"{"kind":"weapon","tier":{"exact":2}},{"kind":"weapon","tier":{"exact":3},"blanket":true}"#,
        r#"{"item":"sword"},{"kind":"thrown_weapon","blanket":true}"#,
        r#"{"kind":"weapon","effect":"Blazing"},{"kind":"weapon","effect":"Chilling","blanket":true}"#,
        r#"{"kind":"weapon","effect":"Annoying"},{"kind":"weapon","uncursed":true,"blanket":true}"#,
        r#"{"item":"wand_frost","upgrade":2},{"item":"wand_lightning","upgrade":3},{"item":"wand_frost","upgrade":3,"blanket":true}"#,
        r#"{"item":"mimic_tooth"},{"item":"rat_skull","blanket":true}"#,
    ] {
        assert!(
            QueryPlan::analyze(&parse_query(requirements)).is_unsatisfiable(),
            "{requirements}"
        );
    }
}

#[test]
fn blanket_witnesses_preserve_alternatives_optional_members_and_reuse() {
    for requirements in [
        r#"{"any_of":[{"item":"wand_frost","upgrade":2},{"item":"wand_lightning","upgrade":3}]},{"kind":"wand","upgrade":3,"blanket":true}"#,
        r#"{"item":"wand_frost","upgrade":2},{"any_of":[{"kind":"wand","upgrade":3,"blanket":true},{"kind":"wand","upgrade":2,"blanket":true}]}"#,
        r#"{"item":"wand_frost","upgrade":3},{"kind":"wand","upgrade":3,"blanket":true},{"kind":"wand","source":"wandmaker_reward","blanket":true}"#,
        r#"{"item":"wand_frost","upgrade":2},{"item":"wand_lightning","upgrade":3},{"kind":"wand","upgrade":2,"blanket":true},{"kind":"wand","upgrade":3,"blanket":true}"#,
        r#"{"kind":"ring","level_sum":{"group":1,"at_least":1}},{"kind":"ring","level_sum":{"group":1,"at_least":1}},{"item":"ring_haste","upgrade":2,"blanket":true}"#,
        r#"{"item":"mimic_tooth"},{"item":"mimic_tooth","blanket":true}"#,
        r#"{"kind":"weapon","tier":{"at_least":3}},{"kind":"weapon","tier":{"at_most":4},"blanket":true}"#,
        r#"{"kind":"weapon","effect":["Blazing","Chilling"]},{"kind":"weapon","effect":["Chilling","Lucky"],"blanket":true}"#,
    ] {
        assert!(
            !QueryPlan::analyze(&parse_query(requirements)).is_unsatisfiable(),
            "{requirements}"
        );
    }
}

#[test]
fn blanket_alternatives_and_documents_round_trip() {
    let query = parse_query(
        r#"
        {"kind":"wand"},
        {"any_of":[
            {"item":"wand_frost","upgrade":3,"blanket":true},
            {"item":"wand_lightning","source":"wandmaker_reward","blanket":true}
        ]}
    "#,
    );
    assert!(query.matches(&world(vec![wand(ItemId::WandFrost, 3, ItemSource::Heap)])));
    assert!(query.matches(&world(vec![wand(
        ItemId::WandLightning,
        2,
        ItemSource::WandmakerReward
    )])));
    assert_eq!(
        json_query::decode(&json_query::encode(&query).to_string()).unwrap(),
        query
    );
    assert_eq!(
        deep_link::decode(&deep_link::encode(&query).unwrap()).unwrap(),
        query
    );
    assert_eq!(
        results_export::decode(&results_export::encode(&query, &[], "test"))
            .unwrap()
            .query,
        query
    );
}

#[test]
fn blankets_can_use_either_reserved_wands_or_resin_donors() {
    let mut query =
        parse_query(r#"{"item":"wand_frost"},{"kind":"wand","upgrade":3,"blanket":true}"#);
    query.arcane_resin = 4;
    let mut world = world(vec![
        wand(ItemId::WandFrost, 2, ItemSource::Heap),
        wand(ItemId::WandLightning, 3, ItemSource::WandmakerReward),
    ]);
    // The +3 donor both supplies resin and witnesses the blanket.
    assert!(query.matches(&world));
    let marks = scout_matches(&world, &query);
    assert_eq!(marks.matched_requirements, 3);
    assert_eq!(marks.total_requirements, 3);
    world.items[0].upgrade = 3;
    world.items[0].source = ItemSource::WandmakerReward;
    world.items[1].upgrade = 1;
    world.items[1].source = ItemSource::Heap;
    assert!(query.matches(&world));
    let marks = scout_matches(&world, &query);
    assert_eq!(marks.matched_requirements, 3);
    assert_eq!(marks.matched_indices(), vec![0, 1]);
    world.items.pop();
    assert!(!query.matches(&world));
}

#[test]
fn version_nine_preserves_blankets_and_resin_without_changing_older_formats() {
    let mut query = parse_query(&format!("{WANDS},{BLANKET}"));
    for resin in [0, 8] {
        query.arcane_resin = resin;
        for uncursed in [true, false] {
            query.arcane_resin_filter.uncursed = uncursed;
            query.arcane_resin_filter.source = Some(ItemSource::Chest);
            query.arcane_resin_filter.max_depth = Some(4);
            let code = deep_link::encode(&query).unwrap();
            assert!(
                code.starts_with('k'),
                "version 9 starts with the 100100 bits"
            );
            assert_eq!(deep_link::decode(&code).unwrap(), query);
            assert_eq!(
                json_query::decode(&json_query::encode(&query).to_string()).unwrap(),
                query
            );
        }
    }
    assert!(json_query::decode(r#"{"arcane_resin":4,"requirements":[]}"#).is_ok());
    assert!(
        json_query::decode(r#"{"arcane_resin":4,"requirements":[{"kind":"wand","blanket":true}]}"#)
            .is_err()
    );
}

#[test]
fn probability_does_not_report_donor_blankets_as_impossible() {
    let mut query = parse_query(
        r#"{"item":"wand_frost","upgrade":2},{"kind":"wand","upgrade":3,"blanket":true}"#,
    );
    query.arcane_resin = 8;
    assert!(estimate_match_probability(&query) > 0.0);
    query.requirements[0].upgrade = shpd_seedfinder_core::query::UpgradeRequirement::AtLeast(2);
    let combined = estimate_match_probability(&query);
    assert!(combined > 0.0);
    query.arcane_resin = 0;
    assert!(estimate_match_probability(&query) > 0.0);
    query.arcane_resin = 8;
    query.arcane_resin_filter.source = Some(ItemSource::Heap);
    query.requirements[1].source = Some(ItemSource::WandmakerReward);
    assert!(estimate_match_probability(&query) > 0.0);
}

#[test]
fn invalid_blanket_structures_are_rejected() {
    for requirements in [
        r#"{"kind":"wand","blanket":true}"#,
        r#"{"kind":"wand"},{"kind":"wand","blanket":"yes"}"#,
        r#"{"kind":"wand"},{"kind":"wand","blanket":true,"identity_group":1}"#,
        r#"{"kind":"ring"},{"kind":"ring","blanket":true,"level_sum":{"group":1,"at_least":1}}"#,
        r#"{"item":"mimic_tooth"},{"item":"mimic_tooth","blanket":true,"select_trinket":true}"#,
        r#"{"any_of":[{"kind":"wand"},{"kind":"wand","blanket":true}]}"#,
    ] {
        assert!(json_query::decode(&format!(r#"{{"requirements":[{requirements}]}}"#)).is_err());
    }
}

#[test]
fn probability_reuses_supply_and_recognizes_redundancy() {
    let base = parse_query(r#"{"item":"wand_frost"}"#);
    let redundant = parse_query(r#"{"item":"wand_frost"},{"kind":"wand","blanket":true}"#);
    assert!(
        (estimate_match_probability(&base) - estimate_match_probability(&redundant)).abs() < 1e-12
    );
    let direct = parse_query(r#"{"item":"wand_frost","upgrade":3,"source":"wandmaker_reward"}"#);
    let blanket = parse_query(&format!(r#"{{"item":"wand_frost"}},{BLANKET}"#));
    assert!(
        (estimate_match_probability(&direct) - estimate_match_probability(&blanket)).abs() < 1e-12
    );
    let impossible =
        parse_query(r#"{"item":"wand_frost"},{"item":"wand_lightning","blanket":true}"#);
    assert!(estimate_match_probability(&impossible).abs() < f64::EPSILON);
    let base = estimate_match_probability(&parse_query(WANDS));
    let narrowed = estimate_match_probability(&parse_query(&format!("{WANDS},{BLANKET}")));
    assert!(narrowed > 0.0 && narrowed < base, "{narrowed} / {base}");
    let duplicated =
        estimate_match_probability(&parse_query(&format!("{WANDS},{BLANKET},{BLANKET}")));
    assert!(
        (duplicated - narrowed).abs() < 1e-12,
        "{duplicated} != {narrowed}"
    );
}

#[test]
fn probability_keeps_ordinary_alternatives_until_blankets_are_applied() {
    for wanted in ["wand_frost", "wand_lightning"] {
        let query = parse_query(&format!(
            r#"{{"any_of":[{{"item":"wand_frost"}},{{"item":"wand_lightning"}}]}},{{"item":"{wanted}","blanket":true}}"#
        ));
        let chance = estimate_match_probability(&query);
        assert!(chance > 0.0 && chance.is_finite());
    }
}

#[test]
fn probability_blankets_reuse_only_assigned_trinket_offers() {
    let base = parse_query(r#"{"item":"mimic_tooth"}"#);
    let repeated = parse_query(r#"{"item":"mimic_tooth"},{"item":"mimic_tooth","blanket":true}"#);
    assert!(
        (estimate_match_probability(&base) - estimate_match_probability(&repeated)).abs() < 1e-12
    );
    let unrelated = parse_query(r#"{"item":"mimic_tooth"},{"item":"rat_skull","blanket":true}"#);
    assert!(estimate_match_probability(&unrelated).abs() < f64::EPSILON);
}
