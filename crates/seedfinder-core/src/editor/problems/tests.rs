//! The problem list: per-row wording, the problems between rows, the
//! list-level check, their order and whom they blame.
//!
//! These port the per-row and group cases of the web's
//! `validation.test.ts` and Windows' `QueryRelationshipsTests` validation
//! facts (in the shared wording), and check the list against the engine's
//! own validation both ways.

use super::super::testing::{
    Rng, arbitrary_requirement, mixed_rows, named, random_rows, requirements, row, validate,
};
use super::*;
use crate::catalog::{ArmorEffect, Effect, ItemId, ItemKind, WeaponCategory, WeaponEffect};
use crate::model::ItemSource;
use crate::query::{EffectSet, LevelSum};

/// The messages of `rows`' problems, in order.
fn messages(rows: &[Row]) -> Vec<String> {
    problems(rows)
        .into_iter()
        .map(|problem| problem.message)
        .collect()
}

/// The messages of one requirement listed alone.
fn alone(requirement: Requirement) -> Vec<String> {
    messages(&[Row {
        key: 1,
        requirement,
    }])
}

fn weapon() -> Requirement {
    Requirement::any(ItemKind::Weapon)
}

fn item_of(item_id: ItemId) -> Requirement {
    named(0, item_id).requirement
}

fn keyed(requirements: &[Requirement]) -> Vec<Row> {
    requirements
        .iter()
        .zip(1..)
        .map(|(&requirement, key)| Row { key, requirement })
        .collect()
}

fn effects(effects: &[Effect]) -> EffectRequirement {
    EffectRequirement::OneOf(EffectSet::from_effects(effects.iter().copied()).unwrap())
}

fn weapon_effects(effects: &[WeaponEffect]) -> EffectRequirement {
    self::effects(
        &effects
            .iter()
            .copied()
            .map(Effect::Weapon)
            .collect::<Vec<_>>(),
    )
}

/// A combined-level membership, as a requirement carries it.
#[allow(clippy::unnecessary_wraps)] // Reads as the field it fills.
fn level_sum(group: u8, minimum_total: u8) -> Option<LevelSum> {
    Some(LevelSum {
        group,
        minimum_total,
    })
}

fn might() -> Requirement {
    item_of(ItemId::RingMight)
}

/// Whether some message contains `needle`.
fn mentions(found: &[String], needle: &str) -> bool {
    found.iter().any(|message| message.contains(needle))
}

// --- web validation.test.ts ----------------------------------------------

#[test]
fn a_tier_on_a_named_item_asks_for_a_wildcard() {
    let found = alone(Requirement {
        tier: TierRequirement::Exact(3),
        ..item_of(ItemId::Sword)
    });
    assert_eq!(found, ["Tier filters require a wildcard weapon or armor."]);
}

#[test]
fn upgrades_stop_at_the_v4_ceilings() {
    let upgrade = |kind: ItemKind, category: Option<WeaponCategory>, value: u8| {
        alone(Requirement {
            weapon_category: category,
            upgrade: UpgradeRequirement::Exact(value),
            ..Requirement::any(kind)
        })
    };
    let melee = Some(WeaponCategory::Melee);
    let thrown = Some(WeaponCategory::Thrown);
    // The Imp's vault reaches +5 on weapons and +4 on everything else.
    for (kind, category, value) in [
        (ItemKind::Weapon, None, 5),
        (ItemKind::Weapon, melee, 5),
        (ItemKind::Weapon, thrown, 5),
        (ItemKind::Armor, None, 4),
        (ItemKind::Wand, None, 4),
        (ItemKind::Ring, None, 4),
    ] {
        assert!(
            upgrade(kind, category, value).is_empty(),
            "{kind:?} +{value}"
        );
    }
    assert_eq!(
        upgrade(ItemKind::Weapon, None, 6),
        ["Upgrade must be 1 through +5."]
    );
    assert_eq!(
        upgrade(ItemKind::Armor, None, 5),
        ["Upgrade must be 1 through +4."]
    );
    assert_eq!(
        upgrade(ItemKind::Wand, None, 5),
        ["Upgrade must be 1 through +4."]
    );
    let haste = Requirement {
        upgrade: UpgradeRequirement::Exact(5),
        ..item_of(ItemId::RingHaste)
    };
    assert_eq!(alone(haste), ["Upgrade must be 1 through +4."]);
    // An "at least" bound answers to the same ceiling, from 0.
    assert!(
        alone(Requirement {
            upgrade: UpgradeRequirement::AtLeast(5),
            ..weapon()
        })
        .is_empty()
    );
    assert_eq!(
        alone(Requirement {
            upgrade: UpgradeRequirement::AtLeast(5),
            ..Requirement::any(ItemKind::Armor)
        }),
        ["Upgrade must be 0 through +4."]
    );
}

#[test]
fn the_top_weapon_upgrade_needs_the_tier_that_reaches_it() {
    let plus5 = |patch: fn(&mut Requirement)| {
        let mut requirement = Requirement {
            upgrade: UpgradeRequirement::Exact(5),
            ..weapon()
        };
        patch(&mut requirement);
        alone(requirement)
    };
    assert!(plus5(|_| {}).is_empty());
    assert!(plus5(|r| r.tier = TierRequirement::Exact(4)).is_empty());
    assert!(plus5(|r| r.tier = TierRequirement::AtLeast(4)).is_empty());
    assert!(plus5(|r| r.item = Some(ItemId::BattleAxe)).is_empty());
    assert!(plus5(|r| r.item = Some(ItemId::Javelin)).is_empty());
    let out_of_reach = ["Upgrade must be 1 through +4; only a tier-4 weapon reaches +5."];
    assert_eq!(plus5(|r| r.tier = TierRequirement::Exact(5)), out_of_reach);
    assert_eq!(plus5(|r| r.tier = TierRequirement::AtMost(3)), out_of_reach);
    assert_eq!(plus5(|r| r.item = Some(ItemId::Sword)), out_of_reach);
}

#[test]
fn effects_belong_to_the_family_and_curses_clash_with_uncursed() {
    use WeaponEffect::{
        Annoying, Blazing, Blocking, Crystal, Eldritch, Pressurized, Projecting, Sacrificial,
        Vampiric, Venomous, Vorpal, Wondrous,
    };
    // The enchantments v4.0.0 added are weapon enchantments.
    let v4 = weapon_effects(&[Venomous, Eldritch, Vorpal, Crystal]);
    assert!(
        alone(Requirement {
            effect: v4,
            ..weapon()
        })
        .is_empty()
    );
    assert!(
        alone(Requirement {
            effect: weapon_effects(&[Blocking, Projecting, Vampiric]),
            ..weapon()
        })
        .is_empty()
    );
    // Only curses with "uncursed" matches nothing; a mixed set is fine.
    let uncursed = |effect| Requirement {
        effect,
        require_uncursed: true,
        ..weapon()
    };
    assert_eq!(
        alone(uncursed(weapon_effects(&[Pressurized, Wondrous]))),
        ["An uncursed item cannot have only curse effects."]
    );
    assert_eq!(
        alone(uncursed(weapon_effects(&[Annoying, Sacrificial]))),
        ["An uncursed item cannot have only curse effects."]
    );
    assert_eq!(
        alone(uncursed(weapon_effects(&[Annoying]))),
        ["An uncursed item cannot have a curse effect."]
    );
    assert!(alone(uncursed(weapon_effects(&[Annoying, Blazing]))).is_empty());
    let glyphs = EffectSet::enchantments(ItemKind::Armor).unwrap();
    assert!(
        alone(Requirement {
            effect: EffectRequirement::OneOf(glyphs),
            require_uncursed: true,
            ..Requirement::any(ItemKind::Armor)
        })
        .is_empty()
    );
    // A weapon enchantment on armor names the stranger.
    assert_eq!(
        alone(Requirement {
            effect: weapon_effects(&[Vorpal]),
            ..Requirement::any(ItemKind::Armor)
        }),
        ["The effect Vorpal does not belong to this category."]
    );
    assert_eq!(
        alone(Requirement {
            effect: weapon_effects(&[Blocking, Projecting]),
            ..Requirement::any(ItemKind::Armor)
        }),
        ["The effect Blocking, Projecting does not belong to this category."]
    );
    // Rings, wands and the rest carry no effect at all.
    let enchantments = EffectSet::enchantments(ItemKind::Weapon).unwrap();
    assert_eq!(
        alone(Requirement {
            effect: EffectRequirement::OneOf(enchantments),
            ..Requirement::any(ItemKind::Ring)
        }),
        ["Effects require a weapon or armor category."]
    );
}

#[test]
fn melee_and_thrown_kinds_check_their_item() {
    let narrowed = |category, item_id: Option<ItemId>| Requirement {
        weapon_category: Some(category),
        item: item_id,
        kind: item_id.map_or(ItemKind::Weapon, |item_id| {
            crate::catalog::item(item_id).kind
        }),
        ..weapon()
    };
    assert!(alone(narrowed(WeaponCategory::Melee, None)).is_empty());
    assert!(
        alone(Requirement {
            tier: TierRequirement::Exact(5),
            ..narrowed(WeaponCategory::Thrown, None)
        })
        .is_empty()
    );
    assert!(alone(narrowed(WeaponCategory::Thrown, Some(ItemId::Shuriken))).is_empty());
    assert!(
        alone(Requirement {
            effect: weapon_effects(&[WeaponEffect::Projecting]),
            ..narrowed(WeaponCategory::Thrown, None)
        })
        .is_empty()
    );
    assert_eq!(
        alone(narrowed(WeaponCategory::Melee, Some(ItemId::Shuriken))),
        ["The item is not a melee weapon."]
    );
    assert_eq!(
        alone(narrowed(WeaponCategory::Thrown, Some(ItemId::Sword))),
        ["The item is not a thrown weapon."]
    );
    // A melee weapon naming a ring: the item is in the wrong category.
    assert_eq!(
        alone(Requirement {
            weapon_category: Some(WeaponCategory::Melee),
            item: Some(ItemId::RingHaste),
            ..weapon()
        }),
        ["The item does not belong to this category."]
    );
    // Only the typed model can narrow a wand; the web folds the narrowing
    // into the kind.
    assert_eq!(
        alone(Requirement {
            weapon_category: Some(WeaponCategory::Melee),
            ..Requirement::any(ItemKind::Wand)
        }),
        ["Only a weapon can be melee or thrown."]
    );
}

#[test]
fn a_full_valid_query_has_no_problems() {
    let anchor = Requirement {
        tier: TierRequirement::AtLeast(3),
        upgrade: UpgradeRequirement::AtLeast(2),
        effect: weapon_effects(&[WeaponEffect::Blazing]),
        source: Some(ItemSource::LockedChest),
        max_depth: Some(12),
        identity_group: Some(1),
        ..weapon()
    };
    let copy = Requirement {
        identity_group: Some(1),
        ..weapon()
    };
    assert!(messages(&keyed(&[anchor, copy])).is_empty());
}

#[test]
fn combined_levels_agree_on_a_reachable_total() {
    let member = |minimum_total, upgrade| Requirement {
        level_sum: level_sum(1, minimum_total),
        upgrade,
        ..might()
    };
    let any = UpgradeRequirement::Any;
    assert!(messages(&keyed(&[member(4, any), member(4, any)])).is_empty());
    assert_eq!(
        messages(&keyed(&[member(2, any), member(3, any)])),
        ["A stack must share one combined level."]
    );
    // An item counts its upgrade plus one, and a world levels only one ring
    // — the Imp vault's prize — past +2: a pair reaches 5 + 3 = 8 together.
    assert_eq!(
        messages(&keyed(&[
            member(10, any),
            member(10, UpgradeRequirement::Exact(3))
        ])),
        ["A combined level of 10 needs more items: these 2 can reach 8."]
    );
    // Only rings combine levels; the sword pair is blamed row by row.
    let sword = Requirement {
        level_sum: level_sum(1, 3),
        ..item_of(ItemId::Sword)
    };
    let found = problems(&keyed(&[sword, sword]));
    assert_eq!(found[0].message, "Only rings can count levels together.");
    assert_eq!(
        (found[0].keys.as_slice(), found[0].scope),
        (&[1][..], ProblemScope::Row)
    );
    assert_eq!(found[1].keys, [2]);
    assert_eq!(
        alone(Requirement {
            level_sum: level_sum(1, 0),
            ..might()
        }),
        ["A combined level must be at least 1."]
    );
    assert_eq!(
        alone(Requirement {
            level_sum: level_sum(5, 1),
            ..might()
        }),
        ["A combined-level group must be 1 through 4."]
    );
    let alternative = Requirement {
        alternative_group: Some(1),
        ..might()
    };
    assert!(mentions(
        &messages(&keyed(&[
            Requirement {
                level_sum: level_sum(1, 2),
                ..alternative
            },
            alternative,
        ])),
        "An either/or alternative cannot count a combined level."
    ));
}

#[test]
fn a_stack_has_one_category_and_one_constrained_unit() {
    let spear = Requirement {
        identity_group: Some(1),
        ..item_of(ItemId::Spear)
    };
    let sword = Requirement {
        identity_group: Some(1),
        ..item_of(ItemId::Sword)
    };
    let copy = Requirement {
        identity_group: Some(1),
        ..weapon()
    };
    let clustered = |requirement: Requirement| Requirement {
        alternative_group: Some(1),
        ..requirement
    };
    // An either/or cluster may anchor a stack; its bare copy follows
    // whichever member matched.
    assert!(messages(&keyed(&[clustered(spear), clustered(sword), copy])).is_empty());
    // Copies of another category can never be the same item.
    let armor = Requirement {
        identity_group: Some(1),
        ..Requirement::any(ItemKind::Armor)
    };
    assert_eq!(
        messages(&keyed(&[spear, armor])),
        ["The copies of a stack must share its category."]
    );
    assert_eq!(
        messages(&keyed(&[copy, armor])),
        ["The copies of a stack must share its category."]
    );
    // A second constrained unit describes two items forced to be one.
    let mace = Requirement {
        identity_group: Some(1),
        ..item_of(ItemId::Mace)
    };
    assert_eq!(
        messages(&keyed(&[clustered(spear), clustered(sword), mace])),
        ["Only one item of a stack can carry constraints; the extra copies are plain."]
    );
}

// --- Windows QueryRelationshipsTests -----------------------------------------

#[test]
fn a_group_that_cannot_reach_its_total_says_so() {
    let ring = |minimum_total| Requirement {
        level_sum: level_sum(1, minimum_total),
        ..Requirement::any(ItemKind::Ring)
    };
    // Two rings of any upgrade reach eight levels together, not nine.
    assert_eq!(
        messages(&keyed(&[ring(9), ring(9)])),
        ["A combined level of 9 needs more items: these 2 can reach 8."]
    );
    assert!(messages(&keyed(&[ring(8), ring(8)])).is_empty());
    assert_eq!(
        messages(&keyed(&[ring(8), ring(5)])),
        ["A stack must share one combined level."]
    );
}

#[test]
fn identity_groups_are_stacks_with_one_anchor() {
    let named = |group, alternative| Requirement {
        upgrade: UpgradeRequirement::Exact(2),
        identity_group: Some(group),
        alternative_group: alternative,
        ..might()
    };
    let plain = |kind, group, max_depth| Requirement {
        identity_group: Some(group),
        max_depth,
        ..Requirement::any(kind)
    };
    let check = |requirements: &[Requirement]| messages(&keyed(requirements));
    let overconstrained =
        ["Only one item of a stack can carry constraints; the extra copies are plain."];
    // One anchor with plain copies; a floor limit on a copy is fine.
    assert!(
        check(&[
            named(1, None),
            plain(ItemKind::Ring, 1, None),
            plain(ItemKind::Ring, 1, Some(6))
        ])
        .is_empty()
    );
    assert!(
        check(&[
            plain(ItemKind::Ring, 1, None),
            plain(ItemKind::Ring, 1, None)
        ])
        .is_empty()
    );
    assert_eq!(check(&[named(1, None), named(1, None)]), overconstrained);
    let uncursed_copy = Requirement {
        require_uncursed: true,
        ..plain(ItemKind::Ring, 1, None)
    };
    assert_eq!(check(&[named(1, None), uncursed_copy]), overconstrained);
    // The members of one alternative group form a single anchor unit.
    assert!(
        check(&[
            named(1, Some(1)),
            named(1, Some(1)),
            plain(ItemKind::Ring, 1, None)
        ])
        .is_empty()
    );
    assert_eq!(
        check(&[named(1, Some(1)), named(1, Some(1)), named(1, None)]),
        overconstrained
    );
    assert_eq!(
        check(&[named(1, None), plain(ItemKind::Wand, 1, None)]),
        ["The copies of a stack must share its category."]
    );
    // A narrowed kind is a constraint, the broad one is plain.
    let thrown = Requirement {
        weapon_category: Some(WeaponCategory::Thrown),
        ..plain(ItemKind::Weapon, 2, None)
    };
    assert!(check(&[thrown, plain(ItemKind::Weapon, 2, None)]).is_empty());
    // Separate groups are separate stacks.
    assert!(check(&[named(1, None), named(2, None)]).is_empty());
}

#[test]
fn what_the_engine_rejects_the_editor_words() {
    let ring = Requirement::any(ItemKind::Ring);
    assert_eq!(
        alone(Requirement {
            alternative_group: Some(1),
            level_sum: level_sum(1, 1),
            ..ring
        }),
        ["An either/or alternative cannot count a combined level."]
    );
    assert_eq!(
        alone(Requirement {
            level_sum: level_sum(1, 1),
            ..Requirement::any(ItemKind::Wand)
        }),
        ["Only rings can count levels together."]
    );
    assert!(alone(Requirement::any(ItemKind::Wand)).is_empty());
}

// --- the remaining per-row rules --------------------------------------------

#[test]
#[allow(clippy::too_many_lines)] // One case per rule, in the web's order.
fn trinkets_artifacts_blankets_and_resin_exclusions_are_checked_in_order() {
    let rat_skull = item_of(ItemId::RatSkull);
    assert_eq!(
        alone(Requirement {
            trinket_transmutations: 14,
            select_trinket: true,
            ..rat_skull
        }),
        [
            "Choose a transmutation count from 1 to 13.",
            "Only an initial offer can be chosen at +3.",
        ]
    );
    assert_eq!(
        alone(Requirement {
            trinket_transmutations: 2,
            artifact_transmutations: 11,
            ..weapon()
        }),
        [
            "Choose an artifact transmutation count from 0 to 10.",
            "Only artifacts can use artifact transmutations.",
            "Only trinkets can require transmutations.",
        ]
    );
    assert_eq!(
        alone(Requirement {
            select_trinket: true,
            exclude_resin: true,
            ..Requirement::any(ItemKind::Ring)
        }),
        [
            "Only an ordinary wand can exclude Auto resin.",
            "Only a trinket can be selected.",
        ]
    );
    assert_eq!(
        row_problems(&Requirement {
            blanket: true,
            exclude_resin: true,
            identity_group: Some(2),
            ..Requirement::any(ItemKind::Wand)
        }),
        [
            "Only an ordinary wand can exclude Auto resin.",
            "A blanket requirement cannot request extra copies, combined levels, or trinket \
             selection.",
        ]
    );
    assert_eq!(
        alone(Requirement::any(ItemKind::Trinket)),
        ["Select a trinket."]
    );
    assert_eq!(
        alone(Requirement::any(ItemKind::Artifact)),
        ["Select an artifact."]
    );
    assert_eq!(
        alone(Requirement {
            item: Some(ItemId::Spear),
            ..Requirement::any(ItemKind::Wand)
        }),
        ["The item does not belong to this category."]
    );
    assert_eq!(
        alone(Requirement {
            tier: TierRequirement::Exact(1),
            ..Requirement::any(ItemKind::Ring)
        }),
        [
            "Tier filters require a wildcard weapon or armor.",
            "Exact tier must be 2 through 5.",
        ]
    );
    assert_eq!(
        alone(Requirement {
            tier: TierRequirement::AtMost(5),
            ..Requirement::any(ItemKind::Armor)
        }),
        ["Tier bounds must be 3 or 4."]
    );
    assert_eq!(
        alone(Requirement {
            max_depth: Some(0),
            ..weapon()
        }),
        ["Requirement floor must be 1 through 24."]
    );
    assert_eq!(
        alone(Requirement {
            identity_group: Some(5),
            ..weapon()
        }),
        ["A stack group must be 1 through 4."]
    );
    assert_eq!(
        alone(Requirement {
            alternative_group: Some(0),
            ..weapon()
        }),
        ["An either/or group must be 1 or higher."]
    );
    // Stones, not swords, on armor: a glyph set is fine there.
    assert!(
        alone(Requirement {
            effect: EffectRequirement::exactly(Effect::Armor(ArmorEffect::Stone)),
            ..Requirement::any(ItemKind::Armor)
        })
        .is_empty()
    );
}

// --- order, blame and the list ---------------------------------------------

#[test]
fn problems_run_row_by_row_then_between_rows_then_for_the_list() {
    // Two blanket wands tied into a stack of two kinds, and a broken floor.
    let blanket = |kind| Requirement {
        blanket: true,
        identity_group: Some(1),
        ..Requirement::any(kind)
    };
    let rows = [
        Row {
            key: 7,
            requirement: blanket(ItemKind::Wand),
        },
        Row {
            key: 3,
            requirement: Requirement {
                max_depth: Some(30),
                ..Requirement::any(ItemKind::Ring)
            },
        },
        Row {
            key: 5,
            requirement: blanket(ItemKind::Ring),
        },
    ];
    let relations =
        "A blanket requirement cannot request extra copies, combined levels, or trinket selection.";
    // With an ordinary ring among them there is no list problem.
    let found = problems(&rows);
    let summary: Vec<(&str, &[u64], ProblemScope)> = found
        .iter()
        .map(|problem| {
            (
                problem.message.as_str(),
                problem.keys.as_slice(),
                problem.scope,
            )
        })
        .collect();
    assert_eq!(
        summary,
        [
            (relations, &[7][..], ProblemScope::Row),
            (
                "Requirement floor must be 1 through 24.",
                &[3][..],
                ProblemScope::Row
            ),
            (relations, &[5][..], ProblemScope::Row),
            (
                "The copies of a stack must share its category.",
                &[7, 5][..],
                ProblemScope::Group
            ),
        ]
    );
    // Without it, the list itself is the last problem.
    let blankets = [rows[0], rows[2]];
    let found = problems(&blankets);
    let last = found.last().unwrap();
    assert_eq!(
        (last.message.as_str(), last.keys.as_slice(), last.scope),
        (NO_ORDINARY_REQUIREMENT, &[][..], ProblemScope::List)
    );
    assert_eq!(found.len(), 4);
    // An empty list is the platforms' to judge.
    assert!(problems(&[]).is_empty());
    assert!(problems(&[row(1, ItemKind::Wand)]).is_empty());
}

#[test]
fn a_group_problem_blames_every_row_of_its_group_in_list_order() {
    let rows = keyed(&[
        Requirement {
            alternative_group: Some(2),
            ..Requirement::any(ItemKind::Wand)
        },
        Requirement::any(ItemKind::Ring),
        Requirement {
            alternative_group: Some(2),
            blanket: true,
            ..Requirement::any(ItemKind::Wand)
        },
        Requirement {
            alternative_group: Some(2),
            ..Requirement::any(ItemKind::Armor)
        },
    ]);
    let found = problems(&rows);
    assert_eq!(found.len(), 1);
    assert_eq!(
        found[0].message,
        "An either/or group cannot mix ordinary and blanket requirements."
    );
    assert_eq!(found[0].keys, [1, 3, 4]);
    assert_eq!(found[0].scope, ProblemScope::Group);
}

#[test]
fn a_requirement_has_no_problem_exactly_when_the_engine_and_the_formats_accept_it() {
    // Every field drawn independently, valid or not (1,024 cases).
    let mut rng = Rng::new(0x00a1_1ce5_bea7);
    let mut clean = 0;
    for case in 0..1024 {
        let requirement = arbitrary_requirement(&mut rng);
        let found = row_problems(&requirement);
        let portable = requirement
            .identity_group
            .is_none_or(|group| (1..=MAX_IDENTITY_GROUP).contains(&group))
            && requirement
                .level_sum
                .is_none_or(|sum| (1..=MAX_LEVEL_SUM_GROUP).contains(&sum.group));
        assert_eq!(
            found.is_empty(),
            requirement.validate().is_ok() && portable,
            "case {case}: {requirement:?} {found:?}"
        );
        // Every message is a sentence, and none repeats.
        for message in &found {
            assert!(
                message.starts_with(|c: char| c.is_ascii_uppercase()) && message.ends_with('.'),
                "{message}"
            );
        }
        let mut unique = found.clone();
        unique.sort();
        unique.dedup();
        assert_eq!(unique.len(), found.len(), "{found:?}");
        clean += usize::from(found.is_empty());
    }
    assert!((50..974).contains(&clean), "{clean} clean requirements");
}

#[test]
fn a_list_has_no_problem_exactly_when_the_engine_accepts_it() {
    // Generated lists: stacks, clusters and combined levels that mostly make
    // sense, with valid rows or rows that often are not (1,024 cases).
    let mut rng = Rng::new(0x0011_570f_1157);
    let (mut clean, mut compared) = (0, 0);
    for case in 0..1024 {
        let rows = if case % 2 == 0 {
            random_rows(&mut rng)
        } else {
            mixed_rows(&mut rng)
        };
        if rows.is_empty() {
            continue;
        }
        let found = problems(&rows);
        let portable = requirements(&rows).iter().all(|requirement| {
            requirement
                .identity_group
                .is_none_or(|group| (1..=MAX_IDENTITY_GROUP).contains(&group))
                && requirement
                    .level_sum
                    .is_none_or(|sum| (1..=MAX_LEVEL_SUM_GROUP).contains(&sum.group))
        });
        let context = format!("case {case}: {rows:?} {found:?}");
        assert_eq!(
            found.is_empty(),
            validate(&rows).is_ok() && portable,
            "{context}"
        );
        // Row problems first, in list order; then groups; then the list.
        let scopes: Vec<ProblemScope> = found.iter().map(|problem| problem.scope).collect();
        let mut sorted = scopes.clone();
        sorted.sort_by_key(|scope| match scope {
            ProblemScope::Row => 0,
            ProblemScope::Group => 1,
            ProblemScope::List => 2,
        });
        assert_eq!(scopes, sorted, "{context}");
        let positions: Vec<usize> = found
            .iter()
            .filter(|problem| problem.scope == ProblemScope::Row)
            .map(|problem| {
                assert_eq!(problem.keys.len(), 1, "{context}");
                rows.iter()
                    .position(|row| row.key == problem.keys[0])
                    .unwrap()
            })
            .collect();
        assert!(positions.is_sorted(), "{context}");
        // The engine's first group error is the first group problem.
        if let Some(first) = found
            .iter()
            .find(|problem| problem.scope == ProblemScope::Group)
            && found
                .iter()
                .all(|problem| problem.scope != ProblemScope::Row)
            && rows.iter().any(|row| !row.requirement.blanket)
        {
            let error = validate(&rows).expect_err("a group problem fails validation");
            assert!(
                matches!(
                    error,
                    QueryError::InconsistentLevelSum { .. }
                        | QueryError::UnattainableLevelSum { .. }
                        | QueryError::MixedBlanketAlternatives
                        | QueryError::InconsistentIdentityGroup
                        | QueryError::OverconstrainedIdentityGroup
                ),
                "{context}"
            );
            assert_eq!(
                first.message,
                group_message(error, first.keys.len()),
                "{context}"
            );
            compared += 1;
        }
        clean += usize::from(found.is_empty());
    }
    // Both outcomes, and the first-error comparison, occur often enough to
    // mean something.
    assert!((200..800).contains(&clean), "{clean} clean lists");
    assert!(compared >= 40, "{compared} group comparisons");
}

#[test]
fn the_web_built_in_presets_have_no_problems() {
    let wand = |upgrade| Requirement {
        upgrade,
        ..Requirement::any(ItemKind::Wand)
    };
    let stacked = |requirement: Requirement| Requirement {
        identity_group: Some(1),
        ..requirement
    };
    let any = UpgradeRequirement::Any;
    let staff = |top| {
        vec![
            stacked(wand(UpgradeRequirement::Exact(top))),
            stacked(wand(any)),
            stacked(wand(any)),
            wand(UpgradeRequirement::AtLeast(1)),
        ]
    };
    let bonanza = vec![
        wand(UpgradeRequirement::Exact(3)),
        Requirement {
            max_depth: Some(4),
            ..wand(UpgradeRequirement::Exact(2))
        },
        Requirement {
            max_depth: Some(4),
            ..wand(UpgradeRequirement::Exact(2))
        },
        wand(UpgradeRequirement::Exact(2)),
    ];
    let wealth = vec![
        Requirement {
            upgrade: UpgradeRequirement::Exact(4),
            source: Some(ItemSource::ImpReward),
            ..item_of(ItemId::RingWealth)
        },
        Requirement {
            upgrade: UpgradeRequirement::Exact(2),
            max_depth: Some(4),
            ..item_of(ItemId::RingWealth)
        },
    ];
    let tier_four = vec![
        stacked(Requirement {
            tier: TierRequirement::Exact(4),
            upgrade: UpgradeRequirement::Exact(5),
            ..weapon()
        }),
        stacked(weapon()),
        stacked(weapon()),
    ];
    for (name, preset) in [
        ("+21 Staff", staff(3)),
        ("+22 Staff", staff(4)),
        ("Wand Bonanza", bonanza),
        ("+21 Ring of Wealth", wealth),
        ("+26 Tier 4 Weapon", tier_four),
    ] {
        let rows = keyed(&preset);
        assert_eq!(problems(&rows), [], "{name}");
        assert_eq!(validate(&rows), Ok(()), "{name}");
    }
}
