//! Bake cross-family competition on the first floor (before any quest prizes).
//! `cargo run --release --example calibrate_first_floor -- 1000000 > crates/seedfinder-core/src/probability_tables/first_floor.rs`
use shpd_seedfinder_core::{
    catalog::item,
    main_world::CanonicalMainWorldGenerator,
    model::WorldItem,
    probability_tables::kind_index,
    search::WorldGenerator,
    seed::{DungeonSeed, TOTAL_SEEDS},
};
use std::sync::atomic::{AtomicUsize, Ordering};

const STATES: usize = 625;
const LEVELS: usize = 4;
const POWERS: [usize; 4] = [1, 5, 25, 125];

fn subsets(
    items: &[&WorldItem],
    from: usize,
    selected: &mut Vec<usize>,
    key: usize,
    moments: &mut [u64],
    present: &mut [bool],
) {
    if selected.len() == 4 {
        return;
    }
    for index in from..items.len() {
        if let Some((group, mut mask)) = items[index].accessibility.scenario_constraint() {
            for &previous in selected.iter() {
                if let Some((other, compatible)) =
                    items[previous].accessibility.scenario_constraint()
                    && group == other
                {
                    mask &= compatible;
                }
            }
            if mask == 0 {
                continue;
            }
        }
        let next = key + POWERS[kind_index(item(items[index].item).kind)];
        moments[next] += 1;
        present[next] = true;
        selected.push(index);
        subsets(items, index + 1, selected, next, moments, present);
        selected.pop();
    }
}

fn main() {
    let samples: usize = std::env::args()
        .nth(1)
        .map_or(1_000_000, |s| s.parse().unwrap());
    assert!(samples > 0);
    let cursor = AtomicUsize::new(0);
    let (moments, presence) = std::thread::scope(|scope| {
        let workers: Vec<_> = (0..std::thread::available_parallelism().unwrap().get())
            .map(|_| {
                scope.spawn(|| {
                    let mut moments = vec![0u64; STATES * LEVELS];
                    let mut presence = vec![0u64; STATES * LEVELS];
                    loop {
                        let index = cursor.fetch_add(1, Ordering::Relaxed);
                        if index >= samples {
                            break;
                        }
                        let seed = DungeonSeed::new(
                            u64::try_from(
                                (3_131_337u128 + index as u128 * 3_355_211_884_971)
                                    % u128::from(TOTAL_SEEDS),
                            )
                            .unwrap(),
                        )
                        .unwrap();
                        let world = CanonicalMainWorldGenerator.generate(seed, 1);
                        let items: Vec<_> = world
                            .items
                            .iter()
                            .filter(|candidate| {
                                !matches!(
                                    item(candidate.item).kind,
                                    shpd_seedfinder_core::catalog::ItemKind::Artifact
                                        | shpd_seedfinder_core::catalog::ItemKind::Trinket
                                )
                            })
                            .collect();
                        for minimum in 0..LEVELS {
                            let filtered: Vec<_> = items
                                .iter()
                                .copied()
                                .filter(|item| usize::from(item.upgrade) >= minimum)
                                .collect();
                            let mut present = [false; STATES];
                            subsets(
                                &filtered,
                                0,
                                &mut Vec::new(),
                                0,
                                &mut moments[minimum * STATES..(minimum + 1) * STATES],
                                &mut present,
                            );
                            for (count, seen) in
                                presence[minimum * STATES..].iter_mut().zip(present)
                            {
                                *count += u64::from(seen);
                            }
                        }
                    }
                    (moments, presence)
                })
            })
            .collect();
        let (mut moments, mut presence) =
            (vec![0u64; STATES * LEVELS], vec![0u64; STATES * LEVELS]);
        for worker in workers {
            let (m, p) = worker.join().unwrap();
            for (a, b) in moments.iter_mut().zip(m) {
                *a += b;
            }
            for (a, b) in presence.iter_mut().zip(p) {
                *a += b;
            }
        }
        (moments, presence)
    });
    render(samples, &moments, &presence);
}

#[allow(clippy::cast_precision_loss)] // Offline sample counters fit exactly in f64.
fn render(samples: usize, moments: &[u64], presence: &[u64]) {
    println!(
        "//! First-floor co-obtainable equipment subsets over {samples} training seeds.\n//! Generated by `examples/calibrate_first_floor.rs`; do not edit by hand.\n//! (625 * minimum upgrade + base-5 count key, independent presence, joint/independent presence,\n//! joint/independent factorial moment). Families: weapon, armor, wand, ring.\n#![allow(clippy::unreadable_literal)] // Generated decimal measurements.\n#[rustfmt::skip]\npub(crate) const CROSS_FAMILY: &[(usize, f64, f64, f64)] = &["
    );
    for minimum in 0..LEVELS {
        for key in 1..STATES {
            let counts = POWERS.map(|power| key / power % 5);
            if counts.iter().sum::<usize>() > 4 || counts.iter().filter(|c| **c > 0).count() < 2 {
                continue;
            }
            let mut independent = 1.0;
            let mut moment = 1.0;
            for (count, power) in counts.into_iter().zip(POWERS) {
                if count > 0 {
                    independent *=
                        presence[minimum * STATES + count * power] as f64 / samples as f64;
                    moment *= moments[minimum * STATES + count * power] as f64 / samples as f64;
                }
            }
            if independent == 0.0
                || moment == 0.0
                || (minimum > 0 && presence[minimum * STATES + key] < 30)
            {
                continue;
            }
            println!(
                "    ({}, {independent:.10}, {:.10}, {:.10}),",
                minimum * STATES + key,
                presence[minimum * STATES + key] as f64 / samples as f64 / independent,
                moments[minimum * STATES + key] as f64 / samples as f64 / moment
            );
        }
    }
    println!("];");
}
