//! Resin is estimated through sufficient sets of distinct donor wands. For
//! each split of the required levels across generated upgrades, score the
//! complete query and keep its most probable sufficient donor plan.
//! This is conservative, like the estimator's alternative and combined-level
//! groups: it does not sum overlapping donor plans. Every plan shares the
//! ordinary equipment allocation, including quest prizes, reserved wands,
//! and selected-trinket supply.

use super::{
    HIGHEST_TABLED_UPGRADE, Predicate, Profile, STATE_FLOOR, blanket_probability,
    equipment_probability, expected_slots,
};
use crate::{
    catalog::ItemKind,
    query::{EffectRequirement, Requirement, SearchQuery, TierRequirement, UpgradeRequirement},
};
use std::collections::BTreeMap;

pub(super) fn probability(query: &SearchQuery, profile: Profile) -> f64 {
    let donor = Requirement {
        kind: ItemKind::Wand,
        weapon_category: None,
        item: None,
        tier: TierRequirement::Any,
        upgrade: UpgradeRequirement::Any,
        effect: EffectRequirement::Any,
        require_uncursed: query.arcane_resin_filter.uncursed,
        select_trinket: false,
        blanket: false,
        source: query.arcane_resin_filter.source,
        identity_group: None,
        max_depth: query.arcane_resin_filter.max_depth,
        alternative_group: None,
        level_sum: None,
    };
    let means: Vec<_> = (0..=HIGHEST_TABLED_UPGRADE)
        .map(|upgrade| {
            expected_slots(
                &Predicate::of(
                    Requirement {
                        upgrade: UpgradeRequirement::AtLeast(upgrade),
                        ..donor
                    },
                    None,
                )
                .within(query, &donor)
                .with_profile(profile),
            )
        })
        .collect();
    let mut planned = query.clone();
    planned.arcane_resin = 0;
    let mut plans = Plans {
        query: planned,
        profile,
        donor,
        means,
        donors: Vec::new(),
        bounds: BTreeMap::new(),
        best: 0.0,
    };
    plans.visit(
        query.arcane_resin.div_ceil(2),
        HIGHEST_TABLED_UPGRADE + 1,
        0,
    );
    plans.best
}

struct Plans {
    query: SearchQuery,
    profile: Profile,
    donor: Requirement,
    /// Expected eligible wands at or above each upgrade.
    means: Vec<f64>,
    /// Synthetic donors remain separate from ordinary blanket witnesses.
    donors: Vec<Requirement>,
    bounds: BTreeMap<(u8, u16), f64>,
    best: f64,
}

impl Plans {
    /// Enumerate integer partitions of the required total in wand levels
    /// (upgrade + 1), highest first. Minimum upgrades also admit overpayment,
    /// so this covers every sufficient allocation without enumerating it twice.
    fn visit(&mut self, remaining: u16, level: u8, used: u16) {
        if self.best >= 1.0 || self.best.is_nan() {
            return;
        }
        let minimum_count = used + remaining.div_ceil(u16::from(level));
        if !self.can_improve(0, minimum_count) {
            return;
        }
        if remaining == 0 {
            let probability = if self.query.requirements.iter().any(|r| r.blanket) {
                blanket_probability(&self.query, self.profile, &self.donors)
            } else {
                let mut allocated = self.query.clone();
                allocated.requirements.extend_from_slice(&self.donors);
                equipment_probability(&allocated, self.profile)
            };
            // An unsupported blanket branch must stay unknown rather than
            // becoming a false zero through f64::max's NaN handling.
            self.best = if probability.is_nan() {
                probability
            } else {
                self.best.max(probability)
            };
            return;
        }
        let upgrade = level - 1;
        let maximum = remaining / u16::from(level);
        let from = if level == 1 { maximum } else { 0 };
        let original = self.donors.len();
        for count in from..=maximum {
            if !self.can_improve(upgrade, used + count) {
                break;
            }
            self.donors.truncate(original);
            self.donors.extend(std::iter::repeat_n(
                Requirement {
                    upgrade: UpgradeRequirement::AtLeast(upgrade),
                    ..self.donor
                },
                usize::from(count),
            ));
            let left = remaining - count * u16::from(level);
            if left == 0 || level > 1 {
                self.visit(left, upgrade.max(1), used + count);
            }
        }
        self.donors.truncate(original);
    }

    /// Every partial plan needs this many donors at this upgrade or better.
    /// Cache that simpler query as a tighter bound than its mean count alone;
    /// it also rules out spending the same quest prize several times. The
    /// analytical bound runs first, before allocating any donor requirements.
    fn can_improve(&mut self, upgrade: u8, count: u16) -> bool {
        let threshold = self.best.max(STATE_FLOOR);
        if count_bound(self.means[usize::from(upgrade)], count) <= threshold {
            return false;
        }
        if count == 0 {
            return true;
        }
        let probability = self.bounds.entry((upgrade, count)).or_insert_with(|| {
            let mut bound = self.query.clone();
            bound.requirements = vec![
                Requirement {
                    upgrade: UpgradeRequirement::AtLeast(upgrade),
                    ..self.donor
                };
                usize::from(count)
            ];
            equipment_probability(&bound, self.profile)
        });
        *probability > threshold
    }
}

/// Chernoff's upper bound on the donor-count tail for the model's independent
/// Bernoulli/Poisson supply. Prune plans that cannot improve the estimate or
/// whose entire probability is below the matching state's numerical floor.
/// Large resin amounts are bounded before any synthetic slots are allocated.
fn count_bound(mean: f64, count: u16) -> f64 {
    if count == 0 {
        return 1.0;
    }
    if mean <= 0.0 {
        return 0.0;
    }
    let count = f64::from(count);
    if count <= mean {
        1.0
    } else {
        (count * (1.0 + (mean / count).ln()) - mean).exp()
    }
}
