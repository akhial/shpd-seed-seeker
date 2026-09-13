//! The pinned `Ballistica.STOP_SOLID` and `ConeAOE` algorithms, used only by scouting.
#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_precision_loss,
    clippy::cast_sign_loss
)]
use crate::{
    geometry::shadow_caster, level::Level, level_flags::LevelFlags, level_map::MapContents,
};
use std::collections::BTreeSet;

pub(super) struct Geometry<'a> {
    level: &'a Level,
    flags: LevelFlags,
    occupied: Vec<bool>,
}
impl<'a> Geometry<'a> {
    pub(super) fn new(level: &'a Level, contents: &MapContents) -> Self {
        let mut occupied = vec![false; level.len()];
        for mob in &contents.mobs {
            occupied[mob.cell] = true;
        }
        Self {
            level,
            flags: LevelFlags::build(&level.map, false),
            occupied,
        }
    }

    /// Includes source and collision. Walls without an actor stop the ray on the
    /// previous cell; closed doors remain valid collision cells, as in Ballistica.
    pub(super) fn ray(&self, from: usize, to: usize, solid: bool, target: bool) -> Vec<usize> {
        let w = self.level.width();
        let (x0, y0) = (
            i32::try_from(from).unwrap() % w,
            i32::try_from(from).unwrap() / w,
        );
        let (x1, y1) = (
            i32::try_from(to).unwrap() % w,
            i32::try_from(to).unwrap() / w,
        );
        let (dx, dy) = ((x1 - x0).abs(), (y1 - y0).abs());
        let (sx, sy) = (if x1 > x0 { 1 } else { -1 }, if y1 > y0 { 1 } else { -1 });
        let (a, b, da, db) = if dx > dy {
            (sx, sy * w, dx, dy)
        } else {
            (sy * w, sx, dy, dx)
        };
        let mut cell = i32::try_from(from).unwrap();
        let mut err = da / 2;
        let mut path = Vec::new();
        while cell >= w
            && cell < i32::try_from(self.level.len()).unwrap() - w
            && cell % w > 0
            && cell % w < w - 1
        {
            let c = cell as usize;
            if solid
                && c != from
                && !self.flags.passable[c]
                && !self.flags.avoid[c]
                && !self.occupied[c]
            {
                break;
            }
            path.push(c);
            if (solid && c != from && self.flags.solid[c]) || (target && c == to) {
                break;
            }
            cell += a;
            err += db;
            if err >= da {
                err -= da;
                cell += b;
            }
        }
        if path.is_empty() {
            path.push(from);
        }
        path
    }

    pub(super) fn field_of_view(&self, cell: usize) -> Vec<bool> {
        let mut fov = vec![false; self.level.len()];
        shadow_caster::cast_shadow(
            i32::try_from(cell).unwrap() % self.level.width(),
            i32::try_from(cell).unwrap() / self.level.width(),
            self.level.width(),
            &mut fov,
            &self.flags.los_blocking,
            8,
        );
        fov
    }

    pub(super) fn cone(
        &self,
        from: usize,
        to: usize,
        shape: [u32; 2],
        fov: &[bool],
    ) -> BTreeSet<usize> {
        let core = self.ray(from, to, false, false);
        let end = *core.last().unwrap();
        let w = self.level.width() as usize;
        let origin = [(from % w) as f32 + 0.5, (from / w) as f32 + 0.5];
        let mut end = [(end % w) as f32 + 0.5, (end / w) as f32 + 0.5];
        let distance =
            |p: [f32; 2]| ((p[0] - origin[0]).powi(2) + (p[1] - origin[1]).powi(2)).sqrt();
        let max = shape[1] as f32 / 1000.0;
        let dist = distance(end);
        if dist > max {
            end = [
                origin[0] + (end[0] - origin[0]) * (max / dist),
                origin[1] + (end[1] - origin[1]) * (max / dist),
            ];
        }
        let radius = distance(end) + 0.5;
        let radians = std::f32::consts::PI / 180.0;
        // Match PointF's float rounding before dividing by G2R.
        let angle =
            f64::from(end[1] - origin[1]).atan2(f64::from(end[0] - origin[0])) as f32 / radians;
        let degrees = shape[0] as f32 / 1000.0;
        let mut targets = BTreeSet::new();
        let mut a = angle + degrees / 2.0;
        while a >= angle - degrees / 2.0 {
            for r in [radius, radius - 1.0]
                .into_iter()
                .take(if radius >= 4.0 { 2 } else { 1 })
            {
                let theta = f64::from(a * radians);
                let mut x = origin[0] + r * (theta.cos() as f32);
                let mut y = origin[1] + r * (theta.sin() as f32);
                x += if origin[0] > x { 0.5 } else { -0.5 };
                y += if origin[1] > y { 0.5 } else { -0.5 };
                let x = (x.floor() as i32).clamp(0, self.level.width() - 1);
                let y = (y.floor() as i32).clamp(0, self.level.height() - 1);
                targets.insert((x + y * self.level.width()) as usize);
            }
            a -= 0.5;
        }
        let mut cells = BTreeSet::new();
        for target in targets {
            cells.extend(self.ray(from, target, true, true).into_iter().skip(1));
        }
        if cells.is_empty() && core.len() >= 2 {
            cells.insert(core[1]);
        }
        cells.retain(|&cell| fov[cell]);
        cells
    }
}
