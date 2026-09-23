//! GameScene.targetedCell resets the existing reticle at a cell. Merge the
//! lasers' schedules so crossings never draw simultaneous warning particles.
use super::{base, curve, gcd, position};
use crate::level_map::{MapCurve, MapDraw, MapEmitter, MapParticle};
use std::collections::{BTreeMap, BTreeSet};

pub(super) fn emitters(
    schedules: BTreeMap<usize, BTreeSet<(u32, u16)>>,
    width: i32,
) -> Vec<MapEmitter> {
    let mut emitters = BTreeMap::new();
    for (cell, schedules) in schedules {
        // At most two warning lasers cross a tile, each with a 3–7 turn
        // cooldown. Their combined loop fits in 42 seconds.
        let period = schedules.iter().fold(1, |period, &(_, repeat)| {
            period / gcd(period, u32::from(repeat)) * u32::from(repeat)
        });
        let period = u16::try_from(period).expect("vault warning loop fits in u16");
        let times: BTreeSet<_> = schedules
            .iter()
            .flat_map(|&(start, repeat)| (start..u32::from(period)).step_by(usize::from(repeat)))
            .collect();
        let times: Vec<_> = times.into_iter().collect();
        for (index, &time) in times.iter().enumerate() {
            let next = times
                .get(index + 1)
                .copied()
                .unwrap_or(times[0] + u32::from(period));
            let lifespan = u16::try_from((next - time).min(1600)).unwrap();
            // End the previous warning when the next one resets it, while
            // keeping its original fade/scale speed up to that instant.
            let emitter = emitters
                .entry((period, lifespan))
                .or_insert_with(|| warning(cell, period, lifespan));
            let [x, y] = position(emitter.cell, cell, width);
            emitter.particles.push(MapParticle {
                birth_ms: u16::try_from(time).unwrap(),
                lifespan_ms: lifespan,
                position: [x - 500, y - 500],
                scale: 1000,
                angle: 0,
            });
        }
    }
    emitters.into_values().collect()
}

fn warning(cell: usize, period: u16, lifespan: u16) -> MapEmitter {
    let mut e = base(cell, 0, period);
    e.image = MapDraw::Blit {
        asset: "icons.png",
        source: [0, 32, 16, 16],
        destination: [0, 0, 16, 16],
        tint: Some([255, 0, 0]),
        opacity: 255,
        glow: None,
    };
    e.alpha = if lifespan == 1000 {
        curve(&[[0, 1000], [400, 600], [1000, 600]])
    } else {
        curve(&[[0, 1000], [250, 600], [625, 600], [1000, 0]])
    };
    e.scale = MapCurve {
        points: (0..=lifespan / 20)
            .map(|i| {
                let time = f32::from(i) * 20.0;
                let alpha = if time <= 1000.0 {
                    (1.0 - time / 1000.0).max(0.6)
                } else {
                    (1600.0 - time) / 1000.0
                };
                [
                    (u32::from(i) * 20000 / u32::from(lifespan)) as u16,
                    (alpha.powf(0.33) * 1000.0).round() as u16,
                ]
            })
            .collect(),
        sqrt: false,
    };
    e
}
