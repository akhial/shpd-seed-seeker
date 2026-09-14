//! One game turn per second, with the original ray/CheckedCell/TargetedCell art.
#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::cast_precision_loss
)]
mod geometry;
use self::geometry::Geometry;
use crate::{
    level::Level,
    level_map::{MapBlend, MapContents, MapCurve, MapDraw, MapEmitter, MapParticle},
};
use std::collections::BTreeSet;

pub(super) fn emitters(level: &Level, contents: &MapContents) -> Vec<MapEmitter> {
    if contents.sentries.is_empty() {
        return Vec::new();
    }
    let geometry = Geometry::new(level, contents);
    let mut out = Vec::new();
    for sentry in &contents.sentries {
        // Opposite treasure-room lasers are scenery only (Integer.MAX_VALUE).
        if sentry.cooldown == i32::MAX as u32
            || sentry.triggers == 0
            || !super::objects::visible(level, sentry.cell)
        {
            continue;
        }
        let count = sentry.directions.len() as u32;
        let triggers = u32::from(sentry.triggers);
        let group = sentry.cooldown + triggers - 1;
        let groups = count / gcd(count, triggers);
        let period = (group * groups * 1000) as u16;
        let first = sentry.initial_cooldown.saturating_sub(1) * 1000;
        let fov = geometry.field_of_view(sentry.cell);
        for group_index in 0..groups {
            for shot in 0..triggers {
                let phase = ((group_index * triggers + shot) % count) as usize;
                let time = first + (group_index * group + shot) * 1000;
                let mut cells = BTreeSet::new();
                for &target in &sentry.directions[phase] {
                    if let Some(shape) = sentry.scan {
                        cells.extend(geometry.cone(sentry.cell, target, shape, &fov));
                    } else {
                        let path = geometry.ray(sentry.cell, target, true, false);
                        if path.len() > 1 {
                            out.push(ray(
                                sentry.cell,
                                *path.last().unwrap(),
                                level.width(),
                                time,
                                period,
                            ));
                            cells.extend(path.into_iter().skip(1));
                        }
                    }
                }
                cells.retain(|&cell| super::objects::visible(level, cell));
                if cells.is_empty() {
                    continue;
                }
                if sentry.scan.is_some() {
                    out.push(checked(sentry.cell, &cells, level.width(), time, period));
                }
                if sentry.warning {
                    // Startup never invents a warning before the first act.
                    let warning_time = if time < 1000 {
                        time + u32::from(period) - 1000
                    } else {
                        time - 1000
                    };
                    out.push(warning(
                        sentry.cell,
                        &cells,
                        level.width(),
                        warning_time,
                        period,
                    ));
                }
            }
        }
    }
    out
}

fn gcd(mut a: u32, mut b: u32) -> u32 {
    while b != 0 {
        (a, b) = (b, a % b);
    }
    a
}
fn curve(points: &[[u16; 2]]) -> MapCurve {
    MapCurve {
        points: points.to_vec(),
        sqrt: false,
    }
}
fn base(cell: usize, start: u32, period: u16) -> MapEmitter {
    MapEmitter {
        cell,
        start_ms: Some(start),
        loop_ms: period,
        wall_mask: true,
        clip_to_chasm: false,
        blend: None,
        image: MapDraw::Fill {
            rgba: [85, 170, 255, 255],
            destination: [0, 0, 1, 1],
        },
        velocity: [0, 0],
        acceleration: [0, 0],
        angular_speed: 0,
        alpha: curve(&[[0, 1000], [1000, 0]]),
        scale: curve(&[[0, 1000], [1000, 0]]),
        scale_y: None,
        particles: Vec::new(),
    }
}
fn position(from: usize, cell: usize, width: i32) -> [i32; 2] {
    [
        (i32::try_from(cell).unwrap() % width - i32::try_from(from).unwrap() % width) * 16000
            + 8500,
        (i32::try_from(cell).unwrap() / width - i32::try_from(from).unwrap() / width) * 16000
            + 8500,
    ]
}
fn checked(
    from: usize,
    cells: &BTreeSet<usize>,
    width: i32,
    start: u32,
    period: u16,
) -> MapEmitter {
    let mut e = base(from, start, period);
    e.alpha = curve(&[[0, 800], [1000, 0]]);
    for &cell in cells {
        let pos = position(from, cell, width);
        let distance = ((pos[0] - 8500) as f32).hypot((pos[1] - 8500) as f32) / 16000.0;
        let delay = (distance - 1.0).max(0.0).powf(0.67) * 100.0;
        e.particles.push(MapParticle {
            birth_ms: delay.round() as u16,
            lifespan_ms: 800,
            position: pos,
            scale: 12800,
            angle: 0,
        });
    }
    e
}
fn warning(
    from: usize,
    cells: &BTreeSet<usize>,
    width: i32,
    start: u32,
    period: u16,
) -> MapEmitter {
    let mut e = base(from, start, period);
    e.image = MapDraw::Blit {
        asset: "icons.png",
        source: [0, 32, 16, 16],
        destination: [0, 0, 16, 16],
        tint: Some([255, 0, 0]),
        opacity: 255,
        glow: None,
    };
    e.alpha = curve(&[[0, 1000], [250, 600], [625, 600], [1000, 0]]);
    e.scale = MapCurve {
        points: (0..=80)
            .map(|i| {
                let time = f32::from(i) * 20.0;
                let alpha = if time <= 1000.0 {
                    (1.0 - time / 1000.0).max(0.6)
                } else {
                    (1600.0 - time) / 1000.0
                };
                [i * 25 / 2, (alpha.powf(0.33) * 1000.0).round() as u16]
            })
            .collect(),
        sqrt: false,
    };
    e.particles = cells
        .iter()
        .map(|&cell| {
            let [x, y] = position(from, cell, width);
            MapParticle {
                birth_ms: 0,
                lifespan_ms: 1600,
                position: [x - 500, y - 500],
                scale: 1000,
                angle: 0,
            }
        })
        .collect();
    e
}
fn ray(from: usize, to: usize, width: i32, start: u32, period: u16) -> MapEmitter {
    let mut e = base(from, start, period);
    // SentrySprite.center (8x15, raised 6px), then raisedTileCenterToWorld.
    let dx =
        (i32::try_from(to).unwrap() % width - i32::try_from(from).unwrap() % width) as f32 * 16.0;
    let dy = (i32::try_from(to).unwrap() / width - i32::try_from(from).unwrap() / width) as f32
        * 16.0
        + 1.6
        - 2.5;
    e.image = MapDraw::Blit {
        asset: "effects.png",
        source: [16, 16, 16, 8],
        destination: [0, 0, dx.hypot(dy).round() as u16, 8],
        tint: None,
        opacity: 255,
        glow: None,
    };
    e.blend = Some(MapBlend::Add);
    e.scale_y = Some(curve(&[[0, 1000], [1000, 0]]));
    e.scale = curve(&[[0, 1000], [1000, 1000]]);
    e.particles.push(MapParticle {
        birth_ms: 0,
        lifespan_ms: 500,
        position: [
            (8000.0 + dx * 500.0).round() as i32,
            (2500.0 + dy * 500.0).round() as i32,
        ],
        scale: 1000,
        angle: dy.atan2(dx).to_degrees().rem_euclid(360.0).round() as u16,
    });
    e
}
