//! Full terrain captured with tooling/oracle-4.0/run.sh SEED 5,15
//! --no-phases --format json --challenges MASK (pinned official v4.0.0).
use shpd_seedfinder_core::{
    challenges::Challenges, level_map::generate_level_map, seed::DungeonSeed,
};

#[test]
fn boss_maps_match_official_terrain_and_secret_room_locations() {
    let fixtures: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/boss-maps.json")).unwrap();
    for case in fixtures.as_array().unwrap() {
        let code = case["seed"].as_str().unwrap();
        let depth = u8::try_from(case["depth"].as_u64().unwrap()).unwrap();
        let challenges =
            Challenges::new(u16::try_from(case["challenges"].as_u64().unwrap()).unwrap()).unwrap();
        let map = generate_level_map(
            DungeonSeed::from_code(code).unwrap(),
            depth,
            challenges,
            None,
        )
        .unwrap();
        let context = format!("{code} depth {depth} challenges {}", challenges.bits());
        assert_eq!(serde_json::json!(map.terrain), case["terrain"], "{context}");
        assert_eq!(serde_json::json!(map.width), case["width"], "{context}");
        assert_eq!(serde_json::json!(map.height), case["height"], "{context}");
        assert_eq!(
            serde_json::json!(map.entrance),
            case["entrance"],
            "{context}"
        );
        assert_eq!(serde_json::json!(map.exit), case["exit"], "{context}");
        assert_eq!(
            serde_json::json!(map.secret_rooms),
            case["secret_rooms"],
            "{context}"
        );
    }
}

#[test]
fn rat_king_art_is_revealed_only_with_secrets_and_pylons_are_visible() {
    use shpd_seedfinder_core::level_map::{LevelMap, MapDraw, MapLayer};
    fn uses_asset(map: &LevelMap, layers: &[MapLayer], asset: &str) -> bool {
        layers
            .iter()
            .flat_map(|layer| layer.cells.iter().flatten())
            .any(|&index| {
                map.scene.sprites[index]
                    .frames
                    .iter()
                    .flatten()
                    .any(|draw| matches!(draw, MapDraw::Blit { asset: id, .. } if *id == asset))
            })
    }
    let sewer = generate_level_map(DungeonSeed::MIN, 5, Challenges::NONE, None).unwrap();
    for asset in ["rat_king_room.png", "ratking.png", "carpet.png"] {
        assert!(uses_asset(&sewer, &sewer.scene.layers, asset));
        assert!(!uses_asset(&sewer, &sewer.scene.concealed_layers, asset));
    }
    assert_eq!(sewer.secret_rooms.len(), 1);
    let caves = generate_level_map(DungeonSeed::MIN, 15, Challenges::NONE, None).unwrap();
    assert!(uses_asset(
        &caves,
        &caves.scene.concealed_layers,
        "pylon.png"
    ));
    let actors = caves
        .scene
        .layers
        .iter()
        .find(|layer| layer.name == "actors")
        .unwrap();
    for cell in [433, 457, 1225, 1249] {
        assert!(actors.cells[cell].is_some(), "pylon at {cell}");
    }
    let harder =
        generate_level_map(DungeonSeed::MIN, 15, Challenges::STRONGER_BOSSES, None).unwrap();
    assert_ne!(caves.terrain, harder.terrain);
}
