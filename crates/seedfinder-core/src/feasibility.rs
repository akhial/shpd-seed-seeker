//! Logic-based query feasibility: which sources can satisfy each requirement,
//! how deep generation must run, and when a partially generated seed can be
//! abandoned early.
//!
//! The rules here mirror structural facts of the v4.0.0 generator:
//!
//! - Natural equipment rolls never exceed +2 ([`crate::equipment`]), so +3
//!   weapons come only from the Sacrificial-fire room, the Ghost quest, the
//!   Blacksmith, a special-room chest prize (the flooded-vault and sentry
//!   rooms bump a weapon, missile, or armor roll by one, and the secret maze
//!   bumps a melee weapon or armor roll — all dropped as chests), or the Imp;
//!   +3 armor only from the Crypt, the Ghost, the Blacksmith, those same
//!   chest prizes, or the Imp; +3 wands only from the Wandmaker or the Imp;
//!   and +3 rings only from the Imp.
//! - Everything above +3 is the Imp's alone. On its City floor the quest
//!   rolls six prizes ([`ItemSource::ImpReward`]: an artifact or ring, a
//!   ring, a tier-5 weapon with a tier-4 thrown weapon or the reverse, plate
//!   armor, and a wand — +2..+4, the tier-4 weapons +3..+5, never cursed,
//!   every weapon and armor carrying a good enchantment or glyph) and opens a
//!   vault whose treasure rooms hold [`ItemSource::VaultTreasure`] equipment
//!   (+0..+3 plus one +4 tier-4 melee weapon, never cursed, effects good or
//!   absent). They are the only sources of +4 armor, wands and rings and of
//!   +4/+5 weapons, and because the player carries exactly one item out of
//!   the vault, both sources together are one mutually exclusive choice —
//!   one [`Quest::Imp`] pick.
//! - Ordinary equipment follows the pinned floor-set tier distribution: tier
//!   two ends at depth 9 and tier three at 19. Shop and vault inventories are
//!   separate; both can still supply low-tier equipment after those limits.
//! - Every quest resolves inside a fixed depth window (Ghost 2–4, Wandmaker
//!   7–9, Blacksmith 12–14, Imp 17–19) and spawns at most once per run, with
//!   the spawn forced on the window's final floor.
//! - Shops stock unupgraded, unenchanted items only, and quest reward choices
//!   are mutually exclusive, so each quest satisfies at most one requirement.
//!
//! Everything derived from these rules is exact: a rejected seed can never
//! match, and a shortened generation depth can never hide a match.
//!
//! The searchable catalog contains equipment only. `NO_SCROLLS` halves the
//! scheduled Scroll of Upgrade drops, but no current requirement can target a
//! consumable or torch, so there is no challenge-dependent availability bound
//! to apply here. Its RNG knock-on effects are handled by generation itself.

use crate::catalog::{ItemId, ItemKind, WeaponCategory};
use crate::model::{ItemSource, WorldItem};
use crate::query::{EffectRequirement, Requirement, SearchQuery, UpgradeRequirement};
use crate::quests::{QuestSummary, WandmakerQuestType};
use crate::search::FloorGate;

/// The four one-per-run reward quests, each offering a mutually exclusive
/// choice, so each can satisfy at most one requirement.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum Quest {
    Ghost,
    Wandmaker,
    Blacksmith,
    Imp,
}

/// Every reward quest, in dungeon order.
pub const QUESTS: [Quest; 4] = [
    Quest::Ghost,
    Quest::Wandmaker,
    Quest::Blacksmith,
    Quest::Imp,
];

impl Quest {
    /// The inclusive depth window inside which the quest can first spawn. The
    /// spawn chance reaches certainty on the final floor, so a run whose item
    /// list has no reward items past the window can never gain them.
    #[must_use]
    pub const fn window(self) -> (u8, u8) {
        match self {
            Self::Ghost => (2, 4),
            Self::Wandmaker => (7, 9),
            Self::Blacksmith => (12, 14),
            Self::Imp => (17, 19),
        }
    }

    const fn bit(self) -> u8 {
        1 << (self as u8)
    }
}

const fn quest_for_source(source: ItemSource) -> Option<Quest> {
    match source {
        ItemSource::GhostReward => Some(Quest::Ghost),
        ItemSource::WandmakerReward => Some(Quest::Wandmaker),
        ItemSource::BlacksmithReward => Some(Quest::Blacksmith),
        // Both vault sources are one pick: the Imp lets exactly one item leave.
        ItemSource::ImpReward | ItemSource::VaultTreasure => Some(Quest::Imp),
        _ => None,
    }
}

/// What `effect` values a source's items can carry.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum EffectPolicy {
    /// Items never carry an enchantment or glyph (shops, wands, rings).
    Never,
    /// Curse-type effects are stripped or replaced; only good effects survive.
    GoodOnly,
    /// The full natural distribution, including curse effects.
    Any,
}

/// Static per-source capabilities for one item kind: the reachable upgrade
/// interval and the effect policy. `None` when the source cannot produce the
/// kind at all.
const fn source_profile(
    source: ItemSource,
    kind: ItemKind,
    weapon_category: Option<WeaponCategory>,
) -> Option<(u8, u8, EffectPolicy)> {
    use EffectPolicy::{Any, GoodOnly, Never};
    use ItemKind::{Armor, Ring, Wand, Weapon};
    use ItemSource as S;
    if matches!(kind, ItemKind::Artifact) {
        return match source {
            S::ImpReward => Some((5, 5, Never)),
            S::Shop
            | S::Heap
            | S::Chest
            | S::LockedChest
            | S::CrystalChest
            | S::Tomb
            | S::Skeleton
            | S::Mimic
            | S::GoldenMimic
            | S::CrystalMimic => Some((0, 0, Never)),
            _ => None,
        };
    }
    if matches!(kind, ItemKind::Trinket) {
        return match source {
            S::Heap
            | S::Chest
            | S::LockedChest
            | S::CrystalChest
            | S::Tomb
            | S::Skeleton
            | S::Mimic
            | S::GoldenMimic
            | S::CrystalMimic => Some((0, 0, Never)),
            _ => None,
        };
    }
    // The Ghost and Sacrificial-fire prizes and both statue drops roll
    // exclusively melee weapons, so a thrown-narrowed weapon
    // requirement can never be satisfied by them. Every thrown-capable
    // source also rolls melee weapons, so a melee filter removes nothing.
    if matches!(kind, Weapon)
        && matches!(weapon_category, Some(WeaponCategory::Thrown))
        && matches!(
            source,
            S::GhostReward | S::SacrificialFire | S::Statue | S::ArmoredStatue
        )
    {
        return None;
    }
    Some(match (source, kind) {
        // Rare +3 rolls outside the quests: the Crypt bumps non-cursed
        // armor, the Sacrificial fire bumps its melee prize, and plain
        // chests front the flooded-vault, sentry, and secret-maze prizes,
        // each bumping one natural weapon/missile/armor roll.
        (S::Chest, Weapon | Armor) | (S::Tomb, Armor) | (S::SacrificialFire, Weapon) => (0, 3, Any),
        // Plain drops and chest variants use the natural rolls, capped at +2,
        // as do crystal chests/mimics (which stock only wands and rings).
        (S::Heap | S::Chest | S::LockedChest | S::Skeleton | S::Mimic, _)
        | (S::CrystalChest | S::CrystalMimic, Wand | Ring) => (0, 2, Any),
        // Golden mimics strip curse effects; statues force a good effect.
        // Neither exceeds the natural +2 cap.
        (S::GoldenMimic, _) | (S::Statue, Weapon) | (S::ArmoredStatue, Weapon | Armor) => {
            (0, 2, GoodOnly)
        }
        // Shop stock is always +0 with no effect.
        (S::Shop, _) => (0, 0, Never),
        // Quest rewards. The vault's treasure armor shares the Ghost's and
        // Blacksmith's profile: +0..+3, never cursed, effects good or absent.
        (S::GhostReward | S::BlacksmithReward, Weapon | Armor) | (S::VaultTreasure, Armor) => {
            (0, 3, GoodOnly)
        }
        (S::WandmakerReward, Wand) => (1, 3, Never),
        // The Imp's final-room options (v4.0.0): every weapon, thrown weapon and
        // the plate armor carry a good effect, wands and rings carry none,
        // nothing is cursed. Tier-4 weapons and thrown weapons roll +3..+5,
        // everything else +2..+4.
        (S::ImpReward, Weapon) => (2, 5, GoodOnly),
        (S::ImpReward, Armor) => (2, 4, GoodOnly),
        (S::ImpReward, Wand | Ring) => (2, 4, Never),
        // Vault treasure rooms: four loot tiers at +0..+3, plus one +4 tier-4
        // melee weapon; effects are good or absent, never curses (the armor
        // arm sits with the Ghost's and Blacksmith's above).
        (S::VaultTreasure, Weapon) => (0, 4, GoodOnly),
        (S::VaultTreasure, Wand | Ring) => (0, 3, Never),
        _ => return None,
    })
}

const fn upgrade_reachable(requirement: UpgradeRequirement, low: u8, high: u8) -> bool {
    match requirement {
        UpgradeRequirement::Any => true,
        UpgradeRequirement::Exact(wanted) => low <= wanted && wanted <= high,
        UpgradeRequirement::AtLeast(minimum) => minimum <= high,
    }
}

fn effect_reachable(
    wanted: EffectRequirement,
    policy: EffectPolicy,
    require_uncursed: bool,
) -> bool {
    let EffectRequirement::OneOf(set) = wanted else {
        return true;
    };
    // Uncursed items never carry curse-type effects, so only the good
    // members of the set stay reachable under that flag.
    let effective = if require_uncursed {
        match set.without_curses() {
            Some(set) => set,
            None => return false,
        }
    } else {
        set
    };
    match policy {
        EffectPolicy::Never => false,
        EffectPolicy::GoodOnly => !effective.is_curses_only(),
        EffectPolicy::Any => true,
    }
}

/// Whether a tier can satisfy both the named identity and the tier filter.
fn can_match_tier(requirement: &Requirement, tier: u8) -> bool {
    requirement.tier.matches(Some(tier))
        && requirement
            .item
            .is_none_or(|id| crate::catalog::item(id).tier == Some(tier))
}

/// Exact equipment rows assigned by `VaultEquipmentLoot::setup_tier`.
/// Keep disjoint upgrade choices separate: their gaps are not intervals.
fn vault_inventory_reachable(requirement: &Requirement) -> bool {
    const MELEE: &[(u8, u8)] = &[(2, 0), (2, 2), (3, 1), (3, 3), (4, 2), (4, 4), (5, 3)];
    const THROWN_OR_ARMOR: &[(u8, u8)] = &[(2, 0), (3, 1), (4, 2), (5, 3)];
    let row_matches = |&(tier, upgrade): &(u8, u8)| {
        can_match_tier(requirement, tier)
            && upgrade_reachable(requirement.upgrade, upgrade, upgrade)
            && effect_reachable(
                requirement.effect,
                if upgrade == 0 {
                    EffectPolicy::Never
                } else {
                    EffectPolicy::GoodOnly
                },
                requirement.require_uncursed,
            )
    };
    match requirement.kind {
        ItemKind::Weapon => {
            let category_matches = |category| {
                requirement
                    .weapon_category
                    .is_none_or(|wanted| wanted == category)
                    && requirement
                        .item
                        .is_none_or(|id| id.weapon_category() == Some(category))
            };
            (category_matches(WeaponCategory::Melee) && MELEE.iter().any(row_matches))
                || (category_matches(WeaponCategory::Thrown)
                    && THROWN_OR_ARMOR.iter().any(row_matches))
        }
        ItemKind::Armor => THROWN_OR_ARMOR.iter().any(row_matches),
        _ => true,
    }
}

/// Whether the pinned floor-set distribution can select a matching tier.
fn floor_set_supports_requirement(requirement: &Requirement, weights: &[f32; 5]) -> bool {
    weights
        .iter()
        .zip(1_u8..=5)
        .any(|(&weight, tier)| weight > 0.0 && can_match_tier(requirement, tier))
}

/// Last depth at which this source can first supply a matching item.
/// Shop stock and quest inventories have their own generation rules; only
/// ordinary sources share the current-or-higher floor-set tier distribution.
fn source_generation_deadline(
    requirement: &Requirement,
    source: ItemSource,
    limit: u8,
) -> Option<u8> {
    if requirement.level_sum.is_some()
        || !matches!(requirement.kind, ItemKind::Weapon | ItemKind::Armor)
    {
        return Some(limit);
    }
    let distributions = &crate::generator::FLOOR_SET_TIER_PROBABILITIES;
    match source {
        // Smith rewards explicitly use floor set three for all equipment.
        ItemSource::BlacksmithReward => {
            floor_set_supports_requirement(requirement, &distributions[3]).then_some(limit)
        }
        ItemSource::Heap
        | ItemSource::Chest
        | ItemSource::LockedChest
        | ItemSource::CrystalChest
        | ItemSource::Tomb
        | ItemSource::Skeleton
        | ItemSource::SacrificialFire
        | ItemSource::Mimic
        | ItemSource::GoldenMimic
        | ItemSource::CrystalMimic
        | ItemSource::Statue
        | ItemSource::ArmoredStatue => distributions
            .iter()
            .rposition(|weights| floor_set_supports_requirement(requirement, weights))
            .map(|floor_set| {
                let last_depth = u8::try_from(5 * floor_set + 4)
                    .expect("five floor sets end at depths four through twenty-four");
                limit.min(last_depth)
            }),
        // In particular, tier-two tipped darts remain possible in late shops,
        // and the Vault has fixed tier-two and tier-three equipment rows.
        _ => Some(limit),
    }
}

/// Narrow the kind-level envelope using the pinned Imp and vault inventories.
/// These restrictions cannot add sources, change effects, or change the RNG.
fn requirement_source_profile(
    requirement: &Requirement,
    source: ItemSource,
) -> Option<(u8, u8, EffectPolicy)> {
    let profile = source_profile(source, requirement.kind, requirement.weapon_category)?;
    // Sum members are optional. Keep their conservative source model until
    // their shared contribution is considered separately by the planner.
    if requirement.level_sum.is_some() {
        return Some(profile);
    }
    if source == ItemSource::VaultTreasure && !vault_inventory_reachable(requirement) {
        return None;
    }
    let (mut low, mut high, policy) = profile;
    match (source, requirement.kind) {
        (ItemSource::VaultTreasure, ItemKind::Wand | ItemKind::Ring) => {
            // VaultEquipmentLoot::setup_tier rerolls these identities.
            if requirement.item.is_some_and(|id| {
                matches!(
                    id,
                    ItemId::WandRegrowth
                        | ItemId::WandTransfusion
                        | ItemId::WandCorruption
                        | ItemId::RingWealth
                        | ItemId::RingMight
                        | ItemId::RingForce
                )
            }) {
                return None;
            }
        }
        (ItemSource::VaultTreasure, ItemKind::Weapon) => {
            // The only treasure above +3 is a tier-four melee weapon.
            let can_match_melee = requirement.weapon_category != Some(WeaponCategory::Thrown)
                && requirement
                    .item
                    .is_none_or(|id| id.weapon_category() == Some(WeaponCategory::Melee));
            if !can_match_melee || !can_match_tier(requirement, 4) {
                high = 3;
            }
        }
        (ItemSource::ImpReward, ItemKind::Weapon) => {
            // The final options pair tier four (+3..+5) with tier five (+2..+4).
            match (
                can_match_tier(requirement, 4),
                can_match_tier(requirement, 5),
            ) {
                (true, true) => {}
                (true, false) => low = 3,
                (false, true) => high = 4,
                (false, false) => return None,
            }
        }
        (ItemSource::ImpReward, ItemKind::Armor)
            if requirement.item.is_some_and(|id| id != ItemId::PlateArmor)
                || !requirement.tier.matches(Some(5)) =>
        {
            return None;
        }
        _ => {}
    }
    Some((low, high, policy))
}

/// Whether `source` can ever produce an item satisfying `requirement`.
fn source_feasible(
    requirement: &Requirement,
    source: ItemSource,
    profile: &impl Fn(&Requirement, ItemSource) -> Option<(u8, u8, EffectPolicy)>,
) -> bool {
    if requirement.source.is_some_and(|wanted| wanted != source) {
        return false;
    }
    let curses_only = match requirement.effect {
        EffectRequirement::OneOf(set) => set.is_curses_only(),
        EffectRequirement::Any => false,
    };
    if requirement.require_uncursed && curses_only {
        return false;
    }
    profile(requirement, source).is_some_and(|(low, high, policy)| {
        upgrade_reachable(requirement.upgrade, low, high)
            && effect_reachable(requirement.effect, policy, requirement.require_uncursed)
    })
}

const ALL_SOURCES: [ItemSource; 18] = [
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
    ItemSource::VaultTreasure,
];

/// Depths carrying a shop, in order. Depth 20 is the Imp's pre-Halls shop.
const SHOP_DEPTHS: [u8; 5] = [6, 11, 16, 20, 21];

/// One requirement's satisfiability horizon.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
struct RequirementPlan {
    requirement: Requirement,
    max_depth: u8,
    /// Bit set of quests (see [`Quest::bit`]) whose reward could satisfy the
    /// requirement inside the query's depth limit.
    quests: u8,
    vault: bool,
    /// Latest depth at which a non-quest source could still first produce a
    /// matching item, or `None` when only quests can satisfy it.
    open_deadline: Option<u8>,
}

/// Query-derived generation plan: how deep worlds must be generated and when
/// a partial world can be abandoned. Built once per search.
///
/// Combined-upgrade groups are not modelled here: they only add constraints
/// on top of their members' own predicates, so ignoring them keeps every
/// `false` answer sound and merely forgoes some early exits.
#[derive(Clone, Debug)]
pub struct QueryPlan {
    auto_trinket: Option<crate::auto_trinkets::AutoTrinketPolicy>,
    selected_slots: Vec<Vec<Requirement>>,
    /// Mandatory slots whose alternatives all name initial trinket offers.
    /// Other predicates remain for the final matcher; absence alone is enough
    /// to prove that no floor can satisfy one of these slots.
    required_trinket_slots: Vec<Vec<ItemId>>,
    /// One entry per query slot: a plain requirement alone, or every member
    /// of an alternative group, any one of which satisfies the slot.
    slots: Vec<Vec<RequirementPlan>>,
    /// Equal mandatory ordinary-only slots that need distinct item indices.
    /// Each entry stores the first slot and the number of required instances.
    closed_multiplicities: Vec<(usize, usize)>,
    generation_depth: u8,
    /// Latest depth by which a required Blacksmith must have appeared.
    blacksmith_deadline: Option<u8>,
    /// Required Wandmaker variant paired with the latest depth by which its
    /// quest must have appeared.
    wandmaker_deadline: Option<(WandmakerQuestType, u8)>,
    /// Whether some requirement could be satisfied by vault treasure, so the
    /// Imp's sub-level must be generated for a seed to be judged.
    needs_vault_treasure: bool,
    unsatisfiable: bool,
}

fn required_trinket_slots(slots: &[Vec<RequirementPlan>]) -> Vec<Vec<ItemId>> {
    slots
        .iter()
        .filter(|slot| {
            slot.iter().all(|plan| {
                let requirement = &plan.requirement;
                requirement.kind == ItemKind::Trinket
                    && requirement.item.is_some()
                    && requirement.level_sum.is_none()
            })
        })
        .map(|slot| {
            slot.iter()
                .map(|plan| plan.requirement.item.expect("named trinket"))
                .collect()
        })
        .collect()
}

// Matching uses distinct item indices. Group only identical singleton mandatory
// slots with no remaining quest source, so their ordinary deadline closes every
// possible source. Full plan equality preserves caps and all predicate metadata.
fn closed_multiplicities(slots: &[Vec<RequirementPlan>]) -> Vec<(usize, usize)> {
    use std::collections::HashMap;

    let eligible = slots.iter().enumerate().filter_map(|(index, slot)| {
        let [plan] = slot.as_slice() else {
            return None;
        };
        (plan.quests == 0
            && plan.open_deadline.is_some()
            && plan.requirement.level_sum.is_none()
            && !plan.requirement.blanket)
            .then_some((index, plan))
    });

    // Distinct item multiplicity needs at least two eligible slots. Avoid
    // even initializing the map on ordinary zero/one-eligible-slot queries.
    let mut remaining = eligible.clone();
    let Some((first_index, first_plan)) = remaining.next() else {
        return Vec::new();
    };
    let Some(second) = remaining.next() else {
        return Vec::new();
    };

    // Borrow immutable plans: Hash and Eq include every requirement field and
    // every derived horizon field. Hash collisions always receive exact Eq.
    let mut counts: HashMap<&RequirementPlan, (usize, usize)> = HashMap::new();
    counts.insert(first_plan, (first_index, 1));
    for (index, plan) in std::iter::once(second).chain(remaining) {
        counts
            .entry(plan)
            .and_modify(|(_, count)| *count += 1)
            .or_insert((index, 1));
    }

    // Never iterate the randomized map to choose evaluation order. A second
    // original-order pass emits each repeated plan at its first slot. Keeping
    // Vec::new also leaves the retained result unallocated when none repeat.
    let mut groups = Vec::new();
    for (index, plan) in eligible {
        let &(first, required) = counts.get(plan).expect("eligible plan was counted");
        if index == first && required > 1 {
            groups.push((first, required));
        }
    }
    groups
}

impl QueryPlan {
    /// Derives the plan for a validated query.
    #[must_use]
    pub fn analyze(query: &SearchQuery) -> Self {
        Self::analyze_with_policies(
            query,
            requirement_source_profile,
            source_generation_deadline,
        )
    }

    #[cfg(test)]
    fn analyze_with_profile(
        query: &SearchQuery,
        profile: impl Fn(&Requirement, ItemSource) -> Option<(u8, u8, EffectPolicy)>,
    ) -> Self {
        Self::analyze_with_policies(query, profile, |_, _, limit| Some(limit))
    }

    #[allow(clippy::too_many_lines)] // Keep source horizons and their cached constraints together.
    fn analyze_with_policies(
        query: &SearchQuery,
        profile: impl Fn(&Requirement, ItemSource) -> Option<(u8, u8, EffectPolicy)>,
        deadline: impl Fn(&Requirement, ItemSource, u8) -> Option<u8>,
    ) -> Self {
        let max_depth = query.max_depth;
        let mut generation_depth = 1;
        let mut needs_vault_treasure = false;
        let mut slots: Vec<Vec<RequirementPlan>> = Vec::new();
        for slot in query.slots() {
            let mut members = Vec::with_capacity(slot.len());
            for requirement in slot.iter().map(|index| &query.requirements[*index]) {
                let requirement_max_depth = requirement
                    .max_depth
                    .unwrap_or(max_depth)
                    .min(max_depth)
                    .min(if requirement.kind == ItemKind::Trinket {
                        3
                    } else {
                        max_depth
                    });
                let mut quests = 0_u8;
                let mut vault = false;
                let mut open_deadline = None;
                for source in ALL_SOURCES {
                    if !source_feasible(requirement, source, &profile) {
                        continue;
                    }
                    if query.exclude_blacksmith_rewards && source == ItemSource::BlacksmithReward {
                        continue;
                    }
                    let Some(source_max_depth) =
                        deadline(requirement, source, requirement_max_depth)
                    else {
                        continue;
                    };
                    if let Some(quest) = quest_for_source(source) {
                        let (window_start, window_end) = quest.window();
                        if window_start <= source_max_depth {
                            // The vault only exists inside the Imp's window,
                            // so a requirement that stops short of it never
                            // needs the sub-level generated.
                            if source == ItemSource::VaultTreasure {
                                needs_vault_treasure = true;
                                vault = true;
                            }
                            quests |= quest.bit();
                            generation_depth =
                                generation_depth.max(window_end.min(source_max_depth));
                        }
                    } else if source == ItemSource::Shop {
                        let deadline = SHOP_DEPTHS
                            .into_iter()
                            .rfind(|&depth| depth <= source_max_depth);
                        if let Some(deadline) = deadline {
                            open_deadline = Some(open_deadline.unwrap_or(0).max(deadline));
                            generation_depth = generation_depth.max(deadline);
                        }
                    } else {
                        open_deadline = Some(open_deadline.unwrap_or(0).max(source_max_depth));
                        generation_depth = generation_depth.max(source_max_depth);
                    }
                }
                members.push(RequirementPlan {
                    requirement: *requirement,
                    max_depth: requirement_max_depth,
                    quests,
                    vault,
                    open_deadline,
                });
            }
            slots.push(members);
        }

        let blacksmith_deadline = if query.require_blacksmith {
            let (window_start, window_end) = Quest::Blacksmith.window();
            if window_start <= max_depth {
                generation_depth = generation_depth.max(window_end.min(max_depth));
                Some(window_end.min(max_depth))
            } else {
                // The window cannot open at all; mark as impossible below by
                // using a deadline of zero, which no completed floor precedes.
                Some(0)
            }
        } else {
            None
        };

        // The Wandmaker's variant is fixed when its quest room is scheduled,
        // so the filter is decided by the giver's own floor and needs the
        // prefix to reach it — even when every requirement was satisfied long
        // before.
        let wandmaker_deadline = query.wandmaker_quest.map(|variant| {
            let deadline = if *WandmakerQuestType::WINDOW.start() <= max_depth {
                let deadline = (*WandmakerQuestType::WINDOW.end()).min(max_depth);
                generation_depth = generation_depth.max(deadline);
                deadline
            } else {
                // The window cannot open at all; a deadline of zero no
                // completed floor precedes marks the query impossible below.
                0
            };
            (variant, deadline)
        });

        let required_trinket_slots = required_trinket_slots(&slots);
        let closed_multiplicities = closed_multiplicities(&slots);

        let mut plan = Self {
            auto_trinket: crate::auto_trinkets::AutoTrinketPolicy::prepare(query),
            selected_slots: crate::trinkets::selection_slots(query),
            required_trinket_slots,
            closed_multiplicities,
            slots,
            generation_depth,
            blacksmith_deadline,
            wandmaker_deadline,
            needs_vault_treasure,
            unsatisfiable: false,
        };
        plan.unsatisfiable = !plan.viable_after_floor(0, &[], &QuestSummary::default());
        plan
    }

    /// Whether no seed can ever match the query (for example a +4 ring with a
    /// depth limit above the Imp's window's start).
    #[must_use]
    pub const fn is_unsatisfiable(&self) -> bool {
        self.unsatisfiable
    }

    /// Deepest floor that generation must reach: past it, no source can first
    /// produce an item any requirement still needs. Never exceeds the query's
    /// depth limit.
    #[must_use]
    pub fn generation_depth(&self) -> u8 {
        self.generation_depth.clamp(1, 24)
    }

    /// Whether a seed whose floors `1..=completed_depth` produced `items` and
    /// scheduled `quests` can still satisfy every requirement. Conservative:
    /// `false` is proof that the final matcher would reject the seed, while
    /// `true` promises nothing.
    #[must_use]
    pub fn viable_after_floor(
        &self,
        completed_depth: u8,
        items: &[WorldItem],
        quests: &QuestSummary,
    ) -> bool {
        self.viable_with_pending_vault(completed_depth, items, *quests, None)
    }

    /// A missing independent vault keeps only its own feasible requirements
    /// alive. Its items still share the Imp's one-choice resource.
    pub(crate) fn viable_with_pending_vault(
        &self,
        completed_depth: u8,
        items: &[WorldItem],
        quests: QuestSummary,
        pending_depth: Option<u8>,
    ) -> bool {
        if let Some((wanted, deadline)) = self.wandmaker_deadline {
            match quests.wandmaker {
                // The variant is rolled once per run and never revised, so a
                // mismatch kills the seed on the Wandmaker's own floor.
                Some(scheduled) => {
                    if scheduled.variant != wanted {
                        return false;
                    }
                }
                None if completed_depth >= deadline => return false,
                None => {}
            }
        }

        // Once all sources close, these slots need enough different indices in
        // the finalized prefix. Ignoring accessibility and identity conflicts
        // only adds candidates; the final matcher still enforces every rule.
        for &(slot, required) in &self.closed_multiplicities {
            let plan = &self.slots[slot][0];
            if plan
                .open_deadline
                .is_some_and(|deadline| completed_depth >= deadline)
                && items
                    .iter()
                    .filter(|item| item.depth <= plan.max_depth && plan.requirement.matches(item))
                    .take(required)
                    .count()
                    < required
            {
                return false;
            }
        }

        // Slots that only quests can still satisfy, grouped by their live
        // quest bit set. An alternative group is one slot, alive while any
        // member is. Each quest offers a mutually exclusive choice, so it can
        // cover at most one slot; Hall's condition over the sixteen quest
        // subsets then decides whether an assignment exists.
        let mut quest_only = [0_u16; 16];
        for slot in &self.slots {
            let open = slot.iter().any(|plan| {
                let satisfied_by_open_item = items.iter().any(|item| {
                    item.depth <= plan.max_depth
                        && quest_for_source(item.source).is_none()
                        && plan.requirement.matches(item)
                });
                satisfied_by_open_item
                    || plan
                        .open_deadline
                        .is_some_and(|deadline| completed_depth < deadline)
            });
            if open {
                continue;
            }
            let mut live = 0_u8;
            for plan in slot {
                for quest in QUESTS {
                    if plan.quests & quest.bit() != 0
                        && (Self::quest_alive(quest, plan, completed_depth, items)
                            || (quest == Quest::Imp
                                && plan.vault
                                && pending_depth.is_some_and(|depth| depth <= plan.max_depth)))
                    {
                        live |= quest.bit();
                    }
                }
            }
            if live == 0 {
                return false;
            }
            // Blankets still need a feasible source, but reuse an ordinary
            // slot's prize and must not consume a second quest reward.
            if !slot[0].requirement.blanket {
                quest_only[usize::from(live)] += 1;
            }
        }
        for subset in 1_u8..16 {
            let mut needed = 0_u32;
            for mask in 1_u8..16 {
                if mask & !subset == 0 {
                    needed += u32::from(quest_only[usize::from(mask)]);
                }
            }
            if needed > subset.count_ones() {
                return false;
            }
        }

        if let Some(deadline) = self.blacksmith_deadline {
            let present = items
                .iter()
                .any(|item| item.source == ItemSource::BlacksmithReward);
            if !present && completed_depth >= deadline {
                return false;
            }
        }
        true
    }

    /// Whether `quest` could still supply an item matching `requirement`.
    /// Reward items appear all at once on the quest's floor, so any item with
    /// the quest's source marks the quest as resolved for the whole run.
    fn quest_alive(
        quest: Quest,
        plan: &RequirementPlan,
        completed_depth: u8,
        items: &[WorldItem],
    ) -> bool {
        let mut resolved = false;
        for item in items {
            if quest_for_source(item.source) == Some(quest) {
                if item.depth <= plan.max_depth && plan.requirement.matches(item) {
                    return true;
                }
                resolved = true;
            }
        }
        !resolved && completed_depth < quest.window().1.min(plan.max_depth)
    }
}

impl FloorGate for QueryPlan {
    fn deferred_vault_plan(&self, target: u8) -> Option<&Self> {
        // Every vault-capable slot remains open at all existing callbacks
        // (completed_depth < target), preserving the original prefix pruning.
        // A wholly non-vault slot must retain a source into the Imp's window:
        // earlier-only slots already survived their last ordinary callback.
        // This last condition only selects an execution strategy; it never
        // rejects a seed. Optional sum members remain outside this path.
        let imp_start = Quest::Imp.window().0;
        (self.needs_vault_treasure
            && self
                .slots
                .iter()
                .flatten()
                .all(|plan| plan.requirement.level_sum.is_none())
            && self.slots.iter().any(|slot| {
                slot.iter().all(|plan| !plan.vault)
                    && slot.iter().any(|plan| {
                        plan.open_deadline.is_some_and(|depth| depth >= imp_start)
                            || plan.quests & Quest::Imp.bit() != 0
                    })
            })
            && self.slots.iter().all(|slot| {
                !slot.iter().any(|plan| plan.vault)
                    || slot
                        .iter()
                        .any(|plan| plan.open_deadline.is_some_and(|depth| depth >= target))
            }))
        .then_some(self)
    }

    fn continue_after_run_init(&self, run: &crate::run::RunState) -> bool {
        if self.required_trinket_slots.is_empty() {
            return true;
        }
        // The catalyst only expands into these initial identities. Peeking
        // clones its private deck, leaving all generation streams untouched.
        let offers = crate::trinkets::initial_offers_from_generator(&run.generator);
        self.required_trinket_slots
            .iter()
            .all(|slot| slot.iter().any(|id| offers.contains(id)))
    }

    fn selected_trinket(&self, seed: crate::seed::DungeonSeed) -> Option<crate::catalog::ItemId> {
        if let Some(policy) = &self.auto_trinket {
            return policy.selected_trinket(seed);
        }
        crate::trinkets::resolve_selection(seed, &self.selected_slots)
    }

    fn continue_after_floor(
        &self,
        completed_depth: u8,
        items_so_far: &[WorldItem],
        quests_so_far: &QuestSummary,
    ) -> bool {
        self.viable_after_floor(completed_depth, items_so_far, quests_so_far)
    }

    fn wants_vault_treasure(&self) -> bool {
        self.needs_vault_treasure
    }
}

#[cfg(test)]
mod tests {
    use crate::catalog::{ArmorEffect, Effect, ItemId, ItemKind, WeaponEffect};
    use crate::model::{Accessibility, ItemSource, WorldItem};
    use crate::query::{
        EffectRequirement, EffectSet, Requirement, SearchQuery, TierRequirement, UpgradeRequirement,
    };

    use crate::quests::QuestSummary;

    use super::QueryPlan;

    fn requirement(kind: ItemKind, upgrade: UpgradeRequirement) -> Requirement {
        Requirement {
            kind,
            weapon_category: None,
            item: None,
            tier: TierRequirement::Any,
            upgrade,
            effect: EffectRequirement::Any,
            require_uncursed: false,
            select_trinket: false,
            blanket: false,
            source: None,
            identity_group: None,
            max_depth: None,
            alternative_group: None,
            level_sum: None,
        }
    }

    fn query(requirements: Vec<Requirement>, max_depth: u8) -> SearchQuery {
        SearchQuery {
            auto_apply_trinket: false,
            requirements,
            max_depth,
            challenges: crate::challenges::Challenges::NONE,
            require_blacksmith: false,
            exclude_blacksmith_rewards: false,
            wandmaker_quest: None,
        }
    }

    /// No plan in these cases carries a quest filter, so the summary is
    /// irrelevant and the assertions stay about the item horizon.
    fn viable(plan: &QueryPlan, completed_depth: u8, items: &[WorldItem]) -> bool {
        plan.viable_after_floor(completed_depth, items, &QuestSummary::default())
    }

    fn item(kind_item: ItemId, upgrade: u8, depth: u8, source: ItemSource) -> WorldItem {
        WorldItem {
            item: kind_item,
            upgrade,
            effect: None,
            cursed: false,
            depth,
            source,
            accessibility: Accessibility::Independent,
            secret: false,
        }
    }

    #[test]
    fn published_quest_windows_match_the_dungeon_and_the_quest_model() {
        use super::{QUESTS, Quest};

        assert_eq!(
            QUESTS.map(Quest::window),
            [(2, 4), (7, 9), (12, 14), (17, 19)]
        );
        // The Wandmaker's own window is the same fact spelled out in
        // `quests`, so the two views can never disagree.
        let (start, end) = Quest::Wandmaker.window();
        assert_eq!(
            start..=end,
            crate::quests::WandmakerQuestType::WINDOW,
            "the Wandmaker window must agree with the quest model"
        );
    }

    #[test]
    fn plus_four_ring_is_imp_only_with_a_depth_nineteen_deadline() {
        let plan = QueryPlan::analyze(&query(
            vec![requirement(ItemKind::Ring, UpgradeRequirement::Exact(4))],
            24,
        ));
        assert!(!plan.is_unsatisfiable());
        assert_eq!(plan.generation_depth(), 19);

        // Below the Imp's window the query is impossible.
        let shallow = QueryPlan::analyze(&query(
            vec![requirement(ItemKind::Ring, UpgradeRequirement::Exact(4))],
            16,
        ));
        assert!(shallow.is_unsatisfiable());
    }

    #[test]
    fn plus_three_wand_comes_from_the_wandmaker_or_the_imp() {
        let plan = QueryPlan::analyze(&query(
            vec![requirement(ItemKind::Wand, UpgradeRequirement::Exact(3))],
            24,
        ));
        assert!(!plan.is_unsatisfiable());
        // The Imp's vault also holds +3 wands, so the horizon is its floor.
        assert_eq!(plan.generation_depth(), 19);
        // A resolved Wandmaker without a +3 wand leaves the Imp alive.
        let mismatched = [item(ItemId::WandFrost, 2, 7, ItemSource::WandmakerReward)];
        assert!(viable(&plan, 7, &mismatched));
        assert!(viable(&plan, 9, &mismatched));
        assert!(viable(&plan, 18, &mismatched));
        // Once the Imp also resolves without one, the seed is dead.
        let both_missed = [
            item(ItemId::WandFrost, 2, 7, ItemSource::WandmakerReward),
            item(ItemId::WandFrost, 2, 18, ItemSource::ImpReward),
        ];
        assert!(!viable(&plan, 18, &both_missed));
        // A matching reward from either giver keeps the seed alive
        // permanently — the vault's treasure counts as the Imp's.
        let wandmaker = [item(ItemId::WandFrost, 3, 8, ItemSource::WandmakerReward)];
        assert!(viable(&plan, 9, &wandmaker));
        assert!(viable(&plan, 19, &wandmaker));
        let vault = [
            item(ItemId::WandFrost, 2, 7, ItemSource::WandmakerReward),
            item(ItemId::WandFrost, 3, 18, ItemSource::VaultTreasure),
        ];
        assert!(viable(&plan, 19, &vault));
        // Below the Imp's window the Wandmaker is the only source and its
        // floor is the horizon.
        let shallow = QueryPlan::analyze(&query(
            vec![requirement(ItemKind::Wand, UpgradeRequirement::Exact(3))],
            16,
        ));
        assert_eq!(shallow.generation_depth(), 9);
        assert!(!viable(&shallow, 9, &[]));
    }

    #[test]
    fn plus_five_weapon_is_imp_only_with_a_depth_nineteen_deadline() {
        let plan = QueryPlan::analyze(&query(
            vec![requirement(ItemKind::Weapon, UpgradeRequirement::Exact(5))],
            24,
        ));
        assert!(!plan.is_unsatisfiable());
        assert_eq!(plan.generation_depth(), 19);
        // Nothing before the Imp can produce it, so an empty prefix stays
        // alive right up to the window's last floor and no further.
        assert!(viable(&plan, 18, &[]));
        assert!(!viable(&plan, 19, &[]));
        // Only the Imp's own prizes reach +5; the vault's treasure stops at
        // +4, so a vault weapon never satisfies it.
        let vault = [item(ItemId::Greatsword, 4, 18, ItemSource::VaultTreasure)];
        assert!(!viable(&plan, 18, &vault));
        let prize = [item(ItemId::Greatsword, 5, 18, ItemSource::ImpReward)];
        assert!(viable(&plan, 19, &prize));
        // At-least +5 is the same question.
        let at_least = QueryPlan::analyze(&query(
            vec![requirement(
                ItemKind::Weapon,
                UpgradeRequirement::AtLeast(5),
            )],
            24,
        ));
        assert_eq!(at_least.generation_depth(), 19);
        // Below the Imp's window the query is impossible.
        let shallow = QueryPlan::analyze(&query(
            vec![requirement(ItemKind::Weapon, UpgradeRequirement::Exact(5))],
            16,
        ));
        assert!(shallow.is_unsatisfiable());
    }

    #[test]
    fn plus_four_wand_armor_and_ring_are_imp_only() {
        for kind in [ItemKind::Wand, ItemKind::Armor, ItemKind::Ring] {
            let plan = QueryPlan::analyze(&query(
                vec![requirement(kind, UpgradeRequirement::Exact(4))],
                24,
            ));
            assert!(!plan.is_unsatisfiable(), "{kind:?}");
            assert_eq!(plan.generation_depth(), 19, "{kind:?}");
            assert!(viable(&plan, 18, &[]), "{kind:?}");
            assert!(!viable(&plan, 19, &[]), "{kind:?}");
            let shallow = QueryPlan::analyze(&query(
                vec![requirement(kind, UpgradeRequirement::Exact(4))],
                16,
            ));
            assert!(shallow.is_unsatisfiable(), "{kind:?}");
        }
        // The vault's treasure stops at +3 for these kinds: a +3 vault wand
        // resolves the Imp without satisfying a +4.
        let wand = QueryPlan::analyze(&query(
            vec![requirement(ItemKind::Wand, UpgradeRequirement::Exact(4))],
            24,
        ));
        let vault = [item(ItemId::WandFrost, 3, 18, ItemSource::VaultTreasure)];
        assert!(!viable(&wand, 18, &vault));
        let prize = [item(ItemId::WandFrost, 4, 18, ItemSource::ImpReward)];
        assert!(viable(&wand, 19, &prize));
    }

    #[test]
    fn plus_four_weapon_comes_from_either_imp_source() {
        let plus_four = requirement(ItemKind::Weapon, UpgradeRequirement::Exact(4));
        let plan = QueryPlan::analyze(&query(vec![plus_four], 24));
        assert!(!plan.is_unsatisfiable());
        assert_eq!(plan.generation_depth(), 19);
        // Pinning either Imp source keeps the query possible; every other
        // source stops at +3.
        for source in [ItemSource::ImpReward, ItemSource::VaultTreasure] {
            let pinned = QueryPlan::analyze(&query(
                vec![Requirement {
                    source: Some(source),
                    ..plus_four
                }],
                24,
            ));
            assert!(!pinned.is_unsatisfiable(), "{source:?}");
            assert_eq!(pinned.generation_depth(), 19, "{source:?}");
        }
        for source in [ItemSource::Chest, ItemSource::BlacksmithReward] {
            let pinned = QueryPlan::analyze(&query(
                vec![Requirement {
                    source: Some(source),
                    ..plus_four
                }],
                24,
            ));
            assert!(pinned.is_unsatisfiable(), "{source:?}");
        }
        // A +4 weapon from either source keeps the seed alive permanently.
        let vault = [item(ItemId::Greatsword, 4, 18, ItemSource::VaultTreasure)];
        assert!(viable(&plan, 19, &vault));
        let prize = [item(ItemId::Greatsword, 4, 18, ItemSource::ImpReward)];
        assert!(viable(&plan, 19, &prize));
        // Both sources are the same quest: prizes from one and treasure from
        // the other appear together, and a miss across both is final.
        let missed = [
            item(ItemId::Greatsword, 3, 18, ItemSource::ImpReward),
            item(ItemId::Greatsword, 3, 18, ItemSource::VaultTreasure),
        ];
        assert!(!viable(&plan, 18, &missed));
    }

    #[test]
    fn two_quest_only_imp_slots_are_impossible() {
        // The player carries one item out of the vault, so two requirements
        // that only the Imp can satisfy can never both be met — whether they
        // hinge on the Imp's own prizes, the vault's treasure, or both.
        let weapon = requirement(ItemKind::Weapon, UpgradeRequirement::Exact(5));
        let wand = requirement(ItemKind::Wand, UpgradeRequirement::Exact(4));
        assert!(QueryPlan::analyze(&query(vec![weapon, wand], 24)).is_unsatisfiable());
        let vault_weapon = Requirement {
            source: Some(ItemSource::VaultTreasure),
            ..requirement(ItemKind::Weapon, UpgradeRequirement::Exact(4))
        };
        assert!(QueryPlan::analyze(&query(vec![weapon, vault_weapon], 24)).is_unsatisfiable());
        let armor = requirement(ItemKind::Armor, UpgradeRequirement::Exact(4));
        let ring = requirement(ItemKind::Ring, UpgradeRequirement::AtLeast(4));
        assert!(QueryPlan::analyze(&query(vec![armor, ring], 24)).is_unsatisfiable());
        // One Imp-only slot beside a slot another quest can cover is fine.
        let plus_three_wand = requirement(ItemKind::Wand, UpgradeRequirement::Exact(3));
        let plan = QueryPlan::analyze(&query(vec![weapon, plus_three_wand], 24));
        assert!(!plan.is_unsatisfiable());
        // ...until the Wandmaker resolves without its wand, leaving both
        // slots to the single Imp pick.
        let missed = [item(ItemId::WandFrost, 2, 8, ItemSource::WandmakerReward)];
        assert!(!viable(&plan, 9, &missed));
    }

    #[test]
    fn excluding_blacksmith_rewards_leaves_the_imp_alone() {
        let excluding = |requirements| SearchQuery {
            exclude_blacksmith_rewards: true,
            ..query(requirements, 24)
        };
        // Imp-only requirements are untouched by the exclusion.
        let wand = QueryPlan::analyze(&excluding(vec![requirement(
            ItemKind::Wand,
            UpgradeRequirement::Exact(4),
        )]));
        assert!(!wand.is_unsatisfiable());
        assert_eq!(wand.generation_depth(), 19);
        for source in [ItemSource::ImpReward, ItemSource::VaultTreasure] {
            let pinned = QueryPlan::analyze(&excluding(vec![Requirement {
                source: Some(source),
                ..requirement(ItemKind::Weapon, UpgradeRequirement::Exact(4))
            }]));
            assert!(!pinned.is_unsatisfiable(), "{source:?}");
        }
        // The Blacksmith's own prizes are what it removes.
        let blacksmith = QueryPlan::analyze(&excluding(vec![Requirement {
            source: Some(ItemSource::BlacksmithReward),
            ..requirement(ItemKind::Armor, UpgradeRequirement::Exact(3))
        }]));
        assert!(blacksmith.is_unsatisfiable());
    }

    #[test]
    fn thrown_plus_five_is_possible() {
        use crate::catalog::WeaponCategory;

        // The Imp's tier-4 thrown prize rolls +3..+5, so a +5 thrown weapon
        // is an Imp-only requirement like any other +5.
        let thrown = Requirement {
            weapon_category: Some(WeaponCategory::Thrown),
            ..requirement(ItemKind::Weapon, UpgradeRequirement::Exact(5))
        };
        let plan = QueryPlan::analyze(&query(vec![thrown], 24));
        assert!(!plan.is_unsatisfiable());
        assert_eq!(plan.generation_depth(), 19);
        let prize = [item(ItemId::ThrowingHammer, 5, 18, ItemSource::ImpReward)];
        assert!(viable(&plan, 19, &prize));
        let pinned = QueryPlan::analyze(&query(
            vec![Requirement {
                source: Some(ItemSource::ImpReward),
                ..thrown
            }],
            24,
        ));
        assert!(!pinned.is_unsatisfiable());
        // A melee +5 prize does not satisfy the thrown filter.
        let melee = [item(ItemId::Greatsword, 5, 18, ItemSource::ImpReward)];
        assert!(!viable(&plan, 18, &melee));
    }

    #[test]
    fn two_plus_four_rings_can_never_coexist() {
        let plan = QueryPlan::analyze(&query(
            vec![
                requirement(ItemKind::Ring, UpgradeRequirement::Exact(4)),
                requirement(ItemKind::Ring, UpgradeRequirement::AtLeast(4)),
            ],
            24,
        ));
        assert!(plan.is_unsatisfiable());
    }

    #[test]
    fn uncursed_plus_four_ring_comes_only_from_the_imp() {
        // v4.0.0 vault prizes are never cursed, so the flag no longer kills
        // the query; the ring is still quest-only and needs the Imp's floor.
        let mut ring = requirement(ItemKind::Ring, UpgradeRequirement::Exact(4));
        ring.require_uncursed = true;

        let plan = QueryPlan::analyze(&query(vec![ring], 24));
        assert!(!plan.is_unsatisfiable());
        assert_eq!(plan.generation_depth(), 19);
    }

    #[test]
    fn exclusive_quest_choices_respect_capacity_after_resolution() {
        // +3 weapon and +3 armor can come from the Ghost, the Blacksmith and
        // the Imp, one each — and Crypt/Sacrifice keep both alive to full
        // depth besides.
        let plan = QueryPlan::analyze(&query(
            vec![
                requirement(ItemKind::Weapon, UpgradeRequirement::Exact(3)),
                requirement(ItemKind::Armor, UpgradeRequirement::Exact(3)),
            ],
            24,
        ));
        assert!(!plan.is_unsatisfiable());
        assert_eq!(plan.generation_depth(), 24);

        // Wands never exceed +2 outside the quests, so a +3 wand is
        // Wandmaker-or-Imp and a +4 wand is Imp-only: two requirements, two
        // quests, one pick each.
        let wands = QueryPlan::analyze(&query(
            vec![
                requirement(ItemKind::Wand, UpgradeRequirement::Exact(3)),
                requirement(ItemKind::Wand, UpgradeRequirement::Exact(4)),
            ],
            24,
        ));
        assert!(!wands.is_unsatisfiable());
        assert_eq!(wands.generation_depth(), 19);
        // A +3 Wandmaker prize leaves the +4 to the Imp.
        let wandmaker_hit = [item(ItemId::WandFrost, 3, 8, ItemSource::WandmakerReward)];
        assert!(viable(&wands, 9, &wandmaker_hit));
        // The Wandmaker resolved without a +3: both requirements now hinge on
        // the single Imp pick, which cannot cover two items.
        let wandmaker_missed = [item(ItemId::WandFrost, 2, 8, ItemSource::WandmakerReward)];
        assert!(!viable(&wands, 9, &wandmaker_missed));
    }

    #[test]
    fn curse_effects_exclude_good_only_sources() {
        // A cursed enchantment on a +3 weapon leaves only the Sacrificial fire.
        let cursed = Requirement {
            effect: EffectRequirement::exactly(Effect::Weapon(WeaponEffect::Sacrificial)),
            require_uncursed: false,
            select_trinket: false,
            blanket: false,
            ..requirement(ItemKind::Weapon, UpgradeRequirement::Exact(3))
        };
        let plan = QueryPlan::analyze(&query(vec![cursed], 24));
        assert!(!plan.is_unsatisfiable());
        assert_eq!(plan.generation_depth(), 24);

        // A good glyph on +3 armor is reachable via the Ghost, the
        // Blacksmith and the Imp as well as the exotic bumps.
        let good = Requirement {
            effect: EffectRequirement::exactly(Effect::Armor(ArmorEffect::Thorns)),
            require_uncursed: false,
            select_trinket: false,
            blanket: false,
            ..requirement(ItemKind::Armor, UpgradeRequirement::Exact(3))
        };
        assert!(!QueryPlan::analyze(&query(vec![good], 24)).is_unsatisfiable());
        // Cursed effects never reach the Imp's good-only prizes either.
        let cursed_plus_four = Requirement {
            effect: EffectRequirement::exactly(Effect::Weapon(WeaponEffect::Pressurized)),
            require_uncursed: false,
            select_trinket: false,
            blanket: false,
            ..requirement(ItemKind::Weapon, UpgradeRequirement::Exact(4))
        };
        assert!(QueryPlan::analyze(&query(vec![cursed_plus_four], 24)).is_unsatisfiable());
        // A new good enchantment is as reachable there as an old one.
        let crystal_plus_five = Requirement {
            effect: EffectRequirement::exactly(Effect::Weapon(WeaponEffect::Crystal)),
            require_uncursed: true,
            select_trinket: false,
            blanket: false,
            ..requirement(ItemKind::Weapon, UpgradeRequirement::Exact(5))
        };
        assert!(!QueryPlan::analyze(&query(vec![crystal_plus_five], 24)).is_unsatisfiable());

        // A mixed set stays reachable through good-only sources, and an
        // uncursed pure-curse set is impossible anywhere.
        let mixed = Requirement {
            effect: EffectRequirement::OneOf(
                EffectSet::from_effects([
                    Effect::Weapon(WeaponEffect::Sacrificial),
                    Effect::Weapon(WeaponEffect::Blazing),
                ])
                .unwrap(),
            ),
            require_uncursed: false,
            select_trinket: false,
            blanket: false,
            ..requirement(ItemKind::Weapon, UpgradeRequirement::Exact(3))
        };
        assert!(!QueryPlan::analyze(&query(vec![mixed], 24)).is_unsatisfiable());
        // Any enchantment on an uncursed +3 weapon keeps the quest sources.
        let any_enchantment = Requirement {
            effect: EffectRequirement::OneOf(EffectSet::enchantments(ItemKind::Weapon).unwrap()),
            require_uncursed: true,
            select_trinket: false,
            blanket: false,
            ..requirement(ItemKind::Weapon, UpgradeRequirement::Exact(3))
        };
        assert!(!QueryPlan::analyze(&query(vec![any_enchantment], 24)).is_unsatisfiable());
    }

    #[test]
    fn alternative_groups_stay_alive_while_any_member_can_still_match() {
        // A +3 wand (Wandmaker-only) or a +4 ring (Imp-only): the slot lives
        // until both quests have resolved without a matching prize.
        let wand = Requirement {
            alternative_group: Some(1),
            ..requirement(ItemKind::Wand, UpgradeRequirement::Exact(3))
        };
        let ring = Requirement {
            alternative_group: Some(1),
            ..requirement(ItemKind::Ring, UpgradeRequirement::Exact(4))
        };
        let plan = QueryPlan::analyze(&query(vec![wand, ring], 24));
        assert!(!plan.is_unsatisfiable());
        // Generation must reach the deepest member's horizon.
        assert_eq!(plan.generation_depth(), 19);
        // A Wandmaker that resolved without the wand leaves the Imp alive.
        let missed_wand = [item(ItemId::WandFrost, 2, 8, ItemSource::WandmakerReward)];
        assert!(viable(&plan, 9, &missed_wand));
        // Once the Imp also resolves without a +4 ring, the slot is dead.
        let both_missed = [
            item(ItemId::WandFrost, 2, 8, ItemSource::WandmakerReward),
            item(ItemId::RingMight, 3, 18, ItemSource::ImpReward),
        ];
        assert!(!viable(&plan, 19, &both_missed));
        // A matching member keeps the slot alive permanently.
        let ring_hit = [
            item(ItemId::WandFrost, 2, 8, ItemSource::WandmakerReward),
            item(ItemId::RingMight, 4, 18, ItemSource::ImpReward),
        ];
        assert!(viable(&plan, 19, &ring_hit));
        // Two quest-only slots still need two quests: a second group whose
        // members both hinge on the Imp is pruned with the first.
        let imp_only = Requirement {
            alternative_group: Some(2),
            ..requirement(ItemKind::Ring, UpgradeRequirement::Exact(4))
        };
        let two_slots = QueryPlan::analyze(&query(vec![wand, ring, imp_only], 24));
        assert!(!two_slots.is_unsatisfiable());
        assert!(!viable(&two_slots, 9, &missed_wand));
        // A group whose members are all impossible is unsatisfiable.
        let impossible = QueryPlan::analyze(&query(vec![ring, wand], 6));
        assert!(impossible.is_unsatisfiable());
    }

    #[test]
    fn shop_pinned_requirements_use_shop_depths() {
        let shop = Requirement {
            source: Some(ItemSource::Shop),
            ..requirement(ItemKind::Weapon, UpgradeRequirement::Any)
        };
        let plan = QueryPlan::analyze(&query(vec![shop], 24));
        assert_eq!(plan.generation_depth(), 21);
        let shallow = QueryPlan::analyze(&query(vec![shop], 12));
        assert_eq!(shallow.generation_depth(), 11);
        let impossible = QueryPlan::analyze(&SearchQuery {
            requirements: vec![Requirement {
                upgrade: UpgradeRequirement::AtLeast(1),
                ..shop
            }],
            ..query(vec![], 24)
        });
        assert!(impossible.is_unsatisfiable());
    }

    #[test]
    fn melee_only_sources_never_satisfy_thrown_requirements() {
        use crate::catalog::WeaponCategory;

        // The Ghost, Sacrificial fire, and both statue kinds roll melee
        // weapons only, so pinning one with a thrown filter is impossible.
        for source in [
            ItemSource::GhostReward,
            ItemSource::SacrificialFire,
            ItemSource::Statue,
            ItemSource::ArmoredStatue,
        ] {
            let thrown = Requirement {
                weapon_category: Some(WeaponCategory::Thrown),
                source: Some(source),
                ..requirement(ItemKind::Weapon, UpgradeRequirement::Any)
            };
            assert!(
                QueryPlan::analyze(&query(vec![thrown], 24)).is_unsatisfiable(),
                "{source:?}"
            );
            let melee = Requirement {
                weapon_category: Some(WeaponCategory::Melee),
                ..thrown
            };
            assert!(
                !QueryPlan::analyze(&query(vec![melee], 24)).is_unsatisfiable(),
                "{source:?}"
            );
        }

        // Unpinned thrown requirements stay satisfiable through open drops.
        let thrown = Requirement {
            weapon_category: Some(WeaponCategory::Thrown),
            ..requirement(ItemKind::Weapon, UpgradeRequirement::Any)
        };
        assert!(!QueryPlan::analyze(&query(vec![thrown], 24)).is_unsatisfiable());
    }

    #[test]
    fn chest_prizes_reach_plus_three() {
        use crate::catalog::WeaponCategory;

        // The flooded-vault, sentry, and secret-maze rooms bump one natural
        // weapon/missile/armor roll and drop it as a plain chest, so a
        // chest-pinned +3 must stay satisfiable — seed AAA-AAA-ACO carries a
        // +3 thrown weapon in exactly such a chest at depth 24.
        for (kind, category) in [
            (ItemKind::Weapon, None),
            (ItemKind::Weapon, Some(WeaponCategory::Melee)),
            (ItemKind::Weapon, Some(WeaponCategory::Thrown)),
            (ItemKind::Armor, None),
        ] {
            let pinned = Requirement {
                weapon_category: category,
                source: Some(ItemSource::Chest),
                ..requirement(kind, UpgradeRequirement::Exact(3))
            };
            assert!(
                !QueryPlan::analyze(&query(vec![pinned], 24)).is_unsatisfiable(),
                "{kind:?} {category:?}"
            );
        }

        // No chest path upgrades wands or rings past the natural rolls.
        let wand = Requirement {
            source: Some(ItemSource::Chest),
            ..requirement(ItemKind::Wand, UpgradeRequirement::Exact(3))
        };
        assert!(QueryPlan::analyze(&query(vec![wand], 24)).is_unsatisfiable());

        // With chests open at any depth, an unpinned thrown +3 must not
        // inherit the Blacksmith's depth-14 deadline: the depth-24 chest
        // prize of seed AAA-AAA-ACO would otherwise be silently skipped.
        let thrown = Requirement {
            weapon_category: Some(WeaponCategory::Thrown),
            ..requirement(ItemKind::Weapon, UpgradeRequirement::Exact(3))
        };
        let plan = QueryPlan::analyze(&query(vec![thrown], 24));
        assert!(!plan.is_unsatisfiable());
        assert_eq!(plan.generation_depth(), 24);
        assert!(viable(&plan, 14, &[]));
    }

    #[test]
    fn require_blacksmith_bounds_depth_and_liveness() {
        let mut base = query(
            vec![requirement(ItemKind::Wand, UpgradeRequirement::Exact(3))],
            24,
        );
        base.require_blacksmith = true;
        let plan = QueryPlan::analyze(&base);
        assert!(!plan.is_unsatisfiable());
        // The +3 wand may also be the Imp's, so the item horizon is floor
        // 19; the Blacksmith condition itself is decided by floor 14.
        assert_eq!(plan.generation_depth(), 19);
        let wand = [item(ItemId::WandFrost, 3, 8, ItemSource::WandmakerReward)];
        assert!(viable(&plan, 13, &wand));
        assert!(!viable(&plan, 14, &wand));
        // With the wand pinned to the Wandmaker, nothing past the Blacksmith
        // matters and the plan ends at its deadline.
        let mut pinned = base.clone();
        pinned.requirements[0].source = Some(ItemSource::WandmakerReward);
        assert_eq!(QueryPlan::analyze(&pinned).generation_depth(), 14);

        base.max_depth = 11;
        assert!(QueryPlan::analyze(&base).is_unsatisfiable());
    }

    #[test]
    fn wandmaker_quest_filter_bounds_depth_and_prunes_on_the_rolled_variant() {
        use crate::quests::{ScheduledQuest, WandmakerQuestType};

        let quested = |max_depth| SearchQuery {
            wandmaker_quest: Some(WandmakerQuestType::Rotberry),
            ..query(
                vec![requirement(ItemKind::Weapon, UpgradeRequirement::Any)],
                max_depth,
            )
        };
        let scheduled = |variant| QuestSummary {
            wandmaker: Some(ScheduledQuest { variant, depth: 8 }),
            ..QuestSummary::default()
        };

        // Open weapon drops alone would run to depth 24; the filter only ever
        // extends generation, never shortens it.
        let plan = QueryPlan::analyze(&quested(24));
        assert!(!plan.is_unsatisfiable());
        assert_eq!(plan.generation_depth(), 24);

        // A wrong variant kills the seed the moment the Wandmaker appears,
        // long before the item horizon runs out.
        assert!(!plan.viable_after_floor(8, &[], &scheduled(WandmakerQuestType::ElementalEmbers)));
        assert!(plan.viable_after_floor(8, &[], &scheduled(WandmakerQuestType::Rotberry)));
        // No Wandmaker yet is fine until its window closes.
        assert!(plan.viable_after_floor(8, &[], &QuestSummary::default()));
        assert!(!plan.viable_after_floor(9, &[], &QuestSummary::default()));

        // A shallow query that stops before the Prison must still reach the
        // Wandmaker's floor, and one that cannot is impossible.
        let shallow = QueryPlan::analyze(&SearchQuery {
            requirements: vec![Requirement {
                max_depth: Some(3),
                ..requirement(ItemKind::Weapon, UpgradeRequirement::Any)
            }],
            ..quested(24)
        });
        assert_eq!(shallow.generation_depth(), 9);
        assert!(QueryPlan::analyze(&quested(6)).is_unsatisfiable());
        // Floors seven and eight can host the giver, so they stay possible.
        assert!(!QueryPlan::analyze(&quested(7)).is_unsatisfiable());
        assert_eq!(QueryPlan::analyze(&quested(7)).generation_depth(), 7);
    }

    #[test]
    fn wildcard_requirements_keep_exact_full_depth_semantics() {
        let plan = QueryPlan::analyze(&query(
            vec![requirement(ItemKind::Weapon, UpgradeRequirement::Any)],
            24,
        ));
        assert!(!plan.is_unsatisfiable());
        assert_eq!(plan.generation_depth(), 24);
        assert!(viable(&plan, 23, &[]));
    }

    #[test]
    fn per_requirement_floor_limit_short_circuits_generation() {
        let limited = Requirement {
            max_depth: Some(5),
            ..requirement(ItemKind::Weapon, UpgradeRequirement::Any)
        };
        let plan = QueryPlan::analyze(&query(vec![limited], 24));
        assert_eq!(plan.generation_depth(), 5);
        assert!(viable(&plan, 4, &[]));
        assert!(!viable(&plan, 5, &[]));

        let in_time = [item(ItemId::Sword, 0, 5, ItemSource::Heap)];
        assert!(viable(&plan, 5, &in_time));
        let too_late = [item(ItemId::Sword, 0, 6, ItemSource::Heap)];
        assert!(!viable(&plan, 6, &too_late));
    }

    #[test]
    fn the_vault_sub_level_is_only_requested_inside_the_imps_window() {
        use crate::search::FloorGate as _;

        // A weapon anywhere in the run can come out of the vault.
        let deep = QueryPlan::analyze(&query(
            vec![requirement(ItemKind::Weapon, UpgradeRequirement::Any)],
            24,
        ));
        assert!(deep.wants_vault_treasure());

        // The same weapon capped above the run still reaches depth 17.
        let edge = QueryPlan::analyze(&query(
            vec![requirement(ItemKind::Weapon, UpgradeRequirement::Any)],
            17,
        ));
        assert!(edge.wants_vault_treasure());

        // One floor short of the window, the vault can supply nothing, so
        // the generator must not pay for the sub-level.
        let shallow = QueryPlan::analyze(&query(
            vec![requirement(ItemKind::Weapon, UpgradeRequirement::Any)],
            16,
        ));
        assert!(!shallow.wants_vault_treasure());

        // A per-requirement cap prunes it even when the run goes deeper.
        let capped = Requirement {
            max_depth: Some(16),
            ..requirement(ItemKind::Weapon, UpgradeRequirement::Any)
        };
        let mixed = QueryPlan::analyze(&query(vec![capped], 24));
        assert!(!mixed.wants_vault_treasure());
    }

    fn multiplicity_query(requirements: Vec<Requirement>, depth: u8) -> SearchQuery {
        let result = query(requirements, depth);
        result.validate().unwrap();
        result
    }

    fn multiplicity_world(items: &[WorldItem]) -> crate::model::GeneratedWorld {
        crate::model::GeneratedWorld {
            seed: crate::seed::DungeonSeed::MIN,
            items: items.to_vec(),
            feelings: Vec::new(),
            quests: QuestSummary::default(),
            ring_gems: crate::run::RingGems::UNSHUFFLED,
        }
    }

    fn early_wand(upgrade: UpgradeRequirement) -> Requirement {
        Requirement {
            max_depth: Some(4),
            ..requirement(ItemKind::Wand, upgrade)
        }
    }

    fn without_closed_multiplicities(plan: &QueryPlan) -> QueryPlan {
        let mut original = plan.clone();
        original.closed_multiplicities.clear();
        original
    }

    #[test]
    fn closed_multiplicities_wait_for_deadline_and_count_item_indices() {
        let wanted = early_wand(UpgradeRequirement::Exact(2));
        let candidate = item(ItemId::WandFrost, 2, 2, ItemSource::Heap);
        for count in [2, 3] {
            let query = multiplicity_query(vec![wanted; count], 24);
            let plan = QueryPlan::analyze(&query);
            let original = without_closed_multiplicities(&plan);
            assert_eq!(plan.closed_multiplicities, vec![(0, count)]);
            assert!(viable(&plan, 3, std::slice::from_ref(&candidate)));
            let short = vec![candidate.clone(); count - 1];
            assert!(
                viable(&original, 4, &short),
                "pin the previous shared-item gap"
            );
            assert!(!viable(&plan, 4, &short));
            assert!(!viable(&plan, 6, &short));
            assert!(!query.matches(&multiplicity_world(&short)));
            // Equal item values at separate vector indices are separate objects.
            // Deduplicating by item identity or WorldItem value would be unsound.
            let enough = vec![candidate.clone(); count];
            assert!(viable(&plan, 4, &enough));
            assert!(query.matches(&multiplicity_world(&enough)));
        }
    }

    #[test]
    fn closed_multiplicities_keep_interleaved_groups_and_counts_separate() {
        let wand = early_wand(UpgradeRequirement::Any);
        let ring = Requirement {
            kind: ItemKind::Ring,
            ..wand
        };
        let query = multiplicity_query(vec![wand, ring, wand, ring, ring], 4);
        let plan = QueryPlan::analyze(&query);
        assert_eq!(plan.closed_multiplicities, vec![(0, 2), (1, 3)]);
        let w = item(ItemId::WandFrost, 1, 2, ItemSource::Heap);
        let r = item(ItemId::RingMight, 1, 3, ItemSource::Heap);
        let short = [w.clone(), r.clone(), w.clone(), r.clone()];
        assert!(viable(&without_closed_multiplicities(&plan), 4, &short));
        assert!(!viable(&plan, 4, &short));
        let enough = [w.clone(), r.clone(), w, r.clone(), r];
        assert!(viable(&plan, 4, &enough));
        assert!(query.matches(&multiplicity_world(&enough)));
    }

    #[test]
    fn closed_multiplicities_respect_item_caps_and_earlier_source_deadlines() {
        let wanted = early_wand(UpgradeRequirement::AtLeast(2));
        let query = multiplicity_query(vec![wanted; 2], 24);
        let plan = QueryPlan::analyze(&query);
        let capped = [
            item(ItemId::WandFrost, 2, 2, ItemSource::Heap),
            item(ItemId::WandLightning, 2, 6, ItemSource::Heap),
        ];
        // The depth6 item is already present at this callback but exceeds the
        // requirement's cap4; Requirement::matches alone does not test depth.
        assert!(viable(&without_closed_multiplicities(&plan), 6, &capped));
        assert!(!viable(&plan, 6, &capped));
        assert!(!query.matches(&multiplicity_world(&capped)));

        let natural = Requirement {
            source: Some(ItemSource::Heap),
            tier: TierRequirement::Exact(2),
            ..requirement(ItemKind::Armor, UpgradeRequirement::Any)
        };
        let query = multiplicity_query(vec![natural; 2], 24);
        let plan = QueryPlan::analyze(&query);
        assert_eq!(plan.slots[0][0].max_depth, 24);
        assert_eq!(plan.slots[0][0].open_deadline, Some(9));
        let leather = item(ItemId::LeatherArmor, 0, 2, ItemSource::Heap);
        assert!(viable(&plan, 8, std::slice::from_ref(&leather)));
        assert!(!viable(&plan, 9, std::slice::from_ref(&leather)));
        assert!(viable(&plan, 9, &[leather.clone(), leather]));

        let later = Requirement {
            max_depth: Some(6),
            ..wanted
        };
        let mixed = multiplicity_query(vec![wanted, later], 24);
        let mixed_plan = QueryPlan::analyze(&mixed);
        assert!(mixed_plan.closed_multiplicities.is_empty());
        // A later deadline must not be pulled forward by a similar earlier slot.
        assert!(viable(&mixed_plan, 4, &capped[..1]));
        assert!(mixed.matches(&multiplicity_world(&capped)));
    }

    #[test]
    fn closed_multiplicities_exclude_or_members_and_optional_sum_members() {
        let wand = early_wand(UpgradeRequirement::AtLeast(2));
        let ring = Requirement {
            max_depth: Some(4),
            ..requirement(ItemKind::Ring, UpgradeRequirement::Any)
        };
        let alternative = multiplicity_query(
            vec![
                wand,
                Requirement {
                    alternative_group: Some(1),
                    ..wand
                },
                Requirement {
                    alternative_group: Some(1),
                    ..ring
                },
            ],
            4,
        );
        let plan = QueryPlan::analyze(&alternative);
        assert!(plan.closed_multiplicities.is_empty());
        let items = [
            item(ItemId::WandFrost, 2, 2, ItemSource::Heap),
            item(ItemId::RingMight, 2, 3, ItemSource::Heap),
        ];
        assert!(viable(&plan, 4, &items));
        assert!(
            alternative.matches(&multiplicity_world(&items)),
            "OR can use the ring"
        );

        let optional = Requirement {
            level_sum: Some(crate::query::LevelSum {
                group: 1,
                minimum_total: 3,
            }),
            ..ring
        };
        let sums = multiplicity_query(vec![optional; 2], 4);
        let plan = QueryPlan::analyze(&sums);
        assert!(plan.closed_multiplicities.is_empty());
        assert!(viable(&plan, 4, &items[1..]));
        assert!(
            sums.matches(&multiplicity_world(&items[1..])),
            "one +2 ring fills the sum"
        );

        let mixed = multiplicity_query(vec![wand, wand, optional, optional], 4);
        let plan = QueryPlan::analyze(&mixed);
        assert_eq!(plan.closed_multiplicities, vec![(0, 2)]);
        let enough = [items[0].clone(), items[0].clone(), items[1].clone()];
        assert!(viable(&plan, 4, &enough));
        assert!(mixed.matches(&multiplicity_world(&enough)));
        assert!(
            !viable(&plan, 4, &items),
            "optional groups do not suppress mandatory checks"
        );
    }

    #[test]
    fn closed_multiplicities_do_not_consume_future_quests_or_pending_vault_capacity() {
        let future = requirement(ItemKind::Wand, UpgradeRequirement::AtLeast(2));
        let query = multiplicity_query(vec![future; 2], 24);
        let plan = QueryPlan::analyze(&query);
        assert_ne!(plan.slots[0][0].quests, 0);
        assert!(plan.closed_multiplicities.is_empty());
        assert!(viable(
            &plan,
            4,
            &[item(ItemId::WandFrost, 2, 2, ItemSource::Heap)]
        ));

        let early = early_wand(UpgradeRequirement::AtLeast(2));
        let treasure = Requirement {
            item: Some(ItemId::RunicBlade),
            source: Some(ItemSource::VaultTreasure),
            ..requirement(ItemKind::Weapon, UpgradeRequirement::Exact(4))
        };
        let query = multiplicity_query(vec![early, early, treasure], 24);
        let plan = QueryPlan::analyze(&query);
        let original = without_closed_multiplicities(&plan);
        assert_eq!(plan.closed_multiplicities, vec![(0, 2)]);
        assert!(plan.slots[2][0].vault);
        let short = [
            item(ItemId::WandFrost, 2, 2, ItemSource::Heap),
            item(ItemId::RingMight, 2, 18, ItemSource::ImpReward),
        ];
        assert!(original.viable_with_pending_vault(19, &short, QuestSummary::default(), Some(18)));
        assert!(!plan.viable_with_pending_vault(19, &short, QuestSummary::default(), Some(18)));
        let enough = [short[0].clone(), short[0].clone(), short[1].clone()];
        assert!(plan.viable_with_pending_vault(19, &enough, QuestSummary::default(), Some(18)));
        assert!(
            !viable(&plan, 19, &enough),
            "without pending treasure the Imp already missed"
        );
    }

    #[test]
    #[allow(clippy::too_many_lines)] // Compare each predicate against a distinct positive world.
    fn closed_multiplicities_do_not_merge_different_item_predicates() {
        fn positive_pair(first: Requirement, second: Requirement, items: &[WorldItem; 2]) {
            let query = multiplicity_query(vec![first, second], 4);
            let plan = QueryPlan::analyze(&query);
            assert!(plan.closed_multiplicities.is_empty());
            assert!(
                query.matches(&multiplicity_world(items)),
                "validated positive metadata control"
            );
            assert!(viable(&plan, 4, items));
        }
        let base = Requirement {
            source: Some(ItemSource::Heap),
            ..early_wand(UpgradeRequirement::Any)
        };
        positive_pair(
            base,
            Requirement {
                source: Some(ItemSource::Chest),
                ..base
            },
            &[
                item(ItemId::WandFrost, 1, 2, ItemSource::Heap),
                item(ItemId::WandFrost, 1, 2, ItemSource::Chest),
            ],
        );
        positive_pair(
            Requirement {
                item: Some(ItemId::WandFrost),
                ..base
            },
            Requirement {
                item: Some(ItemId::WandLightning),
                ..base
            },
            &[
                item(ItemId::WandFrost, 1, 2, ItemSource::Heap),
                item(ItemId::WandLightning, 1, 2, ItemSource::Heap),
            ],
        );
        positive_pair(
            Requirement {
                upgrade: UpgradeRequirement::Exact(1),
                ..base
            },
            Requirement {
                upgrade: UpgradeRequirement::AtLeast(1),
                ..base
            },
            &[
                item(ItemId::WandFrost, 1, 2, ItemSource::Heap),
                item(ItemId::WandLightning, 2, 2, ItemSource::Heap),
            ],
        );
        positive_pair(
            Requirement {
                require_uncursed: true,
                ..base
            },
            base,
            &[
                item(ItemId::WandFrost, 1, 2, ItemSource::Heap),
                WorldItem {
                    cursed: true,
                    ..item(ItemId::WandFrost, 1, 2, ItemSource::Heap)
                },
            ],
        );
        let armor = Requirement {
            kind: ItemKind::Armor,
            ..base
        };
        positive_pair(
            Requirement {
                tier: TierRequirement::Exact(2),
                ..armor
            },
            Requirement {
                tier: TierRequirement::Exact(3),
                ..armor
            },
            &[
                item(ItemId::LeatherArmor, 1, 2, ItemSource::Heap),
                item(ItemId::MailArmor, 1, 2, ItemSource::Heap),
            ],
        );
        let weapon = Requirement {
            kind: ItemKind::Weapon,
            ..base
        };
        let grim = Effect::Weapon(WeaponEffect::Grim);
        let lucky = Effect::Weapon(WeaponEffect::Lucky);
        positive_pair(
            Requirement {
                effect: EffectRequirement::exactly(grim),
                ..weapon
            },
            Requirement {
                effect: EffectRequirement::exactly(lucky),
                ..weapon
            },
            &[
                WorldItem {
                    effect: Some(grim),
                    ..item(ItemId::Shortsword, 1, 2, ItemSource::Heap)
                },
                WorldItem {
                    effect: Some(lucky),
                    ..item(ItemId::Shortsword, 1, 2, ItemSource::Heap)
                },
            ],
        );
        positive_pair(
            Requirement {
                weapon_category: Some(crate::catalog::WeaponCategory::Melee),
                ..weapon
            },
            Requirement {
                weapon_category: Some(crate::catalog::WeaponCategory::Thrown),
                ..weapon
            },
            &[
                item(ItemId::Shortsword, 1, 2, ItemSource::Heap),
                item(ItemId::ThrowingKnife, 1, 2, ItemSource::Heap),
            ],
        );
    }

    #[test]
    fn closed_multiplicities_remain_optimistic_about_identity_and_accessibility() {
        // Valid identity stacks require bare copies: source/upgrade/item filters
        // on both members would be rejected as two constrained anchor units.
        let bare = early_wand(UpgradeRequirement::Any);
        let identity = Requirement {
            identity_group: Some(1),
            ..bare
        };
        let query = multiplicity_query(vec![identity; 2], 4);
        let plan = QueryPlan::analyze(&query);
        assert_eq!(plan.closed_multiplicities, vec![(0, 2)]);
        let different_ids = [
            item(ItemId::WandFrost, 1, 2, ItemSource::Heap),
            item(ItemId::WandLightning, 1, 2, ItemSource::Heap),
        ];
        assert!(viable(&plan, 4, &different_ids));
        assert!(!query.matches(&multiplicity_world(&different_ids)));

        let query = multiplicity_query(vec![bare; 2], 4);
        let plan = QueryPlan::analyze(&query);
        for (first, second) in [
            (
                Accessibility::Choice {
                    group: 7,
                    option: 0,
                },
                Accessibility::Choice {
                    group: 7,
                    option: 1,
                },
            ),
            (
                Accessibility::Scenarios { group: 7, mask: 1 },
                Accessibility::Scenarios { group: 7, mask: 2 },
            ),
        ] {
            let incompatible = [
                WorldItem {
                    accessibility: first,
                    ..different_ids[0].clone()
                },
                WorldItem {
                    accessibility: second,
                    ..different_ids[0].clone()
                },
            ];
            assert!(viable(&plan, 4, &incompatible));
            assert!(!query.matches(&multiplicity_world(&incompatible)));
        }
    }

    #[test]
    fn closed_multiplicity_rejection_implies_nonmatch_for_bounded_complete_worlds() {
        // 156 ordered vectors of length0..3 over five records, for each of two
        // predicates: 312 small final-matcher checks, no generation or RNG.
        // Repetition deliberately creates equal values at distinct item indices.
        // At completed6 all records are in the prefix, but depth5 exceeds cap4.
        let pool = [
            item(ItemId::WandFrost, 1, 2, ItemSource::Heap),
            item(ItemId::WandLightning, 2, 4, ItemSource::Heap),
            item(ItemId::WandFrost, 1, 2, ItemSource::Chest),
            item(ItemId::RingMight, 1, 2, ItemSource::Heap),
            item(ItemId::WandFrost, 1, 5, ItemSource::Heap),
        ];
        let mut newly_rejected = 0;
        let mut retained_matches = 0;
        for upgrade in [UpgradeRequirement::Exact(1), UpgradeRequirement::AtLeast(1)] {
            let wanted = Requirement {
                source: Some(ItemSource::Heap),
                ..early_wand(upgrade)
            };
            let query = multiplicity_query(vec![wanted; 2], 6);
            let plan = QueryPlan::analyze(&query);
            let original = without_closed_multiplicities(&plan);
            assert_eq!(plan.closed_multiplicities, vec![(0, 2)]);
            for length in 0_u32..=3 {
                for mut encoded in 0..pool.len().pow(length) {
                    let mut items = Vec::new();
                    for _ in 0..length {
                        items.push(pool[encoded % pool.len()].clone());
                        encoded /= pool.len();
                    }
                    let before = viable(&original, 6, &items);
                    let after = viable(&plan, 6, &items);
                    let matches = query.matches(&multiplicity_world(&items));
                    assert!(
                        !after || before,
                        "adding the guard cannot revive a rejection"
                    );
                    if !after {
                        assert!(!matches, "unsound rejection: {upgrade:?}, {items:?}");
                    }
                    newly_rejected += usize::from(before && !after);
                    retained_matches += usize::from(after && matches);
                }
            }
        }
        assert!(
            newly_rejected > 0,
            "exercise a real improvement over the old planner"
        );
        assert!(
            retained_matches > 0,
            "property must include matching worlds"
        );
    }

    const TRUE_SIX_WAND_SEED: u64 = 4_689_753_124_998;
    const OBSERVED_EARLY_WAND_MISSES: [u64; 2] = [4_302_629_544_091, 4_830_115_600_014];

    type MultiplicityProductionRecord = (
        crate::auto_trinkets::SeedRecipe,
        crate::model::GeneratedWorld,
        crate::query::ScoutMatches,
    );

    fn multiplicity_production_records(
        found: Vec<Option<crate::auto_trinkets::TrinketSearchMatch>>,
        query: &SearchQuery,
    ) -> Vec<Option<MultiplicityProductionRecord>> {
        found
            .into_iter()
            .map(|result| {
                result.map(|m| {
                    let selection = crate::query::scout_matches(&m.world, query);
                    (m.recipe, m.world, selection)
                })
            })
            .collect()
    }

    fn six_wand_query(exact: bool, auto: bool) -> SearchQuery {
        let mut query = crate::json_query::decode(
            r#"{"max_depth":24,"auto_apply_trinket":false,"requirements":[
                {"kind":"wand","upgrade":{"at_least":3}},
                {"kind":"wand","upgrade":{"at_least":2},"max_depth":4},
                {"kind":"wand","upgrade":{"at_least":2},"max_depth":4},
                {"kind":"wand","upgrade":{"at_least":2}},
                {"kind":"wand","upgrade":{"at_least":4}},
                {"item":"wondrous_resin"}
            ]}"#,
        )
        .unwrap();
        query.auto_apply_trinket = auto;
        if exact {
            for requirement in &mut query.requirements {
                if let UpgradeRequirement::AtLeast(value) = requirement.upgrade {
                    requirement.upgrade = UpgradeRequirement::Exact(value);
                }
            }
        }
        query.validate().unwrap();
        query
    }

    struct MultiplicityProductionCase {
        label: String,
        query: SearchQuery,
        // None means no outcome is asserted for the previously observed seed
        // under these DIFFERENT conditions, while differential parity is required.
        known_match: Option<bool>,
    }

    fn multiplicity_production_cases() -> Vec<MultiplicityProductionCase> {
        let mut cases = Vec::new();
        for exact in [false, true] {
            for auto in [false, true] {
                cases.push(MultiplicityProductionCase {
                    label: format!("six/exact={exact}/auto={auto}"),
                    query: six_wand_query(exact, auto),
                    known_match: Some(true),
                });
            }
            let mut selected = six_wand_query(exact, true);
            selected.requirements[5].select_trinket = true;
            cases.push(MultiplicityProductionCase {
                label: format!("selected-Resin/exact={exact}"),
                query: selected,
                known_match: None,
            });
            let mut at_imp = six_wand_query(exact, false);
            at_imp.max_depth = 18;
            cases.push(MultiplicityProductionCase {
                label: format!("target18/exact={exact}"),
                query: at_imp,
                known_match: Some(true),
            });
        }
        for (cap, exact, expected) in [(17, false, false), (18, true, true)] {
            let mut query = six_wand_query(exact, false);
            query.requirements[4].max_depth = Some(cap);
            cases.push(MultiplicityProductionCase {
                label: format!("Imp-cap{cap}"),
                query,
                known_match: Some(expected),
            });
        }
        let mut earlier = six_wand_query(false, false);
        earlier.requirements[1].max_depth = Some(3);
        earlier.requirements[2].max_depth = Some(3);
        cases.push(MultiplicityProductionCase {
            label: "early-cap3".into(),
            query: earlier,
            known_match: None,
        });
        for (label, challenges, exact) in [
            ("darkness", crate::challenges::Challenges::DARKNESS, false),
            (
                "level-generation",
                crate::challenges::Challenges::LEVEL_GENERATION,
                true,
            ),
        ] {
            let mut query = six_wand_query(exact, false);
            query.challenges = challenges;
            cases.push(MultiplicityProductionCase {
                label: label.into(),
                query,
                known_match: None,
            });
        }
        for case in &cases {
            case.query.validate().unwrap();
        }
        cases
    }

    fn multiplicity_production_seeds() -> Vec<crate::seed::DungeonSeed> {
        use crate::seed::DungeonSeed;
        let known = DungeonSeed::new(TRUE_SIX_WAND_SEED).unwrap();
        let mut seeds: Vec<_> = (0..6_u64)
            .map(|index| {
                DungeonSeed::new(
                    (812_345_678_901 + index * 3_355_211_884_971) % crate::seed::TOTAL_SEEDS,
                )
                .unwrap()
            })
            .collect();
        seeds.insert(3, known); // Retained positive is lane3 of the first SIMD batch.
        seeds.extend(OBSERVED_EARLY_WAND_MISSES.map(|seed| DungeonSeed::new(seed).unwrap()));
        assert_eq!(seeds.len(), 9); // Two four-lane batches plus one scalar remainder.
        assert_eq!(seeds[3], known);
        assert!(crate::trinkets::initial_offers(known).contains(&ItemId::WondrousResin));
        seeds
    }

    #[test]
    #[ignore = "extended seed sweep; run with --release --ignored"]
    fn closed_multiplicities_preserve_production_worlds_recipes_and_selected_witnesses() {
        assert!(
            check_closed_multiplicity_production(
                &multiplicity_production_seeds(),
                &multiplicity_production_cases(),
            ) >= 4
        );
    }

    #[test]
    fn closed_multiplicities_preserve_bounded_production_fixtures() {
        use crate::seed::DungeonSeed;
        // Keep a positive in lane3, observed misses and a scalar tail. These
        // four cases cover exact/minimum upgrades, selected Resin and an Imp cap.
        let seeds = [
            OBSERVED_EARLY_WAND_MISSES[0],
            0,
            OBSERVED_EARLY_WAND_MISSES[1],
            TRUE_SIX_WAND_SEED,
            1,
        ]
        .map(|value| DungeonSeed::new(value).unwrap());
        let cases: Vec<_> = multiplicity_production_cases()
            .into_iter()
            .enumerate()
            .filter_map(|(index, case)| [0, 4, 2, 8].contains(&index).then_some(case))
            .collect();
        assert!(check_closed_multiplicity_production(&seeds, &cases) >= 2);
    }

    #[allow(clippy::too_many_lines)] // Keep reference, raw worlds, and consumer equivalence together.
    fn check_closed_multiplicity_production(
        seeds: &[crate::seed::DungeonSeed],
        cases: &[MultiplicityProductionCase],
    ) -> usize {
        use crate::auto_trinkets::{self, SeedRecipe};
        use crate::main_world::CanonicalMainWorldGenerator;
        use crate::search::{FloorGate, WorldGenerator};
        use crate::seed::DungeonSeed;
        let known = DungeonSeed::new(TRUE_SIX_WAND_SEED).unwrap();
        let known_index = seeds.iter().position(|&seed| seed == known).unwrap();
        let mut known_positives = 0;
        let mut retained_worlds = 0;
        for case in cases {
            let query = &case.query;
            let after = QueryPlan::analyze(query);
            let before = without_closed_multiplicities(&after);
            assert!(!after.closed_multiplicities.is_empty());
            assert_eq!(after.generation_depth(), before.generation_depth());
            assert_eq!(after.wants_vault_treasure(), before.wants_vault_treasure());
            assert_eq!(
                after
                    .deferred_vault_plan(after.generation_depth())
                    .is_some(),
                before
                    .deferred_vault_plan(before.generation_depth())
                    .is_some()
            );
            assert!(
                !auto_trinkets::enabled(query),
                "named Resin disables automatic replay"
            );
            assert_eq!(
                after.selected_trinket(known),
                query.requirements[5]
                    .select_trinket
                    .then_some(ItemId::WondrousResin)
            );
            let generator = CanonicalMainWorldGenerator::with_challenges(query.challenges);
            let before_worlds =
                generator.generate_batch_gated(seeds, before.generation_depth(), &before);
            let after_worlds =
                generator.generate_batch_gated(seeds, after.generation_depth(), &after);
            assert_eq!(before_worlds.len(), seeds.len());
            assert_eq!(after_worlds.len(), seeds.len());
            for (index, (actual, original)) in after_worlds.iter().zip(&before_worlds).enumerate() {
                match (actual, original) {
                    (Some(actual), Some(original)) => {
                        assert_eq!(actual, original, "{}, seed {}", case.label, seeds[index]);
                        assert_eq!(
                            crate::query::scout_matches(actual, query),
                            crate::query::scout_matches(original, query)
                        );
                        assert_eq!(actual.seed, seeds[index]);
                        retained_worlds += 1;
                    }
                    (None, Some(original)) => assert!(
                        !query.matches(original),
                        "new rejection lost a match: {}, seed {}",
                        case.label,
                        seeds[index]
                    ),
                    (Some(_), None) => {
                        panic!("new guard revived an abandoned world: {}", case.label)
                    }
                    (None, None) => {}
                }
            }
            let actual = multiplicity_production_records(
                auto_trinkets::search_batch(&generator, query, &after, seeds),
                query,
            );
            let original = multiplicity_production_records(
                auto_trinkets::search_batch(&generator, query, &before, seeds),
                query,
            );
            assert_eq!(actual, original, "search {}", case.label);
            if let Some(expected) = case.known_match {
                assert_eq!(
                    actual[known_index].is_some(),
                    expected,
                    "known fixture {}",
                    case.label
                );
                if expected {
                    let matched = actual[known_index].as_ref().unwrap();
                    assert_eq!(
                        matched.0,
                        SeedRecipe {
                            seed: known,
                            trinket: None
                        }
                    );
                    assert_eq!(matched.2.matched_requirements, 6);
                    assert_eq!(matched.2.total_requirements, 6);
                    known_positives += 1;
                }
            }
            let before_ready = before_worlds
                .into_iter()
                .flatten()
                .filter(|w| query.matches(w))
                .collect();
            let after_ready = after_worlds
                .into_iter()
                .flatten()
                .filter(|w| query.matches(w))
                .collect();
            let before_finished = multiplicity_production_records(
                auto_trinkets::finish_matches(&generator, query, &before, before_ready)
                    .into_iter()
                    .map(Some)
                    .collect(),
                query,
            );
            let after_finished = multiplicity_production_records(
                auto_trinkets::finish_matches(&generator, query, &after, after_ready)
                    .into_iter()
                    .map(Some)
                    .collect(),
                query,
            );
            assert_eq!(after_finished, before_finished, "finish {}", case.label);
            assert_eq!(
                after_finished.into_iter().flatten().collect::<Vec<_>>(),
                actual.into_iter().flatten().collect::<Vec<_>>(),
                "finish/search {}",
                case.label
            );

            // Bound replay work to four distinct seeds, including the true seed.
            // None, first real offer, and Resin-if-offered are three separate
            // calls: never collapse conflicting choices under the same seed key.
            for choice in 0..3 {
                let saved: Vec<_> = seeds
                    .iter()
                    .take(4)
                    .map(|&seed| {
                        let offers = crate::trinkets::initial_offers(seed);
                        let trinket = match choice {
                            0 => None,
                            1 => Some(offers[0]),
                            _ => Some(if offers.contains(&ItemId::WondrousResin) {
                                ItemId::WondrousResin
                            } else {
                                offers[0]
                            }),
                        };
                        SeedRecipe { seed, trinket }
                    })
                    .collect();
                let filtered = multiplicity_production_records(
                    auto_trinkets::filter_batch(&generator, query, &after, &saved),
                    query,
                );
                assert_eq!(
                    filtered,
                    multiplicity_production_records(
                        auto_trinkets::filter_batch(&generator, query, &before, &saved),
                        query
                    ),
                    "filter {}/{choice}",
                    case.label
                );
                for (saved, matched) in saved.iter().zip(&filtered) {
                    if let Some(matched) = matched {
                        assert_eq!(matched.0, *saved, "named query retains forced recipe");
                    }
                }
                let mut base = query.clone();
                base.requirements[3].upgrade = UpgradeRequirement::Any;
                base.validate().unwrap();
                assert_ne!(&base, query);
                let refined = multiplicity_production_records(
                    auto_trinkets::refine_batch(&generator, query, &after, &base, &saved),
                    query,
                );
                assert_eq!(
                    refined,
                    multiplicity_production_records(
                        auto_trinkets::refine_batch(&generator, query, &before, &base, &saved),
                        query
                    ),
                    "refine {}/{choice}",
                    case.label
                );
                assert_eq!(
                    refined, filtered,
                    "named query has no automatic retry/removal"
                );
                // Forced or explicit Resin outcomes compare only identical
                // selected recipes. No positive or None-world equivalence assumed.
            }
        }
        assert!(known_positives > 0 && retained_worlds >= known_positives);
        known_positives
    }

    #[test]
    fn closed_multiplicities_prune_observed_real_floor_four_misses_nonvacuously() {
        use crate::main_world::generate_main_world_gated;
        use crate::search::FloorGate;
        use crate::seed::DungeonSeed;
        let mut newly_rejected = 0;
        for exact in [false, true] {
            let query = six_wand_query(exact, false);
            let after = QueryPlan::analyze(&query);
            let before = without_closed_multiplicities(&after);
            assert_eq!(after.closed_multiplicities, vec![(1, 2)]);
            for value in OBSERVED_EARLY_WAND_MISSES {
                let seed = DungeonSeed::new(value).unwrap();
                assert_eq!(before.selected_trinket(seed), None);
                assert_eq!(after.selected_trinket(seed), None);
                // At target4 the normal generator does not invoke the final-floor
                // callback. Obtain the real prefix under unchanged accepted gates,
                // then ask precisely the callback the full24 path reaches at4.
                let prefix = generate_main_world_gated(seed, 4, &before)
                    .unwrap()
                    .expect("observed run-init and floors1..3 survive");
                assert_eq!(
                    prefix
                        .items
                        .iter()
                        .filter(|item| { item.depth <= 4 && query.requirements[1].matches(item) })
                        .count(),
                    1,
                    "observed single early wand, seed {seed}"
                );
                assert!(before.continue_after_floor(4, &prefix.items, &prefix.quests));
                assert!(!after.continue_after_floor(4, &prefix.items, &prefix.quests));
                newly_rejected += 1;
                let original = generate_main_world_gated(seed, 24, &before).unwrap();
                let actual = generate_main_world_gated(seed, 24, &after).unwrap();
                assert!(actual.is_none());
                assert!(!original.as_ref().is_some_and(|world| query.matches(world)));
                // In the saved observer, old final results were also None (at7
                // and9). The explicit real-prefix assertion proves earlier work
                // avoidance that final Option equality alone cannot demonstrate.
            }
        }
        assert!(newly_rejected >= 1);
    }

    #[test]
    #[allow(clippy::too_many_lines)] // Keep the two fixtures and complete consumer comparisons together.
    fn closed_multiplicities_active_auto_preserve_required_choices_and_replays() {
        use crate::auto_trinkets::{self, SeedRecipe};
        use crate::main_world::{CanonicalMainWorldGenerator, generate_main_world_with_trinket};
        use crate::search::{FloorGate, WorldGenerator};
        use crate::seed::DungeonSeed;

        let query = crate::json_query::decode(
            r#"{"max_depth":24,"auto_apply_trinket":true,"requirements":[
                {"kind":"wand","upgrade":{"at_least":2},"max_depth":4},
                {"kind":"wand","upgrade":{"at_least":2},"max_depth":4},
                {"kind":"wand","upgrade":{"at_least":3}}
            ]}"#,
        )
        .unwrap();
        query.validate().unwrap();
        assert!(auto_trinkets::enabled(&query));
        let after = QueryPlan::analyze(&query);
        let before = without_closed_multiplicities(&after);
        assert_eq!(after.closed_multiplicities, vec![(0, 2)]);
        assert_eq!(after.generation_depth(), before.generation_depth());
        assert_eq!(after.wants_vault_treasure(), before.wants_vault_treasure());
        assert_eq!(
            after
                .deferred_vault_plan(after.generation_depth())
                .is_some(),
            before
                .deferred_vault_plan(before.generation_depth())
                .is_some()
        );
        let seeds =
            [695_488_469_679, 4_689_753_124_998].map(|value| DungeonSeed::new(value).unwrap());
        assert_eq!(
            after.selected_trinket(seeds[0]),
            Some(ItemId::CrackedSpyglass)
        );
        let generator = CanonicalMainWorldGenerator::with_challenges(query.challenges);

        // The reference differs only in the repeated-slot guard. Compare the
        // complete returned worlds before automatic removal/retry as well.
        let before_worlds =
            generator.generate_batch_gated(&seeds, before.generation_depth(), &before);
        let after_worlds = generator.generate_batch_gated(&seeds, after.generation_depth(), &after);
        assert_eq!(after_worlds.len(), seeds.len());
        assert_eq!(after_worlds, before_worlds);
        for (seed, world) in seeds.iter().zip(&after_worlds) {
            let world = world.as_ref().expect("both automatic fixtures survive");
            assert_eq!(world.seed, *seed);
            assert!(query.matches(world));
        }

        // Also compare the complete requested 24-floor returned world rather
        // than calling a shortened search horizon a complete dungeon.
        let before_full = generator.generate_batch_gated(&seeds, query.max_depth, &before);
        let after_full = generator.generate_batch_gated(&seeds, query.max_depth, &after);
        assert_eq!(after_full, before_full);
        assert_eq!(after_full.len(), seeds.len());
        for world in after_full.iter().flatten() {
            assert_eq!(world.feelings.last().unwrap().depth, query.max_depth);
            assert!(query.matches(world));
        }
        assert!(after_full.iter().all(Option::is_some));

        let original = multiplicity_production_records(
            auto_trinkets::search_batch(&generator, &query, &before, &seeds),
            &query,
        );
        let found = multiplicity_production_records(
            auto_trinkets::search_batch(&generator, &query, &after, &seeds),
            &query,
        );
        assert_eq!(found, original);
        for (index, wanted) in [Some(ItemId::CrackedSpyglass), None]
            .into_iter()
            .enumerate()
        {
            let (recipe, world, scout) = found[index].as_ref().expect("known matching fixture");
            assert_eq!(
                *recipe,
                SeedRecipe {
                    seed: seeds[index],
                    trinket: wanted
                }
            );
            assert!(query.matches(world));
            assert_eq!(scout.matched_requirements, 3);
            assert_eq!(scout.total_requirements, 3);
        }

        let before_finished = multiplicity_production_records(
            auto_trinkets::finish_matches(
                &generator,
                &query,
                &before,
                before_worlds.into_iter().flatten().collect(),
            )
            .into_iter()
            .map(Some)
            .collect(),
            &query,
        );
        let after_finished = multiplicity_production_records(
            auto_trinkets::finish_matches(
                &generator,
                &query,
                &after,
                after_worlds.into_iter().flatten().collect(),
            )
            .into_iter()
            .map(Some)
            .collect(),
            &query,
        );
        assert_eq!(after_finished, before_finished);
        assert_eq!(after_finished, found);

        // Remove an actual mandatory singleton. This is a weaker fixed-world
        // predicate; no optional-sum capacity or OR membership changes. Automatic
        // ranking can change with a query, so this test does not infer the separate
        // SearchQuery::continues/covered-range contract from that implication.
        let mut base = query.clone();
        let removed = base.requirements.remove(0);
        assert!(removed.level_sum.is_none() && removed.alternative_group.is_none());
        base.validate().unwrap();
        assert_ne!(base, query);
        for (_, world, _) in found.iter().flatten() {
            assert!(base.matches(world));
        }

        // Each batch has distinct seeds. Conflicting choices for one seed must
        // stay in separate calls because RecipeGate is keyed by seed value.
        for choice in 0..5 {
            let recipes: Vec<_> = seeds
                .iter()
                .map(|&seed| {
                    let offers = crate::trinkets::initial_offers(seed);
                    let trinket = (choice != 0).then(|| offers[choice - 1]);
                    if let Some(id) = trinket {
                        assert_eq!(
                            crate::trinkets::parse_offered(
                                seed,
                                crate::catalog::item(id).stable_id
                            )
                            .unwrap(),
                            id
                        );
                    }
                    SeedRecipe { seed, trinket }
                })
                .collect();
            let old_filtered = multiplicity_production_records(
                auto_trinkets::filter_batch(&generator, &query, &before, &recipes),
                &query,
            );
            let filtered = multiplicity_production_records(
                auto_trinkets::filter_batch(&generator, &query, &after, &recipes),
                &query,
            );
            assert_eq!(filtered, old_filtered, "filter choice {choice}");
            let old_refined = multiplicity_production_records(
                auto_trinkets::refine_batch(&generator, &query, &before, &base, &recipes),
                &query,
            );
            let refined = multiplicity_production_records(
                auto_trinkets::refine_batch(&generator, &query, &after, &base, &recipes),
                &query,
            );
            assert_eq!(refined, old_refined, "refine choice {choice}");
            if choice == 0 {
                assert!(
                    filtered[0].is_none(),
                    "Spyglass is necessary for this target"
                );
                assert_eq!(
                    filtered[1], found[1],
                    "known six-wand seed needs no trinket"
                );
                assert_eq!(
                    refined, found,
                    "refinement restores the necessary automatic choice"
                );
            }
            for (saved, result) in recipes
                .iter()
                .zip(&filtered)
                .chain(recipes.iter().zip(&refined))
            {
                let Some((recipe, world, scout)) = result else {
                    continue;
                };
                assert_eq!(recipe.seed, saved.seed);
                if let Some(id) = recipe.trinket {
                    assert_eq!(
                        crate::trinkets::parse_offered(
                            recipe.seed,
                            crate::catalog::item(id).stable_id
                        )
                        .unwrap(),
                        id
                    );
                    assert!(
                        saved.trinket == Some(id)
                            || (saved.trinket.is_none()
                                && after.selected_trinket(saved.seed) == Some(id))
                    );
                }
                assert!(query.matches(world));
                assert_eq!(scout.matched_requirements, 3);
                let replay = generate_main_world_with_trinket(
                    recipe.seed,
                    query.max_depth,
                    query.challenges,
                    recipe.trinket,
                )
                .unwrap();
                assert!(
                    query.matches(&replay),
                    "returned recipe survives complete canonical replay"
                );
                assert!(base.matches(&replay));
            }
        }
    }
}

#[cfg(test)]
mod trinket_preflight_tests {
    use super::*;
    use crate::auto_trinkets::{self, SeedRecipe, TrinketSearchMatch};
    use crate::catalog::{ITEMS, ItemId};
    use crate::challenges::Challenges;
    use crate::main_world::CanonicalMainWorldGenerator;
    use crate::model::GeneratedWorld;
    use crate::run::RunState;
    use crate::search::WorldGenerator;
    use crate::seed::DungeonSeed;

    struct WithoutInitGate<'a, G>(&'a G);
    struct PreserveFloorGate<'a>(&'a dyn FloorGate);

    impl FloorGate for PreserveFloorGate<'_> {
        fn selected_trinket(&self, seed: DungeonSeed) -> Option<ItemId> {
            self.0.selected_trinket(seed)
        }
        fn continue_after_floor(
            &self,
            depth: u8,
            items: &[WorldItem],
            quests: &QuestSummary,
        ) -> bool {
            self.0.continue_after_floor(depth, items, quests)
        }
        fn wants_vault_treasure(&self) -> bool {
            self.0.wants_vault_treasure()
        }
        // Intentionally retain only the new hook's default true behavior.
    }

    impl<G: WorldGenerator> WorldGenerator for WithoutInitGate<'_, G> {
        fn generate(&self, seed: DungeonSeed, depth: u8) -> GeneratedWorld {
            self.0.generate(seed, depth)
        }
        fn generate_batch_gated(
            &self,
            seeds: &[DungeonSeed],
            depth: u8,
            gate: &dyn FloorGate,
        ) -> Vec<Option<GeneratedWorld>> {
            self.0
                .generate_batch_gated(seeds, depth, &PreserveFloorGate(gate))
        }
    }

    fn comparable(
        matches: Vec<Option<TrinketSearchMatch>>,
    ) -> Vec<Option<(SeedRecipe, GeneratedWorld)>> {
        matches
            .into_iter()
            .map(|matched| matched.map(|m| (m.recipe, m.world)))
            .collect()
    }

    fn seeds() -> Vec<DungeonSeed> {
        (0..12_u64)
            .map(|index| {
                DungeonSeed::new(
                    (812_345_678_901 + index * 3_355_211_884_971) % crate::seed::TOTAL_SEEDS,
                )
                .unwrap()
            })
            .collect()
    }

    fn compare_paths(query: &SearchQuery, seeds: &[DungeonSeed], replay: bool) -> usize {
        query.validate().unwrap();
        let plan = QueryPlan::analyze(query);
        let generator = CanonicalMainWorldGenerator::with_challenges(query.challenges);
        let reference = WithoutInitGate(&generator);
        assert_eq!(
            comparable(auto_trinkets::search_batch(&generator, query, &plan, seeds)),
            comparable(auto_trinkets::search_batch(&reference, query, &plan, seeds)),
            "{query:?}"
        );
        let actual = generator.generate_batch_gated(seeds, plan.generation_depth(), &plan);
        let expected = reference.generate_batch_gated(seeds, plan.generation_depth(), &plan);
        assert_eq!(actual.len(), seeds.len());
        assert_eq!(expected.len(), seeds.len());
        let mut pruned = 0;
        for (actual, expected) in actual.iter().zip(&expected) {
            match (actual, expected) {
                (Some(actual), Some(expected)) => assert_eq!(actual, expected),
                (None, Some(world)) => {
                    assert!(!query.matches(world));
                    pruned += 1;
                }
                (None, None) => {}
                (Some(_), None) => panic!("new gate cannot resurrect an abandoned world"),
            }
        }
        let finished = |worlds: Vec<Option<GeneratedWorld>>| {
            worlds
                .into_iter()
                .flatten()
                .filter(|world| query.matches(world))
                .collect()
        };
        let actual = auto_trinkets::finish_matches(&generator, query, &plan, finished(actual));
        let expected = auto_trinkets::finish_matches(&reference, query, &plan, finished(expected));
        assert_eq!(
            comparable(actual.into_iter().map(Some).collect()),
            comparable(expected.into_iter().map(Some).collect())
        );
        if replay {
            for choice in [None, Some(0), Some(3)] {
                let recipes: Vec<_> = seeds
                    .iter()
                    .map(|&seed| SeedRecipe {
                        seed,
                        trinket: choice.map(|index| crate::trinkets::trinket_order(seed)[index]),
                    })
                    .collect();
                assert_eq!(
                    comparable(auto_trinkets::filter_batch(
                        &generator, query, &plan, &recipes
                    )),
                    comparable(auto_trinkets::filter_batch(
                        &reference, query, &plan, &recipes
                    ))
                );
            }
        }
        pruned
    }

    #[test]
    #[ignore = "extended seed sweep; run with --release --ignored"]
    fn every_trinket_identity_preserves_worlds_recipes_and_batch_positions() {
        check_trinket_identities(&seeds());
    }

    #[test]
    fn trinket_identity_smoke_preserves_worlds_recipes_and_batch_positions() {
        check_trinket_identities(&seeds()[..2]);
    }

    fn check_trinket_identities(seeds: &[DungeonSeed]) {
        let (plain, saved) = seeds.split_at(seeds.len().min(3));
        let mut pruned = 0;
        for item in ITEMS.iter().filter(|item| item.kind == ItemKind::Trinket) {
            for depth in [1, 3] {
                let query = crate::json_query::decode(&format!(
                    "{{\"max_depth\":{depth},\"auto_apply_trinket\":false,\"requirements\":[{{\"item\":\"{}\"}}]}}",
                    item.stable_id)).unwrap();
                pruned += compare_paths(&query, plain, false);
                if !saved.is_empty() {
                    pruned += compare_paths(&query, saved, true);
                }
            }
        }
        assert!(pruned > 0, "exercise newly abandoned worlds");
    }

    #[test]
    #[ignore = "extended seed sweep; run with --release --ignored"]
    fn trinket_preflight_keeps_alternatives_selection_and_challenge_semantics() {
        check_trinket_preflight_semantics(
            &seeds(),
            &[3, 19],
            &[
                Challenges::NONE,
                Challenges::DARKNESS,
                Challenges::LEVEL_GENERATION,
            ],
        );
    }

    #[test]
    fn trinket_preflight_smoke_keeps_alternatives_and_selection_semantics() {
        check_trinket_preflight_semantics(&seeds()[..2], &[3], &[Challenges::NONE]);
    }

    fn check_trinket_preflight_semantics(
        seeds: &[DungeonSeed],
        depths: &[u8],
        challenge_modes: &[Challenges],
    ) {
        let requirements = [
            r#"[{"any_of":[{"item":"mimic_tooth","select_trinket":true},{"item":"rat_skull","select_trinket":true}]}]"#,
            r#"[{"any_of":[{"item":"trinket_catalyst"},{"item":"mimic_tooth"}]}]"#,
            r#"[{"any_of":[{"item":"rat_skull"},{"kind":"weapon"}]}]"#,
            r#"[{"any_of":[{"item":"rat_skull"},{"item":"mimic_tooth"},{"item":"trinket_catalyst"}]}]"#,
            r#"[{"item":"mimic_tooth"},{"item":"mimic_tooth"}]"#,
            r#"[{"item":"mimic_tooth","source":"heap","max_depth":1}]"#,
            r#"[{"item":"mimic_tooth","select_trinket":true},{"kind":"weapon"}]"#,
            r#"[{"kind":"weapon"}]"#,
        ];
        for requirements in requirements {
            for &depth in depths {
                let mut query = crate::json_query::decode(&format!(
                    "{{\"max_depth\":{depth},\"auto_apply_trinket\":true,\"requirements\":{requirements}}}")).unwrap();
                for &challenges in challenge_modes {
                    query.challenges = challenges;
                    compare_paths(&query, seeds, true);
                }
            }
        }
    }

    #[test]
    fn ambiguous_selection_does_not_make_present_offers_impossible() {
        let query = crate::json_query::decode(
            r#"{"max_depth":19,"requirements":[{"any_of":[{"item":"mimic_tooth","select_trinket":true},{"item":"parchment_scrap"}]},{"kind":"weapon"}]}"#).unwrap();
        let plan = QueryPlan::analyze(&query);
        assert_eq!(plan.selected_trinket(DungeonSeed::MIN), None);
        assert!(plan.continue_after_run_init(&RunState::new(0)));
        compare_paths(&query, &[DungeonSeed::MIN], true);
        let generator = CanonicalMainWorldGenerator::with_challenges(query.challenges);
        let matches = auto_trinkets::search_batch(&generator, &query, &plan, &[DungeonSeed::MIN]);
        assert!(
            matches[0].is_some(),
            "the ambiguous offers still satisfy the query"
        );
        assert_eq!(matches[0].as_ref().unwrap().recipe.trinket, None);
    }

    #[test]
    fn preflight_reads_initial_offers_without_mutating_run_state() {
        let query =
            crate::json_query::decode(r#"{"max_depth":3,"requirements":[{"item":"mimic_tooth"}]}"#)
                .unwrap();
        let plan = QueryPlan::analyze(&query);
        for seed in 0..256_i64 {
            let run = RunState::with_challenges(seed, Challenges::LEVEL_GENERATION);
            let expected = crate::trinkets::order_from_generator(&run.generator)[..4]
                .contains(&ItemId::MimicTooth);
            let before = run.clone();
            assert_eq!(plan.continue_after_run_init(&run), expected);
            assert_eq!(run, before);
        }
    }
}

#[cfg(test)]
mod source_refinement_tests {
    use super::*;

    fn original_plan(query: &SearchQuery) -> QueryPlan {
        QueryPlan::analyze_with_profile(query, |requirement, source| {
            source_profile(source, requirement.kind, requirement.weapon_category)
        })
    }

    fn decode(requirements: &str) -> SearchQuery {
        crate::json_query::decode(&format!(
            r#"{{"max_depth":24,"auto_apply_trinket":false,"requirements":{requirements}}}"#
        ))
        .unwrap()
    }

    fn shape(plan: &QueryPlan) -> (bool, u8, bool) {
        (
            plan.is_unsatisfiable(),
            plan.generation_depth(),
            plan.needs_vault_treasure,
        )
    }

    type PlanShape = (bool, u8, bool);
    const SOURCE_CASES: &[(&str, PlanShape, PlanShape)] = &[
        (
            r#"[{"item":"greatsword","upgrade":4}]"#,
            (false, 19, true),
            (false, 19, false),
        ),
        (
            r#"[{"item":"runic_blade","upgrade":4}]"#,
            (false, 19, true),
            (false, 19, true),
        ),
        (
            r#"[{"item":"javelin","upgrade":4}]"#,
            (false, 19, true),
            (false, 19, false),
        ),
        (
            r#"[{"kind":"thrown_weapon","upgrade":4}]"#,
            (false, 19, true),
            (false, 19, false),
        ),
        (
            r#"[{"kind":"weapon","upgrade":4}]"#,
            (false, 19, true),
            (false, 19, true),
        ),
        (
            r#"[{"kind":"melee_weapon","tier":{"exact":5},"upgrade":4,"source":"vault_treasure"}]"#,
            (false, 19, true),
            (true, 1, false),
        ),
        (
            r#"[{"kind":"melee_weapon","tier":{"at_most":4},"upgrade":4,"source":"vault_treasure"}]"#,
            (false, 19, true),
            (false, 19, true),
        ),
        (
            r#"[{"item":"runic_blade","upgrade":2,"source":"imp_reward"}]"#,
            (false, 19, false),
            (true, 1, false),
        ),
        (
            r#"[{"item":"runic_blade","upgrade":{"at_least":2},"source":"imp_reward"}]"#,
            (false, 19, false),
            (false, 19, false),
        ),
        (
            r#"[{"item":"greatsword","upgrade":2,"source":"imp_reward"}]"#,
            (false, 19, false),
            (false, 19, false),
        ),
        (
            r#"[{"kind":"weapon","tier":{"at_most":3},"source":"imp_reward"}]"#,
            (false, 19, false),
            (true, 1, false),
        ),
        (
            r#"[{"kind":"weapon","tier":{"at_most":4},"upgrade":2,"source":"imp_reward"}]"#,
            (false, 19, false),
            (true, 1, false),
        ),
        (
            r#"[{"kind":"weapon","tier":{"at_least":4},"upgrade":2,"source":"imp_reward"}]"#,
            (false, 19, false),
            (false, 19, false),
        ),
        (
            r#"[{"item":"plate_armor","upgrade":4}]"#,
            (false, 19, false),
            (false, 19, false),
        ),
        (
            r#"[{"item":"scale_armor","upgrade":4}]"#,
            (false, 19, false),
            (true, 1, false),
        ),
        (
            r#"[{"item":"leather_armor","upgrade":4}]"#,
            (false, 19, false),
            (true, 1, false),
        ),
        (
            r#"[{"kind":"armor","tier":{"at_most":4},"source":"imp_reward"}]"#,
            (false, 19, false),
            (true, 1, false),
        ),
        (
            r#"[{"kind":"armor","tier":{"at_least":4},"source":"imp_reward"}]"#,
            (false, 19, false),
            (false, 19, false),
        ),
        (
            r#"[{"item":"ring_might","upgrade":3,"source":"vault_treasure"}]"#,
            (false, 19, true),
            (true, 1, false),
        ),
        (
            r#"[{"item":"ring_accuracy","upgrade":3,"source":"vault_treasure"}]"#,
            (false, 19, true),
            (false, 19, true),
        ),
        (
            r#"[{"any_of":[{"item":"greatsword","upgrade":4,"source":"vault_treasure"},{"item":"wand_fireblast","upgrade":3,"source":"wandmaker_reward"}]}]"#,
            (false, 19, true),
            (false, 9, false),
        ),
        (
            r#"[{"any_of":[{"item":"javelin","upgrade":4,"source":"vault_treasure"},{"item":"runic_blade","upgrade":4,"source":"vault_treasure"}]}]"#,
            (false, 19, true),
            (false, 19, true),
        ),
        (
            r#"[{"any_of":[{"item":"ring_might","source":"vault_treasure"},{"item":"ring_accuracy","source":"vault_treasure"}]}]"#,
            (false, 19, true),
            (false, 19, true),
        ),
        (
            r#"[{"any_of":[{"item":"ring_might","source":"vault_treasure"},{"item":"ring_force","source":"vault_treasure"}]}]"#,
            (false, 19, true),
            (true, 1, false),
        ),
    ];

    #[test]
    fn exact_source_inventory_refines_only_reachable_tiers_and_identities() {
        for &(requirements, original, refined) in SOURCE_CASES {
            let query = decode(requirements);
            assert_eq!(shape(&original_plan(&query)), original, "{requirements}");
            assert_eq!(
                shape(&QueryPlan::analyze(&query)),
                refined,
                "{requirements}"
            );
        }
        for identity in [
            "wand_regrowth",
            "wand_transfusion",
            "wand_corruption",
            "ring_wealth",
            "ring_might",
            "ring_force",
        ] {
            let vault = decode(&format!(
                r#"[{{"item":"{identity}","upgrade":3,"source":"vault_treasure"}}]"#
            ));
            assert!(QueryPlan::analyze(&vault).is_unsatisfiable());
            let imp = decode(&format!(
                r#"[{{"item":"{identity}","upgrade":3,"source":"imp_reward"}}]"#
            ));
            assert!(!QueryPlan::analyze(&imp).is_unsatisfiable());
        }
    }

    #[test]
    fn source_inventory_preserves_optional_sum_profiles_and_deadlines() {
        for identity in ["ring_might", "ring_wealth", "ring_force"] {
            let query = decode(&format!(
                r#"[{{"item":"{identity}","level_sum":{{"group":1,"at_least":4}}}},{{"kind":"ring","level_sum":{{"group":1,"at_least":4}}}}]"#
            ));
            for requirement in &query.requirements {
                for source in ALL_SOURCES {
                    assert_eq!(
                        requirement_source_profile(requirement, source),
                        source_profile(source, requirement.kind, requirement.weapon_category)
                    );
                }
            }
            let original = original_plan(&query);
            let refined = QueryPlan::analyze(&query);
            assert_eq!(shape(&refined), shape(&original));
            for depth in [0, 9, 16, 17, 19, 24] {
                assert_eq!(
                    refined.viable_after_floor(depth, &[], &QuestSummary::default()),
                    original.viable_after_floor(depth, &[], &QuestSummary::default())
                );
            }
        }
        let pinned = decode(
            r#"[{"item":"ring_might","source":"vault_treasure","level_sum":{"group":1,"at_least":3}},{"item":"ring_accuracy","level_sum":{"group":1,"at_least":3}}]"#,
        );
        assert_eq!(
            shape(&QueryPlan::analyze(&pinned)),
            shape(&original_plan(&pinned))
        );
        assert!(QueryPlan::analyze(&pinned).needs_vault_treasure);
        let query = decode(
            r#"[{"item":"ring_might","level_sum":{"group":1,"at_least":1}},{"item":"greatsword","upgrade":4}]"#,
        );
        assert_eq!(shape(&original_plan(&query)), (false, 24, true));
        assert_eq!(shape(&QueryPlan::analyze(&query)), (false, 24, true));
        for depth in [16, 17, 19] {
            let mut query = decode(r#"[{"item":"greatsword","upgrade":4}]"#);
            query.max_depth = depth;
            let old = original_plan(&query);
            let new = QueryPlan::analyze(&query);
            assert_eq!(old.is_unsatisfiable(), new.is_unsatisfiable());
            assert_eq!(old.generation_depth(), new.generation_depth());
            assert!(!new.needs_vault_treasure);
            let mut capped = decode(r#"[{"item":"greatsword","upgrade":4}]"#);
            capped.requirements[0].max_depth = Some(depth);
            assert_eq!(shape(&QueryPlan::analyze(&capped)), shape(&new));
            assert_eq!(shape(&original_plan(&capped)), shape(&old));
        }
    }
    fn assert_produced_source_reachable(item: &WorldItem) {
        use crate::query::{EffectSet, TierRequirement};
        let definition = crate::catalog::item(item.item);
        let mut query = decode(&format!(
            r#"[{{"item":"{}","uncursed":true}}]"#,
            definition.stable_id
        ));
        let requirement = &mut query.requirements[0];
        requirement.source = Some(item.source);
        if item.upgrade > 0 {
            requirement.upgrade = UpgradeRequirement::Exact(item.upgrade);
        }
        if let Some(effect) = item.effect {
            requirement.effect = EffectRequirement::OneOf(EffectSet::single(effect));
        }
        let named = query.requirements[0];
        let mut requirements = vec![named];
        if matches!(definition.kind, ItemKind::Weapon | ItemKind::Armor) {
            let generic = Requirement {
                item: None,
                weapon_category: None,
                ..named
            };
            requirements.push(generic);
            requirements.push(Requirement {
                tier: TierRequirement::Exact(definition.tier.unwrap()),
                ..generic
            });
            if definition.kind == ItemKind::Weapon {
                requirements.push(Requirement {
                    weapon_category: item.item.weapon_category(),
                    ..generic
                });
            }
        }
        for requirement in requirements {
            query.requirements[0] = requirement;
            query.validate().unwrap();
            assert!(
                requirement.matches(item),
                "produced {item:?}, {requirement:?}"
            );
            let (low, high, _) = requirement_source_profile(&requirement, item.source)
                .expect("produced identity stays feasible");
            assert!(
                (low..=high).contains(&item.upgrade),
                "produced {item:?}, envelope {low}..{high}"
            );
            assert!(source_feasible(
                &requirement,
                item.source,
                &requirement_source_profile
            ));
        }
    }

    #[test]
    fn real_imp_and_vault_equipment_stays_inside_refined_source_envelopes() {
        use crate::model::Accessibility;
        use crate::quests::ImpQuest;
        use crate::rng::{RandomStack, seed_for_depth};
        use crate::run::RunState;
        use crate::vault_loot::VaultEquipmentLoot;
        let mut seen = [false; 8];
        for seed in 0..64_i64 {
            let mut run = RunState::new(seed);
            let mut random = RandomStack::with_base_seed(0);
            random.push(seed_for_depth(seed, 19, 0));
            let mut imp = ImpQuest::default();
            assert!(
                imp.schedule_room(&mut random, &mut run.generator, 19)
                    .unwrap()
            );
            assert_eq!(imp.reward_options.len(), 6);
            imp.finish_build_attempt(true);
            let mut items = Vec::new();
            imp.append_world_items(0, &mut items);
            for item in &items {
                let definition = crate::catalog::item(item.item);
                if definition.kind == ItemKind::Weapon {
                    seen[0] |= definition.tier == Some(4) && item.upgrade == 5;
                    seen[1] |= definition.tier == Some(5) && item.upgrade == 2;
                    seen[2] |= definition.tier == Some(5) && item.upgrade == 4;
                    seen[3] |= definition.tier == Some(4)
                        && item.item.weapon_category() == Some(WeaponCategory::Melee);
                    seen[4] |= definition.tier == Some(4)
                        && item.item.weapon_category() == Some(WeaponCategory::Thrown);
                }
                seen[5] |= item.item == ItemId::PlateArmor;
                seen[6] |= definition.kind == ItemKind::Artifact && item.upgrade == 5;
                assert_produced_source_reachable(item);
            }
            random.pop();
            random.push(seed_for_depth(seed, 19, 1));
            let mut loot = VaultEquipmentLoot::default();
            loot.setup_equipment(&mut random);
            for tier in 0..4 {
                for equipment in loot.tier(tier).unwrap().iter().flatten() {
                    let item = WorldItem {
                        item: equipment.item,
                        upgrade: equipment.upgrade,
                        effect: equipment.effect,
                        cursed: false,
                        depth: 19,
                        source: ItemSource::VaultTreasure,
                        accessibility: Accessibility::Independent,
                        secret: false,
                    };
                    seen[7] |= crate::catalog::item(item.item).tier == Some(4)
                        && item.item.weapon_category() == Some(WeaponCategory::Melee)
                        && item.upgrade == 4;
                    assert_produced_source_reachable(&item);
                }
            }
        }
        assert!(
            seen.into_iter().all(|covered| covered),
            "exercise both Imp weapon pairings and upgrade endpoints"
        );
        for seed in [0, 1, 1_334_551, 1_334_612] {
            for depth in 17..=19 {
                let vault = crate::vault_floor::generate_vault(
                    seed,
                    depth,
                    crate::challenges::Challenges::NONE,
                )
                .unwrap();
                for item in vault.world_items(depth, 0, 6) {
                    assert_produced_source_reachable(&item);
                }
            }
        }
    }

    fn recipes(
        matches: Vec<Option<crate::auto_trinkets::TrinketSearchMatch>>,
    ) -> Vec<Option<crate::auto_trinkets::SeedRecipe>> {
        matches
            .into_iter()
            .map(|matched| matched.map(|m| m.recipe))
            .collect()
    }

    fn dispersed_seeds() -> Vec<crate::seed::DungeonSeed> {
        use crate::seed::{DungeonSeed, TOTAL_SEEDS};
        let mut seeds: Vec<_> = (0..20_u64)
            .map(|index| {
                DungeonSeed::new((812_345_678_901 + index * 3_355_211_884_971) % TOTAL_SEEDS)
                    .unwrap()
            })
            .collect();
        seeds.extend(
            ["EYY-RUL-LQG", "SRU-YSU-QHS"].map(|code| DungeonSeed::from_code(code).unwrap()),
        );
        seeds
    }

    #[test]
    #[ignore = "extended seed sweep; run with --release --ignored"]
    #[allow(clippy::too_many_lines)] // Keep query, oracle, and recipe comparisons together.
    fn refined_sources_preserve_full_search_matches_recipes_and_forced_choices() {
        use crate::auto_trinkets::{self, SeedRecipe};
        use crate::challenges::Challenges;
        use crate::main_world::{CanonicalMainWorldGenerator, generate_main_world_with_trinket};
        use crate::search::WorldGenerator;
        let seeds = dispersed_seeds();
        let requirements = [
            r#"[{"item":"runic_blade","upgrade":1,"effect":"Grim"}]"#,
            r#"[{"item":"ring_might","upgrade":3}]"#,
            r#"[{"item":"greatsword","upgrade":4}]"#,
            r#"[{"kind":"thrown_weapon","upgrade":4}]"#,
            r#"[{"item":"runic_blade","upgrade":2,"source":"imp_reward"}]"#,
            r#"[{"any_of":[{"item":"greatsword","upgrade":4,"source":"vault_treasure"},{"item":"wand_fireblast","upgrade":3,"source":"wandmaker_reward"}]}]"#,
            r#"[{"any_of":[{"item":"ring_might","source":"vault_treasure"},{"item":"ring_accuracy","source":"vault_treasure"}]}]"#,
            r#"[{"item":"mimic_tooth","select_trinket":true},{"kind":"thrown_weapon","upgrade":4}]"#,
            r#"[{"kind":"weapon","tier":{"exact":4},"upgrade":3}]"#,
            r#"[{"item":"leather_armor","effect":"any_enchantment"}]"#,
            r#"[{"item":"runic_blade","upgrade":3}]"#,
            r#"[{"item":"greatsword","upgrade":2}]"#,
            r#"[{"any_of":[{"item":"runic_blade","upgrade":3,"source":"vault_treasure"},{"kind":"wand","source":"wandmaker_reward"}]}]"#,
            r#"[{"any_of":[{"item":"runic_blade","upgrade":3,"source":"vault_treasure"},{"item":"greatsword","upgrade":3,"source":"vault_treasure"}]}]"#,
            r#"[{"kind":"melee_weapon","tier":{"exact":2},"upgrade":1}]"#,
            r#"[{"kind":"thrown_weapon","tier":{"exact":2},"upgrade":1}]"#,
            r#"[{"kind":"weapon","tier":{"exact":3},"source":"heap"}]"#,
            r#"[{"kind":"armor","tier":{"exact":3},"source":"heap"}]"#,
        ];
        let mut natural_deadline_survivors = [0_usize; 5];
        let mut finite_row_survivors = [0_usize; 2];
        let mut survivor_count = 0;
        let mut removed_auto = 0;
        let mut retained_forced = 0;
        let mut surviving_omission = 0;
        for (case, requirements) in requirements.into_iter().enumerate() {
            for challenges in [
                Challenges::NONE,
                Challenges::DARKNESS,
                Challenges::LEVEL_GENERATION,
            ] {
                for auto in [false, true] {
                    let mut query = decode(requirements);
                    // These saved recipes exercise auto-removal at their
                    // original depth; the source-refinement cases cover 24.
                    if case == 0 {
                        query.max_depth = 19;
                    }
                    query.challenges = challenges;
                    query.auto_apply_trinket = auto;
                    query.validate().unwrap();
                    let old = original_plan(&query);
                    let previous = previous_candidate_plan(&query);
                    let deadline_case_index = match case {
                        9 => Some(0),
                        14..=17 => Some(case - 13),
                        _ => None,
                    };
                    let new = QueryPlan::analyze(&query);
                    let generator = CanonicalMainWorldGenerator::with_challenges(challenges);
                    for batch in seeds.chunks(9) {
                        let expected =
                            recipes(auto_trinkets::search_batch(&generator, &query, &old, batch));
                        let previous_expected = deadline_case_index.map(|_| {
                            recipes(auto_trinkets::search_batch(
                                &generator, &query, &previous, batch,
                            ))
                        });
                        let matches = auto_trinkets::search_batch(&generator, &query, &new, batch);
                        let gated = generator
                            .generate_batch_gated(batch, new.generation_depth(), &new)
                            .into_iter()
                            .flatten()
                            .filter(|world| query.matches(world))
                            .collect();
                        let finished =
                            auto_trinkets::finish_matches(&generator, &query, &new, gated);
                        assert_eq!(
                            finished
                                .iter()
                                .map(|m| (m.recipe, &m.world))
                                .collect::<Vec<_>>(),
                            matches
                                .iter()
                                .flatten()
                                .map(|m| (m.recipe, &m.world))
                                .collect::<Vec<_>>(),
                            "streaming completion matches batch completion"
                        );
                        for result in matches.iter().flatten() {
                            if let Some(index) = deadline_case_index {
                                assert!(previous.generation_depth() > new.generation_depth());
                                natural_deadline_survivors[index] += 1;
                            }
                            if matches!(case, 8 | 9) {
                                assert!(old.needs_vault_treasure && !new.needs_vault_treasure);
                                finite_row_survivors[case - 8] += 1;
                            }
                            removed_auto += usize::from(
                                auto_trinkets::enabled(&query)
                                    && new.selected_trinket(result.recipe.seed).is_some()
                                    && result.recipe.trinket.is_none(),
                            );
                            surviving_omission +=
                                usize::from(old.needs_vault_treasure && !new.needs_vault_treasure);
                            let mut canonical = generate_main_world_with_trinket(
                                result.recipe.seed,
                                new.generation_depth(),
                                challenges,
                                result.recipe.trinket,
                            )
                            .unwrap();
                            if !new.needs_vault_treasure {
                                canonical
                                    .items
                                    .retain(|item| item.source != ItemSource::VaultTreasure);
                            }
                            assert_eq!(
                                result.world, canonical,
                                "world belongs to its returned recipe"
                            );
                        }
                        let actual = recipes(matches);
                        if let Some(previous_expected) = previous_expected {
                            assert_eq!(
                                actual, previous_expected,
                                "immediate C16 reference: {query:?}"
                            );
                        }
                        survivor_count += actual.iter().flatten().count();
                        assert_eq!(actual, expected, "{query:?}");
                        for (&seed, actual) in batch.iter().zip(&actual) {
                            let selected = new.selected_trinket(seed);
                            let full = generate_main_world_with_trinket(
                                seed,
                                query.max_depth,
                                challenges,
                                selected,
                            )
                            .unwrap();
                            let expected = if query.matches(&full) {
                                let choice = if auto_trinkets::enabled(&query)
                                    && selected.is_some()
                                    && query.matches(
                                        &generate_main_world_with_trinket(
                                            seed,
                                            query.max_depth,
                                            challenges,
                                            None,
                                        )
                                        .unwrap(),
                                    ) {
                                    None
                                } else {
                                    selected
                                };
                                Some(SeedRecipe {
                                    seed,
                                    trinket: choice,
                                })
                            } else {
                                None
                            };
                            assert_eq!(
                                *actual, expected,
                                "full canonical {query:?}, seed {seed:?}"
                            );
                        }
                    }
                    for choice in [None, Some(0), Some(3)] {
                        let saved: Vec<_> = seeds[..5]
                            .iter()
                            .map(|&seed| SeedRecipe {
                                seed,
                                trinket: choice
                                    .map(|index| crate::trinkets::initial_offers(seed)[index]),
                            })
                            .collect();
                        let actual = auto_trinkets::filter_batch(&generator, &query, &new, &saved);
                        if deadline_case_index.is_some() {
                            assert_eq!(
                                actual
                                    .iter()
                                    .map(|matched| matched.as_ref().map(|m| m.recipe))
                                    .collect::<Vec<_>>(),
                                recipes(auto_trinkets::filter_batch(
                                    &generator, &query, &previous, &saved
                                )),
                                "forced choices against immediate C16 reference: {query:?}",
                            );
                        }
                        for (saved, result) in saved.iter().zip(&actual) {
                            let full = generate_main_world_with_trinket(
                                saved.seed,
                                query.max_depth,
                                challenges,
                                saved.trinket,
                            )
                            .unwrap();
                            assert_eq!(
                                result.is_some(),
                                query.matches(&full),
                                "forced full canonical {query:?}"
                            );
                            if let Some(result) = result {
                                retained_forced += usize::from(
                                    saved.trinket.is_some()
                                        && result.recipe.trinket == saved.trinket,
                                );
                                let mut canonical = generate_main_world_with_trinket(
                                    result.recipe.seed,
                                    new.generation_depth(),
                                    challenges,
                                    result.recipe.trinket,
                                )
                                .unwrap();
                                if !new.needs_vault_treasure {
                                    canonical
                                        .items
                                        .retain(|item| item.source != ItemSource::VaultTreasure);
                                }
                                assert_eq!(result.world, canonical);
                            }
                        }
                        assert_eq!(
                            recipes(actual),
                            recipes(auto_trinkets::filter_batch(
                                &generator, &query, &old, &saved
                            )),
                            "forced choices {query:?}"
                        );
                    }
                }
            }
        }
        assert!(
            finite_row_survivors.into_iter().all(|count| count > 0),
            "exercise surviving finite upgrade-gap and effect-free-row omissions"
        );
        for (label, count) in [
            "Leather Armor with an enchantment",
            "tier-two melee weapon +1",
            "tier-two thrown weapon +1",
            "tier-three weapon heap",
            "tier-three armor heap",
        ]
        .into_iter()
        .zip(natural_deadline_survivors)
        {
            assert!(
                count > 0,
                "exercise surviving deadline reduction for {label}"
            );
        }
        assert!(
            survivor_count > 0,
            "exercise surviving matches as well as rejected seeds"
        );
        assert!(
            removed_auto > 0,
            "exercise replacement with a no-trinket world"
        );
        assert!(retained_forced > 0, "exercise retained saved choices");
        assert!(
            surviving_omission > 0,
            "exercise matches after newly skipping vault generation"
        );
    }

    #[test]
    fn skipping_irrelevant_vault_preserves_complete_later_worlds() {
        use crate::challenges::Challenges;
        use crate::main_world::CanonicalMainWorldGenerator;
        use crate::search::WorldGenerator;
        use crate::seed::DungeonSeed;
        struct ConditionsOnly<'a>(&'a QueryPlan);
        impl FloorGate for ConditionsOnly<'_> {
            fn selected_trinket(&self, seed: DungeonSeed) -> Option<ItemId> {
                self.0.selected_trinket(seed)
            }
            fn wants_vault_treasure(&self) -> bool {
                self.0.wants_vault_treasure()
            }
            fn continue_after_floor(&self, _: u8, _: &[WorldItem], _: &QuestSummary) -> bool {
                true
            }
        }
        let seeds = dispersed_seeds();
        let mut removed = 0;
        for requirements in [
            r#"[{"item":"ring_might","upgrade":3}]"#,
            r#"[{"kind":"weapon","tier":{"exact":4},"upgrade":3}]"#,
        ] {
            let removed_before = removed;
            for challenges in [
                Challenges::NONE,
                Challenges::DARKNESS,
                Challenges::LEVEL_GENERATION,
            ] {
                let mut query = decode(requirements);
                query.challenges = challenges;
                query.auto_apply_trinket = true;
                let old = original_plan(&query);
                let new = QueryPlan::analyze(&query);
                assert!(old.needs_vault_treasure && !new.needs_vault_treasure);
                let generator = CanonicalMainWorldGenerator::with_challenges(challenges);
                let original =
                    generator.generate_batch_gated(&seeds[..9], 24, &ConditionsOnly(&old));
                let refined =
                    generator.generate_batch_gated(&seeds[..9], 24, &ConditionsOnly(&new));
                for (original, refined) in original.into_iter().zip(refined) {
                    let mut original = original.unwrap();
                    let refined = refined.unwrap();
                    let old_witness = crate::query::scout_matches(&original, &query);
                    let new_witness = crate::query::scout_matches(&refined, &query);
                    assert_eq!(
                        old_witness.matched_requirements,
                        new_witness.matched_requirements
                    );
                    assert_eq!(
                        old_witness
                            .matched_indices()
                            .into_iter()
                            .map(|index| &original.items[index])
                            .collect::<Vec<_>>(),
                        new_witness
                            .matched_indices()
                            .into_iter()
                            .map(|index| &refined.items[index])
                            .collect::<Vec<_>>(),
                    );
                    let before = original.items.len();
                    original
                        .items
                        .retain(|item| item.source != ItemSource::VaultTreasure);
                    removed += before - original.items.len();
                    assert_eq!(
                        refined, original,
                        "including all Halls floors after the vault"
                    );
                }
            }
            assert!(
                removed > removed_before,
                "exercise each source omission: {requirements}"
            );
        }
        assert!(removed > 0, "exercise actually generated vault treasure");
    }
    #[test]
    fn finite_vault_rows_preserve_upgrade_holes_and_tier_zero_effect_rule() {
        for requirement in [
            r#"{"item":"runic_blade","upgrade":3}"#,
            r#"{"item":"greatsword","upgrade":2}"#,
            r#"{"item":"javelin","upgrade":3}"#,
            r#"{"item":"scale_armor","upgrade":3}"#,
            r#"{"kind":"weapon","tier":{"exact":4},"upgrade":3}"#,
            r#"{"kind":"armor","tier":{"at_most":4},"upgrade":{"at_least":3}}"#,
            r#"{"item":"leather_armor","effect":"any_enchantment"}"#,
            r#"{"kind":"thrown_weapon","tier":{"exact":2},"effect":"any_enchantment"}"#,
        ] {
            let mut query = decode(&format!("[{requirement}]"));
            assert!(
                !QueryPlan::analyze(&query).is_unsatisfiable(),
                "other sources: {requirement}"
            );
            assert!(
                !QueryPlan::analyze(&query).needs_vault_treasure,
                "vault omitted: {requirement}"
            );
            query.requirements[0].source = Some(ItemSource::VaultTreasure);
            assert!(
                QueryPlan::analyze(&query).is_unsatisfiable(),
                "vault alone: {requirement}"
            );
        }
        for requirement in [
            r#"{"item":"runic_blade","upgrade":2}"#,
            r#"{"item":"runic_blade","upgrade":4}"#,
            r#"{"item":"runic_blade","upgrade":{"at_least":3}}"#,
            r#"{"item":"greatsword","upgrade":3}"#,
            r#"{"item":"javelin","upgrade":2}"#,
            r#"{"item":"scale_armor","upgrade":2}"#,
            r#"{"item":"mail_armor","effect":"any_enchantment"}"#,
            r#"{"item":"mail_armor","effect":["Obfuscation","Stench"],"uncursed":true}"#,
            r#"{"item":"leather_armor"}"#,
        ] {
            let mut query = decode(&format!("[{requirement}]"));
            query.requirements[0].source = Some(ItemSource::VaultTreasure);
            let plan = QueryPlan::analyze(&query);
            assert!(
                !plan.is_unsatisfiable() && plan.needs_vault_treasure,
                "{requirement}"
            );
        }
        let query = decode(
            r#"[{"any_of":[{"item":"runic_blade","upgrade":3,"source":"vault_treasure"},{"item":"greatsword","upgrade":3,"source":"vault_treasure"}]}]"#,
        );
        assert!(!QueryPlan::analyze(&query).is_unsatisfiable());
        assert!(QueryPlan::analyze(&query).needs_vault_treasure);
        let query = decode(
            r#"[{"any_of":[{"item":"runic_blade","upgrade":3,"source":"vault_treasure"},{"kind":"wand","source":"wandmaker_reward"}]}]"#,
        );
        assert!(!QueryPlan::analyze(&query).is_unsatisfiable());
        assert!(!QueryPlan::analyze(&query).needs_vault_treasure);
    }

    #[test]
    fn finite_vault_zero_rows_never_carry_effects() {
        for requirement in [
            r#"{"kind":"melee_weapon","tier":{"exact":2},"effect":"any_enchantment"}"#,
            r#"{"kind":"thrown_weapon","tier":{"exact":2},"effect":"any_enchantment"}"#,
            r#"{"item":"leather_armor","effect":"any_enchantment"}"#,
        ] {
            let mut query = decode(&format!("[{requirement}]"));
            // Exact(0) is not a validated search predicate, but the internal
            // row matcher must distinguish +0 from tier two's +2 melee row.
            let requirement = &mut query.requirements[0];
            requirement.upgrade = UpgradeRequirement::Exact(0);
            assert!(!vault_inventory_reachable(requirement));
            requirement.effect = EffectRequirement::Any;
            assert!(vault_inventory_reachable(requirement));
        }
    }
    /// Immediate C16 reference: retain its source inventories, but disable
    /// every new deadline rule, including the Smith tier restriction.
    fn previous_candidate_plan(query: &SearchQuery) -> QueryPlan {
        QueryPlan::analyze_with_policies(query, requirement_source_profile, |_, _, limit| {
            Some(limit)
        })
    }

    const NATURAL_DEADLINE_CASES: &[(&str, PlanShape, PlanShape)] = &[
        (
            r#"[{"item":"leather_armor","effect":"any_enchantment"}]"#,
            (false, 24, false),
            (false, 9, false),
        ),
        (
            r#"[{"kind":"melee_weapon","tier":{"exact":2},"upgrade":1}]"#,
            (false, 24, false),
            (false, 9, false),
        ),
        (
            r#"[{"kind":"thrown_weapon","tier":{"exact":2},"upgrade":1}]"#,
            (false, 24, false),
            (false, 9, false),
        ),
        (
            r#"[{"kind":"thrown_weapon","tier":{"exact":2},"effect":"any_enchantment"}]"#,
            (false, 24, false),
            (false, 9, false),
        ),
        (
            r#"[{"kind":"weapon","tier":{"exact":2},"effect":"any_enchantment"}]"#,
            (false, 24, true),
            (false, 19, true),
        ),
        (
            r#"[{"kind":"weapon","tier":{"exact":2}}]"#,
            (false, 24, true),
            (false, 21, true),
        ),
        (
            r#"[{"kind":"thrown_weapon","tier":{"exact":2}}]"#,
            (false, 24, true),
            (false, 21, true),
        ),
        (
            r#"[{"kind":"weapon","tier":{"exact":3},"source":"heap"}]"#,
            (false, 24, false),
            (false, 19, false),
        ),
        (
            r#"[{"kind":"armor","tier":{"at_most":3},"source":"heap"}]"#,
            (false, 24, false),
            (false, 19, false),
        ),
        (
            r#"[{"kind":"weapon","tier":{"at_least":3},"source":"heap"}]"#,
            (false, 24, false),
            (false, 24, false),
        ),
        (
            r#"[{"kind":"weapon","tier":{"at_least":4},"source":"heap"}]"#,
            (false, 24, false),
            (false, 24, false),
        ),
        (
            r#"[{"kind":"armor","tier":{"at_most":4},"source":"heap"}]"#,
            (false, 24, false),
            (false, 24, false),
        ),
        (
            r#"[{"item":"quarterstaff","source":"heap"}]"#,
            (false, 24, false),
            (false, 9, false),
        ),
        (
            r#"[{"item":"kunai","source":"heap"}]"#,
            (false, 24, false),
            (false, 19, false),
        ),
        (
            r#"[{"kind":"weapon","tier":{"exact":2},"source":"blacksmith_reward"}]"#,
            (false, 14, false),
            (true, 1, false),
        ),
        (
            r#"[{"kind":"weapon","tier":{"exact":3},"source":"blacksmith_reward"}]"#,
            (false, 14, false),
            (false, 14, false),
        ),
        (
            r#"[{"item":"cloth_armor","source":"heap"}]"#,
            (false, 24, false),
            (true, 1, false),
        ),
        (
            r#"[{"item":"worn_shortsword","source":"heap"}]"#,
            (false, 24, false),
            (true, 1, false),
        ),
    ];

    #[test]
    fn natural_deadlines_preserve_source_specific_exceptions() {
        for &(requirements, previous, current) in NATURAL_DEADLINE_CASES {
            let query = decode(requirements);
            assert_eq!(
                shape(&previous_candidate_plan(&query)),
                previous,
                "{requirements}"
            );
            assert_eq!(
                shape(&QueryPlan::analyze(&query)),
                current,
                "{requirements}"
            );
        }
    }

    #[test]
    fn natural_deadline_limits_do_not_replace_requirement_limits() {
        for (tier, last_depth) in [(2, 9), (3, 19)] {
            for limit in [1, 4, 5, 8, 9, 10, 11, 18, 19, 20, 21, 24] {
                for per_item in [false, true] {
                    let mut query = decode(&format!(
                        r#"[{{"kind":"weapon","tier":{{"exact":{tier}}},"source":"heap"}}]"#
                    ));
                    if per_item {
                        query.requirements[0].max_depth = Some(limit);
                    } else {
                        query.max_depth = limit;
                    }
                    query.validate().unwrap();
                    let previous = previous_candidate_plan(&query);
                    let plan = QueryPlan::analyze(&query);
                    let expected = limit.min(last_depth);
                    assert_eq!(previous.generation_depth(), limit);
                    assert_eq!(plan.generation_depth(), expected);
                    assert_eq!(plan.slots[0][0].open_deadline, Some(expected));
                    assert_eq!(plan.slots[0][0].max_depth, limit);
                    assert_eq!(plan.slots[0][0].requirement, query.requirements[0]);
                    assert!(plan.viable_after_floor(expected - 1, &[], &QuestSummary::default()));
                    assert!(!plan.viable_after_floor(expected, &[], &QuestSummary::default()));
                }
            }
        }
    }

    #[test]
    fn natural_deadline_or_slots_and_world_conditions_keep_later_horizons() {
        let mixed = decode(
            r#"[{"any_of":[{"kind":"weapon","tier":{"exact":2},"source":"heap"},{"kind":"ring","source":"heap"}]}]"#,
        );
        let plan = QueryPlan::analyze(&mixed);
        assert_eq!(plan.generation_depth(), 24);
        assert_eq!(plan.slots[0][0].open_deadline, Some(9));
        assert_eq!(plan.slots[0][1].open_deadline, Some(24));
        assert!(plan.viable_after_floor(9, &[], &QuestSummary::default()));
        let mandatory = decode(
            r#"[{"kind":"weapon","tier":{"exact":2},"source":"heap"},{"kind":"ring","source":"heap"}]"#,
        );
        let plan = QueryPlan::analyze(&mandatory);
        assert_eq!(plan.generation_depth(), 24);
        assert!(!plan.viable_after_floor(9, &[], &QuestSummary::default()));
        let vault = decode(
            r#"[{"any_of":[{"kind":"weapon","tier":{"exact":2},"source":"heap"},{"item":"ring_accuracy","upgrade":3,"source":"vault_treasure"}]}]"#,
        );
        assert_eq!(shape(&QueryPlan::analyze(&vault)), (false, 19, true));

        let mut early = decode(r#"[{"item":"leather_armor","effect":"any_enchantment"}]"#);
        early.require_blacksmith = true;
        let plan = QueryPlan::analyze(&early);
        assert_eq!(plan.generation_depth(), 14);
        assert_eq!(plan.blacksmith_deadline, Some(14));
        early.exclude_blacksmith_rewards = true;
        assert_eq!(QueryPlan::analyze(&early).generation_depth(), 14);
        early.require_blacksmith = false;
        assert_eq!(QueryPlan::analyze(&early).generation_depth(), 9);

        let mut quested =
            decode(r#"[{"kind":"weapon","tier":{"exact":2},"source":"heap","max_depth":3}]"#);
        quested.wandmaker_quest = Some(crate::quests::WandmakerQuestType::Rotberry);
        quested.validate().unwrap();
        let plan = QueryPlan::analyze(&quested);
        assert_eq!(plan.generation_depth(), 9);
        assert_eq!(plan.slots[0][0].max_depth, 3);
        assert_eq!(plan.slots[0][0].open_deadline, Some(3));
    }

    #[test]
    fn natural_deadlines_keep_optional_ring_sources_and_slot_limits() {
        let query = decode(
            r#"[{"item":"ring_might","source":"vault_treasure","level_sum":{"group":1,"at_least":3}},{"item":"ring_accuracy","level_sum":{"group":1,"at_least":3}},{"kind":"armor","tier":{"exact":2},"source":"heap"}]"#,
        );
        let previous = previous_candidate_plan(&query);
        let current = QueryPlan::analyze(&query);
        assert_eq!(shape(&current), shape(&previous));
        for slot in 0..2 {
            let old = &previous.slots[slot][0];
            let new = &current.slots[slot][0];
            assert_eq!(new.requirement, old.requirement);
            assert_eq!(
                (new.max_depth, new.quests, new.open_deadline),
                (old.max_depth, old.quests, old.open_deadline)
            );
            for source in ALL_SOURCES {
                assert_eq!(
                    source_generation_deadline(&new.requirement, source, 24),
                    Some(24)
                );
            }
        }
        assert_eq!(current.slots[2][0].open_deadline, Some(9));
    }

    #[test]
    fn natural_deadline_support_matches_pinned_floor_and_category_tables() {
        use crate::generator::FLOOR_SET_TIER_PROBABILITIES;
        use crate::run::GeneratorCategory;
        assert!(
            FLOOR_SET_TIER_PROBABILITIES
                .iter()
                .all(|row| row[0].to_bits() == 0)
        );
        for category in [
            GeneratorCategory::WeaponTier1,
            GeneratorCategory::MissileTier1,
        ] {
            assert_eq!(category.first_probability().to_bits(), 0);
            assert_eq!(category.second_probability().to_bits(), 0);
        }
        for kind in ["weapon", "melee_weapon", "thrown_weapon", "armor"] {
            for (tier, last_depth) in [(2, 9), (3, 19), (4, 24), (5, 24)] {
                let query = decode(&format!(
                    r#"[{{"kind":"{kind}","tier":{{"exact":{tier}}}}}]"#
                ));
                let requirement = &query.requirements[0];
                for source in ALL_SOURCES {
                    if quest_for_source(source).is_none() && source != ItemSource::Shop {
                        assert_eq!(
                            source_generation_deadline(requirement, source, 24),
                            Some(last_depth),
                            "{kind}, tier {tier}, {source:?}"
                        );
                    }
                }
                assert_eq!(
                    source_generation_deadline(requirement, ItemSource::Shop, 24),
                    Some(24)
                );
                assert_eq!(
                    source_generation_deadline(requirement, ItemSource::VaultTreasure, 24),
                    Some(24)
                );
                assert_eq!(
                    source_generation_deadline(requirement, ItemSource::BlacksmithReward, 24),
                    (tier >= 3).then_some(24)
                );
            }
        }
        for identity in ["worn_shortsword", "throwing_knife", "cloth_armor"] {
            let query = decode(&format!(r#"[{{"item":"{identity}"}}]"#));
            for row in FLOOR_SET_TIER_PROBABILITIES {
                assert!(!floor_set_supports_requirement(
                    &query.requirements[0],
                    &row
                ));
            }
            assert_eq!(
                source_generation_deadline(&query.requirements[0], ItemSource::Heap, 24),
                None
            );
        }
    }

    #[test]
    fn natural_deadline_named_tiers_intersect_filters_without_weakening_validation() {
        use crate::query::{QueryError, TierRequirement};
        let query = decode(r#"[{"item":"quarterstaff"}]"#);
        let mut requirement = query.requirements[0];
        assert_eq!(
            source_generation_deadline(&requirement, ItemSource::Heap, 24),
            Some(9)
        );
        // Named items cannot carry an explicit tier filter in a valid query,
        // but the internal predicate still intersects both constraints.
        requirement.tier = TierRequirement::Exact(3);
        assert_eq!(requirement.validate(), Err(QueryError::InvalidTier));
        assert_eq!(
            source_generation_deadline(&requirement, ItemSource::Heap, 24),
            None
        );
        requirement.tier = TierRequirement::Any;
        requirement.weapon_category = Some(WeaponCategory::Thrown);
        assert_eq!(
            requirement.validate(),
            Err(QueryError::InvalidWeaponCategory)
        );
    }

    #[test]
    fn full_worlds_respect_natural_tier_deadlines_and_keep_late_exceptions() {
        use crate::challenges::Challenges;
        use crate::main_world::generate_main_world_with_trinket;
        let mut natural_tiers = [0_usize; 2];
        let mut natural_boundaries = [0_usize; 2];
        let mut late_shop_tier_two = 0;
        let mut late_vault_tier_two = 0;
        let mut smith_rewards = 0;
        let base = decode(r#"[{"kind":"weapon"}]"#).requirements[0];
        for seed in dispersed_seeds() {
            let choices = std::iter::once(None)
                .chain(crate::trinkets::initial_offers(seed).into_iter().map(Some));
            for choice in choices {
                for challenges in [
                    Challenges::NONE,
                    Challenges::DARKNESS,
                    Challenges::LEVEL_GENERATION,
                ] {
                    let world =
                        generate_main_world_with_trinket(seed, 24, challenges, choice).unwrap();
                    for item in &world.items {
                        let definition = crate::catalog::item(item.item);
                        if !matches!(definition.kind, ItemKind::Weapon | ItemKind::Armor) {
                            continue;
                        }
                        let requirement = Requirement {
                            kind: definition.kind,
                            item: Some(item.item),
                            source: Some(item.source),
                            ..base
                        };
                        assert!(requirement.matches(item));
                        let deadline = source_generation_deadline(&requirement, item.source, 24)
                            .expect("an actually generated item keeps a source horizon");
                        assert!(item.depth <= deadline, "{seed:?} {choice:?} {item:?}");
                        match item.source {
                            ItemSource::Shop => {
                                late_shop_tier_two +=
                                    usize::from(definition.tier == Some(2) && item.depth >= 20);
                            }
                            ItemSource::VaultTreasure => {
                                late_vault_tier_two +=
                                    usize::from(definition.tier == Some(2) && item.depth >= 17);
                            }
                            ItemSource::BlacksmithReward => {
                                assert!(definition.tier.is_some_and(|tier| tier >= 3));
                                smith_rewards += 1;
                            }
                            ItemSource::GhostReward
                            | ItemSource::WandmakerReward
                            | ItemSource::ImpReward => {}
                            _ => {
                                assert!(definition.tier.is_some_and(|tier| tier >= 2));
                                if let Some(tier @ (2 | 3)) = definition.tier {
                                    let index = usize::from(tier - 2);
                                    let limit = if tier == 2 { 9 } else { 19 };
                                    assert!(item.depth <= limit, "{item:?}");
                                    natural_tiers[index] += 1;
                                    natural_boundaries[index] += usize::from(item.depth == limit);
                                }
                            }
                        }
                    }
                }
            }
        }
        assert!(natural_tiers.into_iter().all(|count| count > 0));
        assert!(natural_boundaries.into_iter().all(|count| count > 0));
        assert!(
            late_shop_tier_two > 0,
            "exercise tier-two darts at late shops"
        );
        assert!(
            late_vault_tier_two > 0,
            "exercise late tier-two vault equipment"
        );
        assert!(smith_rewards > 0);
    }
}

#[cfg(test)]
mod closed_multiplicity_grouping_tests {
    use super::{QueryPlan, RequirementPlan, closed_multiplicities};
    use crate::catalog::{
        ALL_WEAPON_EFFECTS, ArmorEffect, Effect, ItemId, ItemKind, WeaponCategory, WeaponEffect,
    };
    use crate::challenges::Challenges;
    use crate::model::ItemSource;
    use crate::query::{
        EffectRequirement, EffectSet, LevelSum, Requirement, SearchQuery, TierRequirement,
        UpgradeRequirement,
    };

    fn ordinary_plan() -> RequirementPlan {
        RequirementPlan {
            requirement: Requirement {
                kind: ItemKind::Weapon,
                weapon_category: None,
                item: None,
                tier: TierRequirement::Any,
                upgrade: UpgradeRequirement::Any,
                effect: EffectRequirement::Any,
                require_uncursed: false,
                select_trinket: false,
                blanket: false,
                source: Some(ItemSource::Heap),
                identity_group: None,
                max_depth: Some(4),
                alternative_group: None,
                level_sum: None,
            },
            max_depth: 4,
            quests: 0,
            vault: false,
            open_deadline: Some(4),
        }
    }

    // Intentionally quadratic and test-only: neither hashes nor calls the
    // production eligibility iterator. Equality includes the complete plan.
    fn suffix_count_oracle(slots: &[Vec<RequirementPlan>]) -> Vec<(usize, usize)> {
        let mut result = Vec::new();
        for (first, slot) in slots.iter().enumerate() {
            let [wanted] = slot.as_slice() else {
                continue;
            };
            if wanted.quests != 0
                || wanted.open_deadline.is_none()
                || wanted.requirement.level_sum.is_some()
            {
                continue;
            }
            if slots[..first].iter().any(|previous| previous == slot) {
                continue;
            }
            let count = slots[first..].iter().filter(|later| *later == slot).count();
            if count > 1 {
                result.push((first, count));
            }
        }
        result
    }

    fn check(slots: &[Vec<RequirementPlan>], expected: &[(usize, usize)]) {
        assert_eq!(suffix_count_oracle(slots), expected, "oracle fixture");
        assert_eq!(
            closed_multiplicities(slots),
            expected,
            "complete stable output"
        );
    }

    #[test]
    fn stable_grouping_covers_empty_unique_equal_and_first_repeat_order() {
        let a = ordinary_plan();
        let b = RequirementPlan {
            max_depth: 3,
            ..a.clone()
        };
        check(&[], &[]);
        check(&[Vec::new()], &[]);
        check(&[vec![a.clone()]], &[]);
        check(&[vec![a.clone()], vec![b.clone()]], &[]);
        check(&[vec![a.clone()], vec![a.clone()]], &[(0, 2)]);
        check(&vec![vec![a.clone()]; 17], &[(0, 17)]);
        let unique: Vec<_> = (1..=32)
            .map(|max_depth| {
                vec![RequirementPlan {
                    max_depth,
                    ..a.clone()
                }]
            })
            .collect();
        check(&unique, &[]);
        // B first repeats before A, but A's first slot determines output order.
        let interleaved = [vec![a.clone()], vec![b.clone()], vec![b], vec![a]];
        check(&interleaved, &[(0, 2), (1, 2)]);
    }

    #[test]
    fn stable_grouping_ignores_ineligible_slots_without_renumbering() {
        let a = ordinary_plan();
        let b = RequirementPlan {
            max_depth: 3,
            ..a.clone()
        };
        let quest = RequirementPlan {
            quests: 1,
            ..a.clone()
        };
        let no_open_source = RequirementPlan {
            open_deadline: None,
            ..a.clone()
        };
        let sum = RequirementPlan {
            requirement: Requirement {
                level_sum: Some(LevelSum {
                    group: 1,
                    minimum_total: 3,
                }),
                ..a.requirement
            },
            ..a.clone()
        };
        let ineligible = [
            Vec::new(),
            vec![quest.clone()],
            vec![quest.clone()],
            vec![a.clone(), a.clone()],
            vec![no_open_source.clone()],
            vec![sum.clone()],
            vec![sum.clone()],
        ];
        check(&ineligible, &[]);
        let mut just_one = ineligible.to_vec();
        just_one.push(vec![a.clone()]);
        check(&just_one, &[]);
        let slots = [
            Vec::new(),
            vec![quest],
            vec![a.clone(), a.clone()],
            vec![a.clone()],
            vec![sum.clone()],
            vec![b.clone()],
            vec![no_open_source],
            vec![b],
            vec![sum],
            vec![a],
            Vec::new(),
        ];
        check(&slots, &[(3, 2), (5, 2)]);
    }

    type Change = fn(&mut RequirementPlan);

    fn predicate_changes() -> [(&'static str, Change); 12] {
        [
            ("kind", |p| p.requirement.kind = ItemKind::Armor),
            ("weapon_category_melee", |p| {
                p.requirement.weapon_category = Some(WeaponCategory::Melee);
            }),
            ("weapon_category_thrown", |p| {
                p.requirement.weapon_category = Some(WeaponCategory::Thrown);
            }),
            ("item", |p| p.requirement.item = Some(ItemId::Sword)),
            ("tier_exact", |p| {
                p.requirement.tier = TierRequirement::Exact(2);
            }),
            ("tier_at_least", |p| {
                p.requirement.tier = TierRequirement::AtLeast(2);
            }),
            ("tier_at_most", |p| {
                p.requirement.tier = TierRequirement::AtMost(2);
            }),
            ("upgrade_exact", |p| {
                p.requirement.upgrade = UpgradeRequirement::Exact(2);
            }),
            ("upgrade_at_least", |p| {
                p.requirement.upgrade = UpgradeRequirement::AtLeast(2);
            }),
            ("effect_weapon", |p| {
                p.requirement.effect =
                    EffectRequirement::exactly(Effect::Weapon(WeaponEffect::Blazing));
            }),
            ("effect_armor", |p| {
                p.requirement.effect =
                    EffectRequirement::exactly(Effect::Armor(ArmorEffect::Obfuscation));
            }),
            ("require_uncursed", |p| {
                p.requirement.require_uncursed = true;
            }),
        ]
    }

    fn metadata_changes() -> [(&'static str, Change); 12] {
        [
            ("select_trinket", |p| p.requirement.select_trinket = true),
            ("source_chest", |p| {
                p.requirement.source = Some(ItemSource::Chest);
            }),
            ("source_none", |p| p.requirement.source = None),
            ("identity_group_1", |p| {
                p.requirement.identity_group = Some(1);
            }),
            ("identity_group_2", |p| {
                p.requirement.identity_group = Some(2);
            }),
            ("requirement_max_depth_3", |p| {
                p.requirement.max_depth = Some(3);
            }),
            ("requirement_max_depth_none", |p| {
                p.requirement.max_depth = None;
            }),
            ("alternative_group_1", |p| {
                p.requirement.alternative_group = Some(1);
            }),
            ("alternative_group_2", |p| {
                p.requirement.alternative_group = Some(2);
            }),
            ("plan_max_depth", |p| p.max_depth = 3),
            ("vault", |p| p.vault = true),
            ("open_deadline", |p| p.open_deadline = Some(3)),
        ]
    }

    #[test]
    fn stable_grouping_compares_every_eligible_requirement_and_plan_field() {
        // Synthetic private plans isolate one field at a time. Some are not
        // valid public queries; the separate analyzer fixtures below are.
        let base = ordinary_plan();
        let mut variants = vec![base.clone()];
        for (name, change) in predicate_changes().into_iter().chain(metadata_changes()) {
            let mut different = base.clone();
            change(&mut different);
            assert_ne!(base, different, "field mutation {name}");
            check(
                &[
                    vec![base.clone()],
                    vec![different.clone()],
                    vec![different.clone()],
                    vec![base.clone()],
                ],
                &[(0, 2), (1, 2)],
            );
            variants.push(different);
        }
        // Also separate e.g. Exact(2) from AtLeast(2), Some(1) from Some(2),
        // and the two EffectSet families, not just each variant from Any/None.
        for (index, plan) in variants.iter().enumerate() {
            assert!(!variants[..index].contains(plan));
        }
        let slots: Vec<_> = variants
            .iter()
            .chain(variants.iter().rev())
            .map(|plan| vec![plan.clone()])
            .collect();
        let expected: Vec<_> = (0..variants.len()).map(|index| (index, 2)).collect();
        check(&slots, &expected);
    }

    #[test]
    fn stable_grouping_excludes_all_optional_and_quest_field_variants() {
        let base = ordinary_plan();
        let mut excluded = Vec::new();
        // quests has no two unequal eligible values: every nonzero mask is
        // excluded. Likewise every Some(level_sum) and None(open_deadline).
        for quests in [1, 2, 4, 8, 16, u8::MAX] {
            excluded.push(RequirementPlan {
                quests,
                ..base.clone()
            });
        }
        excluded.push(RequirementPlan {
            open_deadline: None,
            ..base.clone()
        });
        for (group, minimum_total) in [(1, 3), (2, 3), (1, 4)] {
            excluded.push(RequirementPlan {
                requirement: Requirement {
                    level_sum: Some(LevelSum {
                        group,
                        minimum_total,
                    }),
                    ..base.requirement
                },
                ..base.clone()
            });
        }
        for plan in &excluded {
            assert_ne!(plan, &base);
            check(&[vec![plan.clone()], vec![plan.clone()]], &[]);
            check(
                &[
                    vec![plan.clone()],
                    vec![base.clone()],
                    vec![plan.clone()],
                    vec![base.clone()],
                ],
                &[(1, 2)],
            );
        }
    }

    fn analyze(requirements: Vec<Requirement>, max_depth: u8) -> QueryPlan {
        let query = SearchQuery {
            auto_apply_trinket: false,
            requirements,
            max_depth,
            challenges: Challenges::NONE,
            require_blacksmith: false,
            exclude_blacksmith_rewards: false,
            wandmaker_quest: None,
        };
        query.validate().expect("valid structural analyzer fixture");
        let plan = QueryPlan::analyze(&query);
        assert!(!plan.is_unsatisfiable());
        assert_eq!(plan.closed_multiplicities, suffix_count_oracle(&plan.slots));
        plan
    }

    #[test]
    fn stable_grouping_agrees_for_valid_or_quest_and_sum_slots() {
        let ordinary = ordinary_plan().requirement;
        let alternative = Requirement {
            alternative_group: Some(1),
            ..ordinary
        };
        let quest = Requirement {
            source: None,
            max_depth: None,
            ..ordinary
        };
        let optional = Requirement {
            kind: ItemKind::Ring,
            level_sum: Some(LevelSum {
                group: 1,
                minimum_total: 3,
            }),
            ..ordinary
        };
        let plan = analyze(
            vec![
                alternative,
                alternative,
                ordinary,
                quest,
                optional,
                ordinary,
                quest,
                optional,
            ],
            24,
        );
        assert_eq!(plan.slots.len(), 7);
        assert_eq!(plan.slots[0].len(), 2);
        assert_ne!(plan.slots[2][0].quests, 0);
        assert_ne!(plan.slots[5][0].quests, 0);
        assert!(plan.slots[3][0].requirement.level_sum.is_some());
        assert!(plan.slots[6][0].requirement.level_sum.is_some());
        check(&plan.slots, &[(1, 2)]);
    }

    #[test]
    fn stable_grouping_handles_bounded_large_valid_effect_sets() {
        // Bounded quadratic oracle work; this is correctness, not a complexity
        // benchmark. No private raw masks or invalid/mixed-family EffectSets.
        const DISTINCT: usize = 512;
        let effects: Vec<_> = ALL_WEAPON_EFFECTS
            .iter()
            .copied()
            .filter(|effect| !effect.is_curse())
            .take(10)
            .map(Effect::Weapon)
            .collect();
        assert_eq!(effects.len(), 10);
        let base = ordinary_plan().requirement;
        let unique: Vec<_> = (1..=DISTINCT)
            .map(|mask| {
                let set =
                    EffectSet::from_effects(effects.iter().copied().enumerate().filter_map(
                        |(index, effect)| (mask & (1 << index) != 0).then_some(effect),
                    ))
                    .expect("nonempty same-family set");
                Requirement {
                    effect: EffectRequirement::OneOf(set),
                    ..base
                }
            })
            .collect();
        let unique_plan = analyze(unique.clone(), 4);
        assert_eq!(unique_plan.slots.len(), DISTINCT);
        for slot in &unique_plan.slots {
            let [plan] = slot.as_slice() else {
                panic!("singleton valid query")
            };
            assert_eq!(plan.quests, 0);
            assert_eq!(plan.open_deadline, Some(4));
            assert!(!plan.vault);
        }
        check(&unique_plan.slots, &[]);

        let paired: Vec<_> = unique.iter().chain(unique.iter().rev()).copied().collect();
        let paired_plan = analyze(paired, 4);
        let pairs: Vec<_> = (0..DISTINCT).map(|index| (index, 2)).collect();
        check(&paired_plan.slots, &pairs);

        let interleaved: Vec<_> = unique.iter().flat_map(|&wanted| [wanted, wanted]).collect();
        let interleaved_plan = analyze(interleaved, 4);
        let interleaved_pairs: Vec<_> = (0..DISTINCT).map(|index| (index * 2, 2)).collect();
        check(&interleaved_plan.slots, &interleaved_pairs);
        check(
            &analyze(vec![unique[0]; DISTINCT], 4).slots,
            &[(0, DISTINCT)],
        );

        // Construction order does not change a set's logical or hashed key.
        let forward = EffectSet::from_effects(effects.iter().copied()).unwrap();
        let reverse = EffectSet::from_effects(effects.iter().rev().copied()).unwrap();
        assert_eq!(forward, reverse);
        let equivalent = analyze(
            vec![
                Requirement {
                    effect: EffectRequirement::OneOf(forward),
                    ..base
                },
                Requirement {
                    effect: EffectRequirement::OneOf(reverse),
                    ..base
                },
            ],
            4,
        );
        check(&equivalent.slots, &[(0, 2)]);
    }
}
