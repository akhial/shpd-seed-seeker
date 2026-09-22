//! Surplus wand allocation under the ordinary matcher's acquisition choices.

use std::collections::BTreeMap;

use crate::catalog::{ItemKind, item};
use crate::model::{ItemSource, WorldItem};

use super::{EffectRequirement, Requirement, SearchQuery, TierRequirement, UpgradeRequirement};

/// Copies hidden by the editors' stack badge are reserved for reforging, not
/// resin upgrades. Infer them from the portable relationships so older saved
/// queries receive the same rule without changing their documents or links.
pub(crate) fn reforge_copies(query: &SearchQuery) -> Vec<bool> {
    let requirements = &query.requirements;
    let mut copies = vec![false; requirements.len()];
    let mut groups: BTreeMap<u8, Vec<usize>> = BTreeMap::new();
    for (index, r) in requirements.iter().enumerate() {
        if r.kind == ItemKind::Wand
            && let Some(group) = r.identity_group
        {
            groups.entry(group).or_default().push(index);
        }
    }
    let mut extra_counts = BTreeMap::new();
    for members in groups.values() {
        let anchor = members
            .iter()
            .copied()
            .find(|&i| !requirements[i].is_bare())
            .unwrap_or(members[0]);
        let mut count = 0;
        for &index in members {
            let r = requirements[index];
            if index != anchor && r.alternative_group.is_none() && r.is_bare() {
                copies[index] = true;
                count += 1;
            }
        }
        extra_counts.insert(anchor, count);
    }
    // Named stacks fold plain repeats into the nearest preceding chip, up to
    // the three-copy limit shared by all editors. Constrained copies start a
    // separate chip, and alternatives never absorb independent named copies.
    let mut named = BTreeMap::new();
    for (index, r) in requirements.iter().enumerate() {
        if copies[index]
            || r.kind != ItemKind::Wand
            || r.blanket
            || r.alternative_group.is_some()
            || r.level_sum.is_some()
        {
            continue;
        }
        let Some(item) = r.item else { continue };
        let plain = Requirement { item: None, ..*r }.is_bare() && r.identity_group.is_none();
        if plain
            && let Some(count) = named.get_mut(&item)
            && *count < 3
        {
            copies[index] = true;
            *count += 1;
        } else {
            named.insert(item, 1 + extra_counts.get(&index).copied().unwrap_or(0));
        }
    }
    copies
}

/// The item filter shared by resin allocation and blanket witness planning.
pub(crate) fn donor_requirement(query: &SearchQuery) -> Requirement {
    Requirement {
        kind: ItemKind::Wand,
        weapon_category: None,
        item: None,
        tier: TierRequirement::Any,
        upgrade: UpgradeRequirement::Any,
        effect: EffectRequirement::Any,
        require_uncursed: query.arcane_resin_filter.uncursed,
        select_trinket: false,
        blanket: false,
        exclude_resin: false,
        source: query.arcane_resin_filter.source,
        identity_group: None,
        max_depth: query.arcane_resin_filter.max_depth,
        alternative_group: None,
        level_sum: None,
    }
}

/// Filters on the surplus wands consumed for Arcane Resin.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ArcaneResinFilter {
    /// Credit the starting Magic Missile wand recovered with Wand Preservation.
    /// This is a player-supplied assumption, independent of generated donor filters.
    pub include_mage_wand: bool,
    pub uncursed: bool,
    pub max_depth: Option<u8>,
    pub source: Option<ItemSource>,
}

impl Default for ArcaneResinFilter {
    fn default() -> Self {
        Self {
            include_mage_wand: false,
            uncursed: true,
            max_depth: None,
            source: None,
        }
    }
}

pub(super) struct ResinSupply {
    minimum: u16,
    auto: bool,
    credit: u32,
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
                credit: 0,
                candidates: Vec::new(),
            };
        }
        Self {
            minimum: query.arcane_resin,
            auto: query.arcane_resin_auto,
            credit: if query.arcane_resin_filter.include_mage_wand {
                2
            } else {
                0
            },
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
        auto_cost: u32,
        scenarios: &BTreeMap<u16, u64>,
    ) -> Option<Vec<usize>> {
        self.select_required(items, used, scenarios, self.minimum(auto_cost), &[])
    }

    fn minimum(&self, auto_cost: u32) -> u32 {
        let cost = if self.auto {
            auto_cost
        } else {
            u32::from(self.minimum)
        };
        cost.saturating_sub(self.credit)
    }

    /// Choose donors together with blanket witnesses, backtracking over loot
    /// choices. Scout may keep the selection satisfying the most blankets;
    /// the full matcher requires all of them.
    pub fn select_with_blankets(
        &self,
        items: &[WorldItem],
        used: &[bool],
        auto_cost: u32,
        scenarios: &BTreeMap<u16, u64>,
        blankets: &[Vec<usize>],
        require_all: bool,
    ) -> Option<Vec<usize>> {
        if blankets
            .iter()
            .all(|indices| indices.iter().any(|&index| used[index]))
        {
            return self.select(items, used, auto_cost, scenarios);
        }
        let minimum = self.minimum(auto_cost);
        if minimum == 0 {
            return (!require_all).then(Vec::new);
        }
        let blankets: Vec<_> = blankets
            .iter()
            .filter(|indices| !indices.iter().any(|&index| used[index]))
            .map(Vec::as_slice)
            .collect();
        let mut search = BlanketDonors {
            supply: self,
            items,
            used,
            minimum,
            blankets,
            require_all,
            best: None,
            best_count: 0,
        };
        search.visit(0, &mut Vec::new(), &mut scenarios.clone());
        search.best
    }

    fn select_required(
        &self,
        items: &[WorldItem],
        used: &[bool],
        scenarios: &BTreeMap<u16, u64>,
        minimum: u32,
        required: &[usize],
    ) -> Option<Vec<usize>> {
        if minimum == 0 {
            // Auto with no upgrade cost does not consume any donor wands.
            return Some(Vec::new());
        }
        let mut totals: BTreeMap<u16, [u32; 64]> = BTreeMap::new();
        for &(index, quantity) in &self.candidates {
            if used[index] || required.contains(&index) {
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
        let mut selected = required.to_vec();
        let mut total: u32 = self
            .candidates
            .iter()
            .filter(|(index, _)| required.contains(index))
            .map(|(_, quantity)| quantity)
            .sum();
        if total >= minimum {
            return Some(selected);
        }
        for &(index, quantity) in &self.candidates {
            if used[index] || required.contains(&index) {
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

struct BlanketDonors<'a> {
    supply: &'a ResinSupply,
    items: &'a [WorldItem],
    used: &'a [bool],
    minimum: u32,
    blankets: Vec<&'a [usize]>,
    require_all: bool,
    best: Option<Vec<usize>>,
    best_count: usize,
}

impl BlanketDonors<'_> {
    /// Pin a donor for each unresolved blanket before maximizing the remaining
    /// supply. A donor can witness several blankets and adds no Auto cost.
    fn visit(
        &mut self,
        slot: usize,
        required: &mut Vec<usize>,
        scenarios: &mut BTreeMap<u16, u64>,
    ) -> bool {
        let Some(selected) =
            self.supply
                .select_required(self.items, self.used, scenarios, self.minimum, required)
        else {
            return false;
        };
        let count = self
            .blankets
            .iter()
            .filter(|indices| indices.iter().any(|index| selected.contains(index)))
            .count();
        let complete = count == self.blankets.len();
        if (complete || !self.require_all) && (self.best.is_none() || count > self.best_count) {
            self.best_count = count;
            self.best = Some(selected);
        }
        if complete {
            return true;
        }
        if slot == self.blankets.len() || self.minimum == 0 {
            return false;
        }
        if self.blankets[slot]
            .iter()
            .any(|index| required.contains(index))
        {
            return self.visit(slot + 1, required, scenarios);
        }
        for &index in self.blankets[slot] {
            if self.used[index]
                || !self
                    .supply
                    .candidates
                    .iter()
                    .any(|&(donor, _)| donor == index)
            {
                continue;
            }
            let constraint = self.items[index].accessibility.scenario_constraint();
            let previous = if let Some((group, mask)) = constraint {
                let compatible = scenarios.get(&group).copied().unwrap_or(u64::MAX) & mask;
                if compatible == 0 {
                    continue;
                }
                scenarios.insert(group, compatible)
            } else {
                None
            };
            required.push(index);
            let complete = self.visit(slot + 1, required, scenarios);
            required.pop();
            if let Some((group, _)) = constraint {
                super::rewind(scenarios, Some((group, previous)));
            }
            if complete {
                return true;
            }
        }
        !self.require_all && self.visit(slot + 1, required, scenarios)
    }
}

#[cfg(test)]
mod reforge_tests {
    use super::*;

    #[test]
    fn copies_follow_visible_stack_boundaries_and_preserve_independent_wands() {
        for (requirements, expected) in [
            (
                r#"[{"item":"wand_frost"},{"item":"wand_frost"},{"item":"wand_frost"},{"item":"wand_frost"}]"#,
                vec![false, true, true, false],
            ),
            (
                r#"[{"item":"wand_frost","upgrade":2},{"item":"wand_frost","upgrade":2}]"#,
                vec![false, false],
            ),
            (
                r#"[{"item":"wand_frost"},{"item":"wand_frost","exclude_resin":true},{"item":"wand_frost"}]"#,
                vec![false, false, true],
            ),
            (r#"[{"kind":"wand"},{"kind":"wand"}]"#, vec![false, false]),
            (
                r#"[{"kind":"wand","identity_group":1,"max_depth":4},{"kind":"wand","identity_group":1,"upgrade":2},{"kind":"wand","identity_group":1}]"#,
                vec![true, false, true],
            ),
            (
                r#"[{"any_of":[{"item":"wand_frost"},{"item":"wand_lightning"}]},{"item":"wand_frost"},{"item":"wand_frost"}]"#,
                vec![false, false, false, true],
            ),
            (
                r#"[{"item":"wand_frost"},{"item":"wand_frost","blanket":true}]"#,
                vec![false, false],
            ),
        ] {
            let query = crate::json_query::decode(&format!(r#"{{"requirements":{requirements}}}"#))
                .unwrap();
            assert_eq!(reforge_copies(&query), expected, "{requirements}");
        }
    }
}
