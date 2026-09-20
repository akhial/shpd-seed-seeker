//! Sparse coverage sets and matching without a requirement-count bit mask.
use super::{Predicate, Supply};

pub(super) struct Coverages {
    predicates: Vec<Option<Predicate>>,
    supersets: Vec<Vec<usize>>,
    choices: Vec<Vec<usize>>,
}

impl Coverages {
    pub(super) fn of(ordered: &[Predicate]) -> Self {
        // Intersect distinct filters, not all subsets of requirement indices.
        // A hundred identical requirements have one intersection, while
        // mutually exclusive identities never produce a shared coverage.
        let mut intersections = Vec::new();
        for &predicate in ordered {
            if intersections.contains(&predicate) {
                continue;
            }
            let previous = intersections.len();
            intersections.push(predicate);
            for index in 0..previous {
                if let Some(shared) = intersections[index].intersect(predicate) {
                    if !intersections.contains(&shared) {
                        intersections.push(shared);
                    }
                }
            }
        }
        // An intersection also satisfies every broader filter, including all
        // identical copies. Only these closed sets can be an item's coverage.
        let mut sets: Vec<_> = intersections
            .into_iter()
            .map(|predicate| {
                let members: Vec<_> = ordered
                    .iter()
                    .enumerate()
                    .filter_map(|(index, &other)| {
                        (predicate.intersect(other) == Some(predicate)).then_some(index)
                    })
                    .collect();
                (members, Some(predicate))
            })
            .filter(|(members, _)| !members.is_empty())
            .collect();
        // Preserve the old subset order without representing it as an integer.
        // Every strict superset then follows the set it contains.
        sets.sort_by(|left, right| left.0.iter().rev().cmp(right.0.iter().rev()));
        sets.insert(0, (Vec::new(), None));
        let supersets = sets
            .iter()
            .enumerate()
            .map(|(index, (members, _))| {
                sets.iter()
                    .enumerate()
                    .skip(index + 1)
                    .filter_map(|(other, (larger, _))| {
                        members
                            .iter()
                            .all(|member| larger.binary_search(member).is_ok())
                            .then_some(other)
                    })
                    .collect()
            })
            .collect();
        let mut choices = vec![Vec::new(); ordered.len()];
        for (coverage, (members, _)) in sets.iter().enumerate() {
            for &requirement in members {
                choices[requirement].push(coverage);
            }
        }
        Self {
            predicates: sets.into_iter().map(|(_, predicate)| predicate).collect(),
            supersets,
            choices,
        }
    }

    pub(super) fn len(&self) -> usize {
        self.predicates.len()
    }

    pub(super) fn members(&self, coverage: usize) -> Vec<usize> {
        self.choices
            .iter()
            .enumerate()
            .filter_map(|(index, choices)| choices.contains(&coverage).then_some(index))
            .collect()
    }

    /// Convert the probability of satisfying at least a set of filters into
    /// the probability of satisfying exactly that set, subtracting supersets.
    pub(super) fn shares(&self, supply: &Supply, depth: usize) -> Vec<f64> {
        let mut exact: Vec<_> = self
            .predicates
            .iter()
            .map(|predicate| predicate.map_or(1.0, |p| p.slot_probability(supply, depth)))
            .collect();
        for index in (0..exact.len()).rev() {
            for &superset in &self.supersets[index] {
                exact[index] -= exact[superset];
            }
        }
        for share in &mut exact {
            *share = share.max(0.0);
        }
        exact
    }

    /// Capacitated bipartite matching: each requirement needs one item, and a
    /// coverage's count is how many interchangeable items it supplies.
    pub(super) fn matches(&self, counts: &[usize]) -> bool {
        if counts.iter().sum::<usize>() < self.choices.len() {
            return false;
        }
        let mut held = vec![Vec::new(); self.len()];
        (0..self.choices.len()).all(|requirement| {
            self.assign(requirement, counts, &mut held, &mut vec![false; self.len()])
        })
    }

    fn assign(
        &self,
        requirement: usize,
        counts: &[usize],
        held: &mut [Vec<usize>],
        visited: &mut [bool],
    ) -> bool {
        for &coverage in &self.choices[requirement] {
            if visited[coverage] || counts[coverage] == 0 {
                continue;
            }
            visited[coverage] = true;
            if held[coverage].len() < counts[coverage] {
                held[coverage].push(requirement);
                return true;
            }
            // Moving an earlier assignment can free the only slot the current
            // requirement accepts; greedy first-fit would miss these matches.
            for slot in 0..held[coverage].len() {
                if self.assign(held[coverage][slot], counts, held, visited) {
                    held[coverage][slot] = requirement;
                    return true;
                }
            }
        }
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{json_query, probability_tables::trinkets::Profile};

    #[test]
    fn duplicate_requirements_share_storage_but_consume_distinct_items() {
        let query = json_query::decode(r#"{"requirements":[{"kind":"armor"}]}"#).unwrap();
        let predicate = Predicate::of(query.requirements[0], None);
        let coverages = Coverages::of(&vec![predicate; 128]);
        assert_eq!(coverages.len(), 2);
        assert!(!coverages.matches(&[0, 127]));
        assert!(coverages.matches(&[0, 128]));
    }

    #[test]
    fn matching_agrees_with_exhaustive_hall_conditions() {
        // Every possible three-requirement coverage, with zero, one or two
        // items of each. This includes cases where an earlier assignment
        // must move so that a more constrained requirement can take its slot.
        let wanted = 3;
        let size = 1 << wanted;
        let coverages = Coverages {
            predicates: vec![None; size],
            supersets: Vec::new(),
            choices: (0..wanted)
                .map(|requirement| {
                    (1..size)
                        .filter(|coverage| coverage & (1 << requirement) != 0)
                        .collect()
                })
                .collect(),
        };
        for mut encoded in 0..3_usize.pow(7) {
            let mut counts = vec![0; size];
            for count in counts.iter_mut().skip(1) {
                *count = encoded % 3;
                encoded /= 3;
            }
            let hall = (1..size).all(|group| {
                let held: usize = (1..size)
                    .filter(|coverage| coverage & group != 0)
                    .map(|coverage| counts[coverage])
                    .sum();
                held >= group.count_ones() as usize
            });
            assert_eq!(coverages.matches(&counts), hall, "{counts:?}");
        }
    }

    #[test]
    fn sparse_shares_match_dense_subset_inversion() {
        for document in [
            r#"{"requirements":[{"kind":"armor","upgrade":{"at_least":1},"max_depth":9},{"kind":"armor","upgrade":{"at_least":2}},{"item":"mail_armor"},{"kind":"armor","tier":{"at_most":3},"uncursed":true},{"kind":"armor","effect":"any_enchantment"}]}"#,
            r#"{"requirements":[{"kind":"wand"},{"kind":"wand","upgrade":2},{"item":"wand_fireblast"},{"kind":"wand","max_depth":4},{"kind":"wand","source":"shop"}]}"#,
            r#"{"requirements":[{"kind":"weapon"},{"kind":"melee_weapon","upgrade":2},{"kind":"thrown_weapon"},{"kind":"weapon","tier":{"at_least":3}},{"kind":"weapon","uncursed":true}]}"#,
        ] {
            let query = json_query::decode(document).unwrap();
            for profile in [Profile::None, Profile::MimicTooth, Profile::ParchmentScrap] {
                let ordered =
                    super::super::filters(&query, &query.requirements, None, &[], profile);
                let sparse = Coverages::of(&ordered);
                let dense: Vec<_> = (0..1 << ordered.len())
                    .map(|mask| {
                        let mut members = ordered
                            .iter()
                            .enumerate()
                            .filter_map(|(index, &p)| (mask & (1 << index) != 0).then_some(p));
                        let first = members.next()?;
                        members.try_fold(first, Predicate::intersect)
                    })
                    .collect();
                for supply in profile.supply_for(ordered[0].kind) {
                    for depth in 1..=24 {
                        let mut expected: Vec<_> = dense
                            .iter()
                            .map(|predicate| {
                                predicate.map_or(0.0, |p| p.slot_probability(&supply, depth))
                            })
                            .collect();
                        expected[0] = 1.0;
                        for requirement in 0..ordered.len() {
                            let bit = 1 << requirement;
                            for coverage in 0..expected.len() {
                                if coverage & bit == 0 {
                                    expected[coverage] -= expected[coverage | bit];
                                }
                            }
                        }
                        let mut actual = vec![0.0; expected.len()];
                        for (coverage, share) in
                            sparse.shares(&supply, depth).into_iter().enumerate()
                        {
                            let mask = sparse.choices.iter().enumerate().fold(
                                0,
                                |mask, (index, choices)| {
                                    mask | if choices.contains(&coverage) {
                                        1 << index
                                    } else {
                                        0
                                    }
                                },
                            );
                            actual[mask] = share;
                        }
                        for (actual, expected) in actual.into_iter().zip(expected) {
                            assert!(
                                (actual - expected.max(0.0)).abs() < 1e-10,
                                "{profile:?} {document} {:?} floor {depth}: {actual} vs {expected}",
                                supply.source
                            );
                        }
                    }
                }
            }
        }
    }
}
