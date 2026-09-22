//! Measured room presence conditional on depth, feeling, and +3 trinket profile.
//! The two garden classes use their measured intersection, so an either-garden
//! query counts floors with both gardens only once. Other room conjunctions and
//! unions use conditional independence. Different floors and item supply are
//! also approximated as independent; this is an estimate, never a pruning rule.
//! Challenges use the canonical profile, as the item estimator does.
use crate::floor_filters::{FloorRequirement, RoomSet, RoomType};
use crate::probability_tables::trinkets::Profile;
use crate::query::SearchQuery;

const COLUMNS: usize = RoomType::ALL.len() + 2;
const TABLE: &[u8; 8 * 24 * 8 * COLUMNS * 2] = include_bytes!("../probability_tables/floors.bin");

fn count(profile: Profile, depth: u8, feeling: usize, column: usize) -> f64 {
    let offset =
        (((profile as usize * 24 + usize::from(depth) - 1) * 8 + feeling) * COLUMNS + column) * 2;
    f64::from(u16::from_le_bytes([TABLE[offset], TABLE[offset + 1]]))
}

pub(super) fn probability(query: &SearchQuery, profile: Profile) -> f64 {
    query
        .floor_requirements
        .iter()
        .map(|floor| floor_probability(floor, profile))
        .product()
}

fn floor_probability(floor: &FloorRequirement, profile: Profile) -> f64 {
    if !(1..=24).contains(&floor.depth) || floor.depth % 5 == 0 {
        return 0.0;
    }
    let total: f64 = (0..8)
        .map(|feeling| count(profile, floor.depth, feeling, 0))
        .sum();
    if total == 0.0 {
        return f64::NAN;
    }
    let required = RoomSet::from_types(floor.rooms.iter().copied());
    let any = RoomSet::from_types(floor.any_rooms.iter().copied());
    (0..8)
        .filter(|&feeling| {
            floor
                .feeling
                .is_none_or(|wanted| wanted as usize == feeling)
        })
        .map(|feeling| {
            let samples = count(profile, floor.depth, feeling, 0);
            if samples == 0.0 {
                return 0.0;
            }
            let share =
                |room: RoomType| count(profile, floor.depth, feeling, room as usize + 1) / samples;
            let gardens = RoomSet::from_types([RoomType::SpecialGarden, RoomType::SecretGarden]);
            let garden_intersection = count(profile, floor.depth, feeling, COLUMNS - 1) / samples;
            let mut p = samples / total;
            if required.0 & gardens.0 == gardens.0 {
                p *= garden_intersection;
                p *= RoomSet(required.0 & !gardens.0)
                    .iter()
                    .map(share)
                    .product::<f64>();
            } else {
                p *= required.iter().map(share).product::<f64>();
            }
            if any.0 != 0 && required.0 & any.0 == 0 {
                let mut miss = 1.0;
                let mut remaining = any;
                if any.0 & gardens.0 == gardens.0 {
                    miss *= 1.0 - share(RoomType::SpecialGarden) - share(RoomType::SecretGarden)
                        + garden_intersection;
                    remaining.0 &= !gardens.0;
                }
                miss *= remaining
                    .iter()
                    .map(|room| 1.0 - share(room))
                    .product::<f64>();
                p *= 1.0 - miss;
            }
            p
        })
        .sum::<f64>()
        .clamp(0.0, 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::level_prelude::Feeling;

    #[test]
    fn calibrated_tables_have_consistent_counts() {
        for profile in [
            Profile::None,
            Profile::MimicTooth,
            Profile::ParchmentScrap,
            Profile::RatSkull,
            Profile::ExoticCrystals,
            Profile::MossyClump,
            Profile::TrapMechanism,
            Profile::CrackedSpyglass,
        ] {
            for depth in 1..=24 {
                let mut total = 0.0;
                for feeling in 0..8 {
                    let samples = count(profile, depth, feeling, 0);
                    total += samples;
                    for column in 1..COLUMNS {
                        assert!(count(profile, depth, feeling, column) <= samples);
                    }
                }
                assert!((total - if depth % 5 == 0 { 0.0 } else { 8192.0 }).abs() < f64::EPSILON);
            }
        }
    }

    #[test]
    fn garden_union_uses_measured_overlap() {
        for depth in [7, 17, 22] {
            let mut floor = FloorRequirement {
                depth,
                feeling: Some(Feeling::Dark),
                rooms: vec![],
                any_rooms: vec![],
            };
            let dark = floor_probability(&floor, Profile::None);
            assert!((0.055..0.085).contains(&dark));
            floor.rooms = vec![RoomType::SpecialGarden];
            let ordinary = floor_probability(&floor, Profile::None);
            floor.rooms = vec![RoomType::SecretGarden];
            let secret = floor_probability(&floor, Profile::None);
            floor.rooms.push(RoomType::SpecialGarden);
            let both = floor_probability(&floor, Profile::None);
            floor.any_rooms = std::mem::take(&mut floor.rooms);
            let either = floor_probability(&floor, Profile::None);
            assert!((either - (ordinary + secret - both)).abs() < 1e-12);
            assert!(either > 0.001 && either < dark);
        }
    }
}
