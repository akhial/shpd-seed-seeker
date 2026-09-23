#![cfg(feature = "json-query")]
use serde_json::Value;
use shpd_seedfinder_core::{
    challenges::Challenges,
    level_map::{LevelMap, MapDraw, MapEmitter, generate_level_map_in_branch},
    seed::DungeonSeed,
    vault_sentries::VaultSentryPattern,
};
use std::collections::BTreeMap;

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
        check_warnings(&map, expected, code);
    }
}

fn is_warning(emitter: &MapEmitter) -> bool {
    matches!(
        emitter.image,
        MapDraw::Blit {
            asset: "icons.png",
            source: [0, 32, 16, 16],
            ..
        }
    )
}

fn curve_value(curve: &shpd_seedfinder_core::level_map::MapCurve, progress: f64) -> f64 {
    let points = curve
        .points
        .windows(2)
        .find(|points| f64::from(points[1][0]) >= progress * 1000.0)
        .unwrap();
    let [[x0, y0], [x1, y1]] = [points[0], points[1]];
    (f64::from(y0)
        + (f64::from(y1) - f64::from(y0)) * (progress * 1000.0 - f64::from(x0))
            / f64::from(x1 - x0))
        / 1000.0
}

fn check_warnings(map: &LevelMap, expected: &[Value], code: &str) {
    // Crossing lasers have independent 3–7 turn periods: 84 seconds covers
    // two complete cycles even for the longest pair (6 and 7 turns).
    for emitters in [&map.scene.emitters, &map.scene.concealed_emitters] {
        let warnings: Vec<_> = emitters.iter().filter(|e| is_warning(e)).collect();
        for turn in 0..84 {
            for offset in [0, 399, 400, 599, 600, 999] {
                let time = turn * 1000 + offset;
                let mut ages = BTreeMap::<usize, u32>::new();
                for (sentry, expected) in map.contents.sentries.iter().zip(expected) {
                    if !sentry.warning {
                        continue;
                    }
                    let start = if sentry.initial_cooldown == 1 {
                        sentry.cooldown * 1000 - 1000
                    } else {
                        (sentry.initial_cooldown - 2) * 1000
                    };
                    if time < start {
                        continue;
                    }
                    let age = (time - start) % (sentry.cooldown * 1000);
                    if age < 1600 {
                        for cell in expected["coverage"][0].as_array().unwrap() {
                            ages.entry(usize::try_from(cell.as_u64().unwrap()).unwrap())
                                .and_modify(|previous| *previous = (*previous).min(age))
                                .or_insert(age);
                        }
                    }
                }
                let mut active = BTreeMap::new();
                for emitter in &warnings {
                    let cells = particle_cells(emitter, map.width, 8000);
                    for (particle, cell) in emitter.particles.iter().zip(cells) {
                        let start = emitter.start_ms.unwrap() + u32::from(particle.birth_ms);
                        if time < start {
                            continue;
                        }
                        let age = (time - start) % u32::from(emitter.loop_ms);
                        if age >= u32::from(particle.lifespan_ms) {
                            continue;
                        }
                        let cell = usize::try_from(cell.as_u64().unwrap()).unwrap();
                        assert!(
                            active.insert(cell, age).is_none(),
                            "{code}: overlapping warning reticles at cell {cell}, {time}ms"
                        );
                        let alpha = if age <= 1000 {
                            (1.0 - f64::from(age) / 1000.0).max(0.6)
                        } else {
                            f64::from(1600 - age) / 1000.0
                        };
                        let progress = f64::from(age) / f64::from(particle.lifespan_ms);
                        assert!((curve_value(&emitter.alpha, progress) - alpha).abs() < 0.001);
                        // Check the scale before a reset can truncate the
                        // warning; its later fade uses the existing curve.
                        if age <= 1000 {
                            assert!(
                                (curve_value(&emitter.scale, progress) - alpha.powf(0.33)).abs()
                                    < 0.001
                            );
                        }
                    }
                }
                assert_eq!(active, ages, "{code}: warning coverage at {time}ms");
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
    if sentry.cooldown == i32::MAX as u32 {
        assert!(
            beams.is_empty(),
            "{code} decorative sentry {cell} must not fire"
        );
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
}
