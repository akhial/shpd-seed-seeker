//! Blanket witnesses share ordinary reservations or resin donors. Enumerate
//! compatible witness assignments, then combine their conditional estimates.
use super::{
    Predicate, Profile, effective_requirements, equipment_probability, filters, matching_chance,
    resin, sort_filters,
};
use crate::{
    catalog::ItemKind,
    query::{Requirement, SearchQuery, resin_donor_requirement},
};

const BRANCH_LIMIT: usize = 128;

pub(super) fn probability(query: &SearchQuery, profile: Profile) -> f64 {
    let ordinary = SearchQuery {
        requirements: query
            .requirements
            .iter()
            .filter(|r| !r.blanket)
            .copied()
            .collect(),
        ..query.clone()
    };
    let base = equipment_probability(&ordinary, profile);
    if base <= 0.0 || !base.is_finite() {
        return base;
    }
    let Some(variants) = ordinary_variants(&ordinary) else {
        return f64::NAN;
    };
    let mut branches: Vec<_> = variants
        .into_iter()
        .map(|requirements| {
            let variant = SearchQuery {
                requirements,
                ..ordinary.clone()
            };
            Branch {
                ordinary: filters(
                    &variant,
                    &effective_requirements(&variant, profile),
                    None,
                    &[],
                    profile,
                ),
                donors: Vec::new(),
            }
        })
        .collect();
    let unconstrained = branches
        .iter()
        .map(|branch| branch.chance(query, profile))
        .fold(0.0_f64, f64::max);
    if unconstrained <= 0.0 {
        return 0.0;
    }
    let donor = query.needs_resin().then(|| {
        let requirement = resin_donor_requirement(query);
        Predicate::of(requirement, None)
            .within(query, &requirement)
            .with_profile(profile)
    });
    let mut estimate = base;
    for slot in query.blanket_slots() {
        let mut next = Vec::new();
        for branch in &branches {
            for &member in &slot {
                let requirement = query.requirements[member];
                let blanket = Predicate::of(requirement, None)
                    .within(query, &requirement)
                    .with_profile(profile);
                for narrowed in branch.witnesses(blanket, donor, query.arcane_resin_auto) {
                    // Discard duplicate and stronger assignments, including
                    // extra donor copies when one donor witnesses both blankets.
                    if next.iter().any(|kept: &Branch| kept.covers(&narrowed)) {
                        continue;
                    }
                    next.retain(|kept| !narrowed.covers(kept));
                    next.push(narrowed);
                    if next.len() > BRANCH_LIMIT {
                        return f64::NAN;
                    }
                }
            }
        }
        branches = next;
        let missed = branches.iter().fold(1.0, |missed, branch| {
            missed * (1.0 - (branch.chance(query, profile) / unconstrained).clamp(0.0, 1.0))
        });
        estimate = estimate.min(base * (1.0 - missed));
    }
    // Optional combined-level members omitted by the existing approximation
    // can still witness a blanket in the exact matcher.
    if estimate <= 0.0 && !ordinary.level_sum_groups().is_empty() {
        return f64::NAN;
    }
    estimate.clamp(0.0, base)
}

#[derive(Clone)]
struct Branch {
    ordinary: Vec<Predicate>,
    donors: Vec<Predicate>,
}

impl Branch {
    fn chance(&self, query: &SearchQuery, profile: Profile) -> f64 {
        let mut ordinary = self.ordinary.clone();
        let mut donors = self.donors.clone();
        sort_filters(&mut ordinary);
        sort_filters(&mut donors);
        let mut allocated = ordinary.clone();
        allocated.extend(&donors);
        sort_filters(&mut allocated);
        let baseline = matching_chance(&allocated);
        if query.needs_resin() {
            resin::with_resin(query, profile, &ordinary, &donors, baseline)
        } else {
            baseline
        }
    }

    fn covers(&self, other: &Self) -> bool {
        self.ordinary.len() == other.ordinary.len()
            && predicate_branch_covers(&self.ordinary, &other.ordinary)
            && predicate_branch_covers(&self.donors, &other.donors)
    }

    fn witnesses(&self, blanket: Predicate, donor: Option<Predicate>, auto: bool) -> Vec<Self> {
        let mut branches = Vec::new();
        for (is_donor, predicates) in [(false, &self.ordinary), (true, &self.donors)] {
            for (index, &predicate) in predicates.iter().enumerate() {
                if let Some(intersection) = predicate.intersect(blanket) {
                    let mut narrowed = self.clone();
                    if is_donor {
                        narrowed.donors[index] = intersection;
                    } else {
                        narrowed.ordinary[index] = intersection;
                    }
                    branches.push(narrowed);
                }
            }
        }
        if let Some(intersection) = donor.and_then(|donor| donor.intersect(blanket)) {
            let mut narrowed = self.clone();
            narrowed.donors.push(intersection);
            // Auto consumes no donors when every reservation is already +3.
            // Require at least one +0..+2 reservation in each donor branch.
            if auto
                && !self
                    .ordinary
                    .iter()
                    .any(|p| p.kind == ItemKind::Wand && p.upgrades & !0b111 == 0)
            {
                for (index, predicate) in self.ordinary.iter().enumerate() {
                    if predicate.kind == ItemKind::Wand && predicate.upgrades & 0b111 != 0 {
                        let mut positive_cost = narrowed.clone();
                        positive_cost.ordinary[index].upgrades &= 0b111;
                        branches.push(positive_cost);
                    }
                }
            } else {
                branches.push(narrowed);
            }
        }
        branches
    }
}

/// Alternative choices before blanket filtering, with an explicit work cap.
fn ordinary_variants(query: &SearchQuery) -> Option<Vec<Vec<Requirement>>> {
    let mut variants = vec![Vec::new()];
    for slot in query.slots() {
        if variants.len() * slot.len() > BRANCH_LIMIT {
            return None;
        }
        variants = variants
            .into_iter()
            .flat_map(|chosen| {
                slot.iter().map(move |&index| {
                    let mut chosen = chosen.clone();
                    chosen.push(Requirement {
                        alternative_group: None,
                        ..query.requirements[index]
                    });
                    chosen
                })
            })
            .collect();
    }
    Some(variants)
}

/// A sufficient implication test, also identifying permutations of identical
/// copies so they cannot be counted as separate ways to satisfy a blanket.
fn predicate_branch_covers(broad: &[Predicate], narrow: &[Predicate]) -> bool {
    fn cover(
        broad: &[Predicate],
        narrow: &[Predicate],
        owners: &mut [Option<usize>],
        visited: &mut [bool],
        index: usize,
    ) -> bool {
        for (candidate, &predicate) in narrow.iter().enumerate() {
            if !visited[candidate] && predicate.intersect(broad[index]) == Some(predicate) {
                visited[candidate] = true;
                if owners[candidate]
                    .is_none_or(|owner| cover(broad, narrow, owners, visited, owner))
                {
                    owners[candidate] = Some(index);
                    return true;
                }
            }
        }
        false
    }
    let mut owners = vec![None; narrow.len()];
    broad.len() <= narrow.len()
        && (0..broad.len()).all(|index| {
            cover(
                broad,
                narrow,
                &mut owners,
                &mut vec![false; narrow.len()],
                index,
            )
        })
}
