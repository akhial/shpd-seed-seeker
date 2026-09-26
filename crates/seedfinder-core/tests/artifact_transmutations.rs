use serde_json::{Value, json};
use shpd_seedfinder_core::{
    artifacts::{ArtifactDeck, deck_at},
    catalog::{ItemId, ItemKind, item},
    deep_link,
    feasibility::QueryPlan,
    json_query,
    main_world::CanonicalMainWorldGenerator,
    model::{Accessibility, GeneratedWorld, ItemSource, WorldItem},
    query::{SearchQuery, scout_matches},
    search::WorldGenerator,
    seed::DungeonSeed,
};

#[allow(clippy::needless_pass_by_value)] // JSON literals keep query cases readable.
fn query(requirements: Value, depth: u8) -> SearchQuery {
    json_query::decode(
        &json!({"requirements": requirements, "max_depth": depth, "auto_apply_trinket": false})
            .to_string(),
    )
    .unwrap()
}
fn requirement(id: ItemId, count: usize) -> Value {
    json!({"item": item(id).stable_id, "artifact_transmutations": count})
}
fn fixture() -> GeneratedWorld {
    let mut world = CanonicalMainWorldGenerator.generate(DungeonSeed::MIN, 1);
    world.items = vec![WorldItem {
        item: ItemId::DriedRose,
        upgrade: 0,
        effect: None,
        cursed: false,
        depth: 1,
        source: ItemSource::Chest,
        accessibility: Accessibility::Independent,
        secret: false,
    }];
    world.artifact_decks = vec![
        ArtifactDeck {
            depth: 1,
            order: vec![ItemId::EtherealChains, ItemId::SandalsOfNature],
        },
        ArtifactDeck {
            depth: 4,
            order: vec![ItemId::SandalsOfNature],
        },
        ArtifactDeck {
            depth: 9,
            order: vec![],
        },
    ];
    world
}

#[test]
fn remaining_decks_match_official_v4_oracle() {
    // ArtifactOracle, official JAR pinned by tooling/oracle-4.0/build.sh.
    let mut worlds = std::collections::BTreeMap::new();
    for line in include_str!("fixtures/v4-artifact-decks.txt").lines() {
        let parts: Vec<_> = line.split('|').collect();
        let world = worlds.entry(parts[0]).or_insert_with(|| {
            CanonicalMainWorldGenerator.generate(DungeonSeed::from_code(parts[0]).unwrap(), 24)
        });
        let depth = parts[1].parse().unwrap();
        let actual = deck_at(world, depth)
            .iter()
            .map(|identity| format!("{identity:?}"))
            .collect::<Vec<_>>()
            .join(",");
        assert_eq!(actual, parts[2], "{} floor {depth}", parts[0]);
    }
}

#[test]
fn natural_finds_and_each_allowed_prefix_match_without_changing_world() {
    let generator = CanonicalMainWorldGenerator;
    // 4 worlds × at most 11 identities × 11 limits = at most 484 cases.
    for value in [0, 1, 42, 700_709_647_142] {
        let seed = DungeonSeed::new(value).unwrap();
        let world = generator.generate(seed, 9);
        let before = world.clone();
        let donors = world
            .items
            .iter()
            .filter(|entry| item(entry.item).kind == ItemKind::Artifact)
            .count();
        for (position, &id) in deck_at(&world, 9).iter().enumerate() {
            for count in 0..=10 {
                let query = query(json!([requirement(id, count)]), 9);
                let expected = donors > 0 && position < count;
                assert_eq!(
                    query.matches(&world),
                    expected,
                    "seed {value}, position {position}, count {count}"
                );
                let marks = scout_matches(&world, &query);
                assert_eq!(marks.matched_requirements, usize::from(expected));
                assert_eq!(
                    marks.transmuted_artifacts,
                    if expected {
                        vec![(9, position)]
                    } else {
                        vec![]
                    }
                );
                assert_eq!(marks.matched_indices().len(), usize::from(expected));
                if count == 1 || count == 10 {
                    let plan = QueryPlan::analyze(&query);
                    assert_eq!(plan.generation_depth(), 9);
                    let gated =
                        generator.generate_batch_gated(&[seed], plan.generation_depth(), &plan);
                    assert_eq!(
                        gated[0].as_ref().is_some_and(|world| query.matches(world)),
                        expected
                    );
                }
            }
        }
        for donor in world
            .items
            .iter()
            .filter(|entry| item(entry.item).kind == ItemKind::Artifact)
        {
            assert!(query(json!([requirement(donor.item, 10)]), 9).matches(&world));
        }
        assert_eq!(world, before);
    }
}

#[test]
fn floor_boundary_no_donor_exhaustion_and_filters() {
    let mut world = fixture();
    assert!(query(json!([requirement(ItemId::EtherealChains, 1)]), 1).matches(&world));
    assert!(!query(json!([requirement(ItemId::EtherealChains, 1)]), 4).matches(&world));
    assert!(
        query(
            json!([{"item":"ethereal_chains","artifact_transmutations":1,"max_depth":1}]),
            9
        )
        .matches(&world)
    );
    assert!(!query(json!([requirement(ItemId::SandalsOfNature, 10)]), 9).matches(&world));
    assert!(
        !query(
            json!([{"item":"ethereal_chains","artifact_transmutations":1,"source":"shop"}]),
            1
        )
        .matches(&world)
    );
    world.items[0].cursed = true;
    assert!(
        !query(
            json!([{"item":"ethereal_chains","artifact_transmutations":1,"uncursed":true}]),
            1
        )
        .matches(&world)
    );
    world.items.clear();
    assert!(!query(json!([requirement(ItemId::EtherealChains, 1)]), 1).matches(&world));
}

#[test]
fn donors_identities_accessibility_and_blankets_are_not_reused() {
    let mut world = fixture();
    let chains = requirement(ItemId::EtherealChains, 1);
    let sandals = requirement(ItemId::SandalsOfNature, 2);
    assert!(!query(json!([chains, sandals]), 1).matches(&world));
    assert!(!query(json!([chains, {"item":"dried_rose"}]), 1).matches(&world));
    let mut second = world.items[0].clone();
    second.item = ItemId::HornOfPlenty;
    world.items.push(second);
    assert!(query(json!([chains, sandals]), 1).matches(&world));
    assert!(
        !query(
            json!([
                {"item":"ethereal_chains","artifact_transmutations":1,"max_depth":1},
                {"item":"sandals_of_nature","artifact_transmutations":1,"max_depth":4}
            ]),
            4
        )
        .matches(&world)
    );
    assert!(!query(json!([chains, chains]), 1).matches(&world));
    let blanket = json!({"item":"ethereal_chains","artifact_transmutations":1,"blanket":true});
    assert!(query(json!([chains, blanket]), 1).matches(&world));
    assert!(!query(json!([chains, {"item":"dried_rose","blanket":true}]), 1).matches(&world));
    for (index, donor) in world.items.iter_mut().enumerate() {
        donor.accessibility = Accessibility::Choice {
            group: 1,
            option: u8::try_from(index).unwrap(),
        };
    }
    assert!(!query(json!([chains, sandals]), 1).matches(&world));
    assert!(query(json!([{"any_of":[chains,sandals]}]), 1).matches(&world));
}

#[test]
fn json_links_and_scout_packets_round_trip_and_reject_invalid_counts() {
    for count in [1, 10] {
        let query = query(json!([requirement(ItemId::EtherealChains, count)]), 9);
        assert_eq!(
            deep_link::decode(&deep_link::encode(&query).unwrap()).unwrap(),
            query
        );
        assert_eq!(
            json_query::decode(&json_query::encode(&query).to_string()).unwrap(),
            query
        );
        assert!(shpd_seedfinder_core::probability::estimate_match_probability(&query).is_nan());
    }
    for count in [json!(-1), json!(11), json!(1.5), json!("1"), json!(null)] {
        assert!(json_query::decode(&json!({"requirements":[{"item":"ethereal_chains","artifact_transmutations":count}]}).to_string()).is_err());
    }
    for id in ["rat_skull", "runic_blade"] {
        assert!(
            json_query::decode(
                &json!({"requirements":[{"item":id,"artifact_transmutations":1}]}).to_string()
            )
            .is_err()
        );
    }
    let world = CanonicalMainWorldGenerator.generate(DungeonSeed::MIN, 24);
    let packet =
        shpd_seedfinder_core::wire::encode_scout_world_with_artifacts(&world, None).unwrap();
    assert_eq!(&packet[..4], b"SSC9");
    assert_eq!(
        shpd_seedfinder_core::wire::decode_scout_world(&packet).unwrap(),
        world
    );
    assert!(shpd_seedfinder_core::wire::decode_scout_world(&packet[..packet.len() - 1]).is_err());
}
