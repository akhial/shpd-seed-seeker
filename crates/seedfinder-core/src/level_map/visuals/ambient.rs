//! Actor and garden factories from the pinned game source. Sampling affects
//! presentation only; the generation RNG is never consulted.
#![allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]

use super::{
    Level, objects,
    particles::{curve, sample},
};
use crate::level_map::{MapBlend, MapContents, MapDraw, MapEmitter, MapParticle};

pub(super) fn emitters(level: &Level, contents: &MapContents) -> Vec<MapEmitter> {
    let mut result = Vec::new();
    for effect in &contents.effects {
        if effect.kind == "Foliage" && objects::visible(level, effect.cell) {
            result.push(garden(effect.cell));
        }
    }
    for mob in &contents.mobs {
        if !objects::visible(level, mob.cell) {
            continue;
        }
        match mob.kind.as_str() {
            "Shopkeeper" => result.push(coin(mob.cell)),
            "FireElemental"
            | "NewbornFireElemental"
            | "FrostElemental"
            | "ShockElemental"
            | "ChaosElemental" => {
                result.extend(elemental(mob.cell, &mob.kind));
            }
            _ => {}
        }
    }
    result
}

fn pixel(cell: usize, color: [u8; 3], loop_ms: u16) -> MapEmitter {
    MapEmitter {
        start_ms: None,
        wall_mask: true,
        clip_to_chasm: false,
        cell,
        loop_ms,
        blend: Some(MapBlend::Add),
        image: MapDraw::Fill {
            rgba: [color[0], color[1], color[2], 255],
            destination: [0, 0, 1, 1],
        },
        velocity: [0, 0],
        acceleration: [0, 0],
        angular_speed: 0,
        alpha: curve(&[[0, 1000], [1000, 1000]]),
        scale: curve(&[[0, 1000], [1000, 1000]]),
        scale_x: None,
        scale_y: None,
        particles: Vec::new(),
    }
}

fn coin(cell: usize) -> MapEmitter {
    // ShopkeeperSprite.onComplete(idle): throw from the right hand at each
    // nine-frame, 10 FPS loop boundary. PixelParticle keeps a constant size.
    let mut e = pixel(cell, [255, 255, 0], 900);
    e.blend = None;
    e.velocity = [0, -40];
    e.acceleration = [0, 160];
    e.particles.push(MapParticle {
        birth_ms: 0,
        lifespan_ms: 500,
        position: [14500, 3500],
        scale: 1000,
        angle: 0,
    });
    e
}

fn garden(cell: usize) -> MapEmitter {
    // Foliage.use / ShaftParticle: one shaft per cell every .9s, rising at
    // 6px/s. Its width grows 0..4px while its height grows 16..32px.
    let mut e = pixel(cell, [255, 255, 255], 2700);
    e.velocity = [0, -6];
    e.alpha = curve(&[[0, 0], [500, 500], [1000, 0]]);
    e.scale_x = Some(curve(&[[0, 0], [1000, 4000]]));
    e.scale_y = Some(curve(&[[0, 16000], [1000, 32000]]));
    e.particles = (0..3)
        .map(|i| {
            // The game starts with a random invisible delay up to 1.2s. Fold
            // that interval into birth and position, keeping the visible life.
            let delay = (sample(cell, i, 20) * 1200.0) as u16;
            MapParticle {
                birth_ms: (i as u16 * 900 + delay) % e.loop_ms,
                lifespan_ms: 1200,
                position: [
                    500 + (sample(cell, i, 21) * 16000.0) as i32,
                    500 + (sample(cell, i, 22) * 16000.0) as i32 - i32::from(delay) * 6,
                ],
                scale: 1000,
                angle: 0,
            }
        })
        .collect();
    e
}

fn elemental(cell: usize, kind: &str) -> Vec<MapEmitter> {
    let chaos = kind == "ChaosElemental";
    let frost = kind == "FrostElemental";
    let shock = kind == "ShockElemental";
    let color = match kind {
        "FireElemental" => [238, 119, 34],
        "NewbornFireElemental" => [34, 238, 102],
        "FrostElemental" => [136, 204, 255],
        _ => [255, 255, 255],
    };
    let interval = if chaos { 25 } else { 60 };
    let phase = (sample(cell, 0, 23) * 3000.0) as u16;
    let mut base = pixel(cell, color, 3000);
    if chaos || frost {
        base.alpha = curve(&[[0, 1000], [1000, 0]]);
        base.scale = curve(&[[0, 1000], [1000, if chaos { 5000 } else { 4000 }]]);
    } else if shock {
        // SparkParticle.STATIC flickers as it shrinks. A repeatable sampled
        // envelope stands in for the game's per-display-frame Random.Float.
        base.scale = curve(&[
            [0, 4000],
            [120, 800],
            [240, 3400],
            [360, 1000],
            [480, 2200],
            [600, 400],
            [720, 1200],
            [840, 300],
            [1000, 0],
        ]);
    } else {
        base.acceleration = [0, -80];
        base.alpha = curve(&[[0, 0], [200, 1000], [1000, 1000]]);
        base.scale = curve(&[[0, 4000], [1000, 0]]);
    }
    let mut result = Vec::new();
    for i in 0..3000 / interval {
        let n = usize::from(i);
        let particle = MapParticle {
            birth_ms: (i * interval + phase) % 3000,
            lifespan_ms: if shock {
                250 + (sample(cell, n, 24) * 250.0) as u16
            } else if chaos || frost {
                500
            } else {
                600
            },
            // CharSprite.emitter() covers the raised 12x14 actor rectangle.
            position: [
                2500 + (sample(cell, n, 25) * 12000.0) as i32,
                -3500 + (sample(cell, n, 26) * 14000.0) as i32,
            ],
            scale: 1000,
            angle: 0,
        };
        if chaos || frost {
            // Independent velocities/colors are represented by independent
            // emitters, keeping the portable trajectory contract unchanged.
            let mut e = base.clone();
            if chaos {
                let angle = sample(cell, n, 27) * std::f32::consts::TAU;
                let speed = 16.0 + sample(cell, n, 28) * 16.0;
                e.velocity = [
                    (angle.cos() * speed).round() as i16,
                    (angle.sin() * speed).round() as i16,
                ];
                if let MapDraw::Fill { rgba, .. } = &mut e.image {
                    let [red, green, blue] =
                        [29, 30, 31].map(|c| (sample(cell, n, c) * 256.0) as u8);
                    *rgba = [red, green, blue, 255];
                }
            } else {
                e.velocity = [27, 28].map(|c| (sample(cell, n, c) * 20.0 - 10.0).round() as i16);
            }
            e.particles.push(particle);
            result.push(e);
        } else {
            base.particles.push(particle);
        }
    }
    if !base.particles.is_empty() {
        result.push(base);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{level::Feeling, level_map::MapMob};

    #[test]
    fn every_elemental_has_its_factory_even_while_sleeping_and_hidden_cells_emit_nothing() {
        let mut level = Level::new(17, Feeling::None);
        level.set_size(5, 5);
        level.map.cells[12] = crate::geometry::terrain::EMPTY;
        for (kind, color, count, life) in [
            ("FireElemental", [238, 119, 34], 50, 600),
            ("NewbornFireElemental", [34, 238, 102], 50, 600),
            ("FrostElemental", [136, 204, 255], 50, 500),
            ("ShockElemental", [255, 255, 255], 50, 500),
            ("ChaosElemental", [0, 0, 0], 120, 500),
        ] {
            let contents = MapContents {
                mobs: vec![MapMob {
                    cell: 12,
                    kind: kind.into(),
                    sleeping: true,
                    stealthy: false,
                    approximate: false,
                    items: vec![],
                }],
                ..MapContents::default()
            };
            let effects = emitters(&level, &contents);
            assert_eq!(
                effects.iter().map(|e| e.particles.len()).sum::<usize>(),
                count,
                "{kind}"
            );
            for e in &effects {
                assert_eq!(e.blend, Some(MapBlend::Add));
                assert!(e.wall_mask);
                if kind != "ChaosElemental" {
                    assert!(matches!(e.image, MapDraw::Fill { rgba, .. } if rgba[..3] == color));
                }
                assert!(e.particles.iter().all(|p| p.lifespan_ms <= life));
            }
            level.map.cells[12] = crate::geometry::terrain::WALL;
            assert!(emitters(&level, &contents).is_empty());
            level.map.cells[12] = crate::geometry::terrain::EMPTY;
        }
    }

    #[test]
    fn shopkeepers_coin_returns_to_his_hand_in_sync_with_idle() {
        let e = coin(12);
        let p = &e.particles[0];
        let y = |seconds: f64| {
            f64::from(p.position[1]) / 1000.0
                + f64::from(e.velocity[1]) * seconds
                + f64::from(e.acceleration[1]) * seconds * seconds / 2.0
        };
        assert_eq!(e.loop_ms, 900);
        assert_eq!(p.lifespan_ms, 500);
        assert!((y(0.0) - y(0.5)).abs() < 1e-9);
        assert!((y(0.25) - y(0.0) + 5.0).abs() < 1e-9);
    }
}
