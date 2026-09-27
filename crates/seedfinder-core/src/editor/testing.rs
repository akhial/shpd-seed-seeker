//! Shared fixtures for the editor's tests: row builders, a small
//! deterministic generator of valid rows and edits, and the output
//! invariant every edit must keep.

use crate::catalog::{
    ALL_ARMOR_EFFECTS, ALL_WEAPON_EFFECTS, Effect, ITEMS, ItemId, ItemKind, WeaponCategory, item,
};
use crate::model::ItemSource;
use crate::query::{
    ArcaneResinFilter, EffectRequirement, EffectSet, LevelSum, MAX_IDENTITY_GROUP,
    MAX_LEVEL_SUM_GROUP, QueryError, Requirement, SearchQuery, TierRequirement, UpgradeRequirement,
};

use super::{Edit, MAX_KEY, Row};

/// A wildcard row of `kind`.
pub(crate) fn row(key: u64, kind: ItemKind) -> Row {
    Row {
        key,
        requirement: Requirement::any(kind),
    }
}

/// A row naming `item`, open to any upgrade.
pub(crate) fn named(key: u64, item_id: ItemId) -> Row {
    Row {
        key,
        requirement: Requirement {
            item: Some(item_id),
            ..Requirement::any(item(item_id).kind)
        },
    }
}

/// `row` with `patch` applied to its requirement.
pub(crate) fn with(mut row: Row, patch: impl FnOnce(&mut Requirement)) -> Row {
    patch(&mut row.requirement);
    row
}

/// The requirements alone, in order.
pub(crate) fn requirements(rows: &[Row]) -> Vec<Requirement> {
    rows.iter().map(|row| row.requirement).collect()
}

/// The whole list as the engine checks it before a search.
pub(crate) fn validate(rows: &[Row]) -> Result<(), QueryError> {
    query(requirements(rows)).validate()
}

pub(crate) fn query(requirements: Vec<Requirement>) -> SearchQuery {
    SearchQuery {
        auto_apply_trinket: false,
        arcane_resin: 0,
        arcane_resin_auto: false,
        arcane_resin_filter: ArcaneResinFilter::default(),
        requirements,
        floor_requirements: Vec::new(),
        max_depth: 24,
        challenges: crate::challenges::Challenges::NONE,
        require_blacksmith: false,
        exclude_blacksmith_rewards: false,
        wandmaker_quest: None,
    }
}

/// The §3 output invariant: whatever an edit emits from valid rows is a list
/// every platform can hold — unique keys in range, every row passing
/// [`Requirement::validate`], stack and level labels in 1..=4 and only on
/// the families that stack, and every row re-decoding through the document
/// codec unchanged.
pub(crate) fn assert_emittable(rows: &[Row], context: &str) {
    let mut keys: Vec<u64> = rows.iter().map(|row| row.key).collect();
    keys.sort_unstable();
    keys.dedup();
    assert_eq!(keys.len(), rows.len(), "duplicate keys: {context}");
    for row in rows {
        let requirement = row.requirement;
        assert!(
            (1..=MAX_KEY).contains(&row.key),
            "key {} out of range: {context}",
            row.key
        );
        assert_eq!(requirement.validate(), Ok(()), "{requirement:?}: {context}");
        if let Some(label) = requirement.identity_group {
            assert!(
                (1..=MAX_IDENTITY_GROUP).contains(&label),
                "identity {label}: {context}"
            );
        }
        if let Some(sum) = requirement.level_sum {
            assert!(
                (1..=MAX_LEVEL_SUM_GROUP).contains(&sum.group),
                "level sum {}: {context}",
                sum.group
            );
        }
        if requirement.blanket || matches!(requirement.kind, ItemKind::Trinket | ItemKind::Artifact)
        {
            assert_eq!(
                (requirement.identity_group, requirement.level_sum),
                (None, None),
                "a label on a row that never stacks: {context}"
            );
        }
        assert!(
            requirement.alternative_group.is_none() || requirement.level_sum.is_none(),
            "a combined level on an alternative: {context}"
        );
        #[cfg(feature = "json-query")]
        {
            let lone = Requirement {
                alternative_group: None,
                ..requirement
            };
            let document = crate::json_query::encode(&query(vec![lone])).to_string();
            let decoded = crate::json_query::decode_unvalidated(&document)
                .unwrap_or_else(|error| panic!("{document}: {error}: {context}"));
            assert_eq!(decoded.requirements, [lone], "{document}: {context}");
        }
    }
}

/// A small xorshift generator: deterministic, dependency-free, good enough
/// to spread test cases.
pub(crate) struct Rng(u64);

impl Rng {
    pub(crate) fn new(seed: u64) -> Self {
        Self(seed | 1)
    }

    pub(crate) fn next(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0 = x;
        x
    }

    /// Uniform in `0..bound` (bound ≥ 1).
    pub(crate) fn below(&mut self, bound: usize) -> usize {
        usize::try_from(self.next() % u64::try_from(bound).expect("usize fits u64"))
            .expect("below a usize bound")
    }

    /// Uniform in `low..=high`.
    pub(crate) fn range(&mut self, low: u8, high: u8) -> u8 {
        low + u8::try_from(self.below(usize::from(high - low) + 1)).expect("a u8 span")
    }

    /// True with probability `percent`/100.
    pub(crate) fn chance(&mut self, percent: usize) -> bool {
        self.below(100) < percent
    }

    pub(crate) fn pick<T: Copy>(&mut self, items: &[T]) -> T {
        items[self.below(items.len())]
    }
}

fn items_of(kind: ItemKind) -> Vec<ItemId> {
    ITEMS
        .iter()
        .filter(|definition| definition.kind == kind && definition.id != ItemId::TrinketCatalyst)
        .map(|definition| definition.id)
        .collect()
}

/// A random requirement passing [`Requirement::validate`], without group
/// labels. Wands and rings are drawn most, since they stack and count levels.
pub(crate) fn random_requirement(rng: &mut Rng) -> Requirement {
    let kind = rng.pick(&[
        ItemKind::Weapon,
        ItemKind::Weapon,
        ItemKind::Armor,
        ItemKind::Wand,
        ItemKind::Wand,
        ItemKind::Wand,
        ItemKind::Ring,
        ItemKind::Ring,
        ItemKind::Trinket,
        ItemKind::Artifact,
    ]);
    let mut requirement = Requirement::any(kind);
    let named = matches!(kind, ItemKind::Trinket | ItemKind::Artifact) || rng.chance(55);
    if named {
        requirement.item = Some(rng.pick(&items_of(kind)));
    }
    if kind == ItemKind::Weapon && rng.chance(30) {
        requirement.weapon_category = match requirement.item {
            Some(item_id) => item_id.weapon_category(),
            None => Some(rng.pick(&[WeaponCategory::Melee, WeaponCategory::Thrown])),
        };
    }
    if requirement.item.is_none()
        && matches!(kind, ItemKind::Weapon | ItemKind::Armor)
        && rng.chance(30)
    {
        requirement.tier = match rng.below(3) {
            0 => TierRequirement::Exact(rng.range(2, 5)),
            1 => TierRequirement::AtLeast(rng.range(3, 4)),
            _ => TierRequirement::AtMost(rng.range(3, 4)),
        };
    }
    let ceiling = requirement.upgrade_ceiling();
    requirement.upgrade = match rng.below(3) {
        1 if ceiling >= 1 => UpgradeRequirement::Exact(rng.range(1, ceiling)),
        2 => UpgradeRequirement::AtLeast(rng.range(0, ceiling)),
        _ => UpgradeRequirement::Any,
    };
    if matches!(kind, ItemKind::Weapon | ItemKind::Armor) && rng.chance(30) {
        let family: Vec<Effect> = if kind == ItemKind::Weapon {
            ALL_WEAPON_EFFECTS
                .iter()
                .copied()
                .map(Effect::Weapon)
                .collect()
        } else {
            ALL_ARMOR_EFFECTS
                .iter()
                .copied()
                .map(Effect::Armor)
                .collect()
        };
        let set = match rng.below(3) {
            0 => EffectSet::enchantments(kind),
            1 => Some(EffectSet::single(rng.pick(&family))),
            _ => EffectSet::from_effects((0..rng.range(2, 4)).map(|_| rng.pick(&family))),
        };
        if let Some(set) = set {
            requirement.effect = EffectRequirement::OneOf(set);
        }
    }
    requirement.require_uncursed = rng.chance(25)
        && !matches!(requirement.effect, EffectRequirement::OneOf(set) if set.is_curses_only());
    match kind {
        ItemKind::Trinket if rng.chance(30) => requirement.select_trinket = true,
        ItemKind::Trinket if rng.chance(30) => {
            requirement.trinket_transmutations = rng.range(1, 13);
        }
        ItemKind::Artifact if rng.chance(30) => {
            requirement.artifact_transmutations = rng.range(1, 10);
        }
        _ => {}
    }
    requirement.blanket = !requirement.select_trinket && rng.chance(12);
    requirement.exclude_resin = kind == ItemKind::Wand && !requirement.blanket && rng.chance(15);
    if rng.chance(15) {
        requirement.source = Some(rng.pick(ItemSource::ALL));
    }
    if rng.chance(25) {
        requirement.max_depth = Some(rng.range(1, 24));
    }
    debug_assert_eq!(requirement.validate(), Ok(()), "{requirement:?}");
    requirement
}

/// The plain repeat a stack of `anchor` holds.
fn repeat(anchor: &Requirement) -> Requirement {
    Requirement {
        weapon_category: anchor.weapon_category,
        item: anchor.item,
        ..Requirement::any(anchor.kind)
    }
}

/// A random list of up to eight valid rows, keyed 1..=n, with random
/// clusters, stack labels, combined levels, plain repeats and bare copies —
/// each row valid on its own, the list as a whole often not (the editor
/// must cope with both).
pub(crate) fn random_rows(rng: &mut Rng) -> Vec<Row> {
    let mut list: Vec<Requirement> = Vec::new();
    for _ in 0..rng.below(7) {
        let requirement = random_requirement(rng);
        list.push(requirement);
        let stackable = !requirement.blanket
            && !matches!(requirement.kind, ItemKind::Trinket | ItemKind::Artifact);
        if stackable && requirement.item.is_some() && rng.chance(25) {
            for _ in 0..rng.range(1, 3) {
                list.push(repeat(&requirement));
            }
        }
        if stackable && rng.chance(15) {
            let label = rng.range(1, MAX_IDENTITY_GROUP);
            list.last_mut().expect("just pushed").identity_group = Some(label);
            for _ in 0..rng.range(1, 2) {
                list.push(Requirement {
                    identity_group: Some(label),
                    ..Requirement::any(requirement.kind)
                });
            }
        }
    }
    for requirement in &mut list {
        let stackable = !requirement.blanket
            && !matches!(requirement.kind, ItemKind::Trinket | ItemKind::Artifact);
        if rng.chance(20) {
            requirement.alternative_group = Some(rng.range(1, 4));
        }
        if stackable && requirement.identity_group.is_none() && rng.chance(8) {
            requirement.identity_group = Some(rng.range(1, MAX_IDENTITY_GROUP));
        }
        if requirement.kind == ItemKind::Ring
            && !requirement.blanket
            && requirement.alternative_group.is_none()
            && rng.chance(25)
        {
            requirement.level_sum = Some(LevelSum {
                group: rng.range(1, MAX_LEVEL_SUM_GROUP),
                minimum_total: rng.range(1, 12),
            });
        }
    }
    list.into_iter()
        .enumerate()
        .map(|(index, requirement)| Row {
            key: u64::try_from(index + 1).expect("a short list"),
            requirement,
        })
        .collect()
}

/// A random edit naming mostly keys in `rows`.
pub(crate) fn random_edit(rng: &mut Rng, rows: &[Row]) -> Edit {
    let key = |rng: &mut Rng| {
        if rows.is_empty() || rng.chance(8) {
            rng.next() % 12
        } else {
            rows[rng.below(rows.len())].key
        }
    };
    let depth = |rng: &mut Rng| rng.chance(70).then(|| rng.range(0, 26));
    match rng.below(11) {
        0 => Edit::Normalize,
        1 | 2 => Edit::Join {
            source: key(rng),
            target: key(rng),
        },
        3 => Edit::Detach { key: key(rng) },
        4 => Edit::Remove { key: key(rng) },
        5 => Edit::RemoveItem { key: key(rng) },
        6 => Edit::SetCount {
            key: key(rng),
            count: rng.range(0, 5),
        },
        7 => Edit::SetTotal {
            key: key(rng),
            total: rng.chance(80).then(|| rng.range(0, 16)),
        },
        8 => Edit::ToggleLevels { key: key(rng) },
        9 => Edit::SetCopyDepth {
            key: key(rng),
            max_depth: depth(rng),
        },
        _ => {
            let mut requirement = random_requirement(rng);
            // Whatever labels the sheet sends are the edit's to decide.
            if rng.chance(20) {
                requirement.alternative_group = Some(rng.range(1, 4));
            }
            if rng.chance(20) && !requirement.blanket {
                requirement.identity_group = Some(rng.range(1, 4));
            }
            Edit::Save {
                key: match rng.below(4) {
                    0 => None,
                    1 => Some(rng.next() % 40),
                    _ => Some(key(rng)),
                },
                requirement,
                count: rng.range(0, 4),
                total: rng.chance(40).then(|| rng.range(0, 14)),
                copy_depth: depth(rng),
            }
        }
    }
}

/// A random requirement with every field drawn independently, valid or not:
/// items of any family, tiers and upgrades out of range, effects of the
/// other family, flags on families that reject them, labels from 0 up. The
/// problem list must word every combination.
pub(crate) fn arbitrary_requirement(rng: &mut Rng) -> Requirement {
    let kind = rng.pick(&[
        ItemKind::Weapon,
        ItemKind::Armor,
        ItemKind::Wand,
        ItemKind::Ring,
        ItemKind::Trinket,
        ItemKind::Artifact,
    ]);
    let mut requirement = Requirement::any(kind);
    if rng.chance(50) {
        requirement.item = Some(if rng.chance(70) {
            let own = items_of(kind);
            rng.pick(&own)
        } else {
            ITEMS[rng.below(ITEMS.len())].id
        });
    }
    if rng.chance(20) {
        requirement.weapon_category =
            Some(rng.pick(&[WeaponCategory::Melee, WeaponCategory::Thrown]));
    }
    // Small values mostly, and now and then the top of the range, where
    // arithmetic on an unchecked field would overflow.
    let value = |rng: &mut Rng, high: u8| {
        if rng.chance(5) {
            u8::MAX
        } else {
            rng.range(0, high)
        }
    };
    requirement.tier = match rng.below(6) {
        0 => TierRequirement::Exact(value(rng, 7)),
        1 => TierRequirement::AtLeast(value(rng, 7)),
        2 => TierRequirement::AtMost(value(rng, 7)),
        _ => TierRequirement::Any,
    };
    requirement.upgrade = match rng.below(4) {
        0 => UpgradeRequirement::Exact(value(rng, 7)),
        1 => UpgradeRequirement::AtLeast(value(rng, 7)),
        _ => UpgradeRequirement::Any,
    };
    if rng.chance(30) {
        let family: Vec<Effect> = if rng.chance(50) {
            ALL_WEAPON_EFFECTS
                .iter()
                .copied()
                .map(Effect::Weapon)
                .collect()
        } else {
            ALL_ARMOR_EFFECTS
                .iter()
                .copied()
                .map(Effect::Armor)
                .collect()
        };
        let set = if rng.chance(20) {
            EffectSet::enchantments(match family[0] {
                Effect::Weapon(_) => ItemKind::Weapon,
                Effect::Armor(_) => ItemKind::Armor,
            })
        } else {
            EffectSet::from_effects((0..rng.range(1, 3)).map(|_| rng.pick(&family)))
        };
        if let Some(set) = set {
            requirement.effect = EffectRequirement::OneOf(set);
        }
    }
    requirement.require_uncursed = rng.chance(25);
    requirement.select_trinket = rng.chance(10);
    if rng.chance(15) {
        requirement.trinket_transmutations = value(rng, 16);
    }
    if rng.chance(15) {
        requirement.artifact_transmutations = value(rng, 12);
    }
    requirement.blanket = rng.chance(15);
    requirement.exclude_resin = rng.chance(12);
    if rng.chance(15) {
        requirement.source = Some(rng.pick(ItemSource::ALL));
    }
    if rng.chance(20) {
        requirement.identity_group = Some(value(rng, 6));
    }
    if rng.chance(25) {
        requirement.max_depth = Some(value(rng, 27));
    }
    if rng.chance(20) {
        requirement.alternative_group = Some(value(rng, 4));
    }
    if rng.chance(20) {
        requirement.level_sum = Some(LevelSum {
            group: value(rng, 6),
            minimum_total: value(rng, 14),
        });
    }
    requirement
}

/// [`random_rows`] with some rows swapped for [`arbitrary_requirement`]s —
/// lists whose groups mostly make sense and whose rows often do not.
pub(crate) fn mixed_rows(rng: &mut Rng) -> Vec<Row> {
    let mut rows = random_rows(rng);
    for row in &mut rows {
        if rng.chance(30) {
            row.requirement = arbitrary_requirement(rng);
        }
    }
    rows
}
