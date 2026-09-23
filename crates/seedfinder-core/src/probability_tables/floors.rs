//! Compact, measured floor distributions. Shared with the offline calibrator.
//!
//! Layout: magic, u32 samples/profile, then u32 offsets to 8 * 20 * 4 room
//! rows; feeling counts, same-room cross-floor counts, cross-floor feeling
//! counts (all u32), then variable room rows. A room row stores its u32 sample
//! count, u32 marginals, u8 indices of nonconstant variables, and a packed
//! lower triangle of u32 intersections. Constant variables need no pairs.
//! Every number is little endian. The embedded table is decompressed once on
//! first use; individual lookups need no allocation or decompression.
#![allow(clippy::float_cmp)] // Counts are u32 integers represented exactly as f64.

use crate::{
    floor_filters::{RoomSet, RoomType},
    level_prelude::Feeling,
};

const _: () = assert!(
    RoomType::ALL.len() == 95,
    "regenerate floor tables after adding rooms"
);

pub const PROFILES: usize = 8;
pub const FLOORS: usize = 20;
pub const GROUPS: usize = 4;
pub const FEATURES: usize = RoomType::ALL.len() + 1;
pub const GARDENS: usize = FEATURES - 1;
pub const ROWS: usize = PROFILES * FLOORS * GROUPS;
pub const PAIRS: usize = FLOORS * (FLOORS - 1) / 2;
pub const FEELINGS_OFFSET: usize = 8 + ROWS * 4;
pub const ROOMS_OFFSET: usize = FEELINGS_OFFSET + PROFILES * FLOORS * 8 * 4;
pub const CROSS_FEELINGS_OFFSET: usize = ROOMS_OFFSET + PROFILES * PAIRS * FEATURES * 4;
pub const DATA_OFFSET: usize = CROSS_FEELINGS_OFFSET + PROFILES * PAIRS * 64 * 4;
pub const MAGIC: &[u8; 4] = b"FLP4";

#[must_use]
pub const fn floor_index(depth: u8) -> usize {
    (depth - 1 - depth / 5) as usize
}

#[must_use]
pub const fn triangle(a: usize, b: usize) -> usize {
    let (high, low) = if a > b { (a, b) } else { (b, a) };
    high * (high - 1) / 2 + low
}

/// Only Large and Secrets change room scheduling. Pool the ordinary feelings
/// to reduce sampling noise (painting and loot still differ between them).
/// With feeling overrides, Normal identifies worlds before brewing, so keep
/// it separate: conditioning on it also conditions laboratory placement.
#[must_use]
pub const fn group(profile: usize, feeling: Feeling) -> usize {
    match feeling {
        Feeling::None if profile == 5 || profile == 6 => 3,
        Feeling::Large => 1,
        Feeling::Secrets => 2,
        _ => 0,
    }
}

#[must_use]
pub fn features(rooms: RoomSet) -> u128 {
    let gardens = RoomSet::from_types([RoomType::SpecialGarden, RoomType::SecretGarden]);
    rooms.0
        | if rooms.0 & gardens.0 == 0 {
            0
        } else {
            1 << GARDENS
        }
}

fn u32_at(bytes: &[u8], offset: usize) -> f64 {
    f64::from(u32::from_le_bytes(
        bytes[offset..offset + 4].try_into().unwrap(),
    ))
}

#[derive(Clone, Copy)]
pub(crate) struct Tables(pub &'static [u8]);
impl Tables {
    #[must_use]
    pub fn samples(self) -> f64 {
        f64::from(u32::from_le_bytes(self.0[4..8].try_into().unwrap()))
    }

    #[must_use]
    pub fn feeling(self, profile: usize, floor: usize, feeling: usize) -> f64 {
        u32_at(
            self.0,
            FEELINGS_OFFSET + ((profile * FLOORS + floor) * 8 + feeling) * 4,
        )
    }

    #[must_use]
    pub fn row(self, profile: usize, floor: usize, group: usize) -> RoomRow {
        RoomRow::at(self.0, (profile * FLOORS + floor) * GROUPS + group)
    }

    /// FLX1 stores exact-feeling rows for profiles 5 and 6. The sample header
    /// and variable room rows have the same encoding as FLP4.
    pub fn exact_room(self, profile: usize, floor: usize, feeling: usize) -> RoomRow {
        debug_assert!((5..=6).contains(&profile));
        RoomRow::at(self.0, ((profile - 5) * FLOORS + floor) * 8 + feeling)
    }

    #[must_use]
    pub fn cross_room(self, profile: usize, a: usize, b: usize, feature: usize) -> f64 {
        u32_at(
            self.0,
            ROOMS_OFFSET + ((profile * PAIRS + triangle(a, b)) * FEATURES + feature) * 4,
        )
    }

    #[must_use]
    pub fn cross_feeling(self, profile: usize, a: usize, b: usize, fa: usize, fb: usize) -> f64 {
        let (low, high) = if a < b { (fa, fb) } else { (fb, fa) };
        u32_at(
            self.0,
            CROSS_FEELINGS_OFFSET + ((profile * PAIRS + triangle(a, b)) * 64 + low * 8 + high) * 4,
        )
    }
}

#[derive(Clone, Copy)]
pub(crate) struct RoomRow(&'static [u8]);
impl RoomRow {
    fn at(bytes: &'static [u8], index: usize) -> Self {
        let offset = 8 + index * 4;
        let start = u32::from_le_bytes(bytes[offset..offset + 4].try_into().unwrap()) as usize;
        Self(&bytes[start..])
    }
    #[must_use]
    pub fn samples(self) -> f64 {
        u32_at(self.0, 0)
    }

    #[must_use]
    pub fn count(self, room: usize) -> f64 {
        u32_at(self.0, 4 + room * 4)
    }

    #[must_use]
    pub fn pair(self, a: usize, b: usize) -> f64 {
        let (ca, cb, samples) = (self.count(a), self.count(b), self.samples());
        if a == b || ca == samples || cb == samples || ca == 0.0 || cb == 0.0 {
            return ca.min(cb);
        }
        let a = usize::from(self.0[4 + FEATURES * 4 + a]);
        let b = usize::from(self.0[4 + FEATURES * 4 + b]);
        u32_at(self.0, 4 + FEATURES * 5 + triangle(a, b) * 4)
    }
}
