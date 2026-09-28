//! Multi-item query validation and accessibility-aware matching.

mod resin;
mod stacks;
pub use resin::ArcaneResinFilter;
pub(crate) use resin::donor_requirement as resin_donor_requirement;
pub(crate) use resin::reforge_copies;
pub(crate) use resin::upgrade_cost as resin_upgrade_cost;
pub(crate) use stacks::{gate_of, gating_group, member_stack_variants, stack_gates};

use std::collections::BTreeMap;
use std::fmt;

use crate::catalog::{
    ALL_ARMOR_EFFECTS, ALL_WEAPON_EFFECTS, EXTRA_UPGRADE_TIER, Effect, ItemId, ItemKind,
    MAX_GENERATED_UPGRADE, MAX_STANDARD_RING_UPGRADE, WeaponCategory, item,
};
use crate::challenges::Challenges;
use crate::model::{GeneratedWorld, ItemSource, WorldItem};
use crate::quests::WandmakerQuestType;

/// Deepest floor a query may be limited to: the main dungeon ends at 24.
pub const MAX_SEARCH_DEPTH: u8 = 24;

/// Inclusive tier range an exact tier filter accepts. Tier 1 equipment is
/// starting gear the generator never places.
pub const EXACT_TIER_MIN: u8 = 2;
/// Upper end of [`EXACT_TIER_MIN`]'s range.
pub const EXACT_TIER_MAX: u8 = 5;

/// Inclusive tier range an at-least/at-most filter accepts; the bounds
/// outside it are redundant with no filter at all.
pub const BOUNDED_TIER_MIN: u8 = 3;
/// Upper end of [`BOUNDED_TIER_MIN`]'s range.
pub const BOUNDED_TIER_MAX: u8 = 4;

/// Identity group label reserved for "no group", which is why groups start
/// at 1. [`Requirement::validate`] rejects exactly this label.
pub const RESERVED_IDENTITY_GROUP: u8 = 0;

/// Highest identity group label the portable formats and every app's editor
/// can express (groups A..D). The matcher itself accepts any non-reserved
/// label, but a query that travels — as a share link or a results file —
/// must stay inside this range.
pub const MAX_IDENTITY_GROUP: u8 = 4;

/// Group label reserved for "no group" in alternative and combined-level
/// groups, mirroring [`RESERVED_IDENTITY_GROUP`].
pub const RESERVED_GROUP: u8 = 0;

/// Highest combined-level group label the portable formats and every
/// app's editor can express (groups A..D), the counterpart of
/// [`MAX_IDENTITY_GROUP`]. Alternative groups carry no such cap: the
/// portable formats write them structurally, as one `any_of` entry, and
/// renumber them on read.
pub const MAX_LEVEL_SUM_GROUP: u8 = 4;

/// Smallest fixed Arcane Resin minimum a query asks for; zero means no
/// fixed resin condition ([`SearchQuery::arcane_resin`]).
pub const ARCANE_RESIN_MIN: u16 = 1;
/// Largest fixed Arcane Resin minimum: the query format's 16-bit field.
pub const ARCANE_RESIN_MAX: u16 = u16::MAX;

/// Upgrade predicate attached to one item requirement.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum UpgradeRequirement {
    Any,
    Exact(u8),
    AtLeast(u8),
}

/// Optional tier predicate for tiered equipment.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum TierRequirement {
    Any,
    Exact(u8),
    AtLeast(u8),
    AtMost(u8),
}

impl TierRequirement {
    /// Whether a tiered item satisfies this predicate. Untiered items never do.
    #[must_use]
    pub fn matches(self, tier: Option<u8>) -> bool {
        match self {
            Self::Any => true,
            Self::Exact(wanted) => tier == Some(wanted),
            Self::AtLeast(minimum) => tier.is_some_and(|tier| tier >= minimum),
            Self::AtMost(maximum) => tier.is_some_and(|tier| tier <= maximum),
        }
    }
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

/// Non-empty set of same-family effects, stored as a bitmask over the
/// family's upstream catalog ordering ([`ALL_WEAPON_EFFECTS`] and
/// [`ALL_ARMOR_EFFECTS`]). Only weapons and armor carry effects, so a set
/// always belongs to one of those two families.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct EffectSet {
    family: ItemKind,
    mask: u32,
}

impl EffectSet {
    /// Builds a set holding one effect.
    #[must_use]
    pub const fn single(effect: Effect) -> Self {
        Self {
            family: effect_family(effect),
            mask: 1 << effect_index(effect),
        }
    }

    /// Builds a set from effects that must all belong to one family, or
    /// `None` for an empty iterator or a mix of families.
    pub fn from_effects<I: IntoIterator<Item = Effect>>(effects: I) -> Option<Self> {
        let mut combined: Option<Self> = None;
        for effect in effects {
            let single = Self::single(effect);
            combined = Some(match combined {
                None => single,
                Some(existing) if existing.family == single.family => Self {
                    family: existing.family,
                    mask: existing.mask | single.mask,
                },
                Some(_) => return None,
            });
        }
        combined
    }

    /// Every non-curse enchantment or glyph of the family — the "any
    /// enchantment" predicate — or `None` for families that never carry
    /// effects.
    #[must_use]
    pub fn enchantments(kind: ItemKind) -> Option<Self> {
        Self::from_effects(family_effects(kind)?.filter(|effect| !effect.is_curse()))
    }

    /// The item family whose effects this set draws from.
    #[must_use]
    pub const fn family(self) -> ItemKind {
        self.family
    }

    #[must_use]
    pub const fn contains(self, effect: Effect) -> bool {
        effect_family(effect) as u8 == self.family as u8
            && self.mask & (1 << effect_index(effect)) != 0
    }

    /// The member effects in upstream catalog order.
    pub fn effects(self) -> impl Iterator<Item = Effect> {
        family_effects(self.family)
            .into_iter()
            .flatten()
            .filter(move |effect| self.contains(*effect))
    }

    /// Whether every member of this set is also in `other`.
    #[must_use]
    pub const fn is_subset_of(self, other: Self) -> bool {
        self.family as u8 == other.family as u8 && self.mask & !other.mask == 0
    }

    /// The members shared with `other`, or `None` when nothing overlaps.
    #[must_use]
    pub const fn intersection(self, other: Self) -> Option<Self> {
        if self.family as u8 != other.family as u8 {
            return None;
        }
        let mask = self.mask & other.mask;
        if mask == 0 {
            None
        } else {
            Some(Self {
                family: self.family,
                mask,
            })
        }
    }

    /// The set without its curse-type effects, or `None` when only curses
    /// were in it.
    #[must_use]
    pub fn without_curses(self) -> Option<Self> {
        Self::from_effects(self.effects().filter(|effect| !effect.is_curse()))
    }

    /// Whether every member is a curse-type effect.
    #[must_use]
    pub fn is_curses_only(self) -> bool {
        self.without_curses().is_none()
    }

    /// How many effects the set holds; never zero, at most 32.
    #[must_use]
    #[allow(clippy::cast_possible_truncation)] // a u32 mask has at most 32 bits set
    pub const fn count(self) -> u8 {
        self.mask.count_ones() as u8
    }
}

const fn effect_family(effect: Effect) -> ItemKind {
    match effect {
        Effect::Weapon(_) => ItemKind::Weapon,
        Effect::Armor(_) => ItemKind::Armor,
    }
}

const fn effect_index(effect: Effect) -> u8 {
    match effect {
        Effect::Weapon(effect) => effect as u8,
        Effect::Armor(effect) => effect as u8,
    }
}

fn family_effects(kind: ItemKind) -> Option<Box<dyn Iterator<Item = Effect>>> {
    match kind {
        ItemKind::Weapon => Some(Box::new(
            ALL_WEAPON_EFFECTS.iter().copied().map(Effect::Weapon),
        )),
        ItemKind::Armor => Some(Box::new(
            ALL_ARMOR_EFFECTS.iter().copied().map(Effect::Armor),
        )),
        ItemKind::Wand | ItemKind::Ring | ItemKind::Trinket | ItemKind::Artifact => None,
    }
}

/// Effect predicate attached to one item requirement.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum EffectRequirement {
    /// Wildcard: any effect, or none at all.
    Any,
    /// The item must carry one of these effects. "Any enchantment" is the
    /// full non-curse family set from [`EffectSet::enchantments`].
    OneOf(EffectSet),
}

impl EffectRequirement {
    /// The predicate accepting exactly one effect.
    #[must_use]
    pub const fn exactly(effect: Effect) -> Self {
        Self::OneOf(EffectSet::single(effect))
    }

    #[must_use]
    pub const fn matches(self, effect: Option<Effect>) -> bool {
        match self {
            Self::Any => true,
            Self::OneOf(set) => match effect {
                Some(effect) => set.contains(effect),
                None => false,
            },
        }
    }
}

/// Minimum combined item level shared by every requirement in one group.
///
/// An item's level counts as its upgrade plus one — a +0 Ring of Might still
/// grants one strength — and the group is satisfied when some subset of its
/// members, filled by distinct items, reaches `minimum_total` combined
/// levels. Members are *optional*: one +2 ring alone satisfies a two-member
/// group asking for three levels. Combine with [`Requirement::identity_group`]
/// to demand the contributing items be copies of one kind.
///
/// Only ring requirements may carry one: a ring's effect scales with its
/// level, so levels on separate rings add up the way no other family's do.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct LevelSum {
    /// Non-zero group label shared by the participating requirements.
    pub group: u8,
    /// Inclusive lower bound on the assigned members' combined levels,
    /// counting each item as `upgrade + 1`.
    pub minimum_total: u8,
}

/// One required item. `None` fields are wildcards.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
#[allow(clippy::struct_excessive_bools)] // Independent query and allocation options.
pub struct Requirement {
    pub kind: ItemKind,
    /// Optional melee/thrown narrowing; only meaningful for weapon
    /// requirements. `None` matches both, preserving the pre-existing
    /// "any weapon" semantics.
    pub weapon_category: Option<WeaponCategory>,
    pub item: Option<ItemId>,
    pub tier: TierRequirement,
    pub upgrade: UpgradeRequirement,
    pub effect: EffectRequirement,
    /// Whether cursed candidate items are ineligible for this requirement.
    pub require_uncursed: bool,
    /// Choose this offer (or its unique matching OR alternative) at +3 after brewing.
    pub select_trinket: bool,
    /// Maximum acceptable transmutations (0–13), always including initial offers.
    pub trinket_transmutations: u8,
    /// Maximum remaining-deck draws at the floor limit (0–10); natural finds always count.
    pub artifact_transmutations: u8,
    /// Extra predicate on an ordinary assigned item or a selected resin donor.
    /// Blanket slots may reuse that item and never consume another occurrence.
    pub blanket: bool,
    /// Reserve this wand without budgeting Auto resin upgrades for it.
    pub exclude_resin: bool,
    pub source: Option<ItemSource>,
    /// Requirements in the same non-zero group — a *stack* — must resolve
    /// to the same item ID. One anchor unit (a lone requirement, or the
    /// members of one alternative group) may carry constraints; the rest are
    /// bare *copies*, each a separate slot. When the anchor is an
    /// alternative group, the copies are required exactly when a member
    /// carrying the label fills the group's slot, and waived when another
    /// member does: a label on every member means "copies of whichever
    /// matched", a label on one member is that member's own stack.
    pub identity_group: Option<u8>,
    /// Optional inclusive floor limit for this item, independent of the query's
    /// overall generation limit.
    pub max_depth: Option<u8>,
    /// Requirements in the same non-zero group are alternatives: together
    /// they form one *slot*, and a single item matching any member fills it.
    pub alternative_group: Option<u8>,
    /// Optional combined-level constraint shared with other requirements.
    /// Never set on a member of an alternative group.
    pub level_sum: Option<LevelSum>,
}

impl Requirement {
    /// The wildcard requirement for `kind`: any item of the family, with
    /// every predicate open and no relationship — the row every editor
    /// starts a new chip from, and the base a copy is built on.
    #[must_use]
    pub const fn any(kind: ItemKind) -> Self {
        Self {
            kind,
            weapon_category: None,
            item: None,
            tier: TierRequirement::Any,
            upgrade: UpgradeRequirement::Any,
            effect: EffectRequirement::Any,
            require_uncursed: false,
            select_trinket: false,
            trinket_transmutations: 0,
            artifact_transmutations: 0,
            blanket: false,
            exclude_resin: false,
            source: None,
            identity_group: None,
            max_depth: None,
            alternative_group: None,
            level_sum: None,
        }
    }

    #[must_use]
    pub fn matches(self, candidate: &WorldItem) -> bool {
        self.matching_identity(candidate).is_some()
    }

    fn matching_identity(self, candidate: &WorldItem) -> Option<ItemId> {
        if self.trinket_transmutations > 0 || self.artifact_transmutations > 0 {
            // Transmutation requires deck context; Assignment supplies a virtual candidate.
            return None;
        }
        let identity = match self.item {
            None => candidate.item,
            Some(wanted) if wanted == candidate.item => candidate.item,
            Some(_) => return None,
        };
        let definition = item(identity);
        (definition.kind == self.kind
            && self
                .weapon_category
                .is_none_or(|wanted| definition.weapon_category() == Some(wanted))
            && self.tier.matches(definition.tier)
            && self.upgrade.matches(candidate.upgrade)
            && self.effect.matches(candidate.effect)
            && (!self.require_uncursed || !candidate.cursed)
            && self.source.is_none_or(|wanted| wanted == candidate.source))
        .then_some(identity)
    }

    /// The highest upgrade an item of the kind, identity and tier this
    /// requirement asks for can carry, whatever its upgrade filter says.
    ///
    /// Only a tier-[`EXTRA_UPGRADE_TIER`] weapon is levelled past
    /// [`MAX_GENERATED_UPGRADE`], so a requirement that rules that tier out —
    /// by naming an item of another tier, or by filtering the tier away —
    /// stops there.
    #[must_use]
    pub fn upgrade_ceiling(self) -> u8 {
        if self.kind == ItemKind::Artifact {
            return 5;
        }
        if self.kind == ItemKind::Trinket {
            return 0;
        }
        let reaches_the_extra_tier = match self.item {
            Some(item_id) => item(item_id).tier == Some(EXTRA_UPGRADE_TIER),
            None => self.tier.matches(Some(EXTRA_UPGRADE_TIER)),
        };
        if reaches_the_extra_tier {
            self.kind
                .maximum_search_upgrade_for_tier(EXTRA_UPGRADE_TIER)
        } else {
            MAX_GENERATED_UPGRADE
        }
    }

    /// The highest upgrade level an item satisfying this requirement can
    /// carry.
    #[must_use]
    pub fn maximum_upgrade(self) -> u8 {
        match self.upgrade {
            UpgradeRequirement::Exact(wanted) => wanted,
            UpgradeRequirement::Any | UpgradeRequirement::AtLeast(_) => self.upgrade_ceiling(),
        }
    }

    /// The most *levels* — upgrade plus one — an item satisfying this
    /// requirement can contribute to a combined-level group.
    ///
    /// Saturates rather than overflowing: the requirement editor sizes the
    /// groups of rows that have not passed [`Requirement::validate`], where
    /// an exact upgrade can be anything a document held.
    #[must_use]
    pub fn maximum_level(self) -> u8 {
        self.maximum_upgrade().saturating_add(1)
    }

    /// Whether this requirement constrains anything beyond its kind: a named
    /// item, a tier or upgrade bound, an effect, uncursedness, or a source.
    /// A stack's extra copies are exactly the unconstrained requirements; a
    /// per-item floor limit is a placement bound, not an item property, and
    /// does not count.
    #[must_use]
    pub const fn is_bare(self) -> bool {
        self.item.is_none()
            && self.weapon_category.is_none()
            && matches!(self.tier, TierRequirement::Any)
            && matches!(self.upgrade, UpgradeRequirement::Any)
            && matches!(self.effect, EffectRequirement::Any)
            && !self.require_uncursed
            && !self.exclude_resin
            && self.source.is_none()
    }

    /// Checks that an item/effect/upgrade combination is meaningful.
    ///
    /// # Errors
    ///
    /// Returns a validation error for a category mismatch, an effect set of
    /// another family, an upgrade outside the UI's family-specific range, or
    /// an inconsistent group label.
    #[allow(clippy::too_many_lines)] // Validate each independent requirement predicate.
    pub fn validate(self) -> Result<(), QueryError> {
        if self.exclude_resin && (self.kind != ItemKind::Wand || self.blanket) {
            return Err(QueryError::ResinExclusionRequiresWand);
        }
        if self.blanket
            && (self.identity_group.is_some() || self.level_sum.is_some() || self.select_trinket)
        {
            return Err(QueryError::BlanketWithRelations);
        }
        if self.trinket_transmutations > 0
            && (self.kind != ItemKind::Trinket
                || self.trinket_transmutations > crate::trinkets::TRANSMUTATION_COUNT
                || self.select_trinket)
        {
            return Err(QueryError::InvalidTrinketTransmutations);
        }
        if self.artifact_transmutations > 0
            && (self.kind != ItemKind::Artifact
                || self.artifact_transmutations > crate::artifacts::TRANSMUTATION_COUNT)
        {
            return Err(QueryError::InvalidArtifactTransmutations);
        }
        if self.select_trinket && self.kind != ItemKind::Trinket {
            return Err(QueryError::SelectionRequiresTrinket);
        }
        if self.kind == ItemKind::Artifact && self.item.is_none() {
            return Err(QueryError::ArtifactRequiresIdentity);
        }
        if self.kind == ItemKind::Trinket && self.item.is_none() {
            return Err(QueryError::TrinketRequiresIdentity);
        }
        if self
            .item
            .is_some_and(|item_id| item(item_id).kind != self.kind)
        {
            return Err(QueryError::ItemKindMismatch);
        }
        if let Some(category) = self.weapon_category {
            if self.kind != ItemKind::Weapon
                || self
                    .item
                    .is_some_and(|item_id| item_id.weapon_category() != Some(category))
            {
                return Err(QueryError::InvalidWeaponCategory);
            }
        }
        let tierable =
            self.item.is_none() && matches!(self.kind, ItemKind::Weapon | ItemKind::Armor);
        let valid_tier = match self.tier {
            TierRequirement::Any => true,
            TierRequirement::Exact(tier) => {
                tierable && (EXACT_TIER_MIN..=EXACT_TIER_MAX).contains(&tier)
            }
            TierRequirement::AtLeast(tier) | TierRequirement::AtMost(tier) => {
                tierable && (BOUNDED_TIER_MIN..=BOUNDED_TIER_MAX).contains(&tier)
            }
        };
        if !valid_tier {
            return Err(QueryError::InvalidTier);
        }
        let maximum = self.upgrade_ceiling();
        let valid_upgrade = match self.upgrade {
            UpgradeRequirement::Any => true,
            UpgradeRequirement::Exact(upgrade) => (1..=maximum).contains(&upgrade),
            UpgradeRequirement::AtLeast(upgrade) => upgrade <= maximum,
        };
        if !valid_upgrade {
            return Err(QueryError::InvalidUpgrade);
        }
        if self.identity_group == Some(RESERVED_IDENTITY_GROUP) {
            return Err(QueryError::InvalidIdentityGroup);
        }
        if self.alternative_group == Some(RESERVED_GROUP) {
            return Err(QueryError::InvalidAlternativeGroup);
        }
        if self
            .level_sum
            .is_some_and(|sum| sum.group == RESERVED_GROUP || sum.minimum_total == 0)
        {
            return Err(QueryError::InvalidLevelSum);
        }
        // Levels only combine meaningfully across rings — a ring's effect
        // scales with its level, so a +0 and a +1 together grant what one
        // +2 does. No other family adds up that way.
        if self.level_sum.is_some() && self.kind != ItemKind::Ring {
            return Err(QueryError::LevelSumOutsideRings);
        }
        if self.alternative_group.is_some() && self.level_sum.is_some() {
            return Err(QueryError::LevelSumInsideAlternative);
        }
        if self
            .max_depth
            .is_some_and(|depth| !(1..=MAX_SEARCH_DEPTH).contains(&depth))
        {
            return Err(QueryError::InvalidDepth);
        }
        match self.effect {
            EffectRequirement::Any => {}
            EffectRequirement::OneOf(set) if set.family() == self.kind => {}
            EffectRequirement::OneOf(_) => return Err(QueryError::EffectKindMismatch),
        }
        if self.require_uncursed
            && let EffectRequirement::OneOf(set) = self.effect
            && set.is_curses_only()
        {
            return Err(QueryError::UncursedWithCurse);
        }
        Ok(())
    }
}

/// All requirements must be obtainable together in the same generated world.
#[derive(Clone, Debug, Eq, PartialEq)]
#[allow(clippy::struct_excessive_bools)] // Independent search and generation settings.
pub struct SearchQuery {
    /// Automatically choose one offered trinket at +3 before generation.
    /// Explicit trinket requirements take precedence over this setting.
    pub auto_apply_trinket: bool,
    /// Minimum Arcane Resin from surplus wands, after reserving
    /// distinct items for every ordinary requirement. Zero disables fixed
    /// resin; `arcane_resin_auto` derives the amount instead when enabled.
    pub arcane_resin: u16,
    /// Derive the minimum from the assigned wands' costs to reach +3.
    /// When enabled, this takes precedence over `arcane_resin`.
    pub arcane_resin_auto: bool,
    pub arcane_resin_filter: ArcaneResinFilter,
    pub requirements: Vec<Requirement>,
    pub floor_requirements: Vec<crate::floor_filters::FloorRequirement>,
    pub max_depth: u8,
    /// Upstream v3.3.8 challenge mask used while generating candidate worlds.
    pub challenges: Challenges,
    /// Whether an accessible blacksmith room must exist within `max_depth`.
    pub require_blacksmith: bool,
    /// Whether Blacksmith "Smith" rewards are ineligible to satisfy item
    /// requirements. The room may still be required separately for reforging.
    pub exclude_blacksmith_rewards: bool,
    /// Which Wandmaker quest the run must roll, or `None` for any. The quest
    /// item — corpse dust, an elemental ember, or a rotberry seed — is usable
    /// in the dungeon instead of being handed in, so which one a seed offers
    /// is worth searching for on its own; the other three givers' variants
    /// change nothing but the fight, and are reported rather than filtered.
    pub wandmaker_quest: Option<WandmakerQuestType>,
}

impl SearchQuery {
    /// Whether the query reserves surplus wands for a resin condition.
    #[must_use]
    pub const fn needs_resin(&self) -> bool {
        self.arcane_resin_auto || self.arcane_resin > 0
    }

    /// Player-supplied resin, independent of generated donor filters.
    pub(crate) const fn resin_credit(&self) -> u16 {
        if self.arcane_resin_filter.include_mage_wand {
            2
        } else {
            0
        }
    }

    /// Validates bounds and every requirement.
    ///
    /// # Errors
    ///
    /// Returns a [`QueryError`] when no requirements are present, the selected
    /// depth is outside the main dungeon, a requirement is inconsistent, or a
    /// cross-requirement group disagrees with itself.
    pub fn validate(&self) -> Result<(), QueryError> {
        if self
            .requirements
            .iter()
            .all(|requirement| requirement.blanket)
            && (!self.requirements.is_empty()
                || (!self.needs_resin() && self.floor_requirements.is_empty()))
        {
            return Err(QueryError::Empty);
        }
        if !(1..=MAX_SEARCH_DEPTH).contains(&self.max_depth) {
            return Err(QueryError::InvalidDepth);
        }
        if self
            .arcane_resin_filter
            .max_depth
            .is_some_and(|depth| !(1..=MAX_SEARCH_DEPTH).contains(&depth))
        {
            return Err(QueryError::InvalidDepth);
        }
        let mut seen = [false; 25];
        for floor in &self.floor_requirements {
            if !(1..=self.max_depth).contains(&floor.depth) || floor.depth % 5 == 0 {
                return Err(QueryError::InvalidFloorRequirement);
            }
            if seen[usize::from(floor.depth)]
                || (floor.feeling.is_none() && floor.rooms.is_empty() && floor.any_rooms.is_empty())
            {
                return Err(QueryError::InvalidFloorRequirement);
            }
            seen[usize::from(floor.depth)] = true;
        }
        let group_errors = requirement_group_errors(&self.requirements);
        // The rows are checked in order, and a combined-level group whose
        // members disagree on the total is caught at its first dissenting
        // member — so that error outranks the rows after it, as it always
        // has. Every other group error waits until each row passed.
        let dissent = group_errors.first().and_then(|(error, rows)| {
            matches!(error, QueryError::InconsistentLevelSum { .. })
                .then(|| first_dissent(&self.requirements, rows))
                .flatten()
                .map(|index| (index, *error))
        });
        for (index, requirement) in self.requirements.iter().enumerate() {
            requirement.validate()?;
            if let Some((at, error)) = dissent
                && at == index
            {
                return Err(error);
            }
        }
        group_errors
            .first()
            .map_or(Ok(()), |&(error, _)| Err(error))
    }

    /// Every combined-level group of the query, keyed by label.
    #[must_use]
    pub fn level_sum_groups(&self) -> BTreeMap<u8, SumGroup> {
        level_sum_groups(&self.requirements)
    }

    /// The query's slots: requirement indices grouped so that the members of
    /// one alternative group share a slot, in first-appearance order. Every
    /// other requirement is a slot of its own. A world matches when every
    /// ordinary slot is filled by a distinct item matching one of its members;
    /// blanket slots constrain that assignment and its resin donors without
    /// consuming extra items.
    #[must_use]
    pub fn slots(&self) -> Vec<Vec<usize>> {
        slots(&self.requirements)
    }

    /// Slots that require distinct item occurrences.
    #[must_use]
    pub fn ordinary_slots(&self) -> Vec<Vec<usize>> {
        self.slots()
            .into_iter()
            .filter(|slot| !self.requirements[slot[0]].blanket)
            .collect()
    }

    /// Extra conditions evaluated against the assignment and its resin donors.
    #[must_use]
    pub fn blanket_slots(&self) -> Vec<Vec<usize>> {
        self.slots()
            .into_iter()
            .filter(|slot| self.requirements[slot[0]].blanket)
            .collect()
    }

    /// How many slots the query has — what a frontend counts as "requirements"
    /// once alternatives collapse.
    #[must_use]
    pub fn slot_count(&self) -> usize {
        self.slots().len()
    }

    /// How many conditions the scout reports: one per slot, except that all
    /// the slots of one combined-level group collapse into a single
    /// condition, satisfied together or not at all. Floor requirements count once each.
    #[must_use]
    pub fn scout_condition_count(&self) -> usize {
        let slots = self.slots();
        let mut groups: Vec<u8> = Vec::new();
        let mut sum_slots = 0;
        for slot in &slots {
            // Combined-level members never sit in alternative groups.
            if let Some(sum) = self.requirements[slot[0]].level_sum {
                sum_slots += 1;
                if !groups.contains(&sum.group) {
                    groups.push(sum.group);
                }
            }
        }
        slots.len() - sum_slots
            + groups.len()
            + usize::from(self.needs_resin())
            + self.floor_requirements.len()
    }

    /// Matches requirements as an AND query over slots while respecting
    /// distinct item instances, alternative groups, combined-level totals,
    /// and mutually exclusive quest/chest reward branches.
    #[must_use]
    pub fn matches(&self, world: &GeneratedWorld) -> bool {
        if !self
            .floor_requirements
            .iter()
            .all(|floor| floor.matches(world))
        {
            return false;
        }
        // A quest is reported only once its giver's floor is generated, so a
        // world whose prefix stops short of the Wandmaker simply has none and
        // cannot satisfy a variant filter.
        if let Some(wanted) = self.wandmaker_quest
            && !world
                .quests
                .wandmaker
                .is_some_and(|quest| quest.variant == wanted && quest.depth <= self.max_depth)
        {
            return false;
        }
        if self.require_blacksmith
            && !world.items.iter().any(|candidate| {
                candidate.depth <= self.max_depth
                    && candidate.source == ItemSource::BlacksmithReward
            })
        {
            return false;
        }

        let mut assignment = Assignment::prepare(self, world);
        // A member stack's copies may be waived; only the rest must be filled.
        let mandatory = |slot: &&Slot| !slot.optional && slot.gated.is_none();
        if assignment.blankets.iter().any(Vec::is_empty)
            || assignment.slots.iter().filter(mandatory).count() > assignment.items.len()
            || assignment
                .slots
                .iter()
                .filter(mandatory)
                .any(|slot| slot.candidates.is_empty())
        {
            return false;
        }
        assignment.fills_every_slot(0)
    }
}

/// [`SearchQuery::slots`] of a bare requirement list.
fn slots(requirements: &[Requirement]) -> Vec<Vec<usize>> {
    let mut slot_of_group: BTreeMap<u8, usize> = BTreeMap::new();
    let mut slots: Vec<Vec<usize>> = Vec::new();
    for (index, requirement) in requirements.iter().enumerate() {
        match requirement.alternative_group {
            Some(group) => {
                let slot = *slot_of_group.entry(group).or_insert_with(|| {
                    slots.push(Vec::new());
                    slots.len() - 1
                });
                slots[slot].push(index);
            }
            None => slots.push(vec![index]),
        }
    }
    slots
}

/// [`SearchQuery::level_sum_groups`] of a bare requirement list. The sums
/// saturate, since the requirement editor sizes groups of rows that have not
/// been validated.
fn level_sum_groups(requirements: &[Requirement]) -> BTreeMap<u8, SumGroup> {
    let mut groups: BTreeMap<u8, SumGroup> = BTreeMap::new();
    for requirement in requirements {
        if let Some(sum) = requirement.level_sum {
            let group = groups.entry(sum.group).or_default();
            group.members = group.members.saturating_add(1);
            group.minimum_total = u16::from(sum.minimum_total);
            group.capacity = group
                .capacity
                .saturating_add(u16::from(requirement.maximum_level()));
        }
    }
    groups
}

/// The first member of a combined-level group (`rows`, in list order) whose
/// total differs from the first member's, if any does.
fn first_dissent(requirements: &[Requirement], rows: &[usize]) -> Option<usize> {
    let total = |index: usize| requirements[index].level_sum.map(|sum| sum.minimum_total);
    let agreed = total(*rows.first()?);
    rows.iter().copied().find(|&index| total(index) != agreed)
}

/// Every disagreement *between* the requirements of one list — the checks
/// no requirement fails on its own — each with the indices of every
/// requirement of the group at fault, in list order.
///
/// The errors come in the order [`SearchQuery::validate`] meets them (it
/// reports only the first):
///
/// 1. combined-level groups whose members disagree on the total, in the
///    order of their first dissenting member;
/// 2. alternative groups mixing ordinary and blanket requirements, in slot
///    order;
/// 3. identity groups spanning two families or with two anchor units — two
///    constrained units, or a member stack's alternative group beside
///    another group carrying the label or a constrained copy — by label; a
///    group spanning families is not also checked for its units;
/// 4. combined-level groups whose agreed total their members cannot reach
///    ([`SumGroup::attainable_capacity`]), by label.
///
/// The requirement editor lists them all, blaming every row of each group.
/// Rows are not checked on their own here; a row that fails
/// [`Requirement::validate`] still takes part as written.
pub(crate) fn requirement_group_errors(
    requirements: &[Requirement],
) -> Vec<(QueryError, Vec<usize>)> {
    let mut identity_groups: BTreeMap<u8, Vec<usize>> = BTreeMap::new();
    let mut sum_rows: BTreeMap<u8, Vec<usize>> = BTreeMap::new();
    for (index, requirement) in requirements.iter().enumerate() {
        // The reserved label means "no group"; the row reports it itself.
        if let Some(group) = requirement.identity_group
            && group != RESERVED_IDENTITY_GROUP
        {
            identity_groups.entry(group).or_default().push(index);
        }
        if let Some(sum) = requirement.level_sum {
            sum_rows.entry(sum.group).or_default().push(index);
        }
    }
    let mut errors = Vec::new();

    let mut dissenting: Vec<(usize, u8)> = sum_rows
        .iter()
        .filter_map(|(&group, rows)| Some((first_dissent(requirements, rows)?, group)))
        .collect();
    dissenting.sort_unstable();
    for &(_, group) in &dissenting {
        errors.push((
            QueryError::InconsistentLevelSum { group },
            sum_rows[&group].clone(),
        ));
    }

    for slot in slots(requirements) {
        let blanket = requirements[slot[0]].blanket;
        if slot
            .iter()
            .any(|&index| requirements[index].blanket != blanket)
        {
            errors.push((QueryError::MixedBlanketAlternatives, slot));
        }
    }

    // An identity group is a stack: one *anchor unit* — a lone requirement,
    // or the members of one alternative group — may constrain which item
    // the stack binds to; every other member is a bare copy of the anchor's
    // kind. Constraining a second unit would describe two different items
    // forced to be the same, which the stack model deliberately cannot say.
    // When only some members of an alternative group carry the label (a
    // member stack), those members anchor it even when bare: they decide
    // whether its copies are needed at all, so any other group carrying the
    // label, or a constrained copy, is a second anchor. A combined level
    // counts as a constraint there: the copies it would sum may be waived.
    for (label, rows) in identity_groups {
        let kind = requirements[rows[0]].kind;
        if rows.iter().any(|&index| requirements[index].kind != kind) {
            errors.push((QueryError::InconsistentIdentityGroup, rows));
            continue;
        }
        let member_stack = stacks::gating_group(requirements, label).is_some();
        let mut anchor: Option<(Option<u8>, usize)> = None;
        let overconstrained = rows.iter().any(|&index| {
            let requirement = &requirements[index];
            let bare = requirement.is_bare() && !(member_stack && requirement.level_sum.is_some());
            if bare && !(member_stack && requirement.alternative_group.is_some()) {
                return false;
            }
            // Members of one alternative group form a single unit.
            let unit = requirement
                .alternative_group
                .map_or((None, index), |group| (Some(group), 0));
            *anchor.get_or_insert(unit) != unit
        });
        if overconstrained {
            errors.push((QueryError::OverconstrainedIdentityGroup, rows));
        }
    }

    for (group, summary) in level_sum_groups(requirements) {
        if dissenting.iter().any(|&(_, dissent)| dissent == group) {
            continue;
        }
        let attainable = summary.attainable_capacity();
        if summary.minimum_total > attainable {
            errors.push((
                QueryError::UnattainableLevelSum {
                    group,
                    minimum_total: summary.minimum_total,
                    capacity: attainable,
                },
                sum_rows[&group].clone(),
            ));
        }
    }
    errors
}

/// One candidate match for a slot: the world item, the identity the member
/// matched on, the member itself, and whether it is kept rather than reforged.
type SlotCandidate<'query> = (usize, ItemId, &'query Requirement, bool);

/// Size, required total, and upgrade capacity of one combined-level group.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct SumGroup {
    /// How many requirements carry the group.
    pub members: u16,
    /// The total the members agreed on (the last member's, before
    /// validation checks they agree).
    pub minimum_total: u16,
    /// The most combined levels the members could contribute together:
    /// each one's [`Requirement::maximum_level`].
    pub capacity: u16,
}

impl SumGroup {
    /// The most combined levels a generated world can actually put on the
    /// group: [`SumGroup::capacity`] bounded by generation, which levels at
    /// most one ring — the Imp vault's prize — beyond
    /// [`MAX_STANDARD_RING_UPGRADE`]. The matcher keeps pruning against the
    /// per-member `capacity`, which stays sound on any world it is handed;
    /// this tighter bound is what validation holds a total to.
    #[must_use]
    pub fn attainable_capacity(&self) -> u16 {
        let generated = u16::from(MAX_GENERATED_UPGRADE + 1)
            + self
                .members
                .saturating_sub(1)
                .saturating_mul(u16::from(MAX_STANDARD_RING_UPGRADE + 1));
        self.capacity.min(generated)
    }
}

/// Letter every editor shows for a portable group label (A..D), falling
/// back to the number for labels beyond [`MAX_LEVEL_SUM_GROUP`].
#[must_use]
pub fn group_label(group: u8) -> String {
    if (1..=MAX_LEVEL_SUM_GROUP).contains(&group) {
        char::from(b'A' + group - 1).to_string()
    } else {
        group.to_string()
    }
}

/// Running state of one combined-level group inside an assignment.
#[derive(Clone, Copy, Debug, Default)]
struct SumProgress {
    /// Combined levels of the assigned members, each item counting
    /// `upgrade + 1`.
    total: u16,
    /// Level capacity of the members assigned so far.
    spent_capacity: u16,
}

/// One resolved slot: its candidate matches, and whether it may stay empty
/// (a combined-level member is optional — the rest of its group can carry
/// the total).
struct Slot<'query> {
    candidates: Vec<SlotCandidate<'query>>,
    optional: bool,
    /// The alternative group this slot is, when that group gates the copies
    /// of a member stack: whichever member fills it is recorded in
    /// [`Assignment::gates`].
    gate: Option<u8>,
    /// For a copy of a member stack, the gating alternative group and the
    /// stack's label: the copy is waived once a member without the label
    /// fills the group. Such slots are visited after every gate.
    gated: Option<(u8, u8)>,
}

/// Query slots resolved against one world's items: alternatives collapse to
/// one slot, and every mandatory slot must be served by a distinct item.
struct Assignment<'query> {
    resin: resin::ResinSupply,
    items: std::borrow::Cow<'query, [WorldItem]>,
    artifact_outcomes: Vec<crate::artifacts::Outcome>,
    artifact_start: usize,
    conflicts: Vec<Vec<usize>>,
    /// Resolved slots, most constrained slot first.
    slots: Vec<Slot<'query>>,
    sum_groups: BTreeMap<u8, SumGroup>,
    /// Eligible item indices for each blanket condition.
    blankets: Vec<Vec<usize>>,
    used: Vec<bool>,
    resin_cost: u32,
    scenarios: BTreeMap<u16, u64>,
    identities: BTreeMap<u8, ItemId>,
    sums: BTreeMap<u8, SumProgress>,
    /// The identity label of the member filling each gating alternative
    /// group, once one does ([`Slot::gate`]).
    gates: BTreeMap<u8, Option<u8>>,
}

/// What one placement changed, so it can be undone exactly.
#[derive(Clone, Copy)]
struct Undo {
    item_index: usize,
    resin_cost: u32,
    identity: Option<(u8, Option<ItemId>)>,
    scenario: Option<(u16, Option<u64>)>,
    sum: Option<(u8, Option<SumProgress>)>,
}

impl<'query> Assignment<'query> {
    /// Builds per-slot candidate lists under the query's floor limits and the
    /// blacksmith-reward exclusion, sorted most constrained slot first.
    #[allow(clippy::too_many_lines)] // Build candidate lists and their shared acquisition constraints.
    fn prepare(query: &'query SearchQuery, world: &'query GeneratedWorld) -> Self {
        let mut items = crate::trinkets::matching_items(query, world);
        let artifact_start = items.len();
        let artifact_outcomes = crate::artifacts::outcomes(query, world);
        if !artifact_outcomes.is_empty() {
            items
                .to_mut()
                .extend(artifact_outcomes.iter().map(|outcome| {
                    let mut candidate = world.items[outcome.donor].clone();
                    for &identity in
                        &crate::artifacts::deck_at(world, outcome.depth)[..=outcome.step]
                    {
                        candidate.upgrade = candidate.displayed_upgrade();
                        candidate.item = identity;
                    }
                    candidate.depth = outcome.depth;
                    candidate
                }));
        }
        let mut conflicts = vec![Vec::new(); items.len()];
        for (offset, outcome) in artifact_outcomes.iter().enumerate() {
            let index = artifact_start + offset;
            for other in 0..index {
                let conflict = if other >= artifact_start {
                    let previous = &artifact_outcomes[other - artifact_start];
                    previous.donor == outcome.donor || previous.identity == outcome.identity
                        // Snapshots describe independent baseline prefixes, not a replay
                        // of earlier transmutations into later generated floors.
                        || previous.depth != outcome.depth
                        // A later generated donor changes after an earlier transmutation.
                        || world.items[previous.donor].depth > outcome.depth
                        || world.items[outcome.donor].depth > previous.depth
                } else {
                    other == outcome.donor
                        || (item(items[other].item).kind == ItemKind::Artifact
                            && items[other].depth > outcome.depth)
                };
                if conflict {
                    conflicts[index].push(other);
                    conflicts[other].push(index);
                }
            }
        }
        let mut slots: Vec<Slot<'query>> = Vec::new();
        let mut blankets = Vec::new();
        let reforge_copies = if query.arcane_resin_auto {
            resin::reforge_copies(query)
        } else {
            Vec::new()
        };
        let stack_gates = stacks::stack_gates(&query.requirements);
        for slot in query.slots() {
            let slot_first = slot[0];
            let first = &query.requirements[slot_first];
            let gate = first
                .alternative_group
                .filter(|group| stack_gates.contains(&Some(*group)));
            let gated = stacks::gate_of(&stack_gates, slot_first).zip(first.identity_group);
            let mut candidates = Vec::new();
            // Combined-level members never sit in alternative groups, so a
            // slot is optional exactly when its members carry a level sum.
            let optional = slot
                .iter()
                .all(|&member| query.requirements[member].level_sum.is_some());
            for member in slot {
                let requirement = &query.requirements[member];
                let end = (world.items.len() + usize::from(requirement.trinket_transmutations))
                    .min(items.len());
                let predicate = Requirement {
                    trinket_transmutations: 0,
                    artifact_transmutations: 0,
                    ..*requirement
                };
                let artifact_indices =
                    artifact_outcomes
                        .iter()
                        .enumerate()
                        .filter_map(|(offset, outcome)| {
                            (requirement.artifact_transmutations > 0
                                && outcome.depth
                                    == requirement
                                        .max_depth
                                        .unwrap_or(query.max_depth)
                                        .min(query.max_depth)
                                && outcome.step < usize::from(requirement.artifact_transmutations))
                            .then_some(artifact_start + offset)
                        });
                for index in (0..end.min(artifact_start)).chain(artifact_indices) {
                    let candidate = &items[index];
                    if candidate.depth <= query.max_depth
                        && candidate.depth <= requirement.max_depth.unwrap_or(query.max_depth)
                        && (!query.exclude_blacksmith_rewards
                            || candidate.source != ItemSource::BlacksmithReward)
                        && let Some(identity) = predicate.matching_identity(candidate)
                    {
                        candidates.push((
                            index,
                            identity,
                            requirement,
                            !query.arcane_resin_auto || !reforge_copies[member],
                        ));
                    }
                }
            }
            if query.requirements[slot_first].blanket {
                let mut indices: Vec<_> =
                    candidates.iter().map(|&(index, _, _, _)| index).collect();
                indices.sort_unstable();
                indices.dedup();
                blankets.push(indices);
                continue;
            }
            slots.push(Slot {
                candidates,
                optional,
                gate,
                gated,
            });
        }
        let sum_groups = query.level_sum_groups();
        // Fail early by assigning the most constrained slot first — but a
        // member stack's copies only once their gate is filled, which decides
        // whether they are needed.
        slots.sort_by_key(|slot| (slot.gated.is_some(), slot.candidates.len()));
        Self {
            artifact_outcomes,
            artifact_start,
            conflicts,
            resin: resin::ResinSupply::prepare(query, &world.items),
            used: vec![false; items.len()],
            items,
            slots,
            sum_groups,
            blankets,
            resin_cost: 0,
            scenarios: BTreeMap::new(),
            identities: BTreeMap::new(),
            sums: BTreeMap::new(),
            gates: BTreeMap::new(),
        }
    }

    /// Whether the member stack copy in `slot` is waived: its gating group
    /// is filled by a member without the stack's label. A copy whose gate
    /// is still empty (only the scout leaves one empty) is not.
    fn waived(&self, slot: usize) -> bool {
        self.slots[slot].gated.is_some_and(|(group, label)| {
            self.gates
                .get(&group)
                .is_some_and(|filled| *filled != Some(label))
        })
    }

    /// Records which member filled a gating slot, returning the group to
    /// clear when the placement is undone.
    fn fill_gate(&mut self, slot: usize, requirement: &Requirement) -> Option<u8> {
        let group = self.slots[slot].gate?;
        self.gates.insert(group, requirement.identity_group);
        Some(group)
    }

    /// Depth-first assignment requiring every mandatory slot to hold a
    /// distinct item, every combined-level group to reach its total, and
    /// every blanket to match an assigned item or a selected resin donor.
    fn fills_every_slot(&mut self, slot: usize) -> bool {
        if slot == self.slots.len() {
            return self.level_sums_satisfied()
                && self
                    .resin
                    .select_with_blankets(
                        &self.items,
                        &self.used,
                        self.resin_cost,
                        &self.scenarios,
                        &self.blankets,
                        true,
                    )
                    .is_some();
        }
        // Supply only shrinks and Auto's cost only grows as slots claim items.
        if self.resin.enabled() && self.resin_selection().is_none() {
            return false;
        }
        // A waived copy takes no item.
        if self.waived(slot) {
            return self.fills_every_slot(slot + 1);
        }
        for candidate in 0..self.slots[slot].candidates.len() {
            let (item_index, identity, requirement, upgrade_with_resin) =
                self.slots[slot].candidates[candidate];
            let Some(undo) = self.assign(item_index, identity, requirement, upgrade_with_resin)
            else {
                continue;
            };
            let gate = self.fill_gate(slot, requirement);
            if self.fills_every_slot(slot + 1) {
                return true;
            }
            if let Some(group) = gate {
                self.gates.remove(&group);
            }
            self.unassign(undo);
        }
        // A combined-level slot may stay empty: the rest of its group can
        // carry the total.
        self.slots[slot].optional && self.fills_every_slot(slot + 1)
    }

    /// Whether every combined-level group's assigned members reach its total.
    fn level_sums_satisfied(&self) -> bool {
        self.sum_groups.iter().all(|(label, group)| {
            self.sums.get(label).map_or(0, |progress| progress.total) >= group.minimum_total
        })
    }

    fn resin_selection(&self) -> Option<Vec<usize>> {
        self.resin
            .select(&self.items, &self.used, self.resin_cost, &self.scenarios)
    }

    /// Places one item into one slot when every cross-item constraint still
    /// holds, returning the state needed to undo the placement.
    fn assign(
        &mut self,
        item_index: usize,
        identity: ItemId,
        requirement: &Requirement,
        upgrade_with_resin: bool,
    ) -> Option<Undo> {
        if self.used[item_index]
            || self.conflicts[item_index]
                .iter()
                .any(|&other| self.used[other])
        {
            return None;
        }
        let mut undo = Undo {
            item_index,
            resin_cost: 0,
            identity: None,
            scenario: None,
            sum: None,
        };
        if let Some(group) = requirement.identity_group {
            if self
                .identities
                .get(&group)
                .is_some_and(|wanted| *wanted != identity)
            {
                return None;
            }
            undo.identity = Some((group, self.identities.insert(group, identity)));
        }
        if let Some((group, item_scenarios)) =
            self.items[item_index].accessibility.scenario_constraint()
        {
            let compatible =
                self.scenarios.get(&group).copied().unwrap_or(u64::MAX) & item_scenarios;
            if compatible == 0 {
                self.unassign(undo);
                return None;
            }
            undo.scenario = Some((group, self.scenarios.insert(group, compatible)));
        }
        if let Some(sum) = requirement.level_sum {
            let group = self.sum_groups.get(&sum.group).copied().unwrap_or_default();
            let previous = self.sums.get(&sum.group).copied().unwrap_or_default();
            let progress = SumProgress {
                total: previous.total + u16::from(self.items[item_index].upgrade) + 1,
                spent_capacity: previous.spent_capacity + u16::from(requirement.maximum_level()),
            };
            // Prune once even the unassigned members at their caps cannot
            // lift the total to the target.
            let reachable = group.capacity.saturating_sub(progress.spent_capacity);
            if progress.total + reachable < group.minimum_total {
                self.unassign(undo);
                return None;
            }
            undo.sum = Some((sum.group, self.sums.insert(sum.group, progress)));
        }
        if upgrade_with_resin && requirement.kind == ItemKind::Wand && !requirement.exclude_resin {
            undo.resin_cost = resin::upgrade_cost(self.items[item_index].upgrade);
            self.resin_cost += undo.resin_cost;
        }
        self.used[item_index] = true;
        Some(undo)
    }

    fn unassign(&mut self, undo: Undo) {
        self.used[undo.item_index] = false;
        self.resin_cost -= undo.resin_cost;
        rewind(&mut self.sums, undo.sum);
        rewind(&mut self.scenarios, undo.scenario);
        rewind(&mut self.identities, undo.identity);
    }

    /// Combined-level groups short of their total: their assigned members
    /// do not count as satisfied.
    fn failed_sum_groups(&self) -> Vec<u8> {
        self.sum_groups
            .iter()
            .filter(|(label, group)| {
                self.sums.get(label).map_or(0, |progress| progress.total) < group.minimum_total
            })
            .map(|(label, _)| *label)
            .collect()
    }
}

fn rewind<K: Ord, V>(map: &mut BTreeMap<K, V>, previous: Option<(K, Option<V>)>) {
    if let Some((key, previous)) = previous {
        if let Some(previous) = previous {
            map.insert(key, previous);
        } else {
            map.remove(&key);
        }
    }
}

/// Which items of a scouted world satisfy which slots of a query.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ScoutMatches {
    /// One flag per world item, in the scouted world's own item order — the
    /// order [`crate::wire::encode_scout_world`] emits — set for every item
    /// the selection claimed for a satisfied condition.
    pub matched: Vec<bool>,
    /// One flag per world item, set for the matched items the selection
    /// consumes as Arcane Resin donors rather than keeping for a slot.
    pub resin_donors: Vec<bool>,
    /// Matches for transmutations 1–13, separate from generated world item indices.
    pub transmuted_trinkets: [bool; crate::trinkets::TRANSMUTATION_COUNT as usize],
    /// Matched remaining-deck outcomes (floor, zero-based position).
    pub transmuted_artifacts: Vec<(u8, usize)>,
    /// How many conditions the selection satisfies: one per filled plain
    /// slot, plus one per combined-level group whose assigned items reach
    /// its total. A member stack's copy that the selection waives — its
    /// alternative group filled by a member without the stack's label —
    /// counts as satisfied without an item, so a full match satisfies every
    /// condition. A satisfied group flags every contributing item, so more
    /// flags than conditions may be set; items of a group short of its
    /// total are not flagged at all.
    pub matched_requirements: usize,
    /// How many conditions the query has in total — one per slot, with all
    /// the slots of a combined-level group counting once
    /// ([`SearchQuery::scout_condition_count`]).
    pub total_requirements: usize,
}

impl ScoutMatches {
    /// Indices of the selected items, ascending.
    #[must_use]
    pub fn matched_indices(&self) -> Vec<usize> {
        self.matched
            .iter()
            .enumerate()
            .filter_map(|(index, matched)| matched.then_some(index))
            .collect()
    }
}

/// Selects a largest set of distinct world items satisfying as many of
/// `query`'s slots as possible, for explaining a scouted seed: the
/// partial-assignment variant of [`SearchQuery::matches`], which answers the
/// same question but only all-or-nothing.
///
/// The rules are the matcher's: the query's floor limit and each
/// requirement's own, the blacksmith-reward exclusion, one item per slot,
/// any member of an alternative group filling its slot, identity groups
/// bound to a single item ID, a member stack's copies waived when another
/// member fills its group, accessibility scenarios intersected per group,
/// and combined-level groups counting as one condition, satisfied when the
/// assigned members' levels reach the total — a lone +0 ring of a wanted
/// pair that falls short is not highlighted.
/// Floor conditions are counted independently of item witnesses. Other
/// world-level conditions (`require_blacksmith`, the Wandmaker filter) are
/// *not* applied — they say nothing about which item explains which slot.
///
/// A full selection is therefore equivalent to
/// [`SearchQuery::matches`] on a query without those world conditions.
#[must_use]
pub fn scout_matches(world: &GeneratedWorld, query: &SearchQuery) -> ScoutMatches {
    let total_requirements = query.scout_condition_count();
    let mut search = BestSubset {
        assignment: Assignment::prepare(query, world),
        selected: Vec::new(),
        waived: 0,
        best: Vec::new(),
        best_resin: Vec::new(),
        best_conditions: 0,
    };
    search.visit(0);
    let mut matched = vec![false; world.items.len()];
    let mut resin_donors = vec![false; world.items.len()];
    for &index in &search.best_resin {
        resin_donors[index] = true;
    }
    let mut transmuted_trinkets = [false; crate::trinkets::TRANSMUTATION_COUNT as usize];
    let mut transmuted_artifacts = Vec::new();
    for &index in &search.best {
        if index >= search.assignment.artifact_start {
            let outcome =
                &search.assignment.artifact_outcomes[index - search.assignment.artifact_start];
            matched[outcome.donor] = true;
            transmuted_artifacts.push((outcome.depth, outcome.step));
        } else if index < matched.len() {
            matched[index] = true;
        } else {
            transmuted_trinkets[index - matched.len()] = true;
        }
    }
    ScoutMatches {
        matched,
        resin_donors,
        transmuted_trinkets,
        transmuted_artifacts,
        matched_requirements: search.best_conditions
            + query
                .floor_requirements
                .iter()
                .filter(|floor| floor.matches(world))
                .count(),
        total_requirements,
    }
}

/// Backtracking search for the most satisfied conditions, keeping the best
/// selection seen so far and pruning branches which can no longer beat it.
struct BestSubset<'query> {
    assignment: Assignment<'query>,
    /// Assigned items with the combined-level group they serve, if any.
    selected: Vec<(usize, Option<u8>)>,
    /// Member stack copies the selection waives: satisfied conditions that
    /// hold no item.
    waived: usize,
    /// The items of the best selection.
    best: Vec<usize>,
    /// The resin donors among `best`.
    best_resin: Vec<usize>,
    /// The conditions the best selection satisfies.
    best_conditions: usize,
}

impl BestSubset<'_> {
    fn visit(&mut self, slot: usize) {
        if slot == self.assignment.slots.len() {
            // Items serving a group short of its total do not count and are
            // not highlighted; a satisfied group counts once, however many
            // items carried it.
            let failed = self.assignment.failed_sum_groups();
            let mut items: Vec<usize> = Vec::new();
            let mut satisfied_groups: Vec<u8> = Vec::new();
            let mut conditions = self.waived;
            let mut resin_donors = Vec::new();
            for &(item_index, sum_group) in &self.selected {
                match sum_group {
                    None => {
                        conditions += 1;
                        items.push(item_index);
                    }
                    Some(group) if !failed.contains(&group) => {
                        if !satisfied_groups.contains(&group) {
                            satisfied_groups.push(group);
                            conditions += 1;
                        }
                        items.push(item_index);
                    }
                    Some(_) => {}
                }
            }
            if self.assignment.resin.enabled() {
                // An item assigned to an incomplete combined-level group
                // remains reserved but cannot witness a Scout blanket.
                let blankets: Vec<_> = self
                    .assignment
                    .blankets
                    .iter()
                    .map(|indices| {
                        indices
                            .iter()
                            .copied()
                            .filter(|&index| !self.assignment.used[index] || items.contains(&index))
                            .collect()
                    })
                    .collect();
                if let Some(resin_items) = self.assignment.resin.select_with_blankets(
                    &self.assignment.items,
                    &self.assignment.used,
                    self.assignment.resin_cost,
                    &self.assignment.scenarios,
                    &blankets,
                    false,
                ) {
                    conditions += 1;
                    items.extend(&resin_items);
                    resin_donors = resin_items;
                }
            }
            conditions += self
                .assignment
                .blankets
                .iter()
                .filter(|indices| indices.iter().any(|index| items.contains(index)))
                .count();
            if conditions > self.best_conditions {
                self.best_conditions = conditions;
                self.best = items;
                self.best_resin = resin_donors;
            }
            return;
        }
        // The remaining slots bound what this branch can still add: each
        // selected item and each remaining slot satisfies at most one
        // condition.
        if self.selected.len()
            + self.waived
            + (self.assignment.slots.len() - slot)
            + self.assignment.blankets.len()
            + usize::from(self.assignment.resin.enabled())
            <= self.best_conditions
        {
            return;
        }
        // A waived copy is satisfied without an item, and cannot take one.
        if self.assignment.waived(slot) {
            self.waived += 1;
            self.visit(slot + 1);
            self.waived -= 1;
            return;
        }
        for candidate in 0..self.assignment.slots[slot].candidates.len() {
            let (item_index, identity, requirement, upgrade_with_resin) =
                self.assignment.slots[slot].candidates[candidate];
            let Some(undo) =
                self.assignment
                    .assign(item_index, identity, requirement, upgrade_with_resin)
            else {
                continue;
            };
            let gate = self.assignment.fill_gate(slot, requirement);
            self.selected
                .push((item_index, requirement.level_sum.map(|sum| sum.group)));
            self.visit(slot + 1);
            self.selected.pop();
            if let Some(group) = gate {
                self.assignment.gates.remove(&group);
            }
            self.assignment.unassign(undo);
        }
        // Skipping this slot keeps the rest of the selection available.
        self.visit(slot + 1);
    }
}
/// Invalid user query.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum QueryError {
    InvalidFloorRequirement,
    Empty,
    InvalidDepth,
    InvalidUpgrade,
    InvalidTier,
    ItemKindMismatch,
    TrinketRequiresIdentity,
    SelectionRequiresTrinket,
    InvalidTrinketTransmutations,
    InvalidArtifactTransmutations,
    ResinExclusionRequiresWand,
    ArtifactRequiresIdentity,
    InvalidWeaponCategory,
    EffectKindMismatch,
    UncursedWithCurse,
    InvalidIdentityGroup,
    InconsistentIdentityGroup,
    /// An identity group has two anchor units: two of its members outside
    /// one alternative group carry their own constraints, or a member
    /// stack's group sits beside another group carrying the label or a
    /// constrained copy. A stack has one anchor and bare copies.
    OverconstrainedIdentityGroup,
    InvalidAlternativeGroup,
    BlanketWithRelations,
    MixedBlanketAlternatives,
    InvalidLevelSum,
    /// A combined-level group member of a family other than rings.
    LevelSumOutsideRings,
    /// The members of the group disagree on the total.
    InconsistentLevelSum {
        group: u8,
    },
    /// The group's total exceeds what its members can carry together.
    UnattainableLevelSum {
        group: u8,
        minimum_total: u16,
        capacity: u16,
    },
    LevelSumInsideAlternative,
}

impl fmt::Display for QueryError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InconsistentLevelSum { group } => {
                return write!(
                    formatter,
                    "the items in combined level group {} must agree on the total",
                    group_label(*group)
                );
            }
            Self::UnattainableLevelSum {
                group,
                minimum_total,
                capacity,
            } => {
                return write!(
                    formatter,
                    "combined level group {} needs {minimum_total} levels but its items can \
                     reach at most {capacity}",
                    group_label(*group)
                );
            }
            _ => {}
        }
        let message = match self {
            Self::InvalidFloorRequirement => {
                "floor requirements need unique regular floors within the floor limit and a feeling or room filter"
            }
            Self::Empty => {
                "at least one item, resin, or floor requirement is needed; blankets need an ordinary item requirement"
            }
            Self::BlanketWithRelations => {
                "a blanket requirement cannot request extra copies, combined levels, or trinket selection"
            }
            Self::MixedBlanketAlternatives => {
                "an alternative group cannot mix ordinary and blanket requirements"
            }
            Self::InvalidDepth => "maximum depth must be between 1 and 24",
            Self::InvalidUpgrade => {
                "upgrade must be between +1 and +4; only a tier-4 weapon, melee or thrown, reaches +5"
            }
            Self::InvalidTier => {
                "tier filters require a wildcard weapon or armor and a non-redundant tier"
            }
            Self::ResinExclusionRequiresWand => {
                "only an ordinary wand requirement can exclude Auto resin"
            }
            Self::InvalidTrinketTransmutations => {
                "trinket transmutations must be 1–13 on a named trinket without initial-offer selection"
            }
            Self::SelectionRequiresTrinket => "only a named trinket can be selected",
            Self::TrinketRequiresIdentity => "select a trinket",
            Self::InvalidArtifactTransmutations => {
                "artifact transmutations must be 0–10 on a named artifact"
            }
            Self::ArtifactRequiresIdentity => "select an artifact",
            Self::ItemKindMismatch => "selected item is in a different category",
            Self::InvalidWeaponCategory => {
                "melee/thrown filters require a weapon requirement and a matching item"
            }
            Self::EffectKindMismatch => "selected enchantment or glyph is inapplicable",
            Self::UncursedWithCurse => "an uncursed item cannot be limited to curses",
            Self::InvalidIdentityGroup => "identity group zero is reserved for no group",
            Self::InconsistentIdentityGroup => {
                "linked item requirements must use the same category"
            }
            Self::OverconstrainedIdentityGroup => {
                "only one linked requirement (or the members of one alternative group) may \
                 carry item constraints; the extra copies must be plain"
            }
            Self::InvalidAlternativeGroup => "alternative group zero is reserved for no group",
            Self::InvalidLevelSum => "combined level groups need a non-zero group and total",
            Self::LevelSumOutsideRings => "levels are only counted together across rings",
            Self::InconsistentLevelSum { .. } | Self::UnattainableLevelSum { .. } => {
                unreachable!("written above")
            }
            Self::LevelSumInsideAlternative => {
                "a combined level group cannot include alternative requirements"
            }
        };
        formatter.write_str(message)
    }
}

impl std::error::Error for QueryError {}

#[cfg(test)]
mod tests {
    use crate::catalog::{Effect, ItemId, ItemKind, WeaponCategory, WeaponEffect};
    use crate::model::{Accessibility, GeneratedWorld, ItemSource, WorldItem};
    use crate::run::RingGems;
    use crate::seed::DungeonSeed;

    use super::{
        EffectRequirement, EffectSet, LevelSum, QueryError, Requirement, SearchQuery,
        TierRequirement, UpgradeRequirement, scout_matches,
    };

    fn world_item(item: ItemId, accessibility: Accessibility) -> WorldItem {
        WorldItem {
            item,
            upgrade: 2,
            effect: None,
            cursed: false,
            depth: 3,
            source: ItemSource::GhostReward,
            accessibility,
            secret: false,
        }
    }

    fn requirement(item: ItemId) -> Requirement {
        Requirement {
            kind: crate::catalog::item(item).kind,
            weapon_category: None,
            item: Some(item),
            tier: TierRequirement::Any,
            upgrade: UpgradeRequirement::Exact(2),
            effect: EffectRequirement::Any,
            require_uncursed: false,
            select_trinket: false,
            trinket_transmutations: 0,
            artifact_transmutations: 0,
            blanket: false,
            exclude_resin: false,
            source: None,
            identity_group: None,
            max_depth: None,
            alternative_group: None,
            level_sum: None,
        }
    }

    #[test]
    fn and_query_requires_distinct_item_occurrences() {
        let query = SearchQuery {
            floor_requirements: Vec::new(),
            auto_apply_trinket: false,
            arcane_resin_filter: crate::query::ArcaneResinFilter::default(),
            arcane_resin_auto: false,
            arcane_resin: 0,
            requirements: vec![requirement(ItemId::Sword), requirement(ItemId::Sword)],
            max_depth: 4,
            challenges: crate::challenges::Challenges::NONE,
            require_blacksmith: false,
            exclude_blacksmith_rewards: false,
            wandmaker_quest: None,
        };
        let one = GeneratedWorld {
            floor_rooms: Vec::new(),
            artifact_decks: Vec::new(),
            feelings: Vec::new(),
            quests: crate::quests::QuestSummary::default(),
            seed: DungeonSeed::MIN,
            items: vec![world_item(ItemId::Sword, Accessibility::Independent)],
            ring_gems: RingGems::UNSHUFFLED,
        };
        assert!(!query.matches(&one));
        let two = GeneratedWorld {
            floor_rooms: Vec::new(),
            artifact_decks: Vec::new(),
            feelings: Vec::new(),
            quests: crate::quests::QuestSummary::default(),
            seed: DungeonSeed::MIN,
            items: vec![
                world_item(ItemId::Sword, Accessibility::Independent),
                world_item(ItemId::Sword, Accessibility::Independent),
            ],
            ring_gems: RingGems::UNSHUFFLED,
        };
        assert!(query.matches(&two));
    }

    #[test]
    fn wandmaker_filter_needs_the_quest_itself_inside_the_floor_limit() {
        use crate::quests::{QuestSummary, ScheduledQuest, WandmakerQuestType};

        let mut query = SearchQuery {
            floor_requirements: Vec::new(),
            auto_apply_trinket: false,
            arcane_resin_filter: crate::query::ArcaneResinFilter::default(),
            arcane_resin_auto: false,
            arcane_resin: 0,
            requirements: vec![requirement(ItemId::Sword)],
            max_depth: 24,
            challenges: crate::challenges::Challenges::NONE,
            require_blacksmith: false,
            exclude_blacksmith_rewards: false,
            wandmaker_quest: Some(WandmakerQuestType::Rotberry),
        };
        let world = |wandmaker| GeneratedWorld {
            floor_rooms: Vec::new(),
            artifact_decks: Vec::new(),
            feelings: Vec::new(),
            quests: QuestSummary {
                wandmaker,
                ..QuestSummary::default()
            },
            seed: DungeonSeed::MIN,
            items: vec![world_item(ItemId::Sword, Accessibility::Independent)],
            ring_gems: RingGems::UNSHUFFLED,
        };
        let rotberry = ScheduledQuest {
            variant: WandmakerQuestType::Rotberry,
            depth: 8,
        };

        assert!(query.matches(&world(Some(rotberry))));
        assert!(!query.matches(&world(Some(ScheduledQuest {
            variant: WandmakerQuestType::CorpseDust,
            depth: 8,
        }))));
        // A prefix that never reached the Prison has no Wandmaker at all.
        assert!(!query.matches(&world(None)));

        // The item requirement is unaffected either way.
        query.wandmaker_quest = None;
        assert!(query.matches(&world(None)));

        // A quest below the floor limit still counts; one above cannot.
        query.wandmaker_quest = Some(WandmakerQuestType::Rotberry);
        query.max_depth = 8;
        assert!(query.matches(&world(Some(rotberry))));
        query.max_depth = 7;
        assert!(!query.matches(&world(Some(rotberry))));
    }

    #[test]
    fn uncursed_requirement_rejects_cursed_copies() {
        let mut candidate = world_item(ItemId::Sword, Accessibility::Independent);
        let mut wanted = requirement(ItemId::Sword);
        wanted.require_uncursed = true;

        assert!(wanted.matches(&candidate));
        candidate.cursed = true;
        assert!(!wanted.matches(&candidate));
        wanted.require_uncursed = false;
        assert!(wanted.matches(&candidate));
    }

    #[test]
    fn requirement_floor_limit_is_inclusive() {
        let world = GeneratedWorld {
            floor_rooms: Vec::new(),
            artifact_decks: Vec::new(),
            feelings: Vec::new(),
            quests: crate::quests::QuestSummary::default(),
            seed: DungeonSeed::MIN,
            items: vec![world_item(ItemId::Sword, Accessibility::Independent)],
            ring_gems: RingGems::UNSHUFFLED,
        };
        let mut limited = requirement(ItemId::Sword);
        limited.max_depth = Some(2);
        let mut query = SearchQuery {
            floor_requirements: Vec::new(),
            auto_apply_trinket: false,
            arcane_resin_filter: crate::query::ArcaneResinFilter::default(),
            arcane_resin_auto: false,
            arcane_resin: 0,
            requirements: vec![limited],
            max_depth: 24,
            challenges: crate::challenges::Challenges::NONE,
            require_blacksmith: false,
            exclude_blacksmith_rewards: false,
            wandmaker_quest: None,
        };
        assert!(!query.matches(&world));
        query.requirements[0].max_depth = Some(3);
        assert!(query.matches(&world));
    }

    #[test]
    fn mutually_exclusive_rewards_cannot_satisfy_and_query() {
        let query = SearchQuery {
            floor_requirements: Vec::new(),
            auto_apply_trinket: false,
            arcane_resin_filter: crate::query::ArcaneResinFilter::default(),
            arcane_resin_auto: false,
            arcane_resin: 0,
            requirements: vec![requirement(ItemId::Sword), requirement(ItemId::MailArmor)],
            max_depth: 4,
            challenges: crate::challenges::Challenges::NONE,
            require_blacksmith: false,
            exclude_blacksmith_rewards: false,
            wandmaker_quest: None,
        };
        let world = GeneratedWorld {
            floor_rooms: Vec::new(),
            artifact_decks: Vec::new(),
            feelings: Vec::new(),
            quests: crate::quests::QuestSummary::default(),
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
            ring_gems: RingGems::UNSHUFFLED,
        };
        assert!(!query.matches(&world));
    }

    #[test]
    fn same_choice_option_and_independent_rewards_can_match() {
        let query = SearchQuery {
            floor_requirements: Vec::new(),
            auto_apply_trinket: false,
            arcane_resin_filter: crate::query::ArcaneResinFilter::default(),
            arcane_resin_auto: false,
            arcane_resin: 0,
            requirements: vec![requirement(ItemId::Sword), requirement(ItemId::MailArmor)],
            max_depth: 4,
            challenges: crate::challenges::Challenges::NONE,
            require_blacksmith: false,
            exclude_blacksmith_rewards: false,
            wandmaker_quest: None,
        };
        let world = GeneratedWorld {
            floor_rooms: Vec::new(),
            artifact_decks: Vec::new(),
            feelings: Vec::new(),
            quests: crate::quests::QuestSummary::default(),
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
            ring_gems: RingGems::UNSHUFFLED,
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
            floor_rooms: Vec::new(),
            artifact_decks: Vec::new(),
            feelings: Vec::new(),
            quests: crate::quests::QuestSummary::default(),
            seed: DungeonSeed::MIN,
            items: vec![sword, armor, wand],
            ring_gems: RingGems::UNSHUFFLED,
        };

        let compatible = SearchQuery {
            floor_requirements: Vec::new(),
            auto_apply_trinket: false,
            arcane_resin_filter: crate::query::ArcaneResinFilter::default(),
            arcane_resin_auto: false,
            arcane_resin: 0,
            requirements: vec![requirement(ItemId::Sword), requirement(ItemId::MailArmor)],
            max_depth: 4,
            challenges: crate::challenges::Challenges::NONE,
            require_blacksmith: false,
            exclude_blacksmith_rewards: false,
            wandmaker_quest: None,
        };
        assert!(compatible.matches(&world));

        let incompatible = SearchQuery {
            floor_requirements: Vec::new(),
            auto_apply_trinket: false,
            arcane_resin_filter: crate::query::ArcaneResinFilter::default(),
            arcane_resin_auto: false,
            arcane_resin: 0,
            requirements: vec![requirement(ItemId::Sword), requirement(ItemId::WandFrost)],
            max_depth: 4,
            challenges: crate::challenges::Challenges::NONE,
            require_blacksmith: false,
            exclude_blacksmith_rewards: false,
            wandmaker_quest: None,
        };
        assert!(!incompatible.matches(&world));
    }

    #[test]
    fn validation_rejects_wrong_category() {
        let invalid = Requirement {
            kind: ItemKind::Wand,
            weapon_category: None,
            item: Some(ItemId::Sword),
            tier: TierRequirement::Any,
            upgrade: UpgradeRequirement::Exact(2),
            effect: EffectRequirement::Any,
            require_uncursed: false,
            select_trinket: false,
            trinket_transmutations: 0,
            artifact_transmutations: 0,
            blanket: false,
            exclude_resin: false,
            source: None,
            identity_group: None,
            max_depth: None,
            alternative_group: None,
            level_sum: None,
        };
        assert!(invalid.validate().is_err());
    }

    #[test]
    fn weapon_category_narrows_wildcard_weapon_requirements() {
        use crate::catalog::WeaponCategory;

        let any_weapon = Requirement {
            kind: ItemKind::Weapon,
            weapon_category: None,
            item: None,
            tier: TierRequirement::Any,
            upgrade: UpgradeRequirement::Any,
            effect: EffectRequirement::Any,
            require_uncursed: false,
            select_trinket: false,
            trinket_transmutations: 0,
            artifact_transmutations: 0,
            blanket: false,
            exclude_resin: false,
            source: None,
            identity_group: None,
            max_depth: None,
            alternative_group: None,
            level_sum: None,
        };
        let melee = Requirement {
            weapon_category: Some(WeaponCategory::Melee),
            ..any_weapon
        };
        let thrown = Requirement {
            weapon_category: Some(WeaponCategory::Thrown),
            ..any_weapon
        };
        let sword = world_item(ItemId::Sword, Accessibility::Independent);
        let shuriken = world_item(ItemId::Shuriken, Accessibility::Independent);
        let dart = world_item(ItemId::PoisonDart, Accessibility::Independent);

        assert!(any_weapon.matches(&sword));
        assert!(any_weapon.matches(&shuriken));
        assert!(melee.matches(&sword));
        assert!(!melee.matches(&shuriken));
        assert!(!melee.matches(&dart));
        assert!(!thrown.matches(&sword));
        assert!(thrown.matches(&shuriken));
        assert!(thrown.matches(&dart));

        // Tier filters compose with the category filter.
        let tier_five_thrown = Requirement {
            tier: TierRequirement::Exact(5),
            ..thrown
        };
        assert_eq!(tier_five_thrown.validate(), Ok(()));
        assert!(tier_five_thrown.matches(&world_item(
            ItemId::ThrowingHammer,
            Accessibility::Independent
        )));
        assert!(
            !tier_five_thrown.matches(&world_item(ItemId::Greatsword, Accessibility::Independent))
        );
        assert!(!tier_five_thrown.matches(&shuriken));
    }

    #[test]
    fn weapon_category_validation_requires_a_consistent_weapon() {
        use crate::catalog::WeaponCategory;

        let melee_wand = Requirement {
            weapon_category: Some(WeaponCategory::Melee),
            ..requirement(ItemId::WandFrost)
        };
        assert_eq!(
            melee_wand.validate(),
            Err(QueryError::InvalidWeaponCategory)
        );

        let melee_shuriken = Requirement {
            weapon_category: Some(WeaponCategory::Melee),
            ..requirement(ItemId::Shuriken)
        };
        assert_eq!(
            melee_shuriken.validate(),
            Err(QueryError::InvalidWeaponCategory)
        );

        let thrown_shuriken = Requirement {
            weapon_category: Some(WeaponCategory::Thrown),
            ..requirement(ItemId::Shuriken)
        };
        assert_eq!(thrown_shuriken.validate(), Ok(()));
    }

    #[test]
    fn validation_rejects_uncursed_items_with_a_curse() {
        let invalid = Requirement {
            effect: EffectRequirement::exactly(Effect::Weapon(WeaponEffect::Displacing)),
            require_uncursed: true,
            select_trinket: false,
            trinket_transmutations: 0,
            artifact_transmutations: 0,
            blanket: false,
            exclude_resin: false,
            ..requirement(ItemId::Sword)
        };
        assert_eq!(invalid.validate(), Err(QueryError::UncursedWithCurse));
    }

    #[test]
    fn upgrade_ceilings_follow_the_item_kind_and_tier() {
        let ring = Requirement {
            kind: ItemKind::Ring,
            weapon_category: None,
            item: Some(ItemId::RingSharpshooting),
            tier: TierRequirement::Any,
            upgrade: UpgradeRequirement::Exact(4),
            effect: EffectRequirement::Any,
            require_uncursed: false,
            select_trinket: false,
            trinket_transmutations: 0,
            artifact_transmutations: 0,
            blanket: false,
            exclude_resin: false,
            source: None,
            identity_group: None,
            max_depth: None,
            alternative_group: None,
            level_sum: None,
        };
        assert_eq!(ring.validate(), Ok(()));

        let wand = Requirement {
            kind: ItemKind::Wand,
            weapon_category: None,
            item: Some(ItemId::WandFrost),
            tier: TierRequirement::Any,
            upgrade: UpgradeRequirement::Exact(4),
            effect: EffectRequirement::Any,
            require_uncursed: false,
            select_trinket: false,
            trinket_transmutations: 0,
            artifact_transmutations: 0,
            blanket: false,
            exclude_resin: false,
            source: None,
            identity_group: None,
            max_depth: None,
            alternative_group: None,
            level_sum: None,
        };
        assert_eq!(wand.validate(), Ok(()));
        let five_wand = Requirement {
            upgrade: UpgradeRequirement::Exact(5),
            ..wand
        };
        assert_eq!(five_wand.validate(), Err(QueryError::InvalidUpgrade));

        let armor = Requirement {
            kind: ItemKind::Armor,
            item: Some(ItemId::PlateArmor),
            upgrade: UpgradeRequirement::Exact(5),
            ..wand
        };
        assert_eq!(armor.validate(), Err(QueryError::InvalidUpgrade));
    }

    /// Only the tier-4 weapons are levelled past `+4`, melee and thrown
    /// alike; every other tier stops one short.
    #[test]
    fn only_a_tier_four_weapon_reaches_the_top_upgrade() {
        let wand = Requirement {
            kind: ItemKind::Wand,
            weapon_category: None,
            item: Some(ItemId::WandFrost),
            tier: TierRequirement::Any,
            upgrade: UpgradeRequirement::Exact(4),
            effect: EffectRequirement::Any,
            require_uncursed: false,
            select_trinket: false,
            trinket_transmutations: 0,
            artifact_transmutations: 0,
            blanket: false,
            exclude_resin: false,
            source: None,
            identity_group: None,
            max_depth: None,
            alternative_group: None,
            level_sum: None,
        };
        let battle_axe = Requirement {
            kind: ItemKind::Weapon,
            item: Some(ItemId::BattleAxe),
            upgrade: UpgradeRequirement::AtLeast(5),
            ..wand
        };
        assert_eq!(battle_axe.validate(), Ok(()));
        let six_battle_axe = Requirement {
            upgrade: UpgradeRequirement::Exact(6),
            ..battle_axe
        };
        assert_eq!(six_battle_axe.validate(), Err(QueryError::InvalidUpgrade));
        let javelin = Requirement {
            item: Some(ItemId::Javelin),
            weapon_category: Some(WeaponCategory::Thrown),
            upgrade: UpgradeRequirement::Exact(5),
            ..battle_axe
        };
        assert_eq!(javelin.validate(), Ok(()));

        let sword = Requirement {
            item: Some(ItemId::Sword),
            upgrade: UpgradeRequirement::Exact(5),
            ..battle_axe
        };
        assert_eq!(sword.validate(), Err(QueryError::InvalidUpgrade));
        assert_eq!(
            Requirement {
                upgrade: UpgradeRequirement::Exact(4),
                ..sword
            }
            .validate(),
            Ok(())
        );
        let trident = Requirement {
            item: Some(ItemId::Trident),
            ..sword
        };
        assert_eq!(trident.validate(), Err(QueryError::InvalidUpgrade));

        // A wildcard weapon reaches +5 unless its tier filter rules tier 4 out.
        let any_weapon = Requirement {
            item: None,
            upgrade: UpgradeRequirement::Exact(5),
            ..battle_axe
        };
        assert_eq!(any_weapon.validate(), Ok(()));
        for tier in [
            TierRequirement::Exact(4),
            TierRequirement::AtLeast(4),
            TierRequirement::AtMost(4),
        ] {
            assert_eq!(
                Requirement { tier, ..any_weapon }.validate(),
                Ok(()),
                "{tier:?}"
            );
        }
        for tier in [
            TierRequirement::Exact(5),
            TierRequirement::Exact(2),
            TierRequirement::AtMost(3),
        ] {
            assert_eq!(
                Requirement { tier, ..any_weapon }.validate(),
                Err(QueryError::InvalidUpgrade),
                "{tier:?}"
            );
        }
    }

    #[test]
    fn tier_predicates_match_exact_minimum_and_maximum_tiers() {
        let tier_five = Requirement {
            kind: ItemKind::Weapon,
            weapon_category: None,
            item: None,
            tier: TierRequirement::Exact(5),
            upgrade: UpgradeRequirement::Exact(2),
            effect: EffectRequirement::Any,
            require_uncursed: false,
            select_trinket: false,
            trinket_transmutations: 0,
            artifact_transmutations: 0,
            blanket: false,
            exclude_resin: false,
            source: None,
            identity_group: None,
            max_depth: None,
            alternative_group: None,
            level_sum: None,
        };
        assert!(tier_five.matches(&world_item(ItemId::Greatsword, Accessibility::Independent)));
        assert!(!tier_five.matches(&world_item(ItemId::Longsword, Accessibility::Independent)));

        let tier_four_plus = Requirement {
            tier: TierRequirement::AtLeast(4),
            ..tier_five
        };
        assert!(tier_four_plus.matches(&world_item(ItemId::Longsword, Accessibility::Independent)));
        assert!(
            tier_four_plus.matches(&world_item(ItemId::Greatsword, Accessibility::Independent))
        );
        assert!(!tier_four_plus.matches(&world_item(ItemId::Sword, Accessibility::Independent)));

        let tier_four_or_lower = Requirement {
            tier: TierRequirement::AtMost(4),
            ..tier_five
        };
        assert!(
            tier_four_or_lower.matches(&world_item(ItemId::Longsword, Accessibility::Independent))
        );
        assert!(tier_four_or_lower.matches(&world_item(ItemId::Sword, Accessibility::Independent)));
        assert!(
            !tier_four_or_lower
                .matches(&world_item(ItemId::Greatsword, Accessibility::Independent))
        );

        let invalid = Requirement {
            kind: ItemKind::Wand,
            ..tier_five
        };
        assert_eq!(invalid.validate(), Err(QueryError::InvalidTier));

        let tier_one = Requirement {
            tier: TierRequirement::Exact(1),
            ..tier_five
        };
        assert_eq!(tier_one.validate(), Err(QueryError::InvalidTier));

        let redundant_maximum = Requirement {
            tier: TierRequirement::AtMost(5),
            ..tier_five
        };
        assert_eq!(redundant_maximum.validate(), Err(QueryError::InvalidTier));

        for redundant in [
            TierRequirement::AtLeast(2),
            TierRequirement::AtLeast(5),
            TierRequirement::AtMost(2),
        ] {
            assert_eq!(
                Requirement {
                    tier: redundant,
                    ..tier_five
                }
                .validate(),
                Err(QueryError::InvalidTier)
            );
        }
    }

    #[test]
    fn linked_wands_require_distinct_copies_and_a_blacksmith_in_range() {
        let linked = |upgrade, source| Requirement {
            kind: ItemKind::Wand,
            weapon_category: None,
            item: None,
            tier: TierRequirement::Any,
            upgrade,
            effect: EffectRequirement::Any,
            require_uncursed: false,
            select_trinket: false,
            trinket_transmutations: 0,
            artifact_transmutations: 0,
            blanket: false,
            exclude_resin: false,
            source,
            identity_group: Some(1),
            max_depth: None,
            alternative_group: None,
            level_sum: None,
        };
        let mut query = SearchQuery {
            floor_requirements: Vec::new(),
            auto_apply_trinket: false,
            arcane_resin_filter: crate::query::ArcaneResinFilter::default(),
            arcane_resin_auto: false,
            arcane_resin: 0,
            requirements: vec![
                linked(
                    UpgradeRequirement::Exact(3),
                    Some(ItemSource::WandmakerReward),
                ),
                linked(UpgradeRequirement::Any, None),
                linked(UpgradeRequirement::Any, None),
                Requirement {
                    kind: ItemKind::Wand,
                    weapon_category: None,
                    item: None,
                    tier: TierRequirement::Any,
                    upgrade: UpgradeRequirement::Exact(1),
                    effect: EffectRequirement::Any,
                    require_uncursed: false,
                    select_trinket: false,
                    trinket_transmutations: 0,
                    artifact_transmutations: 0,
                    blanket: false,
                    exclude_resin: false,
                    source: None,
                    identity_group: None,
                    max_depth: None,
                    alternative_group: None,
                    level_sum: None,
                },
            ],
            max_depth: 14,
            challenges: crate::challenges::Challenges::NONE,
            require_blacksmith: true,
            exclude_blacksmith_rewards: false,
            wandmaker_quest: None,
        };
        let make = |item, upgrade, depth, source| WorldItem {
            item,
            upgrade,
            effect: None,
            cursed: false,
            depth,
            source,
            accessibility: Accessibility::Independent,
            secret: false,
        };
        let world = GeneratedWorld {
            floor_rooms: Vec::new(),
            artifact_decks: Vec::new(),
            feelings: Vec::new(),
            quests: crate::quests::QuestSummary::default(),
            seed: DungeonSeed::MIN,
            items: vec![
                make(ItemId::WandFrost, 3, 7, ItemSource::WandmakerReward),
                make(ItemId::WandFrost, 0, 2, ItemSource::Heap),
                make(ItemId::WandFrost, 1, 4, ItemSource::Chest),
                make(ItemId::WandLightning, 1, 5, ItemSource::Heap),
                make(ItemId::Sword, 2, 13, ItemSource::BlacksmithReward),
            ],
            ring_gems: RingGems::UNSHUFFLED,
        };

        assert_eq!(query.validate(), Ok(()));
        assert!(query.matches(&world));

        let mut wrong_type = world.clone();
        wrong_type.items[2].item = ItemId::WandLightning;
        assert!(!query.matches(&wrong_type));

        query.max_depth = 12;
        assert!(!query.matches(&world));
    }

    #[test]
    fn smith_rewards_can_be_excluded_without_hiding_the_blacksmith() {
        let mut query = SearchQuery {
            floor_requirements: Vec::new(),
            auto_apply_trinket: false,
            arcane_resin_filter: crate::query::ArcaneResinFilter::default(),
            arcane_resin_auto: false,
            arcane_resin: 0,
            requirements: vec![requirement(ItemId::Sword)],
            max_depth: 14,
            challenges: crate::challenges::Challenges::NONE,
            require_blacksmith: true,
            exclude_blacksmith_rewards: true,
            wandmaker_quest: None,
        };
        let make = |source| WorldItem {
            item: ItemId::Sword,
            upgrade: 2,
            effect: None,
            cursed: false,
            depth: 13,
            source,
            accessibility: Accessibility::Independent,
            secret: false,
        };
        let smith_only = GeneratedWorld {
            floor_rooms: Vec::new(),
            artifact_decks: Vec::new(),
            feelings: Vec::new(),
            quests: crate::quests::QuestSummary::default(),
            seed: DungeonSeed::MIN,
            items: vec![make(ItemSource::BlacksmithReward)],
            ring_gems: RingGems::UNSHUFFLED,
        };

        assert!(!query.matches(&smith_only));

        let mut reforging_setup = smith_only.clone();
        reforging_setup.items.push(make(ItemSource::Heap));
        assert!(query.matches(&reforging_setup));

        query.require_blacksmith = false;
        let no_blacksmith = GeneratedWorld {
            floor_rooms: Vec::new(),
            artifact_decks: Vec::new(),
            feelings: Vec::new(),
            quests: crate::quests::QuestSummary::default(),
            seed: DungeonSeed::MIN,
            items: vec![make(ItemSource::Heap)],
            ring_gems: RingGems::UNSHUFFLED,
        };
        assert!(query.matches(&no_blacksmith));
    }

    #[test]
    fn a_stack_carries_constraints_on_one_member_only() {
        let linked = |item| Requirement {
            kind: ItemKind::Wand,
            weapon_category: None,
            item,
            tier: TierRequirement::Any,
            upgrade: UpgradeRequirement::Any,
            effect: EffectRequirement::Any,
            require_uncursed: false,
            select_trinket: false,
            trinket_transmutations: 0,
            artifact_transmutations: 0,
            blanket: false,
            exclude_resin: false,
            source: None,
            identity_group: Some(1),
            max_depth: None,
            alternative_group: None,
            level_sum: None,
        };
        let query = |members: Vec<Requirement>| SearchQuery {
            floor_requirements: Vec::new(),
            auto_apply_trinket: false,
            arcane_resin_filter: crate::query::ArcaneResinFilter::default(),
            arcane_resin_auto: false,
            arcane_resin: 0,
            requirements: members,
            max_depth: 24,
            challenges: crate::challenges::Challenges::NONE,
            require_blacksmith: false,
            exclude_blacksmith_rewards: false,
            wandmaker_quest: None,
        };

        // Two members naming items would describe two different wands forced
        // to be the same wand; the stack model refuses it.
        assert_eq!(
            query(vec![
                linked(Some(ItemId::WandFrost)),
                linked(None),
                linked(Some(ItemId::WandLightning)),
            ])
            .validate(),
            Err(QueryError::OverconstrainedIdentityGroup)
        );
        // One anchor with bare copies is the intended shape.
        assert_eq!(
            query(vec![
                linked(Some(ItemId::WandFrost)),
                linked(None),
                linked(None),
            ])
            .validate(),
            Ok(())
        );
        // Members of different kinds never describe one item.
        assert_eq!(
            query(vec![
                linked(None),
                Requirement {
                    kind: ItemKind::Ring,
                    ..linked(None)
                },
            ])
            .validate(),
            Err(QueryError::InconsistentIdentityGroup)
        );
    }

    #[test]
    fn a_stack_can_anchor_on_a_whole_alternative_group() {
        // "Runic Blade OR War Hammer, plus two more of whichever matched":
        // the anchor unit is the alternative group, the copies are bare.
        let anchor = |item| Requirement {
            item: Some(item),
            upgrade: UpgradeRequirement::AtLeast(1),
            alternative_group: Some(1),
            identity_group: Some(1),
            ..plain(ItemKind::Weapon)
        };
        let copy = Requirement {
            identity_group: Some(1),
            ..plain(ItemKind::Weapon)
        };
        let query = SearchQuery {
            requirements: vec![
                anchor(ItemId::RunicBlade),
                anchor(ItemId::WarHammer),
                copy,
                copy,
            ],
            ..scout_query(Vec::new())
        };
        assert_eq!(query.validate(), Ok(()));
        assert_eq!(query.slot_count(), 3);

        // Three hammers: the group binds to the hammer and the copies follow.
        assert!(query.matches(&scout_world(vec![
            upgraded(ItemId::WarHammer, 1),
            upgraded(ItemId::WarHammer, 0),
            upgraded(ItemId::WarHammer, 0),
        ])));
        // Copies of the wrong identity do not count, whichever member won.
        assert!(!query.matches(&scout_world(vec![
            upgraded(ItemId::WarHammer, 1),
            upgraded(ItemId::WarHammer, 0),
            upgraded(ItemId::RunicBlade, 0),
        ])));
        // An upgraded blade with two more blades matches through the other
        // alternative.
        assert!(query.matches(&scout_world(vec![
            upgraded(ItemId::RunicBlade, 2),
            upgraded(ItemId::RunicBlade, 0),
            upgraded(ItemId::RunicBlade, 0),
        ])));
    }

    /// A query decoded from a JSON requirement list, validated.
    fn stack_query(requirements: &str) -> SearchQuery {
        crate::json_query::decode(&format!(r#"{{"requirements":{requirements}}}"#)).unwrap()
    }

    /// A world item at `depth`, +0.
    fn found(item: ItemId, depth: u8) -> WorldItem {
        WorldItem {
            depth,
            ..upgraded(item, 0)
        }
    }

    #[test]
    fn a_label_on_one_member_is_that_members_own_stack() {
        use ItemId::{WandDisintegration as Disintegration, WandFrost as Frost};
        // {Frost ×2 | Disintegration}: two Wands of Frost, or one Wand of
        // Disintegration.
        let query = stack_query(
            r#"[{"any_of":[{"item":"wand_frost","identity_group":1},
                {"item":"wand_disintegration"}]},
               {"kind":"wand","identity_group":1}]"#,
        );
        let matches = |items: &[ItemId]| {
            query.matches(&scout_world(
                items.iter().map(|&item| found(item, 3)).collect(),
            ))
        };
        assert!(matches(&[Disintegration]));
        assert!(matches(&[Frost, Frost]));
        assert!(matches(&[Frost, Disintegration]));
        assert!(!matches(&[Frost]));
        // The copy is a copy of the matched Frost, not any wand.
        assert!(!matches(&[Frost, ItemId::WandLightning]));
        assert!(!matches(&[]));
    }

    #[test]
    fn members_may_carry_stacks_of_their_own() {
        use ItemId::{WandDisintegration as Disintegration, WandFrost as Frost};
        // {Frost ×2 | Disintegration ×3}.
        let query = stack_query(
            r#"[{"any_of":[{"item":"wand_frost","identity_group":1},
                {"item":"wand_disintegration","identity_group":2}]},
               {"kind":"wand","identity_group":1},
               {"kind":"wand","identity_group":2},{"kind":"wand","identity_group":2}]"#,
        );
        let matches = |items: &[ItemId]| {
            query.matches(&scout_world(
                items.iter().map(|&item| found(item, 3)).collect(),
            ))
        };
        assert!(matches(&[Frost, Frost]));
        assert!(matches(&[Disintegration, Disintegration, Disintegration]));
        assert!(matches(&[Disintegration, Frost, Disintegration, Frost]));
        assert!(!matches(&[Disintegration, Disintegration]));
        assert!(!matches(&[Frost, Disintegration, Disintegration]));
    }

    #[test]
    fn a_label_on_every_member_keeps_copies_of_whichever_matched() {
        use ItemId::{WandDisintegration as Disintegration, WandFrost as Frost};
        let query = stack_query(
            r#"[{"any_of":[{"item":"wand_frost","identity_group":1},
                {"item":"wand_disintegration","identity_group":1}]},
               {"kind":"wand","identity_group":1}]"#,
        );
        let matches = |items: &[ItemId]| {
            query.matches(&scout_world(
                items.iter().map(|&item| found(item, 3)).collect(),
            ))
        };
        assert!(super::stack_gates(&query.requirements).is_empty());
        assert!(matches(&[Disintegration, Disintegration]));
        assert!(matches(&[Frost, Frost]));
        assert!(!matches(&[Disintegration]));
        assert!(!matches(&[Frost, Disintegration]));
    }

    #[test]
    fn a_member_stacks_copies_keep_their_own_floor_limits() {
        use ItemId::{WandDisintegration as Disintegration, WandFrost as Frost};
        // The copy must be found by floor 3; Frost itself anywhere.
        let query = stack_query(
            r#"[{"any_of":[{"item":"wand_frost","identity_group":1},
                {"item":"wand_disintegration","max_depth":2}]},
               {"kind":"wand","identity_group":1,"max_depth":3}]"#,
        );
        let matches = |items: &[(ItemId, u8)]| {
            query.matches(&scout_world(
                items
                    .iter()
                    .map(|&(item, depth)| found(item, depth))
                    .collect(),
            ))
        };
        assert!(matches(&[(Frost, 9), (Frost, 3)]));
        assert!(!matches(&[(Frost, 9), (Frost, 4)]));
        // A waived copy's limit asks for nothing.
        assert!(matches(&[(Disintegration, 2)]));
        assert!(!matches(&[(Disintegration, 3), (Frost, 5)]));
    }

    #[test]
    fn a_waived_copy_reserves_no_wand_and_costs_no_resin() {
        use ItemId::{WandDisintegration as Disintegration, WandFrost as Frost};
        let world = |items: &[(ItemId, u8)]| {
            scout_world(
                items
                    .iter()
                    .map(|&(item, upgrade)| upgraded(item, upgrade))
                    .collect(),
            )
        };
        let stack = r#"[{"any_of":[{"item":"wand_frost","identity_group":1},
            {"item":"wand_disintegration"}]},{"kind":"wand","identity_group":1}"#;
        // Disintegration fills the group: both Frosts are surplus, four
        // resin. A copy taking one would leave two.
        let fixed =
            crate::json_query::decode(&format!(r#"{{"arcane_resin":4,"requirements":{stack}]}}"#))
                .unwrap();
        let three = world(&[(Disintegration, 0), (Frost, 0), (Frost, 0)]);
        assert!(fixed.matches(&three));
        assert!(!fixed.matches(&world(&[(Disintegration, 0), (Frost, 0)])));
        // Auto: a +2 Disintegration needs three resin to reach +3, which the
        // two surplus Frosts supply only while the copy is waived.
        let auto = crate::json_query::decode(&format!(
            r#"{{"arcane_resin":"auto","requirements":{stack}]}}"#
        ))
        .unwrap();
        assert!(auto.matches(&world(&[(Disintegration, 2), (Frost, 0), (Frost, 0)])));
        assert!(!auto.matches(&world(&[(Disintegration, 2), (Frost, 0)])));
        // Filled by Frost, the copy is a reforge copy: only the anchor's
        // upgrades are budgeted. A +2 anchor needs three resin, which two
        // +0 donors supply; charging the +0 copy too would need nine.
        assert!(auto.matches(&world(&[(Frost, 2), (Frost, 0), (Frost, 0), (Frost, 0)])));
        // A +1 anchor needs five, one +1 donor short.
        assert!(!auto.matches(&world(&[(Frost, 1), (Frost, 1), (Frost, 1)])));
        // The scout shows the Disintegration kept and both Frosts consumed.
        let marks = scout_matches(&three, &fixed);
        assert_eq!(marks.matched_requirements, marks.total_requirements);
        assert_eq!(marks.matched_indices(), vec![0, 1, 2]);
        assert_eq!(marks.resin_donors, vec![false, true, true]);
    }

    #[test]
    fn scout_marks_count_a_waived_copy_without_marking_an_item() {
        use ItemId::{WandDisintegration as Disintegration, WandFrost as Frost};
        let query = stack_query(
            r#"[{"any_of":[{"item":"wand_frost","identity_group":1},
                {"item":"wand_disintegration"}]},
               {"kind":"wand","identity_group":1},{"kind":"wand","identity_group":1}]"#,
        );
        let marks = |items: &[ItemId]| {
            scout_matches(
                &scout_world(items.iter().map(|&item| found(item, 3)).collect()),
                &query,
            )
        };
        // Disintegration satisfies the group and waives both copies.
        let waived = marks(&[Frost, Disintegration]);
        assert_eq!(waived.total_requirements, 3);
        assert_eq!(waived.matched_requirements, 3);
        assert_eq!(waived.matched_indices(), vec![1]);
        // Two Frosts of three: the group and one copy, both marked.
        let short = marks(&[Frost, Frost]);
        assert_eq!(short.matched_requirements, 2);
        assert_eq!(short.matched_indices(), vec![0, 1]);
        // Either way of filling the group satisfies every condition.
        let full = marks(&[Frost, Frost, Disintegration, Frost]);
        assert_eq!(full.matched_requirements, 3);
        assert!([1, 3].contains(&full.matched_indices().len()));
    }

    #[test]
    fn member_stacks_validate_and_the_invalid_shapes_still_fail() {
        let validate = |requirements: &str| {
            crate::json_query::decode_unvalidated(&format!(r#"{{"requirements":{requirements}}}"#))
                .unwrap()
                .validate()
        };
        // One member's stack, and one per member, are the intended shapes.
        assert_eq!(
            validate(
                r#"[{"any_of":[{"item":"wand_frost","identity_group":1},
                    {"item":"plate_armor"}]},{"kind":"wand","identity_group":1}]"#
            ),
            Ok(())
        );
        assert_eq!(
            validate(
                r#"[{"any_of":[{"item":"wand_frost","identity_group":1},
                    {"item":"plate_armor","identity_group":2}]},
                   {"kind":"wand","identity_group":1},{"kind":"armor","identity_group":2}]"#
            ),
            Ok(())
        );
        // A label still spans one kind: an armor copy of a Frost stack.
        assert_eq!(
            validate(
                r#"[{"any_of":[{"item":"wand_frost","identity_group":1},
                    {"item":"plate_armor"}]},{"kind":"armor","identity_group":1}]"#
            ),
            Err(QueryError::InconsistentIdentityGroup)
        );
        // A copy carrying constraints of its own is a second anchor.
        assert_eq!(
            validate(
                r#"[{"any_of":[{"item":"wand_frost","identity_group":1},
                    {"item":"plate_armor"}]},{"kind":"wand","upgrade":2,"identity_group":1}]"#
            ),
            Err(QueryError::OverconstrainedIdentityGroup)
        );
        // Even a bare member anchors a member stack, so the label may not
        // sit on a second group, nor beside a constrained lone requirement.
        assert_eq!(
            validate(
                r#"[{"any_of":[{"kind":"wand","identity_group":1},{"item":"plate_armor"}]},
                   {"any_of":[{"kind":"wand","identity_group":1},{"item":"sword"}]},
                   {"kind":"wand","identity_group":1}]"#
            ),
            Err(QueryError::OverconstrainedIdentityGroup)
        );
        assert_eq!(
            validate(
                r#"[{"any_of":[{"kind":"wand","identity_group":1},{"item":"plate_armor"}]},
                   {"item":"wand_frost","identity_group":1}]"#
            ),
            Err(QueryError::OverconstrainedIdentityGroup)
        );
        // A combined level on the copies would sum rings that may be waived.
        assert_eq!(
            validate(
                r#"[{"any_of":[{"item":"ring_might","identity_group":1},{"item":"ring_energy"}]},
                   {"kind":"ring","identity_group":1,"level_sum":{"group":1,"at_least":3}},
                   {"kind":"ring","identity_group":1,"level_sum":{"group":1,"at_least":3}}]"#
            ),
            Err(QueryError::OverconstrainedIdentityGroup)
        );
        // It stays a lone stack's own, labelled or not.
        assert_eq!(
            validate(
                r#"[{"kind":"ring","identity_group":1,"level_sum":{"group":1,"at_least":3}},
                   {"kind":"ring","identity_group":1,"level_sum":{"group":1,"at_least":3}}]"#
            ),
            Ok(())
        );
        // A group every member of which carries the label gates nothing:
        // its bare members stay copies, as before.
        assert_eq!(
            validate(
                r#"[{"any_of":[{"kind":"wand","identity_group":1}]},
                   {"item":"wand_frost","identity_group":1}]"#
            ),
            Ok(())
        );
    }

    #[test]
    fn member_stacks_spell_out_as_queries_without_them() {
        use ItemId::{WandDisintegration as Disintegration, WandFrost as Frost};
        // {Frost ×2 | Disintegration ×3 | Lightning}.
        let query = stack_query(
            r#"[{"any_of":[{"item":"wand_frost","identity_group":1},
                {"item":"wand_disintegration","identity_group":2},{"item":"wand_lightning"}]},
               {"kind":"wand","identity_group":1},
               {"kind":"wand","identity_group":2},{"kind":"wand","identity_group":2},
               {"kind":"ring"}]"#,
        );
        assert_eq!(
            super::stack_gates(&query.requirements),
            [None, None, None, Some(1), Some(1), Some(1), None]
        );
        let variants = super::member_stack_variants(&query).unwrap();
        let shapes: Vec<Vec<(Option<ItemId>, Option<u8>)>> = variants
            .iter()
            .map(|variant| {
                variant
                    .requirements
                    .iter()
                    .map(|r| (r.item, r.identity_group))
                    .collect()
            })
            .collect();
        assert_eq!(
            shapes,
            [
                vec![(Some(Frost), Some(1)), (None, Some(1)), (None, None)],
                vec![
                    (Some(Disintegration), Some(2)),
                    (None, Some(2)),
                    (None, Some(2)),
                    (None, None)
                ],
                vec![(Some(ItemId::WandLightning), None), (None, None)],
            ]
        );
        for variant in &variants {
            assert_eq!(variant.validate(), Ok(()));
            assert!(super::member_stack_variants(variant).is_none());
        }
        // A world matches the query exactly when it matches some variant.
        let mut rng = crate::editor::testing::Rng::new(0x57ac_4a11_0f11_ed00);
        let pool = [
            Frost,
            Disintegration,
            ItemId::WandLightning,
            ItemId::RingMight,
        ];
        for _ in 0..256 {
            let items = (0..rng.below(6))
                .map(|_| found(pool[rng.below(pool.len())], 3))
                .collect();
            let world = scout_world(items);
            assert_eq!(
                query.matches(&world),
                variants.iter().any(|variant| variant.matches(&world)),
                "{world:?}"
            );
        }
        // Without member stacks there is nothing to spell out.
        assert!(super::member_stack_variants(&stack_query(r#"[{"kind":"wand"}]"#)).is_none());
    }

    fn plain(kind: ItemKind) -> Requirement {
        Requirement {
            kind,
            weapon_category: None,
            item: None,
            tier: TierRequirement::Any,
            upgrade: UpgradeRequirement::Any,
            effect: EffectRequirement::Any,
            require_uncursed: false,
            select_trinket: false,
            trinket_transmutations: 0,
            artifact_transmutations: 0,
            blanket: false,
            exclude_resin: false,
            source: None,
            identity_group: None,
            max_depth: None,
            alternative_group: None,
            level_sum: None,
        }
    }

    fn upgraded(item: ItemId, upgrade: u8) -> WorldItem {
        WorldItem {
            upgrade,
            ..world_item(item, Accessibility::Independent)
        }
    }

    #[test]
    fn effect_sets_hold_one_family_and_match_any_member() {
        let blocking = Effect::Weapon(WeaponEffect::Blocking);
        let grim = Effect::Weapon(WeaponEffect::Grim);
        let thorns = Effect::Armor(crate::catalog::ArmorEffect::Thorns);
        let set = EffectSet::from_effects([blocking, grim]).unwrap();
        assert_eq!(set.count(), 2);
        assert!(set.contains(blocking) && set.contains(grim));
        assert!(!set.contains(thorns));
        assert_eq!(set.effects().collect::<Vec<_>>(), vec![blocking, grim]);
        assert!(EffectSet::from_effects([blocking, thorns]).is_none());
        assert!(EffectSet::from_effects([]).is_none());

        // Every enchantment but no curse; wands and rings carry none.
        let enchantments = EffectSet::enchantments(ItemKind::Weapon).unwrap();
        assert!(enchantments.contains(blocking));
        assert!(!enchantments.contains(Effect::Weapon(WeaponEffect::Annoying)));
        assert!(EffectSet::enchantments(ItemKind::Wand).is_none());
        assert!(EffectSet::single(Effect::Weapon(WeaponEffect::Annoying)).is_curses_only());
        assert_eq!(set.without_curses(), Some(set));
        assert_eq!(
            EffectSet::single(blocking).intersection(set),
            Some(EffectSet::single(blocking))
        );
        assert!(EffectSet::single(thorns).intersection(set).is_none());
        assert!(EffectSet::single(blocking).is_subset_of(set));
        assert!(!set.is_subset_of(EffectSet::single(blocking)));

        let wanted = Requirement {
            effect: EffectRequirement::OneOf(set),
            ..plain(ItemKind::Weapon)
        };
        let mut sword = world_item(ItemId::Sword, Accessibility::Independent);
        assert!(!wanted.matches(&sword));
        sword.effect = Some(grim);
        assert!(wanted.matches(&sword));
        sword.effect = Some(Effect::Weapon(WeaponEffect::Blazing));
        assert!(!wanted.matches(&sword));

        // Validation: the set's family must be the requirement's, and an
        // uncursed item cannot be limited to curses.
        assert_eq!(
            Requirement {
                effect: EffectRequirement::OneOf(set),
                ..plain(ItemKind::Armor)
            }
            .validate(),
            Err(QueryError::EffectKindMismatch)
        );
        assert_eq!(
            Requirement {
                effect: EffectRequirement::OneOf(set),
                ..plain(ItemKind::Ring)
            }
            .validate(),
            Err(QueryError::EffectKindMismatch)
        );
        assert_eq!(
            Requirement {
                effect: EffectRequirement::OneOf(
                    EffectSet::from_effects([
                        Effect::Weapon(WeaponEffect::Annoying),
                        Effect::Weapon(WeaponEffect::Sacrificial),
                    ])
                    .unwrap()
                ),
                require_uncursed: true,
                select_trinket: false,
                trinket_transmutations: 0,
                artifact_transmutations: 0,
                blanket: false,
                exclude_resin: false,
                ..plain(ItemKind::Weapon)
            }
            .validate(),
            Err(QueryError::UncursedWithCurse)
        );
        assert_eq!(
            Requirement {
                effect: EffectRequirement::OneOf(
                    EffectSet::from_effects([Effect::Weapon(WeaponEffect::Annoying), blocking])
                        .unwrap()
                ),
                require_uncursed: true,
                select_trinket: false,
                trinket_transmutations: 0,
                artifact_transmutations: 0,
                blanket: false,
                exclude_resin: false,
                ..plain(ItemKind::Weapon)
            }
            .validate(),
            Ok(())
        );
    }

    #[test]
    fn alternatives_form_one_slot_any_member_can_fill() {
        let spear = Requirement {
            item: Some(ItemId::Spear),
            upgrade: UpgradeRequirement::Exact(3),
            alternative_group: Some(1),
            ..plain(ItemKind::Weapon)
        };
        let shuriken = Requirement {
            item: Some(ItemId::Shuriken),
            upgrade: UpgradeRequirement::Exact(2),
            alternative_group: Some(1),
            ..plain(ItemKind::Weapon)
        };
        let sword = Requirement {
            item: Some(ItemId::Sword),
            upgrade: UpgradeRequirement::Exact(1),
            alternative_group: Some(1),
            ..plain(ItemKind::Weapon)
        };
        let query = SearchQuery {
            requirements: vec![spear, shuriken, sword, plain(ItemKind::Wand)],
            ..scout_query(Vec::new())
        };
        assert_eq!(query.validate(), Ok(()));
        assert_eq!(query.slots(), vec![vec![0, 1, 2], vec![3]]);
        assert_eq!(query.slot_count(), 2);

        // Three members but one slot: two items suffice.
        let wand = upgraded(ItemId::WandFrost, 0);
        assert!(query.matches(&scout_world(vec![upgraded(ItemId::Sword, 1), wand.clone()])));
        assert!(query.matches(&scout_world(vec![
            upgraded(ItemId::Shuriken, 2),
            wand.clone()
        ])));
        assert!(!query.matches(&scout_world(vec![upgraded(ItemId::Sword, 2), wand])));
        assert!(!query.matches(&scout_world(vec![upgraded(ItemId::Sword, 1)])));
        // One item cannot serve the slot and another requirement at once.
        let two_swords = SearchQuery {
            requirements: vec![
                spear,
                sword,
                Requirement {
                    item: Some(ItemId::Sword),
                    ..plain(ItemKind::Weapon)
                },
            ],
            ..scout_query(Vec::new())
        };
        assert!(!two_swords.matches(&scout_world(vec![upgraded(ItemId::Sword, 1)])));
        assert!(two_swords.matches(&scout_world(vec![
            upgraded(ItemId::Sword, 1),
            upgraded(ItemId::Sword, 0)
        ])));

        // The scout counts the group as one requirement.
        let marks = scout_matches(&scout_world(vec![upgraded(ItemId::Sword, 1)]), &query);
        assert_eq!(marks.total_requirements, 2);
        assert_eq!(marks.matched_requirements, 1);
        assert_eq!(marks.matched_indices(), vec![0]);

        // Group zero is reserved, like identity group zero.
        assert_eq!(
            Requirement {
                alternative_group: Some(0),
                ..plain(ItemKind::Wand)
            }
            .validate(),
            Err(QueryError::InvalidAlternativeGroup)
        );
        // Alternatives of one slot may disagree inside an identity group —
        // only one of them is ever assigned — but members of different slots
        // must agree.
        let linked = |item, alternative_group| Requirement {
            item: Some(item),
            identity_group: Some(1),
            alternative_group,
            ..plain(ItemKind::Ring)
        };
        assert_eq!(
            SearchQuery {
                requirements: vec![
                    linked(ItemId::RingMight, Some(1)),
                    linked(ItemId::RingHaste, Some(1)),
                ],
                ..scout_query(Vec::new())
            }
            .validate(),
            Ok(())
        );
        assert_eq!(
            SearchQuery {
                requirements: vec![
                    linked(ItemId::RingMight, Some(1)),
                    linked(ItemId::RingHaste, None),
                ],
                ..scout_query(Vec::new())
            }
            .validate(),
            Err(QueryError::OverconstrainedIdentityGroup)
        );
    }

    #[test]
    fn combined_level_groups_sum_distinct_items_and_members_are_optional() {
        let might = |level_sum| Requirement {
            item: Some(ItemId::RingMight),
            level_sum: Some(level_sum),
            ..plain(ItemKind::Ring)
        };
        let pair = |minimum_total| SearchQuery {
            requirements: vec![
                might(LevelSum {
                    group: 1,
                    minimum_total,
                });
                2
            ],
            ..scout_query(Vec::new())
        };
        let rings = |upgrades: &[u8]| {
            scout_world(
                upgrades
                    .iter()
                    .map(|upgrade| upgraded(ItemId::RingMight, *upgrade))
                    .collect(),
            )
        };
        // +3 strength from Rings of Might: one +2 ring, or a +0 and a +1.
        assert!(pair(3).matches(&rings(&[2])));
        assert!(pair(3).matches(&rings(&[0, 1])));
        assert!(!pair(3).matches(&rings(&[1])));
        assert!(!pair(3).matches(&rings(&[0])));
        // Distinct items only: a pair cannot count one ring twice.
        assert!(pair(8).matches(&rings(&[3, 3])));
        assert!(!pair(8).matches(&rings(&[3, 2])));
        assert!(!pair(8).matches(&rings(&[4])));
        // Backtracking over assignments: the +0 and +1 pair falls short,
        // the +1 and +3 pair carries it.
        assert!(pair(6).matches(&rings(&[0, 1, 3])));
        assert!(!pair(7).matches(&rings(&[0, 1, 3])));

        // The scout counts the whole group as one condition and flags every
        // contributing item once the total is met.
        let met = scout_matches(&rings(&[1, 3]), &pair(6));
        assert_eq!(met.total_requirements, 1);
        assert_eq!(met.matched_requirements, 1);
        assert_eq!(met.matched_indices(), vec![0, 1]);
        let lone = scout_matches(&rings(&[3]), &pair(4));
        assert_eq!(lone.matched_requirements, 1);
        assert_eq!(lone.matched_indices(), vec![0]);
        let short = scout_matches(&rings(&[1]), &pair(4));
        assert_eq!(short.matched_requirements, 0);
        assert!(short.matched_indices().is_empty());
    }

    #[test]
    fn combined_level_validation_caps_totals_and_admits_rings_only() {
        let might = |level_sum| Requirement {
            item: Some(ItemId::RingMight),
            level_sum: Some(level_sum),
            ..plain(ItemKind::Ring)
        };
        let pair = |minimum_total| SearchQuery {
            requirements: vec![
                might(LevelSum {
                    group: 1,
                    minimum_total,
                });
                2
            ],
            ..scout_query(Vec::new())
        };
        // A ring reaches +4 (five levels), but only one per world — the Imp
        // vault's prize; every other ring stops at +2 (three levels). Two
        // rings therefore reach eight levels together, not ten.
        assert_eq!(pair(3).validate(), Ok(()));
        assert_eq!(pair(8).validate(), Ok(()));
        assert_eq!(
            pair(9).validate(),
            Err(QueryError::UnattainableLevelSum {
                group: 1,
                minimum_total: 9,
                capacity: 8,
            })
        );
        assert_eq!(
            pair(9).validate().unwrap_err().to_string(),
            "combined level group A needs 9 levels but its items can reach at most 8"
        );
        // Only rings count levels together.
        assert_eq!(
            Requirement {
                item: Some(ItemId::Sword),
                level_sum: Some(LevelSum {
                    group: 1,
                    minimum_total: 3,
                }),
                ..plain(ItemKind::Weapon)
            }
            .validate(),
            Err(QueryError::LevelSumOutsideRings)
        );

        // Members agree on the total, sums need a group and a total, and a
        // sum cannot live inside an alternative group.
        assert_eq!(
            SearchQuery {
                requirements: vec![
                    might(LevelSum {
                        group: 1,
                        minimum_total: 2
                    }),
                    might(LevelSum {
                        group: 1,
                        minimum_total: 3
                    }),
                ],
                ..scout_query(Vec::new())
            }
            .validate(),
            Err(QueryError::InconsistentLevelSum { group: 1 })
        );
        assert_eq!(
            might(LevelSum {
                group: 0,
                minimum_total: 2
            })
            .validate(),
            Err(QueryError::InvalidLevelSum)
        );
        assert_eq!(
            might(LevelSum {
                group: 1,
                minimum_total: 0
            })
            .validate(),
            Err(QueryError::InvalidLevelSum)
        );
        assert_eq!(
            Requirement {
                alternative_group: Some(1),
                ..might(LevelSum {
                    group: 1,
                    minimum_total: 1
                })
            }
            .validate(),
            Err(QueryError::LevelSumInsideAlternative)
        );
    }

    fn scout_query(requirements: Vec<Requirement>) -> SearchQuery {
        SearchQuery {
            floor_requirements: Vec::new(),
            auto_apply_trinket: false,
            arcane_resin_filter: crate::query::ArcaneResinFilter::default(),
            arcane_resin_auto: false,
            arcane_resin: 0,
            requirements,
            max_depth: 24,
            challenges: crate::challenges::Challenges::NONE,
            require_blacksmith: false,
            exclude_blacksmith_rewards: false,
            wandmaker_quest: None,
        }
    }

    fn scout_world(items: Vec<WorldItem>) -> GeneratedWorld {
        GeneratedWorld {
            floor_rooms: Vec::new(),
            artifact_decks: Vec::new(),
            feelings: Vec::new(),
            quests: crate::quests::QuestSummary::default(),
            seed: DungeonSeed::MIN,
            items,
            ring_gems: RingGems::UNSHUFFLED,
        }
    }

    fn any_requirement(kind: ItemKind) -> Requirement {
        Requirement {
            kind,
            weapon_category: None,
            item: None,
            tier: TierRequirement::Any,
            upgrade: UpgradeRequirement::Any,
            effect: EffectRequirement::Any,
            require_uncursed: false,
            select_trinket: false,
            trinket_transmutations: 0,
            artifact_transmutations: 0,
            blanket: false,
            exclude_resin: false,
            source: None,
            identity_group: None,
            max_depth: None,
            alternative_group: None,
            level_sum: None,
        }
    }

    #[test]
    fn scout_marks_the_largest_satisfiable_selection() {
        let world = scout_world(vec![
            world_item(ItemId::Sword, Accessibility::Independent),
            world_item(ItemId::WandFrost, Accessibility::Independent),
        ]);

        // Two swords wanted, one present: the marks explain the requirement
        // that can be satisfied instead of reporting nothing at all.
        let query = scout_query(vec![requirement(ItemId::Sword), requirement(ItemId::Sword)]);
        assert!(!query.matches(&world));
        let marks = scout_matches(&world, &query);
        assert_eq!(marks.matched, vec![true, false]);
        assert_eq!(marks.matched_indices(), vec![0]);
        assert_eq!(marks.matched_requirements, 1);
        assert_eq!(marks.total_requirements, 2);

        // Every requirement satisfied marks every item it claimed.
        let query = scout_query(vec![
            requirement(ItemId::Sword),
            requirement(ItemId::WandFrost),
        ]);
        let marks = scout_matches(&world, &query);
        assert_eq!(marks.matched, vec![true, true]);
        assert_eq!(marks.matched_requirements, 2);
        assert_eq!(marks.total_requirements, 2);

        // Nothing matching marks nothing.
        let marks = scout_matches(
            &world,
            &scout_query(vec![requirement(ItemId::WandLightning)]),
        );
        assert_eq!(marks.matched, vec![false, false]);
        assert_eq!(marks.matched_requirements, 0);
        assert_eq!(marks.total_requirements, 1);
    }

    #[test]
    fn scout_marks_bind_identity_groups_to_one_item() {
        let linked = Requirement {
            identity_group: Some(1),
            ..any_requirement(ItemKind::Wand)
        };
        let query = scout_query(vec![linked, linked]);

        // Two different wands cannot both answer a linked pair: the group
        // binds the second requirement to the first's item.
        let mixed = scout_world(vec![
            world_item(ItemId::WandFrost, Accessibility::Independent),
            world_item(ItemId::WandLightning, Accessibility::Independent),
        ]);
        assert!(!query.matches(&mixed));
        let marks = scout_matches(&mixed, &query);
        assert_eq!(marks.matched_requirements, 1);
        assert_eq!(marks.matched_indices().len(), 1);

        // Two copies of one wand satisfy both.
        let paired = scout_world(vec![
            world_item(ItemId::WandFrost, Accessibility::Independent),
            world_item(ItemId::WandFrost, Accessibility::Independent),
        ]);
        assert!(query.matches(&paired));
        assert_eq!(scout_matches(&paired, &query).matched, vec![true, true]);
    }

    #[test]
    fn scout_marks_respect_accessibility_scenarios() {
        let query = scout_query(vec![requirement(ItemId::Sword), requirement(ItemId::Sword)]);

        // Two swords on mutually exclusive acquisition plans of one group:
        // only one of them is ever obtainable, so only one is marked.
        let exclusive = scout_world(vec![
            world_item(
                ItemId::Sword,
                Accessibility::Scenarios {
                    group: 1,
                    mask: 0b01,
                },
            ),
            world_item(
                ItemId::Sword,
                Accessibility::Scenarios {
                    group: 1,
                    mask: 0b10,
                },
            ),
        ]);
        assert!(!query.matches(&exclusive));
        assert_eq!(scout_matches(&exclusive, &query).matched_requirements, 1);

        // A shared plan lets both count.
        let compatible = scout_world(vec![
            world_item(
                ItemId::Sword,
                Accessibility::Scenarios {
                    group: 1,
                    mask: 0b11,
                },
            ),
            world_item(
                ItemId::Sword,
                Accessibility::Scenarios {
                    group: 1,
                    mask: 0b10,
                },
            ),
        ]);
        assert!(query.matches(&compatible));
        assert_eq!(scout_matches(&compatible, &query).matched, vec![true, true]);
    }

    #[test]
    fn scout_marks_honour_floor_limits_and_the_blacksmith_exclusion() {
        let world = scout_world(vec![
            WorldItem {
                depth: 5,
                source: ItemSource::BlacksmithReward,
                ..world_item(ItemId::Sword, Accessibility::Independent)
            },
            WorldItem {
                depth: 9,
                ..world_item(ItemId::Sword, Accessibility::Independent)
            },
        ]);
        let mut query = scout_query(vec![requirement(ItemId::Sword)]);
        assert_eq!(scout_matches(&world, &query).matched_indices(), vec![0]);

        // The query's own floor limit hides the deeper copy, then both.
        query.max_depth = 5;
        assert_eq!(scout_matches(&world, &query).matched_indices(), vec![0]);
        query.max_depth = 4;
        assert_eq!(scout_matches(&world, &query).matched_requirements, 0);

        // A per-requirement limit narrows the same way on its own.
        query.max_depth = 24;
        query.requirements[0].max_depth = Some(8);
        assert_eq!(scout_matches(&world, &query).matched_indices(), vec![0]);
        query.requirements[0].max_depth = Some(4);
        assert_eq!(scout_matches(&world, &query).matched_requirements, 0);

        // Excluding Smith rewards drops the shallow copy for the deep one.
        query.requirements[0].max_depth = None;
        query.exclude_blacksmith_rewards = true;
        assert_eq!(scout_matches(&world, &query).matched_indices(), vec![1]);
    }

    #[test]
    fn scout_marks_agree_with_the_matcher_on_scouted_seeds() {
        let linked = |kind| Requirement {
            identity_group: Some(1),
            ..any_requirement(kind)
        };
        let mut shallow = scout_query(vec![any_requirement(ItemKind::Ring)]);
        shallow.max_depth = 6;
        let queries = [
            scout_query(vec![any_requirement(ItemKind::Ring)]),
            scout_query(vec![
                any_requirement(ItemKind::Wand),
                any_requirement(ItemKind::Wand),
                any_requirement(ItemKind::Wand),
            ]),
            scout_query(vec![
                requirement(ItemId::Sword),
                any_requirement(ItemKind::Armor),
            ]),
            scout_query(vec![linked(ItemKind::Ring), linked(ItemKind::Ring)]),
            scout_query(vec![Requirement {
                upgrade: UpgradeRequirement::AtLeast(3),
                ..any_requirement(ItemKind::Weapon)
            }]),
            shallow,
        ];

        let (mut satisfied, mut unsatisfied) = (0, 0);
        for value in [0_u64, 1, 7, 99] {
            let seed = DungeonSeed::new(value).unwrap();
            let world = crate::main_world::generate_main_world(seed, 12).unwrap();
            for query in &queries {
                let marks = scout_matches(&world, query);
                assert_eq!(marks.total_requirements, query.requirements.len());
                assert_eq!(marks.matched_indices().len(), marks.matched_requirements);
                assert_eq!(marks.matched.len(), world.items.len());
                // A full selection is exactly what the search matcher accepts.
                let complete = marks.matched_requirements == marks.total_requirements;
                assert_eq!(complete, query.matches(&world), "seed {value}");
                if complete {
                    satisfied += 1;
                } else {
                    unsatisfied += 1;
                }
            }
        }
        // Both outcomes must occur, or the agreement above proves nothing.
        assert!(satisfied > 0, "no query was fully satisfied");
        assert!(unsatisfied > 0, "every query was fully satisfied");
    }

    /// [`SearchQuery::validate`]'s requirement checks as they stood before
    /// the group checks moved into [`super::requirement_group_errors`], kept
    /// verbatim as the reference the shared helper must not drift from.
    fn validate_before_the_group_helper(requirements: &[Requirement]) -> Result<(), QueryError> {
        use std::collections::BTreeMap;
        type IdentityMember = (usize, Option<u8>, ItemKind, bool);
        let query = crate::editor::testing::query(requirements.to_vec());
        let mut identity_groups: BTreeMap<u8, Vec<IdentityMember>> = BTreeMap::new();
        let mut level_sums: BTreeMap<u8, u8> = BTreeMap::new();
        for (index, requirement) in requirements.iter().enumerate() {
            requirement.validate()?;
            if let Some(group) = requirement.identity_group {
                identity_groups.entry(group).or_default().push((
                    index,
                    requirement.alternative_group,
                    requirement.kind,
                    requirement.is_bare(),
                ));
            }
            if let Some(sum) = requirement.level_sum {
                let agreed = level_sums.entry(sum.group).or_insert(sum.minimum_total);
                if *agreed != sum.minimum_total {
                    return Err(QueryError::InconsistentLevelSum { group: sum.group });
                }
            }
        }
        for slot in query.slots() {
            if slot
                .iter()
                .any(|&index| requirements[index].blanket != requirements[slot[0]].blanket)
            {
                return Err(QueryError::MixedBlanketAlternatives);
            }
        }
        for (&label, members) in &identity_groups {
            let (_, _, first_kind, _) = members[0];
            if members.iter().any(|&(_, _, kind, _)| kind != first_kind) {
                return Err(QueryError::InconsistentIdentityGroup);
            }
            // A member stack: some member of an alternative group carries
            // the label and another does not. Its members anchor it.
            let member_stack = members.iter().any(|&(_, alternative, _, _)| {
                alternative.is_some_and(|group| {
                    requirements.iter().any(|other| {
                        other.alternative_group == Some(group)
                            && other.identity_group != Some(label)
                    })
                })
            });
            let mut anchor: Option<(Option<u8>, usize)> = None;
            for &(index, alternative, _, bare) in members {
                let bare = bare && !(member_stack && requirements[index].level_sum.is_some());
                if bare && !(member_stack && alternative.is_some()) {
                    continue;
                }
                let unit = alternative.map_or((None, index), |group| (Some(group), 0));
                if *anchor.get_or_insert(unit) != unit {
                    return Err(QueryError::OverconstrainedIdentityGroup);
                }
            }
        }
        for (group, summary) in query.level_sum_groups() {
            let attainable = summary.attainable_capacity();
            if summary.minimum_total > attainable {
                return Err(QueryError::UnattainableLevelSum {
                    group,
                    minimum_total: summary.minimum_total,
                    capacity: attainable,
                });
            }
        }
        Ok(())
    }

    #[test]
    fn the_shared_group_checks_keep_validations_first_error() {
        use crate::editor::testing::{Rng, mixed_rows, query};

        let ring = |total: u8| Requirement {
            item: Some(ItemId::RingMight),
            level_sum: Some(LevelSum {
                group: 1,
                minimum_total: total,
            }),
            ..Requirement::any(ItemKind::Ring)
        };
        let broken = Requirement {
            max_depth: Some(40),
            ..Requirement::any(ItemKind::Wand)
        };
        // A disagreeing total is met while walking the rows: it outranks a
        // broken row after it, and a broken row before it outranks it.
        assert_eq!(
            query(vec![ring(2), ring(3), broken]).validate(),
            Err(QueryError::InconsistentLevelSum { group: 1 })
        );
        assert_eq!(
            query(vec![ring(2), broken, ring(3)]).validate(),
            Err(QueryError::InvalidDepth)
        );
        // Every other group check waits for every row.
        let spear = Requirement {
            item: Some(ItemId::Spear),
            identity_group: Some(1),
            ..Requirement::any(ItemKind::Weapon)
        };
        let mace = Requirement {
            item: Some(ItemId::Mace),
            ..spear
        };
        assert_eq!(
            query(vec![spear, mace, broken]).validate(),
            Err(QueryError::InvalidDepth)
        );
        assert_eq!(
            query(vec![spear, mace]).validate(),
            Err(QueryError::OverconstrainedIdentityGroup)
        );
        // All of them are reported, in validation's order, each with the
        // rows of its group.
        let blanket = Requirement {
            alternative_group: Some(3),
            blanket: true,
            ..Requirement::any(ItemKind::Wand)
        };
        let ordinary = Requirement {
            alternative_group: Some(3),
            ..Requirement::any(ItemKind::Wand)
        };
        let unreachable = |total: u8| Requirement {
            level_sum: Some(LevelSum {
                group: 2,
                minimum_total: total,
            }),
            ..Requirement::any(ItemKind::Ring)
        };
        let list = [
            spear,
            unreachable(9),
            blanket,
            ring(2),
            mace,
            ordinary,
            unreachable(9),
            ring(4),
        ];
        assert_eq!(
            super::requirement_group_errors(&list),
            [
                (QueryError::InconsistentLevelSum { group: 1 }, vec![3, 7]),
                (QueryError::MixedBlanketAlternatives, vec![2, 5]),
                (QueryError::OverconstrainedIdentityGroup, vec![0, 4]),
                (
                    QueryError::UnattainableLevelSum {
                        group: 2,
                        minimum_total: 9,
                        capacity: 8,
                    },
                    vec![1, 6],
                ),
            ]
        );

        // And over generated lists — valid rows, broken rows, groups that
        // agree and groups that do not — the first error never moved
        // (1,024 cases).
        let mut generator = Rng::new(0x0ddb_a115_eed0);
        let mut failures = 0;
        for case in 0..1024 {
            let rows = mixed_rows(&mut generator);
            let requirements: Vec<Requirement> = rows.iter().map(|row| row.requirement).collect();
            let expected = if requirements.iter().all(|requirement| requirement.blanket) {
                Err(QueryError::Empty)
            } else {
                validate_before_the_group_helper(&requirements)
            };
            failures += usize::from(expected.is_err());
            assert_eq!(
                query(requirements.clone()).validate(),
                expected,
                "case {case}: {requirements:?}"
            );
        }
        // Both outcomes occur, or the agreement proves little.
        assert!((100..900).contains(&failures), "{failures} failures");
    }
}
