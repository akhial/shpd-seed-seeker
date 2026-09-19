//! Surplus wand allocation under the ordinary matcher's acquisition choices.

use std::collections::BTreeMap;

use crate::catalog::{ItemKind, item};
use crate::model::{ItemSource, WorldItem};

use super::SearchQuery;

pub(super) struct ResinSupply {
    pub minimum: u16,
    candidates: Vec<(usize, u32)>,
}

impl ResinSupply {
    pub fn prepare(query: &SearchQuery, items: &[WorldItem]) -> Self {
        if query.arcane_resin == 0 {
            return Self {
                minimum: 0,
                candidates: Vec::new(),
            };
        }
        Self {
            minimum: query.arcane_resin,
            candidates: items
                .iter()
                .enumerate()
                .filter(|(_, candidate)| {
                    item(candidate.item).kind == ItemKind::Wand
                        && !candidate.cursed
                        && candidate.depth <= query.max_depth
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
        if self.minimum == 0 {
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
            if total >= u32::from(self.minimum) {
                return Some(selected);
            }
        }
        None
    }
}
