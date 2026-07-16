// SPDX-License-Identifier: GPL-3.0-or-later

//! Shared query state and presentation labels for the whole window.

use std::fmt::Write as _;

use shpd_seedfinder_core::catalog::{Effect, ItemId, ItemKind, item};
use shpd_seedfinder_core::challenges::Challenges;
use shpd_seedfinder_core::model::ItemSource;
use shpd_seedfinder_core::query::{Requirement, SearchQuery, TierRequirement, UpgradeRequirement};

pub const MAX_REQUIREMENT_COUNT: usize = 64;

/// Every user-facing item source, in the wire order shared with the other
/// frontends.
pub const ALL_SOURCES: &[ItemSource] = &[
    ItemSource::Heap,
    ItemSource::Chest,
    ItemSource::LockedChest,
    ItemSource::CrystalChest,
    ItemSource::Tomb,
    ItemSource::Skeleton,
    ItemSource::SacrificialFire,
    ItemSource::Mimic,
    ItemSource::GoldenMimic,
    ItemSource::CrystalMimic,
    ItemSource::Statue,
    ItemSource::ArmoredStatue,
    ItemSource::Shop,
    ItemSource::GhostReward,
    ItemSource::WandmakerReward,
    ItemSource::BlacksmithReward,
    ItemSource::ImpReward,
];

/// Every user-facing item family, in presentation order.
pub const ALL_KINDS: &[ItemKind] = &[
    ItemKind::Weapon,
    ItemKind::Armor,
    ItemKind::Wand,
    ItemKind::Ring,
];

/// One item requirement as edited in the interface. All predicate fields
/// mirror [`Requirement`]; `key` is a session-stable row identity.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct UiRequirement {
    pub key: u64,
    pub quantity: u8,
    pub kind: ItemKind,
    pub item: Option<ItemId>,
    pub tier: TierRequirement,
    pub upgrade: UpgradeRequirement,
    pub effect: Option<Effect>,
    pub require_uncursed: bool,
    pub source: Option<ItemSource>,
    pub identity_group: Option<u8>,
    pub max_depth: Option<u8>,
}

impl UiRequirement {
    pub const fn new(key: u64) -> Self {
        Self {
            key,
            quantity: 1,
            kind: ItemKind::Weapon,
            item: None,
            tier: TierRequirement::Any,
            upgrade: UpgradeRequirement::Any,
            effect: None,
            require_uncursed: false,
            source: None,
            identity_group: None,
            max_depth: None,
        }
    }

    #[must_use]
    pub const fn to_core(self) -> Requirement {
        Requirement {
            kind: self.kind,
            item: self.item,
            tier: self.tier,
            upgrade: self.upgrade,
            effect: self.effect,
            require_uncursed: self.require_uncursed,
            source: self.source,
            identity_group: self.identity_group,
            max_depth: self.max_depth,
        }
    }

    /// Whether this row differs from `other` only by identity and quantity.
    #[must_use]
    pub fn has_same_criteria(&self, other: &Self) -> bool {
        self.kind == other.kind
            && self.item == other.item
            && self.tier == other.tier
            && self.upgrade == other.upgrade
            && self.effect == other.effect
            && self.require_uncursed == other.require_uncursed
            && self.source == other.source
            && self.identity_group == other.identity_group
            && self.max_depth == other.max_depth
    }

    /// Primary row label, e.g. `Any Tier 3+ weapon` or `Ring of tenacity`.
    #[must_use]
    pub fn title(&self) -> String {
        if let Some(item_id) = self.item {
            return item(item_id).name.to_owned();
        }
        match self.tier {
            TierRequirement::Any => format!("Any {}", kind_singular(self.kind)),
            TierRequirement::Exact(tier) => {
                format!("Any Tier {tier} {}", kind_singular(self.kind))
            }
            TierRequirement::AtLeast(tier) => {
                format!("Any Tier {tier}+ {}", kind_singular(self.kind))
            }
            TierRequirement::AtMost(tier) => {
                format!("Any Tier {tier} or lower {}", kind_singular(self.kind))
            }
        }
    }

    /// Primary row label including a quantity prefix when more than one item
    /// must satisfy these criteria.
    #[must_use]
    pub fn display_title(&self) -> String {
        let title = self.title();
        if self.quantity == 1 {
            title
        } else {
            format!("{}× {title}", self.quantity)
        }
    }

    /// Secondary row label listing the remaining predicates.
    #[must_use]
    pub fn subtitle(&self) -> String {
        let mut text = match self.upgrade {
            UpgradeRequirement::Any => "Any upgrade".to_owned(),
            UpgradeRequirement::Exact(upgrade) => format!("+{upgrade} exactly"),
            UpgradeRequirement::AtLeast(upgrade) => format!("+{upgrade} or higher"),
        };
        if let Some(effect) = self.effect {
            let _ = write!(text, " · {}", effect.wire_name());
        }
        if self.require_uncursed {
            text.push_str(" · uncursed");
        }
        if let Some(source) = self.source {
            let _ = write!(text, " · {}", source_label(source));
        }
        if let Some(group) = self.identity_group {
            let _ = write!(text, " · same item group {}", group_letter(group));
        }
        if let Some(depth) = self.max_depth {
            let _ = write!(text, " · by floor {depth}");
        }
        text
    }
}

/// The whole persisted query state shared by all panes.
#[derive(Clone, Debug)]
pub struct AppState {
    pub requirements: Vec<UiRequirement>,
    pub max_depth: u8,
    pub require_blacksmith: bool,
    pub exclude_blacksmith_rewards: bool,
    pub fast_mode: bool,
    pub challenges: Challenges,
    next_key: u64,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            requirements: Vec::new(),
            max_depth: 24,
            require_blacksmith: false,
            exclude_blacksmith_rewards: false,
            fast_mode: false,
            challenges: Challenges::NONE,
            next_key: 1,
        }
    }
}

impl AppState {
    /// Hands out a fresh row key, unique within this session.
    pub const fn claim_key(&mut self) -> u64 {
        let key = self.next_key;
        self.next_key += 1;
        key
    }

    /// Adds or replaces one row, then combines identical criteria and orders
    /// rows by floor limit. The first row's key survives coalescing.
    ///
    /// # Errors
    ///
    /// Returns a human-readable validation message without changing the state.
    pub fn upsert_requirement(&mut self, requirement: UiRequirement) -> Result<(), String> {
        let mut candidate = self.requirements.clone();
        if let Some(slot) = candidate
            .iter_mut()
            .find(|other| other.key == requirement.key)
        {
            *slot = requirement;
        } else {
            candidate.push(requirement);
        }
        self.requirements = normalize_requirements(&candidate)?;
        Ok(())
    }

    /// Coalesces and floor-sorts the current requirements in place.
    ///
    /// # Errors
    ///
    /// Returns a human-readable validation message without changing the state.
    pub fn normalize_requirements(&mut self) -> Result<(), String> {
        let normalized = normalize_requirements(&self.requirements)?;
        self.requirements = normalized;
        Ok(())
    }

    /// Builds the validated engine query for the current state.
    ///
    /// # Errors
    ///
    /// Returns the human-readable validation message.
    pub fn to_query(&self) -> Result<SearchQuery, String> {
        let query = SearchQuery {
            requirements: expand_requirements(&self.requirements)?
                .into_iter()
                .map(UiRequirement::to_core)
                .collect(),
            max_depth: self.max_depth,
            challenges: self.challenges,
            require_blacksmith: self.require_blacksmith && self.max_depth < 14,
            exclude_blacksmith_rewards: self.exclude_blacksmith_rewards,
            fast_mode: self.fast_mode,
        };
        query.validate().map_err(|error| error.to_string())?;
        Ok(query)
    }
}

/// Expands compact quantity rows into one copy per required item. Search and
/// scouting call this immediately before native/core matching so each copy
/// must resolve to a distinct world item.
///
/// # Errors
///
/// Returns a human-readable validation message when a row quantity or the
/// expanded total falls outside `1..=64`.
pub fn expand_requirements(requirements: &[UiRequirement]) -> Result<Vec<UiRequirement>, String> {
    let mut expanded = Vec::new();
    for requirement in requirements {
        if !(1..=u8::try_from(MAX_REQUIREMENT_COUNT).unwrap_or(u8::MAX))
            .contains(&requirement.quantity)
        {
            return Err("Requirement quantity must be between 1 and 64".to_owned());
        }
        if expanded.len() + usize::from(requirement.quantity) > MAX_REQUIREMENT_COUNT {
            return Err("Total requirement quantity must be at most 64".to_owned());
        }
        expanded.extend(std::iter::repeat_n(
            UiRequirement {
                quantity: 1,
                ..*requirement
            },
            usize::from(requirement.quantity),
        ));
    }
    Ok(expanded)
}

/// Combines rows with equal criteria and then keeps unrestricted rows first,
/// followed by floor-limited rows from earliest to latest. Both operations are
/// stable, preserving the first row's key and the order of ties.
fn normalize_requirements(requirements: &[UiRequirement]) -> Result<Vec<UiRequirement>, String> {
    // Validate the compact input and the total before adding quantities, which
    // also guarantees a coalesced row cannot exceed 64.
    let _ = expand_requirements(requirements)?;
    let mut normalized: Vec<UiRequirement> = Vec::new();
    for requirement in requirements {
        if let Some(existing) = normalized
            .iter_mut()
            .find(|existing| existing.has_same_criteria(requirement))
        {
            existing.quantity += requirement.quantity;
        } else {
            normalized.push(*requirement);
        }
    }
    normalized.sort_by_key(|requirement| requirement.max_depth.unwrap_or(0));
    Ok(normalized)
}

pub const fn kind_label(kind: ItemKind) -> &'static str {
    match kind {
        ItemKind::Weapon => "Weapon",
        ItemKind::Armor => "Armor",
        ItemKind::Wand => "Wand",
        ItemKind::Ring => "Ring",
    }
}

pub const fn kind_singular(kind: ItemKind) -> &'static str {
    match kind {
        ItemKind::Weapon => "weapon",
        ItemKind::Armor => "armor",
        ItemKind::Wand => "wand",
        ItemKind::Ring => "ring",
    }
}

/// Bundled symbolic icon name for one item family.
pub const fn kind_icon(kind: ItemKind) -> &'static str {
    match kind {
        ItemKind::Weapon => "kind-weapon-symbolic",
        ItemKind::Armor => "kind-armor-symbolic",
        ItemKind::Wand => "kind-wand-symbolic",
        ItemKind::Ring => "kind-ring-symbolic",
    }
}

pub const fn source_label(source: ItemSource) -> &'static str {
    match source {
        ItemSource::Heap => "Floor",
        ItemSource::Chest => "Chest",
        ItemSource::LockedChest => "Locked chest",
        ItemSource::CrystalChest => "Crystal chest",
        ItemSource::Tomb => "Tomb",
        ItemSource::Skeleton => "Skeletal remains",
        ItemSource::SacrificialFire => "Sacrificial fire",
        ItemSource::Mimic => "Mimic",
        ItemSource::GoldenMimic => "Golden mimic",
        ItemSource::CrystalMimic => "Crystal mimic",
        ItemSource::Statue => "Animated statue",
        ItemSource::ArmoredStatue => "Armored statue",
        ItemSource::Shop => "Shop",
        ItemSource::GhostReward => "Sad ghost reward",
        ItemSource::WandmakerReward => "Wandmaker reward",
        ItemSource::BlacksmithReward => "Blacksmith reward",
        ItemSource::ImpReward => "Imp reward",
    }
}

pub const fn group_letter(group: u8) -> char {
    match group {
        1 => 'A',
        2 => 'B',
        3 => 'C',
        _ => 'D',
    }
}

/// Dungeon region name for one depth.
pub const fn region(depth: u8) -> &'static str {
    match depth {
        0..=5 => "Sewers",
        6..=10 => "Prison",
        11..=15 => "Caves",
        16..=20 => "Dwarven City",
        _ => "Demon Halls",
    }
}

/// One upstream challenge with presentation data.
pub struct ChallengeInfo {
    pub challenge: Challenges,
    pub label: &'static str,
    pub changes_generation: bool,
}

/// The nine upstream challenges, in mask order.
pub const ALL_CHALLENGES: &[ChallengeInfo] = &[
    ChallengeInfo {
        challenge: Challenges::NO_FOOD,
        label: "On diet",
        changes_generation: false,
    },
    ChallengeInfo {
        challenge: Challenges::NO_ARMOR,
        label: "Faith is my armor",
        changes_generation: false,
    },
    ChallengeInfo {
        challenge: Challenges::NO_HEALING,
        label: "Pharmacophobia",
        changes_generation: false,
    },
    ChallengeInfo {
        challenge: Challenges::NO_HERBALISM,
        label: "Barren land",
        changes_generation: true,
    },
    ChallengeInfo {
        challenge: Challenges::SWARM_INTELLIGENCE,
        label: "Swarm intelligence",
        changes_generation: false,
    },
    ChallengeInfo {
        challenge: Challenges::DARKNESS,
        label: "Into darkness",
        changes_generation: true,
    },
    ChallengeInfo {
        challenge: Challenges::NO_SCROLLS,
        label: "Forbidden runes",
        changes_generation: true,
    },
    ChallengeInfo {
        challenge: Challenges::CHAMPION_ENEMIES,
        label: "Hostile champions",
        changes_generation: false,
    },
    ChallengeInfo {
        challenge: Challenges::STRONGER_BOSSES,
        label: "Badder bosses",
        changes_generation: false,
    },
];

#[cfg(test)]
mod tests {
    use shpd_seedfinder_core::catalog::{ItemId, ItemKind};
    use shpd_seedfinder_core::query::{TierRequirement, UpgradeRequirement};

    use super::{AppState, UiRequirement};

    #[test]
    fn labels_describe_wildcards_and_predicates() {
        let mut requirement = UiRequirement::new(1);
        assert_eq!(requirement.title(), "Any weapon");
        assert_eq!(requirement.display_title(), "Any weapon");
        assert_eq!(requirement.subtitle(), "Any upgrade");

        requirement.quantity = 3;
        assert_eq!(requirement.display_title(), "3× Any weapon");

        requirement.tier = TierRequirement::AtLeast(4);
        requirement.upgrade = UpgradeRequirement::Exact(2);
        requirement.identity_group = Some(2);
        requirement.max_depth = Some(9);
        requirement.require_uncursed = true;
        assert_eq!(requirement.title(), "Any Tier 4+ weapon");
        assert_eq!(
            requirement.subtitle(),
            "+2 exactly · uncursed · same item group B · by floor 9"
        );

        requirement.tier = TierRequirement::AtMost(3);
        assert_eq!(requirement.title(), "Any Tier 3 or lower weapon");

        requirement.item = Some(ItemId::Greatsword);
        assert_eq!(requirement.title(), "Greatsword");
    }

    #[test]
    fn query_drops_blacksmith_requirement_at_depth_fourteen() {
        let mut state = AppState::default();
        let key = state.claim_key();
        state.requirements.push(UiRequirement::new(key));
        state.require_blacksmith = true;
        state.max_depth = 14;
        assert!(!state.to_query().unwrap().require_blacksmith);
        state.max_depth = 13;
        assert!(state.to_query().unwrap().require_blacksmith);
    }

    #[test]
    fn upsert_coalesces_criteria_and_sorts_by_floor_limit() {
        let requirement = |key, upgrade, max_depth| UiRequirement {
            kind: ItemKind::Wand,
            upgrade: UpgradeRequirement::Exact(upgrade),
            max_depth,
            ..UiRequirement::new(key)
        };
        let mut state = AppState::default();
        state.upsert_requirement(requirement(1, 3, None)).unwrap();
        state
            .upsert_requirement(requirement(2, 2, Some(4)))
            .unwrap();
        state
            .upsert_requirement(requirement(3, 2, Some(9)))
            .unwrap();
        let mut duplicate = requirement(4, 2, Some(4));
        duplicate.quantity = 2;
        state.upsert_requirement(duplicate).unwrap();

        assert_eq!(
            state
                .requirements
                .iter()
                .map(|requirement| requirement.key)
                .collect::<Vec<_>>(),
            [1, 2, 3]
        );
        assert_eq!(
            state
                .requirements
                .iter()
                .map(|requirement| requirement.quantity)
                .collect::<Vec<_>>(),
            [1, 3, 1]
        );
        assert_eq!(state.requirements[1].display_title(), "3× Any wand");
    }

    #[test]
    fn query_expands_quantities_and_caps_total_at_sixty_four() {
        let mut state = AppState::default();
        let mut requirement = UiRequirement {
            kind: ItemKind::Wand,
            upgrade: UpgradeRequirement::Exact(2),
            ..UiRequirement::new(1)
        };
        requirement.quantity = 3;
        state.requirements.push(requirement);

        let query = state.to_query().unwrap();
        assert_eq!(query.requirements.len(), 3);
        assert!(query.requirements.windows(2).all(|pair| pair[0] == pair[1]));

        requirement.quantity = 0;
        state.requirements = vec![requirement];
        assert_eq!(
            state.to_query().unwrap_err(),
            "Requirement quantity must be between 1 and 64"
        );

        requirement.quantity = 64;
        state.requirements = vec![requirement];
        let mut other = requirement;
        other.key = 2;
        other.upgrade = UpgradeRequirement::Exact(3);
        other.quantity = 1;
        state.requirements.push(other);
        assert_eq!(
            state.to_query().unwrap_err(),
            "Total requirement quantity must be at most 64"
        );
    }
}
