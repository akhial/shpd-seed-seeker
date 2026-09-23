//! Source-pinned ambient factories, sampled only for initial visual randomness.
//! Frontends evaluate these trajectories continuously at the display refresh rate.
#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::cast_precision_loss
)]
use super::{Level, MapDraw, objects};
use crate::level_map::{MapBlend, MapContents, MapCurve, MapEmitter, MapParticle};

#[derive(Clone, Copy)]
enum Particle {
    Health,
    Question,
    Bubble,
    Sacrifice,
    Eternal,
    VaultVent,
    CityFlame,
    Toxic,
    RotHeart,
}
impl Particle {
    fn kind(name: &str) -> Option<Self> {
        Some(match name {
            "WaterOfHealth" => Self::Health,
            "WaterOfAwareness" => Self::Question,
            "Alchemy" => Self::Bubble,
            "SacrificialFire" => Self::Sacrifice,
            "EternalFire" => Self::Eternal,
            "ToxicGas" => Self::Toxic,
            _ => return None,
        })
    }
}

pub(super) fn emitters(level: &Level, contents: &MapContents) -> Vec<MapEmitter> {
    let mut result: Vec<_> = contents
        .effects
        .iter()
        .filter(|e| objects::visible(level, e.cell))
        .filter_map(|e| Particle::kind(&e.kind).map(|kind| emitter(e.cell, kind)))
        .collect();
    result.extend(super::ambient::emitters(level, contents));
    result.extend(super::sentries::emitters(level, contents));
    for feature in &contents.features {
        if feature.kind == "VaultFlameTrap" && objects::visible(level, feature.cell) {
            if let Some(cycle) = feature.cycle {
                result.extend(vent_emitters(feature.cell, cycle));
            }
        }
    }
    if level.depth <= 5 {
        for (cell, &tile) in level.map.cells.iter().enumerate() {
            let below = cell + usize::try_from(level.width()).unwrap();
            if tile == crate::geometry::terrain::WALL_DECO
                && below < level.len()
                && objects::visible(level, below)
            {
                result.push(pipe_drips(cell));
            }
        }
    }
    if (16..=20).contains(&level.depth) {
        for (cell, &tile) in level.map.cells.iter().enumerate() {
            if matches!(
                tile,
                crate::geometry::terrain::REGION_DECO | crate::geometry::terrain::REGION_DECO_ALT
            ) {
                result.push(emitter(cell, Particle::CityFlame));
            }
        }
    }
    for mob in &contents.mobs {
        if mob.kind == "RotHeart" && objects::visible(level, mob.cell) {
            result.push(emitter(mob.cell, Particle::RotHeart));
        }
        if mob.kind == "Blacksmith" && objects::visible(level, mob.cell) {
            result.extend(forge_sparks(mob.cell));
        }
        if !mob.sleeping || mob.kind.ends_with("Mimic") || !objects::visible(level, mob.cell) {
            continue;
        }
        let Some(sprite) = super::actors::actor(&mob.kind) else {
            continue;
        };
        result.push(MapEmitter {
            start_ms: None,
            scale_x: None,
            scale_y: None,
            wall_mask: false,
            clip_to_chasm: false,
            cell: mob.cell,
            loop_ms: 800,
            blend: None,
            image: MapDraw::Blit {
                opacity: 255,
                tint: None,
                glow: None,
                asset: "icons.png",
                source: [7, 88, 9, 8],
                destination: [0, 0, 9, 8],
            },
            velocity: [0, 0],
            angular_speed: 0,
            acceleration: [0, 0],
            alpha: curve(&[[0, 1000], [1000, 1000]]),
            scale: curve(&[[0, 1000], [500, 1200], [1000, 1000]]),
            particles: vec![MapParticle {
                angle: 0,
                birth_ms: (sample(mob.cell, 0, 8) * 800.0) as u16,
                lifespan_ms: 800,
                position: [
                    (8000 + i32::from(sprite.width) * 500),
                    (12 - i32::from(sprite.height) - sprite.raise) * 1000,
                ],
                scale: 1000,
            }],
        });
    }
    result
}

pub(super) fn sample(cell: usize, particle: usize, component: u32) -> f32 {
    let mut v = (cell as u32).wrapping_mul(0x9e37_79b9)
        ^ (particle as u32).wrapping_mul(0x85eb_ca6b)
        ^ component.wrapping_mul(0xc2b2_ae35);
    v ^= v >> 16;
    v = v.wrapping_mul(0x7feb_352d);
    v ^= v >> 15;
    v = v.wrapping_mul(0x846c_a68b);
    v ^= v >> 16;
    (v >> 8) as f32 / 16_777_216.0
}
pub(super) fn curve(points: &[[u16; 2]]) -> MapCurve {
    MapCurve {
        points: points.to_vec(),
        sqrt: false,
    }
}

/// Preview one game turn per second. VaultFlameTraps.act first evolves the
/// previous warning, then decrements/reseeds cooldowns; the burst follows one
/// turn after its warning. Particle movement remains at display refresh rate.
fn vent_emitters(cell: usize, cycle: crate::vault_floor::VaultFlameCycle) -> [MapEmitter; 2] {
    let first_turn = if cycle.initial_cooldown == 0 {
        cycle.cooldown
    } else {
        cycle.initial_cooldown
    };
    let first_ms = u32::from(first_turn.saturating_sub(1)) * 1000;
    let make = |kind, burst| {
        let mut e = emitter(cell, kind);
        e.loop_ms = cycle.cooldown * 1000;
        e.start_ms = Some(first_ms + if burst { 1000 } else { 0 });
        let samples = e.particles.clone();
        e.particles.clear();
        // Continuous treasure vents keep the factory's uninterrupted 0.3s flow.
        if !burst && cycle.cooldown == cycle.triggers {
            e.loop_ms = 3000;
        }
        let count = if burst {
            cycle.triggers * 10
        } else if cycle.cooldown == cycle.triggers {
            10
        } else {
            (cycle.triggers * 1000).div_ceil(300)
        };
        for index in 0..count {
            let mut particle = samples[usize::from(index) % samples.len()].clone();
            particle.birth_ms = if burst {
                index / 10 * 1000 + index % 10 * 20
            } else {
                index * 300
            };
            e.particles.push(particle);
        }
        e
    };
    [
        make(Particle::VaultVent, false),
        make(Particle::Eternal, true),
    ]
}

#[allow(clippy::too_many_lines)] // Source factory parameters stay together for auditing.
fn emitter(cell: usize, kind: Particle) -> MapEmitter {
    let (interval, life, velocity, acceleration, speck) = match kind {
        Particle::Health => (500, 1000, -20, 0, Some(0)),
        Particle::Question => (300, 800, 0, 0, Some(3)),
        Particle::Bubble => (330, 1500, -15, 0, Some(12)),
        Particle::Sacrifice => (100, 600, 0, -100, None),
        Particle::Eternal => (20, 600, 0, -80, None),
        Particle::VaultVent => (300, 600, 0, -80, None),
        Particle::CityFlame => (100, 600, 0, -40, None),
        Particle::Toxic => (400, 3000, 0, 0, Some(13)),
        // RotHeartSprite.link pours Speck.TOXIC every .7s over its sprite.
        Particle::RotHeart => (700, 3000, 0, 0, Some(13)),
    };
    let loop_ms = if matches!(kind, Particle::Bubble) {
        3300
    } else if matches!(kind, Particle::Toxic) {
        4000
    } else if matches!(kind, Particle::RotHeart) {
        3500
    } else {
        3000
    };
    let alpha = match kind {
        Particle::Toxic | Particle::RotHeart => MapCurve {
            points: vec![[0, 0], [500, 250], [1000, 0]],
            sqrt: true,
        },
        Particle::Health => curve(&[[0, 1000], [500, 1000], [1000, 0]]),
        Particle::Question => curve(&[[0, 1000], [1000, 1000]]),
        Particle::Sacrifice => curve(&[[0, 0], [250, 1000], [1000, 1000]]),
        _ => curve(&[[0, 0], [200, 1000], [1000, 1000]]),
    };
    let scale = match kind {
        Particle::Toxic | Particle::RotHeart => curve(&[[0, 1000], [1000, 2000]]),
        Particle::Question => MapCurve {
            points: vec![[0, 0], [500, 4500], [1000, 0]],
            sqrt: true,
        },
        Particle::Sacrifice | Particle::Eternal | Particle::VaultVent | Particle::CityFlame => {
            curve(&[[0, 1000], [1000, 0]])
        }
        _ => curve(&[[0, 1000], [1000, 1000]]),
    };
    let image = if let Some(index) = speck {
        let columns = crate::level_map::assets::get("specks.png").unwrap().width / 7;
        MapDraw::Blit {
            opacity: 255,
            tint: matches!(kind, Particle::Toxic | Particle::RotHeart).then_some([80, 255, 96]),
            glow: None,
            asset: "specks.png",
            source: [index % columns * 7, index / columns * 7, 7, 7],
            destination: [0, 0, 7, 7],
        }
    } else {
        MapDraw::Fill {
            rgba: if matches!(kind, Particle::Sacrifice) {
                [68, 136, 238, 255]
            } else {
                [34, 238, 102, 255]
            },
            destination: [0, 0, 1, 1],
        }
    };
    let phase = (sample(cell, 0, 5) * f32::from(loop_ms)) as u16;
    let particles = (0..loop_ms / interval)
        .map(|i| {
            let n = usize::from(i);
            MapParticle {
                angle: if matches!(kind, Particle::Toxic | Particle::RotHeart) {
                    (sample(cell, n, 6) * 360.0) as u16
                } else {
                    0
                },
                birth_ms: (i * interval + phase) % loop_ms,
                lifespan_ms: if matches!(kind, Particle::Bubble) {
                    800 + (sample(cell, n, 3) * 700.0) as u16
                } else if matches!(kind, Particle::Toxic | Particle::RotHeart) {
                    1000 + (sample(cell, n, 3) * 2000.0) as u16
                } else {
                    life
                },
                position: if matches!(kind, Particle::VaultVent) {
                    // VaultFlameTraps.use: centered 20% emitter bounds.
                    [
                        6900 + (sample(cell, n, 1) * 3200.0) as i32,
                        6900 + (sample(cell, n, 2) * 3200.0) as i32,
                    ]
                } else if matches!(kind, Particle::CityFlame) {
                    // raisedTileCenter (8, 1.6), emitter rect (-2,-5,4,4).
                    [
                        6500 + (sample(cell, n, 1) * 4000.0) as i32,
                        -2900 + (sample(cell, n, 2) * 4000.0) as i32,
                    ]
                } else {
                    [
                        (sample(cell, n, 1) * 16000.0) as i32
                            + if speck.is_none() { 500 } else { 0 },
                        (sample(cell, n, 2) * 16000.0) as i32
                            + if speck.is_none() { 500 } else { 0 }
                            - if matches!(kind, Particle::Sacrifice) {
                                4000
                            } else if matches!(kind, Particle::RotHeart) {
                                // CharSprite.emitter covers the 16x16 heart,
                                // raised three pixels in the map scene.
                                3000
                            } else {
                                0
                            },
                    ]
                },
                scale: if matches!(kind, Particle::Bubble) {
                    800 + (sample(cell, n, 4) * 200.0) as u16
                } else if speck.is_none() {
                    4000
                } else {
                    1000
                },
            }
        })
        .collect();
    MapEmitter {
        start_ms: None,
        scale_x: None,
        scale_y: None,
        wall_mask: true,
        clip_to_chasm: false,
        cell,
        loop_ms,
        blend: if speck.is_none() {
            Some(MapBlend::Add)
        } else {
            None
        },
        image,
        velocity: [0, velocity],
        angular_speed: if matches!(kind, Particle::Toxic | Particle::RotHeart) {
            30
        } else {
            0
        },
        acceleration: [0, acceleration],
        alpha,
        scale,
        particles,
    }
}

fn forge_sparks(cell: usize) -> Vec<MapEmitter> {
    // BlacksmithSprite emits three Speck.FORGE stars at the end of each idle loop.
    (0..3)
        .map(|i| {
            let angle = -sample(cell, i, 9) * std::f32::consts::PI;
            let speed = sample(cell, i, 10) * 64.0;
            MapEmitter {
                start_ms: None,
                scale_x: None,
                scale_y: None,
                wall_mask: true,
                clip_to_chasm: false,
                cell,
                loop_ms: 792,
                blend: None,
                image: MapDraw::Blit {
                    opacity: 255,
                    tint: None,
                    glow: None,
                    asset: "specks.png",
                    source: [7, 0, 7, 7],
                    destination: [0, 0, 7, 7],
                },
                velocity: [(angle.cos() * speed) as i16, (angle.sin() * speed) as i16],
                acceleration: [0, 128],
                angular_speed: (sample(cell, i, 11) * 720.0 - 360.0) as i16,
                alpha: curve(&[[0, 0], [200, 1000], [1000, 0]]),
                scale: curve(&[[0, 1000], [1000, 0]]),
                particles: vec![MapParticle {
                    birth_ms: 0,
                    lifespan_ms: 510,
                    position: [9000, 6000],
                    scale: 1000,
                    angle: (sample(cell, i, 12) * 360.0) as u16,
                }],
            }
        })
        .collect()
}

fn pipe_drips(cell: usize) -> MapEmitter {
    MapEmitter {
        start_ms: None,
        scale_x: None,
        scale_y: None,
        wall_mask: true,
        clip_to_chasm: false,
        cell,
        loop_ms: 400,
        blend: None,
        image: MapDraw::Fill {
            rgba: [93, 143, 117, 128],
            destination: [0, 0, 2, 2],
        },
        velocity: [0, 0],
        acceleration: [0, 50],
        angular_speed: 0,
        alpha: curve(&[[0, 1000], [1000, 1000]]),
        scale: curve(&[[0, 1000], [1000, 1000]]),
        particles: (0..4)
            .map(|i| MapParticle {
                birth_ms: (i * 100) as u16,
                lifespan_ms: 400,
                angle: 0,
                position: [6500 + (sample(cell, i, 7) * 4000.0) as i32, 11500],
                scale: 1000,
            })
            .collect(),
    }
}
