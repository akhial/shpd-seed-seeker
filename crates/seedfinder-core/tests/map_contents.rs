#![cfg(feature = "json-query")]

use serde_json::Value;
use shpd_seedfinder_core::{
    challenges::Challenges,
    level_map::{LevelMap, MapDraw, assets, generate_level_map_in_branch},
    seed::DungeonSeed,
};

#[test]
fn spyglass_generation_marks_only_phantom_heaps_after_activation() {
    use shpd_seedfinder_core::catalog::ItemId;
    let seed = (0..100)
        .map(|value| DungeonSeed::new(value).unwrap())
        .find(|&seed| {
            shpd_seedfinder_core::trinkets::trinket_order(seed)[..4]
                .contains(&ItemId::CrackedSpyglass)
        })
        .unwrap();
    for (depth, trinket, expected) in [
        (1, Some(ItemId::CrackedSpyglass), false),
        (6, None, false),
        (6, Some(ItemId::CrackedSpyglass), true),
    ] {
        let map = generate_level_map_in_branch(seed, depth, 0, Challenges::NONE, trinket).unwrap();
        let phantoms = map
            .contents
            .heaps
            .iter()
            .filter(|heap| heap.phantom)
            .count();
        if expected {
            assert!((1..=2).contains(&phantoms));
            assert!(map.contents.heaps.len() > phantoms);
        } else {
            assert_eq!(phantoms, 0);
        }
    }
}

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
fn garden_shafts_follow_blob_cells_and_secret_visibility() {
    let mut ordinary = false;
    let mut secret = false;
    for depth in [
        1, 2, 3, 4, 6, 7, 8, 9, 11, 12, 13, 14, 16, 17, 18, 19, 21, 22, 23, 24,
    ] {
        let map = generate_level_map_in_branch(DungeonSeed::MIN, depth, 0, Challenges::NONE, None)
            .unwrap();
        for effect in map.contents.effects.iter().filter(|e| e.kind == "Foliage") {
            let e = map
                .scene
                .emitters
                .iter()
                .find(|e| e.cell == effect.cell && e.scale_x.is_some())
                .unwrap();
            assert_eq!(e.scale_x.as_ref().unwrap().points, [[0, 0], [1000, 4000]]);
            assert_eq!(
                e.scale_y.as_ref().unwrap().points,
                [[0, 16000], [1000, 32000]]
            );
            let x = i32::try_from(effect.cell % usize::try_from(map.width).unwrap()).unwrap();
            let y = i32::try_from(effect.cell / usize::try_from(map.width).unwrap()).unwrap();
            let hidden = map
                .secret_rooms
                .iter()
                .any(|&[left, top, right, bottom]| x > left && x < right && y > top && y < bottom);
            let concealed = map
                .scene
                .concealed_emitters
                .iter()
                .any(|e| e.cell == effect.cell && e.scale_x.is_some());
            assert_eq!(concealed, !hidden);
            secret |= hidden;
            ordinary |= !hidden;
        }
    }
    assert!(ordinary && secret, "exercise both garden room kinds");
}

#[test]
fn shop_bag_previews_follow_the_requested_purchase_sequence() {
    for (depth, kind, image) in [
        (6, "ScrollHolder", 483),
        (11, "PotionBandolier", 484),
        (16, "MagicalHolster", 485),
    ] {
        let map = generate_level_map_in_branch(DungeonSeed::MIN, depth, 0, Challenges::NONE, None)
            .unwrap();
        let bags: Vec<_> = map
            .contents
            .heaps
            .iter()
            .filter(|h| h.kind == "ForSale")
            .flat_map(|h| &h.items)
            .filter(|i| (481..=485).contains(&i.image))
            .collect();
        assert_eq!(bags.len(), 1);
        assert_eq!(
            (bags[0].kind.as_str(), bags[0].image, bags[0].deterministic),
            (kind, image, false)
        );
    }
}

#[test]
fn enchanted_heaps_glow_without_tinting_containers_or_shadows() {
    use shpd_seedfinder_core::{catalog::ItemId, level_map::MapGlow};
    let map = generate_level_map_in_branch(
        DungeonSeed::from_code("FOI-QDX-EMJ").unwrap(),
        22,
        0,
        Challenges::NONE,
        Some(ItemId::ParchmentScrap),
    )
    .unwrap();
    // Official v4.0: Unstable javelin, Blocking hammer, Venomous gauntlet in a chest.
    for (cell, color) in [
        (1518, [153, 153, 153]),
        (1660, [0, 0, 255]),
        (1205, [68, 0, 170]),
    ] {
        let heap = map.contents.heaps.iter().find(|h| h.cell == cell).unwrap();
        assert_eq!(
            heap.items[0].glow,
            Some(MapGlow {
                color,
                period_ms: 1000
            })
        );
    }
    let glows: Vec<_> = map
        .scene
        .layers
        .iter()
        .filter(|l| l.name == "heaps")
        .flat_map(|l| l.cells.iter().flatten())
        .flat_map(|&i| &map.scene.sprites[i].frames[0])
        .filter_map(|draw| match draw {
            MapDraw::Blit {
                glow: Some(glow),
                tint,
                source,
                ..
            } => {
                assert!(tint.is_none(), "shadows must never pulse");
                assert_eq!(source[1] / 16, 9, "only exposed missiles pulse");
                Some(glow.color)
            }
            _ => None,
        })
        .collect();
    assert!(glows.contains(&[153, 153, 153]));
    assert!(glows.contains(&[0, 0, 255]));
    assert!(
        !glows.contains(&[68, 0, 170]),
        "closed chest hides the gauntlet glow"
    );
}

#[test]
fn vault_vent_cycles_match_official_engine_and_preview_each_turn() {
    let fixture: Value = serde_json::from_str(include_str!("fixtures/vault-flames.json")).unwrap();
    for sample in fixture["samples"].as_array().unwrap() {
        let code = sample["seed"].as_str().unwrap();
        let map = generate_level_map_in_branch(
            DungeonSeed::from_code(code).unwrap(),
            u8::try_from(sample["depth"].as_u64().unwrap()).unwrap(),
            1,
            Challenges::NONE,
            None,
        )
        .unwrap();
        let vents: Vec<_> = map
            .contents
            .features
            .iter()
            .filter(|f| f.kind == "VaultFlameTrap")
            .collect();
        let actual: Vec<_> = vents
            .iter()
            .map(|vent| {
                let cycle = vent.cycle.unwrap();
                serde_json::json!([
                    vent.cell,
                    cycle.initial_cooldown,
                    cycle.cooldown,
                    cycle.triggers
                ])
            })
            .collect();
        assert_eq!(
            actual,
            *sample["cycles"].as_array().unwrap(),
            "{code} seeded cooldowns"
        );
        for vent in vents {
            let cycle = vent.cycle.unwrap();
            let emitters: Vec<_> = map
                .scene
                .emitters
                .iter()
                .filter(|e| e.cell == vent.cell && e.start_ms.is_some())
                .collect();
            assert_eq!(emitters.len(), 2, "{code} vent {}", vent.cell);
            assert!(emitters.iter().all(|e| e.wall_mask));
            let visible = |index: usize, time: u32| {
                let emitter = emitters[index];
                emitter.particles.iter().any(|p| {
                    let first = emitter.start_ms.unwrap() + u32::from(p.birth_ms);
                    time >= first
                        && (time - first) % u32::from(emitter.loop_ms) < u32::from(p.lifespan_ms)
                })
            };
            // VaultFlameTraps.act evolves the existing blob, then decrements
            // cooldowns and seeds warnings. Compare 20 turns, including startup.
            let mut cooldown = cycle.initial_cooldown;
            let mut remaining = 0_u16;
            for turn in 0..20 {
                let burst = remaining > 0;
                remaining = remaining.saturating_sub(1);
                if cooldown == 0 {
                    cooldown = cycle.cooldown;
                }
                cooldown -= 1;
                if cooldown == 0 {
                    remaining = cycle.triggers;
                }
                assert_eq!(
                    visible(0, turn * 1000 + 700),
                    remaining > 0,
                    "{code} vent {} warning at turn {turn}",
                    vent.cell
                );
                assert_eq!(
                    visible(1, turn * 1000 + 200),
                    burst,
                    "{code} vent {} burst at turn {turn}",
                    vent.cell
                );
            }
        }
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
                        "ScrollHolder"
                            | "PotionBandolier"
                            | "MagicalHolster"
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
