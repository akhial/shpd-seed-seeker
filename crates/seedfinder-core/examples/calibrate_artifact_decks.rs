//! Offline joint artifact donor/deck calibration, never run by CI or the UI.
//! `cargo run --release --example calibrate_artifact_decks -- 8192 output.bin`
use shpd_seedfinder_core::{
    artifacts::deck_at,
    catalog::{ItemId, ItemKind, item},
    challenges::Challenges,
    main_world::generate_main_world_with_trinket,
    model::WorldItem,
    probability_tables::source_index,
    seed::{DungeonSeed, TOTAL_SEEDS},
};
use std::{
    collections::BTreeMap,
    sync::atomic::{AtomicUsize, Ordering},
    time::Instant,
};

fn compatible(entries: &[&WorldItem], subset: u16) -> bool {
    let mut scenarios = BTreeMap::new();
    for (index, entry) in entries.iter().enumerate() {
        if subset & (1 << index) == 0 {
            continue;
        }
        if let Some((group, mask)) = entry.accessibility.scenario_constraint() {
            let allowed = scenarios.entry(group).or_insert(u64::MAX);
            *allowed &= mask;
            if *allowed == 0 {
                return false;
            }
        }
    }
    true
}

fn record(index: usize, profile: Option<ItemId>, offset: u64) -> Vec<u8> {
    let value = (u128::from(offset) + index as u128 * 3_355_211_884_971) % u128::from(TOTAL_SEEDS);
    let world = generate_main_world_with_trinket(
        DungeonSeed::new(u64::try_from(value).unwrap()).unwrap(),
        24,
        Challenges::NONE,
        profile,
    )
    .unwrap();
    let mut entries: Vec<_> = world
        .items
        .iter()
        .filter(|e| item(e.item).kind == ItemKind::Artifact)
        .collect();
    entries.sort_by_key(|e| e.depth);
    assert!(entries.len() <= 11);
    // Every draw is represented, including inaccessible alternatives. Only the
    // identity permutation is integrated analytically by the runtime model.
    for depth in 1..=24 {
        assert_eq!(
            entries.iter().filter(|e| e.depth <= depth).count() + deck_at(&world, depth).len(),
            11
        );
    }
    let all = (1u16 << entries.len()) - 1;
    let maximal: Vec<_> = (0..=all)
        .filter(|&mask| {
            compatible(&entries, mask)
                && (0..entries.len())
                    .all(|i| mask & (1 << i) != 0 || !compatible(&entries, mask | (1 << i)))
        })
        .collect();
    let mut bytes = vec![u8::try_from(entries.len()).unwrap()];
    for entry in entries {
        bytes.extend([
            entry.depth,
            u8::try_from(source_index(entry.source)).unwrap() | (u8::from(entry.cursed) << 5),
        ]);
    }
    bytes.push(u8::try_from(maximal.len()).unwrap());
    for mask in maximal {
        bytes.extend(mask.to_le_bytes());
    }
    bytes
}

fn main() {
    let args: Vec<_> = std::env::args().skip(1).collect();
    let samples: usize = args.first().map_or(8192, |s| s.parse().unwrap());
    let output = args.get(1).expect("output path");
    let offset: u64 = args.get(2).map_or(314_159_265, |s| s.parse().unwrap());
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
    let mut bytes = b"ADP1".to_vec();
    bytes.extend(u32::try_from(samples).unwrap().to_le_bytes());
    let started = Instant::now();
    for profile in profiles {
        let cursor = AtomicUsize::new(0);
        let mut records = std::thread::scope(|scope| {
            let workers: Vec<_> = (0..std::thread::available_parallelism().unwrap().get())
                .map(|_| {
                    scope.spawn(|| {
                        let mut records = Vec::new();
                        loop {
                            let index = cursor.fetch_add(1, Ordering::Relaxed);
                            if index >= samples {
                                break;
                            }
                            records.push((index, record(index, profile, offset)));
                        }
                        records
                    })
                })
                .collect();
            workers
                .into_iter()
                .flat_map(|worker| worker.join().unwrap())
                .collect::<Vec<_>>()
        });
        records.sort_unstable_by_key(|(index, _)| *index);
        for (_, record) in records {
            bytes.extend(record);
        }
        eprintln!("{profile:?}: {samples} worlds in {:?}", started.elapsed());
    }
    std::fs::write(output, &bytes).unwrap();
    eprintln!(
        "{} bytes; offset {offset}; stride 3355211884971",
        bytes.len()
    );
}
