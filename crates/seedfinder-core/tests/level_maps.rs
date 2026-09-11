use serde_json::{Value, json};
use shpd_seedfinder_core::catalog::ItemId;
use shpd_seedfinder_core::challenges::Challenges;
use shpd_seedfinder_core::level_map::json::{decode_request, document};
use shpd_seedfinder_core::level_map::{
    LevelMap, MapDraw, MapError, MapKind, SUPPORTED_DEPTHS, assets, generate_level_map,
    generate_level_map_in_branch,
};
use shpd_seedfinder_core::seed::DungeonSeed;

#[test]
fn completed_map_matches_saved_game_fixtures_in_every_region() {
    let first = generate_level_map(DungeonSeed::MIN, 1, Challenges::NONE, None).unwrap();
    assert_eq!(
        (first.width, first.height, first.entrance, first.exit),
        (37, 43, Some(278), Some(974))
    );
    assert_eq!(
        first
            .terrain
            .iter()
            .fold(1_i32, |h, &v| h.wrapping_mul(31).wrapping_add(v)),
        -72_472_821
    );
    for fixture in [
        include_str!("../../../tooling/oracle-4.0/tests/prison-floors.expected.json"),
        include_str!("../../../tooling/oracle-4.0/tests/caves-floors.expected.json"),
        include_str!("../../../tooling/oracle-4.0/tests/city-floors.expected.json"),
        include_str!("../../../tooling/oracle-4.0/tests/halls-floors.expected.json"),
    ] {
        let fixture: Value = serde_json::from_str(fixture).unwrap();
        for level in fixture["seeds"]["AAA-AAA-AAA"]["levels"]
            .as_array()
            .unwrap()
        {
            let depth = u8::try_from(level["depth"].as_u64().unwrap()).unwrap();
            let map = generate_level_map(DungeonSeed::MIN, depth, Challenges::NONE, None).unwrap();
            assert_eq!(
                json!([map.width, map.height]),
                level["size"],
                "depth {depth}"
            );
            let hash = map
                .terrain
                .iter()
                .fold(1_i32, |h, &v| h.wrapping_mul(31).wrapping_add(v));
            assert_eq!(json!(hash), level["map_hash"], "depth {depth}");
            assert_eq!(json!(map.entrance), level["entrance"]);
            assert_eq!(json!(map.exit), level["exit"]);
        }
    }
}

#[test]
fn selection_matches_scout_query_and_explicit_overrides() {
    let mut request = json!({"seed":"AAA-AAA-AAA", "depth":4,
        "query":{"requirements":[{"item":"mimic_tooth","select_trinket":true}]}});
    assert_eq!(
        decode_request(&request.to_string())
            .unwrap()
            .selected_trinket,
        Some(ItemId::MimicTooth)
    );
    request["trinket"] = "none".into();
    assert_eq!(
        decode_request(&request.to_string())
            .unwrap()
            .selected_trinket,
        None
    );
    request["trinket"] = "parchment_scrap".into();
    assert_eq!(
        decode_request(&request.to_string())
            .unwrap()
            .selected_trinket,
        Some(ItemId::ParchmentScrap)
    );
    request["trinket"] = "rat_skull".into();
    assert!(decode_request(&request.to_string()).is_err());
}

#[test]
fn selected_trinket_changes_later_maps_after_brewing_and_is_repeatable() {
    // Find an initial Mossy Clump offer; offer generation doesn't build floors.
    let seed = (0..1000)
        .map(|v| DungeonSeed::new(v).unwrap())
        .find(|&seed| {
            shpd_seedfinder_core::trinkets::trinket_order(seed)[..4].contains(&ItemId::MossyClump)
        })
        .unwrap();
    let initial = generate_level_map(seed, 1, Challenges::NONE, None).unwrap();
    let selected = generate_level_map(seed, 1, Challenges::NONE, Some(ItemId::MossyClump)).unwrap();
    assert_eq!(
        initial.terrain, selected.terrain,
        "selection must not activate before brewing"
    );
    let initial = generate_level_map(seed, 9, Challenges::NONE, None).unwrap();
    let selected = generate_level_map(seed, 9, Challenges::NONE, Some(ItemId::MossyClump)).unwrap();
    assert_ne!(initial.terrain, selected.terrain);
    assert_eq!(
        selected,
        generate_level_map(seed, 9, Challenges::NONE, Some(ItemId::MossyClump)).unwrap()
    );
    assert_eq!(
        initial,
        generate_level_map(seed, 9, Challenges::NONE, None).unwrap()
    );
}

#[test]
fn all_supported_depths_have_bounded_draws_and_explicit_metadata() {
    for depth in SUPPORTED_DEPTHS {
        let map = generate_level_map(DungeonSeed::MIN, depth, Challenges::NONE, None).unwrap();
        let doc = document(&map);
        assert_eq!(doc["schemaVersion"], 2);
        assert_eq!(doc["selectedTrinket"], Value::Null);
        assert_eq!(doc["shpdVersion"], shpd_seedfinder_core::SHPD_VERSION);
        assert_eq!(
            map.terrain.len(),
            usize::try_from(map.width * map.height).unwrap()
        );
        assert_drawing_bounds(&map);
    }
}

#[test]
fn malformed_and_unsupported_requests_are_rejected() {
    for input in [
        "{}",
        "[]",
        "not json",
        r#"{"seed":"AAA","depth":1}"#,
        r#"{"seed":"AAA-AAA-AAA","depth":1,"branch":1}"#,
        r#"{"seed":"AAA-AAA-AAA","depth":1,"challenges":["bad"]}"#,
        r#"{"seed":"AAA-AAA-AAA","depth":1,"trinket":"longsword"}"#,
    ] {
        assert!(decode_request(input).is_err(), "{input}");
    }
    for depth in [0, 5, 10, 15, 20, 25, 255] {
        assert_eq!(
            generate_level_map(DungeonSeed::MIN, depth, Challenges::NONE, None),
            Err(MapError::UnsupportedDepth(depth))
        );
        assert!(decode_request(&json!({"seed":"AAA-AAA-AAA","depth":depth}).to_string()).is_err());
    }
    assert!(assets::get("../tiles_sewers.png").is_none());
}

#[test]
fn challenge_json_uses_the_game_fixture_profile() {
    let fixture: Value = serde_json::from_str(include_str!(
        "../../../tooling/oracle-4.0/tests/challenges.expected.json"
    ))
    .unwrap();
    for code in ["AAA-AAA-AAA", "AAA-AAA-AAF"] {
        let request = decode_request(
            &json!({
                "seed": code, "depth": 14,
                "challenges": ["barren_land", "into_darkness", "forbidden_runes"]
            })
            .to_string(),
        )
        .unwrap();
        assert_eq!(request.challenges.bits(), 104);
        let map: Value = serde_json::from_str(&request.generate_document().unwrap()).unwrap();
        let hash = map["terrain"]
            .as_array()
            .unwrap()
            .iter()
            .fold(1_i32, |h, v| {
                h.wrapping_mul(31)
                    .wrapping_add(i32::try_from(v.as_i64().unwrap()).unwrap())
            });
        let run = &fixture["runs"][format!("{code}/104")];
        let level = run["levels"]
            .as_array()
            .unwrap()
            .iter()
            .find(|level| level[0] == 14)
            .unwrap();
        assert_eq!(json!(hash), level[1], "{code}");
        assert_eq!(
            map["challenges"],
            json!(["barren_land", "into_darkness", "forbidden_runes"])
        );
    }
}

fn assert_drawing_bounds(map: &LevelMap) {
    for layer in map.scene.layers.iter().chain(&map.scene.concealed_layers) {
        assert_eq!(layer.cells.len(), map.terrain.len());
        assert!(
            layer
                .cells
                .iter()
                .flatten()
                .all(|&index| index < map.scene.sprites.len())
        );
    }
    for sprite in &map.scene.sprites {
        assert!(!sprite.frames.is_empty());
        assert_ne!(sprite.frame_duration_ms, 0);
        for frame in &sprite.frames {
            for draw in frame {
                let destination = match draw {
                    MapDraw::Blit {
                        asset,
                        source: [x, y, w, h],
                        destination,
                    } => {
                        let asset = assets::get(asset).expect("every sprite ships its texture");
                        assert!(*w > 0 && *h > 0 && x + w <= asset.width && y + h <= asset.height);
                        destination
                    }
                    MapDraw::Fill { destination, .. } => destination,
                };
                assert!(destination[2] > 0 && destination[3] > 0);
                assert!(
                    destination[0] + destination[2] <= 16 && destination[1] + destination[3] <= 16
                );
            }
        }
    }
}

#[test]
fn vault_maps_match_saved_java_fixtures_at_every_imp_depth() {
    let fixture: Value = serde_json::from_str(include_str!(
        "../../../tooling/oracle-4.0/tests/vault.expected.json"
    ))
    .unwrap();
    for (code, row) in fixture["seeds"].as_object().unwrap() {
        let seed = DungeonSeed::from_code(code).unwrap();
        let depth = u8::try_from(row["imp_depth"].as_u64().unwrap()).unwrap();
        let request =
            decode_request(&json!({"seed":code,"depth":depth,"branch":1}).to_string()).unwrap();
        let map = generate_level_map_in_branch(seed, depth, 1, Challenges::NONE, None).unwrap();
        assert_eq!(map.kind, MapKind::ImpVault);
        assert_eq!(json!([map.width, map.height]), row["vault"]["size"]);
        assert_eq!(json!(map.entrance), row["vault"]["entrance"]);
        assert_eq!(map.exit, None);
        let hash = map
            .terrain
            .iter()
            .fold(1_i32, |h, &v| h.wrapping_mul(31).wrapping_add(v));
        assert_eq!(json!(hash), row["vault"]["map_hash"]);
        assert!(
            map.traps
                .iter()
                .any(|t| t.kind == shpd_seedfinder_core::level::TrapKind::VaultFlame)
        );
        assert_eq!(
            serde_json::from_str::<Value>(&request.generate_document().unwrap()).unwrap(),
            document(&map)
        );
        assert_drawing_bounds(&map);
        let parent = generate_level_map(seed, depth, Challenges::NONE, None).unwrap();
        assert_eq!(parent.branches.len(), 1);
        assert_eq!(parent.branches[0].kind, map.kind);
        assert_eq!(
            parent.terrain[parent.branches[0].entrance],
            shpd_seedfinder_core::geometry::terrain::EXIT
        );
    }
}

#[test]
fn quest_branches_follow_the_selected_scout_run_and_do_not_change_main_generation() {
    use shpd_seedfinder_core::main_world::generate_main_world_with_trinket;
    use shpd_seedfinder_core::quests::BlacksmithQuestType;
    let mut kinds = Vec::new();
    for (value, selected) in [
        (0, None),
        (1, None),
        (0, Some(ItemId::MimicTooth)),
        (0, Some(ItemId::ParchmentScrap)),
    ] {
        let seed = DungeonSeed::new(value).unwrap();
        let world = generate_main_world_with_trinket(seed, 19, Challenges::NONE, selected).unwrap();
        let quest = world.quests.blacksmith.unwrap();
        let kind = match quest.variant {
            BlacksmithQuestType::Crystal => MapKind::BlacksmithCrystal,
            BlacksmithQuestType::Gnoll => MapKind::BlacksmithGnoll,
        };
        kinds.push(kind);
        let parent = generate_level_map(seed, quest.depth, Challenges::NONE, selected).unwrap();
        assert_eq!(parent.branches.len(), 1);
        let link = &parent.branches[0];
        assert_eq!((link.depth, link.branch, link.kind), (quest.depth, 1, kind));
        let mine =
            generate_level_map_in_branch(seed, link.depth, link.branch, Challenges::NONE, selected)
                .unwrap();
        assert_eq!(mine.kind, kind);
        assert_eq!(mine.selected_trinket, selected);
        assert!(mine.entrance.is_some());
        assert!(mine.exit.is_none());
        assert_eq!(mine.secret_rooms.len(), 2);
        assert_drawing_bounds(&mine);
        let doc = document(&mine);
        assert_eq!(doc["branch"], 1);
        assert!(
            doc["assets"]
                .as_array()
                .unwrap()
                .iter()
                .any(|a| a["id"] == "caves_quest.png")
        );
        assert_eq!(
            mine,
            generate_level_map_in_branch(seed, quest.depth, 1, Challenges::NONE, selected).unwrap()
        );
        for depth in [12, 13, 14, 17, 18, 19] {
            if depth != quest.depth && depth != world.quests.imp.unwrap().depth {
                assert_eq!(
                    generate_level_map_in_branch(seed, depth, 1, Challenges::NONE, selected),
                    Err(MapError::MissingQuestBranch)
                );
            }
        }
        assert_eq!(
            world,
            generate_main_world_with_trinket(seed, 19, Challenges::NONE, selected).unwrap()
        );
    }
    assert!(kinds.contains(&MapKind::BlacksmithCrystal));
    assert!(kinds.contains(&MapKind::BlacksmithGnoll));
}
