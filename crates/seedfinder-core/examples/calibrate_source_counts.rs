//! `cargo run --release --example calibrate_source_counts -- 65536 output.bin`
//! Counts scattered stock by source without assuming independent arrivals.
use shpd_seedfinder_core::{
    catalog::{ItemId, ItemKind, item},
    challenges::Challenges,
    main_world::generate_main_world_with_trinket,
    probability_tables::{
        DEPTHS, bundle_size, line_of, prize_group, source_counts as format, source_index,
    },
    seed::{DungeonSeed, TOTAL_SEEDS},
};
use std::{
    collections::BTreeMap,
    sync::atomic::{AtomicUsize, Ordering},
    time::Instant,
};

const BUCKETS: usize = format::GROUPS * format::SOURCES;
const COUNTS: usize = 64;

fn measure(samples: u32, selected: Option<ItemId>) -> Vec<u32> {
    let cursor = AtomicUsize::new(0);
    std::thread::scope(|scope| {
        let workers: Vec<_> = (0..std::thread::available_parallelism().unwrap().get())
            .map(|_| {
                scope.spawn(|| {
                    let mut histogram = vec![0u32; BUCKETS * DEPTHS * COUNTS];
                    loop {
                        let index = cursor.fetch_add(1, Ordering::Relaxed);
                        if index >= usize::try_from(samples).unwrap() {
                            break;
                        }
                        let value = u64::try_from(
                            (57_721_566u128 + index as u128 * 3_355_211_884_971)
                                % u128::from(TOTAL_SEEDS),
                        )
                        .unwrap();
                        let world = generate_main_world_with_trinket(
                            DungeonSeed::new(value).unwrap(),
                            24,
                            Challenges::NONE,
                            selected,
                        )
                        .unwrap();
                        let mut counts = vec![0usize; BUCKETS * DEPTHS];
                        let mut choices: BTreeMap<(usize, u16), Vec<(usize, u64)>> =
                            BTreeMap::new();
                        for candidate in &world.items {
                            let kind = item(candidate.item).kind;
                            if kind == ItemKind::Trinket
                                || prize_group(candidate.source).is_some()
                                || bundle_size(candidate.source, kind) != 0
                            {
                                continue;
                            }
                            let bucket = format::group(kind, line_of(candidate.item))
                                * format::SOURCES
                                + source_index(candidate.source);
                            let depth = usize::from(candidate.depth) - 1;
                            if let Some((choice, mask)) =
                                candidate.accessibility.scenario_constraint()
                            {
                                choices
                                    .entry((bucket, choice))
                                    .or_default()
                                    .push((depth, mask));
                            } else {
                                for count in
                                    &mut counts[bucket * DEPTHS + depth..(bucket + 1) * DEPTHS]
                                {
                                    *count += 1;
                                }
                            }
                        }
                        for ((bucket, _), options) in choices {
                            let mut best = [0; DEPTHS];
                            let mut mask =
                                options.iter().fold(0, |mask, (_, option)| mask | option);
                            while mask != 0 {
                                let bit = 1u64 << mask.trailing_zeros();
                                mask &= mask - 1;
                                let mut offered = [0; DEPTHS];
                                for (depth, allowed) in &options {
                                    if allowed & bit != 0 {
                                        for n in &mut offered[*depth..] {
                                            *n += 1;
                                        }
                                    }
                                }
                                for (a, b) in best.iter_mut().zip(offered) {
                                    *a = (*a).max(b);
                                }
                            }
                            for (count, extra) in counts[bucket * DEPTHS..(bucket + 1) * DEPTHS]
                                .iter_mut()
                                .zip(best)
                            {
                                *count += extra;
                            }
                        }
                        for (row, count) in counts.into_iter().enumerate() {
                            assert!(count < COUNTS, "increase the offline histogram capacity");
                            if count > 0 {
                                histogram[row * COUNTS + count] += 1;
                            }
                        }
                    }
                    histogram
                })
            })
            .collect();
        let mut histogram = vec![0u32; BUCKETS * DEPTHS * COUNTS];
        for worker in workers {
            for (a, b) in histogram.iter_mut().zip(worker.join().unwrap()) {
                *a += b;
            }
        }
        histogram
    })
}

#[allow(clippy::cast_precision_loss)] // Counts up to 65,536 are exact in f32.
fn main() {
    let samples: u32 = std::env::args()
        .nth(1)
        .map_or(65_536, |s| s.parse().unwrap());
    assert!(samples > 0);
    let output = std::env::args().nth(2).expect("output path");
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
    let mut bytes = vec![0; format::HEADER];
    bytes[..4].copy_from_slice(format::MAGIC);
    bytes[4..8].copy_from_slice(&samples.to_le_bytes());
    let started = Instant::now();
    for (profile, selected) in profiles.into_iter().enumerate() {
        let histogram = measure(samples, selected);
        for (row, values) in histogram.chunks_exact(COUNTS).enumerate() {
            let Some(last) = values.iter().rposition(|n| *n > 0) else {
                continue;
            };
            let offset = 8 + (profile * BUCKETS * DEPTHS + row) * 4;
            let start = u32::try_from(bytes.len()).unwrap();
            bytes[offset..offset + 4].copy_from_slice(&start.to_le_bytes());
            bytes.push(u8::try_from(last + 1).unwrap());
            bytes.extend_from_slice(
                &((samples - values.iter().sum::<u32>()) as f32 / samples as f32).to_le_bytes(),
            );
            for &count in &values[1..=last] {
                bytes.extend_from_slice(&(count as f32 / samples as f32).to_le_bytes());
            }
        }
        eprintln!("{selected:?}: {samples} worlds, {:?}", started.elapsed());
    }
    std::fs::write(output, &bytes).unwrap();
    eprintln!("{} bytes", bytes.len());
}
