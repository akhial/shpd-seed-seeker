//! The board view: chip names, titles, tags, details, relations, badges,
//! descriptions, the resin chip, and where problems show.
//!
//! These port the chip-text cases of the web's `summary.test.ts` and board,
//! Windows' `QueryRelationshipsTests` (`ShortTitle`, `Tags`, `ChipDetail`) and
//! Linux's `state.rs` label tests, in the shared wording: "Any Tier 3+
//! weapon" titles, "effect: A/B" sets, "any glyph" on armor.

use super::super::testing::{Rng, mixed_rows, named, random_edit, row, with};
use super::super::{Edit, apply, board_items, join_candidates, problems, stack_view};
use super::*;
use crate::catalog::{ArmorEffect, WeaponCategory, WeaponEffect};
use crate::model::ItemSource;
use crate::query::{LevelSum, TierRequirement};

fn view(rows: &[Row]) -> BoardView {
    board_view(rows, None)
}

/// The chip showing the row `key`.
fn chip(board: &BoardView, key: u64) -> &ChipView {
    board
        .items
        .iter()
        .flat_map(|item| &item.chips)
        .find(|chip| chip.key == key)
        .expect("a chip shows the row")
}

/// The entry holding the row `key` as a member or a hidden copy.
fn entry(board: &BoardView, key: u64) -> &ItemView {
    board
        .items
        .iter()
        .find(|item| item.members.contains(&key) || item.extras.contains(&key))
        .expect("an entry holds the row")
}

fn texts(tags: &[Tag]) -> Vec<&str> {
    tags.iter().map(|tag| tag.text.as_str()).collect()
}

/// The rows after `edits`, which must all apply.
fn edited(rows: &[Row], edits: &[Edit]) -> Vec<Row> {
    let result = apply(rows, None, edits);
    assert_eq!(result.refused, None, "{edits:?}");
    result.rows
}

/// A new chip saved from the sheet with `count` items.
fn saved(requirement: Requirement, count: u8, copy_depth: Option<u8>) -> Edit {
    Edit::Save {
        key: None,
        requirement,
        count,
        total: None,
        copy_depth,
    }
}

fn weapon_effects(effects: &[WeaponEffect]) -> EffectRequirement {
    EffectRequirement::OneOf(
        EffectSet::from_effects(effects.iter().copied().map(Effect::Weapon)).unwrap(),
    )
}

fn relation(glyph: RelationGlyph, text: &str) -> Relation {
    Relation {
        glyph,
        text: text.to_owned(),
    }
}

fn lone(requirement: Requirement) -> ChipView {
    let rows = [Row {
        key: 1,
        requirement,
    }];
    chip(&view(&rows), 1).clone()
}

// --- names, titles and tags --------------------------------------------------

#[test]
fn a_chip_names_itself_and_carries_its_qualifiers_as_tags() {
    // Windows `AChipNamesItselfAndCarriesItsQualifiersAsTags`.
    let wildcard = lone(Requirement {
        weapon_category: Some(WeaponCategory::Thrown),
        tier: TierRequirement::AtLeast(3),
        upgrade: UpgradeRequirement::AtLeast(2),
        max_depth: Some(4),
        ..Requirement::any(ItemKind::Weapon)
    });
    assert_eq!(wildcard.name, "Any thrown");
    assert_eq!(wildcard.title, "Any Tier 3+ thrown weapon");
    assert_eq!(texts(&wildcard.tags), ["T3+", "+2↑", "F≤4"]);
    assert_eq!(
        wildcard
            .tags
            .iter()
            .map(|tag| tag.style)
            .collect::<Vec<_>>(),
        [TagStyle::Plain, TagStyle::Upgrade, TagStyle::Plain]
    );
    assert_eq!(
        (wildcard.kind, wildcard.family, wildcard.item),
        (KindName::ThrownWeapon, ItemKind::Weapon, None)
    );
    // A named item shows no tier: it is the tier it is.
    let might = lone(
        with(named(1, ItemId::RingMight), |r| {
            r.upgrade = UpgradeRequirement::Exact(2);
        })
        .requirement,
    );
    assert_eq!(
        (might.name.as_str(), might.title.as_str()),
        ("Ring of Might", "Ring of Might")
    );
    assert_eq!(texts(&might.tags), ["+2"]);
    assert_eq!(might.item, Some(ItemId::RingMight));
    assert_eq!(lone(Requirement::any(ItemKind::Wand)).name, "Any wand");
    let artifact = lone(Requirement::any(ItemKind::Artifact));
    assert_eq!(
        (artifact.name.as_str(), artifact.title.as_str()),
        ("Artifact", "Artifact")
    );
}

#[test]
fn tags_run_transmute_or_choose_then_tier_upgrade_floor_and_no_resin_trails() {
    let weapon = lone(Requirement {
        tier: TierRequirement::Exact(3),
        upgrade: UpgradeRequirement::Exact(2),
        max_depth: Some(9),
        effect: weapon_effects(&[WeaponEffect::Blazing]),
        require_uncursed: true,
        ..Requirement::any(ItemKind::Weapon)
    });
    assert_eq!(texts(&weapon.tags), ["T3", "+2", "F≤9"]);
    assert!(weapon.trailing_tags.is_empty());
    assert_eq!(
        weapon.effect.as_ref().map(|effect| effect.label.as_str()),
        Some("Blazing")
    );
    assert!(weapon.uncursed);

    let wand = lone(Requirement {
        upgrade: UpgradeRequirement::AtLeast(1),
        max_depth: Some(4),
        exclude_resin: true,
        ..Requirement::any(ItemKind::Wand)
    });
    assert_eq!(texts(&wand.tags), ["+1↑", "F≤4"]);
    assert_eq!(texts(&wand.trailing_tags), ["No resin"]);
    assert_eq!(wand.trailing_tags[0].style, TagStyle::Plain);

    let transmuted =
        lone(with(named(1, ItemId::RatSkull), |r| r.trinket_transmutations = 3).requirement);
    assert_eq!(texts(&transmuted.tags), ["Transmute ≤3"]);
    let artifact = lone(
        with(named(1, ItemId::ChaliceOfBlood), |r| {
            r.artifact_transmutations = 2;
            r.max_depth = Some(6);
        })
        .requirement,
    );
    assert_eq!(texts(&artifact.tags), ["Transmute ≤2", "F≤6"]);
    // A tier on a named item is a problem, never a tag.
    let tiered = lone(
        with(named(1, ItemId::Sword), |r| {
            r.tier = TierRequirement::Exact(3);
        })
        .requirement,
    );
    assert!(tiered.tags.is_empty());
    assert!(tiered.problem.is_some());
}

#[test]
fn a_trinket_chosen_at_plus_three_says_so_on_the_chip_and_in_its_description() {
    let chosen = lone(with(named(1, ItemId::RatSkull), |r| r.select_trinket = true).requirement);
    assert_eq!(texts(&chosen.tags), ["choose at +3"]);
    assert_eq!(chosen.details, ["choose at +3"]);
    assert_eq!(chosen.description, "Rat Skull, choose at +3");
}

#[test]
fn the_description_is_the_title_then_each_detail_once() {
    let transmuted =
        lone(with(named(1, ItemId::RatSkull), |r| r.trinket_transmutations = 3).requirement);
    assert_eq!(transmuted.details, ["within 3 transmutations"]);
    assert_eq!(transmuted.description, "Rat Skull, within 3 transmutations");
    let once = lone(
        with(named(1, ItemId::DriedRose), |r| {
            r.artifact_transmutations = 1;
        })
        .requirement,
    );
    assert_eq!(once.description, "Dried Rose, within 1 transmutation");
    // Trinkets and artifacts are never upgraded in a search: no "any upgrade".
    assert_eq!(
        lone(named(1, ItemId::ChaliceOfBlood).requirement).description,
        "Chalice of Blood"
    );
    assert_eq!(
        lone(Requirement::any(ItemKind::Weapon)).description,
        "Any weapon, any upgrade"
    );
    let everything = lone(Requirement {
        upgrade: UpgradeRequirement::AtLeast(2),
        exclude_resin: true,
        require_uncursed: true,
        source: Some(ItemSource::LockedChest),
        max_depth: Some(9),
        ..Requirement::any(ItemKind::Wand)
    });
    assert_eq!(
        everything.description,
        "Any wand, +2 or higher, uncursed, excluded from Auto resin, Locked chest, floors 1–9"
    );
}

// --- Linux state.rs labels -----------------------------------------------------

#[test]
fn labels_describe_wildcards_and_predicates() {
    let mut requirement = Requirement::any(ItemKind::Weapon);
    let chip = lone(requirement);
    assert_eq!(
        (chip.title.as_str(), chip.name.as_str()),
        ("Any weapon", "Any weapon")
    );
    assert_eq!(chip.details, ["any upgrade"]);

    requirement.tier = TierRequirement::AtLeast(4);
    requirement.upgrade = UpgradeRequirement::Exact(2);
    requirement.identity_group = Some(2);
    requirement.max_depth = Some(9);
    requirement.require_uncursed = true;
    let chip = lone(requirement);
    assert_eq!(chip.title, "Any Tier 4+ weapon");
    // The chip keeps the tier out of the name: it rides beside it as a tag.
    assert_eq!(chip.name, "Any weapon");
    assert_eq!(chip.details, ["exactly +2", "uncursed", "floors 1–9"]);

    requirement.tier = TierRequirement::AtMost(3);
    assert_eq!(lone(requirement).title, "Any Tier 3 or lower weapon");

    requirement.tier = TierRequirement::Any;
    requirement.item = Some(ItemId::Greatsword);
    let chip = lone(requirement);
    assert_eq!(
        (chip.title.as_str(), chip.name.as_str()),
        ("Greatsword", "Greatsword")
    );
}

#[test]
fn a_weapon_type_narrows_the_name_and_the_title() {
    let mut requirement = Requirement {
        weapon_category: Some(WeaponCategory::Thrown),
        ..Requirement::any(ItemKind::Weapon)
    };
    assert_eq!(lone(requirement).title, "Any thrown weapon");
    requirement.tier = TierRequirement::Exact(5);
    assert_eq!(lone(requirement).title, "Any Tier 5 thrown weapon");
    requirement.weapon_category = Some(WeaponCategory::Melee);
    requirement.tier = TierRequirement::Any;
    let chip = lone(requirement);
    assert_eq!(
        (chip.title.as_str(), chip.name.as_str()),
        ("Any melee weapon", "Any melee")
    );
    assert_eq!(chip.kind, KindName::MeleeWeapon);
}

#[test]
fn details_describe_effect_sets_and_predicates() {
    let blazing = Requirement {
        effect: weapon_effects(&[WeaponEffect::Blazing]),
        ..Requirement::any(ItemKind::Weapon)
    };
    assert_eq!(chip_details(&blazing, false), ["any upgrade", "Blazing"]);
    // Catalog order, not selection order.
    let pair = Requirement {
        effect: weapon_effects(&[WeaponEffect::Projecting, WeaponEffect::Blocking]),
        ..blazing
    };
    assert_eq!(
        chip_details(&pair, false),
        ["any upgrade", "effect: Blocking/Projecting"]
    );
    let glyphs = Requirement {
        effect: EffectRequirement::OneOf(EffectSet::enchantments(ItemKind::Armor).unwrap()),
        require_uncursed: true,
        ..Requirement::any(ItemKind::Armor)
    };
    assert_eq!(
        chip_details(&glyphs, false),
        ["any upgrade", "any glyph", "uncursed"]
    );
    let stone = Requirement {
        effect: EffectRequirement::exactly(Effect::Armor(ArmorEffect::Stone)),
        upgrade: UpgradeRequirement::AtLeast(1),
        max_depth: Some(4),
        ..Requirement::any(ItemKind::Armor)
    };
    assert_eq!(
        chip_details(&stone, false),
        ["+1 or higher", "Stone", "floors 1–4"]
    );
    // A combined level speaks for the upgrades; a bound still shows.
    assert!(chip_details(&named(1, ItemId::RingMight).requirement, true).is_empty());
    let exact = with(named(1, ItemId::RingMight), |r| {
        r.upgrade = UpgradeRequirement::Exact(2);
    });
    assert_eq!(chip_details(&exact.requirement, true), ["exactly +2"]);
}

// --- web summary.test.ts ---------------------------------------------------------

#[test]
fn the_effect_cue_names_a_filter_and_lists_its_effects() {
    assert_eq!(effect_badge(&Requirement::any(ItemKind::Weapon)), None);
    let one = effect_badge(&Requirement {
        effect: weapon_effects(&[WeaponEffect::Blazing]),
        ..Requirement::any(ItemKind::Weapon)
    })
    .unwrap();
    assert_eq!(
        one,
        EffectBadge {
            label: "Blazing".to_owned(),
            effects: vec![Effect::Weapon(WeaponEffect::Blazing)],
            any_enchantment: false,
            curses_only: false,
        }
    );
    let pair = effect_badge(&Requirement {
        effect: weapon_effects(&[WeaponEffect::Projecting, WeaponEffect::Blocking]),
        ..Requirement::any(ItemKind::Weapon)
    })
    .unwrap();
    assert_eq!(pair.label, "effect: Blocking/Projecting");
    assert_eq!(
        pair.effects,
        [
            Effect::Weapon(WeaponEffect::Blocking),
            Effect::Weapon(WeaponEffect::Projecting)
        ]
    );
    // "Any enchantment" lists the whole non-curse family, so a glow can be
    // drawn from it, and says what it is.
    let weapons = EffectSet::enchantments(ItemKind::Weapon).unwrap();
    let any = effect_badge(&Requirement {
        effect: EffectRequirement::OneOf(weapons),
        ..Requirement::any(ItemKind::Weapon)
    })
    .unwrap();
    assert_eq!(any.label, "any enchantment");
    assert!(any.any_enchantment && !any.curses_only);
    assert_eq!(any.effects, weapons.effects().collect::<Vec<_>>());
    let glyphs = effect_badge(&Requirement {
        effect: EffectRequirement::OneOf(EffectSet::enchantments(ItemKind::Armor).unwrap()),
        ..Requirement::any(ItemKind::Armor)
    })
    .unwrap();
    assert_eq!(glyphs.label, "any glyph");
    let curses = effect_badge(&Requirement {
        effect: weapon_effects(&[WeaponEffect::Annoying, WeaponEffect::Wayward]),
        ..Requirement::any(ItemKind::Weapon)
    })
    .unwrap();
    assert!(curses.curses_only && !curses.any_enchantment);
}

#[test]
fn the_popover_lists_the_combined_level_and_the_stack() {
    let rings = edited(
        &[],
        &[
            saved(named(0, ItemId::RingMight).requirement, 2, None),
            Edit::SetTotal {
                key: 1,
                total: Some(4),
            },
        ],
    );
    let board = view(&rings);
    let anchor = chip(&board, 1);
    assert!(anchor.details.is_empty());
    assert_eq!(
        anchor.relations,
        [relation(RelationGlyph::Sum, "up to 2 — levels add to ≥ 4")]
    );
    assert_eq!(anchor.description, "Ring of Might");

    let weapons = edited(
        &[],
        &[saved(
            Requirement {
                upgrade: UpgradeRequirement::Exact(2),
                effect: weapon_effects(&[WeaponEffect::Blocking, WeaponEffect::Vampiric]),
                ..Requirement::any(ItemKind::Weapon)
            },
            2,
            None,
        )],
    );
    let board = view(&weapons);
    let anchor = chip(&board, 1);
    assert_eq!(anchor.details, ["exactly +2", "effect: Blocking/Vampiric"]);
    assert_eq!(
        anchor.relations,
        [relation(
            RelationGlyph::Times,
            "2 of the same kind — the extra copies: any upgrade, any floor"
        )]
    );
}

#[test]
fn a_cluster_is_captioned_and_counted_once() {
    let rows = [
        with(named(1, ItemId::Spear), |r| r.alternative_group = Some(1)),
        with(named(2, ItemId::Shuriken), |r| {
            r.alternative_group = Some(1);
        }),
        with(named(3, ItemId::Sword), |r| r.alternative_group = Some(1)),
        row(4, ItemKind::Wand),
    ];
    let board = view(&rows);
    assert_eq!(board.items.len(), 2);
    assert_eq!(board.items[0].label.as_deref(), Some("Any of 3"));
    assert_eq!(board.items[0].id, ItemKey::Cluster(1));
    assert_eq!(board.items[0].members, [1, 2, 3]);
    assert_eq!(board.items[1].label, None);
    assert_eq!(board.items[1].id, ItemKey::Chip(4));
    assert_eq!(
        board.counts,
        Counts {
            ordinary: 2,
            blanket: 0
        }
    );
    // Every member names its peers.
    assert_eq!(
        chip(&board, 2).relations,
        [relation(RelationGlyph::Or, "Spear, Sword")]
    );
    assert!(chip(&board, 2).in_cluster && chip(&board, 2).can_detach);
    assert!(!chip(&board, 4).in_cluster && !chip(&board, 4).can_detach);
}

// --- Windows ChipDetail ------------------------------------------------------------

#[test]
fn the_popover_reads_the_stack_and_the_relations_around_it() {
    let longswords = edited(
        &[],
        &[saved(named(0, ItemId::Longsword).requirement, 3, Some(4))],
    );
    let board = view(&longswords);
    let anchor = chip(&board, 1);
    assert_eq!(anchor.details, ["any upgrade"]);
    assert_eq!(
        anchor.relations,
        [relation(
            RelationGlyph::Times,
            "3 of the same kind — the extra copies: any upgrade, floors 1–4"
        )]
    );

    // A combined level speaks for the upgrades, so the chip's own says nothing.
    let rings = edited(
        &[],
        &[
            saved(named(0, ItemId::RingMight).requirement, 3, None),
            Edit::SetTotal {
                key: 1,
                total: Some(5),
            },
        ],
    );
    let board = view(&rings);
    assert!(chip(&board, 1).details.is_empty());
    assert_eq!(
        chip(&board, 1).relations,
        [relation(RelationGlyph::Sum, "up to 3 — levels add to ≥ 5")]
    );

    // A cluster member names its peers.
    let joined = edited(
        &[named(1, ItemId::Spear), named(2, ItemId::Shuriken)],
        &[Edit::Join {
            source: 2,
            target: 1,
        }],
    );
    let board = view(&joined);
    assert_eq!(chip(&board, 1).details, ["any upgrade"]);
    assert_eq!(
        chip(&board, 1).relations,
        [relation(RelationGlyph::Or, "Shuriken")]
    );
    assert_eq!(chip(&board, 1).problem, None);

    // v4.0.0's vault treasure reads as its own source, like every other one.
    let vault = [with(named(1, ItemId::Longsword), |r| {
        r.source = Some(ItemSource::VaultTreasure);
    })];
    assert_eq!(
        chip(&view(&vault), 1).details,
        ["any upgrade", "Vault treasure"]
    );
}

#[test]
fn the_stack_line_says_where_the_copies_may_lie() {
    // Hand-written repeats with their own floor limits fold into one chip.
    let rows = [
        named(1, ItemId::Spear),
        with(named(2, ItemId::Spear), |r| r.max_depth = Some(4)),
        with(named(3, ItemId::Spear), |r| r.max_depth = Some(6)),
    ];
    assert_eq!(
        chip(&view(&rows), 1).relations,
        [relation(
            RelationGlyph::Times,
            "3 of the same kind — the extra copies: any upgrade, own floor limits"
        )]
    );
    // The anchor's own floor limit describes the anchor alone.
    let rows = [
        with(named(1, ItemId::Spear), |r| r.max_depth = Some(9)),
        named(2, ItemId::Spear),
    ];
    let spear = chip(&view(&rows), 1).clone();
    assert_eq!(texts(&spear.tags), ["F≤9"]);
    assert_eq!(
        spear.relations,
        [relation(
            RelationGlyph::Times,
            "2 of the same kind — the extra copies: any upgrade, any floor"
        )]
    );
    // A stacked cluster's line shows on every member, after the peers.
    let rows = edited(
        &[
            with(named(1, ItemId::Spear), |r| r.alternative_group = Some(1)),
            with(named(2, ItemId::Sword), |r| r.alternative_group = Some(1)),
        ],
        &[Edit::SetCount { key: 1, count: 2 }],
    );
    let board = view(&rows);
    for (key, peer) in [(1, "Sword"), (2, "Spear")] {
        assert_eq!(
            chip(&board, key).relations,
            [
                relation(RelationGlyph::Or, peer),
                relation(
                    RelationGlyph::Times,
                    "2 of the same kind — the extra copies: any upgrade, any floor"
                ),
            ]
        );
    }
}

// --- badges ----------------------------------------------------------------------

#[test]
fn badges_show_the_count_and_the_combined_level() {
    // A lone chip shows no badge, but its stepper still reads ×1.
    let board = view(&[named(1, ItemId::Spear)]);
    assert_eq!(board.items[0].badges, Badges::default());
    assert_eq!(board.items[0].stack.count_text, "×1");
    assert_eq!(board.items[0].stack.total_text, "Σ ≥ 0");

    let spears = edited(&[], &[saved(named(0, ItemId::Spear).requirement, 2, None)]);
    let board = view(&spears);
    assert_eq!(
        board.items[0].badges,
        Badges {
            count: Some(Badge {
                text: "×2".to_owned(),
                compact_text: "×2".to_owned(),
                tooltip: "2 of the same kind".to_owned(),
            }),
            total: None,
        }
    );

    let rings = edited(
        &[],
        &[
            saved(named(0, ItemId::RingMight).requirement, 3, None),
            Edit::SetTotal {
                key: 1,
                total: Some(5),
            },
        ],
    );
    let board = view(&rings);
    let item = &board.items[0];
    assert_eq!(
        item.badges,
        Badges {
            count: Some(Badge {
                text: "≤3".to_owned(),
                compact_text: "≤3".to_owned(),
                tooltip: "Up to 3 items".to_owned(),
            }),
            total: Some(Badge {
                text: "Σ ≥ 5".to_owned(),
                compact_text: "Σ≥5".to_owned(),
                tooltip: "Levels add to at least 5 (a +0 item counts 1)".to_owned(),
            }),
        }
    );
    assert_eq!(item.stack, stack_view(&rings, &board_items(&rings)[0]));
    assert_eq!(item.extras.len(), 2);
}

// --- problems on the board ----------------------------------------------------------

#[test]
fn a_hidden_copys_problem_shows_on_its_entry_and_its_anchor() {
    // A hand-written repeat past the last floor folds into the spear's stack.
    let rows = [
        named(1, ItemId::Spear),
        with(named(2, ItemId::Spear), |r| r.max_depth = Some(30)),
        row(3, ItemKind::Wand),
    ];
    let board = view(&rows);
    assert_eq!(board.items.len(), 2);
    assert_eq!(board.items[0].extras, [2]);
    let floor = "Requirement floor must be 1 through 24.";
    assert_eq!(board.items[0].problem.as_deref(), Some(floor));
    assert_eq!(chip(&board, 1).problem.as_deref(), Some(floor));
    assert_eq!(board.items[1].problem, None);
    assert_eq!(chip(&board, 3).problem, None);
    assert_eq!(board.problems, problems(&rows));
    assert_eq!(board.problems[0].keys, [2]);

    // The anchor's own problem comes first.
    let rows = [
        with(named(1, ItemId::Spear), |r| {
            r.upgrade = UpgradeRequirement::Exact(9);
        }),
        with(named(2, ItemId::Spear), |r| r.max_depth = Some(30)),
    ];
    let board = view(&rows);
    let upgrade = "Upgrade must be 1 through +4; only a tier-4 weapon reaches +5.";
    assert_eq!(chip(&board, 1).problem.as_deref(), Some(upgrade));
    assert_eq!(board.items[0].problem.as_deref(), Some(upgrade));
}

/// A hand-written combined level anchored on a bare copy of a named ring's
/// stack: the web folded the anchor into the stack and lost its copies, so
/// a copy's problem showed on no chip. The combined level stays a chip of
/// its own, and its copy's problem shows there.
#[test]
fn a_combined_levels_copies_speak_through_its_chip_beside_a_stack() {
    let sum = Some(LevelSum {
        group: 1,
        minimum_total: 2,
    });
    let rows = [
        with(named(1, ItemId::RingMight), |r| r.identity_group = Some(1)),
        with(row(2, ItemKind::Ring), |r| {
            r.identity_group = Some(1);
            r.level_sum = sum;
        }),
        with(row(3, ItemKind::Ring), |r| {
            r.level_sum = sum;
            r.max_depth = Some(40);
        }),
    ];
    let board = view(&rows);
    assert_eq!(board.items.len(), 2);
    assert_eq!(board.items[1].members, [2]);
    assert_eq!(board.items[1].extras, [3]);
    assert_eq!(board.items[1].stack.total, Some(2));
    let floor = "Requirement floor must be 1 through 24.";
    assert_eq!(board.problems[0].keys, [3]);
    assert_eq!(board.items[1].problem.as_deref(), Some(floor));
    assert_eq!(chip(&board, 2).problem.as_deref(), Some(floor));
    assert_eq!(board.items[0].problem, None);
}

#[test]
fn a_clusters_copies_speak_through_its_first_member_only() {
    let rows = [
        with(named(1, ItemId::Spear), |r| {
            r.alternative_group = Some(1);
            r.identity_group = Some(1);
        }),
        with(named(2, ItemId::Sword), |r| {
            r.alternative_group = Some(1);
            r.identity_group = Some(1);
        }),
        with(row(3, ItemKind::Weapon), |r| {
            r.identity_group = Some(1);
            r.max_depth = Some(0);
        }),
    ];
    let board = view(&rows);
    assert_eq!(board.items.len(), 1);
    assert_eq!(board.items[0].extras, [3]);
    let floor = "Requirement floor must be 1 through 24.";
    assert_eq!(board.items[0].problem.as_deref(), Some(floor));
    assert_eq!(chip(&board, 1).problem.as_deref(), Some(floor));
    assert_eq!(chip(&board, 2).problem, None);
}

#[test]
fn a_problem_between_rows_marks_every_chip_it_blames() {
    // Two constrained members of one stack cannot collapse: two chips.
    let rows = [
        with(named(1, ItemId::Spear), |r| r.identity_group = Some(1)),
        with(named(2, ItemId::Mace), |r| r.identity_group = Some(1)),
        row(3, ItemKind::Ring),
    ];
    let board = view(&rows);
    let message = "Only one item of a stack can carry constraints; the extra copies are plain.";
    for key in [1, 2] {
        assert_eq!(chip(&board, key).problem.as_deref(), Some(message));
        assert_eq!(entry(&board, key).problem.as_deref(), Some(message));
    }
    assert_eq!(chip(&board, 3).problem, None);
    assert_eq!(board.problems.len(), 1);
    assert_eq!(board.problems[0].keys, [1, 2]);
    assert_eq!(board.problems[0].scope, ProblemScope::Group);

    // A row's own problem outranks the group's.
    let rows = [
        with(named(1, ItemId::Spear), |r| {
            r.identity_group = Some(1);
            r.max_depth = Some(25);
        }),
        with(named(2, ItemId::Mace), |r| r.identity_group = Some(1)),
    ];
    let board = view(&rows);
    assert_eq!(
        chip(&board, 1).problem.as_deref(),
        Some("Requirement floor must be 1 through 24.")
    );
    assert_eq!(chip(&board, 2).problem.as_deref(), Some(message));
}

#[test]
fn a_blanket_only_list_shows_its_problem_in_the_list_not_on_a_chip() {
    let rows = [with(row(1, ItemKind::Wand), |r| r.blanket = true)];
    let board = view(&rows);
    assert_eq!(
        board.counts,
        Counts {
            ordinary: 0,
            blanket: 1
        }
    );
    assert!(board.items[0].blanket);
    assert_eq!(board.items[0].problem, None);
    assert_eq!(chip(&board, 1).problem, None);
    assert_eq!(
        board.problems[0].message,
        "Add at least one ordinary requirement."
    );
    assert_eq!(board.problems[0].scope, ProblemScope::List);
}

// --- joins and sections -------------------------------------------------------------

#[test]
fn every_chip_carries_the_rows_it_may_join_and_those_it_is_refused() {
    let rows = edited(
        &[row(3, ItemKind::Wand), named(4, ItemId::RingMight)],
        &[saved(named(0, ItemId::Spear).requirement, 2, None)],
    );
    // [wand 3, ring 4, spear 5, spear copy 6]
    let board = view(&rows);
    let spear = chip(&board, 5);
    assert!(spear.join.is_empty());
    assert_eq!(
        spear.refuse,
        [
            (3, Refusal::MixedCategoryStack),
            (4, Refusal::MixedCategoryStack)
        ]
    );
    assert_eq!(chip(&board, 3).join, [4]);
    assert_eq!(chip(&board, 3).refuse, [(5, Refusal::MixedCategoryStack)]);
    // The answers are the board's join candidates, row by row.
    let candidates = join_candidates(&rows, &board_items(&rows));
    for item in &board.items {
        for chip in &item.chips {
            let index = rows.iter().position(|row| row.key == chip.key).unwrap();
            assert_eq!(chip.join, candidates[index].join);
            assert_eq!(chip.refuse, candidates[index].refuse);
        }
    }
}

#[test]
fn entries_sit_in_their_own_section_and_are_counted_there() {
    let rows = [
        row(1, ItemKind::Wand),
        with(row(2, ItemKind::Ring), |r| r.blanket = true),
        with(row(3, ItemKind::Wand), |r| {
            r.blanket = true;
            r.alternative_group = Some(1);
        }),
        with(row(4, ItemKind::Armor), |r| {
            r.blanket = true;
            r.alternative_group = Some(1);
        }),
        named(5, ItemId::RingMight),
    ];
    let board = view(&rows);
    let sections: Vec<(ItemKey, bool)> = board
        .items
        .iter()
        .map(|item| (item.id, item.blanket))
        .collect();
    assert_eq!(
        sections,
        [
            (ItemKey::Chip(1), false),
            (ItemKey::Chip(2), true),
            (ItemKey::Cluster(1), true),
            (ItemKey::Chip(5), false),
        ]
    );
    assert_eq!(
        board.counts,
        Counts {
            ordinary: 2,
            blanket: 2
        }
    );
}

#[test]
fn the_built_in_staff_preset_reads_as_one_stack_and_one_wand() {
    let stacked = |upgrade| Requirement {
        upgrade,
        identity_group: Some(1),
        ..Requirement::any(ItemKind::Wand)
    };
    let rows: Vec<Row> = [
        stacked(UpgradeRequirement::Exact(3)),
        stacked(UpgradeRequirement::Any),
        stacked(UpgradeRequirement::Any),
        Requirement {
            upgrade: UpgradeRequirement::AtLeast(1),
            ..Requirement::any(ItemKind::Wand)
        },
    ]
    .into_iter()
    .zip(1..)
    .map(|(requirement, key)| Row { key, requirement })
    .collect();
    let board = view(&rows);
    assert!(board.problems.is_empty());
    assert_eq!(board.items.len(), 2);
    let staff = chip(&board, 1);
    assert_eq!(
        (staff.name.as_str(), texts(&staff.tags)),
        ("Any wand", vec!["+3"])
    );
    assert_eq!(
        board.items[0]
            .badges
            .count
            .as_ref()
            .map(|badge| badge.text.as_str()),
        Some("×3")
    );
    assert_eq!(
        staff.relations,
        [relation(
            RelationGlyph::Times,
            "3 of the same kind — the extra copies: any upgrade, any floor"
        )]
    );
    assert_eq!(texts(&chip(&board, 4).tags), ["+1↑"]);
}

// --- the resin chip -------------------------------------------------------------------

#[test]
fn the_resin_chip_tags_the_amount_and_describes_the_donors() {
    assert_eq!(view(&[]).resin, None);
    let auto = ResinState {
        amount: ResinAmount::Auto,
        filter: ArcaneResinFilter::default(),
    };
    let board = board_view(&[], Some(&auto));
    assert_eq!(
        board.resin,
        Some(ResinChip {
            name: "Arcane Resin".to_owned(),
            tags: vec![Tag::plain("Auto")],
            uncursed: true,
            tooltip: None,
            amount_tooltip: Some(
                "Enough resin to upgrade kept wands to +3, excluding No resin wands and reforge \
                 copies"
                    .to_owned()
            ),
            details: vec!["Auto".to_owned(), "uncursed wands".to_owned()],
            description: "Arcane Resin, Auto, uncursed wands".to_owned(),
        })
    );

    let fixed = ResinState {
        amount: ResinAmount::AtLeast(12),
        filter: ArcaneResinFilter {
            include_mage_wand: true,
            uncursed: false,
            max_depth: Some(9),
            source: Some(ItemSource::LockedChest),
        },
    };
    let chip = resin_chip(&fixed);
    assert_eq!(texts(&chip.tags), ["≥12", "Mage +2", "F≤9"]);
    assert!(!chip.uncursed);
    assert_eq!(chip.tooltip.as_deref(), Some("Locked chest"));
    assert_eq!(chip.amount_tooltip, None);
    assert_eq!(
        chip.details,
        [
            "at least 12",
            "starting Magic Missile contributes 2 resin",
            "any wands",
            "Locked chest",
            "floors 1–9"
        ]
    );
    assert_eq!(
        chip.description,
        "Arcane Resin, at least 12, starting Magic Missile contributes 2 resin, any wands, \
         Locked chest, floors 1–9"
    );
    // Auto and a source: the source is the chip's hover text and Auto's
    // explanation the amount tag's, each on its own — a platform never
    // splits an English string to lay them out as the web does.
    let both = resin_chip(&ResinState {
        amount: ResinAmount::Auto,
        ..fixed
    });
    assert_eq!(both.tooltip.as_deref(), Some("Locked chest"));
    assert_eq!(texts(&both.tags)[0], "Auto");
    assert_eq!(
        both.amount_tooltip.as_deref(),
        Some(
            "Enough resin to upgrade kept wands to +3, excluding No resin wands and reforge \
             copies"
        )
    );
    let plain = resin_chip(&ResinState {
        amount: ResinAmount::AtLeast(3),
        filter: ArcaneResinFilter::default(),
    });
    assert_eq!((plain.tooltip, plain.amount_tooltip), (None, None));
}

// --- kinds ------------------------------------------------------------------------------

#[test]
fn every_kind_names_its_family_and_narrowing_both_ways() {
    let names: Vec<&str> = KindName::ALL.iter().map(|kind| kind.name()).collect();
    assert_eq!(
        names,
        [
            "weapon",
            "melee_weapon",
            "thrown_weapon",
            "armor",
            "wand",
            "ring",
            "trinket",
            "artifact"
        ]
    );
    let labels: Vec<&str> = KindName::ALL.iter().map(|kind| kind.label()).collect();
    assert_eq!(
        labels,
        [
            "Weapon",
            "Melee weapon",
            "Thrown weapon",
            "Armor",
            "Wand",
            "Ring",
            "Trinket",
            "Artifact"
        ]
    );
    for kind in KindName::ALL {
        assert_eq!(KindName::of(kind.family(), kind.weapon_category()), kind);
    }
    // A narrowing means nothing outside weapons.
    assert_eq!(
        KindName::of(ItemKind::Wand, Some(WeaponCategory::Melee)),
        KindName::Wand
    );
}

// --- the whole view ----------------------------------------------------------------------

#[test]
fn the_board_view_agrees_with_the_fold_the_problems_and_the_candidates() {
    // Generated lists, valid rows or not, as given or after a random edit
    // (1,024 cases).
    let mut rng = Rng::new(0xb0a2_d51e_face);
    for case in 0..1024 {
        let mut rows = mixed_rows(&mut rng);
        if case % 2 == 1 {
            let edit = random_edit(&mut rng, &rows);
            rows = apply(&rows, None, &[edit]).rows;
        }
        let board = view(&rows);
        let context = format!("case {case}: {rows:?}");
        let items = board_items(&rows);
        let candidates = join_candidates(&rows, &items);
        let found = problems(&rows);
        assert_eq!(board.problems, found, "{context}");
        // The board shows every row, so every problem naming a row shows on
        // the entry holding it.
        for key in found.iter().flat_map(|problem| &problem.keys) {
            assert!(
                board.items.iter().any(|view| view.problem.is_some()
                    && (view.members.contains(key) || view.extras.contains(key))),
                "{context}"
            );
        }
        assert_eq!(board.items.len(), items.len(), "{context}");
        assert_eq!(
            board.counts.ordinary + board.counts.blanket,
            items.len(),
            "{context}"
        );
        for (view, item) in board.items.iter().zip(&items) {
            let keys = |indices: &[usize]| -> Vec<u64> {
                indices.iter().map(|&index| rows[index].key).collect()
            };
            assert_eq!(view.members, keys(&item.members), "{context}");
            assert_eq!(view.extras, keys(&item.extras), "{context}");
            assert_eq!(view.cluster, item.cluster, "{context}");
            assert_eq!(view.label.is_some(), item.cluster.is_some(), "{context}");
            assert_eq!(view.stack, stack_view(&rows, item), "{context}");
            assert_eq!(view.badges.count.is_some(), item.count() > 1, "{context}");
            assert_eq!(
                view.badges.total.is_some(),
                item.total.is_some(),
                "{context}"
            );
            let touched = found.iter().any(|problem| {
                problem
                    .keys
                    .iter()
                    .any(|key| view.members.contains(key) || view.extras.contains(key))
            });
            assert_eq!(view.problem.is_some(), touched, "{context}");
            assert_eq!(view.chips.len(), item.members.len(), "{context}");
            for (chip, &index) in view.chips.iter().zip(&item.members) {
                assert_eq!(chip.key, rows[index].key, "{context}");
                assert_eq!(chip.join, candidates[index].join, "{context}");
                assert_eq!(chip.refuse, candidates[index].refuse, "{context}");
                assert!(chip.description.starts_with(&chip.title), "{context}");
                assert!(
                    chip.problem.is_none() || view.problem.is_some(),
                    "{context}"
                );
                assert_eq!(chip.in_cluster, item.cluster.is_some(), "{context}");
                // The chip's own row problem is the one it shows first.
                let own = found.iter().find(|problem| {
                    problem.scope == ProblemScope::Row && problem.keys == [chip.key]
                });
                if let Some(own) = own {
                    assert_eq!(chip.problem.as_ref(), Some(&own.message), "{context}");
                }
            }
        }
    }
}
