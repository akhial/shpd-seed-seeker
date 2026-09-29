//! Member stacks: identity labels carried by some members of an alternative
//! group.
//!
//! An identity group is a stack. Its *copies* are the requirements carrying
//! the label outside any alternative group, bare copies of the anchor's
//! kind. When the anchor is an alternative group, the label on its members
//! binds the copies to those members: the copies are required, as copies of
//! the matched item, exactly when a member carrying the label fills the
//! group's slot, and waived when another member does. So
//! `{Frost ×2 | Disintegration}` — the label on Frost, one copy — asks for
//! two Wands of Frost or one Wand of Disintegration, and
//! `{Frost ×2 | Disintegration ×3}` gives each member a label of its own.
//! When every member carries the label, whichever member fills the slot
//! carries it: the copies are always required ("N of the same item,
//! whichever alternative matched").
//!
//! A waived copy is not a slot of the match at all: it reserves no item,
//! costs no Arcane Resin, and counts as satisfied for the scout.

use super::{Requirement, SearchQuery};

/// For each requirement, the alternative group whose filling member decides
/// whether it is needed: set on the copies of a member stack — a label some
/// but not all members of an alternative group carry. Copies of a stack
/// every member of its group carries, copies of a lone anchor, and every
/// other requirement are always needed and read `None`.
///
/// Empty — for every requirement — when the list holds no member stack,
/// which is decided without allocating: the matcher asks once per world.
/// Validation lets a member stack's label sit on one alternative group and
/// bare copies only; on an invalid list the first gating group wins.
pub(crate) fn stack_gates(requirements: &[Requirement]) -> Vec<Option<u8>> {
    // Most lists have no member stack: find one before allocating.
    if !requirements.iter().any(|member| {
        member.alternative_group.is_some()
            && member.identity_group.is_some_and(|label| {
                gating_group(requirements, label).is_some()
                    && requirements.iter().any(|copy| {
                        copy.alternative_group.is_none() && copy.identity_group == Some(label)
                    })
            })
    }) {
        return Vec::new();
    }
    requirements
        .iter()
        .map(|copy| {
            if copy.alternative_group.is_some() {
                return None;
            }
            gating_group(requirements, copy.identity_group?)
        })
        .collect()
}

/// The alternative group that gates the copies of stack `label`: the first
/// group holding both a member carrying the label and one that does not.
pub(crate) fn gating_group(requirements: &[Requirement], label: u8) -> Option<u8> {
    requirements
        .iter()
        .filter(|member| member.identity_group == Some(label))
        .filter_map(|member| member.alternative_group)
        .find(|&group| {
            requirements.iter().any(|other| {
                other.alternative_group == Some(group) && other.identity_group != Some(label)
            })
        })
}

/// The gating group of requirement `index` — set when it is a member
/// stack's copy — in `gates`, a [`stack_gates`] answer (which may be empty).
pub(crate) fn gate_of(gates: &[Option<u8>], index: usize) -> Option<u8> {
    gates.get(index).copied().flatten()
}

/// Most variants [`member_stack_variants`] spells out.
const MAX_VARIANTS: usize = 64;

/// The query as the union of queries without member stacks: one variant per
/// way of filling each gating alternative group — by the members carrying
/// one label, or by the members carrying none — holding only those members
/// of the group and dropping the copies the choice waives. A world matches
/// the query exactly when it matches some variant. Every variant's copies
/// are then always required, so a consumer that only understands stacks
/// every member carries (or none) can take the best of the variants.
///
/// `None` when the query holds no member stack, or when spelling it out
/// would take more than [`MAX_VARIANTS`] queries.
pub(crate) fn member_stack_variants(query: &SearchQuery) -> Option<Vec<SearchQuery>> {
    let gates = stack_gates(&query.requirements);
    if gates.is_empty() {
        return None;
    }
    // Each gating group with the member labels it can be filled under.
    let mut choices: Vec<(u8, Vec<Option<u8>>)> = Vec::new();
    for group in gates.iter().flatten().copied() {
        if choices.iter().any(|(seen, _)| *seen == group) {
            continue;
        }
        let mut labels = Vec::new();
        for member in &query.requirements {
            if member.alternative_group == Some(group) && !labels.contains(&member.identity_group) {
                labels.push(member.identity_group);
            }
        }
        choices.push((group, labels));
    }
    let count = choices
        .iter()
        .try_fold(1_usize, |count, (_, labels)| {
            count.checked_mul(labels.len())
        })
        .filter(|&count| count <= MAX_VARIANTS)?;
    let mut variants = Vec::with_capacity(count);
    for mut number in 0..count {
        let mut chosen = Vec::with_capacity(choices.len());
        for (group, labels) in &choices {
            chosen.push((*group, labels[number % labels.len()]));
            number /= labels.len();
        }
        let label_of = |group: u8| {
            chosen
                .iter()
                .find(|(chosen, _)| *chosen == group)
                .map(|(_, label)| *label)
        };
        let requirements = query
            .requirements
            .iter()
            .enumerate()
            .filter(|&(index, requirement)| {
                if let Some(group) = requirement.alternative_group
                    && let Some(label) = label_of(group)
                {
                    return requirement.identity_group == label;
                }
                gate_of(&gates, index)
                    .and_then(label_of)
                    .is_none_or(|label| label == requirement.identity_group)
            })
            .map(|(_, requirement)| *requirement)
            .collect();
        variants.push(SearchQuery {
            requirements,
            ..query.clone()
        });
    }
    Some(variants)
}
