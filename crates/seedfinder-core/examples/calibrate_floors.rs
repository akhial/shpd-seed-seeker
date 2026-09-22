//! Regenerate floor probability tables with the canonical engine:
//! `cargo run --release --example calibrate_floors -- 8192`
//! Writes `probability_tables/floors.bin`. Profiles follow `Profile` declaration order.
use shpd_seedfinder_core::{
    catalog::ItemId,
    challenges::Challenges,
    floor_filters::RoomType,
    main_world::generate_main_world_with_trinket,
    seed::{DungeonSeed, TOTAL_SEEDS},
};
use std::sync::atomic::{AtomicUsize, Ordering};

fn main() {
    let samples: u16 = std::env::args().nth(1).map_or(8192, |s| s.parse().unwrap());
    assert!(samples > 0);
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
    let columns = RoomType::ALL.len() + 2;
    let cursor = AtomicUsize::new(0);
    let tables = std::thread::scope(|scope| {
        let handles: Vec<_> = (0..std::thread::available_parallelism().unwrap().get().min(16))
            .map(|_| {
                scope.spawn(|| {
                    let mut counts = vec![0u16; 8 * 24 * 8 * columns];
                    loop {
                        let task = cursor.fetch_add(1, Ordering::Relaxed);
                        if task >= 8 * usize::from(samples) {
                            break;
                        }
                        let profile = task / usize::from(samples);
                        let index = task % usize::from(samples);
                        let seed = DungeonSeed::new(
                            (17_389 + index as u64 * 3_355_211_884_971) % TOTAL_SEEDS,
                        )
                        .unwrap();
                        let world = generate_main_world_with_trinket(
                            seed,
                            24,
                            Challenges::NONE,
                            profiles[profile],
                        )
                        .unwrap();
                        for (floor, feeling) in world.floor_rooms.iter().zip(&world.feelings) {
                            assert_eq!(floor.depth, feeling.depth);
                            let offset = ((profile * 24 + usize::from(floor.depth) - 1) * 8
                                + feeling.feeling as usize)
                                * columns;
                            counts[offset] += 1;
                            for room in floor.rooms.iter() {
                                counts[offset + 1 + room as usize] += 1;
                            }
                            if floor.rooms.contains(RoomType::SpecialGarden)
                                && floor.rooms.contains(RoomType::SecretGarden)
                            {
                                counts[offset + columns - 1] += 1;
                            }
                        }
                    }
                    counts
                })
            })
            .collect();
        handles
            .into_iter()
            .map(|h| h.join().unwrap())
            .collect::<Vec<_>>()
    });
    let mut sums = vec![0u16; 8 * 24 * 8 * columns];
    for counts in tables {
        for (sum, count) in sums.iter_mut().zip(counts) {
            *sum += count;
        }
    }
    let bytes: Vec<_> = sums.iter().flat_map(|count| count.to_le_bytes()).collect();
    std::fs::write(
        "crates/seedfinder-core/src/probability_tables/floors.bin",
        bytes,
    )
    .unwrap();
    eprintln!(
        "Calibrated {samples} worlds per profile, {} room types",
        RoomType::ALL.len()
    );
}
