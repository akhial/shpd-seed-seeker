#![cfg(feature = "json-query")]

use serde_json::Value;
use shpd_seedfinder_core::{
    challenges::Challenges,
    level_map::{LevelMap, MapDraw, assets, generate_level_map_in_branch},
    seed::DungeonSeed,
};

fn normalized(value: &str) -> String {
    let value = value
        .rsplit(['.', '$'])
        .next()
        .unwrap()
        .replace('_', "")
        .to_ascii_lowercase();
    if matches!(value.as_str(), "fire" | "frost" | "shock" | "chaos") {
        format!("{value}elemental")
    } else {
        value
    }
}

#[test]
fn initial_contents_match_official_engine_in_every_region() {
    let fixture: Value = serde_json::from_str(include_str!("fixtures/map-contents.json")).unwrap();
    for sample in fixture["samples"].as_array().unwrap() {
        let seed = DungeonSeed::from_code(sample["seed"].as_str().unwrap()).unwrap();
        let depth = u8::try_from(sample["depth"].as_u64().unwrap()).unwrap();
        let branch = u8::try_from(sample["branch"].as_u64().unwrap()).unwrap();
        let challenges =
            Challenges::new(u16::try_from(sample["challenges"].as_u64().unwrap()).unwrap())
                .unwrap();
        let trinket = sample["trinket"].as_str().map(|id| {
            shpd_seedfinder_core::catalog::item_by_stable_id(id)
                .unwrap()
                .id
        });
        let map = generate_level_map_in_branch(seed, depth, branch, challenges, trinket).unwrap();
        let context = format!("{seed} floor {depth} branch {branch}");
        let hash = map.terrain.iter().fold(1_i32, |hash, tile| {
            hash.wrapping_mul(31).wrapping_add(*tile)
        });
        assert_eq!(
            i64::from(hash),
            sample["terrainHash"].as_i64().unwrap(),
            "{context} terrain"
        );
        check_heaps(&map, sample, &context);
        check_mobs(&map, sample, &context);
        check_plants_effects(&map, sample, &context);
        check_draws(&map, &context);
        check_traps_partners(&map, sample, &context);
    }
}

fn check_heaps(map: &LevelMap, sample: &Value, context: &str) {
    let expected = sample["heaps"].as_array().unwrap();
    assert_eq!(
        map.contents.heaps.len(),
        expected.len(),
        "{context} heap count"
    );
    for (heap, expected) in map.contents.heaps.iter().zip(expected) {
        assert_eq!(
            heap.cell as u64,
            expected["cell"].as_u64().unwrap(),
            "{context} heap cell"
        );
        assert_eq!(
            normalized(&heap.kind),
            normalized(expected["kind"].as_str().unwrap()),
            "{context} container"
        );
        assert_eq!(
            heap.items.len(),
            expected["items"].as_array().unwrap().len(),
            "{context} heap {} item count",
            heap.cell
        );
        for (item, expected) in heap.items.iter().zip(expected["items"].as_array().unwrap()) {
            if item.deterministic {
                assert_eq!(
                    u64::from(item.image),
                    expected["image"].as_u64().unwrap(),
                    "{context} heap {} {} image",
                    heap.cell,
                    item.kind
                );
                assert_eq!(
                    i64::from(item.quantity),
                    expected["quantity"].as_i64().unwrap(),
                    "{context} heap {} quantity",
                    heap.cell
                );
            } else {
                assert!(
                    matches!(
                        item.kind.as_str(),
                        "RuntimeShopBag"
                            | "RuntimeVaultConsumable"
                            | "RuntimeLaboratoryPotion"
                            | "RuntimeLibraryScroll"
                    ),
                    "{context}: unclassified runtime item {}",
                    item.kind
                );
            }
        }
    }
}

fn check_mobs(map: &LevelMap, sample: &Value, context: &str) {
    let expected = sample["mobs"].as_array().unwrap();
    assert_eq!(
        map.contents.mobs.iter().filter(|m| !m.approximate).count(),
        expected.len(),
        "{context} actor count"
    );
    for (mob, expected) in map
        .contents
        .mobs
        .iter()
        .filter(|m| !m.approximate)
        .zip(expected)
    {
        assert_eq!(
            mob.cell as u64,
            expected["cell"].as_u64().unwrap(),
            "{context} actor cell"
        );
        assert_eq!(
            mob.sleeping,
            expected["sleeping"].as_bool().unwrap(),
            "{context} {} sleeping",
            mob.kind
        );
        assert_eq!(
            mob.items.len(),
            expected["items"].as_array().unwrap().len(),
            "{context} {} carried items",
            mob.kind
        );
        for (item, expected) in mob.items.iter().zip(expected["items"].as_array().unwrap()) {
            if item.deterministic {
                assert_eq!(
                    u64::from(item.image),
                    expected["image"].as_u64().unwrap(),
                    "{context} {} carried {}",
                    mob.kind,
                    item.kind
                );
                assert_eq!(
                    i64::from(item.quantity),
                    expected["quantity"].as_i64().unwrap(),
                    "{context} {} quantity",
                    mob.kind
                );
            }
        }
        assert_eq!(
            normalized(&mob.kind),
            normalized(expected["kind"].as_str().unwrap()),
            "{context} actor at {}",
            mob.cell
        );
    }
}

fn check_plants_effects(map: &LevelMap, sample: &Value, context: &str) {
    let expected = sample["plants"].as_array().unwrap();
    assert_eq!(
        map.contents.plants.len(),
        expected.len(),
        "{context} plant count"
    );
    for (plant, expected) in map.contents.plants.iter().zip(expected) {
        assert_eq!(
            plant.cell as u64,
            expected["cell"].as_u64().unwrap(),
            "{context} plant cell"
        );
        assert_eq!(
            u64::from(plant.image),
            expected["image"].as_u64().unwrap(),
            "{context} plant image"
        );
    }
    let mut effects: Vec<_> = map
        .contents
        .effects
        .iter()
        .map(|e| {
            (
                e.cell,
                normalized(if e.kind == "WeakFloorWell" {
                    "WellID"
                } else {
                    &e.kind
                }),
            )
        })
        .collect();
    let mut expected: Vec<_> = sample["effects"]
        .as_array()
        .unwrap()
        .iter()
        .map(|e| {
            (
                usize::try_from(e["cell"].as_u64().unwrap()).unwrap(),
                normalized(e["kind"].as_str().unwrap()),
            )
        })
        .collect();
    effects.sort();
    expected.sort();
    assert_eq!(effects, expected, "{context} emitter locations and types");
}

fn check_draws(map: &LevelMap, context: &str) {
    for layer in map.scene.layers.iter().chain(&map.scene.concealed_layers) {
        assert_eq!(layer.cells.len(), map.terrain.len());
        for id in layer.cells.iter().flatten() {
            for draw in map.scene.sprites[*id].frames.iter().flatten() {
                let destination = match draw {
                    MapDraw::Blit {
                        asset,
                        source,
                        destination,
                        ..
                    } => {
                        let asset = assets::get(asset).expect("embedded sprite");
                        assert!(
                            source[0] + source[2] <= asset.width
                                && source[1] + source[3] <= asset.height,
                            "{context} {} {:?}",
                            asset.id,
                            source
                        );
                        destination
                    }
                    MapDraw::Fill { destination, .. } => destination,
                };
                assert!(
                    destination[0] + destination[2] <= 16 && destination[1] + destination[3] <= 16,
                    "{context} clipped raised sprite"
                );
            }
        }
    }
}

fn check_traps_partners(map: &LevelMap, sample: &Value, context: &str) {
    let width = usize::try_from(map.width).unwrap();
    let expected = sample["traps"].as_array().unwrap();
    assert_eq!(map.traps.len(), expected.len(), "{context} trap count");
    for trap in &map.traps {
        let expected = expected
            .iter()
            .find(|t| t["cell"].as_u64() == Some(trap.cell as u64))
            .expect("trap cell");
        assert_eq!(
            trap.active,
            expected["active"].as_bool().unwrap(),
            "{context} trap active"
        );
        assert_eq!(
            trap.hidden,
            expected["hidden"].as_bool().unwrap(),
            "{context} trap hidden"
        );
    }
    for mob in map.contents.mobs.iter().filter(|m| m.approximate) {
        assert_eq!(mob.kind, "Ghoul");
        assert_eq!(
            map.contents
                .mobs
                .iter()
                .filter(|m| m.cell == mob.cell)
                .count(),
            1
        );
        assert!(map.contents.mobs.iter().any(|parent| !parent.approximate
            && parent.kind == "Ghoul"
            && (parent.cell % width).abs_diff(mob.cell % width)
                + (parent.cell / width).abs_diff(mob.cell / width)
                == 1));
    }
}
