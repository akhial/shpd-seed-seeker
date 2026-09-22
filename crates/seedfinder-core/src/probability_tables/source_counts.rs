//! Baked counts of co-obtainable scattered items by source and depth prefix.
//! SCF1: magic, u32 samples/profile, u32 offsets, then variable rows containing
//! u8 length and that many little-endian f32 probabilities for counts 0, 1, ….
use super::{DEPTHS, Line, kind_index, line_index, source_index};
use crate::{catalog::ItemKind, model::ItemSource};

pub const GROUPS: usize = 7;
pub const SOURCES: usize = 18;
pub const PROFILES: usize = 8;
pub const ROWS: usize = PROFILES * GROUPS * SOURCES * DEPTHS;
pub const HEADER: usize = 8 + ROWS * 4;
pub const MAGIC: &[u8; 4] = b"SCF1";
const DATA: &[u8] = include_bytes!("source_counts.bin");

#[must_use]
pub const fn group(kind: ItemKind, line: Line) -> usize {
    if matches!(kind, ItemKind::Weapon) {
        line_index(line)
    } else {
        kind_index(kind) + 2
    }
}

#[derive(Clone, Copy)]
pub(crate) struct Histogram(&'static [u8]);
impl Histogram {
    pub fn probabilities(self) -> impl Iterator<Item = (usize, f64)> {
        self.0[1..=usize::from(self.0[0]) * 4]
            .chunks_exact(4)
            .enumerate()
            .map(|(count, bytes)| {
                (
                    count,
                    f64::from(f32::from_le_bytes(bytes.try_into().unwrap())),
                )
            })
    }
}

pub(crate) fn histogram(
    profile: usize,
    kind: ItemKind,
    line: Line,
    source: ItemSource,
    depth: usize,
) -> Option<Histogram> {
    let index = ((profile * GROUPS + group(kind, line)) * SOURCES + source_index(source)) * DEPTHS
        + depth
        - 1;
    let offset = 8 + index * 4;
    let start = u32::from_le_bytes(DATA[offset..offset + 4].try_into().unwrap()) as usize;
    (start > 0).then(|| Histogram(&DATA[start..]))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn baked_distributions_are_finite_and_normalized() {
        assert_eq!(&DATA[..4], MAGIC);
        assert_eq!(u32::from_le_bytes(DATA[4..8].try_into().unwrap()), 65_536);
        let mut populated = 0;
        let mut end = HEADER;
        for row in 0..ROWS {
            let offset = 8 + row * 4;
            let start = u32::from_le_bytes(DATA[offset..offset + 4].try_into().unwrap()) as usize;
            if start == 0 {
                continue;
            }
            assert_eq!(start, end);
            let histogram = Histogram(&DATA[start..]);
            assert!(histogram.0[0] > 1);
            let mut sum = 0.0;
            for (_, p) in histogram.probabilities() {
                assert!(p.is_finite() && (0.0..=1.0).contains(&p));
                sum += p;
            }
            assert!((sum - 1.0).abs() < 1e-6);
            end = start + 1 + usize::from(histogram.0[0]) * 4;
            populated += 1;
        }
        assert!(populated > 2000);
        assert_eq!(end, DATA.len());
    }
}
