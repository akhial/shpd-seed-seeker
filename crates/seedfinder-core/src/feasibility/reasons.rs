//! Explanations for the same structural constraints used to block searches.

use super::{
    ALL_SOURCES, EffectPolicy, ITEMS, ItemId, ItemKind, ItemSource, QUESTS, QueryPlan, Quest,
    Requirement, SHOP_DEPTHS, SearchQuery, quest_for_source, source_feasible,
};
use crate::trinkets::{INITIAL_OFFER_COUNT, TRANSMUTATION_COUNT};

/// Each mandatory trinket slot consumes one distinct entry in a shared deck.
/// OR members share one slot and its loosest deadline. Mixed equipment slots
/// may avoid the deck entirely; blankets reuse an assigned item. Ignoring both
/// is conservative, and avoids charging for an optional or reused trinket.
fn trinket_capacity_reason(query: &SearchQuery) -> Option<String> {
    let limits: Vec<_> = query
        .ordinary_slots()
        .iter()
        .filter(|slot| {
            slot.iter().all(|&i| {
                let r = query.requirements[i];
                r.kind == ItemKind::Trinket && r.level_sum.is_none()
            })
        })
        .map(|slot| {
            slot.iter()
                .map(|&i| query.requirements[i].trinket_transmutations)
                .max()
                .expect("nonempty query slot")
        })
        .collect();
    for limit in 0..=TRANSMUTATION_COUNT {
        let needed = limits.iter().filter(|&&n| n <= limit).count();
        let available = INITIAL_OFFER_COUNT + usize::from(limit);
        if needed <= available {
            continue;
        }
        return Some(if limit == 0 {
            format!(
                "Requires {needed} initial trinket offers, but each seed offers only {INITIAL_OFFER_COUNT}."
            )
        } else {
            let noun = if limit == 1 {
                "transmutation"
            } else {
                "transmutations"
            };
            format!(
                "{needed} trinket requirements allow at most {limit} {noun}; only {available} distinct trinkets are reachable."
            )
        });
    }
    None
}

fn trinket_identity_reason(slots: &[Vec<Requirement>]) -> Option<String> {
    if slots.len() < 2 {
        return None;
    }
    let slots: Vec<_> = slots
        .iter()
        .filter(|slot| slot.first().is_some_and(|r| !r.blanket))
        .collect();
    let mut owners: Vec<_> = ITEMS
        .iter()
        .filter(|item| item.kind == ItemKind::Trinket)
        .map(|item| (item.id, None))
        .collect();
    for index in 0..slots.len() {
        let mut seen = vec![false; owners.len()];
        let mut needed = 0;
        if assign_trinket(index, &slots, &mut owners, &mut seen, &mut needed) {
            continue;
        }
        let choices = seen.iter().filter(|&&visited| visited).count();
        return Some(if choices == 1 {
            let id = owners[seen
                .iter()
                .position(|&visited| visited)
                .expect("one choice")]
            .0;
            format!(
                "{} is required more than once, but each trinket appears only once in the deck.",
                crate::catalog::item(id).name
            )
        } else {
            format!(
                "{needed} trinket requirements share only {choices} distinct choices; each trinket appears once in the deck."
            )
        });
    }
    None
}

// A failed augmenting path identifies a deficient set of slots and identities.
// Limits are deliberately ignored here: this is an optimistic identity check.
fn assign_trinket(
    index: usize,
    slots: &[&Vec<Requirement>],
    owners: &mut [(ItemId, Option<usize>)],
    seen: &mut [bool],
    needed: &mut usize,
) -> bool {
    *needed += 1;
    for candidate in 0..owners.len() {
        if seen[candidate]
            || !slots[index]
                .iter()
                .any(|r| r.item == Some(owners[candidate].0))
        {
            continue;
        }
        seen[candidate] = true;
        if owners[candidate]
            .1
            .is_none_or(|owner| assign_trinket(owner, slots, owners, seen, needed))
        {
            owners[candidate].1 = Some(index);
            return true;
        }
    }
    false
}

impl QueryPlan {
    pub(super) fn impossibility_reason(
        &self,
        query: &SearchQuery,
        profile: &impl Fn(&Requirement, ItemSource) -> Option<(u8, u8, EffectPolicy)>,
        deadline: &impl Fn(&Requirement, ItemSource, u8) -> Option<u8>,
    ) -> Option<String> {
        if let Some(reason) = trinket_identity_reason(&self.required_trinket_slots) {
            return Some(reason);
        }
        if let Some(reason) = trinket_capacity_reason(query) {
            return Some(reason);
        }
        if let Some(feeling) = query.floor_requirements.iter().find_map(|floor| {
            floor.feeling.filter(|&feeling| {
                floor.depth == 1 && feeling != crate::level_prelude::Feeling::None
            })
        }) {
            return Some(format!("Floor 1 cannot have the {feeling:?} feeling."));
        }
        if self.blacksmith_deadline == Some(0) {
            return Some(format!(
                "The Blacksmith requires floor 12 or later; the floor limit is {}.",
                query.max_depth
            ));
        }
        if self.wandmaker_deadline.is_some_and(|(_, depth)| depth == 0) {
            return Some(format!(
                "The Wandmaker requires floor 7 or later; the floor limit is {}.",
                query.max_depth
            ));
        }
        // Each dead slot has no remaining ordinary or quest source. Diagnose
        // the same source policies used to build it, including per-item caps.
        for (members, slot) in query.slots().iter().zip(&self.slots) {
            if slot
                .iter()
                .any(|p| p.open_deadline.is_some() || p.quests != 0)
            {
                continue;
            }
            let number = members[0] + 1;
            if slot.is_empty() && query.requirements[members[0]].blanket {
                return Some(format!(
                    "Blanket requirement {number} has no compatible required item or resin donor."
                ));
            }
            let label = if let [index] = members.as_slice() {
                query.requirements[*index].item.map_or_else(
                    || format!("Requirement {number}"),
                    |id| format!("Requirement {number} ({})", crate::catalog::item(id).name),
                )
            } else {
                format!("Requirement {number}'s alternatives")
            };
            let earliest = slot
                .iter()
                .filter_map(|p| {
                    (p.max_depth + 1..=24).find(|&depth| {
                        ALL_SOURCES.iter().any(|&source| {
                            !(query.exclude_blacksmith_rewards
                                && source == ItemSource::BlacksmithReward)
                                && source_feasible(&p.requirement, source, profile)
                                && deadline(&p.requirement, source, depth).is_some_and(|end| {
                                    quest_for_source(source).map_or_else(
                                        || source != ItemSource::Shop || end >= SHOP_DEPTHS[0],
                                        |quest| end >= quest.window().0,
                                    )
                                })
                        })
                    })
                })
                .min();
            return Some(earliest.map_or_else(
                || format!("{label} cannot be generated with the requested filters."),
                |depth| format!("{label} needs floor {depth} or later, beyond its floor limit."),
            ));
        }
        // Hall's condition, as in viable_with_pending_vault: every quest
        // supplies at most one distinct reward, including the whole Imp vault.
        for subset in 1_u8..16 {
            let needed = self
                .slots
                .iter()
                .filter(|slot| {
                    !slot[0].requirement.blanket
                        && slot.iter().all(|p| p.open_deadline.is_none())
                        && slot.iter().fold(0, |mask, p| mask | p.quests) & !subset == 0
                })
                .count();
            let available = subset.count_ones();
            if needed > available as usize {
                let names = QUESTS
                    .iter()
                    .filter(|quest| quest.bit() & subset != 0)
                    .map(|quest| match quest {
                        Quest::Ghost => "Ghost",
                        Quest::Wandmaker => "Wandmaker",
                        Quest::Blacksmith => "Blacksmith",
                        Quest::Imp => "Imp",
                    })
                    .collect::<Vec<_>>()
                    .join(", ");
                let noun = if available == 1 { "reward" } else { "rewards" };
                return Some(format!(
                    "{needed} requirements need separate rewards from {names}, but only {available} {noun} can be taken."
                ));
            }
        }
        None
    }
}
