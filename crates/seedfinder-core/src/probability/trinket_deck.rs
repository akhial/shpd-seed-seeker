//! Exact availability over the initial offers and bounded prefixes of the tail.
//! Condition on each four-card offer set, then place only requested identities
//! into intervals ending at the query's deadlines. Cards within an interval
//! are interchangeable. Equipment retains the existing supply approximation.

use super::{Profile, complete_with_trinkets, equipment_probability, trinket_mask};
use crate::auto_trinkets::AutoTrinketPolicy;
use crate::catalog::{ItemId, ItemKind};
use crate::query::SearchQuery;
use crate::seed::DungeonSeed;
use crate::trinkets::trinket_order;
use std::collections::BTreeMap;

#[allow(clippy::too_many_lines)] // Keep offer/depth conditioning and shared caches together.
pub(crate) fn probability(query: &SearchQuery, policy: Option<&AutoTrinketPolicy>) -> f64 {
    if query
        .requirements
        .iter()
        .any(|r| r.kind == ItemKind::Trinket && (r.source.is_some() || r.level_sum.is_some()))
    {
        return f64::NAN;
    }
    let identities = trinket_order(DungeonSeed::MIN);
    let ordinary_count = query.ordinary_slots().len();
    let slots: Vec<_> = query
        .ordinary_slots()
        .into_iter()
        .chain(query.blanket_slots())
        .collect();
    let equipment_slots: Vec<_> = slots
        .iter()
        .map(|members| {
            members
                .iter()
                .any(|&i| query.requirements[i].kind != ItemKind::Trinket)
        })
        .collect();
    let selected_mask = crate::trinkets::selection_slots(query)
        .iter()
        .flatten()
        .fold(0, |mask, r| {
            mask | identities
                .iter()
                .position(|id| Some(*id) == r.item)
                .map_or(0, |i| 1 << i)
        });
    let mut boundaries: Vec<_> = query
        .requirements
        .iter()
        .filter(|r| r.kind == ItemKind::Trinket)
        .map(|r| r.trinket_transmutations)
        .filter(|&n| n > 0)
        .chain([13])
        .collect();
    boundaries.sort_unstable();
    boundaries.dedup();
    let capacities: Vec<_> = boundaries
        .iter()
        .scan(0, |previous, &end| {
            let size = end - *previous;
            *previous = end;
            Some(size)
        })
        .collect();
    let independent = query.requirements.iter().all(|r| {
        !r.blanket
            && (r.kind != ItemKind::Trinket || (r.alternative_group.is_none() && r.item.is_some()))
    });
    let mut equipment = query.clone();
    equipment
        .requirements
        .retain(|r| r.kind != ItemKind::Trinket);
    let mut equipment_cache = BTreeMap::new();
    let mut residuals = BTreeMap::new();
    let mut total = 0.0;
    let mut work = 1_000_000_u32;
    for depth in 1..=3 {
        let initial: Vec<_> = slots
            .iter()
            .map(|members| trinket_mask(query, members, &identities, depth))
            .collect();
        let relevant = initial.iter().fold(0, |mask, bits| mask | bits);
        let tail: Vec<Vec<_>> = boundaries
            .iter()
            .map(|&end| {
                slots
                    .iter()
                    .map(|members| {
                        let eligible: Vec<_> = members
                            .iter()
                            .copied()
                            .filter(|&i| query.requirements[i].trinket_transmutations >= end)
                            .collect();
                        trinket_mask(query, &eligible, &identities, depth)
                    })
                    .collect()
            })
            .collect();
        let tail_relevant = tail.iter().flatten().fold(0, |mask, bits| mask | bits);
        let mut conditional = BTreeMap::new();
        for a in 0..14 {
            for b in a + 1..15 {
                for c in b + 1..16 {
                    for d in c + 1..17 {
                        let offered = (1 << a) | (1 << b) | (1 << c) | (1 << d);
                        let profile = policy.map_or_else(
                            || Profile::for_matches(offered & selected_mask, &identities),
                            |policy| {
                                policy
                                    .choose(&[
                                        identities[a],
                                        identities[b],
                                        identities[c],
                                        identities[d],
                                    ])
                                    .map_or(Profile::None, Profile::of)
                            },
                        );
                        let value = *conditional
                            .entry((offered & relevant, profile))
                            .or_insert_with(|| {
                                if independent {
                                    let chance =
                                        independent_chance(query, &identities, offered, relevant);
                                    return chance
                                        * *equipment_cache.entry(profile).or_insert_with(|| {
                                            equipment_probability(&equipment, profile)
                                        });
                                }
                                let mut masks: Vec<_> =
                                    initial.iter().map(|mask| mask & offered).collect();
                                let mut context = Tail {
                                    query,
                                    slots: &slots,
                                    equipment_slots: &equipment_slots,
                                    eligible: &tail,
                                    ordinary_count,
                                    residuals: &mut residuals,
                                    profile,
                                    work: &mut work,
                                };
                                context.place(
                                    tail_relevant & !offered,
                                    0,
                                    &mut capacities.clone(),
                                    &mut masks,
                                )
                            });
                        if value.is_nan() {
                            return value;
                        }
                        total += value;
                    }
                }
            }
        }
    }
    (total / (3.0 * 2380.0)).clamp(0.0, 1.0)
}

/// Assign tighter deadlines first: each earlier target uses one of the slots
/// available to every later target. Initial offers need no tail position.
fn independent_chance(
    query: &SearchQuery,
    identities: &[ItemId],
    offered: u32,
    relevant: u32,
) -> f64 {
    let mut seen = 0_u32;
    let mut deadlines = Vec::new();
    for r in query
        .requirements
        .iter()
        .filter(|r| r.kind == ItemKind::Trinket)
    {
        let Some(index) = identities.iter().position(|id| Some(*id) == r.item) else {
            return 0.0;
        };
        let bit = 1 << index;
        // Each identity occurs once and must pass its predicate at this depth.
        if seen & bit != 0 || relevant & bit == 0 {
            return 0.0;
        }
        seen |= bit;
        if offered & bit == 0 {
            deadlines.push(r.trinket_transmutations);
        }
    }
    deadlines.sort_unstable();
    let mut chance = 1.0;
    for (i, deadline) in deadlines.into_iter().enumerate() {
        let i = u8::try_from(i).expect("at most 17 identities");
        if deadline <= i {
            return 0.0;
        }
        chance *= f64::from(deadline - i) / f64::from(13 - i);
    }
    chance
}

struct Tail<'a> {
    query: &'a SearchQuery,
    slots: &'a [Vec<usize>],
    equipment_slots: &'a [bool],
    eligible: &'a [Vec<u32>],
    ordinary_count: usize,
    residuals: &'a mut BTreeMap<(Profile, Vec<usize>), f64>,
    profile: Profile,
    work: &'a mut u32,
}

impl Tail<'_> {
    fn place(
        &mut self,
        remaining: u32,
        placed: u8,
        capacities: &mut [u8],
        masks: &mut [u32],
    ) -> f64 {
        if *self.work == 0 {
            return f64::NAN;
        }
        *self.work -= 1;
        if remaining == 0 {
            return complete_with_trinkets(
                self.query,
                self.slots,
                &masks[..self.ordinary_count],
                self.equipment_slots,
                u32::MAX,
                0,
                &masks[self.ordinary_count..],
                &mut Vec::new(),
                self.residuals,
                self.profile,
            );
        }
        let bit = 1 << remaining.trailing_zeros();
        let mut probability = 0.0;
        for bucket in 0..capacities.len() {
            let capacity = capacities[bucket];
            if capacity == 0 {
                continue;
            }
            capacities[bucket] -= 1;
            for (mask, eligible) in masks.iter_mut().zip(&self.eligible[bucket]) {
                *mask |= eligible & bit;
            }
            probability += f64::from(capacity) / f64::from(13 - placed)
                * self.place(remaining & !bit, placed + 1, capacities, masks);
            for mask in masks.iter_mut() {
                *mask &= !bit;
            }
            capacities[bucket] += 1;
            if probability.is_nan() {
                break;
            }
        }
        probability
    }
}
