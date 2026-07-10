//! Multi-item query validation and accessibility-aware matching.

use std::collections::BTreeMap;
use std::fmt;

use crate::catalog::{Effect, ItemId, ItemKind, item};
use crate::model::{GeneratedWorld, ItemSource, WorldItem};

/// Upgrade predicate attached to one item requirement.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum UpgradeRequirement {
    Any,
    Exact(u8),
    AtLeast(u8),
}

impl UpgradeRequirement {
    const fn matches(self, upgrade: u8) -> bool {
        match self {
            Self::Any => true,
            Self::Exact(wanted) => upgrade == wanted,
            Self::AtLeast(minimum) => upgrade >= minimum,
        }
    }
}

/// One required item. `None` fields are wildcards.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Requirement {
    pub kind: ItemKind,
    pub item: Option<ItemId>,
    pub upgrade: UpgradeRequirement,
    pub effect: Option<Effect>,
    pub source: Option<ItemSource>,
    /// Requirements in the same non-zero group must resolve to the same item ID.
    pub identity_group: Option<u8>,
}

impl Requirement {
    #[must_use]
    pub fn matches(self, candidate: &WorldItem) -> bool {
        let definition = item(candidate.item);
        definition.kind == self.kind
            && self.item.is_none_or(|wanted| wanted == candidate.item)
            && self.upgrade.matches(candidate.upgrade)
            && self
                .effect
                .is_none_or(|wanted| candidate.effect == Some(wanted))
            && self.source.is_none_or(|wanted| wanted == candidate.source)
    }

    /// Checks that an item/effect/upgrade combination is meaningful.
    ///
    /// # Errors
    ///
    /// Returns a validation error for a category mismatch, an effect intended
    /// for another family, or an upgrade outside the UI's family-specific range.
    pub fn validate(self) -> Result<(), QueryError> {
        if self
            .item
            .is_some_and(|item_id| item(item_id).kind != self.kind)
        {
            return Err(QueryError::ItemKindMismatch);
        }
        let maximum = self.kind.maximum_search_upgrade();
        let valid_upgrade = match self.upgrade {
            UpgradeRequirement::Any => true,
            UpgradeRequirement::Exact(upgrade) => (1..=maximum).contains(&upgrade),
            UpgradeRequirement::AtLeast(upgrade) => upgrade <= maximum,
        };
        if !valid_upgrade {
            return Err(QueryError::InvalidUpgrade);
        }
        if self.identity_group == Some(0) {
            return Err(QueryError::InvalidIdentityGroup);
        }
        match (self.kind, self.effect) {
            (ItemKind::Weapon, None | Some(Effect::Weapon(_)))
            | (ItemKind::Armor, None | Some(Effect::Armor(_)))
            | (ItemKind::Wand | ItemKind::Ring, None) => Ok(()),
            _ => Err(QueryError::EffectKindMismatch),
        }
    }
}

/// All requirements must be obtainable together in the same generated world.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SearchQuery {
    pub requirements: Vec<Requirement>,
    pub max_depth: u8,
    /// Reforging plans need an accessible blacksmith room within `max_depth`.
    pub require_blacksmith: bool,
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
        let mut identity_groups = BTreeMap::new();
        for requirement in &self.requirements {
            requirement.validate()?;
            if let Some(group) = requirement.identity_group {
                let current = (requirement.kind, requirement.item);
                if let Some(previous) = identity_groups.insert(group, current) {
                    if previous.0 != current.0
                        || previous.1.zip(current.1).is_some_and(|(left, right)| left != right)
                    {
                        return Err(QueryError::InconsistentIdentityGroup);
                    }
                }
            }
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
        if self.require_blacksmith
            && !world.items.iter().any(|candidate| {
                candidate.depth <= self.max_depth && candidate.source == ItemSource::BlacksmithReward
            })
        {
            return false;
        }

        let mut candidates: Vec<(Option<u8>, Vec<usize>)> = self
            .requirements
            .iter()
            .map(|requirement| {
                (
                    requirement.identity_group,
                    world
                        .items
                        .iter()
                        .enumerate()
                        .filter_map(|(index, candidate)| {
                            (candidate.depth <= self.max_depth && requirement.matches(candidate))
                                .then_some(index)
                        })
                        .collect(),
                )
            })
            .collect();
        if candidates.iter().any(|(_, values)| values.is_empty()) {
            return false;
        }

        // Fail early by assigning the most constrained requirement first.
        candidates.sort_by_key(|(_, values)| values.len());
        let mut used = vec![false; world.items.len()];
        let mut scenarios = BTreeMap::new();
        let mut identities = BTreeMap::new();
        match_recursive(
            &candidates,
            0,
            &world.items,
            &mut used,
            &mut scenarios,
            &mut identities,
        )
    }
}

fn match_recursive(
    candidates: &[(Option<u8>, Vec<usize>)],
    requirement_index: usize,
    items: &[WorldItem],
    used: &mut [bool],
    scenarios: &mut BTreeMap<u16, u64>,
    identities: &mut BTreeMap<u8, ItemId>,
) -> bool {
    if requirement_index == candidates.len() {
        return true;
    }

    let (identity_group, requirement_candidates) = &candidates[requirement_index];
    for &item_index in requirement_candidates {
        if used[item_index] {
            continue;
        }
        let mut previous_identity = None;
        if let Some(group) = identity_group {
            if identities
                .get(group)
                .is_some_and(|wanted| *wanted != items[item_index].item)
            {
                continue;
            }
            previous_identity = Some((*group, identities.insert(*group, items[item_index].item)));
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
        if match_recursive(
            candidates,
            requirement_index + 1,
            items,
            used,
            scenarios,
            identities,
        ) {
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
        if let Some((group, previous)) = previous_identity {
            if let Some(previous) = previous {
                identities.insert(group, previous);
            } else {
                identities.remove(&group);
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
    InvalidIdentityGroup,
    InconsistentIdentityGroup,
}

impl fmt::Display for QueryError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let message = match self {
            Self::Empty => "at least one item requirement is needed",
            Self::InvalidDepth => "maximum depth must be between 1 and 24",
            Self::InvalidUpgrade => "upgrade must be +1, +2, or +3 (+4 for rings)",
            Self::ItemKindMismatch => "selected item is in a different category",
            Self::EffectKindMismatch => "selected enchantment or glyph is inapplicable",
            Self::InvalidIdentityGroup => "identity group zero is reserved for no group",
            Self::InconsistentIdentityGroup => {
                "linked item requirements must use the same category and item"
            }
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

    use super::{QueryError, Requirement, SearchQuery};

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

    #[test]
    fn plus_four_is_valid_only_for_rings() {
        let ring = Requirement {
            kind: ItemKind::Ring,
            item: Some(ItemId::RingSharpshooting),
            upgrade: Some(4),
            effect: None,
        };
        assert_eq!(ring.validate(), Ok(()));

        let wand = Requirement {
            kind: ItemKind::Wand,
            item: Some(ItemId::WandFrost),
            upgrade: Some(4),
            effect: None,
        };
        assert_eq!(wand.validate(), Err(QueryError::InvalidUpgrade));
    }
}
