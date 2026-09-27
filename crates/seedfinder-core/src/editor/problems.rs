//! What is wrong with a requirement list, in the words the editor shows.
//!
//! The engine's own check, [`crate::query::SearchQuery::validate`], stops at
//! the first error and names no requirement, which is right for a search
//! that must refuse to start but useless on a board: a chip needs to know
//! whether it is at fault, and the footer wants the whole list. Every
//! platform therefore kept its own message table — each worded, ordered and
//! blamed differently. The list here is the one they share: every row's
//! problems in list order, then every problem between rows (blaming each
//! row of the group), then the list's own.
//!
//! The wording is the web editor's, whose messages the platforms' tables
//! paraphrased. Query-level checks — the floor limit, the resin amount, an
//! empty query, floor requirements — stay with the platforms, which own
//! those settings; the one list-level check here is about the rows.

use crate::artifacts::TRANSMUTATION_COUNT as ARTIFACT_TRANSMUTATIONS;
use crate::catalog::{EXTRA_UPGRADE_MAXIMUM, EXTRA_UPGRADE_TIER, Effect, ItemKind, item};
use crate::query::{
    BOUNDED_TIER_MAX, BOUNDED_TIER_MIN, EXACT_TIER_MAX, EXACT_TIER_MIN, EffectRequirement,
    MAX_IDENTITY_GROUP, MAX_LEVEL_SUM_GROUP, MAX_SEARCH_DEPTH, QueryError, RESERVED_GROUP,
    Requirement, TierRequirement, UpgradeRequirement, requirement_group_errors,
};
use crate::trinkets::TRANSMUTATION_COUNT as TRINKET_TRANSMUTATIONS;

use super::Row;

/// What a [`Problem`] is about.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum ProblemScope {
    /// One requirement on its own; [`Problem::keys`] names it.
    Row,
    /// Requirements that disagree with each other — a stack, a combined
    /// level, an either/or group; [`Problem::keys`] names every row of it.
    Group,
    /// The list as a whole; [`Problem::keys`] is empty.
    List,
}

/// One thing the list must fix before it can be searched.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Problem {
    /// A sentence the editor shows as it is.
    pub message: String,
    /// The rows at fault, in list order — hidden copies included, since a
    /// hand-written list can put a problem on a copy the board folds away.
    pub keys: Vec<u64>,
    pub scope: ProblemScope,
}

/// The list-level problem of a list holding only blanket requirements:
/// blankets constrain the items the ordinary requirements reserve, so
/// without one they constrain nothing.
pub const NO_ORDINARY_REQUIREMENT: &str = "Add at least one ordinary requirement.";

/// Everything wrong with one requirement on its own, in the web editor's
/// order and wording (`validateRequirement`), minus the states a
/// [`Requirement`] cannot hold (no category, an empty effect set, a
/// non-boolean flag) and plus the two only it can (a melee/thrown narrowing
/// outside weapons, the reserved either/or label).
///
/// The list is empty exactly when [`Requirement::validate`] passes and any
/// stack or combined-level label lies in `1..=4` ([`MAX_IDENTITY_GROUP`],
/// [`MAX_LEVEL_SUM_GROUP`]), the labels every platform's model and the
/// portable formats can carry.
#[must_use]
#[allow(clippy::too_many_lines)] // One check per predicate, in the web's order.
pub fn row_problems(requirement: &Requirement) -> Vec<String> {
    let mut problems: Vec<String> = Vec::new();
    let mut say = |message: &str| problems.push(message.to_owned());
    let family = requirement.kind;

    if requirement.artifact_transmutations > ARTIFACT_TRANSMUTATIONS {
        say(&format!(
            "Choose an artifact transmutation count from 0 to {ARTIFACT_TRANSMUTATIONS}."
        ));
    }
    if requirement.artifact_transmutations > 0 && family != ItemKind::Artifact {
        say("Only artifacts can use artifact transmutations.");
    }
    if requirement.trinket_transmutations > TRINKET_TRANSMUTATIONS {
        say(&format!(
            "Choose a transmutation count from 1 to {TRINKET_TRANSMUTATIONS}."
        ));
    }
    if requirement.trinket_transmutations > 0 && family != ItemKind::Trinket {
        say("Only trinkets can require transmutations.");
    }
    if requirement.trinket_transmutations > 0 && requirement.select_trinket {
        say("Only an initial offer can be chosen at +3.");
    }
    if requirement.exclude_resin && (family != ItemKind::Wand || requirement.blanket) {
        say("Only an ordinary wand can exclude Auto resin.");
    }
    if requirement.blanket
        && (requirement.identity_group.is_some()
            || requirement.level_sum.is_some()
            || requirement.select_trinket)
    {
        say(
            "A blanket requirement cannot request extra copies, combined levels, or trinket \
             selection.",
        );
    }
    if requirement.select_trinket && family != ItemKind::Trinket {
        say("Only a trinket can be selected.");
    }
    if family == ItemKind::Trinket && requirement.item.is_none() {
        say("Select a trinket.");
    }
    if family == ItemKind::Artifact && requirement.item.is_none() {
        say("Select an artifact.");
    }

    if let Some(item_id) = requirement.item {
        if item(item_id).kind != family {
            say("The item does not belong to this category.");
        } else if family == ItemKind::Weapon
            && let Some(category) = requirement.weapon_category
            && item_id.weapon_category() != Some(category)
        {
            say(&format!(
                "The item is not a {} weapon.",
                super::labels::weapon_type_label(Some(category)).to_ascii_lowercase()
            ));
        }
    }
    // The web folds the narrowing into the kind ("melee_weapon"), so it
    // cannot put one on a wand; a typed requirement can.
    if requirement.weapon_category.is_some() && family != ItemKind::Weapon {
        say("Only a weapon can be melee or thrown.");
    }

    if requirement.tier != TierRequirement::Any {
        if requirement.item.is_some() || !matches!(family, ItemKind::Weapon | ItemKind::Armor) {
            say("Tier filters require a wildcard weapon or armor.");
        }
        match requirement.tier {
            TierRequirement::Exact(tier) if !(EXACT_TIER_MIN..=EXACT_TIER_MAX).contains(&tier) => {
                say(&format!(
                    "Exact tier must be {EXACT_TIER_MIN} through {EXACT_TIER_MAX}."
                ));
            }
            TierRequirement::AtLeast(tier) | TierRequirement::AtMost(tier)
                if !(BOUNDED_TIER_MIN..=BOUNDED_TIER_MAX).contains(&tier) =>
            {
                say(&format!(
                    "Tier bounds must be {BOUNDED_TIER_MIN} or {BOUNDED_TIER_MAX}."
                ));
            }
            _ => {}
        }
    }

    let bound = match requirement.upgrade {
        UpgradeRequirement::Any => None,
        UpgradeRequirement::Exact(upgrade) => Some((upgrade, 1)),
        UpgradeRequirement::AtLeast(upgrade) => Some((upgrade, 0)),
    };
    if let Some((upgrade, minimum)) = bound {
        let maximum = requirement.upgrade_ceiling();
        if upgrade < minimum || upgrade > maximum {
            // A weapon that cannot be tier 4 misses the vault's +5; say why.
            say(
                &if maximum < family.maximum_search_upgrade_for_tier(EXTRA_UPGRADE_TIER) {
                    format!(
                        "Upgrade must be {minimum} through +{maximum}; only a \
                         tier-{EXTRA_UPGRADE_TIER} weapon reaches +{EXTRA_UPGRADE_MAXIMUM}."
                    )
                } else {
                    format!("Upgrade must be {minimum} through +{maximum}.")
                },
            );
        }
    }

    if requirement
        .max_depth
        .is_some_and(|depth| !(1..=MAX_SEARCH_DEPTH).contains(&depth))
    {
        say(&format!(
            "Requirement floor must be 1 through {MAX_SEARCH_DEPTH}."
        ));
    }

    if let EffectRequirement::OneOf(set) = requirement.effect {
        if !matches!(family, ItemKind::Weapon | ItemKind::Armor) {
            say("Effects require a weapon or armor category.");
        } else if set.family() != family {
            let names: Vec<&str> = set.effects().map(Effect::wire_name).collect();
            say(&format!(
                "The effect {} does not belong to this category.",
                names.join(", ")
            ));
        } else if requirement.require_uncursed && set.is_curses_only() {
            say(if set.count() == 1 {
                "An uncursed item cannot have a curse effect."
            } else {
                "An uncursed item cannot have only curse effects."
            });
        }
    }

    if let Some(sum) = requirement.level_sum {
        if !(1..=MAX_LEVEL_SUM_GROUP).contains(&sum.group) {
            say(&format!(
                "A combined-level group must be 1 through {MAX_LEVEL_SUM_GROUP}."
            ));
        }
        if sum.minimum_total < 1 {
            say("A combined level must be at least 1.");
        }
        if family != ItemKind::Ring {
            say("Only rings can count levels together.");
        }
        if requirement.alternative_group.is_some() {
            say("An either/or alternative cannot count a combined level.");
        }
    }
    if requirement
        .identity_group
        .is_some_and(|group| !(1..=MAX_IDENTITY_GROUP).contains(&group))
    {
        say(&format!(
            "A stack group must be 1 through {MAX_IDENTITY_GROUP}."
        ));
    }
    if requirement.alternative_group == Some(RESERVED_GROUP) {
        say("An either/or group must be 1 or higher.");
    }
    problems
}

/// A problem between rows as the editor words it (web `validateQuery`);
/// `members` is how many rows the group holds.
fn group_message(error: QueryError, members: usize) -> String {
    match error {
        QueryError::InconsistentLevelSum { .. } => {
            "A stack must share one combined level.".to_owned()
        }
        QueryError::UnattainableLevelSum {
            minimum_total,
            capacity,
            ..
        } => format!(
            "A combined level of {minimum_total} needs more items: these {members} can reach \
             {capacity}."
        ),
        QueryError::MixedBlanketAlternatives => {
            "An either/or group cannot mix ordinary and blanket requirements.".to_owned()
        }
        QueryError::InconsistentIdentityGroup => {
            "The copies of a stack must share its category.".to_owned()
        }
        QueryError::OverconstrainedIdentityGroup => {
            "Only one item of a stack can carry constraints; the extra copies are plain.".to_owned()
        }
        // The group check reports nothing else; should that change, the
        // engine's own wording beats silence.
        other => other.to_string(),
    }
}

/// A [`Problem`] with its rows named by their index in the list rather than
/// their key, for callers that attribute problems to board entries (which
/// hold indices, and which a list with a duplicate key would confuse).
pub(crate) struct IndexedProblem {
    pub(crate) message: String,
    pub(crate) rows: Vec<usize>,
    pub(crate) scope: ProblemScope,
}

/// [`problems`], by row index.
pub(crate) fn indexed_problems(rows: &[Row]) -> Vec<IndexedProblem> {
    let mut found = Vec::new();
    for (index, row) in rows.iter().enumerate() {
        for message in row_problems(&row.requirement) {
            found.push(IndexedProblem {
                message,
                rows: vec![index],
                scope: ProblemScope::Row,
            });
        }
    }
    let requirements: Vec<Requirement> = rows.iter().map(|row| row.requirement).collect();
    for (error, members) in requirement_group_errors(&requirements) {
        found.push(IndexedProblem {
            message: group_message(error, members.len()),
            rows: members,
            scope: ProblemScope::Group,
        });
    }
    if !rows.is_empty() && rows.iter().all(|row| row.requirement.blanket) {
        found.push(IndexedProblem {
            message: NO_ORDINARY_REQUIREMENT.to_owned(),
            rows: Vec::new(),
            scope: ProblemScope::List,
        });
    }
    found
}

/// Names the rows of `found`, [`indexed_problems`] of `rows`, by key.
pub(crate) fn keyed(rows: &[Row], found: Vec<IndexedProblem>) -> Vec<Problem> {
    found
        .into_iter()
        .map(|problem| Problem {
            message: problem.message,
            keys: problem
                .rows
                .into_iter()
                .map(|index| rows[index].key)
                .collect(),
            scope: problem.scope,
        })
        .collect()
}

/// Everything wrong with the list: every row's own problems in list order
/// (hidden copies included), then every problem between rows — the engine's
/// group checks, all of them rather than the first — blaming each row of the
/// group, then the list's own ([`NO_ORDINARY_REQUIREMENT`]).
///
/// Platforms gate Start and Share on their own query-level checks first and
/// then on the first problem here; the web prefixes it with `Requirement N:`,
/// N being the first key's 1-based position in the list.
#[must_use]
pub fn problems(rows: &[Row]) -> Vec<Problem> {
    keyed(rows, indexed_problems(rows))
}

#[cfg(test)]
mod tests;
