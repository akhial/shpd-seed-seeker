//! Bake floor marginals and co-occurrences; run from the repository root:
//! `cargo run --release --example calibrate_floors -- 65535 [output.bin] [seed_offset]`
//! Calibration and `probability_sweep` use disjoint deterministic seed streams.
//! A final `exact` argument bakes per-feeling room rows for Mossy Clump and
//! Trap Mechanism into an FLX1 table; ordinary runs produce FLP4 counts.
use shpd_seedfinder_core::{
    catalog::ItemId,
    challenges::Challenges,
    main_world::generate_main_world_with_trinket,
    probability_tables::floors::{
        self as format, FEATURES, FLOORS, GROUPS, PAIRS, PROFILES, triangle,
    },
    seed::{DungeonSeed, TOTAL_SEEDS},
};
use std::{
    sync::atomic::{AtomicUsize, Ordering},
    time::Instant,
};

const ROOM_PAIRS: usize = FEATURES * (FEATURES - 1) / 2;
const COLUMNS: usize = 1 + FEATURES + ROOM_PAIRS;

struct Tally {
    rows: Vec<u32>,
    feelings: Vec<u32>,
    cross_rooms: Vec<u32>,
    cross_feelings: Vec<u32>,
}
impl Tally {
    fn new(groups: usize) -> Self {
        Self {
            rows: vec![0; FLOORS * groups * COLUMNS],
            feelings: vec![0; FLOORS * 8],
            cross_rooms: vec![0; PAIRS * FEATURES],
            cross_feelings: vec![0; PAIRS * 64],
        }
    }
    fn merge(&mut self, other: Self) {
        for (target, source) in [
            (&mut self.rows, other.rows),
            (&mut self.feelings, other.feelings),
            (&mut self.cross_rooms, other.cross_rooms),
            (&mut self.cross_feelings, other.cross_feelings),
        ] {
            for (a, b) in target.iter_mut().zip(source) {
                *a += b;
            }
        }
    }
}

fn put32(bytes: &mut [u8], offset: usize, count: u32) {
    bytes[offset..offset + 4].copy_from_slice(&count.to_le_bytes());
}

fn bake(bytes: &mut Vec<u8>, tally: &Tally, profile: usize) {
    for (values, start) in [
        (
            &tally.feelings,
            format::FEELINGS_OFFSET + profile * FLOORS * 8 * 4,
        ),
        (
            &tally.cross_rooms,
            format::ROOMS_OFFSET + profile * PAIRS * FEATURES * 4,
        ),
        (
            &tally.cross_feelings,
            format::CROSS_FEELINGS_OFFSET + profile * PAIRS * 64 * 4,
        ),
    ] {
        for (index, count) in values.iter().enumerate() {
            put32(bytes, start + index * 4, *count);
        }
    }
    bake_rows(bytes, &tally.rows, profile * FLOORS * GROUPS);
}

fn bake_rows(bytes: &mut Vec<u8>, rows: &[u32], first: usize) {
    for (index, row) in rows.chunks_exact(COLUMNS).enumerate() {
        let offset = 8 + (first + index) * 4;
        let start = u32::try_from(bytes.len()).unwrap();
        bytes[offset..offset + 4].copy_from_slice(&start.to_le_bytes());
        let active: Vec<_> = (0..FEATURES)
            .filter(|room| row[1 + room] > 0 && row[1 + room] < row[0])
            .collect();
        for count in &row[..=FEATURES] {
            bytes.extend_from_slice(&count.to_le_bytes());
        }
        for room in 0..FEATURES {
            bytes.push(
                active
                    .iter()
                    .position(|r| *r == room)
                    .map_or(255, |p| u8::try_from(p).unwrap()),
            );
        }
        for (a, &room) in active.iter().enumerate() {
            for &other in &active[..a] {
                bytes.extend_from_slice(&row[1 + FEATURES + triangle(room, other)].to_le_bytes());
            }
        }
    }
}

fn main() {
    let samples: u32 = std::env::args()
        .nth(1)
        .map_or(65_535, |s| s.parse().unwrap());
    assert!(samples > 0);
    let output = std::env::args()
        .nth(2)
        .unwrap_or_else(|| "crates/seedfinder-core/src/probability_tables/floors.bin".into());
    let seed_offset = std::env::args()
        .nth(3)
        .map_or(17_389, |s| s.parse::<u64>().unwrap())
        % TOTAL_SEEDS;
    let exact = std::env::args().nth(4).is_some_and(|s| s == "exact");
    let groups = if exact { 8 } else { GROUPS };
    let profiles = [
        None,
        Some(ItemId::MimicTooth),
        Some(ItemId::ParchmentScrap),
        Some(ItemId::RatSkull),
        Some(ItemId::ExoticCrystals),
        Some(ItemId::MossyClump),
        Some(ItemId::TrapMechanism),
        Some(ItemId::CrackedSpyglass),
    ];
    assert_eq!(profiles.len(), PROFILES);
    let mut bytes = vec![
        0;
        if exact {
            8 + 2 * FLOORS * 8 * 4
        } else {
            format::DATA_OFFSET
        }
    ];
    bytes[..4].copy_from_slice(if exact { b"FLX1" } else { format::MAGIC });
    bytes[4..8].copy_from_slice(&samples.to_le_bytes());
    let started = Instant::now();
    for (profile, selected) in profiles.into_iter().enumerate() {
        if exact && !matches!(profile, 5 | 6) {
            continue;
        }
        let tally = measure(samples, selected, profile, seed_offset, groups);
        if exact {
            bake_rows(&mut bytes, &tally.rows, (profile - 5) * FLOORS * 8);
        } else {
            bake(&mut bytes, &tally, profile);
        }
        eprintln!(
            "{selected:?}: {samples} worlds; elapsed {:?}",
            started.elapsed()
        );
    }
    // Build completely before replacing the old table.
    std::fs::write(output, &bytes).unwrap();
    eprintln!("{} bytes; {samples} samples per profile", bytes.len());
}

fn measure(
    samples: u32,
    selected: Option<ItemId>,
    profile: usize,
    seed_offset: u64,
    groups: usize,
) -> Tally {
    let cursor = AtomicUsize::new(0);
    std::thread::scope(|scope| {
        let workers: Vec<_> = (0..std::thread::available_parallelism().unwrap().get())
            .map(|_| {
                scope.spawn(|| {
                    let mut tally = Tally::new(groups);
                    loop {
                        let index = cursor.fetch_add(1, Ordering::Relaxed);
                        if index >= usize::try_from(samples).unwrap() {
                            break;
                        }
                        let seed = DungeonSeed::new(
                            u64::try_from(
                                (u128::from(seed_offset) + index as u128 * 3_355_211_884_971)
                                    % u128::from(TOTAL_SEEDS),
                            )
                            .unwrap(),
                        )
                        .unwrap();
                        let world =
                            generate_main_world_with_trinket(seed, 24, Challenges::NONE, selected)
                                .unwrap();
                        let mut masks = [0u128; FLOORS];
                        let mut feelings = [0usize; FLOORS];
                        for (floor, (rooms, feeling)) in
                            world.floor_rooms.iter().zip(&world.feelings).enumerate()
                        {
                            assert_eq!(rooms.depth, feeling.depth);
                            masks[floor] = format::features(rooms.rooms);
                            feelings[floor] = feeling.feeling as usize;
                            tally.feelings[floor * 8 + feelings[floor]] += 1;
                            let row = &mut tally.rows[(floor * groups
                                + if groups == 8 {
                                    feelings[floor]
                                } else {
                                    format::group(profile, feeling.feeling)
                                })
                                * COLUMNS..][..COLUMNS];
                            row[0] += 1;
                            let mut mask = masks[floor];
                            while mask != 0 {
                                let room = mask.trailing_zeros() as usize;
                                row[1 + room] += 1;
                                mask &= mask - 1;
                                let mut remaining = mask;
                                while remaining != 0 {
                                    let other = remaining.trailing_zeros() as usize;
                                    row[1 + FEATURES + triangle(room, other)] += 1;
                                    remaining &= remaining - 1;
                                }
                            }
                        }
                        if groups == 8 {
                            continue;
                        }
                        for high in 1..FLOORS {
                            for low in 0..high {
                                let pair = triangle(low, high);
                                tally.cross_feelings
                                    [pair * 64 + feelings[low] * 8 + feelings[high]] += 1;
                                let mut common = masks[low] & masks[high];
                                while common != 0 {
                                    tally.cross_rooms
                                        [pair * FEATURES + common.trailing_zeros() as usize] += 1;
                                    common &= common - 1;
                                }
                            }
                        }
                    }
                    tally
                })
            })
            .collect();
        let mut tally = Tally::new(groups);
        for worker in workers {
            tally.merge(worker.join().unwrap());
        }
        tally
    })
}
