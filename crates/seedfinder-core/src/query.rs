//! Multi-item query validation and accessibility-aware matching.

use std::collections::BTreeMap;
use std::fmt;

use crate::catalog::{Effect, ItemId, ItemKind, item};
use crate::model::{GeneratedWorld, WorldItem};

/// One required item. `None` fields are wildcards.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Requirement {
    pub kind: ItemKind,
    pub item: Option<ItemId>,
    pub upgrade: Option<u8>,
    pub effect: Option<Effect>,
}

impl Requirement {
    #[must_use]
    pub fn matches(self, candidate: &WorldItem) -> bool {
        let definition = item(candidate.item);
        definition.kind == self.kind
            && self.item.is_none_or(|wanted| wanted == candidate.item)
            && self
                .upgrade
                .is_none_or(|wanted| wanted == candidate.upgrade)
            && self
                .effect
                .is_none_or(|wanted| candidate.effect == Some(wanted))
    }

    /// Checks that an item/effect/upgrade combination is meaningful.
    ///
    /// # Errors
    ///
    /// Returns a validation error for a category mismatch, an effect intended
    /// for another family, or an upgrade outside the UI's `+1..=+3` range.
    pub fn validate(self) -> Result<(), QueryError> {
        if self
            .item
            .is_some_and(|item_id| item(item_id).kind != self.kind)
        {
            return Err(QueryError::ItemKindMismatch);
        }
        if self
            .upgrade
            .is_some_and(|upgrade| !(1..=3).contains(&upgrade))
        {
            return Err(QueryError::InvalidUpgrade);
        }
        match (self.kind, self.effect) {
            (ItemKind::Weapon, None | Some(Effect::Weapon(_)))
            | (ItemKind::Armor, None | Some(Effect::Armor(_)))
            | (ItemKind::Wand, None) => Ok(()),
            _ => Err(QueryError::EffectKindMismatch),
        }
    }
}

/// All requirements must be obtainable together in the same generated world.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SearchQuery {
    pub requirements: Vec<Requirement>,
    pub max_depth: u8,
}

impl SearchQuery {
    /// Validates bounds and every requirement.
    ///
    /// # Errors
    ///
    /// Returns a [`QueryError`] when no requirements are present, the selected
    /// depth is outside the main dungeon, or a requirement is inconsistent.
    pub fn validate(&self) -> Result<(), QueryError> {
        if self.requirements.is_empty() {
            return Err(QueryError::Empty);
        }
        if !(1..=24).contains(&self.max_depth) {
            return Err(QueryError::InvalidDepth);
        }
        for requirement in &self.requirements {
            requirement.validate()?;
        }
        Ok(())
    }

    /// Matches requirements as an AND query while respecting distinct item
    /// instances and mutually exclusive quest/chest reward branches.
    #[must_use]
    pub fn matches(&self, world: &GeneratedWorld) -> bool {
        if self.requirements.len() > world.items.len() {
            return false;
        }

        let mut candidates: Vec<Vec<usize>> = self
            .requirements
            .iter()
            .map(|requirement| {
                world
                    .items
                    .iter()
                    .enumerate()
                    .filter_map(|(index, candidate)| {
                        (candidate.depth <= self.max_depth && requirement.matches(candidate))
                            .then_some(index)
                    })
                    .collect()
            })
            .collect();
        if candidates.iter().any(Vec::is_empty) {
            return false;
        }

        // Fail early by assigning the most constrained requirement first.
        candidates.sort_by_key(Vec::len);
        let mut used = vec![false; world.items.len()];
        let mut scenarios = BTreeMap::new();
        match_recursive(&candidates, 0, &world.items, &mut used, &mut scenarios)
    }
}

fn match_recursive(
    candidates: &[Vec<usize>],
    requirement_index: usize,
    items: &[WorldItem],
    used: &mut [bool],
    scenarios: &mut BTreeMap<u16, u64>,
) -> bool {
    if requirement_index == candidates.len() {
        return true;
    }

    for &item_index in &candidates[requirement_index] {
        if used[item_index] {
            continue;
        }
        let mut previous_scenarios = None;
        if let Some((group, item_scenarios)) = items[item_index].accessibility.scenario_constraint()
        {
            let compatible = scenarios.get(&group).copied().unwrap_or(u64::MAX) & item_scenarios;
            if compatible == 0 {
                continue;
            }
            previous_scenarios = Some((group, scenarios.insert(group, compatible)));
        }

        used[item_index] = true;
        if match_recursive(candidates, requirement_index + 1, items, used, scenarios) {
            return true;
        }
        used[item_index] = false;
        if let Some((group, previous)) = previous_scenarios {
            if let Some(previous) = previous {
                scenarios.insert(group, previous);
            } else {
                scenarios.remove(&group);
            }
        }
    }
    false
}

/// Invalid user query.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum QueryError {
    Empty,
    InvalidDepth,
    InvalidUpgrade,
    ItemKindMismatch,
    EffectKindMismatch,
}

impl fmt::Display for QueryError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let message = match self {
            Self::Empty => "at least one item requirement is needed",
            Self::InvalidDepth => "maximum depth must be between 1 and 24",
            Self::InvalidUpgrade => "upgrade must be +1, +2, or +3",
            Self::ItemKindMismatch => "selected item is in a different category",
            Self::EffectKindMismatch => "selected enchantment or glyph is inapplicable",
        };
        formatter.write_str(message)
    }
}

impl std::error::Error for QueryError {}

#[cfg(test)]
mod tests {
    use crate::catalog::{ItemId, ItemKind};
    use crate::model::{Accessibility, GeneratedWorld, ItemSource, WorldItem};
    use crate::seed::DungeonSeed;

    use super::{Requirement, SearchQuery};

    fn world_item(item: ItemId, accessibility: Accessibility) -> WorldItem {
        WorldItem {
            item,
            upgrade: 2,
            effect: None,
            cursed: false,
            depth: 3,
            source: ItemSource::GhostReward,
            accessibility,
        }
    }

    fn requirement(item: ItemId) -> Requirement {
        Requirement {
            kind: crate::catalog::item(item).kind,
            item: Some(item),
            upgrade: Some(2),
            effect: None,
        }
    }

    #[test]
    fn and_query_requires_distinct_item_occurrences() {
        let query = SearchQuery {
            requirements: vec![requirement(ItemId::Sword), requirement(ItemId::Sword)],
            max_depth: 4,
        };
        let one = GeneratedWorld {
            seed: DungeonSeed::MIN,
            items: vec![world_item(ItemId::Sword, Accessibility::Independent)],
        };
        assert!(!query.matches(&one));
        let two = GeneratedWorld {
            seed: DungeonSeed::MIN,
            items: vec![
                world_item(ItemId::Sword, Accessibility::Independent),
                world_item(ItemId::Sword, Accessibility::Independent),
            ],
        };
        assert!(query.matches(&two));
    }

    #[test]
    fn mutually_exclusive_rewards_cannot_satisfy_and_query() {
        let query = SearchQuery {
            requirements: vec![requirement(ItemId::Sword), requirement(ItemId::MailArmor)],
            max_depth: 4,
        };
        let world = GeneratedWorld {
            seed: DungeonSeed::MIN,
            items: vec![
                world_item(
                    ItemId::Sword,
                    Accessibility::Choice {
                        group: 1,
                        option: 0,
                    },
                ),
                world_item(
                    ItemId::MailArmor,
                    Accessibility::Choice {
                        group: 1,
                        option: 1,
                    },
                ),
            ],
        };
        assert!(!query.matches(&world));
    }

    #[test]
    fn same_choice_option_and_independent_rewards_can_match() {
        let query = SearchQuery {
            requirements: vec![requirement(ItemId::Sword), requirement(ItemId::MailArmor)],
            max_depth: 4,
        };
        let world = GeneratedWorld {
            seed: DungeonSeed::MIN,
            items: vec![
                world_item(
                    ItemId::Sword,
                    Accessibility::Choice {
                        group: 2,
                        option: 0,
                    },
                ),
                world_item(
                    ItemId::MailArmor,
                    Accessibility::Choice {
                        group: 2,
                        option: 0,
                    },
                ),
            ],
        };
        assert!(query.matches(&world));
    }

    #[test]
    fn scenario_masks_model_prerequisite_paths_without_false_choices() {
        let sword = world_item(
            ItemId::Sword,
            Accessibility::Scenarios {
                group: 7,
                mask: 0b0011,
            },
        );
        let armor = world_item(
            ItemId::MailArmor,
            Accessibility::Scenarios {
                group: 7,
                mask: 0b0110,
            },
        );
        let wand = world_item(
            ItemId::WandFrost,
            Accessibility::Scenarios {
                group: 7,
                mask: 0b1100,
            },
        );
        let world = GeneratedWorld {
            seed: DungeonSeed::MIN,
            items: vec![sword, armor, wand],
        };

        let compatible = SearchQuery {
            requirements: vec![requirement(ItemId::Sword), requirement(ItemId::MailArmor)],
            max_depth: 4,
        };
        assert!(compatible.matches(&world));

        let incompatible = SearchQuery {
            requirements: vec![requirement(ItemId::Sword), requirement(ItemId::WandFrost)],
            max_depth: 4,
        };
        assert!(!incompatible.matches(&world));
    }

    #[test]
    fn validation_rejects_wrong_category() {
        let invalid = Requirement {
            kind: ItemKind::Wand,
            item: Some(ItemId::Sword),
            upgrade: Some(2),
            effect: None,
        };
        assert!(invalid.validate().is_err());
    }
}
