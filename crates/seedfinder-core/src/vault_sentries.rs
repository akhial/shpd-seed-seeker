//! Scouting-only snapshots of the pinned VaultLaser/VaultSentry room setup.

#[derive(Clone, Debug, Eq, PartialEq)]
#[cfg_attr(feature = "json-query", derive(serde::Serialize))]
#[cfg_attr(feature = "json-query", serde(rename_all = "camelCase"))]
pub struct VaultSentryPattern {
    pub cell: usize,
    pub initial_cooldown: u32,
    pub cooldown: u32,
    pub triggers: u16,
    pub warning: bool,
    /// Target cells, grouped by consecutive shots/scans. Lasers have one target.
    pub directions: Vec<Vec<usize>>,
    /// Absent for purple lasers. Blue scan width/length use thousandths.
    #[cfg_attr(feature = "json-query", serde(skip_serializing_if = "Option::is_none"))]
    pub scan: Option<[u32; 2]>,
}

impl VaultSentryPattern {
    pub(crate) fn laser(
        cell: usize,
        target: usize,
        initial: u32,
        cooldown: u32,
        triggers: u16,
        warning: bool,
    ) -> Self {
        Self {
            cell,
            initial_cooldown: initial,
            cooldown,
            triggers,
            warning,
            directions: vec![vec![target]],
            scan: None,
        }
    }

    fn scan(cell: usize, directions: Vec<Vec<usize>>, width: u32, length: u32) -> Self {
        Self {
            cell,
            initial_cooldown: 1,
            cooldown: 1,
            triggers: 1,
            warning: false,
            directions,
            scan: Some([width, length]),
        }
    }

    pub(crate) fn cross(cell: usize, width: i32) -> Self {
        let directions = offsets(cell, width, &[(-1, 0), (0, -1), (1, 0), (0, 1)]);
        let mut out = Self::scan(cell, directions, 90000, 4000);
        out.cooldown = 3;
        out
    }

    pub(crate) fn circle(cell: usize, width: i32, pattern: i32) -> Self {
        let (directions, degrees) = match pattern {
            0 => (
                offsets(
                    cell,
                    width,
                    &[
                        (-1, 0),
                        (-1, -1),
                        (0, -1),
                        (1, -1),
                        (1, 0),
                        (1, 1),
                        (0, 1),
                        (-1, 1),
                    ],
                ),
                90000,
            ),
            1 | 2 => {
                let mut dirs = offsets(cell, width, &HALF_CIRCLE);
                for targets in &mut dirs {
                    targets.push(2 * cell - targets[0]);
                }
                (dirs, 45000)
            }
            _ => {
                let dirs = [(-3, 0), (-3, -1), (-3, -2), (-3, -3), (-2, -3), (-1, -3)]
                    .into_iter()
                    .map(|(x, y)| {
                        offsets(cell, width, &[(x, y), (-y, x), (-x, -y), (y, -x)])
                            .into_iter()
                            .flatten()
                            .collect()
                    })
                    .collect();
                (dirs, 22500)
            }
        };
        Self::scan(cell, directions, degrees, 4490)
    }

    pub(crate) fn circle_treasure(cell: usize, width: i32, clockwise: bool) -> Self {
        let mut directions = offsets(cell, width, &HALF_CIRCLE);
        directions.extend(offsets(cell, width, &HALF_CIRCLE.map(|(x, y)| (-x, -y))));
        if !clockwise {
            directions.reverse();
        }
        Self::scan(cell, directions, 45000, 4490)
    }

    pub(crate) fn many_scans(cell: usize, target: usize) -> Self {
        Self::scan(cell, vec![vec![target]], 70000, 7000)
    }
}

const HALF_CIRCLE: [(i32, i32); 8] = [
    (-2, 0),
    (-2, -1),
    (-2, -2),
    (-1, -2),
    (0, -2),
    (1, -2),
    (2, -2),
    (2, -1),
];

fn offsets(cell: usize, width: i32, points: &[(i32, i32)]) -> Vec<Vec<usize>> {
    points
        .iter()
        .map(|&(x, y)| vec![usize::try_from(i32::try_from(cell).unwrap() + x + y * width).unwrap()])
        .collect()
}
