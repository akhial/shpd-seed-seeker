//! Query probability estimates derived from measured deterministic item supply.
//!
//! The estimate answers "what fraction of seeds satisfies this query", not "how
//! likely is one item to match", so it has to know how much equipment a run
//! actually offers. That supply lives in [`crate::probability_tables`]: expected
//! reward slots per floor and source, with the upgrade, curse, enchantment, and
//! tier distributions each source produces.
//!
//! Every requirement becomes a filter over those slots. Floor limits carve the
//! dungeon into stretches, each holding its own supply, so two items wanted by
//! floor four compete over what those four floors offer rather than over the
//! whole run. Within a stretch a line's slots arrive as a run of independent
//! chances rather than a Poisson process, because the generator deals item
//! categories from a decrementing deck, and they all come out of that one run —
//! which is what stops two requirements from each being handed an item when only
//! one was ever produced. A shop places a fixed number of slots on a single
//! floor instead, and a shelf holding mutually exclusive stock counts once,
//! since a run can only carry one of them out.
//!
//! Requirements are then matched one-to-one onto slots, so three wands are not
//! scored as one wand three times and one shelf is not spent twice. Requirements
//! linked to one identity are summed over the identities they could share and
//! resolved alongside the rest of their family, discounted by the deck-driven
//! scarcity of duplicates.
//!
//! A slot's tier and its upgrade level are tabled apart and multiplied, which
//! holds wherever the generator rolls them apart. The Imp's two hoards do not:
//! its vault stocks four fixed shelves and its reward flips a coin over which
//! of two weapons is levelled furthest, so at both a tier names the levels it
//! can carry — including the `+5` that only ever lands on a tier-4 weapon.
//! Those sources carry a measured upgrade-per-tier table instead, and are
//! scored against it; every other source multiplies its two marginals.
//!
//! Quest prizes are not supply like that, and they are the whole reason
//! families cannot be resolved apart. A giver lays out a pool spanning every
//! family — the Imp's five reward options beside its vault's treasure rooms —
//! and the player carries exactly one item away, so the Imp's haul can answer
//! the `+4` ring a query wants or its `+3` armor, never both. Each pool is
//! therefore lifted out of the per-family matching and spent once across the
//! whole query, on whichever requirement it reaches that helps most.
//!
//! Selected trinkets choose a measured +3 profile per unique initial-offer
//! match. Those profiles include brewing timing and per-floor modifier shares;
//! ambiguous offers retain the canonical supply. See [`crate::probability_tables`].
//! Arcane Resin integrates generated upgrades and surplus resin in a joint
//! supply model. A wand is reserved or consumed, and later cheaper matches can
//! replace earlier reservations. Reward pools still supply just one choice.
//! Artifacts use their measured supply for single-item estimates. Joint
//! artifact requirements average valid identity assignments over sampled
//! anonymous layouts, retaining floor/source/curse filters and accessibility
//! scenarios. Identities are dealt without replacement from eleven cards;
//! two copies of an artifact are impossible. The layout distribution is
//! sampled separately from identity draws; identity-dependent RNG consumption
//! is not conditioned on. Mixed equipment/artifact pools retain the same
//! conservative allocation approximation as equipment-only pools.
//!
//! Coverage sets contain only distinct intersections of the requested filters,
//! and their slot counts grow with the query. Repeated requirements share a
//! coverage set but still need separate items; no query-size cutoff is used.
//!
//! Known simplifications: challenges shift item placement but are ignored. A
//! pool is spent on its single best use rather than on whichever of them the
//! seed left open. Overlapping reward filters are nested; disjoint offers are
//! approximated independently conditional on the quest appearing. Duplicate
//! scarcity is measured by weapon tier for the canonical profile and by whole
//! line otherwise, so a linked group whose members want very different
//! items — one `+3` alongside two plain ones — is discounted as heavily as one
//! wanting three alike. Those approximations read low.

mod artifacts;
mod blankets;
mod cache;
mod coverage;
mod floors;
mod resin;

use coverage::Coverages;

use std::cmp::Ordering;
use std::collections::BTreeMap;

use crate::catalog::{Effect, ItemId, ItemKind, WeaponCategory, item};
use crate::equipment::{
    ARMOR_COMMON, ARMOR_CURSES, ARMOR_RARE, ARMOR_UNCOMMON, WEAPON_COMMON, WEAPON_CURSES,
    WEAPON_RARE, WEAPON_UNCOMMON,
};
use crate::generator::{
    ARMOR_ITEMS, ARTIFACT_ITEMS, RING_ITEMS, WAND_ITEMS, WEAPON_TIER_1_ITEMS, WEAPON_TIER_2_ITEMS,
    WEAPON_TIER_3_ITEMS, WEAPON_TIER_4_ITEMS, WEAPON_TIER_5_ITEMS,
};
use crate::model::ItemSource;
use crate::probability_tables::{
    DEEPEST_FLOOR, DEPTHS, FLOOR_SETS, HIGHEST_TABLED_UPGRADE, HIGHEST_TIER, IDENTITY_REPEAT_LIMIT,
    LINES_ORDER, Line, PRIZE_GROUPS, PrizeGroup, Supply, TIERS, TIPPED_DARTS, kind_index, line_of,
    missile_tier, missile_tier_items, prize_group, source_index, spread_index, tipped_index,
};
use crate::probability_tables::{line_index, trinkets::Profile};
use crate::query::{EffectRequirement, Requirement, SearchQuery, UpgradeRequirement};
use crate::quests::WandmakerQuestType;

/// Estimates the fraction of seeds satisfying a query.
///
/// The result is fixed for a search: observed results never feed back into it.
/// Returns `NaN` for filters the supply model cannot estimate or witness
/// assignments exceeding its bounded work budget.
///
/// Alternative groups are approximated by their most plentiful member — a
/// pessimistic simplification, since any member can satisfy the group.
/// Combined-level groups collapse to their cheapest sufficient subset: the
/// fewest members that can reach the total, each carrying an equal share of
/// it — pessimistic, since lopsided splits and larger subsets also satisfy
/// the group.
#[must_use]
pub fn estimate_match_probability(query: &SearchQuery) -> f64 {
    if let Some(policy) = crate::auto_trinkets::AutoTrinketPolicy::prepare(query) {
        return crate::auto_trinkets::probability(query, &policy);
    }
    if query
        .requirements
        .iter()
        .any(|r| r.kind == ItemKind::Trinket)
    {
        return trinket_probability(query);
    }
    equipment_probability(query, Profile::None)
}

pub(crate) fn equipment_probability(query: &SearchQuery, profile: Profile) -> f64 {
    if query.floor_requirements.is_empty() {
        return equipment_probability_without_floors(query, profile);
    }
    let floor_probability = floors::probability(query, profile);
    let mut items = query.clone();
    items.floor_requirements.clear();
    floor_probability * equipment_probability_without_floors(&items, profile)
}

fn equipment_probability_without_floors(query: &SearchQuery, profile: Profile) -> f64 {
    if query.requirements.iter().any(|r| r.blanket) {
        return blankets::probability(query, profile);
    }
    if query.needs_resin() {
        return resin::probability(query, profile);
    }
    let requirements = effective_requirements(query, profile);
    let mut linked: BTreeMap<u8, Vec<Requirement>> = BTreeMap::new();
    let mut independent: Vec<Requirement> = Vec::new();
    for requirement in requirements {
        match requirement.identity_group {
            Some(group) => linked.entry(group).or_default().push(requirement),
            None => independent.push(requirement),
        }
    }

    let mut probability =
        blacksmith_probability(query, profile) * wandmaker_quest_probability(query);
    let mut groups: Vec<Vec<Requirement>> = Vec::new();
    for members in linked.into_values() {
        // A linked group that names its item constrains nothing extra: every
        // member already matches that one identity. Neither does a group of
        // one, which has nothing to agree with.
        if let Some(pinned) = members.iter().find_map(|member| member.item) {
            independent.extend(members.into_iter().map(|member| Requirement {
                item: Some(pinned),
                ..member
            }));
        } else if members.len() < 2 {
            independent.extend(members);
        } else {
            groups.push(members);
        }
    }
    for members in groups {
        // Requirements draw on the same items whether or not they are linked,
        // and a quest prize is one pick across every family, so the group is
        // resolved alongside everything left rather than as though they never
        // met. Later groups are then resolved on their own: nesting the sums
        // over the identities each could take would cost their product.
        let others = std::mem::take(&mut independent);
        probability *= linked_probability(query, &members, &others, profile);
    }
    probability *= competing_probability(query, &independent, profile);
    if probability <= 0.0 {
        0.0
    } else {
        probability.min(1.0)
    }
}

/// Average over the 2,380 equally likely four-card subsets of the private
/// trinket deck. Matching consumes identities, so overlapping alternatives and
/// repeated requirements cannot reuse one offer. The catalyst's common floor
/// is averaged once, rather than independently for each requirement.
///
/// Equipment remains a supply-table estimate, treated as independent of this
/// private deck and the catalyst's placement/accessibility. For mixed-family
/// alternatives, keep the best feasible residual equipment query; like the
/// equipment estimator's alternatives this is conservative, not a union of
/// all ways equipment could complete the query. Explicit catalyst source
/// filters have no measured distribution and remain unsupported.
fn trinket_probability(query: &SearchQuery) -> f64 {
    use crate::catalog::ITEMS;

    if query.requirements.iter().any(|requirement| {
        requirement.kind == ItemKind::Trinket
            && (requirement.source.is_some() || requirement.level_sum.is_some())
    }) {
        return f64::NAN;
    }
    let identities: Vec<_> = ITEMS
        .iter()
        .filter(|definition| {
            definition.kind == ItemKind::Trinket && definition.id != ItemId::TrinketCatalyst
        })
        .map(|definition| definition.id)
        .collect();
    let ordinary_count = query.ordinary_slots().len();
    let slots: Vec<_> = query
        .ordinary_slots()
        .into_iter()
        .chain(query.blanket_slots())
        .collect();
    let selection_slots = crate::trinkets::selection_slots(query);
    let selected_mask = identities
        .iter()
        .enumerate()
        .fold(0u32, |mask, (index, id)| {
            mask | if selection_slots
                .iter()
                .flatten()
                .any(|r| r.item == Some(*id))
            {
                1 << index
            } else {
                0
            }
        });
    let equipment_slots: Vec<_> = slots
        .iter()
        .map(|members| {
            members
                .iter()
                .any(|&index| query.requirements[index].kind != ItemKind::Trinket)
        })
        .collect();
    let mut residuals = BTreeMap::new();
    let mut total = 0.0;
    let mut samples = 0u32;
    for depth in 1..=3 {
        let masks: Vec<u32> = slots
            .iter()
            .map(|members| trinket_mask(query, members, &identities, depth))
            .collect();
        for a in 0..identities.len() {
            for b in a + 1..identities.len() {
                for c in b + 1..identities.len() {
                    for d in c + 1..identities.len() {
                        let available = (1 << a) | (1 << b) | (1 << c) | (1 << d);
                        let profile = Profile::for_matches(available & selected_mask, &identities);
                        total += complete_with_trinkets(
                            query,
                            &slots,
                            &masks[..ordinary_count],
                            &equipment_slots,
                            available,
                            0,
                            &masks[ordinary_count..],
                            &mut Vec::new(),
                            &mut residuals,
                            profile,
                        );
                        samples += 1;
                    }
                }
            }
        }
    }
    total / f64::from(samples)
}

/// Offers that could satisfy this slot at the catalyst's floor.
fn trinket_mask(query: &SearchQuery, members: &[usize], identities: &[ItemId], depth: u8) -> u32 {
    use crate::model::{Accessibility, WorldItem};
    identities
        .iter()
        .enumerate()
        .fold(0, |mask, (index, &identity)| {
            let candidate = WorldItem {
                item: identity,
                upgrade: 0,
                effect: None,
                cursed: false,
                depth,
                source: ItemSource::Heap,
                accessibility: Accessibility::Independent,
                secret: false,
            };
            let matches = members.iter().any(|&member| {
                let requirement = query.requirements[member];
                requirement.kind == ItemKind::Trinket
                    && depth
                        <= query
                            .max_depth
                            .min(requirement.max_depth.unwrap_or(query.max_depth))
                    && requirement.matches(&candidate)
            });
            mask | if matches { 1 << index } else { 0 }
        })
}

/// Try distinct trinkets in successive query slots, retaining equipment-only
/// slots for the table estimator. Cache each residual query across all decks.
#[allow(clippy::too_many_arguments)]
fn complete_with_trinkets(
    query: &SearchQuery,
    slots: &[Vec<usize>],
    masks: &[u32],
    equipment_slots: &[bool],
    available: u32,
    used: u32,
    blanket_masks: &[u32],
    residual: &mut Vec<usize>,
    cache: &mut BTreeMap<(Profile, Vec<usize>), f64>,
    profile: Profile,
) -> f64 {
    let ordinary_count = slots.len() - blanket_masks.len();
    let slot = ordinary_count - masks.len();
    let Some((&mask, tail)) = masks.split_first() else {
        let mut remaining = residual.clone();
        for (index, &mask) in blanket_masks.iter().enumerate() {
            if mask & used == 0 {
                let index = ordinary_count + index;
                if !equipment_slots[index] {
                    return 0.0;
                }
                remaining.push(index);
            }
        }
        return *cache
            .entry((profile, remaining.clone()))
            .or_insert_with(|| {
                let requirements: Vec<_> = remaining
                    .iter()
                    .flat_map(|&slot| {
                        slots[slot]
                            .iter()
                            .map(|&index| query.requirements[index])
                            .filter(|requirement| requirement.kind != ItemKind::Trinket)
                    })
                    .collect();
                equipment_probability(
                    &SearchQuery {
                        requirements,
                        ..query.clone()
                    },
                    profile,
                )
            });
    };
    let mut best: f64 = 0.0;
    let mut choices = mask & available;
    while choices != 0 {
        let choice = 1 << choices.trailing_zeros();
        choices &= !choice;
        let completed = complete_with_trinkets(
            query,
            slots,
            tail,
            equipment_slots,
            available & !choice,
            used | choice,
            blanket_masks,
            residual,
            cache,
            profile,
        );
        if completed.is_nan() {
            return completed;
        }
        best = best.max(completed);
    }
    if equipment_slots[slot] {
        residual.push(slot);
        let completed = complete_with_trinkets(
            query,
            slots,
            tail,
            equipment_slots,
            available,
            used,
            blanket_masks,
            residual,
            cache,
            profile,
        );
        residual.pop();
        if completed.is_nan() {
            return completed;
        }
        best = best.max(completed);
    }
    best
}

/// Requirements reduced to the flat, independent form the supply tables can
/// answer: each alternative group collapses to its most plentiful member, and
/// each combined-level group collapses to its cheapest sufficient subset —
/// the fewest members whose level capacity reaches the total, each tightened
/// to carry an equal share. Members the subset does not need are optional in
/// the matcher and are dropped here.
fn effective_requirements(query: &SearchQuery, profile: Profile) -> Vec<Requirement> {
    // For each group: how many members a satisfying subset needs at least,
    // given the members' level capacities, and the upgrade each of those
    // members then has to carry.
    let mut group_members: BTreeMap<u8, Vec<u8>> = BTreeMap::new();
    let mut group_totals: BTreeMap<u8, u8> = BTreeMap::new();
    for requirement in &query.requirements {
        if let Some(sum) = requirement.level_sum {
            group_members
                .entry(sum.group)
                .or_default()
                .push(requirement.maximum_level());
            group_totals.entry(sum.group).or_insert(sum.minimum_total);
        }
    }
    let mut group_plan: BTreeMap<u8, (usize, u8)> = BTreeMap::new();
    for (group, mut capacities) in group_members {
        capacities.sort_unstable_by(|a, b| b.cmp(a));
        let total = u16::from(group_totals.get(&group).copied().unwrap_or(0));
        let mut reached = 0u16;
        let mut needed = 0usize;
        for capacity in &capacities {
            if reached >= total {
                break;
            }
            reached += u16::from(*capacity);
            needed += 1;
        }
        let needed = needed.max(1);
        // Levels each taken member must average; its upgrade is one less.
        let share = total.div_ceil(u16::try_from(needed).unwrap_or(1));
        let implied_upgrade = u8::try_from(share.saturating_sub(1)).unwrap_or(u8::MAX);
        group_plan.insert(group, (needed, implied_upgrade));
    }
    let mut taken: BTreeMap<u8, usize> = BTreeMap::new();
    let mut alternatives: BTreeMap<u8, Requirement> = BTreeMap::new();
    let mut flattened: Vec<Requirement> = Vec::new();
    for requirement in &query.requirements {
        let mut requirement = *requirement;
        if let Some(sum) = requirement.level_sum.take() {
            let (needed, implied) = group_plan.get(&sum.group).copied().unwrap_or((1, 0));
            let already = taken.entry(sum.group).or_insert(0);
            if *already >= needed {
                // An optional member the cheapest subset does not use.
                continue;
            }
            *already += 1;
            requirement.upgrade = match requirement.upgrade {
                UpgradeRequirement::Any => UpgradeRequirement::AtLeast(implied),
                UpgradeRequirement::AtLeast(minimum) => {
                    UpgradeRequirement::AtLeast(minimum.max(implied))
                }
                exact @ UpgradeRequirement::Exact(_) => exact,
            };
        }
        match requirement.alternative_group.take() {
            None => flattened.push(requirement),
            Some(group) => {
                let slots = |candidate: &Requirement| {
                    expected_slots(
                        &Predicate::of(*candidate, None)
                            .within(query, candidate)
                            .with_profile(profile),
                    )
                };
                let replace = alternatives
                    .get(&group)
                    .is_none_or(|kept| slots(&requirement) > slots(kept));
                if replace {
                    alternatives.insert(group, requirement);
                }
            }
        }
    }
    flattened.extend(alternatives.into_values());
    flattened
}

/// Probability that an accessible Blacksmith exists within the search depth.
fn blacksmith_probability(query: &SearchQuery, profile: Profile) -> f64 {
    if !query.require_blacksmith {
        return 1.0;
    }
    profile
        .supply_for(ItemKind::Armor)
        .filter(|supply| supply.source == ItemSource::BlacksmithReward)
        .map(|supply| {
            supply.depth_slots[..usize::from(query.max_depth).min(DEPTHS)]
                .iter()
                .map(|slots| f64::from(*slots))
                .sum::<f64>()
                / f64::from(supply.bundle.max(1))
        })
        .sum::<f64>()
        .clamp(0.0, 1.0)
}

/// Probability that the Wandmaker spawns within the search depth *and* rolls
/// the demanded quest.
///
/// This factor is exact rather than measured. `Wandmaker.Quest.spawnRoom`
/// draws `Int(10 - depth) == 0` on each Prison floor above six, so the giver
/// arrives on floor seven one time in three, on floor eight one of the
/// remaining two times, and on floor nine always. The variant is then a flat
/// `Int(3)` over the three quests, independent of the floor.
fn wandmaker_quest_probability(query: &SearchQuery) -> f64 {
    if query.wandmaker_quest.is_none() {
        return 1.0;
    }
    let mut missed = 1.0;
    for depth in WandmakerQuestType::WINDOW {
        if depth > query.max_depth {
            break;
        }
        missed *= 1.0 - 1.0 / f64::from(10 - u16::from(depth));
    }
    (1.0 - missed) / 3.0
}

/// Probability that requirements sharing an identity group are all satisfied.
///
/// Every member has to resolve to the same item, so the group is evaluated once
/// per candidate identity and the results combined. Same-identity duplicates are
/// rarer than independent draws suggest because the generator deals items from
/// decrementing decks, which [`crate::probability_tables::IDENTITY_REPEATS`] corrects for.
fn linked_probability(
    query: &SearchQuery,
    members: &[Requirement],
    others: &[Requirement],
    profile: Profile,
) -> f64 {
    let Some(kind) = members.first().map(|member| member.kind) else {
        return 1.0;
    };
    let mut none = 1.0;
    for (identity, alike) in identities(kind) {
        let shared = together_probability(query, members, Some(identity), others, profile);
        none *= (1.0 - shared.clamp(0.0, 1.0)).powi(alike);
    }
    1.0 - none
}

/// How much rarer it is to hold several items of one identity than independent
/// draws suggest.
///
/// The generator deals each family from a decrementing deck, so drawing a wand
/// makes the same wand less likely next time. Requirements that all name one
/// item — or that are linked to share one — feel that suppression.
///
/// [`crate::probability_tables::IDENTITY_REPEATS`] is measured against independent draws, so a family
/// asking for copies of one item is resolved on that footing too: the run of
/// chances a line's slots normally arrive on already carries some of the same
/// scarcity, and counting it twice would make duplicates look far rarer than
/// they are.
///
/// The table counts how many sets of copies a world offers rather than how
/// often it offers any, since only the former survives the upgrade and curse
/// filters a query puts on top. [`thinned_by`] puts the matching's answer on
/// the same footing, applies the scarcity there, and reads it back.
fn repeat_correction(ordered: &[Predicate], holding: f64, copies: usize) -> f64 {
    let Some((repeated, _)) = repeated_identity(ordered) else {
        return holding;
    };
    // The filters can span families now, so the scarcity is read off the
    // family the repeated identity belongs to rather than whichever filter
    // sorted first.
    let kind = item(repeated).kind;
    let depth = ordered
        .iter()
        .map(|predicate| predicate.max_depth)
        .max()
        .unwrap_or(DEEPEST_FLOOR);
    let line = spread_index(kind, line_for(kind, repeated));
    let copies = copies.min(IDENTITY_REPEAT_LIMIT);
    let depth = usize::from(depth).clamp(1, DEPTHS) - 1;
    let scarcity = if ordered[0].profile == Profile::None
        && kind == ItemKind::Weapon
        && let Some(tier) = item(repeated).tier
    {
        crate::probability_tables::weapon_repeats::REPEATS
            [line_index(line_for(kind, repeated)) * TIERS + usize::from(tier) - 1][copies - 1]
            [depth]
    } else {
        f64::from(ordered[0].profile.repeat(line, copies - 1, depth))
    };
    thinned_by(holding, copies, scarcity)
}

/// Applies a scarcity measured on sets of `copies` items to a chance of holding
/// that many.
///
/// A world that barely ever has the copies offers about one set when it does,
/// so the scarcity multiplies straight through. A world that usually has them
/// to spare offers several, and thinning those still leaves it some. Reading
/// the answer back as a stream of arrivals gives the count of sets to thin;
/// a run holding sets that scarce holds at least one about `1 - e^-sets` of
/// the time.
///
/// Nothing converts the thinned count back into a chance of holding `copies`
/// exactly, because a deck that suppresses duplicates rarely hands over more
/// than the copies asked for: once they are that scarce, offering a set and
/// holding one are close to the same event.
fn thinned_by(holding: f64, copies: usize, scarcity: f64) -> f64 {
    if holding <= 0.0 || copies == 0 {
        return holding.max(0.0);
    }
    let mut low = 0.0;
    let mut high = BUSIEST_RUN;
    for _ in 0..ARRIVAL_STEPS {
        let middle = f64::midpoint(low, high);
        if poisson_at_least(middle, copies) < holding {
            low = middle;
        } else {
            high = middle;
        }
    }
    let mean = f64::midpoint(low, high);
    let sets = (1..=copies).fold(scarcity, |sets, taken| sets * mean / tally(taken));
    let missing = (-sets.max(0.0)).exp();
    1.0 - missing
}

/// Chance of at least `count` arrivals from a stream of that average.
fn poisson_at_least(mean: f64, count: usize) -> f64 {
    let mut term = (-mean).exp();
    let mut below = term;
    for step in 1..count {
        term *= mean / tally(step);
        below += term;
    }
    (1.0 - below).clamp(0.0, 1.0)
}

/// Widest average the arrival count is read back as. Anything busier is already
/// certain to hand over the copies a query can ask for.
const BUSIEST_RUN: f64 = 64.0;

/// Bisection steps used to read an arrival count back from a probability.
const ARRIVAL_STEPS: usize = 48;

/// The identity a family wants more than one of, with how many it wants.
fn repeated_identity(ordered: &[Predicate]) -> Option<(ItemId, usize)> {
    ordered
        .iter()
        .filter_map(|predicate| predicate.item)
        .fold(
            BTreeMap::new(),
            |mut counts: BTreeMap<ItemId, usize>, item| {
                *counts.entry(item).or_default() += 1;
                counts
            },
        )
        .into_iter()
        .max_by_key(|(_, copies)| *copies)
        .filter(|(_, copies)| *copies > 1)
}

/// Whether one generator line produces weapons of one melee/thrown class.
/// The plain line rolls wielded weapons; missiles and tipped darts are thrown.
const fn line_matches_category(line: Line, category: WeaponCategory) -> bool {
    match category {
        WeaponCategory::Melee => matches!(line, Line::Plain),
        WeaponCategory::Thrown => matches!(line, Line::Thrown | Line::Tipped),
    }
}

/// The line an identity belongs to. Only weapons have more than one.
fn line_for(kind: ItemKind, item: ItemId) -> Line {
    if kind == ItemKind::Weapon {
        line_of(item)
    } else {
        Line::Plain
    }
}

fn vault_identity_probability(wanted: ItemId, identities: &[ItemId], excluded: &[ItemId]) -> f64 {
    if !identities.contains(&wanted) || excluded.contains(&wanted) {
        0.0
    } else {
        1.0 / tally(identities.len() - excluded.len())
    }
}

/// Probability that every requirement outside a linked group is satisfied at
/// once.
///
/// Families are resolved together rather than one at a time: they draw on
/// separate supply, but a quest lays its prizes out across all of them and
/// lets exactly one leave, so a ring and an armor both wanting the Imp's haul
/// are competitors.
fn competing_probability(
    query: &SearchQuery,
    requirements: &[Requirement],
    profile: Profile,
) -> f64 {
    together_probability(query, requirements, None, &[], profile)
}

/// Probability that every requirement is satisfied by a distinct item.
///
/// The quest prizes are spent across the whole query and the rest of the
/// supply is matched family by family; see [`matching_chance`].
fn together_probability(
    query: &SearchQuery,
    requirements: &[Requirement],
    identity: Option<ItemId>,
    others: &[Requirement],
    profile: Profile,
) -> f64 {
    let group = filters(query, requirements, identity, &[], profile);
    let Some((_, copies)) = repeated_identity(&group) else {
        return matching_chance(&filters(query, requirements, identity, others, profile));
    };
    // Copies of one identity are scored against independent draws, since that is
    // the footing [`repeat_correction`] was measured on. The scarcity is read off
    // the group on its own, because what it corrects is how often one identity
    // turns up that many times — not how the rest of the family fares alongside.
    let alone = matching_chance(&group);
    if alone <= 0.0 {
        return 0.0;
    }
    let scarcer = repeat_correction(&group, alone, copies) / alone;
    if others.is_empty() {
        return (alone * scarcer).clamp(0.0, 1.0);
    }
    let together = matching_chance(&filters(query, requirements, identity, others, profile));
    (together * scarcer).clamp(0.0, 1.0)
}

/// The requirements reduced to filters, scarcest first.
fn filters(
    query: &SearchQuery,
    requirements: &[Requirement],
    identity: Option<ItemId>,
    others: &[Requirement],
    profile: Profile,
) -> Vec<Predicate> {
    let mut ordered: Vec<Predicate> = requirements
        .iter()
        .map(|requirement| {
            Predicate::of(*requirement, identity)
                .within(query, requirement)
                .with_profile(profile)
        })
        .chain(others.iter().map(|requirement| {
            Predicate::of(*requirement, None)
                .within(query, requirement)
                .with_profile(profile)
        }))
        .collect();
    sort_filters(&mut ordered);
    ordered
}

fn sort_filters(ordered: &mut [Predicate]) {
    ordered.sort_by(|left, right| {
        expected_slots(left)
            .partial_cmp(&expected_slots(right))
            .unwrap_or(Ordering::Equal)
    });
}

/// Probability that the supply can serve every filter with a distinct item.
///
/// The dungeon offers two kinds of supply. Scattered drops, chests and shop
/// shelves belong to one family each, so families never compete over them and
/// their answers multiply. A quest prize does not: the giver lays out a pool
/// spanning every family and the player carries exactly one item away, so the
/// Imp's haul can answer the `+4` ring a query wants or its `+3` armor, never
/// both.
///
/// So the prize pools are lifted out and resolved across the whole query,
/// while the supply that stays inside a family is still matched family by
/// family. Conditioning on what each pool reaches turns the estimate into a
/// sum over those reaches of the best use the query can make of them.
fn matching_chance(ordered: &[Predicate]) -> f64 {
    cache::matching(ordered, || matching_chance_uncached(ordered))
}

fn matching_chance_uncached(ordered: &[Predicate]) -> f64 {
    if ordered.is_empty() {
        return 1.0;
    }
    // An artifact leaves the global deck on its first draw. No combination of
    // room and quest supplies can produce a second copy of that identity.
    let mut artifacts = std::collections::BTreeSet::new();
    if ordered
        .iter()
        .filter(|predicate| predicate.kind == ItemKind::Artifact)
        .filter_map(|predicate| predicate.item)
        .any(|identity| !artifacts.insert(identity))
    {
        return 0.0;
    }
    // A named artifact can appear only once, so its expected matching count
    // is already its probability, including mutually exclusive room offers.
    if ordered.len() == 1 && ordered[0].kind == ItemKind::Artifact {
        return expected_slots(&ordered[0]).clamp(0.0, 1.0);
    }
    if artifacts.len() == ordered.len() {
        return artifacts::probability(ordered, false);
    }
    let deepest = ordered
        .iter()
        .map(|predicate| usize::from(predicate.max_depth).clamp(1, DEPTHS))
        .max()
        .unwrap_or(DEPTHS);
    let pools: Vec<PrizePool> = PRIZE_GROUPS
        .into_iter()
        .filter_map(|group| prize_reach(group, ordered, deepest))
        .collect();
    let mut open = OpenSupply {
        ordered,
        answered: BTreeMap::new(),
        families: BTreeMap::new(),
        prizes: BTreeMap::new(),
    };
    prize_chance(&pools, 0, &[], &mut open).clamp(0.0, 1.0)
}

/// The chance every requirement is served once the prize pools from `group`
/// onwards have been spent, given the requirements `discharged` by the pools
/// already spent.
///
/// A pool leaves one item, so it answers at most one requirement. Which one is
/// the query's choice, so the pool is spent on the requirement whose removal
/// helps most and that it can actually reach, walking them best-first: the
/// pool takes the best it reaches, and only misses out entirely when it
/// reaches none of them.
///
/// Overlapping filters retain nested reaches: offering an item matching the
/// narrower filter also reaches the broader one. Disjoint offers use an
/// independent approximation conditional on the quest appearing, so a different
/// wanted identity can be offered when the preferred one is absent. Either
/// partition spends the pool only once.
fn prize_chance(
    pools: &[PrizePool],
    group: usize,
    discharged: &[usize],
    open: &mut OpenSupply,
) -> f64 {
    let Some(pool) = pools.get(group) else {
        return open.chance(discharged);
    };
    let reach = &pool.reach;
    if let Some(answer) = open.prizes.get(&(group, discharged.to_vec())) {
        return *answer;
    }
    // Best-first over the requirements this pool could still be spent on.
    let mut spending = Vec::new();
    let mut considered = Vec::new();
    for (requirement, &reached) in reach.iter().enumerate() {
        if discharged.contains(&requirement)
            || reached <= 0.0
            || considered.contains(&open.ordered[requirement])
        {
            continue;
        }
        // Identical copies are interchangeable, but spending a prize still
        // removes only one of them. Keep a canonical choice for the cache.
        considered.push(open.ordered[requirement]);
        let mut served = discharged.to_vec();
        let position = served.partition_point(|&index| index < requirement);
        served.insert(position, requirement);
        spending.push((reached, prize_chance(pools, group + 1, &served, open)));
    }
    spending.sort_by(|left, right| right.1.partial_cmp(&left.1).unwrap_or(Ordering::Equal));

    // Each requirement claims only the offers left by better uses. What no
    // requirement reaches falls through to the next pool.
    let mut claimed = 0.0_f64;
    let mut total = 0.0;
    for (reached, served) in spending {
        let newly_reached = if pool.disjoint {
            reached * (1.0 - claimed / pool.appeared.max(f64::EPSILON)).max(0.0)
        } else {
            (reached - claimed).max(0.0)
        };
        total += newly_reached * served;
        claimed += newly_reached;
    }
    total += (1.0 - claimed) * prize_chance(pools, group + 1, discharged, open);
    open.prizes.insert((group, discharged.to_vec()), total);
    total
}

/// The family-by-family matching over everything a quest prize is not, with
/// the answers it has already worked out.
struct OpenSupply<'a> {
    ordered: &'a [Predicate],
    answered: BTreeMap<Vec<usize>, f64>,
    families: BTreeMap<Vec<usize>, f64>,
    prizes: BTreeMap<(usize, Vec<usize>), f64>,
}

impl OpenSupply<'_> {
    /// Chance the supply outside the prize pools serves every requirement
    /// except the `discharged` ones, which the pools have already answered.
    fn chance(&mut self, discharged: &[usize]) -> f64 {
        if let Some(answer) = self.answered.get(discharged) {
            return *answer;
        }
        let mut families: BTreeMap<usize, Vec<usize>> = BTreeMap::new();
        for (requirement, predicate) in self.ordered.iter().enumerate() {
            if !discharged.contains(&requirement) {
                families
                    .entry(kind_index(predicate.kind))
                    .or_default()
                    .push(requirement);
            }
        }
        let spent_artifacts = self
            .ordered
            .iter()
            .enumerate()
            .filter(|(index, predicate)| {
                predicate.kind == ItemKind::Artifact && discharged.contains(index)
            })
            .count();
        let answer: f64 = families
            .into_values()
            .map(|family| {
                let conditional = if self.ordered[family[0]].kind == ItemKind::Artifact {
                    if family.len() + spent_artifacts > artifact_identity_count() {
                        return 0.0;
                    }
                    (0..family.len()).fold(1.0, |factor, used| {
                        factor * tally(artifact_identity_count() - used)
                            / tally(artifact_identity_count() - spent_artifacts - used)
                    })
                } else {
                    1.0
                };
                let chance = self.families.entry(family.clone()).or_insert_with(|| {
                    let predicates: Vec<_> =
                        family.iter().map(|&index| self.ordered[index]).collect();
                    open_chance(&predicates)
                });
                (*chance * conditional).min(1.0)
            })
            .product();
        let answer = first_floor_probability(self.ordered, discharged, answer).min(1.0);
        self.answered.insert(discharged.to_vec(), answer);
        answer
    }
}

/// Before quests and shop stock, the same small first-floor room budget
/// supplies every family. Baked co-obtainable subset counts capture that
/// competition without scanning sampled worlds on the interface thread.
fn first_floor_probability(ordered: &[Predicate], discharged: &[usize], probability: f64) -> f64 {
    let mut bare = true;
    let mut counts = [0usize; 4];
    let mut common_upgrades = None;
    let mut same_upgrades = true;
    for (_, predicate) in ordered
        .iter()
        .enumerate()
        .filter(|(i, _)| !discharged.contains(i))
    {
        if predicate.max_depth != 1 || predicate.kind == ItemKind::Artifact {
            return probability;
        }
        counts[kind_index(predicate.kind)] += 1;
        same_upgrades &= common_upgrades.is_none_or(|upgrades| upgrades == predicate.upgrades);
        common_upgrades = Some(predicate.upgrades);
        bare &= predicate.item.is_none()
            && predicate.weapon_category.is_none()
            && predicate.tiers == (1 << HIGHEST_TIER) - 1
            && predicate.effect == EffectRequirement::Any
            && !predicate.require_uncursed
            && predicate.source.is_none();
    }
    if counts.iter().sum::<usize>() > 4 {
        return probability;
    }
    let key: usize = counts
        .into_iter()
        .zip([1, 5, 25, 125])
        .map(|(count, power)| count * power)
        .sum();
    let minimum = common_upgrades.filter(|_| same_upgrades).and_then(|mask| {
        (0..4).find(|minimum| {
            mask == (((1 << (HIGHEST_TABLED_UPGRADE + 1)) - 1) & !((1 << minimum) - 1))
        })
    });
    bare &= minimum.is_some();
    let key = key + minimum.unwrap_or(0) * 625;
    let table = crate::probability_tables::first_floor::CROSS_FAMILY;
    let index = table.binary_search_by_key(&key, |row| row.0).or_else(|_| {
        bare = false;
        table.binary_search_by_key(&(key % 625), |row| row.0)
    });
    let Ok(index) = index else {
        return probability;
    };
    let (_, independent, presence_ratio, moment_ratio) = table[index];
    if bare {
        return independent * presence_ratio;
    }
    // Broad filters see presence, rare filters thin the factorial moments.
    // Interpolate between those measured limits using the query's selectivity.
    let broad = (probability / independent).clamp(0.0, 1.0);
    probability * (broad * presence_ratio + (1.0 - broad) * moment_ratio)
}

/// Probability that one family's own supply serves every one of its filters
/// with a distinct item.
///
/// Each reward slot in the dungeon covers some set of the requirements, and the
/// query succeeds exactly when the slots can be matched one-to-one onto the
/// requirements. An augmenting-path matching checks that every requirement
/// can take a distinct slot, including when their accepted items overlap.
///
/// Working in coverage sets rather than per requirement is what stops one slot
/// from being spent twice: a shop shelf can hold the `+0` wand a query asks for
/// or one of its plain wands, never both.
fn open_chance(ordered: &[Predicate]) -> f64 {
    cache::open(ordered, || open_chance_uncached(ordered))
}

fn open_artifact_chance(ordered: &[Predicate]) -> f64 {
    if ordered.len() == 1 {
        let predicate = ordered[0];
        return predicate
            .profile
            .supply_for(ItemKind::Artifact)
            .filter(|supply| prize_group(supply.source).is_none())
            .flat_map(|supply| {
                supply
                    .depth_slots
                    .into_iter()
                    .enumerate()
                    .map(move |(depth, slots)| {
                        f64::from(slots) * predicate.slot_probability(&supply, depth + 1)
                    })
            })
            .sum::<f64>()
            .clamp(0.0, 1.0);
    }
    artifacts::probability(ordered, true)
}

fn open_chance_uncached(ordered: &[Predicate]) -> f64 {
    let Some(kind) = ordered.first().map(|predicate| predicate.kind) else {
        return 1.0;
    };
    if kind == ItemKind::Artifact {
        return open_artifact_chance(ordered);
    }
    let shared = Coverages::of(ordered);
    let coverages = shared.len();

    // Floor limits carve the dungeon into stretches that different requirements
    // can reach. Each is its own supply: two items wanted by floor four compete
    // over what those four floors hold, not over the whole run. Nothing past the
    // deepest floor any requirement accepts can serve the query at all.
    let mut limits: Vec<usize> = ordered
        .iter()
        .map(|predicate| usize::from(predicate.max_depth).clamp(1, DEPTHS))
        .collect();
    limits.sort_unstable();
    limits.dedup();

    let steady = repeated_identity(ordered).is_none();
    let measured_source = ordered[0].source.filter(|source| {
        steady && limits.len() == 1 && ordered.iter().all(|p| p.source == Some(*source))
    });
    let mut streams: Vec<Stream> = Vec::new();
    // A shop stocks one melee weapon, one missile, and one tipped dart. Keep
    // those fixed lines separate instead of replacing them with three random
    // weapon draws. A shop restocks on every shop floor, so its floors are not
    // alternatives the way a quest's are — and a quest's prizes are not here at
    // all, having been lifted out to [`prize_reach`].
    let mut bundles: BTreeMap<(usize, usize, usize), (u8, Vec<f64>)> = BTreeMap::new();
    for (line, (from, until)) in LINES_ORDER
        .into_iter()
        .flat_map(|line| stretches(&limits).map(move |stretch| (line, stretch)))
    {
        let mut placed = 0.0;
        let mut covered = vec![0.0; coverages];
        for supply in ordered[0].profile.supply_for(kind).filter(|supply| {
            supply.line == line && measured_source.is_none_or(|source| source == supply.source)
        }) {
            if prize_group(supply.source).is_some() {
                continue;
            }
            for depth in from..=until {
                let available = f64::from(supply.depth_slots[depth - 1]);
                if available <= 0.0 {
                    continue;
                }
                if supply.bundle == 0 {
                    placed += available;
                }
                let covered_by = shared.shares(&supply, depth);
                if covered_by.iter().skip(1).all(|share| *share <= 0.0) {
                    continue;
                }
                if supply.bundle == 0 {
                    for (coverage, share) in covered_by.iter().enumerate().skip(1) {
                        covered[coverage] += available * share;
                    }
                    continue;
                }
                let fixed_shop_line = supply.source == ItemSource::Shop && kind == ItemKind::Weapon;
                let size = if fixed_shop_line { 1 } else { supply.bundle };
                let appearances = available / f64::from(size);
                let bundle = bundles
                    .entry((
                        source_index(supply.source),
                        depth,
                        if fixed_shop_line { line_index(line) } else { 0 },
                    ))
                    .or_insert_with(|| (size, vec![0.0; coverages]));
                for (coverage, share) in covered_by.iter().enumerate().skip(1) {
                    bundle.1[coverage] += appearances * share;
                }
            }
        }
        if covered.iter().skip(1).any(|mass| *mass > 0.0) {
            streams.push(Stream::of(
                ordered[0].profile,
                spread_index(kind, line),
                until,
                placed,
                covered,
                steady,
                measured_source.and_then(|source| {
                    crate::probability_tables::source_counts::histogram(
                        ordered[0].profile as usize,
                        kind,
                        line,
                        source,
                        until,
                    )
                }),
            ));
        }
    }
    let mut slots: Vec<Slot> = Vec::new();
    for (bundle, covers) in bundles.into_values() {
        let claimed: f64 = covers.iter().skip(1).sum();
        for _ in 0..bundle {
            slots.push(Slot {
                // A slot covers one set of requirements at most, so pooling the
                // lines cannot leave it more than fully spoken for.
                covers: covers.iter().map(|mass| mass / claimed.max(1.0)).collect(),
            });
        }
    }
    matching_probability(&shared, ordered.len(), &streams, &slots).clamp(0.0, 1.0)
}

/// How likely one quest's prize pool is to be able to serve each requirement.
///
/// The giver lays out a whole pool — the Imp's five reward options beside its
/// vault's treasure rooms — and the player leaves with one item, so what
/// matters per requirement is whether anything in the pool would answer it.
/// Working that union out per floor and weighting it by the chance the quest
/// landed there keeps a floor limit that cuts the quest's window from claiming
/// the prize.
///
/// Returns `None` when nothing the pool holds can serve the query.
struct PrizePool {
    reach: Vec<f64>,
    appeared: f64,
    disjoint: bool,
}

fn prize_reach(group: PrizeGroup, ordered: &[Predicate], deepest: usize) -> Option<PrizePool> {
    let mut kinds: Vec<ItemKind> = ordered.iter().map(|predicate| predicate.kind).collect();
    kinds.sort_unstable_by_key(|kind| kind_index(*kind));
    kinds.dedup();
    let mut reach = vec![0.0; ordered.len()];
    let mut appearances = 0.0;
    for depth in 1..=deepest {
        // Every row of one quest shares its giver's floor distribution, so any
        // of them reports the chance the prize is waiting on this floor.
        let mut appeared = 0.0_f64;
        let mut missing = vec![1.0; ordered.len()];
        let mut shared_rolls = vec![1.0_f64; ordered.len()];
        for supply in kinds
            .iter()
            .flat_map(|kind| ordered[0].profile.supply_for(*kind))
            .filter(|supply| prize_group(supply.source) == Some(group))
        {
            let available = f64::from(supply.depth_slots[depth - 1]);
            if available <= 0.0 {
                continue;
            }
            appeared = appeared.max(available);
            for (requirement, predicate) in ordered.iter().enumerate() {
                let reached = predicate.slot_probability(&supply, depth);
                if group == PrizeGroup::Blacksmith && predicate.kind == supply.kind {
                    // Melee weapons, the missile, and armor share the level
                    // roll. Weapon lines also share their enchantment. Union
                    // their identities first, then spend the shared roll once.
                    let rolled = (predicate.upgrade_probability(&supply)
                        * predicate.effect_probability(&supply)
                        * predicate.uncursed_probability(&supply))
                    .clamp(0.0, 1.0);
                    shared_rolls[requirement] = rolled;
                    if rolled > 0.0 {
                        missing[requirement] *= 1.0 - (reached / rolled).clamp(0.0, 1.0);
                    }
                } else {
                    missing[requirement] *= 1.0 - reached;
                }
            }
        }
        if appeared <= 0.0 {
            continue;
        }
        appearances += appeared;
        for (requirement, missed) in missing.into_iter().enumerate() {
            reach[requirement] += appeared * (1.0 - missed) * shared_rolls[requirement];
        }
    }
    if reach.iter().all(|chance| *chance <= 0.0) {
        return None;
    }
    for chance in &mut reach {
        *chance = chance.clamp(0.0, 1.0);
    }
    // Separate identities can each be offered by the same quest. Nesting
    // their reaches suppresses every fallback to a different wanted item.
    // Approximate disjoint offers independently, conditional on the quest
    // appearing. Shared Blacksmith rolls and overlapping filters stay nested.
    let active: Vec<_> = ordered
        .iter()
        .zip(&reach)
        .filter(|(_, p)| **p > 0.0)
        .map(|(p, _)| p)
        .collect();
    let disjoint = group != PrizeGroup::Blacksmith
        && active.iter().enumerate().all(|(i, p)| {
            active[..i]
                .iter()
                .all(|other| p.intersect(**other).is_none())
        });
    Some(PrizePool {
        reach,
        appeared: appearances.min(1.0),
        disjoint,
    })
}

/// The stretches of floors the query's limits carve out, as inclusive ranges.
fn stretches(limits: &[usize]) -> impl Iterator<Item = (usize, usize)> + '_ {
    limits
        .iter()
        .scan(1, |from, until| {
            let stretch = (*from, *until);
            *from = until + 1;
            Some(stretch)
        })
        .filter(|(from, until)| from <= until)
}

/// One reward slot, with the chance it covers each set of requirements.
struct Slot {
    covers: Vec<f64>,
}

/// The scattered supply of one generator line.
///
/// A line deals its items from a decrementing deck, so its slots arrive as a run
/// of independent chances rather than a Poisson process: the same average, but
/// far less likely to hand over three items where one was expected. All of the
/// line's slots come out of that one run, which is what stops two requirements
/// from each being handed their own item as though the other had not taken one.
struct Stream {
    /// Chances the line takes, or `None` when its count is spread widely enough
    /// that random arrivals describe it just as well.
    trials: Option<f64>,
    /// Expected slots covering each set of requirements.
    covered: Vec<f64>,
    placed: f64,
    histogram: Option<crate::probability_tables::source_counts::Histogram>,
}

impl Stream {
    fn of(
        profile: Profile,
        line: usize,
        reach: usize,
        placed: f64,
        covered: Vec<f64>,
        steady: bool,
        histogram: Option<crate::probability_tables::source_counts::Histogram>,
    ) -> Self {
        let steadiness = f64::from(profile.spread(line, reach - 1)).clamp(0.0, 1.0);
        let chance = 1.0 - steadiness;
        let trials = placed / chance;
        let runs = steady && chance > 0.0 && trials <= MAX_CHANCES && placed > 0.0;
        Self {
            trials: runs.then_some(trials),
            covered,
            placed,
            histogram,
        }
    }

    /// Folds this line's slots into the states reached so far.
    ///
    /// The sets are taken one at a time out of the same run of chances, each
    /// drawing on what the earlier ones left. That is what keeps two
    /// requirements from both being handed an item when the line only ever
    /// produced one, and it fades out on its own as the run grows longer.
    fn fold(&self, states: States, cap: usize, capacities: &[usize]) -> States {
        // A different line or depth stretch has its own arrival budget. Build
        // this stream's distribution before combining it with existing stock;
        // subtracting the incoming state's items would spend other streams'
        // trials a second time.
        let empty = vec![0; self.covered.len()].into_boxed_slice();
        let distribution = self.distribution(BTreeMap::from([(empty, 1.0)]), cap);
        // Once the stream has spent its own trials, surplus items with the
        // same coverage are interchangeable. Keep only as many as could be
        // assigned to that coverage's requirements before convolving streams.
        // Applying this earlier would change the stream's remaining trials.
        let mut compact = BTreeMap::new();
        for (state, reached) in distribution {
            let state = state
                .iter()
                .zip(capacities)
                .map(|(count, capacity)| (*count).min(*capacity))
                .collect::<Vec<_>>()
                .into_boxed_slice();
            accumulate(&mut compact, state, reached);
        }
        if states.len() == 1
            && let Some((held, weight)) = states.first_key_value()
            && held.iter().all(|count| *count == 0)
        {
            for chance in compact.values_mut() {
                *chance *= weight;
            }
            return compact;
        }
        let mut combined = BTreeMap::new();
        for (held, reached) in states {
            for (arrived, share) in &compact {
                let state = held
                    .iter()
                    .zip(arrived.iter())
                    .zip(capacities)
                    .map(|((a, b), capacity)| (a + b).min(*capacity))
                    .collect::<Vec<_>>()
                    .into_boxed_slice();
                accumulate(&mut combined, state, reached * share);
            }
        }
        prune(combined)
    }

    fn distribution(&self, states: States, cap: usize) -> States {
        if let Some(histogram) = self.histogram {
            let mut combined = BTreeMap::new();
            for (count, weight) in histogram.probabilities().filter(|(_, p)| *p > 0.0) {
                let stream = Self {
                    trials: Some(tally(count)),
                    covered: self
                        .covered
                        .iter()
                        .map(|mean| mean / self.placed * tally(count))
                        .collect(),
                    placed: tally(count),
                    histogram: None,
                };
                for (state, chance) in stream.distribution(states.clone(), cap) {
                    accumulate(&mut combined, state, chance * weight);
                }
            }
            return prune(combined);
        }
        let mut states = states;
        // How much of the run earlier sets have taken: the share of its chances
        // they claimed, and the slots they took that a state cannot record.
        let mut claimed = 0.0_f64;
        let mut hidden = 0.0_f64;
        for (coverage, mean) in self.covered.iter().enumerate().skip(1) {
            if *mean <= 0.0 {
                continue;
            }
            let chance = self
                .trials
                .map(|trials| (mean / trials / (1.0 - claimed).max(f64::EPSILON)).clamp(0.0, 1.0));
            let mut arrivals: BTreeMap<usize, Vec<f64>> = BTreeMap::new();
            let mut next = BTreeMap::new();
            for (state, reached) in &states {
                let spent = state.iter().sum();
                let counts = arrivals
                    .entry(spent)
                    .or_insert_with(|| self.counts(tally(spent) + hidden, *mean, chance, cap));
                for (count, share) in counts.iter().enumerate() {
                    if *share > 0.0 {
                        accumulate(
                            &mut next,
                            add_count(state, coverage, count, cap),
                            reached * share,
                        );
                    }
                }
            }
            if let Some(trials) = self.trials {
                let typical = self.counts(hidden, *mean, chance, cap);
                let recorded: f64 = typical
                    .iter()
                    .enumerate()
                    .map(|(count, share)| tally(count) * share)
                    .sum();
                hidden += (mean - recorded).max(0.0);
                claimed += mean / trials;
            }
            states = prune(next);
        }
        states
    }

    /// Chances of each number of slots covering one set, out of what is left of
    /// the run once `spent` of its chances have gone.
    fn counts(&self, spent: f64, mean: f64, chance: Option<f64>, cap: usize) -> Vec<f64> {
        match (self.trials, chance) {
            (Some(trials), Some(chance)) => binomial_counts((trials - spent).max(0.0), chance, cap),
            _ => poisson_counts(mean, cap),
        }
    }
}

/// Probability that the slots can be matched one-to-one onto the requirements.
///
/// Each scattered line contributes its whole run of chances at once; quest and
/// shop slots are then folded in one at a time, each covering one set or
/// nothing. The surviving states admit a distinct item for every requirement.
fn matching_probability(
    coverages: &Coverages,
    wanted: usize,
    streams: &[Stream],
    slots: &[Slot],
) -> f64 {
    let cap = wanted;
    let capacities: Vec<_> = (0..coverages.len())
        .map(|coverage| coverages.members(coverage).len())
        .collect();
    let mut states = BTreeMap::from([(vec![0; coverages.len()].into_boxed_slice(), 1.0)]);
    for stream in streams {
        states = stream.fold(states, cap, &capacities);
    }
    for slot in slots {
        let missed = (1.0 - slot.covers.iter().skip(1).sum::<f64>()).max(0.0);
        let mut next = BTreeMap::new();
        for (state, reached) in &states {
            for (coverage, landed) in slot.covers.iter().enumerate().skip(1) {
                if *landed > 0.0 {
                    accumulate(
                        &mut next,
                        add_count(state, coverage, 1, capacities[coverage]),
                        reached * landed,
                    );
                }
            }
            accumulate(&mut next, state.clone(), reached * missed);
        }
        states = prune(next);
    }
    states
        .iter()
        .filter(|(state, _)| coverages.matches(state))
        .map(|(_, reached)| reached)
        .sum::<f64>()
        .clamp(0.0, 1.0)
}

/// Slot counts for the query's distinct coverage sets, without fixed bit widths.
type States = BTreeMap<Box<[usize]>, f64>;

/// States below this carry no weight worth the work of tracking them.
const STATE_FLOOR: f64 = 1e-15;

/// Largest number of probability states kept between steps.
const STATE_LIMIT: usize = 4096;

fn add_count(state: &[usize], coverage: usize, count: usize, cap: usize) -> Box<[usize]> {
    let mut next: Box<[usize]> = Box::from(state);
    next[coverage] = (next[coverage] + count).min(cap);
    next
}

fn accumulate(states: &mut States, state: Box<[usize]>, reached: f64) {
    if reached > 0.0 {
        *states.entry(state).or_insert(0.0) += reached;
    }
}

fn prune(states: States) -> States {
    let mut kept: States = states
        .into_iter()
        .filter(|(_, reached)| *reached > STATE_FLOOR)
        .collect();
    if kept.len() > STATE_LIMIT {
        let mut weights: Vec<f64> = kept.values().copied().collect();
        weights.sort_by(|left, right| right.partial_cmp(left).unwrap_or(Ordering::Equal));
        let floor = weights[STATE_LIMIT];
        kept.retain(|_, reached| *reached > floor);
    }
    kept
}

/// Expected number of slots one requirement can draw on.
fn expected_slots(predicate: &Predicate) -> f64 {
    cache::mean(*predicate, || expected_slots_uncached(predicate))
}

fn expected_slots_uncached(predicate: &Predicate) -> f64 {
    predicate
        .profile
        .supply_for(predicate.kind)
        .map(|supply| {
            (1..=usize::from(predicate.max_depth).min(DEPTHS))
                .map(|depth| {
                    f64::from(supply.depth_slots[depth - 1])
                        * predicate.slot_probability(&supply, depth)
                })
                .sum::<f64>()
        })
        .sum()
}

/// One requirement reduced to the filters the supply tables can answer.
///
/// Tiers and upgrades become bit sets so that requirements can be intersected:
/// the matching needs to know which of them one item could serve at once.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
struct Predicate {
    profile: Profile,
    kind: ItemKind,
    weapon_category: Option<WeaponCategory>,
    item: Option<ItemId>,
    tiers: u8,
    upgrades: u8,
    effect: EffectRequirement,
    require_uncursed: bool,
    source: Option<ItemSource>,
    max_depth: u8,
    exclude_blacksmith: bool,
}

impl Predicate {
    fn with_profile(mut self, profile: Profile) -> Self {
        self.profile = profile;
        self
    }
    fn of(requirement: Requirement, identity: Option<ItemId>) -> Self {
        let mut tiers = 0;
        for tier in 1..=HIGHEST_TIER {
            if requirement.tier.matches(Some(tier)) {
                tiers |= 1 << (tier - 1);
            }
        }
        let mut upgrades = 0;
        for upgrade in 0..=HIGHEST_TABLED_UPGRADE {
            let matches = match requirement.upgrade {
                UpgradeRequirement::Any => true,
                UpgradeRequirement::Exact(wanted) => upgrade == wanted,
                UpgradeRequirement::AtLeast(minimum) => upgrade >= minimum,
            };
            if matches {
                upgrades |= 1 << upgrade;
            }
        }
        Self {
            profile: Profile::None,
            kind: requirement.kind,
            weapon_category: requirement.weapon_category,
            item: identity.or(requirement.item),
            tiers,
            upgrades,
            effect: requirement.effect,
            require_uncursed: requirement.require_uncursed,
            source: requirement.source,
            max_depth: requirement.max_depth.unwrap_or(DEEPEST_FLOOR),
            exclude_blacksmith: false,
        }
    }

    /// Narrows the filter with the query-wide settings.
    fn within(mut self, query: &SearchQuery, requirement: &Requirement) -> Self {
        self.max_depth = effective_depth(query, requirement);
        self.exclude_blacksmith = query.exclude_blacksmith_rewards;
        self
    }

    /// The filter matching exactly the items both accept, or `None` when no item
    /// can satisfy both.
    fn intersect(self, other: Self) -> Option<Self> {
        if self.kind != other.kind {
            return None;
        }
        let weapon_category = match (self.weapon_category, other.weapon_category) {
            (Some(left), Some(right)) if left != right => return None,
            (left, right) => left.or(right),
        };
        let item = match (self.item, other.item) {
            (Some(left), Some(right)) if left != right => return None,
            (left, right) => left.or(right),
        };
        let source = match (self.source, other.source) {
            (Some(left), Some(right)) if left != right => return None,
            (left, right) => left.or(right),
        };
        let effect = match (self.effect, other.effect) {
            (EffectRequirement::OneOf(left), EffectRequirement::OneOf(right)) => {
                EffectRequirement::OneOf(left.intersection(right)?)
            }
            (EffectRequirement::Any, other) | (other, EffectRequirement::Any) => other,
        };
        let tiers = self.tiers & other.tiers;
        let upgrades = self.upgrades & other.upgrades;
        let require_uncursed = self.require_uncursed || other.require_uncursed;
        let curses_only = match effect {
            EffectRequirement::OneOf(set) => set.is_curses_only(),
            EffectRequirement::Any => false,
        };
        if tiers == 0 || upgrades == 0 || (require_uncursed && curses_only) {
            return None;
        }
        Some(Self {
            profile: self.profile,
            kind: self.kind,
            weapon_category,
            item,
            tiers,
            upgrades,
            effect,
            require_uncursed,
            source,
            max_depth: self.max_depth.min(other.max_depth),
            exclude_blacksmith: self.exclude_blacksmith || other.exclude_blacksmith,
        })
    }

    /// Probability that one reward slot of `supply` on `depth` satisfies this
    /// filter.
    ///
    /// A slot holding mutually exclusive alternatives matches when any one of
    /// them does, since the query is free to claim whichever qualifies. Whether
    /// that is several chances or one depends on how the source rolls them: the
    /// Blacksmith upgrades its whole weapon rack together, so a `+3` there is a
    /// single chance however many weapons it lays out.
    fn slot_probability(mut self, supply: &Supply, depth: usize) -> f64 {
        if depth <= 2 {
            self.profile = Profile::None;
        }
        if self.kind != supply.kind
            || usize::from(self.max_depth) < depth
            || self.source.is_some_and(|wanted| wanted != supply.source)
            || (self.exclude_blacksmith && supply.source == ItemSource::BlacksmithReward)
        {
            return 0.0;
        }
        if self.weapon_category.is_some_and(|category| {
            self.kind == ItemKind::Weapon && !line_matches_category(supply.line, category)
        }) {
            return 0.0;
        }
        let mut stock = supply.tiers[((depth - 1) / 5).min(FLOOR_SETS - 1)];
        if supply.source == ItemSource::Shop
            && supply.line != Line::Tipped
            && matches!(self.kind, ItemKind::Weapon | ItemKind::Armor)
        {
            stock.fill(0.0);
            // The Imp shop is on 20, in the same five-floor bucket as shop
            // 16, but carries tier-five stock. Region averages mix the two.
            let tier = match depth {
                6 => 2,
                11 => 3,
                16 => 4,
                20 | 21 => 5,
                _ => return 0.0,
            };
            stock[tier - 1] = 1.0;
        }
        let tiers = &stock;
        let identity = self.identity_probability(supply, tiers);
        let modifiers = self.effect_probability(supply) * self.uncursed_probability(supply);
        let options = f64::from(supply.options);
        if self.kind == ItemKind::Artifact
            || (supply.source == ItemSource::VaultTreasure
                && matches!(self.kind, ItemKind::Wand | ItemKind::Ring)
                && (self.item.is_some() || self.upgrades.is_power_of_two()))
        {
            // Artifact offers and vault wands/rings have distinct identities.
            // The vault also holds only one wand/ring at each exact level,
            // so their matching counts are already presence probabilities.
            return (identity * options * self.upgrade_probability(supply) * modifiers)
                .clamp(0.0, 1.0);
        }
        if supply.shared_roll {
            // No source that rolls its alternatives as one locks their levels
            // to their tiers, so the identity keeps the tabled tier shares.
            let rolled = self.upgrade_probability(supply) * modifiers;
            rolled.clamp(0.0, 1.0) * (1.0 - (1.0 - identity.clamp(0.0, 1.0)).powf(options))
        } else {
            // Rounded f32 shares can sum to slightly more than one. A
            // negative miss chance raised to a fractional option count (as
            // in selected-trinket profiles) would turn the estimate into NaN.
            let matched = if supply.source == ItemSource::VaultTreasure
                && matches!(self.kind, ItemKind::Weapon | ItemKind::Armor)
            {
                self.vault_equipment_probability(supply, tiers)
            } else if supply.source == ItemSource::Chest
                && matches!(self.kind, ItemKind::Weapon | ItemKind::Armor)
            {
                self.chest_equipment_probability(supply, tiers, depth)
            } else {
                self.identity_and_upgrade_probability(supply, tiers) * modifiers
            }
            .clamp(0.0, 1.0);
            1.0 - (1.0 - matched).powf(options)
        }
    }

    /// Chest equipment reaches +3 only through a boosted room prize. Both
    /// `generated_high_prize` and the secret maze use the next region's tier
    /// distribution and clear curses before adding a level. Preserve measured
    /// overall marginals by subtracting that component from the lower levels.
    fn chest_equipment_probability(
        self,
        supply: &Supply,
        tiers: &[f32; TIERS],
        depth: usize,
    ) -> f64 {
        let high_share = f64::from(supply.upgrades[3]);
        let modifiers = self.effect_probability(supply) * self.uncursed_probability(supply);
        if high_share <= 0.0 {
            return self.identity_and_upgrade_probability(supply, tiers) * modifiers;
        }
        let advanced = crate::generator::FLOOR_SET_TIER_PROBABILITIES
            [((depth - 1) / 5 + 1).min(FLOOR_SETS - 1)]
        .map(|p| p / 100.0);
        let mut clean = *supply;
        clean.cursed = 0.0;
        let high = high_share
            * self.identity_probability(supply, &advanced)
            * self.effect_probability(&clean)
            * self.uncursed_probability(&clean);
        let total = self.identity_probability(supply, tiers) * modifiers;
        let low_share: f64 = supply.upgrades[..3]
            .iter()
            .enumerate()
            .filter(|(level, _)| self.upgrades & (1 << level) != 0)
            .map(|(_, p)| f64::from(*p))
            .sum();
        (total - high).max(0.0) * low_share / (1.0 - high_share).max(f64::EPSILON)
            + if self.upgrades & (1 << 3) != 0 {
                high
            } else {
                0.0
            }
    }

    /// Vault shelves jointly determine tier, level, and enchantment chance.
    /// The first melee weapon is one level ahead of its shelf; every other
    /// item has the shelf's level. Averaging enchantments across shelves would
    /// invent enchanted tier-two armor and overstate low-tier weapons.
    fn vault_equipment_probability(self, supply: &Supply, tiers: &[f32; TIERS]) -> f64 {
        let Some(levels) = supply.levels else {
            return 0.0;
        };
        let mut probability = 0.0;
        for (tier, levels) in levels.iter().enumerate() {
            let mut selected = [0.0; TIERS];
            selected[tier] = tiers[tier];
            let identity = self.identity_probability(supply, &selected);
            for (upgrade, share) in levels.iter().enumerate() {
                if self.upgrades & (1 << upgrade) == 0 || *share == 0.0 {
                    continue;
                }
                let loot_tier = if supply.kind == ItemKind::Weapon
                    && supply.line == Line::Plain
                    && upgrade == tier + 1
                {
                    upgrade.saturating_sub(1)
                } else {
                    upgrade
                };
                probability += identity
                    * f64::from(*share)
                    * self
                        .effect_probability_with_enchantment(supply, tally(loot_tier.min(3)) / 3.0)
                    * self.uncursed_probability(supply);
            }
        }
        probability
    }

    /// Probability that one alternative of `supply` is an item this filter
    /// accepts, identity and upgrade together.
    ///
    /// Tiers and upgrades are tabled apart and multiply, save at a source that
    /// [locks the two together](crate::probability_tables::locks_levels_to_tiers):
    /// those carry their own upgrade-per-tier table and are scored against it.
    /// Every level the generator ties to one tier is stocked by such a source,
    /// which `only_a_locked_row_reaches_the_level_tied_to_one_tier` holds the
    /// tables to, so nothing outside them needs a tier and a level reconciled.
    fn identity_and_upgrade_probability(self, supply: &Supply, tiers: &[f32; TIERS]) -> f64 {
        let tiered = matches!(supply.kind, ItemKind::Weapon | ItemKind::Armor);
        let Some(levels) = supply.levels.filter(|_| tiered) else {
            return self.upgrade_probability(supply) * self.identity_probability(supply, tiers);
        };
        // Each tier is worth only its own share of the levels this filter
        // accepts, not the whole source's: asking for a tier and a level that
        // never came off the same shelf has to score zero.
        let allowed = self.upgrades;
        let mut reachable = [0.0_f32; TIERS];
        for ((share, tabled), carried) in reachable.iter_mut().zip(tiers).zip(levels) {
            let held: f64 = carried
                .iter()
                .enumerate()
                .filter(|(upgrade, _)| allowed & (1 << upgrade) != 0)
                .map(|(_, chance)| f64::from(*chance))
                .sum();
            #[allow(clippy::cast_possible_truncation)]
            {
                *share = (f64::from(*tabled) * held) as f32;
            }
        }
        self.identity_probability(supply, &reachable)
    }

    fn identity_probability(self, supply: &Supply, tiers: &[f32; TIERS]) -> f64 {
        match (self.kind, self.item) {
            (ItemKind::Weapon, Some(wanted)) => {
                if line_of(wanted) != supply.line {
                    return 0.0;
                }
                // A tipped dart's identity is the plant seed it was tipped with,
                // which the generator does not hand out evenly.
                if let Some(dart) = tipped_index(wanted) {
                    return self.tier_probability(tiers) * f64::from(self.profile.tipped(dart));
                }
                let Some((tier, siblings)) = weapon_family(wanted) else {
                    return 0.0;
                };
                if self.tiers & (1 << (tier - 1)) == 0 {
                    return 0.0;
                }
                f64::from(tiers[usize::from(tier) - 1]) / tally(siblings)
            }
            // One generic armor exists per tier, so identity and tier coincide.
            (ItemKind::Armor, Some(wanted)) => item(wanted)
                .tier
                .filter(|tier| ARMOR_ITEMS.contains(&wanted) && self.tiers & (1 << (tier - 1)) != 0)
                .map_or(0.0, |tier| f64::from(tiers[usize::from(tier) - 1])),
            (ItemKind::Weapon | ItemKind::Armor, None) => self.tier_probability(tiers),
            (ItemKind::Wand, Some(wanted)) => {
                if supply.source == ItemSource::VaultTreasure {
                    vault_identity_probability(
                        wanted,
                        &WAND_ITEMS,
                        &crate::vault_loot::EXCLUDED_WANDS,
                    )
                } else if WAND_ITEMS.contains(&wanted) {
                    1.0 / tally(WAND_ITEMS.len())
                } else {
                    0.0
                }
            }
            (ItemKind::Ring, Some(wanted)) => {
                if supply.source == ItemSource::VaultTreasure {
                    let identities = RING_ITEMS.map(crate::run::RingKind::item_id);
                    vault_identity_probability(
                        wanted,
                        &identities,
                        &crate::vault_loot::EXCLUDED_RINGS,
                    )
                } else if RING_ITEMS.iter().any(|ring| ring.item_id() == wanted) {
                    1.0 / tally(RING_ITEMS.len())
                } else {
                    0.0
                }
            }
            (ItemKind::Wand | ItemKind::Ring, None) => 1.0,
            (ItemKind::Artifact, Some(wanted)) => {
                if ARTIFACT_ITEMS
                    .iter()
                    .any(|artifact| artifact.item_id() == Some(wanted))
                {
                    1.0 / tally(artifact_identity_count())
                } else {
                    0.0
                }
            }
            (ItemKind::Artifact, None) | (ItemKind::Trinket, _) => 0.0,
        }
    }

    fn tier_probability(self, tiers: &[f32; TIERS]) -> f64 {
        tiers
            .iter()
            .enumerate()
            .filter(|(index, _)| self.tiers & (1 << index) != 0)
            .map(|(_, share)| f64::from(*share))
            .sum()
    }

    /// The tabled share of the upgrade levels this filter accepts, over a
    /// source's items as a whole rather than one of its tiers.
    fn upgrade_probability(self, supply: &Supply) -> f64 {
        (0..supply.upgrades.len())
            .filter(|upgrade| self.upgrades & (1 << upgrade) != 0)
            .map(|upgrade| f64::from(supply.upgrades[upgrade]))
            .sum()
    }

    fn effect_probability(self, supply: &Supply) -> f64 {
        self.effect_probability_with_enchantment(supply, f64::from(supply.enchanted))
    }

    fn effect_probability_with_enchantment(self, supply: &Supply, enchanted: f64) -> f64 {
        let EffectRequirement::OneOf(set) = self.effect else {
            return 1.0;
        };
        // Uncursed items never carry curse effects, so those members of the
        // set can never be the match.
        let set = if self.require_uncursed {
            match set.without_curses() {
                Some(set) => set,
                None => return 0.0,
            }
        } else {
            set
        };
        // Each member is a disjoint outcome for one item, so their chances add.
        set.effects()
            .map(|effect| {
                if effect.is_curse() {
                    f64::from(supply.cursed) / curse_count(effect)
                } else {
                    enchanted * rarity_probability(effect)
                }
            })
            .sum()
    }

    fn uncursed_probability(self, supply: &Supply) -> f64 {
        if !self.require_uncursed {
            return 1.0;
        }
        match self.effect {
            // Positive enchantments and glyphs are generated only on clean
            // items, and `effect_probability` already dropped the curses.
            EffectRequirement::OneOf(_) => 1.0,
            EffectRequirement::Any => 1.0 - f64::from(supply.cursed),
        }
    }
}

fn effective_depth(query: &SearchQuery, requirement: &Requirement) -> u8 {
    requirement
        .max_depth
        .map_or(query.max_depth, |limit| limit.min(query.max_depth))
}

/// Tier of a melee or thrown weapon and how many identities share that tier.
/// `None` for anything the generator never produces.
fn weapon_family(wanted: ItemId) -> Option<(u8, usize)> {
    if let Some(tier) = melee_tier(wanted) {
        return Some((tier, melee_tier_items(tier).iter().flatten().count()));
    }
    missile_tier(wanted).map(|tier| {
        let siblings = missile_tier_items(tier)
            .iter()
            .filter(|kind| kind.item_id().is_some())
            .count();
        (tier, siblings)
    })
}

fn melee_tier(wanted: ItemId) -> Option<u8> {
    (1..=5).find(|tier| melee_tier_items(*tier).contains(&Some(wanted)))
}

fn melee_tier_items(tier: u8) -> &'static [Option<ItemId>] {
    match tier {
        1 => &WEAPON_TIER_1_ITEMS,
        2 => &WEAPON_TIER_2_ITEMS,
        3 => &WEAPON_TIER_3_ITEMS,
        4 => &WEAPON_TIER_4_ITEMS,
        _ => &WEAPON_TIER_5_ITEMS,
    }
}

/// Every identity a family can generate, collapsed onto one standing for each
/// group the supply tables cannot tell apart, with how many it stands for.
///
/// Weapons of one tier are drawn equally often, and so are wands and rings, so
/// resolving one of them and raising the answer to the size of its group is
/// exact — and much cheaper than resolving all forty-odd weapon identities.
fn artifact_identity_count() -> usize {
    ARTIFACT_ITEMS
        .iter()
        .filter(|artifact| artifact.item_id().is_some())
        .count()
}

fn identities(kind: ItemKind) -> Vec<(ItemId, i32)> {
    match kind {
        ItemKind::Trinket => Vec::new(),
        ItemKind::Artifact => ARTIFACT_ITEMS
            .iter()
            .filter_map(|artifact| artifact.item_id().map(|id| (id, 1)))
            .collect(),
        ItemKind::Weapon => (1..=HIGHEST_TIER)
            .filter_map(|tier| {
                let items = melee_tier_items(tier);
                Some((
                    *items.iter().flatten().next()?,
                    alike(items.iter().flatten().count()),
                ))
            })
            .chain((1..=HIGHEST_TIER).filter_map(|tier| {
                let items = missile_tier_items(tier);
                let first = items.iter().find_map(|kind| kind.item_id())?;
                let generated = items.iter().filter_map(|kind| kind.item_id()).count();
                Some((first, alike(generated)))
            }))
            // Tipped darts follow the plant seeds a run happens to grow, which
            // do not come up equally often.
            .chain(TIPPED_DART_IDS.map(|dart| (dart, 1)))
            .collect(),
        ItemKind::Armor => ARMOR_ITEMS.iter().map(|armor| (*armor, 1)).collect(),
        ItemKind::Wand => {
            partition_vault_identities(&WAND_ITEMS, &crate::vault_loot::EXCLUDED_WANDS)
        }
        ItemKind::Ring => partition_vault_identities(
            &RING_ITEMS.map(crate::run::RingKind::item_id),
            &crate::vault_loot::EXCLUDED_RINGS,
        ),
    }
}

fn partition_vault_identities(items: &[ItemId], excluded: &[ItemId]) -> Vec<(ItemId, i32)> {
    [false, true]
        .into_iter()
        .filter_map(|banned| {
            let mut members = items
                .iter()
                .copied()
                .filter(|id| excluded.contains(id) == banned);
            let first = members.next()?;
            Some((first, alike(1 + members.count())))
        })
        .collect()
}

/// Identities one representative stands for, as a power.
#[allow(clippy::cast_possible_truncation, clippy::cast_possible_wrap)]
fn alike(count: usize) -> i32 {
    count.min(i32::MAX as usize) as i32
}

/// Every tipped dart the generator can produce, in catalog order.
const TIPPED_DART_IDS: [ItemId; TIPPED_DARTS] = [
    ItemId::RotDart,
    ItemId::IncendiaryDart,
    ItemId::AdrenalineDart,
    ItemId::HealingDart,
    ItemId::ChillingDart,
    ItemId::ShockingDart,
    ItemId::PoisonDart,
    ItemId::CleansingDart,
    ItemId::ParalyticDart,
    ItemId::HolyDart,
    ItemId::DisplacingDart,
    ItemId::BlindingDart,
];

/// Curses are drawn uniformly from the family's list.
fn curse_count(effect: Effect) -> f64 {
    match effect {
        Effect::Weapon(_) => tally(WEAPON_CURSES.len()),
        Effect::Armor(_) => tally(ARMOR_CURSES.len()),
    }
}

/// Chance that one positive-effect draw lands on `effect`: rarities come up
/// 50/40/10 and each rarity picks uniformly from its list.
fn rarity_probability(effect: Effect) -> f64 {
    fn bucket_share<T: Copy + PartialEq>(effect: T, buckets: [&[T]; 3]) -> f64 {
        const RARITY_SHARES: [f64; 3] = [0.50, 0.40, 0.10];
        buckets
            .iter()
            .zip(RARITY_SHARES)
            .find(|(bucket, _)| bucket.contains(&effect))
            .map_or(0.0, |(bucket, share)| share / tally(bucket.len()))
    }
    match effect {
        Effect::Weapon(effect) => {
            bucket_share(effect, [&WEAPON_COMMON, &WEAPON_UNCOMMON, &WEAPON_RARE])
        }
        Effect::Armor(effect) => {
            bucket_share(effect, [&ARMOR_COMMON, &ARMOR_UNCOMMON, &ARMOR_RARE])
        }
    }
}

/// Table sizes and item counts are small enough to be exact in `f64`.
#[allow(clippy::cast_precision_loss)]
fn tally(count: usize) -> f64 {
    count as f64
}

/// Past this many chances a run is indistinguishable from a Poisson process.
const MAX_CHANCES: f64 = 64.0;

/// Chances of zero through `cap` arrivals, with everything past the cap folded
/// into the last bucket.
fn poisson_counts(mean: f64, cap: usize) -> Vec<f64> {
    let mut counts = vec![0.0; cap + 1];
    if mean <= 0.0 {
        counts[0] = 1.0;
        return counts;
    }
    let mut term = (-mean).exp();
    counts[0] = term;
    for (index, count) in counts.iter_mut().enumerate().skip(1) {
        term *= mean / tally(index);
        *count = term;
    }
    let overflow = 1.0 - counts.iter().sum::<f64>();
    counts[cap] += overflow.max(0.0);
    counts
}

fn binomial_counts(chances: f64, chance: f64, cap: usize) -> Vec<f64> {
    // The measured arrival budget can be fractional. Interpolate adjacent
    // integer distributions; extending the binomial recurrence to fractional
    // n otherwise creates excess mass and negative terms in the tail.
    let lower = chances.floor();
    let fraction = chances - lower;
    let mut counts = integer_binomial_counts(lower, chance, cap);
    if fraction > 0.0 {
        let upper = integer_binomial_counts(lower + 1.0, chance, cap);
        for (count, more) in counts.iter_mut().zip(upper) {
            *count = (1.0 - fraction) * *count + fraction * more;
        }
    }
    counts
}

fn integer_binomial_counts(chances: f64, chance: f64, cap: usize) -> Vec<f64> {
    let mut counts = vec![0.0; cap + 1];
    if chance >= 1.0 {
        let index = (0..cap).find(|&i| tally(i) >= chances).unwrap_or(cap);
        counts[index] = 1.0;
        return counts;
    }
    let mut term = (1.0 - chance).powf(chances);
    counts[0] = term;
    for (index, count) in counts.iter_mut().enumerate().skip(1) {
        let remaining = chances - tally(index) + 1.0;
        if remaining <= 0.0 {
            break;
        }
        term *= chance / (1.0 - chance) * remaining / tally(index);
        *count = term;
    }
    let overflow = 1.0 - counts.iter().sum::<f64>();
    counts[cap] += overflow.max(0.0);
    counts
}

#[cfg(test)]
mod tests {
    use crate::catalog::{ArmorEffect, Effect, ItemId, ItemKind};
    use crate::challenges::Challenges;
    use crate::model::ItemSource;
    use crate::query::{
        EffectRequirement, EffectSet, LevelSum, Requirement, SearchQuery, TierRequirement,
        UpgradeRequirement,
    };

    use super::{estimate_match_probability, rarity_probability};

    #[test]
    fn separate_item_streams_keep_their_own_arrival_budget() {
        let stream = super::Stream {
            trials: Some(1.0),
            covered: vec![0.0, 1.0],
            placed: 1.0,
            histogram: None,
        };
        // One guaranteed item already came from another line/depth stretch.
        let held = std::collections::BTreeMap::from([(vec![0, 1].into_boxed_slice(), 1.0)]);
        let combined = stream.fold(held, 2, &[0, 2]);
        assert!((combined[&vec![0, 2].into_boxed_slice()] - 1.0).abs() < 1e-12);
    }

    #[test]
    fn fractional_arrival_budgets_preserve_mass_and_mean() {
        for trials in [0.0, 0.3, 1.0, 1.5, 3.7] {
            for chance in [0.0, 0.25, 0.9, 1.0] {
                let counts = super::binomial_counts(trials, chance, 5);
                assert!(counts.iter().all(|p| p.is_finite() && *p >= 0.0));
                assert!((counts.iter().sum::<f64>() - 1.0).abs() < 1e-12);
                let mean: f64 = counts
                    .iter()
                    .enumerate()
                    .map(|(i, p)| super::tally(i) * p)
                    .sum();
                assert!((mean - trials * chance).abs() < 1e-12);
            }
        }
    }

    fn requirement(kind: ItemKind) -> Requirement {
        Requirement {
            kind,
            weapon_category: None,
            item: None,
            tier: TierRequirement::Any,
            upgrade: UpgradeRequirement::Any,
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
            floor_requirements: Vec::new(),
            auto_apply_trinket: false,
            arcane_resin_filter: crate::query::ArcaneResinFilter::default(),
            arcane_resin_auto: false,
            arcane_resin: 0,
            requirements,
            max_depth,
            challenges: Challenges::NONE,
            require_blacksmith: false,
            exclude_blacksmith_rewards: false,
            wandmaker_quest: None,
        }
    }

    fn trinket(identity: ItemId) -> Requirement {
        Requirement {
            item: Some(identity),
            ..requirement(ItemKind::Trinket)
        }
    }

    fn assert_probability(requirements: Vec<Requirement>, expected: f64) {
        let actual = estimate_match_probability(&query(requirements, 24));
        assert!((actual - expected).abs() < 1e-10, "{actual} vs {expected}");
    }

    #[test]
    fn rounded_shares_keep_fractional_option_counts_finite() {
        let mut supply = crate::probability_tables::trinkets::Profile::None
            .supply_for(ItemKind::Wand)
            .next()
            .unwrap();
        supply.upgrades = [0.1, 0.2, 0.3, 0.4, 0.0, 0.0];
        supply.options = 1.5;
        supply.shared_roll = false;
        let predicate = super::Predicate::of(requirement(ItemKind::Wand), None);
        assert!(predicate.upgrade_probability(&supply) > 1.0);
        assert!((predicate.slot_probability(&supply, 3) - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn blacksmith_weapon_lines_share_one_upgrade_and_enchantment_roll() {
        for (upgrade, expected) in [(1, 0.45), (2, 0.20), (3, 0.05)] {
            let mut wanted = requirement(ItemKind::Weapon);
            wanted.source = Some(ItemSource::BlacksmithReward);
            wanted.upgrade = UpgradeRequirement::Exact(upgrade);
            let p = estimate_match_probability(&query(vec![wanted], 24));
            assert!((p - expected).abs() < 0.003, "{upgrade}: {p}");
            wanted.effect =
                EffectRequirement::OneOf(EffectSet::enchantments(ItemKind::Weapon).unwrap());
            let p = estimate_match_probability(&query(vec![wanted], 24));
            assert!(
                (p - expected * 0.3).abs() < 0.003,
                "enchanted {upgrade}: {p}"
            );
        }
    }

    #[test]
    fn vault_exclusions_and_distinct_identity_draws_are_respected() {
        for id in crate::vault_loot::EXCLUDED_RINGS
            .into_iter()
            .chain(crate::vault_loot::EXCLUDED_WANDS)
        {
            let wanted = Requirement {
                item: Some(id),
                source: Some(ItemSource::VaultTreasure),
                ..requirement(crate::catalog::item(id).kind)
            };
            assert!(estimate_match_probability(&query(vec![wanted], 24)).abs() < f64::EPSILON);
        }
        let wanted = Requirement {
            item: Some(ItemId::RingHaste),
            source: Some(ItemSource::VaultTreasure),
            ..requirement(ItemKind::Ring)
        };
        let predicate = super::Predicate::of(wanted, None);
        let supply = crate::probability_tables::trinkets::Profile::None
            .supply_for(ItemKind::Ring)
            .find(|s| s.source == ItemSource::VaultTreasure)
            .unwrap();
        let expected = f64::from(supply.options) / 9.0;
        assert!((predicate.slot_probability(&supply, 19) - expected).abs() < 1e-7);
    }

    #[test]
    fn selected_trinkets_keep_mixed_query_estimates_available() {
        for document in [
            r#"{"requirements":[{"item":"mimic_tooth","select_trinket":true},{"kind":"weapon","upgrade":2}]}"#,
            r#"{"requirements":[{"any_of":[{"item":"mimic_tooth","select_trinket":true},{"item":"rat_skull"}]},{"kind":"ring"}]}"#,
            r#"{"requirements":[{"item":"parchment_scrap","select_trinket":true}]}"#,
        ] {
            let selected = crate::json_query::decode(document).unwrap();
            let estimate = estimate_match_probability(&selected);
            assert!(estimate.is_finite() && estimate > 0.0 && estimate <= 1.0);
        }
    }

    #[test]
    fn selected_profiles_change_loot_estimates_and_keep_prebrew_queries() {
        let estimate = |id: &str, selected: bool, equipment: &str, depth: u8| {
            estimate_match_probability(&crate::json_query::decode(&format!(
                r#"{{"requirements":[{{"item":"{id}","select_trinket":{selected}}},{equipment}],"max_depth":{depth}}}"#
            )).unwrap())
        };
        let mimic = r#"{"kind":"weapon","source":"mimic","upgrade":1}"#;
        assert!(
            estimate("mimic_tooth", true, mimic, 24) > estimate("mimic_tooth", false, mimic, 24)
        );
        let enchanted = r#"{"kind":"weapon","effect":"any_enchantment","source":"ghost_reward"}"#;
        assert!(
            estimate("parchment_scrap", true, enchanted, 9)
                > estimate("parchment_scrap", false, enchanted, 9)
        );
        let ring = r#"{"kind":"ring","source":"heap"}"#;
        assert!(
            estimate("cracked_spyglass", true, ring, 24)
                > estimate("cracked_spyglass", false, ring, 24)
        );
        for (id, depth) in [
            ("mimic_tooth", 1),
            ("parchment_scrap", 1),
            ("mimic_tooth", 2),
            ("parchment_scrap", 2),
            ("salt_cube", 24),
        ] {
            let equipment = r#"{"kind":"weapon"}"#;
            assert!(
                (estimate(id, true, equipment, depth) - estimate(id, false, equipment, depth))
                    .abs()
                    < 1e-12
            );
        }
    }

    #[test]
    fn selected_or_estimate_weights_unique_and_ambiguous_offers() {
        use crate::probability_tables::trinkets::Profile;
        let equipment = crate::json_query::decode(
            r#"{"requirements":[{"kind":"weapon","source":"mimic","upgrade":1}],"max_depth":24}"#,
        )
        .unwrap();
        let plain = super::equipment_probability(&equipment, Profile::None);
        let applied = super::equipment_probability(&equipment, Profile::MimicTooth);
        let either = crate::json_query::decode(r#"{"requirements":[{"any_of":[{"item":"mimic_tooth","select_trinket":true},{"item":"salt_cube"}]},{"kind":"weapon","source":"mimic","upgrade":1}],"max_depth":24}"#).unwrap();
        // 455 subsets contain only Mimic Tooth, 455 only Salt Cube (no
        // generation effect), and 105 contain both (ambiguous => No Trinket).
        let expected = (455.0 * applied + 560.0 * plain) / 2380.0;
        assert!((estimate_match_probability(&either) - expected).abs() < 1e-10);
        let mut two_effects = either;
        two_effects.requirements[1].item = Some(ItemId::ParchmentScrap);
        let parchment = super::equipment_probability(&equipment, Profile::ParchmentScrap);
        let expected = (455.0 * applied + 455.0 * parchment + 105.0 * plain) / 2380.0;
        assert!((estimate_match_probability(&two_effects) - expected).abs() < 1e-10);
    }

    #[test]
    fn trinket_subsets_have_exact_without_replacement_probabilities() {
        let cards = [
            ItemId::RatSkull,
            ItemId::MimicTooth,
            ItemId::SaltCube,
            ItemId::EyeOfNewt,
            ItemId::FerretTuft,
        ];
        assert_probability(vec![trinket(cards[0])], 4.0 / 17.0);
        assert_probability(
            cards[..2].iter().copied().map(trinket).collect(),
            3.0 / 68.0,
        );
        assert_probability(
            cards[..3].iter().copied().map(trinket).collect(),
            1.0 / 170.0,
        );
        assert_probability(
            cards[..4].iter().copied().map(trinket).collect(),
            1.0 / 2380.0,
        );
        assert_probability(cards.iter().copied().map(trinket).collect(), 0.0);
        assert_probability(vec![trinket(cards[0]); 2], 0.0);
    }

    #[test]
    fn trinket_alternatives_union_identities_and_preserve_distinct_assignment() {
        let alternative = |identity, group| Requirement {
            alternative_group: Some(group),
            ..trinket(identity)
        };
        let either = vec![
            alternative(ItemId::RatSkull, 1),
            alternative(ItemId::MimicTooth, 1),
        ];
        assert_probability(either.clone(), 29.0 / 68.0);
        // Repeating an OR member must not count its probability twice.
        assert_probability(vec![alternative(ItemId::RatSkull, 1); 2], 4.0 / 17.0);
        // A AND (A OR B) requires both A and B, not merely A.
        let mut overlap = either;
        overlap.push(trinket(ItemId::RatSkull));
        assert_probability(overlap, 3.0 / 68.0);
        // Two identical OR slots still need two distinct offers.
        assert_probability(
            vec![
                alternative(ItemId::RatSkull, 1),
                alternative(ItemId::MimicTooth, 1),
                alternative(ItemId::RatSkull, 2),
                alternative(ItemId::MimicTooth, 2),
            ],
            3.0 / 68.0,
        );
    }

    #[test]
    fn trinkets_share_one_catalyst_floor() {
        let early = Requirement {
            max_depth: Some(1),
            ..trinket(ItemId::RatSkull)
        };
        let later = Requirement {
            max_depth: Some(2),
            ..trinket(ItemId::MimicTooth)
        };
        assert_probability(vec![early, later], (3.0 / 68.0) / 3.0);
        let shallow = query(vec![trinket(ItemId::RatSkull)], 2);
        assert!((estimate_match_probability(&shallow) - (4.0 / 17.0) * (2.0 / 3.0)).abs() < 1e-10);
    }

    #[test]
    fn mixed_equipment_and_trinkets_have_finite_conditional_estimates() {
        let equipment = Requirement {
            item: Some(ItemId::RingMight),
            ..requirement(ItemKind::Ring)
        };
        let base = estimate_match_probability(&query(vec![equipment], 24));
        assert_probability(
            vec![equipment, trinket(ItemId::RatSkull)],
            base * 4.0 / 17.0,
        );
        assert_probability(
            vec![
                Requirement {
                    alternative_group: Some(1),
                    ..equipment
                },
                Requirement {
                    alternative_group: Some(1),
                    ..trinket(ItemId::RatSkull)
                },
            ],
            4.0 / 17.0 + (13.0 / 17.0) * base,
        );
    }

    #[test]
    fn trinket_queries_retain_world_conditions_once() {
        let mut wanted = query(vec![trinket(ItemId::RatSkull)], 24);
        wanted.wandmaker_quest = Some(crate::quests::WandmakerQuestType::Rotberry);
        assert!((estimate_match_probability(&wanted) - (4.0 / 17.0) / 3.0).abs() < 1e-10);
        wanted.max_depth = 6;
        assert!(estimate_match_probability(&wanted).abs() < f64::EPSILON);
    }

    /// A `+4` armor and a `+4` ring are each common enough — the Imp hands one
    /// of each out about a third of the time — but only the Imp reaches `+4`
    /// in those families, and the Escape Crystal lets exactly one item leave.
    /// Wanting both is therefore impossible, however plentiful each looks
    /// alone. Scoring the families apart put it at one seed in nine.
    #[test]
    fn one_quest_prize_cannot_answer_two_families_at_once() {
        let plus_four = |kind| Requirement {
            upgrade: UpgradeRequirement::AtLeast(4),
            ..requirement(kind)
        };
        let armor = estimate_match_probability(&query(vec![plus_four(ItemKind::Armor)], 24));
        let ring = estimate_match_probability(&query(vec![plus_four(ItemKind::Ring)], 24));
        assert!(armor > 0.2, "{armor:e}");
        assert!(ring > 0.2, "{ring:e}");
        let both = estimate_match_probability(&query(
            vec![plus_four(ItemKind::Armor), plus_four(ItemKind::Ring)],
            24,
        ));
        assert!(both <= 1e-9, "{both:e} for a pair no seed can satisfy");
    }

    /// The same pick cannot be spent twice within one family either: the Imp's
    /// reward armor and its vault's treasure armors are one choice, not two.
    #[test]
    fn one_quest_prize_cannot_answer_two_requirements_of_a_family() {
        let plus_three = || Requirement {
            upgrade: UpgradeRequirement::AtLeast(3),
            ..requirement(ItemKind::Armor)
        };
        let one = estimate_match_probability(&query(vec![plus_three()], 24));
        let two = estimate_match_probability(&query(vec![plus_three(), plus_three()], 24));
        // Sampled over 200000 seeds: 0.909 and 0.173, so the pair is far below
        // the 0.83 that scoring the two independently would give. The bands are
        // the sweep's own factor-of-two tolerance around those rates.
        assert!((0.45..=1.0).contains(&one), "{one:e}");
        assert!((0.086..=0.346).contains(&two), "{two:e}");
        assert!(two < one / 2.0, "{two:e} against {one:e}");
    }

    /// The Imp's hoards hand a tier and a level out together, so a source
    /// filter pinned to one of them has to score the pairs they never build at
    /// nothing, however plentiful each half looks on its own.
    #[test]
    fn a_locked_source_supplies_no_tier_and_level_it_never_pairs() {
        let from = |source, requirement| {
            estimate_match_probability(&query(
                vec![Requirement {
                    source: Some(source),
                    ..requirement
                }],
                24,
            ))
        };
        // The vault's armor is +0 at tier 2 and climbs one level a tier, so it
        // stocks tier-5 armor at +3 and nothing below tier 5 at +3.
        let armor = |tier| Requirement {
            tier,
            upgrade: UpgradeRequirement::Exact(3),
            ..requirement(ItemKind::Armor)
        };
        let stocked = from(ItemSource::VaultTreasure, armor(TierRequirement::Exact(5)));
        let never = from(ItemSource::VaultTreasure, armor(TierRequirement::AtMost(3)));
        assert!(stocked > 0.05, "{stocked:e}");
        assert!(never <= 1e-9, "{never:e} for armor the vault never shelves");
        // Whichever of the Imp's two weapons is tier 5 is the one levelled
        // +2..=+4, so its tier-4 weapon is never the +2 one.
        let weapon = |upgrade| Requirement {
            tier: TierRequirement::Exact(4),
            upgrade,
            ..requirement(ItemKind::Weapon)
        };
        let rolled = from(ItemSource::ImpReward, weapon(UpgradeRequirement::Exact(3)));
        let skipped = from(ItemSource::ImpReward, weapon(UpgradeRequirement::Exact(2)));
        assert!(rolled > 0.05, "{rolled:e}");
        assert!(
            skipped <= 1e-9,
            "{skipped:e} for a level the Imp keeps for tier 5"
        );
    }

    #[test]
    fn weapon_category_narrows_the_estimate() {
        use crate::catalog::WeaponCategory;

        let exact_two = Requirement {
            upgrade: UpgradeRequirement::Exact(2),
            ..requirement(ItemKind::Weapon)
        };
        let melee = Requirement {
            weapon_category: Some(WeaponCategory::Melee),
            ..exact_two
        };
        let thrown = Requirement {
            weapon_category: Some(WeaponCategory::Thrown),
            ..exact_two
        };
        let any = estimate_match_probability(&query(vec![exact_two], 6));
        let melee = estimate_match_probability(&query(vec![melee], 6));
        let thrown = estimate_match_probability(&query(vec![thrown], 6));

        assert!(melee > 0.0, "{melee}");
        assert!(thrown > 0.0, "{thrown}");
        assert!(melee < any, "{melee} vs {any}");
        assert!(thrown < any, "{thrown} vs {any}");
        // One weapon of either class is at least as likely as one of a fixed
        // class, and no likelier than having one of each class to pick from.
        assert!(any <= melee + thrown + 1e-9, "{any} vs {melee} + {thrown}");
    }

    fn staff(max_depth: u8) -> SearchQuery {
        let mut requirements = vec![Requirement {
            upgrade: UpgradeRequirement::Exact(3),
            identity_group: Some(1),
            ..requirement(ItemKind::Wand)
        }];
        requirements.extend([1, 2].map(|_| Requirement {
            identity_group: Some(1),
            ..requirement(ItemKind::Wand)
        }));
        requirements.push(Requirement {
            upgrade: UpgradeRequirement::AtLeast(1),
            ..requirement(ItemKind::Wand)
        });
        query(requirements, max_depth)
    }

    #[test]
    fn searching_deeper_finds_more_seeds() {
        let shallow = estimate_match_probability(&staff(7));
        let middle = estimate_match_probability(&staff(9));
        let deep = estimate_match_probability(&staff(24));
        assert!(
            shallow < middle && middle < deep,
            "{shallow:e} {middle:e} {deep:e}"
        );
    }

    #[test]
    fn a_per_item_floor_limit_binds_independently_of_the_search_depth() {
        let anywhere = query(
            vec![Requirement {
                upgrade: UpgradeRequirement::Exact(2),
                ..requirement(ItemKind::Wand)
            }],
            24,
        );
        let early = query(
            vec![Requirement {
                upgrade: UpgradeRequirement::Exact(2),
                max_depth: Some(4),
                ..requirement(ItemKind::Wand)
            }],
            24,
        );
        let shallow_search = query(anywhere.requirements.clone(), 4);
        let limited = estimate_match_probability(&early);
        assert!(limited < estimate_match_probability(&anywhere));
        // Capping one item at floor four is the same as searching four floors.
        let difference = (limited - estimate_match_probability(&shallow_search)).abs();
        assert!(difference < 1e-12, "{difference:e}");
    }

    #[test]
    fn a_guaranteed_reward_is_certain() {
        let ghost = query(
            vec![Requirement {
                source: Some(ItemSource::GhostReward),
                ..requirement(ItemKind::Armor)
            }],
            24,
        );
        assert!(estimate_match_probability(&ghost) > 0.99);
    }

    #[test]
    fn each_extra_copy_costs_something() {
        let one = query(
            vec![Requirement {
                upgrade: UpgradeRequirement::Exact(2),
                ..requirement(ItemKind::Wand)
            }],
            24,
        );
        let two = query(
            vec![
                Requirement {
                    upgrade: UpgradeRequirement::Exact(2),
                    ..requirement(ItemKind::Wand)
                };
                2
            ],
            24,
        );
        assert!(estimate_match_probability(&two) < estimate_match_probability(&one));
    }

    #[test]
    fn large_queries_still_share_one_quest_prize_across_families() {
        let mut wanted = query(vec![requirement(ItemKind::Wand); 5], 19);
        wanted.requirements.push(Requirement {
            item: Some(ItemId::RingEnergy),
            upgrade: UpgradeRequirement::Exact(4),
            ..requirement(ItemKind::Ring)
        });
        let before = estimate_match_probability(&wanted);
        assert!(before.is_finite() && before > 0.0);
        wanted.requirements.push(Requirement {
            item: Some(ItemId::ChaliceOfBlood),
            upgrade: UpgradeRequirement::Exact(5),
            ..requirement(ItemKind::Artifact)
        });
        // Both upgraded items spend the Imp's single reward choice.
        assert!(estimate_match_probability(&wanted) <= 0.0);
    }

    #[test]
    fn large_families_keep_finite_estimates_with_automatic_and_explicit_trinkets() {
        let mut wanted = query(vec![requirement(ItemKind::Armor); 5], 19);
        let supported = estimate_match_probability(&wanted);
        assert!(supported.is_finite() && supported > 0.0);
        wanted.requirements.push(requirement(ItemKind::Armor));
        let larger = estimate_match_probability(&wanted);
        assert!(larger > 0.0 && larger < supported);
        for auto_apply in [false, true] {
            wanted.auto_apply_trinket = auto_apply;
            let probability = estimate_match_probability(&wanted);
            assert!(probability.is_finite() && probability > 0.0 && probability <= 1.0);
        }
        wanted.requirements.push(trinket(ItemId::RatSkull));
        for selected in [false, true] {
            wanted.requirements.last_mut().unwrap().select_trinket = selected;
            let probability = estimate_match_probability(&wanted);
            assert!(probability.is_finite() && probability > 0.0 && probability < 1.0);
        }
        // Both branches of a mixed-family alternative retain the large
        // residual equipment query.
        wanted.requirements.last_mut().unwrap().alternative_group = Some(1);
        wanted.requirements.push(Requirement {
            alternative_group: Some(1),
            ..requirement(ItemKind::Ring)
        });
        let probability = estimate_match_probability(&wanted);
        assert!(probability.is_finite() && probability > 0.0 && probability < 1.0);
    }

    #[test]
    fn extra_requirements_keep_costing_probability_beyond_five() {
        let mut previous =
            estimate_match_probability(&query(vec![requirement(ItemKind::Armor); 5], 19));
        for count in 6..=20 {
            let wanted = query(vec![requirement(ItemKind::Armor); count], 19);
            let probability = estimate_match_probability(&wanted);
            assert!(
                probability.is_finite() && probability >= 0.0 && probability <= previous,
                "{count} armors: {probability} vs {previous}"
            );
            previous = probability;
        }
    }

    #[test]
    fn large_queries_do_not_depend_on_machine_word_width() {
        for count in [32, 64, 128] {
            let wanted = query(vec![requirement(ItemKind::Armor); count], 24);
            let probability = estimate_match_probability(&wanted);
            assert!(probability.is_finite() && (0.0..=1.0).contains(&probability));
        }
    }

    #[test]
    fn large_families_keep_distinct_and_overlapping_filters() {
        let mut wanted = query(Vec::new(), 24);
        let mut previous = 1.0;
        for identity in [
            ItemId::WandFireblast,
            ItemId::WandFrost,
            ItemId::WandLightning,
            ItemId::WandDisintegration,
            ItemId::WandPrismaticLight,
            ItemId::WandCorrosion,
        ] {
            wanted.requirements.push(Requirement {
                item: Some(identity),
                ..requirement(ItemKind::Wand)
            });
            let probability = estimate_match_probability(&wanted);
            assert!(probability.is_finite() && probability > 0.0 && probability < previous);
            previous = probability;
        }
        wanted.requirements.push(requirement(ItemKind::Wand));
        let probability = estimate_match_probability(&wanted);
        assert!(probability > 0.0 && probability < previous);
    }

    #[test]
    fn artifact_estimates_support_the_entire_deck() {
        let mut wanted = query(
            super::identities(ItemKind::Artifact)
                .into_iter()
                .map(|(identity, _)| Requirement {
                    item: Some(identity),
                    ..requirement(ItemKind::Artifact)
                })
                .collect(),
            24,
        );
        let probability = estimate_match_probability(&wanted);
        assert!(probability.is_finite() && (0.0..=1.0).contains(&probability));
        wanted.requirements.push(requirement(ItemKind::Wand));
        let mixed = estimate_match_probability(&wanted);
        assert!(mixed.is_finite() && mixed >= 0.0 && mixed <= probability);
        wanted.requirements.push(wanted.requirements[0]);
        assert!(estimate_match_probability(&wanted) <= 0.0);
    }

    #[test]
    fn unreachable_requirements_are_impossible() {
        // The Wandmaker never hands out armor, and no source stocks a wand
        // beyond the depth its quest occupies.
        let armor_from_wandmaker = query(
            vec![Requirement {
                source: Some(ItemSource::WandmakerReward),
                ..requirement(ItemKind::Armor)
            }],
            24,
        );
        assert!(estimate_match_probability(&armor_from_wandmaker) <= 0.0);

        let wand_before_the_wandmaker = query(
            vec![Requirement {
                source: Some(ItemSource::WandmakerReward),
                max_depth: Some(3),
                ..requirement(ItemKind::Wand)
            }],
            24,
        );
        assert!(estimate_match_probability(&wand_before_the_wandmaker) <= 0.0);
    }

    #[test]
    fn tipped_darts_are_obtainable() {
        // Darts are weapons to the catalog but come from plant seeds sold in
        // shops, so they are not in the weapon deck at all.
        let dart = query(
            vec![Requirement {
                item: Some(ItemId::BlindingDart),
                ..requirement(ItemKind::Weapon)
            }],
            24,
        );
        assert!(estimate_match_probability(&dart) > 0.1);
        // The one dart the generator never tips is still impossible.
        let never = query(
            vec![Requirement {
                item: Some(ItemId::RotDart),
                ..requirement(ItemKind::Weapon)
            }],
            24,
        );
        assert!(estimate_match_probability(&never) <= 0.0);
    }

    #[test]
    fn a_per_item_floor_limit_makes_its_own_supply_compete() {
        // Two wands wanted by floor four draw on those four floors alone, so the
        // second costs far more than it would with the run to draw on.
        let shallow = |copies: usize| {
            query(
                vec![
                    Requirement {
                        upgrade: UpgradeRequirement::AtLeast(1),
                        max_depth: Some(4),
                        ..requirement(ItemKind::Wand)
                    };
                    copies
                ],
                24,
            )
        };
        let one = estimate_match_probability(&shallow(1));
        let two = estimate_match_probability(&shallow(2));
        assert!(
            two < one * one,
            "{two:e} is not scarcer than {:e}",
            one * one
        );
    }

    #[test]
    fn a_linked_group_competes_with_the_rest_of_its_family() {
        let mut linked: Vec<Requirement> = (0..2)
            .map(|_| Requirement {
                identity_group: Some(1),
                ..requirement(ItemKind::Wand)
            })
            .collect();
        let alone = estimate_match_probability(&query(linked.clone(), 6));
        linked.push(Requirement {
            upgrade: UpgradeRequirement::AtLeast(2),
            ..requirement(ItemKind::Wand)
        });
        let alongside = estimate_match_probability(&query(linked, 6));
        assert!(alongside < alone);
    }

    #[test]
    fn thrown_weapons_are_not_confused_with_melee_ones() {
        let thrown = query(
            vec![Requirement {
                item: Some(ItemId::ThrowingClub),
                ..requirement(ItemKind::Weapon)
            }],
            24,
        );
        let melee = query(
            vec![Requirement {
                item: Some(ItemId::Sword),
                ..requirement(ItemKind::Weapon)
            }],
            24,
        );
        assert!(estimate_match_probability(&thrown) > 0.0);
        assert!(estimate_match_probability(&melee) > 0.0);
    }

    #[test]
    fn rarer_modifiers_are_rarer() {
        let common = query(
            vec![Requirement {
                effect: EffectRequirement::exactly(Effect::Armor(ArmorEffect::Viscosity)),
                ..requirement(ItemKind::Armor)
            }],
            24,
        );
        let rare = query(
            vec![Requirement {
                effect: EffectRequirement::exactly(Effect::Armor(ArmorEffect::Thorns)),
                ..requirement(ItemKind::Armor)
            }],
            24,
        );
        assert!(estimate_match_probability(&rare) < estimate_match_probability(&common));
    }

    #[test]
    fn broader_effect_sets_are_likelier_than_their_members() {
        // Shallow on purpose: by depth 24 the broadest of these queries rounds
        // to exactly 1.0 in `f64` and the strict orderings collapse.
        let depth = 6;
        let single = |effect| {
            query(
                vec![Requirement {
                    effect: EffectRequirement::exactly(Effect::Armor(effect)),
                    ..requirement(ItemKind::Armor)
                }],
                depth,
            )
        };
        let both = query(
            vec![Requirement {
                effect: EffectRequirement::OneOf(
                    EffectSet::from_effects([
                        Effect::Armor(ArmorEffect::Viscosity),
                        Effect::Armor(ArmorEffect::Thorns),
                    ])
                    .unwrap(),
                ),
                ..requirement(ItemKind::Armor)
            }],
            depth,
        );
        let any_glyph = query(
            vec![Requirement {
                effect: EffectRequirement::OneOf(EffectSet::enchantments(ItemKind::Armor).unwrap()),
                ..requirement(ItemKind::Armor)
            }],
            depth,
        );
        let viscosity = estimate_match_probability(&single(ArmorEffect::Viscosity));
        let thorns = estimate_match_probability(&single(ArmorEffect::Thorns));
        let pair = estimate_match_probability(&both);
        let any = estimate_match_probability(&any_glyph);
        assert!(pair > viscosity && pair > thorns, "{pair:e}");
        assert!(any > pair, "{any:e} vs {pair:e}");
        assert!(
            any < estimate_match_probability(&query(vec![requirement(ItemKind::Armor)], depth))
        );
    }

    #[test]
    fn every_enchantment_is_reachable_and_each_family_sums_to_one() {
        for effects in [
            EffectSet::enchantments(ItemKind::Weapon).unwrap(),
            EffectSet::enchantments(ItemKind::Armor).unwrap(),
        ] {
            let total: f64 = effects
                .effects()
                .map(|effect| {
                    let chance = rarity_probability(effect);
                    assert!(chance > 0.0, "{effect:?} can never be drawn");
                    chance
                })
                .sum();
            assert!((total - 1.0).abs() < 1e-12, "{total:e}");
        }
    }

    #[test]
    fn alternatives_score_at_least_their_best_member() {
        let spear_only = query(
            vec![Requirement {
                item: Some(ItemId::Spear),
                upgrade: UpgradeRequirement::Exact(3),
                ..requirement(ItemKind::Weapon)
            }],
            24,
        );
        let either = query(
            vec![
                Requirement {
                    item: Some(ItemId::Spear),
                    upgrade: UpgradeRequirement::Exact(3),
                    alternative_group: Some(1),
                    ..requirement(ItemKind::Weapon)
                },
                Requirement {
                    item: Some(ItemId::Sword),
                    upgrade: UpgradeRequirement::Exact(1),
                    alternative_group: Some(1),
                    ..requirement(ItemKind::Weapon)
                },
            ],
            24,
        );
        let alone = estimate_match_probability(&spear_only);
        let grouped = estimate_match_probability(&either);
        assert!(grouped >= alone, "{grouped:e} vs {alone:e}");
    }

    #[test]
    fn combined_upgrade_totals_cost_something() {
        let pair = |level_sum| {
            query(
                vec![
                    Requirement {
                        item: Some(ItemId::RingMight),
                        identity_group: Some(1),
                        level_sum,
                        ..requirement(ItemKind::Ring)
                    };
                    2
                ],
                24,
            )
        };
        let plain = estimate_match_probability(&pair(None));
        let modest = estimate_match_probability(&pair(Some(LevelSum {
            group: 1,
            minimum_total: 4,
        })));
        let steep = estimate_match_probability(&pair(Some(LevelSum {
            group: 1,
            minimum_total: 7,
        })));
        assert!(modest <= plain, "{modest:e} vs {plain:e}");
        assert!(steep < plain, "{steep:e} vs {plain:e}");
    }

    #[test]
    fn requiring_a_blacksmith_needs_the_floors_it_lives_on() {
        let mut early = query(vec![requirement(ItemKind::Armor)], 8);
        early.require_blacksmith = true;
        assert!(estimate_match_probability(&early) <= 0.0);

        let mut late = query(vec![requirement(ItemKind::Armor)], 14);
        late.require_blacksmith = true;
        assert!(estimate_match_probability(&late) > 0.9);
    }

    #[test]
    fn a_wandmaker_quest_costs_exactly_its_spawn_and_variant_odds() {
        use crate::quests::WandmakerQuestType;

        let quested = |max_depth| SearchQuery {
            wandmaker_quest: Some(WandmakerQuestType::Rotberry),
            ..query(vec![requirement(ItemKind::Armor)], max_depth)
        };
        let open = |max_depth| query(vec![requirement(ItemKind::Armor)], max_depth);
        let ratio = |max_depth| {
            estimate_match_probability(&quested(max_depth))
                / estimate_match_probability(&open(max_depth))
        };

        // Below the Prison the Wandmaker never spawns at all.
        assert!(estimate_match_probability(&quested(6)) <= 0.0);
        // Floor seven spawns one run in three, floor eight two in three, and
        // floor nine always; one of three quests then has to be the wanted one.
        assert!((ratio(7) - 1.0 / 9.0).abs() < 1e-9);
        assert!((ratio(8) - 2.0 / 9.0).abs() < 1e-9);
        assert!((ratio(24) - 1.0 / 3.0).abs() < 1e-9);
    }
}
