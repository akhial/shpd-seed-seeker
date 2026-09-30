// SPDX-License-Identifier: GPL-3.0-or-later

//! Presets bundled with every installation.

use shpd_seedfinder_core::catalog::{
    self, ArmorEffect, Effect, ItemId, ItemKind, WeaponCategory, WeaponEffect,
};
use shpd_seedfinder_core::editor::Row;
use shpd_seedfinder_core::floor_filters::{FloorRequirement, RoomType};
use shpd_seedfinder_core::level_prelude::Feeling;
use shpd_seedfinder_core::query::{
    EffectRequirement, Requirement, TierRequirement, UpgradeRequirement,
};
use shpd_seedfinder_core::quests::WandmakerQuestType;

use crate::state::AppState;

/// One read-only query shipped with the application.
#[derive(Clone, Debug)]
pub struct BuiltInPreset {
    pub name: &'static str,
    pub state: AppState,
}

/// Returns the protected presets in presentation order.
#[must_use]
pub fn built_in() -> [BuiltInPreset; 5] {
    [
        disintegrate(),
        guerilla_assassin(),
        ring_of_wealth(),
        necromancer(),
        blood_berserker(),
    ]
}

fn preset(
    name: &'static str,
    requirements: impl IntoIterator<Item = Requirement>,
) -> BuiltInPreset {
    let mut state = AppState::default();
    for requirement in requirements {
        let key = state.claim_key();
        state.requirements.push(Row { key, requirement });
    }
    BuiltInPreset { name, state }
}

fn named(item: ItemId) -> Requirement {
    Requirement {
        item: Some(item),
        ..Requirement::any(catalog::item(item).kind)
    }
}

fn disintegrate() -> BuiltInPreset {
    let wand = named(ItemId::WandDisintegration);
    let mut preset = preset(
        "DISINTEGRATE",
        [
            Requirement {
                upgrade: UpgradeRequirement::AtLeast(3),
                ..wand
            },
            wand,
            wand,
            Requirement {
                trinket_transmutations: 1,
                ..named(ItemId::EyeOfNewt)
            },
            Requirement {
                upgrade: UpgradeRequirement::AtLeast(2),
                ..named(ItemId::RingEnergy)
            },
        ],
    );
    preset.state.max_depth = 19;
    preset
}

fn guerilla_assassin() -> BuiltInPreset {
    preset(
        "Guerilla Assassin",
        [
            Requirement {
                upgrade: UpgradeRequirement::Exact(3),
                effect: EffectRequirement::exactly(Effect::Weapon(WeaponEffect::Blooming)),
                max_depth: Some(7),
                ..named(ItemId::AssassinsBlade)
            },
            Requirement {
                effect: EffectRequirement::exactly(Effect::Armor(ArmorEffect::Camouflage)),
                ..Requirement::any(ItemKind::Armor)
            },
            Requirement {
                upgrade: UpgradeRequirement::AtLeast(2),
                ..named(ItemId::RingArcana)
            },
        ],
    )
}

/// Early gear for a wealth run, and the dark garden floor 17 farms on. It
/// already names its trinket, so automatic trinket selection is off.
fn ring_of_wealth() -> BuiltInPreset {
    let mut preset = preset(
        "Ring of Wealth",
        [
            Requirement {
                upgrade: UpgradeRequirement::Exact(4),
                ..named(ItemId::RingWealth)
            },
            Requirement {
                max_depth: Some(9),
                ..named(ItemId::DriedRose)
            },
            Requirement {
                tier: TierRequirement::AtMost(4),
                upgrade: UpgradeRequirement::Exact(3),
                max_depth: Some(4),
                ..Requirement::any(ItemKind::Armor)
            },
            Requirement {
                tier: TierRequirement::AtMost(4),
                upgrade: UpgradeRequirement::Exact(3),
                max_depth: Some(9),
                ..Requirement::any(ItemKind::Weapon)
            },
            Requirement {
                trinket_transmutations: 1,
                ..named(ItemId::DimensionalSundial)
            },
        ],
    );
    preset.state.auto_apply_trinket = false;
    preset.state.floor_requirements.push(FloorRequirement {
        depth: 17,
        feeling: Some(Feeling::Dark),
        rooms: Vec::new(),
        any_rooms: vec![RoomType::SpecialGarden, RoomType::SecretGarden],
    });
    preset
}

fn necromancer() -> BuiltInPreset {
    let mut preset = preset(
        "Necromancer",
        [
            Requirement {
                upgrade: UpgradeRequirement::Exact(3),
                ..named(ItemId::WandCorruption)
            },
            Requirement {
                weapon_category: Some(WeaponCategory::Melee),
                tier: TierRequirement::Exact(5),
                upgrade: UpgradeRequirement::Exact(3),
                ..Requirement::any(ItemKind::Weapon)
            },
            Requirement {
                upgrade: UpgradeRequirement::Exact(3),
                ..named(ItemId::PlateArmor)
            },
        ],
    );
    preset.state.max_depth = 14;
    preset.state.wandmaker_quest = Some(WandmakerQuestType::CorpseDust);
    preset
}

fn blood_berserker() -> BuiltInPreset {
    preset(
        "Blood Berserker",
        [
            Requirement {
                weapon_category: Some(WeaponCategory::Melee),
                tier: TierRequirement::Exact(5),
                upgrade: UpgradeRequirement::Exact(3),
                effect: EffectRequirement::exactly(Effect::Weapon(WeaponEffect::Vampiric)),
                ..Requirement::any(ItemKind::Weapon)
            },
            Requirement {
                upgrade: UpgradeRequirement::Exact(3),
                effect: EffectRequirement::exactly(Effect::Armor(ArmorEffect::Thorns)),
                ..named(ItemId::PlateArmor)
            },
            Requirement {
                upgrade: UpgradeRequirement::Exact(4),
                ..named(ItemId::RingArcana)
            },
            named(ItemId::ChaliceOfBlood),
        ],
    )
}

#[cfg(test)]
mod tests {
    use shpd_seedfinder_core::json_query;

    use super::{
        Effect, ItemId, ItemKind, TierRequirement, UpgradeRequirement, WeaponCategory,
        WeaponEffect, built_in,
    };
    use crate::state::is_farming_requirement;

    /// Each preset as the shared query-document format writes it, so the
    /// literals above are checked against the queries they were taken from.
    const DOCUMENTS: [(&str, &str); 5] = [
        (
            "DISINTEGRATE",
            r#"{"auto_apply_trinket":true,"max_depth":19,"requirements":[
                {"item":"wand_disintegration","kind":"wand","upgrade":{"at_least":3}},
                {"item":"wand_disintegration","kind":"wand"},
                {"item":"wand_disintegration","kind":"wand"},
                {"item":"eye_of_newt","kind":"trinket","trinket_transmutations":1},
                {"item":"ring_energy","kind":"ring","upgrade":{"at_least":2}}]}"#,
        ),
        (
            "Guerilla Assassin",
            r#"{"auto_apply_trinket":true,"requirements":[
                {"effect":"Blooming","item":"assassins_blade","kind":"weapon","max_depth":7,"upgrade":3},
                {"effect":"Camouflage","kind":"armor"},
                {"item":"ring_arcana","kind":"ring","upgrade":{"at_least":2}}]}"#,
        ),
        (
            "Ring of Wealth",
            r#"{
                "floor_requirements":[{"any_rooms":["garden","secret_garden"],"depth":17,"feeling":"dark"}],
                "requirements":[
                {"item":"ring_wealth","kind":"ring","upgrade":4},
                {"item":"dried_rose","kind":"artifact","max_depth":9},
                {"kind":"armor","max_depth":4,"tier":{"at_most":4},"upgrade":3},
                {"kind":"weapon","max_depth":9,"tier":{"at_most":4},"upgrade":3},
                {"item":"dimensional_sundial","kind":"trinket","trinket_transmutations":1}]}"#,
        ),
        (
            "Necromancer",
            r#"{"auto_apply_trinket":true,"max_depth":14,"wandmaker_quest":"corpse_dust","requirements":[
                {"item":"wand_corruption","kind":"wand","upgrade":3},
                {"kind":"melee_weapon","tier":{"exact":5},"upgrade":3},
                {"item":"plate_armor","kind":"armor","upgrade":3}]}"#,
        ),
        (
            "Blood Berserker",
            r#"{"auto_apply_trinket":true,"requirements":[
                {"effect":"Vampiric","kind":"melee_weapon","tier":{"exact":5},"upgrade":3},
                {"effect":"Thorns","item":"plate_armor","kind":"armor","upgrade":3},
                {"item":"ring_arcana","kind":"ring","upgrade":4},
                {"item":"chalice_of_blood","kind":"artifact"}]}"#,
        ),
    ];

    #[test]
    fn tier_five_build_weapons_match_melee_but_not_thrown() {
        use shpd_seedfinder_core::model::{Accessibility, ItemSource, WorldItem};

        let [_, _, _, necromancer, berserker] = built_in();
        for preset in [necromancer, berserker] {
            let weapon = preset
                .state
                .requirements
                .iter()
                .find(|row| row.requirement.kind == ItemKind::Weapon)
                .expect("build has a weapon requirement")
                .requirement;
            assert_eq!(
                weapon.weapon_category,
                Some(WeaponCategory::Melee),
                "{}",
                preset.name
            );
            assert_eq!(weapon.tier, TierRequirement::Exact(5), "{}", preset.name);
            assert_eq!(
                weapon.upgrade,
                UpgradeRequirement::Exact(3),
                "{}",
                preset.name
            );

            let greatsword = WorldItem {
                item: ItemId::Greatsword,
                upgrade: 3,
                effect: Some(Effect::Weapon(WeaponEffect::Vampiric)),
                cursed: false,
                depth: 1,
                source: ItemSource::Heap,
                accessibility: Accessibility::Independent,
                secret: false,
            };
            assert!(
                weapon.matches(&greatsword),
                "{} accepts +3 T5 melee",
                preset.name
            );
            assert!(
                !weapon.matches(&WorldItem {
                    item: ItemId::ThrowingHammer,
                    ..greatsword
                }),
                "{} rejects +3 T5 thrown even with the same effect",
                preset.name
            );
            assert!(
                !weapon.matches(&WorldItem {
                    item: ItemId::Sword,
                    ..greatsword
                }),
                "{} still rejects lower-tier melee",
                preset.name
            );
            assert!(
                !weapon.matches(&WorldItem {
                    upgrade: 2,
                    ..greatsword
                }),
                "{} still requires exactly +3",
                preset.name
            );
        }
    }

    #[test]
    fn every_preset_is_the_query_it_was_taken_from() {
        let presets = built_in();
        assert_eq!(
            presets.each_ref().map(|preset| preset.name),
            DOCUMENTS.map(|(name, _)| name)
        );
        for (preset, (name, document)) in presets.iter().zip(DOCUMENTS) {
            assert_eq!(
                preset.state.unvalidated_query(),
                json_query::decode_unvalidated(document).expect(name),
                "{name}"
            );
        }
    }

    /// A preset loads as the board writes it: the editor finds nothing to
    /// rewrite, so editing one chip never reshapes the rest.
    #[test]
    fn every_preset_is_already_in_the_editors_encoding() {
        for mut preset in built_in() {
            let rows = preset.state.requirements.clone();
            assert!(
                !preset
                    .state
                    .apply(&[shpd_seedfinder_core::editor::Edit::Normalize])
                    .changed,
                "{}",
                preset.name
            );
            assert_eq!(preset.state.requirements, rows, "{}", preset.name);
        }
    }

    #[test]
    fn every_preset_builds_a_runnable_query() {
        for preset in built_in() {
            assert!(
                preset.state.to_query().is_ok(),
                "{} does not validate: {:?}",
                preset.name,
                preset.state.to_query().err()
            );
        }
    }

    /// The wealth preset's floor is the one the farming toggle writes, so
    /// the toggle shows it selected.
    #[test]
    fn ring_of_wealth_farms_the_floor_the_toggle_selects() {
        let [_, _, wealth, _, _] = built_in();
        assert_eq!(wealth.name, "Ring of Wealth");
        assert!(matches!(
            wealth.state.floor_requirements.as_slice(),
            [floor] if floor.depth == 17 && is_farming_requirement(floor)
        ));
    }
}
