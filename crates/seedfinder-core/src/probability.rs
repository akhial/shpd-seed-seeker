//! Query probability estimates derived from the canonical v3.3.8 item tables.

use crate::catalog::{ArmorEffect, Effect, ItemKind, WeaponEffect, item};
use crate::generator::FLOOR_SET_TIER_PROBABILITIES;
use crate::query::{Requirement, SearchQuery, UpgradeRequirement};

/// Estimates a query's intrinsic spawn probability before traversal starts.
///
/// Each requirement is evaluated from the same tier, identity, upgrade, and
/// modifier weights used by generation, then AND requirements are multiplied.
/// This deliberately remains fixed for a search; observed results never feed
/// back into the estimate.
#[must_use]
pub fn estimate_match_probability(query: &SearchQuery) -> f64 {
    query
        .requirements
        .iter()
        .map(|requirement| requirement_probability(*requirement, query.max_depth))
        .product::<f64>()
        .clamp(0.0, 1.0)
}

fn requirement_probability(requirement: Requirement, max_depth: u8) -> f64 {
    identity_probability(requirement, max_depth)
        * upgrade_probability(requirement.kind, requirement.upgrade)
        * modifier_probability(requirement.effect)
}

fn identity_probability(requirement: Requirement, max_depth: u8) -> f64 {
    let Some(item_id) = requirement.item else {
        return 1.0;
    };
    let definition = item(item_id);
    match requirement.kind {
        ItemKind::Weapon => {
            let tier = usize::from(definition.tier.unwrap_or(1)).saturating_sub(1);
            average_tier_probability(tier, max_depth) / weapon_identity_count(tier) as f64
        }
        ItemKind::Armor => {
            let tier = usize::from(definition.tier.unwrap_or(1)).saturating_sub(1);
            average_tier_probability(tier, max_depth)
        }
        ItemKind::Wand => 1.0 / 13.0,
        ItemKind::Ring => 1.0 / 12.0,
    }
}

fn average_tier_probability(tier: usize, max_depth: u8) -> f64 {
    let mut total = 0.0;
    let mut floors = 0_u32;
    for depth in 1..=max_depth {
        if depth % 5 == 0 {
            continue;
        }
        let floor_set = usize::from((depth - 1) / 5).min(4);
        total += f64::from(FLOOR_SET_TIER_PROBABILITIES[floor_set][tier]) / 100.0;
        floors += 1;
    }
    if floors == 0 {
        0.0
    } else {
        total / f64::from(floors)
    }
}

const fn weapon_identity_count(tier: usize) -> u32 {
    match tier {
        0 => 5,
        1 | 2 => 6,
        _ => 7,
    }
}

const fn upgrade_probability(kind: ItemKind, requirement: UpgradeRequirement) -> f64 {
    match requirement {
        UpgradeRequirement::Any => 1.0,
        UpgradeRequirement::Exact(upgrade) => exact_upgrade_probability(kind, upgrade),
        UpgradeRequirement::AtLeast(minimum) => {
            let maximum = kind.maximum_search_upgrade();
            let mut probability = 0.0;
            let mut upgrade = minimum;
            while upgrade <= maximum {
                probability += exact_upgrade_probability(kind, upgrade);
                upgrade += 1;
            }
            if probability > 1.0 { 1.0 } else { probability }
        }
    }
}

const fn exact_upgrade_probability(kind: ItemKind, upgrade: u8) -> f64 {
    match (kind, upgrade) {
        (_, 0) => match kind {
            ItemKind::Weapon | ItemKind::Armor => 0.75,
            ItemKind::Wand | ItemKind::Ring => 2.0 / 3.0,
        },
        (ItemKind::Weapon | ItemKind::Armor, 1) => 0.20,
        (ItemKind::Weapon | ItemKind::Armor, 2 | 3) => 0.05,
        (ItemKind::Wand | ItemKind::Ring, 1) => 4.0 / 15.0,
        (ItemKind::Wand | ItemKind::Ring, 2) => 1.0 / 15.0,
        // Wandmaker and Imp rewards add one and two upgrades respectively.
        (ItemKind::Wand, 3) | (ItemKind::Ring, 4) => 1.0 / 15.0,
        (ItemKind::Ring, 3) => 4.0 / 15.0,
        _ => 0.0,
    }
}

const fn modifier_probability(effect: Option<Effect>) -> f64 {
    match effect {
        None => 1.0,
        Some(Effect::Weapon(effect)) if effect.is_curse() => 0.30 / 8.0,
        Some(Effect::Weapon(effect)) => 0.10 * weapon_enchantment_probability(effect),
        Some(Effect::Armor(effect)) if effect.is_curse() => 0.30 / 8.0,
        Some(Effect::Armor(effect)) => 0.15 * armor_glyph_probability(effect),
    }
}

const fn weapon_enchantment_probability(effect: WeaponEffect) -> f64 {
    match effect as u8 {
        0..=3 => 0.50 / 4.0,
        4..=9 => 0.40 / 6.0,
        _ => 0.10 / 3.0,
    }
}

const fn armor_glyph_probability(effect: ArmorEffect) -> f64 {
    match effect as u8 {
        0..=3 => 0.50 / 4.0,
        4..=9 => 0.40 / 6.0,
        _ => 0.10 / 3.0,
    }
}

#[cfg(test)]
mod tests {
    use crate::catalog::{ArmorEffect, Effect, ItemId, ItemKind, WeaponEffect};
    use crate::query::{Requirement, SearchQuery, UpgradeRequirement};

    use super::estimate_match_probability;

    fn query(requirements: Vec<Requirement>) -> SearchQuery {
        SearchQuery {
            requirements,
            max_depth: 24,
            require_blacksmith: false,
            exclude_blacksmith_rewards: false,
            fast_mode: false,
        }
    }

    #[test]
    fn combines_canonical_identity_upgrade_and_modifier_weights() {
        let probability = estimate_match_probability(&query(vec![
            Requirement {
                kind: ItemKind::Weapon,
                item: Some(ItemId::Sword),
                upgrade: UpgradeRequirement::Exact(2),
                effect: Some(Effect::Weapon(WeaponEffect::Lucky)),
                source: None,
                identity_group: None,
            },
            Requirement {
                kind: ItemKind::Armor,
                item: Some(ItemId::PlateArmor),
                upgrade: UpgradeRequirement::Exact(1),
                effect: Some(Effect::Armor(ArmorEffect::Brimstone)),
                source: None,
                identity_group: None,
            },
        ]));

        assert!((probability - 7.857_777_777_777_78e-9).abs() < 1e-20);
    }
}
