//! Brute-force oracle for the slot-based matcher: alternative groups,
//! combined-level totals with optional members, same-kind stacks and
//! accessibility
//! scenarios are all exercised on random small worlds and random queries,
//! and the engine's answers are compared with an exhaustive enumeration of
//! every assignment.

use std::collections::BTreeMap;

use shpd_seedfinder_core::catalog::{
    ALL_ARMOR_EFFECTS, ALL_WEAPON_EFFECTS, Effect, ItemId, ItemKind, item,
};
use shpd_seedfinder_core::challenges::Challenges;
use shpd_seedfinder_core::model::{Accessibility, GeneratedWorld, ItemSource, WorldItem};
use shpd_seedfinder_core::query::{
    EffectRequirement, EffectSet, LevelSum, Requirement, SearchQuery, TierRequirement,
    UpgradeRequirement, scout_matches,
};
use shpd_seedfinder_core::quests::QuestSummary;
use shpd_seedfinder_core::run::RingGems;
use shpd_seedfinder_core::seed::DungeonSeed;

const POOL: [ItemId; 8] = [
    ItemId::Sword,
    ItemId::Mace,
    ItemId::Spear,
    ItemId::Shuriken,
    ItemId::MailArmor,
    ItemId::WandFrost,
    ItemId::RingMight,
    ItemId::RingHaste,
];

struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }

    fn below(&mut self, bound: usize) -> usize {
        usize::try_from(self.next() % bound as u64).unwrap()
    }

    fn chance(&mut self, percent: usize) -> bool {
        self.below(100) < percent
    }
}

fn family_effects(kind: ItemKind) -> Vec<Effect> {
    match kind {
        ItemKind::Weapon => ALL_WEAPON_EFFECTS
            .iter()
            .copied()
            .map(Effect::Weapon)
            .collect(),
        ItemKind::Armor => ALL_ARMOR_EFFECTS
            .iter()
            .copied()
            .map(Effect::Armor)
            .collect(),
        ItemKind::Wand | ItemKind::Ring | ItemKind::Trinket | ItemKind::Artifact => Vec::new(),
    }
}

fn random_world(rng: &mut Rng) -> GeneratedWorld {
    let count = 1 + rng.below(6);
    let items = (0..count)
        .map(|_| {
            let id = POOL[rng.below(POOL.len())];
            let kind = item(id).kind;
            let effects = family_effects(kind);
            let effect = (!effects.is_empty() && rng.chance(50))
                .then(|| effects[rng.below(effects.len().min(6))]);
            let cursed = effect.is_some_and(Effect::is_curse) || rng.chance(15);
            let accessibility = match rng.below(4) {
                0 => Accessibility::Choice {
                    group: 1,
                    option: u8::try_from(rng.below(2)).unwrap(),
                },
                1 => Accessibility::Scenarios {
                    group: 2,
                    mask: 1 + u64::try_from(rng.below(7)).unwrap(),
                },
                _ => Accessibility::Independent,
            };
            WorldItem {
                item: id,
                upgrade: u8::try_from(rng.below(usize::from(kind.maximum_search_upgrade()) + 1))
                    .unwrap(),
                effect,
                cursed,
                depth: 1 + u8::try_from(rng.below(10)).unwrap(),
                source: if rng.chance(10) {
                    ItemSource::BlacksmithReward
                } else {
                    ItemSource::Heap
                },
                accessibility,
                secret: false,
            }
        })
        .collect();
    GeneratedWorld {
        feelings: Vec::new(),
        seed: DungeonSeed::MIN,
        items,
        quests: QuestSummary::default(),
        ring_gems: RingGems::UNSHUFFLED,
    }
}

fn random_requirement(rng: &mut Rng) -> Requirement {
    let id = POOL[rng.below(POOL.len())];
    let kind = item(id).kind;
    let cap = usize::from(kind.maximum_search_upgrade());
    let upgrade = match rng.below(3) {
        0 => UpgradeRequirement::Any,
        1 => UpgradeRequirement::Exact(1 + u8::try_from(rng.below(cap)).unwrap()),
        _ => UpgradeRequirement::AtLeast(u8::try_from(rng.below(cap + 1)).unwrap()),
    };
    let effects = family_effects(kind);
    let effect = if effects.is_empty() || rng.chance(50) {
        EffectRequirement::Any
    } else {
        let wanted: Vec<Effect> = effects
            .iter()
            .take(6)
            .copied()
            .filter(|_| rng.chance(40))
            .collect();
        match EffectSet::from_effects(wanted) {
            Some(set) => EffectRequirement::OneOf(set),
            None => EffectRequirement::OneOf(EffectSet::enchantments(kind).unwrap()),
        }
    };
    Requirement {
        kind,
        weapon_category: None,
        item: rng.chance(50).then_some(id),
        tier: TierRequirement::Any,
        upgrade,
        effect,
        require_uncursed: rng.chance(20),
        select_trinket: false,
        blanket: false,
        source: None,
        identity_group: None,
        max_depth: rng
            .chance(30)
            .then(|| 1 + u8::try_from(rng.below(10)).unwrap()),
        alternative_group: None,
        level_sum: None,
    }
}

#[allow(clippy::too_many_lines)] // Keep seeded generation of the interacting query features together.
fn random_query(rng: &mut Rng) -> Option<SearchQuery> {
    let slots = 1 + rng.below(3);
    let mut requirements = Vec::new();
    for slot in 0..slots {
        let members = if rng.chance(40) { 1 + rng.below(3) } else { 1 };
        for _ in 0..members {
            let mut requirement = random_requirement(rng);
            if members > 1 {
                requirement.alternative_group = Some(u8::try_from(slot).unwrap() + 1);
            }
            requirements.push(requirement);
        }
    }
    if rng.chance(40) {
        // Put every single-member ring slot into one level-sum group;
        // members are optional in the matcher, so any subset may carry the
        // total. Only rings may combine levels, and validation caps the
        // total at one +4 ring plus +2 for every further member.
        let singles: Vec<usize> = requirements
            .iter()
            .enumerate()
            .filter(|(_, requirement)| {
                requirement.alternative_group.is_none() && requirement.kind == ItemKind::Ring
            })
            .map(|(index, _)| index)
            .collect();
        if singles.len() >= 2 {
            let capacity: u16 = singles
                .iter()
                .map(|&index| u16::from(requirements[index].maximum_level()))
                .sum();
            let attainable = capacity.min(5 + 3 * (u16::try_from(singles.len()).unwrap() - 1));
            let minimum_total = 1 + u8::try_from(rng.below(usize::from(attainable))).unwrap();
            for index in singles {
                requirements[index].level_sum = Some(LevelSum {
                    group: 1,
                    minimum_total,
                });
            }
        }
    }
    if rng.chance(40) {
        // Turn one slot into a stack anchor: its members share an identity
        // label with one or two appended bare copies of the same kind. Only
        // a slot whose members agree on a kind can anchor a stack.
        let slots: Vec<Vec<usize>> = {
            let query = SearchQuery {
                auto_apply_trinket: false,
                arcane_resin_filter: shpd_seedfinder_core::query::ArcaneResinFilter::default(),
                arcane_resin_auto: false,
                arcane_resin: 0,
                requirements: requirements.clone(),
                max_depth: 10,
                challenges: Challenges::NONE,
                require_blacksmith: false,
                exclude_blacksmith_rewards: false,
                wandmaker_quest: None,
            };
            query.slots()
        };
        let slot = &slots[rng.below(slots.len())];
        let kind = requirements[slot[0]].kind;
        if slot.iter().all(|&member| requirements[member].kind == kind) {
            for &member in slot {
                requirements[member].identity_group = Some(1);
            }
            for _ in 0..=rng.below(2) {
                requirements.push(Requirement {
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
                    identity_group: Some(1),
                    max_depth: None,
                    alternative_group: None,
                    level_sum: None,
                });
            }
        }
    }
    if rng.chance(30) {
        for _ in 0..=rng.below(2) {
            let mut blanket = random_requirement(rng);
            blanket.blanket = true;
            blanket.identity_group = None;
            blanket.level_sum = None;
            blanket.alternative_group = rng.chance(30).then_some(100);
            requirements.push(blanket);
        }
    }
    let query = SearchQuery {
        auto_apply_trinket: false,
        arcane_resin_filter: shpd_seedfinder_core::query::ArcaneResinFilter::default(),
        arcane_resin_auto: rng.chance(25),
        arcane_resin: if rng.chance(35) {
            1 + u16::try_from(rng.below(10)).unwrap()
        } else {
            0
        },
        requirements,
        max_depth: 10,
        challenges: Challenges::NONE,
        require_blacksmith: false,
        exclude_blacksmith_rewards: rng.chance(20),
        wandmaker_quest: None,
    };
    query.validate().ok().map(|()| query)
}

/// One slot's candidate members as `(requirement index, item index)` pairs
/// under the query's floor limits and blacksmith exclusion.
fn candidates(query: &SearchQuery, world: &GeneratedWorld) -> Vec<Vec<(usize, usize)>> {
    query
        .ordinary_slots()
        .into_iter()
        .map(|slot| {
            let mut pairs = Vec::new();
            for member in slot {
                let requirement = &query.requirements[member];
                for (index, candidate) in world.items.iter().enumerate() {
                    if candidate.depth <= query.max_depth
                        && candidate.depth <= requirement.max_depth.unwrap_or(query.max_depth)
                        && (!query.exclude_blacksmith_rewards
                            || candidate.source != ItemSource::BlacksmithReward)
                        && requirement.matches(candidate)
                    {
                        pairs.push((member, index));
                    }
                }
            }
            pairs
        })
        .collect()
}

fn required_resin(query: &SearchQuery, world: &GeneratedWorld, used: &[bool]) -> u16 {
    if query.arcane_resin_auto {
        world
            .items
            .iter()
            .zip(used)
            .filter(|(candidate, used)| **used && item(candidate.item).kind == ItemKind::Wand)
            .map(|(candidate, _)| ((u16::from(candidate.upgrade) + 1)..=3).sum::<u16>())
            .sum()
    } else {
        query.arcane_resin
    }
}

/// Whether a full or partial assignment respects every cross-item rule, and
/// how many slots it counts as satisfying (sum groups all-or-nothing).
fn score(
    query: &SearchQuery,
    world: &GeneratedWorld,
    chosen: &[Option<(usize, usize)>],
) -> Option<usize> {
    let mut used = vec![false; world.items.len()];
    let mut identities: BTreeMap<u8, ItemId> = BTreeMap::new();
    let mut scenarios: BTreeMap<u16, u64> = BTreeMap::new();
    let mut sums: BTreeMap<u8, (usize, u16)> = BTreeMap::new();
    for (member, index) in chosen.iter().flatten() {
        if std::mem::replace(&mut used[*index], true) {
            return None;
        }
        let requirement = &query.requirements[*member];
        let candidate = &world.items[*index];
        if let Some(group) = requirement.identity_group {
            if *identities.entry(group).or_insert(candidate.item) != candidate.item {
                return None;
            }
        }
        if let Some((group, mask)) = candidate.accessibility.scenario_constraint() {
            let compatible = scenarios.entry(group).or_insert(u64::MAX);
            *compatible &= mask;
            if *compatible == 0 {
                return None;
            }
        }
        if let Some(sum) = requirement.level_sum {
            let entry = sums.entry(sum.group).or_insert((0, 0));
            entry.0 += 1;
            entry.1 += u16::from(candidate.upgrade) + 1;
        }
    }
    let mut group_totals: BTreeMap<u8, u16> = BTreeMap::new();
    for requirement in &query.requirements {
        if let Some(sum) = requirement.level_sum {
            group_totals
                .entry(sum.group)
                .or_insert(u16::from(sum.minimum_total));
        }
    }
    let satisfied = |group: u8| {
        let (_, total) = sums.get(&group).copied().unwrap_or((0, 0));
        total >= group_totals[&group]
    };
    // One condition per assigned plain slot, plus one per level-sum group
    // whose assigned members reach the total.
    let plain = chosen
        .iter()
        .flatten()
        .filter(|(member, _)| query.requirements[*member].level_sum.is_none())
        .count();
    let groups = group_totals
        .keys()
        .filter(|group| satisfied(**group))
        .count();
    let blankets = query
        .blanket_slots()
        .iter()
        .filter(|slot| {
            slot.iter().any(|&member| {
                let requirement = query.requirements[member];
                chosen.iter().flatten().any(|&(ordinary, index)| {
                    query.requirements[ordinary]
                        .level_sum
                        .is_none_or(|sum| satisfied(sum.group))
                        && world.items[index].depth
                            <= requirement.max_depth.unwrap_or(query.max_depth)
                        && requirement.matches(&world.items[index])
                })
            })
        })
        .count();
    let minimum = required_resin(query, world, &used);
    let resin = query.needs_resin()
        && (0..(1_u64 << world.items.len())).any(|subset| {
            // Exhaustively try every surplus subset, independently of the
            // production matcher's per-scenario maximum calculation.
            let mut compatible = scenarios.clone();
            let mut total = 0;
            for (index, candidate) in world.items.iter().enumerate() {
                if subset & (1 << index) == 0 {
                    continue;
                }
                if used[index]
                    || item(candidate.item).kind != ItemKind::Wand
                    || candidate.cursed
                    || candidate.depth > query.max_depth
                    || (query.exclude_blacksmith_rewards
                        && candidate.source == ItemSource::BlacksmithReward)
                {
                    return false;
                }
                if let Some((group, mask)) = candidate.accessibility.scenario_constraint() {
                    let remaining = compatible.entry(group).or_insert(u64::MAX);
                    *remaining &= mask;
                    if *remaining == 0 {
                        return false;
                    }
                }
                total += 2 * (u16::from(candidate.upgrade) + 1);
            }
            total >= minimum
        });
    Some(plain + groups + blankets + usize::from(resin))
}

fn best_partial(
    query: &SearchQuery,
    world: &GeneratedWorld,
    candidates: &[Vec<(usize, usize)>],
    slots: &[Vec<usize>],
    slot: usize,
    chosen: &mut Vec<Option<(usize, usize)>>,
    full_only: bool,
) -> Option<usize> {
    if slot == candidates.len() {
        return score(query, world, chosen);
    }
    let mut best = None;
    for pair in &candidates[slot] {
        chosen.push(Some(*pair));
        if let Some(value) =
            best_partial(query, world, candidates, slots, slot + 1, chosen, full_only)
        {
            best = Some(best.map_or(value, |current: usize| current.max(value)));
        }
        chosen.pop();
    }
    // Level-sum members are optional even in a full assignment: the rest of
    // their group may carry the total.
    let optional = query.requirements[slots[slot][0]].level_sum.is_some();
    if !full_only || optional {
        chosen.push(None);
        if let Some(value) =
            best_partial(query, world, candidates, slots, slot + 1, chosen, full_only)
        {
            best = Some(best.map_or(value, |current: usize| current.max(value)));
        }
        chosen.pop();
    }
    best
}

#[test]
fn matcher_and_scout_agree_with_exhaustive_enumeration() {
    let mut rng = Rng(0x9E37_79B9_7F4A_7C15);
    let mut checked = 0;
    let mut matched = 0;
    while checked < 3_000 {
        let Some(query) = random_query(&mut rng) else {
            continue;
        };
        let world = random_world(&mut rng);
        let slots = query.ordinary_slots();
        let candidates = candidates(&query, &world);
        // A match fills every plain slot and satisfies every level-sum
        // group, which is exactly a full-mode score of every condition.
        let conditions = query.scout_condition_count();
        let full = best_partial(
            &query,
            &world,
            &candidates,
            &slots,
            0,
            &mut Vec::new(),
            true,
        );
        let expected = full == Some(conditions);
        assert_eq!(
            query.matches(&world),
            expected,
            "matcher disagrees with brute force for {query:?} on {world:?}"
        );
        let best = best_partial(
            &query,
            &world,
            &candidates,
            &slots,
            0,
            &mut Vec::new(),
            false,
        )
        .expect("skipping every slot is always a valid selection");
        let marks = scout_matches(&world, &query);
        assert_eq!(marks.total_requirements, conditions);
        assert_eq!(
            marks.matched_requirements, best,
            "scout disagrees with brute force for {query:?} on {world:?}"
        );
        // A satisfied level-sum group flags every contributing item, so the
        // flags can outnumber the conditions but never undercut them.
        // Auto can satisfy its condition without donors when its cost is zero.
        assert!(
            marks.matched_indices().len()
                + query.blanket_slots().len()
                + usize::from(query.arcane_resin_auto)
                >= marks.matched_requirements
        );
        if expected {
            assert_eq!(marks.matched_requirements, conditions);
        }
        checked += 1;
        matched += usize::from(expected);
    }
    // Resin and blankets add constraints; still require a healthy mix of matches and misses.
    assert!(
        (150..2_800).contains(&matched),
        "{matched} of {checked} matched"
    );
}
