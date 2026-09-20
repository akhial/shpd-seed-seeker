//! Surplus wand allocation under the ordinary matcher's acquisition choices.

use std::collections::BTreeMap;

use crate::catalog::{ItemKind, item};
use crate::model::{ItemSource, WorldItem};

use super::SearchQuery;

/// Filters on the surplus wands consumed for Arcane Resin.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ArcaneResinFilter {
    pub uncursed: bool,
    pub max_depth: Option<u8>,
    pub source: Option<ItemSource>,
}

impl Default for ArcaneResinFilter {
    fn default() -> Self {
        Self {
            uncursed: true,
            max_depth: None,
            source: None,
        }
    }
}

impl ArcaneResinFilter {
    pub(super) fn implies(self, base: Self, max_depth: u8) -> bool {
        (self.uncursed || !base.uncursed)
            && self.max_depth.unwrap_or(max_depth).min(max_depth)
                <= base.max_depth.unwrap_or(max_depth).min(max_depth)
            && base.source.is_none_or(|source| self.source == Some(source))
    }
}

pub(super) struct ResinSupply {
    minimum: u16,
    auto: bool,
    candidates: Vec<(usize, u32)>,
}

/// Resin charges the resulting upgrade level for each step, up to +3.
pub(crate) const fn upgrade_cost(upgrade: u8) -> u32 {
    match upgrade {
        0 => 6,
        1 => 5,
        2 => 3,
        _ => 0,
    }
}

impl ResinSupply {
    pub const fn enabled(&self) -> bool {
        self.auto || self.minimum > 0
    }

    pub fn prepare(query: &SearchQuery, items: &[WorldItem]) -> Self {
        if !query.needs_resin() {
            return Self {
                minimum: 0,
                auto: false,
                candidates: Vec::new(),
            };
        }
        Self {
            minimum: query.arcane_resin,
            auto: query.arcane_resin_auto,
            candidates: items
                .iter()
                .enumerate()
                .filter(|(_, candidate)| {
                    item(candidate.item).kind == ItemKind::Wand
                        && (!query.arcane_resin_filter.uncursed || !candidate.cursed)
                        && candidate.depth <= query.max_depth
                        && query
                            .arcane_resin_filter
                            .max_depth
                            .is_none_or(|depth| candidate.depth <= depth)
                        && query
                            .arcane_resin_filter
                            .source
                            .is_none_or(|source| candidate.source == source)
                        && (!query.exclude_blacksmith_rewards
                            || candidate.source != ItemSource::BlacksmithReward)
                })
                // Generated wands have no applied resin bonus. Count only
                // their generated upgrades, with no hero talent bonuses.
                .map(|(index, candidate)| (index, 2 * (u32::from(candidate.upgrade) + 1)))
                .collect(),
        }
    }

    /// Find sufficient surplus wands without changing the reserved items.
    /// Acquisition groups are independent of each other; within each group,
    /// choose the compatible scenario with the largest remaining resin yield.
    pub fn select(
        &self,
        items: &[WorldItem],
        used: &[bool],
        scenarios: &BTreeMap<u16, u64>,
    ) -> Option<Vec<usize>> {
        let minimum = if self.auto {
            items
                .iter()
                .zip(used)
                .filter(|(candidate, used)| **used && item(candidate.item).kind == ItemKind::Wand)
                .map(|(candidate, _)| upgrade_cost(candidate.upgrade))
                .sum()
        } else {
            u32::from(self.minimum)
        };
        if minimum == 0 {
            return Some(Vec::new());
        }
        let mut totals: BTreeMap<u16, [u32; 64]> = BTreeMap::new();
        for &(index, quantity) in &self.candidates {
            if used[index] {
                continue;
            }
            if let Some((group, mask)) = items[index].accessibility.scenario_constraint() {
                let mut allowed = mask & scenarios.get(&group).copied().unwrap_or(u64::MAX);
                let total = totals.entry(group).or_insert([0; 64]);
                while allowed != 0 {
                    total[allowed.trailing_zeros() as usize] += quantity;
                    allowed &= allowed - 1;
                }
            }
        }
        let choices: BTreeMap<u16, u64> = totals
            .into_iter()
            .map(|(group, totals)| {
                let best = (0..64).max_by_key(|&bit| totals[bit]).unwrap();
                (group, 1 << best)
            })
            .collect();
        let mut selected = Vec::new();
        let mut total = 0;
        for &(index, quantity) in &self.candidates {
            if used[index] {
                continue;
            }
            if let Some((group, mask)) = items[index].accessibility.scenario_constraint()
                && mask & choices[&group] & scenarios.get(&group).copied().unwrap_or(u64::MAX) == 0
            {
                continue;
            }
            selected.push(index);
            total += quantity;
            if total >= minimum {
                return Some(selected);
            }
        }
        None
    }
}
