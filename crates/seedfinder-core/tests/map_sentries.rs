#![cfg(feature = "json-query")]
use serde_json::Value;
use shpd_seedfinder_core::{
    challenges::Challenges,
    level_map::{LevelMap, MapDraw, MapEmitter, generate_level_map_in_branch},
    seed::DungeonSeed,
    vault_sentries::VaultSentryPattern,
};

#[test]
fn sentry_patterns_and_scan_coverage_match_the_official_engine() {
    let fixture: Value =
        serde_json::from_str(include_str!("fixtures/vault-sentries.json")).unwrap();
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
        let expected = sample["sentries"].as_array().unwrap();
        assert_eq!(
            map.contents.sentries.len(),
            expected.len(),
            "{code} sentry count"
        );
        for (sentry, expected) in map.contents.sentries.iter().zip(expected) {
            assert_eq!(
                serde_json::to_value(sentry).unwrap(),
                expected["pattern"],
                "{code} sentry {} setup",
                sentry.cell
            );
            if sentry.scan.is_some() {
                check_scan(&map, sentry, expected, code);
            } else {
                check_laser(&map, sentry, expected, code);
            }
        }
    }
}

fn check_scan(map: &LevelMap, sentry: &VaultSentryPattern, expected: &Value, code: &str) {
    let cell = sentry.cell;
    let checked: Vec<_> = map
        .scene
        .emitters
        .iter()
        .filter(|e| {
            e.cell == cell
                && matches!(
                    &e.image,
                    MapDraw::Fill {
                        rgba: [85, 170, 255, 255],
                        ..
                    }
                )
        })
        .collect();
    let phases: Vec<_> = expected["coverage"]
        .as_array()
        .unwrap()
        .iter()
        .enumerate()
        .filter(|(_, cells)| !cells.as_array().unwrap().is_empty())
        .collect();
    assert_eq!(
        checked.len(),
        phases.len(),
        "{code} sentry {cell} scan phases"
    );
    for (emitter, (phase, cells)) in checked.iter().zip(phases) {
        assert_eq!(
            particle_cells(emitter, map.width, 8500),
            *cells.as_array().unwrap(),
            "{code} sentry {cell} phase {phase}"
        );
        assert_eq!(
            emitter.start_ms,
            Some(u32::try_from(phase).unwrap() * sentry.cooldown * 1000)
        );
        assert!(emitter.wall_mask);
    }
}

fn particle_cells(emitter: &MapEmitter, width: i32, offset: i32) -> Vec<Value> {
    emitter
        .particles
        .iter()
        .map(|p| {
            let dx = (p.position[0] - offset) / 16000;
            let dy = (p.position[1] - offset) / 16000;
            serde_json::json!(i32::try_from(emitter.cell).unwrap() + dx + dy * width)
        })
        .collect()
}

fn check_laser(map: &LevelMap, sentry: &VaultSentryPattern, expected: &Value, code: &str) {
    let cell = sentry.cell;
    let beams: Vec<_> = map
        .scene
        .emitters
        .iter()
        .filter(|e| {
            e.cell == cell
                && matches!(
                    &e.image,
                    MapDraw::Blit {
                        asset: "effects.png",
                        ..
                    }
                )
        })
        .collect();
    let warnings: Vec<_> = map
        .scene
        .emitters
        .iter()
        .filter(|e| {
            e.cell == cell
                && matches!(
                    &e.image,
                    MapDraw::Blit {
                        asset: "icons.png",
                        source: [0, 32, 16, 16],
                        ..
                    }
                )
        })
        .collect();
    if sentry.cooldown == i32::MAX as u32 {
        assert!(
            beams.is_empty(),
            "{code} decorative sentry {cell} must not fire"
        );
        assert!(warnings.is_empty());
        return;
    }
    assert_eq!(beams.len(), usize::from(sentry.triggers));
    let path = expected["coverage"][0].as_array().unwrap();
    let target = i32::try_from(path.last().unwrap().as_u64().unwrap()).unwrap();
    for (shot, beam) in beams.iter().enumerate() {
        assert_eq!(
            beam.start_ms,
            Some((sentry.initial_cooldown - 1 + u32::try_from(shot).unwrap()) * 1000)
        );
        assert_eq!(
            u32::from(beam.loop_ms),
            (sentry.cooldown + u32::from(sentry.triggers) - 1) * 1000
        );
        let p = &beam.particles[0];
        let dx = (2 * p.position[0] - 16000 + 8000).div_euclid(16000);
        let dy = (2 * p.position[1] - 4100 + 8000).div_euclid(16000);
        assert_eq!(
            i32::try_from(cell).unwrap() + dx + dy * map.width,
            target,
            "{code} sentry {cell} beam end"
        );
        assert_eq!(p.lifespan_ms, 500);
        assert!(beam.wall_mask);
    }
    if sentry.warning {
        assert_eq!(warnings.len(), 1);
        let mut path = path.clone();
        path.sort_by_key(|v| v.as_u64().unwrap());
        assert_eq!(particle_cells(warnings[0], map.width, 8000), path);
        let first = beams[0].start_ms.unwrap();
        let warning = if first == 0 {
            sentry.cooldown * 1000 - 1000
        } else {
            first - 1000
        };
        assert_eq!(warnings[0].start_ms, Some(warning));
    } else {
        assert!(warnings.is_empty());
    }
}
