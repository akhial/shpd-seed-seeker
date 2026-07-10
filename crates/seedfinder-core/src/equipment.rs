//! Exact v3.3.8 randomization of generated weapons, armor, and wands.

use crate::catalog::{ArmorEffect, Effect, WeaponEffect};
use crate::rng::RandomStack;

const WEAPON_COMMON: [WeaponEffect; 4] = [
    WeaponEffect::Blazing,
    WeaponEffect::Chilling,
    WeaponEffect::Kinetic,
    WeaponEffect::Shocking,
];
const WEAPON_UNCOMMON: [WeaponEffect; 6] = [
    WeaponEffect::Blocking,
    WeaponEffect::Blooming,
    WeaponEffect::Elastic,
    WeaponEffect::Lucky,
    WeaponEffect::Projecting,
    WeaponEffect::Unstable,
];
const WEAPON_RARE: [WeaponEffect; 3] = [
    WeaponEffect::Corrupting,
    WeaponEffect::Grim,
    WeaponEffect::Vampiric,
];
const WEAPON_CURSES: [WeaponEffect; 8] = [
    WeaponEffect::Annoying,
    WeaponEffect::Displacing,
    WeaponEffect::Dazzling,
    WeaponEffect::Explosive,
    WeaponEffect::Sacrificial,
    WeaponEffect::Wayward,
    WeaponEffect::Polarized,
    WeaponEffect::Friendly,
];

const ARMOR_COMMON: [ArmorEffect; 4] = [
    ArmorEffect::Obfuscation,
    ArmorEffect::Swiftness,
    ArmorEffect::Viscosity,
    ArmorEffect::Potential,
];
const ARMOR_UNCOMMON: [ArmorEffect; 6] = [
    ArmorEffect::Brimstone,
    ArmorEffect::Stone,
    ArmorEffect::Entanglement,
    ArmorEffect::Repulsion,
    ArmorEffect::Camouflage,
    ArmorEffect::Flow,
];
const ARMOR_RARE: [ArmorEffect; 3] = [
    ArmorEffect::Affection,
    ArmorEffect::AntiMagic,
    ArmorEffect::Thorns,
];
const ARMOR_CURSES: [ArmorEffect; 8] = [
    ArmorEffect::AntiEntropy,
    ArmorEffect::Corrosion,
    ArmorEffect::Displacement,
    ArmorEffect::Metabolism,
    ArmorEffect::Multiplicity,
    ArmorEffect::Stench,
    ArmorEffect::Overgrowth,
    ArmorEffect::Bulk,
];

/// Mutable properties assigned by an equipment class's `random()` method.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct EquipmentRoll {
    pub upgrade: u8,
    pub effect: Option<Effect>,
    pub cursed: bool,
}

/// Mirrors `Weapon.random()` for the canonical profile (no Parchment Scrap).
pub fn roll_weapon(random: &mut RandomStack) -> EquipmentRoll {
    let upgrade = nested_upgrade_roll(random, 4);

    // Upstream deliberately isolates effect variance in a child RNG.
    let effect_seed = random.long();
    random.push(effect_seed);
    let effect_roll = random.float();
    let result = if effect_roll < 0.3 {
        EquipmentRoll {
            upgrade,
            effect: Some(Effect::Weapon(select(random, &WEAPON_CURSES))),
            cursed: true,
        }
    } else if effect_roll >= 0.9 {
        EquipmentRoll {
            upgrade,
            effect: Some(Effect::Weapon(random_weapon_enchantment(random))),
            cursed: false,
        }
    } else {
        EquipmentRoll {
            upgrade,
            effect: None,
            cursed: false,
        }
    };
    random.pop();
    result
}

/// Mirrors `Armor.random()` for the canonical profile (no Parchment Scrap).
pub fn roll_armor(random: &mut RandomStack) -> EquipmentRoll {
    let upgrade = nested_upgrade_roll(random, 4);

    let effect_seed = random.long();
    random.push(effect_seed);
    let effect_roll = random.float();
    let result = if effect_roll < 0.3 {
        EquipmentRoll {
            upgrade,
            effect: Some(Effect::Armor(select(random, &ARMOR_CURSES))),
            cursed: true,
        }
    } else if effect_roll >= 0.85 {
        EquipmentRoll {
            upgrade,
            effect: Some(Effect::Armor(random_armor_glyph(random))),
            cursed: false,
        }
    } else {
        EquipmentRoll {
            upgrade,
            effect: None,
            cursed: false,
        }
    };
    random.pop();
    result
}

/// Mirrors `Wand.random()`. Wands have a cursed flag but no enchantment type.
pub fn roll_wand(random: &mut RandomStack) -> EquipmentRoll {
    EquipmentRoll {
        upgrade: nested_upgrade_roll(random, 3),
        effect: None,
        cursed: random.float() < 0.3,
    }
}

fn nested_upgrade_roll(random: &mut RandomStack, first_bound: i32) -> u8 {
    if random.int_bound(first_bound) != 0 {
        return 0;
    }
    if random.int_bound(5) == 0 { 2 } else { 1 }
}

/// Mirrors `Weapon.Enchantment.random()` outside an item's `random()` method.
pub fn random_weapon_enchantment(random: &mut RandomStack) -> WeaponEffect {
    match random.chances(&[50.0, 40.0, 10.0]).unwrap_or_default() {
        0 => select(random, &WEAPON_COMMON),
        1 => select(random, &WEAPON_UNCOMMON),
        _ => select(random, &WEAPON_RARE),
    }
}

/// Mirrors `Armor.Glyph.random()` outside an item's `random()` method.
pub fn random_armor_glyph(random: &mut RandomStack) -> ArmorEffect {
    match random.chances(&[50.0, 40.0, 10.0]).unwrap_or_default() {
        0 => select(random, &ARMOR_COMMON),
        1 => select(random, &ARMOR_UNCOMMON),
        _ => select(random, &ARMOR_RARE),
    }
}

fn select<T: Copy>(random: &mut RandomStack, values: &[T]) -> T {
    let bound = i32::try_from(values.len()).unwrap_or(1);
    let index = usize::try_from(random.int_bound(bound)).unwrap_or_default();
    values[index]
}

#[cfg(test)]
mod tests {
    use crate::catalog::{ArmorEffect, Effect, WeaponEffect};
    use crate::rng::RandomStack;

    use super::{EquipmentRoll, roll_armor, roll_wand, roll_weapon};

    fn stack(seed: i64) -> RandomStack {
        let mut random = RandomStack::with_base_seed(0);
        random.push(seed);
        random
    }

    #[test]
    fn actual_java_game_sequence_matches_for_abc_numeric_seed() {
        let mut random = stack(8_687_205_886);
        assert_eq!(
            roll_weapon(&mut random),
            EquipmentRoll {
                upgrade: 2,
                effect: None,
                cursed: false
            }
        );
        assert_eq!(
            roll_armor(&mut random),
            EquipmentRoll {
                upgrade: 0,
                effect: None,
                cursed: false
            }
        );
        assert_eq!(
            roll_wand(&mut random),
            EquipmentRoll {
                upgrade: 0,
                effect: None,
                cursed: false
            }
        );
        assert_eq!(roll_weapon(&mut random).upgrade, 0);
        assert_eq!(roll_armor(&mut random).upgrade, 0);
        assert_eq!(roll_wand(&mut random).upgrade, 0);
    }

    #[test]
    fn actual_java_game_curse_and_enchantment_fixtures_match() {
        let weapon_zero = roll_weapon(&mut stack(0));
        assert_eq!(weapon_zero.upgrade, 0);
        assert_eq!(
            weapon_zero.effect,
            Some(Effect::Weapon(WeaponEffect::Polarized))
        );
        assert!(weapon_zero.cursed);

        let armor_zero = roll_armor(&mut stack(0));
        assert_eq!(
            armor_zero.effect,
            Some(Effect::Armor(ArmorEffect::Overgrowth))
        );
        assert!(armor_zero.cursed);

        let weapon_nine = roll_weapon(&mut stack(9));
        assert_eq!(
            weapon_nine.effect,
            Some(Effect::Weapon(WeaponEffect::Shocking))
        );
        assert!(!weapon_nine.cursed);

        let armor_nine = roll_armor(&mut stack(9));
        assert_eq!(
            armor_nine.effect,
            Some(Effect::Armor(ArmorEffect::Potential))
        );
        assert!(!armor_nine.cursed);
    }
}
