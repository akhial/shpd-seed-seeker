// SPDX-License-Identifier: GPL-3.0-or-later

//! Native Cairo compositor for the engine's raised sprite scene.
#![allow(
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss
)]
// The sprite contract uses conventional XY/WH rectangles and RGB channels.
#![allow(clippy::many_single_char_names)]

use std::cell::RefCell;
use std::collections::HashMap;

use gtk::prelude::*;
use gtk::{cairo, gdk, glib};
use shpd_seedfinder_core::level_map::{
    self, LevelMap, MapBlend, MapCurve, MapDraw, MapEmitter, MapParticle,
};

type TextureKey = (&'static str, [u16; 4], Option<[u8; 3]>);
thread_local! {
    // Assets are revision-pinned into this executable. Crops are shared between maps.
    static TEXTURES: RefCell<HashMap<TextureKey, cairo::ImageSurface>> = RefCell::default();
    static ASSETS: RefCell<HashMap<&'static str, (Vec<u8>, usize)>> = RefCell::default();
}

fn surface(width: i32, height: i32) -> Result<cairo::ImageSurface, String> {
    cairo::ImageSurface::create(cairo::Format::ARgb32, width, height).map_err(|e| e.to_string())
}

fn texture(key: TextureKey) -> Result<cairo::ImageSurface, String> {
    TEXTURES.with(|cache| {
        if let Some(image) = cache.borrow().get(&key) {
            return Ok(image.clone());
        }
        let image = ASSETS.with(|assets| {
            let mut assets = assets.borrow_mut();
            if !assets.contains_key(key.0) {
                let asset = level_map::assets::get(key.0).ok_or("Unknown map texture")?;
                let texture = gdk::Texture::from_bytes(&glib::Bytes::from_static(asset.png))
                    .map_err(|e| e.to_string())?;
                let stride = texture.width() as usize * 4;
                let mut pixels = vec![0; stride * texture.height() as usize];
                texture.download(&mut pixels, stride);
                assets.insert(key.0, (pixels, stride));
            }
            let (pixels, stride) = &assets[key.0];
            let [x, y, w, h] = key.1.map(usize::from);
            let mut image = surface(i32::from(key.1[2]), i32::from(key.1[3]))?;
            let target_stride = image.stride() as usize;
            {
                let mut data = image.data().map_err(|e| e.to_string())?;
                for row in 0..h {
                    for col in 0..w {
                        let offset = (y + row) * stride + (x + col) * 4;
                        let mut word =
                            u32::from_ne_bytes(pixels[offset..offset + 4].try_into().unwrap());
                        if let Some(tint) = key.2 {
                            let mut tinted = word & 0xff00_0000;
                            for (channel, shift) in tint.into_iter().zip([16, 8, 0]) {
                                tinted |=
                                    (((word >> shift) & 255) * u32::from(channel) / 255) << shift;
                            }
                            word = tinted;
                        }
                        let offset = row * target_stride + col * 4;
                        data[offset..offset + 4].copy_from_slice(&word.to_ne_bytes());
                    }
                }
            }
            Ok::<_, String>(image)
        })?;
        // A bounded crop cache avoids retaining every map ever explored.
        if cache.borrow().len() >= 4096 {
            cache.borrow_mut().clear();
        }
        cache.borrow_mut().insert(key, image.clone());
        Ok(image)
    })
}

fn draw(context: &cairo::Context, command: &MapDraw, elapsed: u64) -> Result<(), String> {
    context.save().map_err(|e| e.to_string())?;
    let result = (|| {
        match command {
            MapDraw::Fill { destination, rgba } => {
                let [x, y, w, h] = destination.map(f64::from);
                let [r, g, b, a] = rgba.map(|v| f64::from(v) / 255.0);
                context.set_source_rgba(r, g, b, a);
                context.rectangle(x, y, w, h);
                context.fill().map_err(|e| e.to_string())?;
            }
            MapDraw::Blit {
                asset,
                source,
                destination,
                tint,
                glow,
                opacity,
            } => {
                let image = texture((asset, *source, *tint))?;
                let [x, y, w, h] = destination.map(f64::from);
                context.translate(x, y);
                context.rectangle(0.0, 0.0, w, h);
                context.clip();
                context.scale(w / f64::from(source[2]), h / f64::from(source[3]));
                let operator = context.operator();
                context.push_group();
                context.set_operator(cairo::Operator::Over);
                context
                    .set_source_surface(&image, 0.0, 0.0)
                    .map_err(|e| e.to_string())?;
                context.source().set_filter(cairo::Filter::Nearest);
                context.paint().map_err(|e| e.to_string())?;
                if let Some(glow) = glow {
                    let phase = (elapsed as f64 / f64::from(glow.period_ms.max(1))) % 2.0;
                    let value = phase.min(2.0 - phase) * 0.6;
                    let [r, g, b] = glow.color.map(|v| f64::from(v) / 255.0);
                    context.set_operator(cairo::Operator::Atop);
                    context.set_source_rgba(r, g, b, value);
                    context.paint().map_err(|e| e.to_string())?;
                }
                context.pop_group_to_source().map_err(|e| e.to_string())?;
                context.set_operator(operator);
                context
                    .paint_with_alpha(f64::from(*opacity) / 255.0)
                    .map_err(|e| e.to_string())?;
            }
        }
        Ok(())
    })();
    let restored = context.restore().map_err(|e| e.to_string());
    result.and(restored)
}

pub fn curve_value(curve: &MapCurve, progress: f64) -> f64 {
    let p = progress.clamp(0.0, 1.0) * 1000.0;
    let mut value = curve.points.last().map_or(0.0, |point| f64::from(point[1]));
    for (index, [x, y]) in curve.points.iter().enumerate() {
        if f64::from(*x) >= p {
            value = if index == 0 {
                f64::from(*y)
            } else {
                let [x0, y0] = curve.points[index - 1];
                f64::from(y0)
                    + (f64::from(*y) - f64::from(y0)) * (p - f64::from(x0))
                        / f64::from((*x - x0).max(1))
            };
            break;
        }
    }
    value /= 1000.0;
    if curve.sqrt { value.sqrt() } else { value }
}

#[derive(Debug)]
pub struct ParticleState {
    pub x: f64,
    pub y: f64,
    pub scale: f64,
    pub scale_y: f64,
    pub alpha: f64,
    pub angle: f64,
}

pub fn particle_state(
    emitter: &MapEmitter,
    particle: &MapParticle,
    elapsed: u64,
) -> Option<ParticleState> {
    let clock = elapsed as f64 - f64::from(emitter.start_ms.unwrap_or(0));
    if emitter.start_ms.is_some() && clock < f64::from(particle.birth_ms) {
        return None;
    }
    let age = (clock - f64::from(particle.birth_ms)).rem_euclid(f64::from(emitter.loop_ms.max(1)));
    if age >= f64::from(particle.lifespan_ms) {
        return None;
    }
    let progress = age / f64::from(particle.lifespan_ms);
    let seconds = age / 1000.0;
    let position = |axis| {
        f64::from(particle.position[axis]) / 1000.0
            + f64::from(emitter.velocity[axis]) * seconds
            + f64::from(emitter.acceleration[axis]) * seconds * seconds / 2.0
    };
    Some(ParticleState {
        x: position(0),
        y: position(1),
        scale: f64::from(particle.scale) / 1000.0 * curve_value(&emitter.scale, progress),
        scale_y: emitter
            .scale_y
            .as_ref()
            .map_or(1.0, |curve| curve_value(curve, progress)),
        alpha: curve_value(&emitter.alpha, progress),
        angle: (f64::from(particle.angle) + f64::from(emitter.angular_speed) * seconds)
            .to_radians(),
    })
}

pub struct Renderer {
    scenery: cairo::ImageSurface,
    walls: cairo::ImageSurface,
    darkness: cairo::ImageSurface,
    secrets: Option<bool>,
    frames: Vec<usize>,
    glows: Vec<bool>,
}

impl Renderer {
    pub fn new(map: &LevelMap) -> Result<Self, String> {
        Ok(Self {
            scenery: surface(map.width * 16, map.height * 16)?,
            walls: surface(map.width * 16, map.height * 16)?,
            darkness: surface(map.width * 16, map.height * 16)?,
            secrets: None,
            frames: vec![usize::MAX; map.scene.sprites.len()],
            glows: map
                .scene
                .sprites
                .iter()
                .map(|s| {
                    s.frames
                        .iter()
                        .flatten()
                        .any(|d| matches!(d, MapDraw::Blit { glow: Some(_), .. }))
                })
                .collect(),
        })
    }

    #[allow(clippy::too_many_lines)] // Ordered scenery, world-particle and status passes.
    pub fn render(
        &mut self,
        map: &LevelMap,
        secrets: bool,
        elapsed: u64,
        target: &cairo::Context,
    ) -> Result<(), String> {
        let changed_mode = self.secrets != Some(secrets);
        let layers = if secrets {
            &map.scene.layers
        } else {
            &map.scene.concealed_layers
        };
        let emitters = if secrets {
            &map.scene.emitters
        } else {
            &map.scene.concealed_emitters
        };
        let mut changed = vec![changed_mode; self.frames.len()];
        for (index, sprite) in map.scene.sprites.iter().enumerate() {
            let frame = (elapsed / u64::from(sprite.frame_duration_ms.max(1))) as usize
                % sprite.frames.len().max(1);
            changed[index] |= self.frames[index] != frame || self.glows[index];
            self.frames[index] = frame;
        }
        let scenery = cairo::Context::new(&self.scenery).map_err(|e| e.to_string())?;
        // Commands are cell bounded: replay just damaged cells through every layer.
        for cell in 0..(map.width * map.height) as usize {
            if !changed_mode
                && !layers
                    .iter()
                    .any(|l| l.cells[cell].is_some_and(|s| changed[s]))
            {
                continue;
            }
            scenery.save().map_err(|e| e.to_string())?;
            scenery.translate(
                (cell % map.width as usize) as f64 * 16.0,
                (cell / map.width as usize) as f64 * 16.0,
            );
            scenery.set_operator(cairo::Operator::Source);
            scenery.set_source_rgb(0.0, 0.0, 0.0);
            scenery.rectangle(0.0, 0.0, 16.0, 16.0);
            scenery.fill().map_err(|e| e.to_string())?;
            for layer in layers {
                scenery.set_operator(if layer.blend == Some(MapBlend::Add) {
                    cairo::Operator::Add
                } else {
                    cairo::Operator::Over
                });
                if let Some(sprite) = layer.cells[cell] {
                    for command in map.scene.sprites[sprite].frame(elapsed) {
                        draw(&scenery, command, elapsed)?;
                    }
                }
            }
            scenery.restore().map_err(|e| e.to_string())?;
        }
        if changed_mode {
            for (surface, names) in [
                (
                    &self.walls,
                    &["raised", "walls", "room_walls", "boss_walls"][..],
                ),
                (&self.darkness, &["darkness"][..]),
            ] {
                let context = cairo::Context::new(surface).map_err(|e| e.to_string())?;
                context.set_operator(cairo::Operator::Clear);
                context.paint().map_err(|e| e.to_string())?;
                context.set_operator(cairo::Operator::Over);
                for layer in layers.iter().filter(|l| names.contains(&l.name)) {
                    for (cell, sprite) in layer.cells.iter().enumerate() {
                        if let Some(sprite) = sprite {
                            context.save().map_err(|e| e.to_string())?;
                            context.translate(
                                (cell % map.width as usize) as f64 * 16.0,
                                (cell / map.width as usize) as f64 * 16.0,
                            );
                            for command in map.scene.sprites[*sprite].frame(0) {
                                draw(&context, command, 0)?;
                            }
                            context.restore().map_err(|e| e.to_string())?;
                        }
                    }
                }
            }
            self.secrets = Some(secrets);
        }
        target
            .set_source_surface(&self.scenery, 0.0, 0.0)
            .map_err(|e| e.to_string())?;
        target.source().set_filter(cairo::Filter::Nearest);
        target.paint().map_err(|e| e.to_string())?;
        if emitters.is_empty() {
            return Ok(());
        }
        // Particles draw at the viewport's device resolution so fractional
        // motion remains continuous when the pixel-art scenery is enlarged.
        target.save().map_err(|e| e.to_string())?;
        target.rectangle(
            0.0,
            0.0,
            f64::from(map.width * 16),
            f64::from(map.height * 16),
        );
        target.clip();
        target.push_group();
        let context = target;
        context.set_operator(cairo::Operator::Source);
        context
            .set_source_surface(&self.scenery, 0.0, 0.0)
            .map_err(|e| e.to_string())?;
        context.source().set_filter(cairo::Filter::Nearest);
        context.paint().map_err(|e| e.to_string())?;
        context.save().map_err(|e| e.to_string())?;
        for emitter in emitters.iter().filter(|e| e.clip_to_chasm) {
            context.rectangle(
                (emitter.cell % map.width as usize) as f64 * 16.0,
                (emitter.cell / map.width as usize) as f64 * 16.0,
                16.0,
                16.0,
            );
        }
        context.clip();
        for emitter in emitters.iter().filter(|e| e.clip_to_chasm) {
            draw_emitter(context, map, emitter, elapsed)?;
        }
        context.restore().map_err(|e| e.to_string())?;
        for emitter in emitters.iter().filter(|e| e.wall_mask && !e.clip_to_chasm) {
            draw_emitter(context, map, emitter, elapsed)?;
        }
        erase(context, &self.walls)?;
        for emitter in emitters.iter().filter(|e| !e.wall_mask && !e.clip_to_chasm) {
            draw_emitter(context, map, emitter, elapsed)?;
        }
        erase(context, &self.darkness)?;
        target.pop_group_to_source().map_err(|e| e.to_string())?;
        target.set_operator(cairo::Operator::Over);
        target.paint().map_err(|e| e.to_string())?;
        target.restore().map_err(|e| e.to_string())
    }
}

fn erase(context: &cairo::Context, mask: &cairo::ImageSurface) -> Result<(), String> {
    context.set_operator(cairo::Operator::DestOut);
    context
        .set_source_surface(mask, 0.0, 0.0)
        .map_err(|e| e.to_string())?;
    context.source().set_filter(cairo::Filter::Nearest);
    context.paint().map_err(|e| e.to_string())
}

fn draw_emitter(
    context: &cairo::Context,
    map: &LevelMap,
    emitter: &MapEmitter,
    elapsed: u64,
) -> Result<(), String> {
    for particle in &emitter.particles {
        let Some(state) = particle_state(emitter, particle, elapsed) else {
            continue;
        };
        if state.scale <= 0.0 || state.scale_y <= 0.0 || state.alpha <= 0.0 {
            continue;
        }
        context.save().map_err(|e| e.to_string())?;
        context.translate(
            (emitter.cell % map.width as usize) as f64 * 16.0 + state.x,
            (emitter.cell / map.width as usize) as f64 * 16.0 + state.y,
        );
        context.rotate(state.angle);
        context.scale(state.scale, state.scale * state.scale_y);
        let destination = match &emitter.image {
            MapDraw::Blit { destination, .. } | MapDraw::Fill { destination, .. } => destination,
        };
        context.translate(
            -f64::from(destination[0]) - f64::from(destination[2]) / 2.0,
            -f64::from(destination[1]) - f64::from(destination[3]) / 2.0,
        );
        let [x, y, w, h] = destination.map(f64::from);
        context.rectangle(x, y, w, h);
        context.clip();
        context.push_group();
        context.set_operator(cairo::Operator::Over);
        draw(context, &emitter.image, elapsed)?;
        context.pop_group_to_source().map_err(|e| e.to_string())?;
        context.set_operator(if emitter.blend == Some(MapBlend::Add) {
            cairo::Operator::Add
        } else {
            cairo::Operator::Over
        });
        context
            .paint_with_alpha(state.alpha)
            .map_err(|e| e.to_string())?;
        context.restore().map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use shpd_seedfinder_core::level_map::{MapContents, MapKind, MapLayer, MapScene, MapSprite};

    fn constant(value: u16) -> MapCurve {
        MapCurve {
            points: vec![[0, value], [1000, value]],
            sqrt: false,
        }
    }

    fn emitter() -> MapEmitter {
        MapEmitter {
            start_ms: None,
            wall_mask: true,
            clip_to_chasm: false,
            cell: 0,
            loop_ms: 1000,
            blend: Some(MapBlend::Add),
            image: MapDraw::Fill {
                destination: [0, 0, 16, 16],
                rgba: [40, 0, 0, 255],
            },
            velocity: [0, 0],
            acceleration: [0, 0],
            angular_speed: 0,
            alpha: constant(1000),
            scale: constant(1000),
            scale_y: None,
            particles: vec![MapParticle {
                birth_ms: 0,
                angle: 0,
                lifespan_ms: 1000,
                position: [8000, 8000],
                scale: 1000,
            }],
        }
    }

    fn map() -> LevelMap {
        let base = MapLayer {
            name: "terrain",
            blend: None,
            cells: vec![Some(0)],
        };
        let wall = MapLayer {
            name: "walls",
            blend: None,
            cells: vec![Some(1)],
        };
        let darkness = MapLayer {
            name: "darkness",
            blend: None,
            cells: vec![Some(2)],
        };
        LevelMap {
            seed: shpd_seedfinder_core::seed::DungeonSeed::from_code("AAA-AAA-AAA").unwrap(),
            depth: 1,
            kind: MapKind::Regular,
            branches: vec![],
            challenges: shpd_seedfinder_core::challenges::Challenges::NONE,
            selected_trinket: None,
            feeling: shpd_seedfinder_core::level::Feeling::None,
            width: 1,
            height: 1,
            terrain: vec![0],
            entrance: None,
            exit: None,
            secret_rooms: vec![],
            traps: vec![],
            contents: MapContents::default(),
            scene: MapScene {
                tile_size: 16,
                sprites: vec![
                    MapSprite {
                        frame_duration_ms: 100,
                        frames: vec![
                            vec![MapDraw::Fill {
                                destination: [0, 0, 16, 16],
                                rgba: [20, 30, 40, 255],
                            }],
                            vec![MapDraw::Fill {
                                destination: [0, 0, 16, 16],
                                rgba: [30, 40, 50, 255],
                            }],
                        ],
                    },
                    MapSprite {
                        frame_duration_ms: 1,
                        frames: vec![vec![MapDraw::Fill {
                            destination: [0, 0, 8, 16],
                            rgba: [0, 80, 0, 255],
                        }]],
                    },
                    MapSprite {
                        frame_duration_ms: 1,
                        frames: vec![vec![MapDraw::Fill {
                            destination: [0, 0, 16, 4],
                            rgba: [0, 0, 0, 255],
                        }]],
                    },
                ],
                layers: vec![base.clone(), wall.clone()],
                concealed_layers: vec![base, wall, darkness],
                emitters: vec![emitter()],
                concealed_emitters: vec![emitter()],
            },
        }
    }

    fn pixel(image: &mut cairo::ImageSurface, x: usize, y: usize) -> u32 {
        let offset = y * image.stride() as usize + x * 4;
        u32::from_ne_bytes(
            image.data().unwrap()[offset..offset + 4]
                .try_into()
                .unwrap(),
        )
    }

    #[test]
    fn compositor_preserves_additive_scenery_wall_masks_and_secret_darkness() {
        let map = map();
        let mut renderer = Renderer::new(&map).unwrap();
        let mut image = surface(16, 16).unwrap();
        {
            let context = cairo::Context::new(&image).unwrap();
            renderer.render(&map, false, 0, &context).unwrap();
        }
        assert_eq!(pixel(&mut image, 12, 8), 0xff3c_1e28); // Red light adds to ground.
        assert_eq!(pixel(&mut image, 4, 8), 0xff00_5000); // Opaque wall occludes light.
        assert_eq!(pixel(&mut image, 12, 2), 0xff00_0000); // Secrets stay black.
        {
            let context = cairo::Context::new(&image).unwrap();
            renderer.render(&map, true, 100, &context).unwrap();
        }
        assert_eq!(pixel(&mut image, 12, 2), 0xff46_2832); // Reveal switches entire stack; frame advances.
        assert_eq!(pixel(&mut image, 4, 8), 0xff00_5000);
    }

    #[test]
    fn chasm_wind_clips_motion_and_status_icons_stay_above_walls() {
        let mut map = map();
        map.scene.emitters[0].clip_to_chasm = true;
        map.scene.emitters[0].particles[0].position = [18000, 8000];
        let mut status = emitter();
        status.wall_mask = false;
        status.blend = None;
        status.image = MapDraw::Fill {
            destination: [0, 0, 2, 2],
            rgba: [0, 0, 255, 255],
        };
        status.particles[0].position = [4000, 8000];
        map.scene.emitters.push(status);
        let mut renderer = Renderer::new(&map).unwrap();
        let mut image = surface(32, 16).unwrap();
        {
            let context = cairo::Context::new(&image).unwrap();
            renderer.render(&map, true, 0, &context).unwrap();
        }
        assert_eq!(pixel(&mut image, 4, 8), 0xff00_00ff);
        assert_eq!(pixel(&mut image, 15, 8), 0xff3c_1e28);
        assert_eq!(pixel(&mut image, 17, 8), 0);
    }

    #[test]
    fn zoomed_particles_move_between_map_pixels() {
        let mut map = map();
        map.scene.emitters[0].wall_mask = false;
        map.scene.emitters[0].blend = None;
        map.scene.emitters[0].velocity = [1, 0];
        map.scene.emitters[0].image = MapDraw::Fill {
            destination: [0, 0, 1, 1],
            rgba: [0, 0, 255, 255],
        };
        let mut renderer = Renderer::new(&map).unwrap();
        let mut image = surface(64, 64).unwrap();
        for (elapsed, expected) in [(0, 0xff00_00ff), (250, 0xff00_5000)] {
            {
                let context = cairo::Context::new(&image).unwrap();
                context.scale(4.0, 4.0);
                renderer.render(&map, true, elapsed, &context).unwrap();
            }
            assert_eq!(pixel(&mut image, 30, 32), expected);
        }
    }

    #[test]
    fn scheduled_particles_do_not_wrap_before_first_birth_and_beams_thin() {
        let mut e = emitter();
        e.start_ms = Some(2000);
        e.particles[0].birth_ms = 200;
        e.velocity = [10, -4];
        e.acceleration = [0, 8];
        e.angular_speed = 90;
        e.scale_y = Some(MapCurve {
            points: vec![[0, 1000], [1000, 0]],
            sqrt: false,
        });
        assert!(particle_state(&e, &e.particles[0], 2199).is_none());
        let p = particle_state(&e, &e.particles[0], 2700).unwrap();
        assert!((p.x - 13.0).abs() < 1e-9);
        assert!((p.y - 7.0).abs() < 1e-9);
        assert!((p.scale_y - 0.5).abs() < 1e-9);
        assert!((p.angle - std::f64::consts::FRAC_PI_4).abs() < 1e-9);
        assert!(
            (curve_value(
                &MapCurve {
                    points: vec![[0, 0], [1000, 1000]],
                    sqrt: true
                },
                0.25
            ) - 0.5)
                .abs()
                < 1e-9
        );
        e.start_ms = None;
        assert!(particle_state(&e, &e.particles[0], 0).is_some()); // Ambient effects prewarm.
    }
}
