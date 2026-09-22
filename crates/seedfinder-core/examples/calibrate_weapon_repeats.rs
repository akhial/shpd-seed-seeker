//! Bake same-identity weapon scarcity by line and tier, using co-obtainable
//! subsets. `cargo run --release --example calibrate_weapon_repeats -- 500000`
use shpd_seedfinder_core::{
    catalog::{ItemKind, item},
    main_world::CanonicalMainWorldGenerator,
    model::WorldItem,
    probability_tables::{DEPTHS, IDENTITY_REPEAT_LIMIT, LINES, TIERS, line_index, line_of},
    search::WorldGenerator,
    seed::{DungeonSeed, TOTAL_SEEDS},
};
use std::{
    collections::BTreeMap,
    sync::atomic::{AtomicUsize, Ordering},
};

const GROUPS: usize = LINES * TIERS;
const IDENTITIES: usize = 256;

fn subsets(items: &[&WorldItem], from: usize, chosen: &mut Vec<usize>, counts: &mut [u64]) {
    if chosen.len() == IDENTITY_REPEAT_LIMIT {
        return;
    }
    for index in from..items.len() {
        if let Some((group, mut mask)) = items[index].accessibility.scenario_constraint() {
            for &previous in chosen.iter() {
                if let Some((other, allowed)) = items[previous].accessibility.scenario_constraint()
                    && group == other
                {
                    mask &= allowed;
                }
            }
            if mask == 0 {
                continue;
            }
        }
        chosen.push(index);
        let depth = chosen
            .iter()
            .map(|&i| usize::from(items[i].depth) - 1)
            .max()
            .unwrap();
        for count in &mut counts[(chosen.len() - 1) * DEPTHS + depth..chosen.len() * DEPTHS] {
            *count += 1;
        }
        subsets(items, index + 1, chosen, counts);
        chosen.pop();
    }
}

fn main() {
    let samples: usize = std::env::args()
        .nth(1)
        .map_or(500_000, |s| s.parse().unwrap());
    assert!(samples > 0);
    let cursor = AtomicUsize::new(0);
    let (means, moments, groups) = std::thread::scope(|scope| {
        let workers: Vec<_> = (0..std::thread::available_parallelism().unwrap().get())
            .map(|_| {
                scope.spawn(|| {
                    let mut means = vec![0u64; IDENTITIES * DEPTHS];
                    let mut moments = vec![0u64; GROUPS * IDENTITY_REPEAT_LIMIT * DEPTHS];
                    let mut groups = vec![None; IDENTITIES];
                    loop {
                        let index = cursor.fetch_add(1, Ordering::Relaxed);
                        if index >= samples {
                            break;
                        }
                        let value = u64::try_from(
                            (8_675_309u128 + index as u128 * 3_355_211_884_971)
                                % u128::from(TOTAL_SEEDS),
                        )
                        .unwrap();
                        let world = CanonicalMainWorldGenerator
                            .generate(DungeonSeed::new(value).unwrap(), 24);
                        let mut identities: BTreeMap<usize, Vec<&WorldItem>> = BTreeMap::new();
                        for candidate in &world.items {
                            let definition = item(candidate.item);
                            if definition.kind != ItemKind::Weapon {
                                continue;
                            }
                            let Some(tier) = definition.tier else {
                                continue;
                            };
                            let identity = candidate.item as usize;
                            let group =
                                line_index(line_of(candidate.item)) * TIERS + usize::from(tier) - 1;
                            groups[identity] = Some(group);
                            for count in &mut means[identity * DEPTHS + usize::from(candidate.depth)
                                - 1
                                ..(identity + 1) * DEPTHS]
                            {
                                *count += 1;
                            }
                            identities.entry(identity).or_default().push(candidate);
                        }
                        for (identity, items) in identities {
                            let group = groups[identity].unwrap();
                            subsets(
                                &items,
                                0,
                                &mut Vec::new(),
                                &mut moments[group * IDENTITY_REPEAT_LIMIT * DEPTHS
                                    ..(group + 1) * IDENTITY_REPEAT_LIMIT * DEPTHS],
                            );
                        }
                    }
                    (means, moments, groups)
                })
            })
            .collect();
        let mut means = vec![0u64; IDENTITIES * DEPTHS];
        let mut moments = vec![0u64; GROUPS * IDENTITY_REPEAT_LIMIT * DEPTHS];
        let mut groups = vec![None; IDENTITIES];
        for worker in workers {
            let (m, c, g) = worker.join().unwrap();
            for (a, b) in means.iter_mut().zip(m) {
                *a += b;
            }
            for (a, b) in moments.iter_mut().zip(c) {
                *a += b;
            }
            for (a, b) in groups.iter_mut().zip(g) {
                *a = a.or(b);
            }
        }
        (means, moments, groups)
    });
    render(samples, &means, &moments, &groups);
}

#[allow(clippy::cast_precision_loss)] // Offline counters fit exactly in f64.
fn render(samples: usize, means: &[u64], moments: &[u64], groups: &[Option<usize>]) {
    println!(
        "//! Co-obtainable weapon identity repeats over {samples} training seeds.\n//! Generated by `calibrate_weapon_repeats`; [line * 5 + tier - 1][copies - 1][depth - 1].\n#![allow(clippy::unreadable_literal)]\n#[rustfmt::skip]\npub(crate) const REPEATS: [[[f64; 24]; 4]; 15] = ["
    );
    for group in 0..GROUPS {
        println!("    [");
        for copies in 1..=IDENTITY_REPEAT_LIMIT {
            let row: Vec<_> = (0..DEPTHS)
                .map(|depth| {
                    let factorial: f64 = (1..=copies).map(|c| c as f64).product();
                    let independent: f64 = (0..IDENTITIES)
                        .filter(|&id| groups[id] == Some(group))
                        .map(|id| {
                            (means[id * DEPTHS + depth] as f64 / samples as f64)
                                .powi(i32::try_from(copies).unwrap())
                                / factorial
                        })
                        .sum();
                    let observed = moments
                        [(group * IDENTITY_REPEAT_LIMIT + copies - 1) * DEPTHS + depth]
                        as f64
                        / samples as f64;
                    format!(
                        "{:.9}",
                        if independent > 0.0 {
                            (observed / independent).clamp(0.0, 4.0)
                        } else {
                            1.0
                        }
                    )
                })
                .collect();
            println!("        [{}],", row.join(", "));
        }
        println!("    ],");
    }
    println!("];");
}
