//! Joint donor/deck layouts, with artifact identities integrated without replacement.
//! No world generation or random sampling occurs while estimating a query.
use super::{HIGHEST_TABLED_UPGRADE, Predicate, matching_chance, tally};
use crate::{
    catalog::ItemKind,
    model::ItemSource,
    probability_tables::{embedded, sources},
    query::EffectRequirement,
};
use std::{collections::HashMap, sync::OnceLock};

const CARDS: usize = 11;
const WORK_LIMIT: usize = 250_000;

struct Placement {
    depth: u8,
    source: ItemSource,
    cursed: bool,
}
struct Layout {
    placements: Vec<Placement>,
    scenarios: Vec<u16>,
}

fn layouts(profile: usize) -> &'static [Layout] {
    static LAYOUTS: OnceLock<Vec<Vec<Layout>>> = OnceLock::new();
    &LAYOUTS.get_or_init(|| {
        let bytes = &embedded::ARTIFACT_DECKS;
        assert_eq!(&bytes[..4], b"ADP1");
        let samples = u32::from_le_bytes(bytes[4..8].try_into().unwrap());
        let mut cursor = 8;
        let profiles = (0..8)
            .map(|_| {
                (0..samples)
                    .map(|_| {
                        let count = usize::from(bytes[cursor]);
                        cursor += 1;
                        assert!(count <= CARDS);
                        let placements = (0..count)
                            .map(|_| {
                                let record = &bytes[cursor..cursor + 2];
                                cursor += 2;
                                Placement {
                                    depth: record[0],
                                    source: sources()[usize::from(record[1] & 31)],
                                    cursed: record[1] & 32 != 0,
                                }
                            })
                            .collect();
                        let count = usize::from(bytes[cursor]);
                        cursor += 1;
                        let scenarios = (0..count)
                            .map(|_| {
                                let mask = u16::from_le_bytes(
                                    bytes[cursor..cursor + 2].try_into().unwrap(),
                                );
                                cursor += 2;
                                mask
                            })
                            .collect();
                        Layout {
                            placements,
                            scenarios,
                        }
                    })
                    .collect()
            })
            .collect::<Vec<_>>();
        assert_eq!(cursor, bytes.len());
        profiles
    })[profile]
}

fn eligible(layout: &Layout, predicate: Predicate, open_only: bool) -> u16 {
    layout
        .placements
        .iter()
        .enumerate()
        .fold(0, |mask, (index, entry)| {
            let upgrade = if entry.source == ItemSource::ImpReward {
                5
            } else {
                0
            };
            let allowed = entry.depth <= predicate.max_depth
                && predicate.source.is_none_or(|source| source == entry.source)
                && (!predicate.require_uncursed || !entry.cursed)
                && predicate.upgrades & (1 << upgrade) != 0
                && predicate.effect == EffectRequirement::Any
                && (!(open_only || predicate.exclude_imp) || entry.source != ItemSource::ImpReward);
            mask | if allowed { 1 << index } else { 0 }
        })
}

fn prefix(layout: &Layout, depth: u8) -> u16 {
    (1 << layout
        .placements
        .iter()
        .take_while(|entry| entry.depth <= depth)
        .count())
        - 1
}

fn tail(layout: &Layout, predicate: Predicate) -> u16 {
    let generated = prefix(layout, predicate.max_depth).count_ones();
    let count = u32::from(predicate.artifact_transmutations).min(11 - generated);
    ((1 << count) - 1) << generated
}

/// Split artifact success into runs which can keep the Imp prize for equipment
/// and runs which need to spend it on an artifact (natural or transmuted).
/// Other cross-family room correlations retain the existing supply approximation.
pub(super) fn mixed_probability(ordered: &[Predicate]) -> f64 {
    let artifacts: Vec<_> = ordered
        .iter()
        .filter(|p| p.kind == ItemKind::Artifact)
        .copied()
        .collect();
    let (all, open) = probability(&artifacts);
    if !all.is_finite() || all <= 0.0 {
        return all;
    }
    let mut equipment: Vec<_> = ordered
        .iter()
        .filter(|p| p.kind != ItemKind::Artifact)
        .copied()
        .collect();
    if equipment.is_empty() {
        return all;
    }
    let available = matching_chance(&equipment);
    if all <= open {
        return all * available;
    }
    for predicate in &mut equipment {
        predicate.exclude_imp = true;
    }
    open * available + (all - open) * matching_chance(&equipment)
}

pub(super) fn probability(predicates: &[Predicate]) -> (f64, f64) {
    if predicates.is_empty() {
        return (1.0, 1.0);
    }
    if predicates.len() > CARDS {
        return (0.0, 0.0);
    }
    // Exact transferred levels depend on unnamed intermediate identities.
    // Ordinary artifact editors use Any; do not silently guess for imported
    // upgrade-filtered transmutation queries until those paths are calibrated.
    let any_upgrade = (1 << (HIGHEST_TABLED_UPGRADE + 1)) - 1;
    if predicates
        .iter()
        .any(|p| p.artifact_transmutations > 0 && p.upgrades != any_upgrade)
    {
        return (f64::NAN, f64::NAN);
    }
    let worlds = layouts(predicates[0].profile as usize);
    let mut all = 0_u64;
    let mut open = 0_u64;
    let mut cache = HashMap::new();
    let mut work = 0;
    for layout in worlds {
        if predicates.len() == 1 {
            let predicate = predicates[0];
            let later = u64::from(tail(layout, predicate).count_ones());
            let natural = eligible(layout, predicate, false);
            let independent = eligible(layout, predicate, true);
            all += u64::from(natural.count_ones()) + if natural == 0 { 0 } else { later };
            open += u64::from(independent.count_ones()) + if independent == 0 { 0 } else { later };
        } else {
            for (open_only, total) in [(false, &mut all), (true, &mut open)] {
                let key = Signature::new(layout, predicates, open_only);
                let ways = if let Some(&ways) = cache.get(&key) {
                    ways
                } else {
                    let Some(ways) = key.assignments(&mut work) else {
                        return (f64::NAN, f64::NAN);
                    };
                    cache.insert(key, ways);
                    ways
                };
                *total += ways;
            }
        }
    }
    let denominator = (0..predicates.len()).fold(tally(worlds.len()), |n, i| n * tally(CARDS - i));
    #[allow(clippy::cast_precision_loss)] // At most 8,192 × 11!, exactly representable in f64.
    (all as f64 / denominator, open as f64 / denominator)
}

#[derive(Eq, Hash, PartialEq)]
struct Signature {
    natural: Vec<u16>,
    tails: Vec<u16>,
    prefixes: Vec<u16>,
    depths: Vec<u8>,
    scenarios: Vec<u16>,
}

impl Signature {
    fn new(layout: &Layout, predicates: &[Predicate], open_only: bool) -> Self {
        let natural: Vec<_> = predicates
            .iter()
            .map(|&p| eligible(layout, p, open_only))
            .collect();
        let union = natural.iter().fold(0, |mask, &next| mask | next);
        let mut scenarios: Vec<_> = layout.scenarios.iter().map(|mask| mask & union).collect();
        scenarios.sort_unstable();
        scenarios.dedup();
        Self {
            tails: predicates
                .iter()
                .zip(&natural)
                .map(|(&p, &donors)| if donors == 0 { 0 } else { tail(layout, p) })
                .collect(),
            prefixes: predicates
                .iter()
                .map(|p| prefix(layout, p.max_depth))
                .collect(),
            depths: predicates.iter().map(|p| p.max_depth).collect(),
            natural,
            scenarios,
        }
    }

    fn assignments(&self, work: &mut usize) -> Option<u64> {
        let needed = u32::try_from(self.natural.len()).unwrap();
        if self.scenarios.iter().all(|mask| mask.count_ones() < needed) {
            return Some(0);
        }
        // Nested limits over the same obtainable donors are falling factorials,
        // including all eleven requirements. Never enumerate 11! permutations.
        if self.natural.iter().all(|m| *m == self.natural[0])
            && self.depths.iter().all(|d| *d == self.depths[0])
            && self.scenarios.contains(&self.natural[0])
        {
            let mut sizes: Vec<_> = self
                .tails
                .iter()
                .map(|tail| (tail | self.natural[0]).count_ones())
                .collect();
            sizes.sort_unstable();
            return Some(sizes.into_iter().enumerate().fold(1, |ways, (i, size)| {
                ways * u64::from(size.saturating_sub(u32::try_from(i).unwrap()))
            }));
        }
        // Each state aggregates disjoint assignments of named identities to
        // card positions. Donor assignments are existential, not counted twice.
        let mut states = HashMap::from([((0_u16, 0_u16, 0_u16), 1_u64)]);
        for slot in 0..self.natural.len() {
            let mut next = HashMap::new();
            for ((cards, kept, transformed), ways) in states {
                let prior = (transformed != 0).then(|| transformed.trailing_zeros() as usize);
                let mut natural = self.natural[slot] & !cards;
                if let Some(prior) = prior {
                    natural &= self.prefixes[prior];
                }
                while natural != 0 {
                    let bit = natural & natural.wrapping_neg();
                    natural &= !bit;
                    *work += 1;
                    if *work > WORK_LIMIT {
                        return None;
                    }
                    if !self
                        .scenarios
                        .iter()
                        .any(|mask| mask & (kept | bit) == (kept | bit))
                    {
                        continue;
                    }
                    *next
                        .entry((cards | bit, kept | bit, transformed))
                        .or_insert(0) += ways;
                }
                if prior.is_some_and(|prior| self.depths[prior] != self.depths[slot])
                    || kept & !self.prefixes[slot] != 0
                {
                    continue;
                }
                let mut targets = self.tails[slot] & !cards;
                while targets != 0 {
                    let bit = targets & targets.wrapping_neg();
                    targets &= !bit;
                    *work += 1;
                    *next
                        .entry((cards | bit, kept, transformed | (1 << slot)))
                        .or_insert(0) += ways;
                }
                if *work > WORK_LIMIT {
                    return None;
                }
            }
            states = next;
        }
        Some(
            states
                .into_iter()
                .filter(|((_, kept, transformed), _)| {
                    self.scenarios.iter().any(|scenario| {
                        scenario & kept == *kept && self.can_donate(*transformed, scenario & !kept)
                    })
                })
                .map(|(_, ways)| ways)
                .sum(),
        )
    }

    fn can_donate(&self, remaining: u16, available: u16) -> bool {
        // Bipartite augmenting paths bound donor matching to O(11³), even for
        // overlapping filters that fail Hall's condition late in the search.
        let mut owners = [None; CARDS];
        (0..self.natural.len())
            .filter(|slot| remaining & (1 << slot) != 0)
            .all(|slot| self.claim_donor(slot, available, &mut owners, &mut 0))
    }

    fn claim_donor(
        &self,
        slot: usize,
        available: u16,
        owners: &mut [Option<usize>; CARDS],
        visited: &mut u16,
    ) -> bool {
        let mut donors = self.natural[slot] & available;
        while donors != 0 {
            let bit = donors & donors.wrapping_neg();
            donors &= !bit;
            if *visited & bit != 0 {
                continue;
            }
            *visited |= bit;
            let donor = bit.trailing_zeros() as usize;
            if owners[donor].is_none_or(|owner| self.claim_donor(owner, available, owners, visited))
            {
                owners[donor] = Some(slot);
                return true;
            }
        }
        false
    }
}
