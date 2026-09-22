//! Measure co-obtainable copies of each wand, including exclusive rewards.
//! `cargo run --release --example calibrate_wand_repeats -- 262144 output.json [trinket]`
use shpd_seedfinder_core::{
    catalog::{ItemKind, item, item_by_stable_id},
    challenges::Challenges,
    generator::WAND_ITEMS,
    main_world::generate_main_world_with_trinket,
    model::{ItemSource, WorldItem},
    probability_tables::wand_repeats,
    seed::{DungeonSeed, TOTAL_SEEDS},
};
use std::sync::atomic::{AtomicUsize, Ordering};

const DEPTHS: usize = 24;
const COPIES: usize = 4;
const LEVELS: usize = 4;
const BANDS: usize = wand_repeats::BANDS;
const ROWS: usize = wand_repeats::ROWS;
const OFFSET: u64 = 4_669_201_609;
const STRIDE: u64 = 3_355_211_884_971;

fn subsets(
    items: &[&WorldItem],
    from: usize,
    chosen: &mut Vec<usize>,
    earliest: &mut [[u8; COPIES]; BANDS],
) {
    if chosen.len() == COPIES {
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
        let depth = chosen.iter().map(|&i| items[i].depth).max().unwrap();
        let minimum = chosen.iter().map(|&i| items[i].upgrade).min().unwrap();
        for row in earliest[..LEVELS].iter_mut().take(usize::from(minimum) + 1) {
            row[chosen.len() - 1] = row[chosen.len() - 1].min(depth);
        }
        for source in [
            None,
            Some(ItemSource::WandmakerReward),
            Some(ItemSource::ImpReward),
            Some(ItemSource::VaultTreasure),
        ] {
            let best = chosen
                .iter()
                .filter_map(|&i| {
                    source
                        .is_none_or(|s| s == items[i].source)
                        .then_some(items[i].upgrade)
                })
                .max();
            if let Some(best) = best {
                for minimum in 0..=usize::from(best).min(4) {
                    let row = &mut earliest[wand_repeats::anchor_band(source, minimum).unwrap()];
                    row[chosen.len() - 1] = row[chosen.len() - 1].min(depth);
                }
            }
        }
        subsets(items, index + 1, chosen, earliest);
        chosen.pop();
    }
}

fn main() {
    let args: Vec<_> = std::env::args().skip(1).collect();
    let samples: usize = args[0].parse().unwrap();
    assert!(samples > 0 && samples <= u32::MAX as usize / WAND_ITEMS.len());
    let selected = args.get(2).map(|id| {
        let definition = item_by_stable_id(id).expect("trinket ID");
        assert_eq!(definition.kind, ItemKind::Trinket);
        definition.id
    });
    let started = std::time::Instant::now();
    let cursor = AtomicUsize::new(0);
    let counts = std::thread::scope(|scope| {
        let workers: Vec<_> = (0..std::thread::available_parallelism().unwrap().get())
            .map(|_| {
                scope.spawn(|| {
                    let mut counts = vec![0u32; ROWS];
                    loop {
                        let index = cursor.fetch_add(1, Ordering::Relaxed);
                        if index >= samples {
                            break;
                        }
                        let value = (u128::from(OFFSET) + index as u128 * u128::from(STRIDE))
                            % u128::from(TOTAL_SEEDS);
                        let world = generate_main_world_with_trinket(
                            DungeonSeed::new(u64::try_from(value).unwrap()).unwrap(),
                            24,
                            Challenges::NONE,
                            selected,
                        )
                        .unwrap();
                        for identity in WAND_ITEMS {
                            let items: Vec<_> =
                                world.items.iter().filter(|i| i.item == identity).collect();
                            let mut earliest = [[25; COPIES]; BANDS];
                            subsets(&items, 0, &mut Vec::new(), &mut earliest);
                            let class = wand_repeats::class(identity);
                            for (minimum, depths) in earliest.into_iter().enumerate() {
                                for (copies, first) in depths.into_iter().enumerate() {
                                    let start =
                                        ((class * BANDS + minimum) * COPIES + copies) * DEPTHS;
                                    for count in
                                        &mut counts[start + usize::from(first - 1)..start + DEPTHS]
                                    {
                                        *count += 1;
                                    }
                                }
                            }
                        }
                    }
                    counts
                })
            })
            .collect();
        let mut counts = vec![0u32; ROWS];
        for worker in workers {
            for (a, b) in counts.iter_mut().zip(worker.join().unwrap()) {
                *a += b;
            }
        }
        counts
    });
    let report = serde_json::json!({"samples":samples,"profile":selected.map_or("none",|id|item(id).stable_id),"seed_offset":OFFSET,"seed_stride":STRIDE,"elapsed_seconds":started.elapsed().as_secs_f64(),"bands":BANDS,"counts":counts});
    std::fs::write(&args[1], serde_json::to_vec_pretty(&report).unwrap()).unwrap();
    eprintln!("{samples} worlds, {:?}", started.elapsed());
}

#[cfg(test)]
mod tests {
    use super::*;
    use shpd_seedfinder_core::{
        catalog::ItemId,
        model::{Accessibility, ItemSource},
    };

    #[test]
    fn availability_respects_joint_scenarios_levels_and_deadlines() {
        let wand = |depth, upgrade, mask| WorldItem {
            item: ItemId::WandFireblast,
            depth,
            upgrade,
            effect: None,
            cursed: false,
            source: ItemSource::Chest,
            secret: false,
            accessibility: Accessibility::Scenarios { group: 1, mask },
        };
        // Every pair is possible, but no acquisition plan yields all three.
        let items = [wand(2, 0, 0b011), wand(7, 2, 0b110), wand(9, 1, 0b101)];
        let mut earliest = [[25; COPIES]; BANDS];
        subsets(
            &items.iter().collect::<Vec<_>>(),
            0,
            &mut Vec::new(),
            &mut earliest,
        );
        assert_eq!(earliest[0], [2, 7, 25, 25]);
        assert_eq!(earliest[1], [7, 9, 25, 25]);
        assert_eq!(earliest[2], [7, 25, 25, 25]);
        assert_eq!(earliest[3], [25; COPIES]);
        assert_eq!(
            earliest[wand_repeats::anchor_band(None, 2).unwrap()],
            [7, 7, 25, 25]
        );
        assert_eq!(
            earliest[wand_repeats::anchor_band(Some(ItemSource::ImpReward), 0).unwrap()],
            [25; COPIES]
        );
    }
}
