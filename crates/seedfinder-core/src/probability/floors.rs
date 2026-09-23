//! Baked floor/room marginals and pairwise co-occurrences. Room conjunctions
//! and unions of two types use their measured joint distribution. Wider sets
//! use a strongest-dependency tree; repeated rooms across floors use measured
//! deck depletion. These are estimates, never search/pruning predicates.
use crate::floor_filters::{FloorRequirement, RoomSet, RoomType};
use crate::level_prelude::Feeling;
use crate::probability_tables::{
    floors::{self as table, RoomRow, Tables},
    trinkets::Profile,
};
use crate::query::SearchQuery;

const TABLE: Tables = Tables(include_bytes!("../probability_tables/floors.bin"));
const EXACT_ROOMS: Tables = Tables(include_bytes!("../probability_tables/feeling_rooms.bin"));
const FEELINGS: [Feeling; 8] = [
    Feeling::None,
    Feeling::Chasm,
    Feeling::Water,
    Feeling::Grass,
    Feeling::Dark,
    Feeling::Large,
    Feeling::Traps,
    Feeling::Secrets,
];

fn feeling_probability(profile: Profile, floor: usize, feeling: Feeling) -> f64 {
    if floor == 0 {
        return f64::from(feeling == Feeling::None);
    }
    if matches!(profile, Profile::MossyClump | Profile::TrapMechanism) {
        TABLE.feeling(profile as usize, floor, feeling as usize) / TABLE.samples()
    } else if feeling == Feeling::None {
        0.5
    } else {
        1.0 / 14.0
    }
}

pub(super) fn probability(query: &SearchQuery, profile: Profile) -> f64 {
    if query
        .floor_requirements
        .iter()
        .any(|f| !(1..=24).contains(&f.depth) || f.depth % 5 == 0)
    {
        return 0.0;
    }
    let mut floors: Vec<_> = query.floor_requirements.iter().collect();
    floors.sort_unstable_by_key(|f| f.depth);
    let marginals: Vec<_> = floors
        .iter()
        .map(|f| floor_probability(f, profile))
        .collect();
    tree_probability(&marginals, |a, b| {
        let independent = marginals[a] * marginals[b];
        independent * cross_floor_correction(floors[a], floors[b], profile)
    })
}

fn floor_probability(floor: &FloorRequirement, profile: Profile) -> f64 {
    let index = table::floor_index(floor.depth);
    let required = RoomSet::from_types(floor.rooms.iter().copied());
    let any = RoomSet::from_types(floor.any_rooms.iter().copied());
    FEELINGS
        .iter()
        .copied()
        .filter(|feeling| floor.feeling.is_none_or(|wanted| wanted == *feeling))
        .map(|feeling| {
            let weight = feeling_probability(profile, index, feeling);
            if weight == 0.0 {
                return 0.0;
            }
            let row = if matches!(profile, Profile::MossyClump | Profile::TrapMechanism) {
                EXACT_ROOMS.exact_room(profile as usize, index, feeling as usize)
            } else {
                TABLE.row(
                    profile as usize,
                    index,
                    table::group(profile as usize, feeling),
                )
            };
            weight * room_probability(row, required, any)
        })
        .sum::<f64>()
        .clamp(0.0, 1.0)
}

fn room_probability(row: RoomRow, required: RoomSet, any: RoomSet) -> f64 {
    if required.0 == 0 && any.0 == 0 {
        return 1.0;
    }
    if row.samples() == 0.0 {
        return 0.0;
    }
    let mut events: Vec<_> = required.iter().map(|room| (room as usize, true)).collect();
    let present = room_events(row, &events);
    if any.0 == 0 || any.0 & required.0 != 0 || present == 0.0 {
        return present;
    }
    // P(all required AND at least one alternative) = P(required) -
    // P(required AND every alternative absent). Overlaps were resolved above.
    events.extend(any.iter().map(|room| (room as usize, false)));
    (present - room_events(row, &events)).clamp(0.0, present)
}

fn room_events(row: RoomRow, events: &[(usize, bool)]) -> f64 {
    let n = row.samples();
    let marginals: Vec<_> = events
        .iter()
        .map(|(room, present)| {
            let p = row.count(*room) / n;
            if *present { p } else { 1.0 - p }
        })
        .collect();
    tree_probability(&marginals, |a, b| {
        let (a, pa) = events[a];
        let (b, pb) = events[b];
        let joint = row.pair(a, b) / n;
        match (pa, pb) {
            (true, true) => joint,
            (true, false) => row.count(a) / n - joint,
            (false, true) => row.count(b) / n - joint,
            (false, false) => 1.0 - row.count(a) / n - row.count(b) / n + joint,
        }
    })
}

/// A tree of pairwise dependencies is exact for up to two variables, and
/// costs O(n²) for larger sets. No independence multiplier is applied twice
/// around a cycle. Check all pairs for zero before selecting the tree.
fn tree_probability(marginals: &[f64], joint: impl Fn(usize, usize) -> f64) -> f64 {
    let n = marginals.len();
    if n == 0 {
        return 1.0;
    }
    if marginals.contains(&0.0) {
        return 0.0;
    }
    if n == 1 {
        return marginals[0];
    }
    if n == 2 {
        return joint(0, 1).clamp(0.0, marginals[0].min(marginals[1]));
    }
    let mut pairs = vec![0.0; n * n];
    for a in 0..n {
        for b in 0..a {
            let p = joint(a, b).clamp(0.0, marginals[a].min(marginals[b]));
            if p <= 0.0 {
                return 0.0;
            }
            pairs[a * n + b] = p;
            pairs[b * n + a] = p;
        }
    }
    let mut selected = vec![false; n];
    let mut scores = vec![f64::NEG_INFINITY; n];
    let mut factors = vec![1.0; n];
    selected[0] = true;
    let mut last = 0;
    let mut p = marginals[0];
    for _ in 1..n {
        for next in 0..n {
            if selected[next] {
                continue;
            }
            let pair = pairs[last * n + next];
            let score = (pair / (marginals[last] * marginals[next])).ln().abs();
            if score > scores[next] {
                scores[next] = score;
                factors[next] = pair / marginals[last];
            }
        }
        let next = (0..n)
            .filter(|&index| !selected[index])
            .max_by(|&a, &b| scores[a].total_cmp(&scores[b]).then_with(|| b.cmp(&a)))
            .unwrap();
        p *= factors[next];
        selected[next] = true;
        last = next;
    }
    p.clamp(0.0, marginals.iter().copied().fold(1.0, f64::min))
}

fn common_features(floor: &FloorRequirement) -> u128 {
    let required = RoomSet::from_types(floor.rooms.iter().copied()).0;
    let any = RoomSet::from_types(floor.any_rooms.iter().copied()).0;
    let gardens = RoomSet::from_types([RoomType::SpecialGarden, RoomType::SecretGarden]).0;
    required
        | if any.is_power_of_two() {
            any
        } else if any == gardens {
            1 << table::GARDENS
        } else {
            0
        }
}

fn cross_floor_correction(a: &FloorRequirement, b: &FloorRequirement, profile: Profile) -> f64 {
    let (fa, fb) = (table::floor_index(a.depth), table::floor_index(b.depth));
    if fa == fb {
        return 1.0;
    } // Repeated depths are rejected by query validation.
    let mut correction = 1.0;
    let mut common = common_features(a) & common_features(b);
    while common != 0 {
        let feature = common.trailing_zeros() as usize;
        let count = |floor| {
            (0..table::GROUPS)
                .map(|group| TABLE.row(profile as usize, floor, group).count(feature))
                .sum::<f64>()
        };
        let independent = count(fa) * count(fb);
        if independent > 0.0 {
            let ratio =
                TABLE.cross_room(profile as usize, fa, fb, feature) * TABLE.samples() / independent;
            if ratio == 0.0 {
                return 0.0;
            }
            // Multiple shared features often describe the same depletion event.
            // Keep its strongest dependency rather than counting it repeatedly.
            if ratio.ln().abs() > f64::ln(correction).abs() {
                correction = ratio;
            }
        }
        common &= common - 1;
    }
    // These trinkets alternate override feelings across floors. Other profiles
    // use independent, exact 1/14 feeling rolls and need no sampled correction.
    if matches!(profile, Profile::MossyClump | Profile::TrapMechanism)
        && let (Some(a), Some(b)) = (a.feeling, b.feeling)
    {
        let independent = TABLE.feeling(profile as usize, fa, a as usize)
            * TABLE.feeling(profile as usize, fb, b as usize);
        if independent > 0.0 {
            correction *= TABLE.cross_feeling(profile as usize, fa, fb, a as usize, b as usize)
                * TABLE.samples()
                / independent;
        }
    }
    correction
}

#[cfg(test)]
#[allow(clippy::float_cmp)] // Exact integer counts and analytical fractions.
mod tests {
    use super::*;

    #[test]
    fn exact_feeling_rows_have_consistent_counts() {
        assert_eq!(&EXACT_ROOMS.0[..4], b"FLX1");
        assert_eq!(EXACT_ROOMS.samples(), 524_288.0);
        for profile in [5, 6] {
            for floor in 0..table::FLOORS {
                let mut total = 0.0;
                for feeling in 0..8 {
                    let row = EXACT_ROOMS.exact_room(profile, floor, feeling);
                    total += row.samples();
                    for a in 0..table::FEATURES {
                        assert!(row.count(a) <= row.samples());
                        for b in 0..a {
                            let joint = row.pair(a, b);
                            assert!(joint <= row.count(a).min(row.count(b)));
                            assert!(
                                joint >= (row.count(a) + row.count(b) - row.samples()).max(0.0)
                            );
                        }
                    }
                }
                assert_eq!(total, EXACT_ROOMS.samples());
            }
        }
    }

    #[test]
    fn calibrated_tables_have_consistent_counts() {
        assert_eq!(&TABLE.0[..4], table::MAGIC);
        assert_eq!(TABLE.samples(), 1_048_560.0);
        for profile in 0..table::PROFILES {
            for floor in 0..table::FLOORS {
                assert_eq!(
                    (0..8)
                        .map(|feeling| TABLE.feeling(profile, floor, feeling))
                        .sum::<f64>(),
                    TABLE.samples()
                );
                assert_eq!(
                    (0..table::GROUPS)
                        .map(|group| TABLE.row(profile, floor, group).samples())
                        .sum::<f64>(),
                    TABLE.samples()
                );
                for group in 0..table::GROUPS {
                    let row = TABLE.row(profile, floor, group);
                    for a in 0..table::FEATURES {
                        assert!(row.count(a) <= row.samples());
                        for b in 0..a {
                            let pair = row.pair(a, b);
                            assert!(pair <= row.count(a).min(row.count(b)));
                            assert!(pair >= (row.count(a) + row.count(b) - row.samples()).max(0.0));
                        }
                    }
                    assert_eq!(
                        row.count(table::GARDENS),
                        row.count(RoomType::SpecialGarden as usize)
                            + row.count(RoomType::SecretGarden as usize)
                            - row.pair(
                                RoomType::SpecialGarden as usize,
                                RoomType::SecretGarden as usize
                            )
                    );
                }
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
            assert_eq!(dark, 1.0 / 14.0);
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

    #[test]
    fn unions_and_conjunctions_respect_shared_rooms() {
        let row = TABLE.row(Profile::None as usize, table::floor_index(7), 0);
        let garden = RoomSet::from_types([RoomType::SpecialGarden]);
        let both = RoomSet::from_types([RoomType::SpecialGarden, RoomType::SecretGarden]);
        assert_eq!(
            room_probability(row, garden, both),
            room_probability(row, garden, RoomSet(0))
        );
        assert!(
            (room_probability(row, garden, RoomSet::from_types([RoomType::SecretGarden]))
                - room_probability(row, both, RoomSet(0)))
            .abs()
                < 1e-12
        );
    }
}
