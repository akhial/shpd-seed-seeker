//! Cell-clipped object sprites. Splitting raised images preserves the cell drawing contract
//! on every frontend, including large actors crossing several neighbouring cells.
use super::{Level, MapDraw, MapLayer, MapScene, MapSprite, intern, layer};
use crate::level_map::MapContents;

pub(super) struct ActorSprite {
    pub asset: &'static str,
    pub width: u16,
    pub height: u16,
    pub frames: &'static [u16],
    pub duration: u16,
    pub raise: i32,
}

#[allow(clippy::too_many_lines)] // Ordered heap/actor composition and animation-clock packing.
pub(super) fn layers(scene: &mut MapScene, level: &Level, contents: &MapContents) -> Vec<MapLayer> {
    let mut heaps = layer("heaps", level.len());
    let mut actors: Vec<MapLayer> = vec![];
    for heap in &contents.heaps {
        if !visible(level, heap.cell) {
            continue;
        }
        let image = match heap.kind.as_str() {
            "Skeleton" => 32,
            "Tomb" => 34,
            "Chest" => 36,
            "LockedChest" => 37,
            "CrystalChest" => 38,
            _ => heap.items.first().map_or(0, |i| i.image),
        };
        let [w, h] = super::item_rects::ITEM_SIZES[usize::from(image)];
        let raise = 5 + i32::from(8_u16.saturating_sub(h));
        let sources = [[image % 16 * 16, image / 16 * 16, w, h]];
        let offset = [(17 - i32::from(w)) / 2, 16 - i32::from(h) - raise];
        shadow(
            scene,
            &mut heaps,
            level,
            heap.cell,
            "items.png",
            &sources,
            1,
            offset,
            [1.0, 0.25, 0.5],
        );
        stamp(
            scene,
            &mut heaps,
            level,
            heap.cell,
            "items.png",
            &[[image % 16 * 16, image / 16 * 16, w, h]],
            1,
            offset,
        );
    }
    for mob in &contents.mobs {
        if !visible(level, mob.cell) {
            continue;
        }
        if let Some(sprite) = super::actors::actor(&mob.kind) {
            let columns = crate::level_map::assets::get(sprite.asset)
                .expect("actor atlas")
                .width
                / sprite.width;
            let frames = if mob.kind == "ArmoredStatue" {
                let tier = mob
                    .items
                    .iter()
                    .filter_map(|item| crate::catalog::item_by_stable_id(&item.kind))
                    .find(|item| item.kind == crate::catalog::ItemKind::Armor)
                    .and_then(|item| item.tier)
                    .unwrap_or(0);
                let offset = [0, 21, 32, 43, 54, 65][usize::from(tier.min(5))];
                sprite.frames.iter().map(|frame| frame + offset).collect()
            } else if mob.stealthy && mob.kind.ends_with("Mimic") {
                vec![sprite.frames[0] - 1]
            } else {
                sprite.frames.to_vec()
            };
            let sources: Vec<_> = frames
                .iter()
                .map(|&frame| {
                    [
                        frame % columns * sprite.width,
                        frame / columns * sprite.height,
                        sprite.width,
                        sprite.height,
                    ]
                })
                .collect();
            let mut target = layer("actors", level.len());
            let offset = [
                (17 - i32::from(sprite.width)).div_euclid(2),
                16 - i32::from(sprite.height) - sprite.raise,
            ];
            if !matches!(
                mob.kind.as_str(),
                "Piranha"
                    | "PhantomPiranha"
                    | "Spinner"
                    | "FungalSpinner"
                    | "RotHeart"
                    | "Pylon"
                    | "VaultMirror"
                    | "VaultTokenDoor"
            ) {
                let shape = if mob.kind.ends_with("Mimic") {
                    [1.0, 0.25, -0.4]
                } else if mob.kind == "DemonSpawner" {
                    [1.0, 0.4, 1.25]
                } else {
                    [1.2, 0.25, 0.25]
                };
                shadow(
                    scene,
                    &mut target,
                    level,
                    mob.cell,
                    sprite.asset,
                    &sources,
                    sprite.duration,
                    offset,
                    shape,
                );
            }
            stamp(
                scene,
                &mut target,
                level,
                mob.cell,
                sprite.asset,
                &sources,
                sprite.duration,
                offset,
            );
            // Overlapping actors must keep independent animation clocks. Pack
            // disjoint sprites together, above any earlier overlapping actor.
            let index = actors
                .iter()
                .rposition(|layer| {
                    layer
                        .cells
                        .iter()
                        .zip(&target.cells)
                        .any(|(a, b)| a.is_some() && b.is_some())
                })
                .map_or(0, |index| index + 1);
            if index == actors.len() {
                actors.push(layer("actors", level.len()));
            }
            for (dest, src) in actors[index].cells.iter_mut().zip(target.cells) {
                if src.is_some() {
                    *dest = src;
                }
            }
        }
    }
    let mut layers = vec![heaps];
    layers.extend(actors);
    layers
}

pub(super) fn visible(level: &Level, cell: usize) -> bool {
    cell < level.len() && !crate::level_map::projection::wall(level.map.cells[cell])
}

/// Draw a sprite with signed pixel offset, splitting all source/destination
/// rectangles at cell boundaries. Animations in a layer share their clock.
#[allow(clippy::too_many_arguments)]
pub(super) fn stamp(
    scene: &mut MapScene,
    target: &mut MapLayer,
    level: &Level,
    cell: usize,
    asset: &'static str,
    sources: &[[u16; 4]],
    duration: u16,
    offset: [i32; 2],
) {
    stamp_scaled(
        scene,
        target,
        level,
        cell,
        asset,
        sources,
        duration,
        offset,
        [sources[0][2], sources[0][3]],
        false,
    );
}

#[allow(
    clippy::too_many_arguments,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss
)]
fn shadow(
    scene: &mut MapScene,
    target: &mut MapLayer,
    level: &Level,
    cell: usize,
    asset: &'static str,
    sources: &[[u16; 4]],
    duration: u16,
    offset: [i32; 2],
    shape: [f32; 3],
) {
    let [_, _, w, h] = sources[0];
    let [width, height, raise] = shape;
    stamp_scaled(
        scene,
        target,
        level,
        cell,
        asset,
        sources,
        duration,
        [
            offset[0] + (f32::from(w) * (1.0 - width) / 2.0).round() as i32,
            offset[1] + (f32::from(h) * (1.0 - height) + raise).round() as i32,
        ],
        [
            (f32::from(w) * width).round() as u16,
            (f32::from(h) * height).round() as u16,
        ],
        true,
    );
}

#[allow(clippy::too_many_arguments)]
fn stamp_scaled(
    scene: &mut MapScene,
    target: &mut MapLayer,
    level: &Level,
    cell: usize,
    asset: &'static str,
    sources: &[[u16; 4]],
    duration: u16,
    offset: [i32; 2],
    size: [u16; 2],
    shadow: bool,
) {
    let point = level.map.cell_to_point(cell);
    let [w, h] = size;
    let [ox, oy] = offset;
    for cy in oy.div_euclid(16)..=(oy + i32::from(h) - 1).div_euclid(16) {
        for cx in ox.div_euclid(16)..=(ox + i32::from(w) - 1).div_euclid(16) {
            let x = point.x + cx;
            let y = point.y + cy;
            if x < 0 || y < 0 || x >= level.width() || y >= level.height() {
                continue;
            }
            let dx = ox.max(cx * 16);
            let dy = oy.max(cy * 16);
            let rw = (ox + i32::from(w)).min((cx + 1) * 16) - dx;
            let rh = (oy + i32::from(h)).min((cy + 1) * 16) - dy;
            let source_width = (dx - ox + rw) * i32::from(sources[0][2]) / i32::from(w)
                - (dx - ox) * i32::from(sources[0][2]) / i32::from(w);
            let source_height = (dy - oy + rh) * i32::from(sources[0][3]) / i32::from(h)
                - (dy - oy) * i32::from(sources[0][3]) / i32::from(h);
            if source_width == 0 || source_height == 0 {
                continue;
            }
            let frames = sources
                .iter()
                .map(|s| {
                    vec![MapDraw::Blit {
                        opacity: if shadow { 153 } else { 255 },
                        tint: shadow.then_some([0, 0, 0]),
                        asset,
                        source: [
                            s[0] + u16::try_from((dx - ox) * i32::from(s[2]) / i32::from(w))
                                .unwrap(),
                            s[1] + u16::try_from((dy - oy) * i32::from(s[3]) / i32::from(h))
                                .unwrap(),
                            u16::try_from(
                                (dx - ox + rw) * i32::from(s[2]) / i32::from(w)
                                    - (dx - ox) * i32::from(s[2]) / i32::from(w),
                            )
                            .unwrap(),
                            u16::try_from(
                                (dy - oy + rh) * i32::from(s[3]) / i32::from(h)
                                    - (dy - oy) * i32::from(s[3]) / i32::from(h),
                            )
                            .unwrap(),
                        ],
                        destination: [
                            u16::try_from(dx - cx * 16).unwrap(),
                            u16::try_from(dy - cy * 16).unwrap(),
                            u16::try_from(rw).unwrap(),
                            u16::try_from(rh).unwrap(),
                        ],
                    }]
                })
                .collect();
            let index = usize::try_from(x + y * level.width()).unwrap();
            append(
                scene,
                target,
                index,
                MapSprite {
                    frame_duration_ms: duration,
                    frames,
                },
            );
        }
    }
}

pub(super) fn append(scene: &mut MapScene, target: &mut MapLayer, cell: usize, sprite: MapSprite) {
    let result = if let Some(existing) = target.cells[cell] {
        let previous = &scene.sprites[existing];
        assert!(
            previous.frames.len() == 1
                || sprite.frames.len() == 1
                || (previous.frames.len() == sprite.frames.len()
                    && previous.frame_duration_ms == sprite.frame_duration_ms)
        );
        MapSprite {
            frame_duration_ms: if sprite.frames.len() > 1 {
                sprite.frame_duration_ms
            } else {
                previous.frame_duration_ms
            },
            frames: (0..previous.frames.len().max(sprite.frames.len()))
                .map(|i| {
                    let mut commands = previous.frames[i % previous.frames.len()].clone();
                    commands.extend_from_slice(&sprite.frames[i % sprite.frames.len()]);
                    commands
                })
                .collect(),
        }
    } else {
        sprite
    };
    target.cells[cell] = Some(intern(scene, result));
}
