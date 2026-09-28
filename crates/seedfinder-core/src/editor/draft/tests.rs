//! The requirement sheet: opening, every change rule, what the form shows
//! per family and state, the item picker, the Arcane Resin flows, the save
//! guard and the shapes a save writes.
//!
//! These port what the web's `ArcaneResinEditor.test.tsx`,
//! `trinkets.test.tsx`, `artifacts.test.tsx`, `blankets.test.tsx` and
//! `impossible-query.test.tsx` pin of `RequirementEditor.tsx` and
//! `QueryPanel.tsx`;
//! Windows' `ItemCatalogTests` picker facts; Linux's picker counts and its
//! `validate_draft` cases (`state.rs`); and the shared design's settled
//! choices where they differ from the web — a resin filter seeded from the
//! query rather than the wand draft, at-least bounds of +1…max−1, floor
//! sliders stepping over empty boss floors, and a save guard.

use super::super::testing::{
    Rng, arbitrary_requirement, assert_emittable, mixed_rows, named, random_change, random_edit,
    random_requirement, random_resin, random_rows, row, validate, with,
};
use super::super::{
    DropAction, DropTarget, Edit, EditResult, NO_ORDINARY_REQUIREMENT, Refusal, apply, board_items,
    drop_action, problems, row_problems,
};
use super::*;
use crate::catalog::{WeaponEffect, item};
use crate::query::LevelSum;

// --- helpers -------------------------------------------------------------

/// A sheet on a new ordinary chip, on a platform that offers resin.
fn new_sheet(rows: &[Row]) -> Draft {
    open(rows, None, false, None, true, false)
}

/// A sheet on the row `key`.
fn sheet(rows: &[Row], key: u64) -> Draft {
    open(rows, Some(key), false, None, true, false)
}

/// `draft` after `changes`, in order.
fn after(draft: &Draft, changes: &[Change]) -> Draft {
    changes
        .iter()
        .fold(draft.clone(), |draft, step| change(&draft, step))
}

/// The save of `draft` onto the rows it was opened on, which must succeed.
fn saved(draft: &Draft) -> (EditResult, ResinOutcome) {
    match save(draft, &draft.rows, None) {
        SaveResult::Saved { result, resin } => (result, resin),
        SaveResult::Refused { form, .. } => panic!("refused: {:?}", form.errors),
    }
}

/// The saved rows' requirements.
fn stored(draft: &Draft) -> Vec<Requirement> {
    saved(draft)
        .0
        .rows
        .iter()
        .map(|row| row.requirement)
        .collect()
}

/// Why saving `draft` is refused; the refusal's form agrees.
fn refusal(draft: &Draft) -> Vec<String> {
    match save(draft, &draft.rows, None) {
        SaveResult::Saved { result, .. } => panic!("saved: {:?}", result.rows),
        SaveResult::Refused { form, .. } => {
            assert!(!form.can_save);
            assert_eq!(form.preview, None);
            form.errors
        }
    }
}

fn labels<T>(options: &[Opt<T>]) -> Vec<&str> {
    options.iter().map(|option| option.label.as_str()).collect()
}

fn choices(form: &Form) -> Vec<ItemChoice> {
    form.item
        .options
        .iter()
        .map(|option| option.value)
        .collect()
}

/// The picker of a fresh sheet on `kind`.
fn picker(kind: ItemKind, category: Option<WeaponCategory>) -> Vec<Opt<ItemChoice>> {
    let draft = after(
        &open(&[], None, false, None, false, false),
        &[Change::SetKind(kind, category)],
    );
    form(&draft).item.options
}

/// The picker of a sheet on a stored `requirement`.
fn picker_for(requirement: Requirement) -> Vec<Opt<ItemChoice>> {
    let rows = [Row {
        key: 1,
        requirement,
    }];
    form(&open(&rows, Some(1), false, None, false, false))
        .item
        .options
}

/// The requirement naming `item_id`.
fn named_requirement(item_id: ItemId) -> Requirement {
    named(0, item_id).requirement
}

/// A new chip saved with its stack, as the sheet's save writes it.
fn add(requirement: Requirement, count: u8, total: Option<u8>, copy_depth: Option<u8>) -> Edit {
    Edit::Save {
        key: None,
        requirement,
        count,
        total,
        copy_depth,
    }
}

/// The rows the editor writes for `edits` from an empty list.
fn written(edits: &[Edit]) -> Vec<Row> {
    let result = apply(&[], None, edits);
    assert_eq!(result.refused, None, "{edits:?}");
    result.rows
}

/// The number of items each board entry asks for.
fn counts(rows: &[Row]) -> Vec<usize> {
    board_items(rows).iter().map(BoardItem::count).collect()
}

fn keys(rows: &[Row]) -> Vec<u64> {
    rows.iter().map(|row| row.key).collect()
}

/// The names of the controls a form shows, in form order.
fn shown(form: &Form) -> Vec<&'static str> {
    [
        ("weapon_type", form.weapon_type.visible),
        ("tier", form.tier.visible),
        ("upgrade", form.upgrade.visible),
        ("effect", form.effect.visible),
        ("uncursed", form.uncursed.visible),
        ("source", form.source.visible),
        ("floor_limit", form.floor_limit.visible),
        ("exclude_resin", form.exclude_resin.visible),
        ("transmutations", form.transmutations.visible),
        ("select_trinket", form.select_trinket.visible),
        ("stack", form.stack.visible),
        ("copy_depth", form.stack.copy_depth.visible),
        ("count_levels", form.stack.count_levels.visible),
        ("resin", form.resin.visible),
    ]
    .into_iter()
    .filter_map(|(name, visible)| visible.then_some(name))
    .collect()
}

const MELEE: Option<WeaponCategory> = Some(WeaponCategory::Melee);
const THROWN: Option<WeaponCategory> = Some(WeaponCategory::Thrown);

fn weapon(effect: WeaponEffect) -> Effect {
    Effect::Weapon(effect)
}

// --- opening -------------------------------------------------------------

#[test]
fn a_new_sheet_starts_on_any_weapon_with_the_default_resin() {
    let draft = open(&[], None, false, None, false, false);
    assert_eq!(draft.v, DRAFT_VERSION);
    assert_eq!((draft.origin, draft.key), (Origin::New, None));
    assert_eq!(draft.requirement, Requirement::any(ItemKind::Weapon));
    assert_eq!(
        (draft.count, draft.total, draft.copy_depth),
        (1, None, None)
    );
    assert_eq!((draft.tier_value, draft.upgrade_value), (3, 1));
    assert_eq!(
        (
            draft.floor_limit_memory,
            draft.copy_depth_memory,
            draft.transmutations_memory
        ),
        (4, 4, 1)
    );
    assert!(!draft.blanket && !draft.in_cluster && !draft.resin_picked);
    assert_eq!(
        draft.resin,
        ResinDraft {
            auto: false,
            amount: Some(2.0),
            include_mage_wand: false,
            uncursed: true,
            max_depth: None,
            source: None,
        }
    );
    assert_eq!(draft.resin, ResinDraft::default());

    let form = form(&draft);
    assert_eq!(
        (form.v, form.mode, form.origin),
        (1, FormMode::New, Origin::New)
    );
    assert_eq!(form.category.value, ItemKind::Weapon);
    assert_eq!(
        labels(&form.category.options),
        ["Weapon", "Armor", "Wand", "Ring", "Trinket", "Artifact"]
    );
    assert_eq!(form.kind.value, KindName::Weapon);
    assert_eq!(
        labels(&form.kind.options),
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
    assert_eq!(
        labels(&form.weapon_type.options),
        ["Any", "Melee", "Thrown"]
    );
    assert_eq!(form.weapon_type.value, None);
    assert_eq!(form.item.value, ItemChoice::Any);
    assert_eq!(form.item.options[0].label, "Any weapon");
    assert_eq!(
        labels(&form.tier.modes),
        ["Any", "Exactly", "At least", "At most"]
    );
    assert_eq!(labels(&form.upgrade.modes), ["Any", "Exactly", "At least"]);
    assert_eq!(form.source.options.len(), 1 + ItemSource::ALL.len());
    assert_eq!(form.source.options[0].label, "Any");
    assert_eq!(form.source.options[3].label, "Locked chest");
    assert!(form.can_save && form.errors.is_empty());
    let preview = form.preview.expect("a valid draft previews its chip");
    assert_eq!(preview.key, 0);
    assert_eq!(
        (preview.name.as_str(), preview.title.as_str()),
        ("Any weapon", "Any weapon")
    );
    assert!(preview.join.is_empty() && preview.refuse.is_empty());
}

#[test]
fn a_new_blanket_takes_the_first_ordinary_kind_and_weapon_type() {
    let melee = with(row(2, ItemKind::Weapon), |r| r.weapon_category = MELEE);
    let rows = [
        with(row(1, ItemKind::Ring), |r| r.blanket = true),
        melee,
        row(3, ItemKind::Wand),
    ];
    let draft = open(&rows, None, true, None, true, false);
    assert!(draft.blanket);
    assert_eq!(
        draft.requirement,
        Requirement {
            weapon_category: MELEE,
            blanket: true,
            ..Requirement::any(ItemKind::Weapon)
        }
    );
    // Without an ordinary row, any weapon.
    assert_eq!(
        open(&rows[..1], None, true, None, false, false).requirement,
        Requirement {
            blanket: true,
            ..Requirement::any(ItemKind::Weapon)
        }
    );
    // A trinket first: the blanket names the first trinket, as every
    // trinket requirement must.
    let trinket_first = [named(1, ItemId::MimicTooth), row(2, ItemKind::Wand)];
    let draft = open(&trinket_first, None, true, None, false, false);
    assert_eq!(draft.requirement.kind, ItemKind::Trinket);
    assert_eq!(draft.requirement.item, Some(ItemId::RatSkull));
    // A blanket wand sheet offers no resin, no stack and no resin exclusion.
    let wand = form(&open(
        &[row(1, ItemKind::Wand)],
        None,
        true,
        None,
        true,
        false,
    ));
    assert!(!choices(&wand).contains(&ItemChoice::ArcaneResin));
    assert_eq!(
        shown(&wand),
        ["upgrade", "uncursed", "source", "floor_limit"]
    );
    assert!(wand.blanket);
}

/// Web `blankets.test.tsx`: an edited blanket stays a blanket beside an
/// identical ordinary chip, and blankets never stack.
#[test]
fn an_edited_blanket_stays_on_its_board() {
    let frost = named(1, ItemId::WandFrost);
    let blanket = |key| with(named(key, ItemId::WandFrost), |r| r.blanket = true);
    let rows = [frost, blanket(2), blanket(3)];
    assert_eq!(counts(&rows), [1, 1, 1]);
    let draft = sheet(&rows, 2);
    assert!(draft.blanket);
    let edited = stored(&after(
        &draft,
        &[Change::SetSource(Some(ItemSource::WandmakerReward))],
    ));
    assert!(edited[1].blanket);
    assert_eq!(edited[1].source, Some(ItemSource::WandmakerReward));
    assert_eq!(edited.len(), 3);
}

#[test]
fn an_existing_row_opens_with_the_stack_its_board_entry_shows() {
    let might = named_requirement(ItemId::RingMight);
    let spear = Requirement {
        upgrade: UpgradeRequirement::Exact(3),
        ..named_requirement(ItemId::Spear)
    };
    let rows = written(&[
        add(might, 3, Some(6), None),
        add(spear, 2, None, Some(6)),
        add(Requirement::any(ItemKind::Wand), 1, None, None),
        add(Requirement::any(ItemKind::Ring), 1, None, None),
        Edit::Join {
            source: 7,
            target: 6,
        },
    ]);
    assert_eq!(counts(&rows), [3, 2, 1]);

    let ring = sheet(&rows, 1);
    assert_eq!((ring.origin, ring.key), (Origin::Row(1), Some(1)));
    assert_eq!(ring.requirement, might);
    assert_eq!(
        (ring.count, ring.total, ring.copy_depth),
        (3, Some(6), None)
    );
    assert!(!ring.in_cluster);
    let form_ring = form(&ring);
    assert_eq!(form_ring.mode, FormMode::Edit);
    assert_eq!(
        shown(&form_ring),
        ["uncursed", "source", "floor_limit", "stack", "count_levels"]
    );
    let levels = &form_ring.stack.count_levels;
    assert!(levels.enabled);
    assert_eq!((levels.value, levels.min, levels.max), (6, 1, 11));
    assert_eq!(levels.value_label, "≥ 6 across up to 3");
    assert_eq!(levels.label, "Count levels together");

    let spears = sheet(&rows, 4);
    assert_eq!(spears.requirement, spear);
    assert_eq!(
        (spears.count, spears.total, spears.copy_depth),
        (2, None, Some(6))
    );
    let stack = form(&spears).stack;
    assert_eq!((stack.count, stack.value_label.as_str()), (2, "×2"));
    assert!(stack.copy_depth.visible && stack.copy_depth.enabled);
    assert_eq!(stack.copy_depth.value, 6);
    assert_eq!(stack.copy_depth.label, "Limit the extra copies to a floor");
    assert_eq!(stack.copy_depth.value_label, "Copies within first 6 floors");
    assert!(!stack.count_levels.visible);

    // A cluster member's stack is its cluster's: the sheet shows none.
    let member = sheet(&rows, 7);
    assert!(member.in_cluster);
    assert!(member.requirement.alternative_group.is_some());
    assert!(!form(&member).stack.visible);
    assert!(form(&member).in_cluster);

    // A hidden copy has no chip; its key opens the chip it belongs to.
    let copy = sheet(&rows, 2);
    assert_eq!(copy.origin, Origin::Row(1));
    assert_eq!(copy, ring);
}

/// Critic M16: the sliders run +1…max (exactly) and +1…max−1 (at least), so
/// "+0 or higher" and "+max or higher" used to be silently rewritten. They
/// open as what they mean. Saved untouched, the row keeps what it says;
/// saved with any change, it takes what the sheet showed.
#[test]
fn upgrade_bounds_the_sliders_cannot_hold_open_as_what_they_mean() {
    let tier_three = Requirement {
        tier: TierRequirement::Exact(3),
        ..Requirement::any(ItemKind::Weapon)
    };
    let cases = [
        (
            Requirement::any(ItemKind::Wand),
            UpgradeRequirement::AtLeast(0),
            UpgradeRequirement::Any,
        ),
        (
            Requirement::any(ItemKind::Wand),
            UpgradeRequirement::AtLeast(4),
            UpgradeRequirement::Exact(4),
        ),
        // Any weapon may be tier 4, which reaches +5.
        (
            Requirement::any(ItemKind::Weapon),
            UpgradeRequirement::AtLeast(5),
            UpgradeRequirement::Exact(5),
        ),
        (
            Requirement::any(ItemKind::Weapon),
            UpgradeRequirement::AtLeast(4),
            UpgradeRequirement::AtLeast(4),
        ),
        (
            tier_three,
            UpgradeRequirement::AtLeast(4),
            UpgradeRequirement::Exact(4),
        ),
        (
            named_requirement(ItemId::RingMight),
            UpgradeRequirement::AtLeast(3),
            UpgradeRequirement::AtLeast(3),
        ),
        (
            named_requirement(ItemId::RingMight),
            UpgradeRequirement::Exact(2),
            UpgradeRequirement::Exact(2),
        ),
    ];
    for (base, upgrade, opened) in cases {
        let requirement = Requirement { upgrade, ..base };
        assert_eq!(requirement.validate(), Ok(()), "{requirement:?}");
        let rows = [Row {
            key: 1,
            requirement,
        }];
        let draft = sheet(&rows, 1);
        assert_eq!(draft.requirement.upgrade, opened, "{requirement:?}");
        let slider = form(&draft).upgrade;
        assert!(slider.min <= slider.value && slider.value <= slider.max);
        let (result, _) = saved(&draft);
        assert_eq!(result.rows, rows, "{requirement:?}");
        assert!(!result.changed, "{requirement:?}");
        let (result, _) = saved(&after(&draft, &[Change::SetFloorLimitEnabled(true)]));
        assert_eq!(
            result.rows[0].requirement.upgrade, opened,
            "{requirement:?}"
        );
        assert!(result.changed, "{requirement:?}");
    }
}

/// Web `trinkets.test.tsx` and `artifacts.test.tsx`: both always name one
/// item; trinkets show no details, artifacts keep their floor limit, and
/// neither shows an upgrade or a stack.
#[test]
fn trinkets_and_artifacts_open_on_a_named_item_without_the_controls_they_never_use() {
    let legacy = Requirement {
        source: Some(ItemSource::LockedChest),
        max_depth: Some(2),
        ..Requirement::any(ItemKind::Trinket)
    };
    assert!(row_problems(&legacy).contains(&"Select a trinket.".to_owned()));
    let draft = sheet(
        &[Row {
            key: 1,
            requirement: legacy,
        }],
        1,
    );
    assert_eq!(
        draft.requirement,
        Requirement {
            item: Some(ItemId::RatSkull),
            ..legacy
        },
        "the first trinket, its placement filters kept though the sheet shows none"
    );
    let trinket = form(&draft);
    assert_eq!(shown(&trinket), ["transmutations", "select_trinket"]);
    assert_eq!(
        trinket.select_trinket.label,
        "Choose matching trinket at +3"
    );
    assert!(!choices(&trinket).contains(&ItemChoice::Any));
    assert_eq!(trinket.item.options.len(), 17);
    assert!(!trinket.transmutations.enabled);
    assert_eq!(trinket.transmutations.label, "Allow transmutations");

    // A transmuting trinket in a cluster (web: `At most 13`).
    let transmuting = Requirement {
        trinket_transmutations: 13,
        alternative_group: Some(1),
        ..named_requirement(ItemId::RatSkull)
    };
    let rows = [
        Row {
            key: 1,
            requirement: transmuting,
        },
        with(named(2, ItemId::MimicTooth), |r| {
            r.alternative_group = Some(1);
        }),
    ];
    let form_transmuting = form(&sheet(&rows, 1));
    let control = &form_transmuting.transmutations;
    assert!(control.enabled);
    assert_eq!((control.value, control.max), (13, 13));
    assert_eq!(control.value_label, "At most 13");
    assert!(
        control
            .caption
            .as_deref()
            .unwrap()
            .starts_with("Matches an initial offer or any of the next 13 trinkets.")
    );
    assert!(!form_transmuting.select_trinket.visible);

    let sandals = Requirement {
        upgrade: UpgradeRequirement::Exact(5),
        max_depth: Some(19),
        ..named_requirement(ItemId::SandalsOfNature)
    };
    assert_eq!(sandals.validate(), Ok(()));
    let draft = sheet(
        &[Row {
            key: 1,
            requirement: sandals,
        }],
        1,
    );
    assert_eq!(draft.requirement, sandals, "the vault's +5 is kept");
    let artifact = form(&draft);
    assert_eq!(
        shown(&artifact),
        ["uncursed", "source", "floor_limit", "transmutations"]
    );
    assert_eq!(
        (artifact.upgrade.mode, artifact.upgrade.value),
        (UpgradeMode::Exact, 5)
    );
    assert!(!choices(&artifact).contains(&ItemChoice::Any));
    assert_eq!(artifact.item.options.len(), 11);
    assert!(artifact.floor_limit.enabled);
    assert_eq!(artifact.floor_limit.value, 19);
    assert_eq!(artifact.transmutations.max, 10);
    assert!(
        artifact
            .transmutations
            .caption
            .as_deref()
            .unwrap()
            .starts_with("Includes natural finds")
    );

    // A wildcard artifact opens on the first one.
    let wildcard = [row(1, ItemKind::Artifact)];
    assert_eq!(
        sheet(&wildcard, 1).requirement.item,
        Some(ItemId::AlchemistsToolkit)
    );
}

/// The web's report: an artifact asking for the city vault's +5 lost its
/// upgrade on an untouched save, which answered `changed: true`. The query
/// format accepts an artifact upgrade (+1…+5) and no platform ever offered a
/// control for one, so the sheet keeps it hidden and keeps it — as it keeps
/// a trinket's source, floor limit and uncursed filter — through saves that
/// change something else. Only a category switch, which resets what the new
/// family does not share, lets them go.
#[test]
fn fields_the_sheet_does_not_show_survive_its_saves() {
    let sandals = Requirement {
        upgrade: UpgradeRequirement::Exact(5),
        max_depth: Some(19),
        ..named_requirement(ItemId::SandalsOfNature)
    };
    let rows = [Row {
        key: 1,
        requirement: sandals,
    }];
    let draft = sheet(&rows, 1);
    assert!(!form(&draft).upgrade.visible);
    let (result, _) = saved(&draft);
    assert_eq!(
        (result.changed, result.rows.clone()),
        (false, rows.to_vec())
    );
    assert_eq!(result.focus, Some(1));
    let transmuting = after(&draft, &[Change::SetTransmutationsEnabled(true)]);
    assert_eq!(
        stored(&transmuting),
        [Requirement {
            artifact_transmutations: 1,
            ..sandals
        }]
    );
    // Another artifact keeps it; changes to the hidden control change nothing.
    let other = after(
        &draft,
        &[
            Change::SetItem(ItemChoice::Item(ItemId::HornOfPlenty)),
            Change::SetUpgradeMode(UpgradeMode::Any),
            Change::SetUpgrade(2),
        ],
    );
    assert_eq!(other.requirement.upgrade, UpgradeRequirement::Exact(5));
    let ring = after(&draft, &[Change::SetCategory(ItemKind::Ring)]);
    assert_eq!(ring.requirement.upgrade, UpgradeRequirement::Any);

    let trinket = Requirement {
        source: Some(ItemSource::LockedChest),
        max_depth: Some(9),
        require_uncursed: true,
        ..named_requirement(ItemId::RatSkull)
    };
    assert_eq!(trinket.validate(), Ok(()));
    let rows = [Row {
        key: 1,
        requirement: trinket,
    }];
    let draft = sheet(&rows, 1);
    assert_eq!(shown(&form(&draft)), ["transmutations", "select_trinket"]);
    let chosen = after(
        &draft,
        &[
            Change::SetSelectTrinket(true),
            Change::SetSource(None),
            Change::SetFloorLimitEnabled(false),
            Change::SetUncursed(false),
        ],
    );
    assert_eq!(
        stored(&chosen),
        [Requirement {
            select_trinket: true,
            ..trinket
        }]
    );
    let wand = after(
        &draft,
        &[
            Change::SetCategory(ItemKind::Wand),
            Change::SetCategory(ItemKind::Trinket),
        ],
    );
    assert_eq!(
        (wand.requirement.source, wand.requirement.max_depth),
        (None, None)
    );
}

#[test]
fn a_key_not_in_the_list_opens_a_new_chip_that_saves_under_it() {
    // Linux claims a key before the sheet opens.
    let rows = [row(1, ItemKind::Wand)];
    let draft = open(&rows, Some(40), false, None, false, false);
    assert_eq!((draft.origin, draft.key), (Origin::New, Some(40)));
    assert_eq!(form(&draft).mode, FormMode::New);
    let (result, resin) = saved(&draft);
    assert_eq!(keys(&result.rows), [1, 40]);
    assert_eq!((result.next_key, result.focus), (41, Some(40)));
    assert_eq!(resin, ResinOutcome::Unchanged);
}

/// A hand-written list can put a row in a stack and a combined level at
/// once. The board still shows every row, so every key opens a chip: a
/// combined level's copy opens its anchor, which is no longer folded away.
#[test]
fn every_row_of_a_hand_written_list_opens_its_chip() {
    let sum = |total| {
        Some(LevelSum {
            group: 3,
            minimum_total: total,
        })
    };
    let rows = [
        with(row(1, ItemKind::Ring), |r| {
            r.upgrade = UpgradeRequirement::Exact(4);
            r.identity_group = Some(3);
        }),
        with(row(2, ItemKind::Ring), |r| {
            r.identity_group = Some(3);
            r.level_sum = sum(10);
        }),
        with(row(3, ItemKind::Ring), |r| r.identity_group = Some(3)),
        with(named(4, ItemId::RingElements), |r| r.level_sum = sum(6)),
    ];
    for (key, opens) in [(1, 1), (2, 2), (3, 1), (4, 2)] {
        let draft = open(&rows, Some(key), false, None, false, false);
        assert_eq!(
            (draft.origin, draft.key),
            (Origin::Row(opens), Some(opens)),
            "{key}"
        );
    }
}

/// The sheet saves onto the list as it is now. When the row it was opened
/// on has since become a hidden copy of another chip, the save adds a chip
/// rather than vanishing into the copy and reporting success.
#[test]
fn a_save_onto_a_row_since_folded_away_adds_a_chip() {
    let opened = [
        named(1, ItemId::Spear),
        with(named(2, ItemId::Mace), |r| {
            r.upgrade = UpgradeRequirement::Exact(2);
        }),
    ];
    let draft = after(&sheet(&opened, 2), &[Change::SetUpgrade(3)]);
    assert!(form(&draft).can_save);
    let now = [named(1, ItemId::Spear), named(2, ItemId::Spear)];
    assert_eq!(counts(&now), [2]);
    let SaveResult::Saved { result, .. } = save(&draft, &now, None) else {
        panic!("the save goes through");
    };
    assert!(result.changed);
    assert_eq!(keys(&result.rows), [1, 2, 3]);
    assert_eq!(result.rows[..2], now);
    assert_eq!(result.rows[2].requirement.item, Some(ItemId::Mace));
    assert_eq!(
        result.rows[2].requirement.upgrade,
        UpgradeRequirement::Exact(3)
    );
    assert_eq!(result.focus, Some(3));
}

// --- changes -------------------------------------------------------------

#[test]
fn re_picking_the_family_keeps_the_draft() {
    let draft = after(
        &new_sheet(&[]),
        &[
            Change::SetWeaponType(MELEE),
            Change::SetItem(ItemChoice::Item(ItemId::Spear)),
            Change::SetUpgradeMode(UpgradeMode::Exact),
            Change::SetEffectMode(EffectMode::AnyEnchantment),
        ],
    );
    assert_eq!(
        after(&draft, &[Change::SetCategory(ItemKind::Weapon)]),
        draft
    );
    assert_eq!(
        after(&draft, &[Change::SetKind(ItemKind::Weapon, MELEE)]),
        draft
    );
}

/// Web `setKind`, and `ArcaneResinEditor.test.tsx`: a wand's resin
/// exclusion leaves with the wand.
#[test]
fn switching_the_family_resets_what_does_not_carry_over() {
    let wand = after(
        &new_sheet(&[]),
        &[
            Change::SetCategory(ItemKind::Wand),
            Change::SetItem(ItemChoice::Item(ItemId::WandLightning)),
            Change::SetExcludeResin(true),
            Change::SetUncursed(true),
            Change::SetSource(Some(ItemSource::Shop)),
            Change::SetFloorLimitEnabled(true),
            Change::SetUpgradeMode(UpgradeMode::Exact),
            Change::SetUpgrade(3),
            Change::SetCount(3),
            Change::SetCopyDepthEnabled(true),
        ],
    );
    assert!(stored(&wand)[0].exclude_resin);
    assert!(form(&wand).exclude_resin.visible);

    let ring = after(&wand, &[Change::SetCategory(ItemKind::Ring)]);
    assert_eq!(
        ring.requirement,
        Requirement {
            upgrade: UpgradeRequirement::Exact(3),
            require_uncursed: true,
            source: Some(ItemSource::Shop),
            max_depth: Some(4),
            ..Requirement::any(ItemKind::Ring)
        }
    );
    assert!(!form(&ring).exclude_resin.visible);
    assert_eq!((ring.count, ring.copy_depth), (3, Some(4)));

    let artifact = after(&wand, &[Change::SetCategory(ItemKind::Artifact)]);
    assert_eq!(
        artifact.requirement,
        Requirement {
            require_uncursed: true,
            source: Some(ItemSource::Shop),
            max_depth: Some(4),
            ..named_requirement(ItemId::AlchemistsToolkit)
        }
    );
    assert_eq!(
        (artifact.count, artifact.total, artifact.copy_depth),
        (1, None, None)
    );

    let trinket = after(&ring, &[Change::SetCategory(ItemKind::Trinket)]);
    assert_eq!(trinket.requirement, named_requirement(ItemId::RatSkull));
    assert_eq!(
        (trinket.count, trinket.total, trinket.copy_depth),
        (1, None, None)
    );

    let weapon = after(
        &trinket,
        &[
            Change::SetSelectTrinket(true),
            Change::SetCategory(ItemKind::Weapon),
        ],
    );
    assert_eq!(weapon.requirement, Requirement::any(ItemKind::Weapon));
    assert_eq!(weapon.upgrade_value, 1);
}

#[test]
fn the_weapon_type_keeps_the_item_only_if_it_belongs() {
    let spear = after(
        &new_sheet(&[]),
        &[Change::SetItem(ItemChoice::Item(ItemId::Spear))],
    );
    assert_eq!(
        after(&spear, &[Change::SetWeaponType(THROWN)]).requirement,
        Requirement {
            weapon_category: THROWN,
            ..Requirement::any(ItemKind::Weapon)
        }
    );
    let melee = after(&spear, &[Change::SetWeaponType(MELEE)]);
    assert_eq!(melee.requirement.item, Some(ItemId::Spear));
    assert_eq!(form(&melee).kind.value, KindName::MeleeWeapon);
    assert_eq!(
        after(&melee, &[Change::SetWeaponType(None)])
            .requirement
            .item,
        Some(ItemId::Spear)
    );

    // The flat kind picker narrows in one step.
    let thrown = after(
        &new_sheet(&[]),
        &[
            Change::SetCategory(ItemKind::Wand),
            Change::SetKind(ItemKind::Weapon, THROWN),
        ],
    );
    assert_eq!(form(&thrown).kind.value, KindName::ThrownWeapon);
    assert_eq!(form(&thrown).item.options[0].label, "Any thrown weapon");

    // Only weapons have a type.
    let wand = after(&new_sheet(&[]), &[Change::SetCategory(ItemKind::Wand)]);
    assert!(!form(&wand).weapon_type.visible);
    assert_eq!(after(&wand, &[Change::SetWeaponType(MELEE)]), wand);
}

#[test]
fn naming_an_item_drops_the_tier_and_the_wildcard_drops_the_combined_level() {
    let tiered = after(
        &new_sheet(&[]),
        &[Change::SetTierMode(TierMode::Exact), Change::SetTier(4)],
    );
    assert_eq!(tiered.requirement.tier, TierRequirement::Exact(4));
    let spear = after(&tiered, &[Change::SetItem(ItemChoice::Item(ItemId::Spear))]);
    assert_eq!(spear.requirement.tier, TierRequirement::Any);
    assert!(!form(&spear).tier.visible);
    // The tier control remembers its value for the wildcard.
    let wildcard = after(
        &spear,
        &[
            Change::SetItem(ItemChoice::Any),
            Change::SetTierMode(TierMode::Exact),
        ],
    );
    assert_eq!(wildcard.requirement.tier, TierRequirement::Exact(4));

    // Items of another family or weapon type are not the picker's to offer.
    assert_eq!(
        after(
            &spear,
            &[Change::SetItem(ItemChoice::Item(ItemId::WandFrost))]
        ),
        spear
    );
    let melee = after(&new_sheet(&[]), &[Change::SetWeaponType(MELEE)]);
    assert_eq!(
        after(
            &melee,
            &[Change::SetItem(ItemChoice::Item(ItemId::ThrowingKnife))]
        ),
        melee
    );
    // Trinkets and artifacts have no wildcard.
    let trinket = after(&new_sheet(&[]), &[Change::SetCategory(ItemKind::Trinket)]);
    assert_eq!(
        after(&trinket, &[Change::SetItem(ItemChoice::Any)]),
        trinket
    );

    let ring = after(
        &new_sheet(&[]),
        &[
            Change::SetCategory(ItemKind::Ring),
            Change::SetItem(ItemChoice::Item(ItemId::RingMight)),
            Change::SetCount(2),
            Change::SetCountLevels(true),
        ],
    );
    assert_eq!(ring.total, Some(2));
    assert_eq!(
        after(&ring, &[Change::SetItem(ItemChoice::Any)]).total,
        None
    );
    // Another ring keeps it.
    assert_eq!(
        after(
            &ring,
            &[Change::SetItem(ItemChoice::Item(ItemId::RingHaste))]
        )
        .total,
        Some(2)
    );
}

#[test]
fn tier_modes_clamp_the_remembered_tier_to_their_range() {
    let draft = new_sheet(&[]);
    let top = after(
        &draft,
        &[Change::SetTierMode(TierMode::Exact), Change::SetTier(5)],
    );
    assert_eq!(top.requirement.tier, TierRequirement::Exact(5));
    assert_eq!(form(&top).tier.value_label, "Tier 5");
    let bound = after(&top, &[Change::SetTierMode(TierMode::AtLeast)]);
    assert_eq!(bound.requirement.tier, TierRequirement::AtLeast(4));
    let control = form(&bound).tier;
    assert_eq!((control.min, control.max), (3, 4));
    assert_eq!(control.value_label, "Tier 4 or higher");
    let exact = after(&bound, &[Change::SetTierMode(TierMode::Exact)]);
    assert_eq!(exact.requirement.tier, TierRequirement::Exact(4));
    assert_eq!(
        after(&exact, &[Change::SetTier(9)]).requirement.tier,
        TierRequirement::Exact(5)
    );
    assert_eq!(
        after(&exact, &[Change::SetTier(0)]).requirement.tier,
        TierRequirement::Exact(2)
    );
    let lower = after(
        &exact,
        &[Change::SetTier(2), Change::SetTierMode(TierMode::AtMost)],
    );
    assert_eq!(lower.requirement.tier, TierRequirement::AtMost(3));
    assert_eq!(form(&lower).tier.value_label, "Tier 3 or lower");
    let any = after(&lower, &[Change::SetTierMode(TierMode::Any)]);
    assert_eq!(any.requirement.tier, TierRequirement::Any);
    let control = form(&any).tier;
    assert_eq!(
        (control.mode, control.value, control.min, control.max),
        (TierMode::Any, 3, 2, 5)
    );
    // The value slider is hidden under "any" and does not move.
    assert_eq!(after(&any, &[Change::SetTier(5)]), any);

    // Armor has tiers too; named items and other families do not.
    let armor = after(
        &draft,
        &[
            Change::SetCategory(ItemKind::Armor),
            Change::SetTierMode(TierMode::AtLeast),
        ],
    );
    assert_eq!(armor.requirement.tier, TierRequirement::AtLeast(3));
    let wand = after(&draft, &[Change::SetCategory(ItemKind::Wand)]);
    assert_eq!(after(&wand, &[Change::SetTierMode(TierMode::Exact)]), wand);
}

#[test]
fn the_upgrade_follows_what_the_item_and_tier_can_reach() {
    let plus_five = after(
        &new_sheet(&[]),
        &[
            Change::SetUpgradeMode(UpgradeMode::Exact),
            Change::SetUpgrade(5),
        ],
    );
    assert_eq!(plus_five.requirement.upgrade, UpgradeRequirement::Exact(5));
    assert_eq!(form(&plus_five).upgrade.max, 5);
    assert_eq!(form(&plus_five).upgrade.value_label, "+5");
    // Only a tier-4 weapon reaches +5.
    for (changes, upgrade) in [
        (
            vec![Change::SetTierMode(TierMode::Exact), Change::SetTier(5)],
            4,
        ),
        (vec![Change::SetTierMode(TierMode::AtLeast)], 5),
        (vec![Change::SetTierMode(TierMode::AtMost)], 4),
        (vec![Change::SetItem(ItemChoice::Item(ItemId::Spear))], 4),
        (
            vec![Change::SetItem(ItemChoice::Item(ItemId::Longsword))],
            5,
        ),
        (vec![Change::SetCategory(ItemKind::Armor)], 4),
    ] {
        assert_eq!(
            after(&plus_five, &changes).requirement.upgrade,
            UpgradeRequirement::Exact(upgrade),
            "{changes:?}"
        );
    }
    // "At least" runs +1…max−1: "+max or higher" is "exactly +max".
    let at_least = after(&plus_five, &[Change::SetUpgradeMode(UpgradeMode::AtLeast)]);
    assert_eq!(at_least.requirement.upgrade, UpgradeRequirement::AtLeast(4));
    let control = form(&at_least).upgrade;
    assert_eq!((control.min, control.max), (1, 4));
    assert_eq!(control.value_label, "+4 or higher");
    assert_eq!(
        after(&at_least, &[Change::SetUpgrade(0)])
            .requirement
            .upgrade,
        UpgradeRequirement::AtLeast(1)
    );
    // "Any" remembers the value.
    let any = after(&at_least, &[Change::SetUpgradeMode(UpgradeMode::Any)]);
    assert_eq!(any.requirement.upgrade, UpgradeRequirement::Any);
    assert_eq!(form(&any).upgrade.value, 4);
    assert_eq!(
        after(&any, &[Change::SetUpgradeMode(UpgradeMode::Exact)])
            .requirement
            .upgrade,
        UpgradeRequirement::Exact(4)
    );
    assert_eq!(after(&any, &[Change::SetUpgrade(2)]), any);

    // Trinkets and artifacts are never searched by upgrade, and a counting
    // stack's total speaks for its upgrades.
    let trinket = after(&new_sheet(&[]), &[Change::SetCategory(ItemKind::Trinket)]);
    assert_eq!(
        after(&trinket, &[Change::SetUpgradeMode(UpgradeMode::Exact)]),
        trinket
    );
    let counting = after(
        &new_sheet(&[]),
        &[
            Change::SetCategory(ItemKind::Ring),
            Change::SetItem(ItemChoice::Item(ItemId::RingMight)),
            Change::SetCount(2),
            Change::SetCountLevels(true),
        ],
    );
    assert!(!form(&counting).upgrade.visible);
    assert_eq!(
        after(&counting, &[Change::SetUpgradeMode(UpgradeMode::Exact)]),
        counting
    );
}

/// Web `RequirementEditor`: Any clears the filter, Any enchantment takes
/// the whole non-curse set, Specific… starts empty from it (and saves as
/// any while nothing is ticked), and ticking every enchantment is "any
/// enchantment" again.
#[test]
fn the_effect_mode_moves_between_any_any_enchantment_and_specific() {
    let draft = new_sheet(&[]);
    let enchantments = EffectSet::enchantments(ItemKind::Weapon).unwrap();
    let any_enchantment = after(&draft, &[Change::SetEffectMode(EffectMode::AnyEnchantment)]);
    assert_eq!(
        any_enchantment.requirement.effect,
        EffectRequirement::OneOf(enchantments)
    );
    let control = form(&any_enchantment).effect;
    assert_eq!(control.mode, EffectMode::AnyEnchantment);
    assert_eq!(
        labels(&control.modes),
        ["Any", "Any enchantment", "Specific…"]
    );
    assert_eq!(labels(&control.groups), ["Enchantments", "Curses"]);
    assert_eq!(control.choices.len(), 27);

    let specific = after(
        &any_enchantment,
        &[Change::SetEffectMode(EffectMode::Specific)],
    );
    assert_eq!(specific.requirement.effect, EffectRequirement::Any);
    let control = form(&specific).effect;
    assert_eq!(control.mode, EffectMode::Specific);
    assert_eq!(
        control.caption,
        "Tick the effects the item may carry; none ticked means any."
    );
    assert!(control.choices.iter().all(|choice| !choice.selected));
    assert!(form(&specific).can_save);
    assert_eq!(stored(&specific)[0].effect, EffectRequirement::Any);

    let one = after(
        &specific,
        &[Change::ToggleEffect(weapon(WeaponEffect::Blazing))],
    );
    assert_eq!(
        one.requirement.effect,
        EffectRequirement::exactly(weapon(WeaponEffect::Blazing))
    );
    assert_eq!(form(&one).effect.caption, "Matches any one of 1 effect.");
    let two = after(&one, &[Change::ToggleEffect(weapon(WeaponEffect::Grim))]);
    let control = form(&two).effect;
    assert_eq!(control.caption, "Matches any one of 2 effects.");
    assert_eq!(
        control
            .choices
            .iter()
            .filter(|choice| choice.selected)
            .map(|choice| choice.label.as_str())
            .collect::<Vec<_>>(),
        ["Blazing", "Grim"]
    );
    assert_eq!(
        after(&two, &[Change::ToggleEffect(weapon(WeaponEffect::Blazing))])
            .requirement
            .effect,
        EffectRequirement::exactly(weapon(WeaponEffect::Grim))
    );
    // Specific… keeps a set being built.
    assert_eq!(
        after(&two, &[Change::SetEffectMode(EffectMode::Specific)]),
        two
    );
    // Every enchantment ticked is "any enchantment".
    let all = enchantments
        .effects()
        .fold(specific.clone(), |draft, effect| {
            change(&draft, &Change::ToggleEffect(effect))
        });
    assert_eq!(all.effect_mode, EffectMode::AnyEnchantment);
    assert_eq!(
        all.requirement.effect,
        EffectRequirement::OneOf(enchantments)
    );
    // Any clears.
    let cleared = after(&two, &[Change::SetEffectMode(EffectMode::Any)]);
    assert_eq!(cleared.requirement.effect, EffectRequirement::Any);
    assert_eq!(form(&cleared).effect.mode, EffectMode::Any);

    // The grid answers only in Specific…, and only for the family's effects.
    assert_eq!(
        after(
            &draft,
            &[Change::ToggleEffect(weapon(WeaponEffect::Blazing))]
        ),
        draft
    );
    let stone = Effect::Armor(crate::catalog::ArmorEffect::Stone);
    assert_eq!(after(&specific, &[Change::ToggleEffect(stone)]), specific);

    let armor = form(&after(&draft, &[Change::SetCategory(ItemKind::Armor)])).effect;
    assert_eq!(labels(&armor.modes), ["Any", "Any glyph", "Specific…"]);
    assert_eq!(labels(&armor.groups), ["Glyphs", "Curses"]);
    assert!(
        armor
            .choices
            .iter()
            .any(|choice| choice.label == "Anti-Magic")
    );
    let wand = after(&draft, &[Change::SetCategory(ItemKind::Wand)]);
    assert!(!form(&wand).effect.visible);
    assert_eq!(
        after(&wand, &[Change::SetEffectMode(EffectMode::AnyEnchantment)]),
        wand
    );
}

#[test]
fn requiring_uncursed_drops_curses_from_the_selection_and_the_grid() {
    let specific = after(
        &new_sheet(&[]),
        &[
            Change::SetEffectMode(EffectMode::Specific),
            Change::ToggleEffect(weapon(WeaponEffect::Blazing)),
            Change::ToggleEffect(weapon(WeaponEffect::Wayward)),
        ],
    );
    assert!(
        form(&specific)
            .effect
            .choices
            .iter()
            .any(|choice| choice.group == EffectGroup::Curse)
    );
    let uncursed = after(&specific, &[Change::SetUncursed(true)]);
    assert_eq!(
        uncursed.requirement.effect,
        EffectRequirement::exactly(weapon(WeaponEffect::Blazing))
    );
    let control = form(&uncursed).effect;
    assert!(
        control
            .choices
            .iter()
            .all(|choice| choice.group == EffectGroup::Enchantment)
    );
    assert_eq!(labels(&control.groups), ["Enchantments"]);
    assert_eq!(
        after(
            &uncursed,
            &[Change::ToggleEffect(weapon(WeaponEffect::Wayward))]
        ),
        uncursed
    );
    // A selection of curses only empties, and stays Specific….
    let curses = after(
        &new_sheet(&[]),
        &[
            Change::SetEffectMode(EffectMode::Specific),
            Change::ToggleEffect(weapon(WeaponEffect::Wayward)),
            Change::SetUncursed(true),
        ],
    );
    assert_eq!(curses.requirement.effect, EffectRequirement::Any);
    assert_eq!(form(&curses).effect.mode, EffectMode::Specific);
    assert!(form(&curses).can_save);
}

/// Linux's `floor_limit_skip_target`, now [`skip_boss_floor`], on every
/// floor slider of the sheet.
#[test]
fn floor_sliders_step_over_empty_boss_floors() {
    let draft = after(&new_sheet(&[]), &[Change::SetFloorLimitEnabled(true)]);
    assert_eq!(draft.requirement.max_depth, Some(4));
    let control = form(&draft).floor_limit;
    assert!(control.enabled);
    assert_eq!(control.label, "Limit this item to a floor");
    assert_eq!(control.value_label, "Within first 4 floors");
    let floors: Vec<u8> = control.options.iter().map(|option| option.value).collect();
    assert_eq!(floors.len(), 21);
    assert!(![5, 10, 15].iter().any(|floor| floors.contains(floor)));
    assert!(floors.contains(&20) && floors.contains(&24));
    for (from, to, lands) in [
        (4, 5, 6),
        (6, 5, 4),
        (9, 10, 11),
        (11, 10, 9),
        (14, 15, 16),
        (4, 10, 9),
        (24, 15, 14),
        (4, 0, 1),
        (4, 30, 24),
        (19, 20, 20),
    ] {
        let at = after(&draft, &[Change::SetFloorLimit(from)]);
        assert_eq!(at.requirement.max_depth, Some(from));
        let moved = after(&at, &[Change::SetFloorLimit(to)]);
        assert_eq!(moved.requirement.max_depth, Some(lands), "{from} → {to}");
    }
    assert_eq!(
        form(&after(&draft, &[Change::SetFloorLimit(1)]))
            .floor_limit
            .value_label,
        "Within first 1 floor"
    );
    // Switched off, the slider keeps its floor and does not move.
    let off = after(
        &draft,
        &[
            Change::SetFloorLimit(11),
            Change::SetFloorLimitEnabled(false),
        ],
    );
    assert_eq!(off.requirement.max_depth, None);
    assert_eq!(form(&off).floor_limit.value, 11);
    assert!(!form(&off).floor_limit.enabled);
    assert_eq!(after(&off, &[Change::SetFloorLimit(2)]), off);
    assert_eq!(
        after(&off, &[Change::SetFloorLimitEnabled(true)])
            .requirement
            .max_depth,
        Some(11)
    );

    // The copies' floor and the resin donors' floor step the same way.
    let copies = after(
        &new_sheet(&[]),
        &[
            Change::SetCount(2),
            Change::SetCopyDepthEnabled(true),
            Change::SetCopyDepth(5),
        ],
    );
    assert_eq!(copies.copy_depth, Some(6));
    let resin = after(
        &new_sheet(&[]),
        &[
            Change::SetCategory(ItemKind::Wand),
            Change::SetItem(ItemChoice::ArcaneResin),
            Change::SetFloorLimitEnabled(true),
            Change::SetFloorLimit(5),
        ],
    );
    assert_eq!(resin.resin.max_depth, Some(6));
    assert_eq!(resin.requirement.max_depth, None);

    // A stored boss floor opens as the floor below, which it means anyway.
    let boss = [with(row(1, ItemKind::Wand), |r| r.max_depth = Some(10))];
    assert_eq!(sheet(&boss, 1).requirement.max_depth, Some(9));
}

#[test]
fn transmutations_switch_on_at_the_remembered_count_and_rule_out_choosing() {
    let trinket = after(
        &new_sheet(&[]),
        &[
            Change::SetCategory(ItemKind::Trinket),
            Change::SetSelectTrinket(true),
        ],
    );
    assert!(trinket.requirement.select_trinket);
    let on = after(&trinket, &[Change::SetTransmutationsEnabled(true)]);
    assert_eq!(on.requirement.trinket_transmutations, 1);
    assert!(!on.requirement.select_trinket);
    let control = form(&on);
    assert!(!control.select_trinket.visible);
    assert_eq!(control.transmutations.value_label, "At most 1");
    assert!(
        control
            .transmutations
            .caption
            .as_deref()
            .unwrap()
            .starts_with("Matches an initial offer or any of the next 1 trinket.")
    );
    assert_eq!(after(&on, &[Change::SetSelectTrinket(true)]), on);
    let most = after(&on, &[Change::SetTransmutations(20)]);
    assert_eq!(most.requirement.trinket_transmutations, 13);
    let off = after(&most, &[Change::SetTransmutationsEnabled(false)]);
    assert_eq!(off.requirement.trinket_transmutations, 0);
    assert_eq!(form(&off).transmutations.value, 13);
    assert!(form(&off).select_trinket.visible);
    assert_eq!(after(&off, &[Change::SetTransmutations(3)]), off);
    assert_eq!(
        after(&off, &[Change::SetTransmutationsEnabled(true)])
            .requirement
            .trinket_transmutations,
        13
    );

    let artifact = after(
        &new_sheet(&[]),
        &[
            Change::SetCategory(ItemKind::Artifact),
            Change::SetTransmutationsEnabled(true),
            Change::SetTransmutations(12),
        ],
    );
    assert_eq!(artifact.requirement.artifact_transmutations, 10);
    assert_eq!(artifact.requirement.trinket_transmutations, 0);
    assert_eq!(stored(&artifact)[0].artifact_transmutations, 10);

    let wand = after(&new_sheet(&[]), &[Change::SetCategory(ItemKind::Wand)]);
    assert_eq!(
        after(&wand, &[Change::SetTransmutationsEnabled(true)]),
        wand
    );
}

/// Web: a count below two clears the total and another clamps it; counting
/// starts at one level per item. Linux `validate_draft`: two rings reach
/// eight levels together and three eleven — the sheet cannot go past them,
/// and what it allows passes the engine's check.
#[test]
fn the_count_clears_or_clamps_the_combined_level() {
    let ring = after(
        &new_sheet(&[]),
        &[
            Change::SetCategory(ItemKind::Ring),
            Change::SetItem(ItemChoice::Item(ItemId::RingMight)),
        ],
    );
    assert_eq!(
        shown(&form(&ring)),
        ["upgrade", "uncursed", "source", "floor_limit", "stack"]
    );
    assert_eq!(form(&ring).stack.value_label, "×1");
    // One ring counts nothing.
    assert_eq!(after(&ring, &[Change::SetCountLevels(true)]), ring);

    let two = after(&ring, &[Change::SetCount(2)]);
    let levels = form(&two).stack.count_levels;
    assert!(levels.visible && !levels.enabled);
    assert_eq!((levels.value, levels.max), (2, 8));
    assert_eq!(after(&two, &[Change::SetTotal(5)]), two);

    let counting = after(&two, &[Change::SetCountLevels(true)]);
    assert_eq!(counting.total, Some(2));
    assert_eq!(
        shown(&form(&counting)),
        ["uncursed", "source", "floor_limit", "stack", "count_levels"]
    );
    assert_eq!(
        form(&counting).stack.count_levels.value_label,
        "≥ 2 across up to 2"
    );
    assert_eq!(after(&counting, &[Change::SetTotal(9)]).total, Some(8));
    assert_eq!(after(&counting, &[Change::SetTotal(0)]).total, Some(1));
    let three = after(&counting, &[Change::SetCount(3), Change::SetTotal(12)]);
    assert_eq!(three.total, Some(11));
    assert_eq!(form(&three).stack.count_levels.max, 11);
    assert_eq!(after(&three, &[Change::SetCount(2)]).total, Some(8));
    assert_eq!(after(&three, &[Change::SetCount(1)]).total, None);
    assert_eq!(after(&ring, &[Change::SetCount(9)]).count, 3);
    assert_eq!(after(&two, &[Change::SetCount(0)]).count, 1);

    for (count, total) in [(2, 8), (3, 11)] {
        let draft = after(
            &ring,
            &[
                Change::SetCount(count),
                Change::SetCountLevels(true),
                Change::SetTotal(total),
            ],
        );
        let (result, _) = saved(&draft);
        assert_eq!(validate(&result.rows), Ok(()));
        assert_eq!(result.rows.len(), usize::from(count));
        assert!(
            result.rows.iter().all(|row| {
                row.requirement.level_sum.map(|sum| sum.minimum_total) == Some(total)
            })
        );
    }

    // Only a named ring stack counts levels.
    for kind in [ItemKind::Ring, ItemKind::Wand] {
        let wildcard = after(
            &new_sheet(&[]),
            &[Change::SetCategory(kind), Change::SetCount(2)],
        );
        assert!(!form(&wildcard).stack.count_levels.visible);
        assert_eq!(after(&wildcard, &[Change::SetCountLevels(true)]), wildcard);
    }
}

// --- the form ------------------------------------------------------------

#[test]
fn each_family_shows_its_own_controls() {
    let expected: [(ItemKind, &[&str]); 6] = [
        (
            ItemKind::Weapon,
            &[
                "weapon_type",
                "tier",
                "upgrade",
                "effect",
                "uncursed",
                "source",
                "floor_limit",
                "stack",
            ],
        ),
        (
            ItemKind::Armor,
            &[
                "tier",
                "upgrade",
                "effect",
                "uncursed",
                "source",
                "floor_limit",
                "stack",
            ],
        ),
        (
            ItemKind::Wand,
            &[
                "upgrade",
                "uncursed",
                "source",
                "floor_limit",
                "exclude_resin",
                "stack",
            ],
        ),
        (
            ItemKind::Ring,
            &["upgrade", "uncursed", "source", "floor_limit", "stack"],
        ),
        (ItemKind::Trinket, &["transmutations", "select_trinket"]),
        (
            ItemKind::Artifact,
            &["uncursed", "source", "floor_limit", "transmutations"],
        ),
    ];
    for (kind, controls) in expected {
        let draft = after(&new_sheet(&[]), &[Change::SetCategory(kind)]);
        assert_eq!(shown(&form(&draft)), controls, "{kind:?}");
        // With a second item the copies' floor appears where a stack can grow.
        let stacked = form(&after(&draft, &[Change::SetCount(2)]));
        assert_eq!(
            stacked.stack.copy_depth.visible,
            controls.contains(&"stack"),
            "{kind:?}"
        );
    }
    // A named weapon has no tier.
    let spear = after(
        &new_sheet(&[]),
        &[Change::SetItem(ItemChoice::Item(ItemId::Spear))],
    );
    assert!(!form(&spear).tier.visible);
    // Blankets never stack, never exclude resin and never choose a trinket.
    let blanket = |kind| {
        let rows = [row(1, kind)];
        shown(&form(&open(&rows, None, true, None, true, false)))
    };
    assert_eq!(
        blanket(ItemKind::Wand),
        ["upgrade", "uncursed", "source", "floor_limit"]
    );
    assert_eq!(blanket(ItemKind::Trinket), ["transmutations"]);
    // The resin section replaces the wand's own.
    let resin = form(&after(
        &new_sheet(&[]),
        &[
            Change::SetCategory(ItemKind::Wand),
            Change::SetItem(ItemChoice::ArcaneResin),
        ],
    ));
    assert_eq!(
        shown(&resin),
        ["uncursed", "source", "floor_limit", "resin"]
    );
    assert!(resin.resin_picked);
    assert_eq!(resin.uncursed.label, "Require uncursed wands");
    assert_eq!(resin.floor_limit.label, "Limit wands to a floor");
    assert_eq!(resin.preview, None);
}

/// The words the platforms kept beside the form, now the form's: the
/// help texts under check boxes, the section labels, the resin section and
/// its bounds — each the wording most platforms shared.
#[test]
fn the_form_words_every_control_the_platforms_worded_themselves() {
    let wand = form(&after(
        &new_sheet(&[]),
        &[Change::SetCategory(ItemKind::Wand)],
    ));
    assert_eq!(
        wand.exclude_resin.caption.as_deref(),
        Some(
            "Keep this wand without budgeting resin to upgrade it. Useful for imbuing: resin \
             upgrades do not transfer to the staff. Extra copies are reserved for reforging and \
             never need Auto resin."
        )
    );
    assert_eq!(wand.uncursed.caption, None);
    assert_eq!(wand.stack.label, "Total item count");
    let trinket = form(&after(
        &new_sheet(&[]),
        &[Change::SetCategory(ItemKind::Trinket)],
    ));
    assert_eq!(
        trinket.select_trinket.caption.as_deref(),
        Some(
            "Applies after the first brewing opportunity. If several alternatives are offered, \
             no trinket is chosen."
        )
    );
    let rings = form(&after(
        &new_sheet(&[]),
        &[
            Change::SetCategory(ItemKind::Ring),
            Change::SetItem(ItemChoice::Item(ItemId::RingMight)),
            Change::SetCount(2),
        ],
    ));
    assert!(rings.stack.count_levels.visible);
    assert_eq!(
        rings.stack.count_levels.caption.as_deref(),
        Some("Each item counts its upgrade plus one, and spare items may go unused.")
    );
    // The effect section is titled for its family.
    let weapon = form(&new_sheet(&[]));
    assert_eq!(weapon.effect.label, "Enchantment");
    let armor = form(&after(
        &new_sheet(&[]),
        &[Change::SetCategory(ItemKind::Armor)],
    ));
    assert_eq!(armor.effect.label, "Glyph");

    let resin = form(&after(
        &new_sheet(&[]),
        &[
            Change::SetCategory(ItemKind::Wand),
            Change::SetItem(ItemChoice::ArcaneResin),
        ],
    ));
    let control = &resin.resin;
    assert_eq!(control.label, "Minimum resin");
    assert_eq!(
        control
            .modes
            .iter()
            .map(|mode| (mode.value, mode.label.as_str()))
            .collect::<Vec<_>>(),
        [(false, "Amount"), (true, "Auto")]
    );
    assert_eq!(
        control.caption,
        "Upgrade each kept wand to +3. Excluded wands and extra copies reserved for reforging \
         need no resin."
    );
    assert_eq!((control.min, control.max), (1, 65_535));
    assert_eq!(
        control.include_mage_wand,
        Toggle {
            visible: true,
            value: false,
            label: "Include Mage’s starting wand".to_owned(),
            caption: Some(
                "Add 2 resin from the Magic Missile wand recovered with Wand Preservation when \
                 imbuing another wand. The preserved wand is +0, regardless of the staff’s level."
                    .to_owned()
            ),
        }
    );
    let credited = form(&after(
        &new_sheet(&[]),
        &[
            Change::SetCategory(ItemKind::Wand),
            Change::SetItem(ItemChoice::ArcaneResin),
            Change::SetIncludeMageWand(true),
        ],
    ));
    assert!(credited.resin.include_mage_wand.value);
    // The draft's amount error names the very bounds the form carries.
    assert_eq!(
        RESIN_AMOUNT_RANGE,
        format!("Enter an amount from {} to {}.", control.min, control.max)
    );
}

/// Which part of a control shows follows from the form alone: the value
/// slider of a mode picker outside "any", the effect grid in "Specific…".
#[test]
fn the_form_says_when_a_value_or_the_effect_grid_shows() {
    let any = form(&new_sheet(&[]));
    assert!(any.tier.visible && !any.tier.value_visible);
    assert!(any.upgrade.visible && !any.upgrade.value_visible);
    assert!(any.effect.visible && !any.effect.choices_visible);
    let bounded = form(&after(
        &new_sheet(&[]),
        &[
            Change::SetTierMode(TierMode::AtLeast),
            Change::SetUpgradeMode(UpgradeMode::Exact),
            Change::SetEffectMode(EffectMode::Specific),
        ],
    ));
    assert!(bounded.tier.value_visible && bounded.upgrade.value_visible);
    assert!(bounded.effect.choices_visible);
    let enchanted = form(&after(
        &new_sheet(&[]),
        &[Change::SetEffectMode(EffectMode::AnyEnchantment)],
    ));
    assert!(!enchanted.effect.choices_visible);
    // A hidden control shows no part of itself.
    let named = form(&after(
        &new_sheet(&[]),
        &[
            Change::SetTierMode(TierMode::Exact),
            Change::SetItem(ItemChoice::Item(ItemId::Spear)),
        ],
    ));
    assert!(!named.tier.visible && !named.tier.value_visible);
    let wand = form(&after(
        &new_sheet(&[]),
        &[
            Change::SetEffectMode(EffectMode::Specific),
            Change::SetCategory(ItemKind::Wand),
        ],
    ));
    assert!(!wand.effect.visible && !wand.effect.choices_visible);
}

/// iOS draws its cluster "How many" sheet's copy floor from a sheet opened
/// on a member: the form hides the stack there — it is the cluster's — but
/// fills it all, as it fills every hidden control.
#[test]
fn a_hidden_copy_floor_is_filled_for_a_cluster_member() {
    let rows = apply(
        &[
            named(1, ItemId::Spear),
            named(2, ItemId::Mace),
            named(3, ItemId::Sword),
        ],
        None,
        &[
            Edit::Join {
                source: 2,
                target: 1,
            },
            Edit::SetCount { key: 1, count: 2 },
            Edit::SetCopyDepth {
                key: 1,
                max_depth: Some(6),
            },
        ],
    )
    .rows;
    let member = form(&sheet(&rows, 2));
    assert!(member.in_cluster && !member.stack.visible);
    let floor = &member.stack.copy_depth;
    assert!(!floor.visible && floor.enabled);
    assert_eq!(floor.label, "Limit the extra copies to a floor");
    assert_eq!(floor.value, 6);
    assert_eq!(floor.value_label, "Copies within first 6 floors");
    assert_eq!(floor.options.len(), 21);
    assert!(
        !floor
            .options
            .iter()
            .any(|option| option.value % 5 == 0 && option.value < 20)
    );
    // Without a copy floor yet it starts where the switch turns on.
    let unlimited = form(&sheet(&rows[..2], 2));
    let floor = &unlimited.stack.copy_depth;
    assert!(!floor.enabled);
    assert_eq!(
        (floor.value, floor.value_label.as_str()),
        (4, "Copies within first 4 floors")
    );
}

/// Windows `ItemCatalogTests` and Linux's picker tests: the fresh pickers
/// leave out tier-1 gear, tipped darts and the catalyst; there are 17
/// trinkets and 11 artifacts; weapons are grouped by tier.
#[test]
fn the_item_pickers_offer_what_can_be_searched_for() {
    let trinkets = picker(ItemKind::Trinket, None);
    assert_eq!(trinkets.len(), 17);
    assert!(trinkets.iter().all(|option| {
        matches!(option.value, ItemChoice::Item(id) if item(id).kind == ItemKind::Trinket)
            && option.value != ItemChoice::Item(ItemId::TrinketCatalyst)
            && !option.hidden
    }));
    assert_eq!(trinkets[0].value, ItemChoice::Item(ItemId::RatSkull));
    assert!(trinkets.iter().any(|option| option.label == "Mimic Tooth"));
    let artifacts = picker(ItemKind::Artifact, None);
    assert_eq!(artifacts.len(), 11);
    assert!(
        artifacts
            .iter()
            .any(|option| option.value == ItemChoice::Item(ItemId::SandalsOfNature))
    );
    // Wands and rings have no tiers, so nothing hides; the wildcard leads.
    assert_eq!(picker(ItemKind::Wand, None).len(), 1 + 13);
    assert_eq!(picker(ItemKind::Ring, None).len(), 1 + 12);
    assert_eq!(picker(ItemKind::Ring, None)[0].label, "Any ring");
    let armor = picker(ItemKind::Armor, None);
    assert_eq!(labels(&armor)[..2], ["Any armor", "Leather Armor"]);
    assert_eq!(armor.len(), 1 + 4);

    let weapons = picker(ItemKind::Weapon, None);
    assert_eq!(weapons[0].label, "Any weapon");
    let listed: Vec<&ItemDefinition> = weapons[1..]
        .iter()
        .map(|option| match option.value {
            ItemChoice::Item(id) => item(id),
            other => panic!("{other:?}"),
        })
        .collect();
    assert_eq!(listed.len(), 26 + 12);
    assert!(
        listed
            .iter()
            .all(|definition| { definition.tier != Some(1) && !definition.id.is_tipped_dart() })
    );
    // Tier by tier, catalog order within each.
    assert!(
        listed
            .windows(2)
            .all(|pair| { (pair[0].tier, pair[0].id as u8) < (pair[1].tier, pair[1].id as u8) })
    );
    for (option, definition) in weapons[1..].iter().zip(&listed) {
        assert_eq!(
            option.group.as_deref(),
            Some(format!("Tier {}", definition.tier.unwrap()).as_str())
        );
    }
    let groups: Vec<&str> = weapons[1..]
        .iter()
        .filter_map(|option| option.group.as_deref())
        .collect();
    assert_eq!(groups.first(), Some(&"Tier 2"));
    assert_eq!(groups.last(), Some(&"Tier 5"));
    let melee = picker(ItemKind::Weapon, MELEE);
    assert_eq!(melee[0].label, "Any melee weapon");
    assert_eq!(melee.len(), 1 + 26);
    let thrown = picker(ItemKind::Weapon, THROWN);
    assert_eq!(thrown.len(), 1 + 12);
    assert!(thrown[1..].iter().all(|option| {
        matches!(option.value, ItemChoice::Item(id) if id.weapon_category() == THROWN)
    }));
}

/// Windows `ItemCatalogTests`: imports and share links can name an item the
/// fresh pickers leave out; the sheet lists that one item — where the
/// catalog puts it, marked hidden — so it shows and saves back unchanged.
#[test]
fn the_picker_lists_the_current_item_even_where_it_would_hide_it() {
    let worn = named_requirement(ItemId::WornShortsword);
    for category in [None, MELEE] {
        let fresh = picker(ItemKind::Weapon, category);
        let listed = picker_for(Requirement {
            weapon_category: category,
            ..worn
        });
        assert_eq!(listed.len(), fresh.len() + 1);
        // First among the items: tier 1 comes before tier 2.
        assert_eq!(listed[1].value, ItemChoice::Item(ItemId::WornShortsword));
        assert!(listed[1].hidden);
        assert_eq!(listed[1].group.as_deref(), Some("Tier 1"));
        assert_eq!(listed.iter().filter(|option| option.hidden).count(), 1);
        // Only that item: the other tier-1 weapons stay hidden.
        assert!(
            !listed
                .iter()
                .any(|option| option.value == ItemChoice::Item(ItemId::Cudgel))
        );
    }
    let draft = sheet(
        &[Row {
            key: 1,
            requirement: worn,
        }],
        1,
    );
    assert_eq!(
        form(&draft).item.value,
        ItemChoice::Item(ItemId::WornShortsword)
    );
    let (result, _) = saved(&draft);
    assert!(!result.changed);

    // A tipped dart sits among the tier-2 thrown weapons, in catalog order.
    let dart = picker_for(named_requirement(ItemId::PoisonDart));
    let position = dart
        .iter()
        .position(|option| option.value == ItemChoice::Item(ItemId::PoisonDart))
        .expect("the dart is listed");
    assert!(dart[position].hidden);
    assert_eq!(dart[position].group.as_deref(), Some("Tier 2"));
    assert_eq!(dart[position + 1].group.as_deref(), Some("Tier 3"));
    // The catalyst, the same way among the trinkets.
    let catalyst = picker_for(named_requirement(ItemId::TrinketCatalyst));
    assert_eq!(catalyst.len(), 18);
    assert!(catalyst.last().unwrap().hidden);

    // Every catalog item is listed on a sheet naming it, under its own kind.
    for definition in crate::catalog::ITEMS {
        let requirement = named_requirement(definition.id);
        for category in [None, definition.weapon_category()] {
            let listed = picker_for(Requirement {
                weapon_category: category,
                ..requirement
            });
            let option = listed
                .iter()
                .find(|option| option.value == ItemChoice::Item(definition.id))
                .unwrap_or_else(|| panic!("{:?}", definition.id));
            assert_eq!(option.hidden, !pickable(definition.id));
            assert_eq!(option.label, definition.display_name);
        }
    }
    // An item of another kind is never listed; the errors say why.
    let misfiled = Requirement {
        item: Some(ItemId::WornShortsword),
        ..Requirement::any(ItemKind::Ring)
    };
    assert!(
        !picker_for(misfiled)
            .iter()
            .any(|option| option.value == ItemChoice::Item(ItemId::WornShortsword))
    );
    let thrown_worn = Requirement {
        weapon_category: THROWN,
        ..worn
    };
    assert!(
        !picker_for(thrown_worn)
            .iter()
            .any(|option| option.value == ItemChoice::Item(ItemId::WornShortsword))
    );
    let draft = sheet(
        &[Row {
            key: 1,
            requirement: misfiled,
        }],
        1,
    );
    assert_eq!(
        form(&draft).errors,
        ["The item does not belong to this category."]
    );
}

#[test]
fn arcane_resin_is_offered_among_ordinary_wands_only() {
    let wand = after(&new_sheet(&[]), &[Change::SetCategory(ItemKind::Wand)]);
    assert_eq!(
        labels(&form(&wand).item.options)[..2],
        ["Any wand", "Arcane Resin"]
    );
    // Not where the platform keeps its own resin dialog.
    let unoffered = after(
        &open(&[], None, false, None, false, false),
        &[Change::SetCategory(ItemKind::Wand)],
    );
    assert!(!choices(&form(&unoffered)).contains(&ItemChoice::ArcaneResin));
    assert_eq!(
        after(&unoffered, &[Change::SetItem(ItemChoice::ArcaneResin)]),
        unoffered
    );
    // Not among other families.
    let ring = after(&new_sheet(&[]), &[Change::SetCategory(ItemKind::Ring)]);
    assert!(!choices(&form(&ring)).contains(&ItemChoice::ArcaneResin));
    assert_eq!(
        after(&ring, &[Change::SetItem(ItemChoice::ArcaneResin)]),
        ring
    );
}

// --- Arcane Resin ----------------------------------------------------------

/// Web `ArcaneResinEditor.test.tsx` "adds resin from the second wand
/// option, edits its filters": with the shared design's resin filter, which
/// starts from the query's (uncursed by default) rather than from the new
/// wand's — the web inherited the wand's "not uncursed".
#[test]
fn resin_picked_from_the_wand_picker_saves_the_query_resin() {
    let wand = after(
        &new_sheet(&[]),
        &[
            Change::SetCategory(ItemKind::Wand),
            Change::SetItem(ItemChoice::Item(ItemId::WandLightning)),
            Change::SetUpgradeMode(UpgradeMode::Exact),
        ],
    );
    let resin = after(&wand, &[Change::SetItem(ItemChoice::ArcaneResin)]);
    let control = form(&resin);
    assert_eq!(control.item.value, ItemChoice::ArcaneResin);
    assert!(control.resin_picked && control.resin.visible);
    assert!(!control.upgrade.visible && !control.stack.visible);
    assert!(control.uncursed.value);
    assert_eq!(control.resin.amount, Some(2.0));
    let resin = after(&resin, &[Change::SetFloorLimitEnabled(true)]);
    assert_eq!(resin.resin.max_depth, Some(4));
    assert_eq!(resin.requirement.max_depth, None);
    let (result, outcome) = saved(&resin);
    assert!(result.rows.is_empty() && !result.changed);
    let expected = ResinState {
        amount: ResinAmount::AtLeast(2),
        filter: ArcaneResinFilter {
            max_depth: Some(4),
            ..ArcaneResinFilter::default()
        },
    };
    assert_eq!(outcome, ResinOutcome::Set(expected));
    // Back to a wand: the wand draft is as it was.
    let back = after(
        &resin,
        &[Change::SetItem(ItemChoice::Item(ItemId::WandLightning))],
    );
    assert_eq!(back.requirement, wand.requirement);
    assert!(!back.resin_picked);

    // Editing the resin chip starts from the query's resin.
    let reopened = open(&[], None, false, Some(&expected), false, true);
    assert_eq!((reopened.origin, reopened.key), (Origin::Resin, None));
    let control = form(&reopened);
    assert_eq!(
        (control.mode, control.origin),
        (FormMode::Edit, Origin::Resin)
    );
    assert_eq!(control.item.value, ItemChoice::ArcaneResin);
    assert!(control.floor_limit.enabled);
    assert_eq!(control.floor_limit.value, 4);
    let edited = after(
        &reopened,
        &[
            Change::SetUncursed(false),
            Change::SetFloorLimitEnabled(false),
        ],
    );
    assert_eq!(
        saved(&edited).1,
        ResinOutcome::Set(ResinState {
            amount: ResinAmount::AtLeast(2),
            filter: ArcaneResinFilter {
                uncursed: false,
                ..ArcaneResinFilter::default()
            },
        })
    );
}

/// Web "loads legacy resin and can turn it into an ordinary wand
/// requirement": saving the resin chip as anything else clears the resin.
#[test]
fn the_resin_chip_saved_as_a_requirement_clears_the_resin() {
    let legacy = ResinState {
        amount: ResinAmount::AtLeast(6),
        filter: ArcaneResinFilter::default(),
    };
    let draft = open(&[], None, false, Some(&legacy), false, true);
    let control = form(&draft);
    assert!(control.uncursed.value);
    assert_eq!(control.resin.amount, Some(6.0));
    assert!(choices(&control).contains(&ItemChoice::ArcaneResin));
    let wand = after(
        &draft,
        &[Change::SetItem(ItemChoice::Item(ItemId::WandLightning))],
    );
    let control = form(&wand);
    assert!(control.upgrade.visible && !control.resin.visible);
    let (result, outcome) = saved(&wand);
    assert_eq!(outcome, ResinOutcome::Clear);
    assert_eq!(
        result
            .rows
            .iter()
            .map(|row| row.requirement)
            .collect::<Vec<_>>(),
        [named_requirement(ItemId::WandLightning)],
        "the wand draft's own filters, not the resin's"
    );
    // Any family clears it.
    let ring = after(&draft, &[Change::SetCategory(ItemKind::Ring)]);
    assert_eq!(saved(&ring).1, ResinOutcome::Clear);
}

/// Android's report: a resin sheet opened on a query without resin said
/// "edit", so the platform decided Add and Remove from its own state. There
/// is no resin chip to edit, so the sheet is a new one with Arcane Resin
/// picked: the chrome says Add and offers no Remove, and saving it as a
/// wand adds the wand and clears nothing.
#[test]
fn a_resin_sheet_on_a_query_without_resin_adds_one() {
    let rows = [row(1, ItemKind::Ring)];
    let two = ResinState {
        amount: ResinAmount::AtLeast(2),
        filter: ArcaneResinFilter::default(),
    };
    let draft = open(&rows, None, false, None, false, true);
    assert_eq!((draft.origin, draft.key), (Origin::New, None));
    let control = form(&draft);
    assert_eq!((control.mode, control.origin), (FormMode::New, Origin::New));
    assert!(control.resin_picked && control.resin.visible);
    assert_eq!(control.title, ARCANE_RESIN);
    assert_eq!(control.item.value, ItemChoice::ArcaneResin);
    let (result, outcome) = saved(&draft);
    assert_eq!(result.rows, rows);
    assert_eq!(outcome, ResinOutcome::Set(two));
    let wand = after(&draft, &[Change::SetItem(ItemChoice::Any)]);
    let (result, outcome) = saved(&wand);
    assert_eq!(outcome, ResinOutcome::Unchanged);
    assert_eq!(result.rows.len(), 2);

    // With the query's resin it is the resin chip, which the sheet edits.
    let draft = open(&rows, None, false, Some(&two), false, true);
    let control = form(&draft);
    assert_eq!(
        (control.mode, control.origin),
        (FormMode::Edit, Origin::Resin)
    );
}

/// Web "selects Auto, preserves filters, and restores the mode when
/// editing" and "preserves the Mage credit while switching resin modes".
#[test]
fn auto_resin_keeps_the_amount_and_the_filters_across_modes() {
    let auto = after(
        &new_sheet(&[]),
        &[
            Change::SetCategory(ItemKind::Wand),
            Change::SetItem(ItemChoice::ArcaneResin),
            Change::SetResinAuto(true),
            Change::SetFloorLimitEnabled(true),
        ],
    );
    assert!(form(&auto).resin.auto);
    let ResinOutcome::Set(state) = saved(&auto).1 else {
        panic!("resin saves");
    };
    assert_eq!(state.amount, ResinAmount::Auto);
    assert_eq!(state.filter.max_depth, Some(4));
    let reopened = open(&[], None, false, Some(&state), true, true);
    assert!(reopened.resin.auto);
    assert_eq!(reopened.resin.amount, Some(2.0));
    let amount = after(&reopened, &[Change::SetResinAuto(false)]);
    assert_eq!(
        saved(&amount).1,
        ResinOutcome::Set(ResinState {
            amount: ResinAmount::AtLeast(2),
            filter: state.filter,
        })
    );

    let mage = after(
        &open(&[], None, false, Some(&state), true, true),
        &[Change::SetIncludeMageWand(true)],
    );
    let ResinOutcome::Set(credited) = saved(&mage).1 else {
        panic!("resin saves");
    };
    assert!(credited.filter.include_mage_wand);
    let switched = after(
        &open(&[], None, false, Some(&credited), true, true),
        &[Change::SetResinAuto(false)],
    );
    let ResinOutcome::Set(switched) = saved(&switched).1 else {
        panic!("resin saves");
    };
    assert_eq!(switched.amount, ResinAmount::AtLeast(2));
    assert!(switched.filter.include_mage_wand);
}

/// The Apple review's regression: the resin filter starts from the query's
/// and leaves the wand draft's own filters alone, both ways.
#[test]
fn the_resin_filter_is_the_querys_not_the_wand_drafts() {
    let query = ResinState {
        amount: ResinAmount::AtLeast(5),
        filter: ArcaneResinFilter {
            include_mage_wand: false,
            uncursed: false,
            max_depth: Some(9),
            source: Some(ItemSource::Shop),
        },
    };
    let wand = after(
        &open(&[], None, false, Some(&query), true, false),
        &[
            Change::SetCategory(ItemKind::Wand),
            Change::SetUncursed(true),
            Change::SetSource(Some(ItemSource::Heap)),
        ],
    );
    let resin = after(&wand, &[Change::SetItem(ItemChoice::ArcaneResin)]);
    let control = form(&resin);
    assert!(!control.uncursed.value);
    assert_eq!(control.source.value, Some(ItemSource::Shop));
    assert!(control.floor_limit.enabled);
    assert_eq!(control.floor_limit.value, 9);
    assert_eq!(control.resin.amount, Some(5.0));
    let resin = after(&resin, &[Change::SetSource(None)]);
    let back = after(&resin, &[Change::SetItem(ItemChoice::Any)]);
    assert!(back.requirement.require_uncursed);
    assert_eq!(back.requirement.source, Some(ItemSource::Heap));
    assert_eq!(back.resin.source, None);
}

/// Web `QueryPanel.onSaveResin`: a wand chip turned into the resin leaves
/// the board — a cluster member alone, a chip with its copies.
#[test]
fn a_wand_chip_turned_into_resin_leaves_the_board() {
    let rows = written(&[
        add(named_requirement(ItemId::WandLightning), 2, None, None),
        add(Requirement::any(ItemKind::Ring), 1, None, None),
        add(Requirement::any(ItemKind::Wand), 1, None, None),
        Edit::Join {
            source: 4,
            target: 3,
        },
    ]);
    assert_eq!(counts(&rows), [2, 1]);
    let chip = after(
        &sheet(&rows, 1),
        &[Change::SetItem(ItemChoice::ArcaneResin)],
    );
    let (result, outcome) = saved(&chip);
    assert_eq!(keys(&result.rows), [3, 4]);
    assert!(matches!(outcome, ResinOutcome::Set(_)));
    let member = after(
        &sheet(&rows, 4),
        &[Change::SetItem(ItemChoice::ArcaneResin)],
    );
    let (result, _) = saved(&member);
    assert_eq!(keys(&result.rows), [1, 2, 3]);
    assert_eq!(counts(&result.rows), [2, 1]);
}

/// Web: the amount is typed, so the field can hold anything; only whole
/// numbers from 1 to 65535 save. Auto needs no amount.
#[test]
fn a_resin_amount_saves_only_as_a_whole_number_in_range() {
    let draft = after(
        &new_sheet(&[]),
        &[
            Change::SetCategory(ItemKind::Wand),
            Change::SetItem(ItemChoice::ArcaneResin),
        ],
    );
    for amount in [None, Some(0.0), Some(1.5), Some(-3.0), Some(70_000.0)] {
        let typed = after(&draft, &[Change::SetResinAmount(amount)]);
        let control = form(&typed);
        assert_eq!(control.errors, [RESIN_AMOUNT_RANGE], "{amount:?}");
        assert!(!control.can_save);
        assert_eq!(control.resin.amount, amount);
        assert_eq!(refusal(&typed), [RESIN_AMOUNT_RANGE]);
    }
    for amount in [1_u16, 65_535] {
        let typed = after(&draft, &[Change::SetResinAmount(Some(f64::from(amount)))]);
        let ResinOutcome::Set(state) = saved(&typed).1 else {
            panic!("resin saves");
        };
        assert_eq!(state.amount, ResinAmount::AtLeast(amount));
    }
    let auto = after(
        &draft,
        &[Change::SetResinAmount(None), Change::SetResinAuto(true)],
    );
    assert!(form(&auto).can_save);
    assert_eq!(after(&auto, &[Change::SetResinAmount(Some(9.0))]), auto);
}

// --- errors and the save guard ---------------------------------------------

/// Web `impossible-query.test.tsx`: the deck holds each trinket once, so a
/// second ordinary requirement for one is an error; a blanket may reuse it,
/// and the row itself is no duplicate of itself.
#[test]
fn a_trinket_another_ordinary_row_names_is_a_duplicate() {
    let rows = [
        named(1, ItemId::RatSkull),
        with(named(2, ItemId::MimicTooth), |r| r.blanket = true),
    ];
    for blanket in [false, true] {
        let draft = after(
            &open(&rows[..1], None, blanket, None, false, false),
            &[
                Change::SetCategory(ItemKind::Trinket),
                Change::SetItem(ItemChoice::Item(ItemId::RatSkull)),
                Change::SetTransmutationsEnabled(true),
                Change::SetTransmutations(13),
            ],
        );
        assert_eq!(
            form(&draft).errors.contains(&DUPLICATE_TRINKET.to_owned()),
            !blanket
        );
    }
    let own = sheet(&rows, 1);
    assert!(own.taken_trinkets.is_empty());
    assert!(form(&own).can_save);
    let other = after(&new_sheet(&rows), &[Change::SetCategory(ItemKind::Trinket)]);
    assert_eq!(other.taken_trinkets, [ItemId::RatSkull]);
    assert_eq!(refusal(&other), [DUPLICATE_TRINKET]);
    assert!(
        form(&after(
            &other,
            &[Change::SetItem(ItemChoice::Item(ItemId::MimicTooth))]
        ))
        .can_save,
        "a blanket's trinket is not taken"
    );
}

/// Critic M4: a cluster member of a stack edited into another category
/// broke the stack on every platform but Linux, whose whole-query check
/// refused it. The save now follows the join rule (#190) and is refused
/// everywhere — trinkets and artifacts too, although no label is spread onto
/// them and so no problem would have blamed the row.
#[test]
fn the_save_guard_refuses_a_save_that_breaks_a_stack() {
    let rows = written(&[
        add(named_requirement(ItemId::Spear), 1, None, None),
        add(named_requirement(ItemId::Mace), 1, None, None),
        Edit::Join {
            source: 2,
            target: 1,
        },
        Edit::SetCount { key: 1, count: 2 },
    ]);
    assert_eq!(counts(&rows), [2]);
    let mixed = Refusal::MixedCategoryStack.to_string();
    for changes in [
        &[Change::SetCategory(ItemKind::Wand)][..],
        &[
            Change::SetCategory(ItemKind::Trinket),
            Change::SetItem(ItemChoice::Item(ItemId::MimicTooth)),
        ],
        &[Change::SetCategory(ItemKind::Artifact)],
    ] {
        let draft = after(&sheet(&rows, 2), changes);
        let control = form(&draft);
        assert_eq!(control.errors, std::slice::from_ref(&mixed), "{changes:?}");
        assert!(!control.can_save);
        assert_eq!(control.preview, None);
        let SaveResult::Refused { draft: kept, form } = save(&draft, &rows, None) else {
            panic!("the save is refused: {changes:?}");
        };
        assert_eq!(kept.rows, rows);
        assert_eq!(form, control);
    }
    // Another weapon is fine, and so is a lone chip changing category.
    assert!(form_can_save(&after(
        &sheet(&rows, 2),
        &[Change::SetItem(ItemChoice::Item(ItemId::Sword))]
    )));
    let lone = written(&[add(named_requirement(ItemId::Mace), 2, None, None)]);
    let wand = after(&sheet(&lone, 1), &[Change::SetCategory(ItemKind::Wand)]);
    assert_eq!(stored(&wand).len(), 2);
}

/// A cluster shrunk to ×1 keeps its stack label (M2). Moving a member into
/// another category then saves as a drag of that item onto the cluster
/// would: the leftover labels are cleared and nothing is deleted — the sheet
/// used to refuse it with a message about copies the board did not show.
#[test]
fn a_member_of_a_cluster_shrunk_to_one_changes_category_like_a_join() {
    let rows = written(&[
        add(named_requirement(ItemId::Spear), 1, None, None),
        add(named_requirement(ItemId::Mace), 1, None, None),
        Edit::Join {
            source: 2,
            target: 1,
        },
        Edit::SetCount { key: 1, count: 2 },
        Edit::SetCount { key: 1, count: 1 },
    ]);
    assert_eq!(counts(&rows), [1]);
    assert!(
        rows.iter()
            .all(|row| row.requirement.identity_group.is_some())
    );
    let wand = [Row {
        key: 9,
        requirement: named_requirement(ItemId::WandFrost),
    }];
    let beside: Vec<Row> = rows.iter().copied().chain(wand).collect();
    assert_eq!(
        drop_action(&beside, 9, DropTarget::Row(1)),
        DropAction::Join { target: 1 }
    );
    for changes in [
        &[Change::SetCategory(ItemKind::Wand)][..],
        &[Change::SetCategory(ItemKind::Trinket)],
    ] {
        let draft = after(&sheet(&rows, 2), changes);
        assert!(form(&draft).can_save, "{:?}", form(&draft).errors);
        let (result, _) = saved(&draft);
        assert_eq!(keys(&result.rows), keys(&rows));
        assert_emittable(&result.rows, "a member left its category");
        assert!(result.rows.iter().all(|row| {
            row.requirement.identity_group.is_none() && row.requirement.alternative_group.is_some()
        }));
        assert!(problems(&result.rows).is_empty());
    }
}

fn form_can_save(draft: &Draft) -> bool {
    form(draft).can_save
}

/// The guard blames only what the save causes: not a row that was wrong
/// already, not the list-level "add an ordinary requirement" of a first
/// blanket, not a problem the edited row already had.
#[test]
fn the_save_guard_ignores_problems_the_save_did_not_cause() {
    let broken = with(row(1, ItemKind::Ring), |r| r.exclude_resin = true);
    assert!(!row_problems(&broken.requirement).is_empty());
    let draft = after(
        &new_sheet(&[broken]),
        &[Change::SetCategory(ItemKind::Wand)],
    );
    assert!(form(&draft).can_save);
    assert_eq!(saved(&draft).0.rows.len(), 2);

    let blanket = open(&[], None, true, None, false, false);
    assert!(form(&blanket).can_save);
    let (result, _) = saved(&blanket);
    assert_eq!(problems(&result.rows)[0].message, NO_ORDINARY_REQUIREMENT);

    // Two constrained members of one stack: a hand-written problem both
    // rows already have. Editing one of them still saves.
    let over = [
        with(named(1, ItemId::Spear), |r| r.identity_group = Some(1)),
        with(named(2, ItemId::Mace), |r| r.identity_group = Some(1)),
    ];
    assert!(!problems(&over).is_empty());
    let draft = after(
        &sheet(&over, 1),
        &[
            Change::SetUpgradeMode(UpgradeMode::Exact),
            Change::SetUpgrade(2),
        ],
    );
    assert!(form(&draft).can_save, "{:?}", form(&draft).errors);
    let (result, _) = saved(&draft);
    assert_eq!(
        result.rows[0].requirement.upgrade,
        UpgradeRequirement::Exact(2)
    );
    assert!(!problems(&result.rows).is_empty());
}

/// Every identity label in use: the stack a new chip asks for cannot be
/// written, and the sheet says so rather than dropping its copies.
#[test]
fn a_stack_without_a_free_label_is_an_error() {
    let rows = written(
        &(0..4)
            .map(|_| add(Requirement::any(ItemKind::Wand), 2, None, None))
            .collect::<Vec<_>>(),
    );
    assert_eq!(counts(&rows), [2, 2, 2, 2]);
    let draft = after(
        &new_sheet(&rows),
        &[Change::SetCategory(ItemKind::Armor), Change::SetCount(2)],
    );
    assert_eq!(form(&draft).errors, [Refusal::NoFreeGroup.to_string()]);
    assert_eq!(refusal(&draft), [Refusal::NoFreeGroup.to_string()]);
    // Without the stack it saves.
    assert!(form_can_save(&after(&draft, &[Change::SetCount(1)])));
}

// --- what a save writes ------------------------------------------------------

#[test]
fn a_save_writes_only_the_stack_the_sheet_shows() {
    // A cluster member saves without a stack of its own, whatever the draft
    // holds.
    let cluster = written(&[
        add(named_requirement(ItemId::Spear), 1, None, None),
        add(named_requirement(ItemId::Mace), 1, None, None),
        Edit::Join {
            source: 2,
            target: 1,
        },
        Edit::SetCount { key: 1, count: 2 },
    ]);
    let mut member = sheet(&cluster, 2);
    member.count = 3;
    let (result, _) = saved(&member);
    assert!(!result.changed);
    assert_eq!(counts(&result.rows), [2]);
    // A blanket never stacks.
    let blankets = [
        row(1, ItemKind::Wand),
        with(row(2, ItemKind::Wand), |r| r.blanket = true),
    ];
    let mut blanket = sheet(&blankets, 2);
    blanket.count = 3;
    assert_eq!(saved(&blanket).0.rows.len(), 2);

    // A total only while the combined level shows and is on.
    let mut frost = after(
        &new_sheet(&[]),
        &[
            Change::SetCategory(ItemKind::Wand),
            Change::SetItem(ItemChoice::Item(ItemId::WandFrost)),
            Change::SetCount(2),
        ],
    );
    frost.total = Some(3);
    let rows = stored(&frost);
    assert_eq!(rows.len(), 2);
    assert!(
        rows.iter()
            .all(|requirement| requirement.level_sum.is_none())
    );
    let ring = after(
        &new_sheet(&[]),
        &[
            Change::SetCategory(ItemKind::Ring),
            Change::SetItem(ItemChoice::Item(ItemId::RingMight)),
            Change::SetCount(2),
            Change::SetCopyDepthEnabled(true),
            Change::SetCountLevels(true),
            Change::SetTotal(5),
        ],
    );
    assert_eq!(ring.copy_depth, Some(4));
    let rows = stored(&ring);
    assert_eq!(
        rows,
        vec![
            Requirement {
                level_sum: Some(LevelSum {
                    group: 1,
                    minimum_total: 5
                }),
                ..named_requirement(ItemId::RingMight)
            };
            2
        ],
        "counting: no copy floor"
    );
    let plain = after(&ring, &[Change::SetCountLevels(false)]);
    let rows = stored(&plain);
    assert!(
        rows.iter()
            .all(|requirement| requirement.level_sum.is_none())
    );
    assert_eq!(rows[1].max_depth, Some(4), "the copy floor is back");

    // A copy floor only while it shows and is on.
    let copies = after(
        &new_sheet(&[]),
        &[
            Change::SetCategory(ItemKind::Wand),
            Change::SetItem(ItemChoice::Item(ItemId::WandFrost)),
            Change::SetCount(2),
            Change::SetCopyDepthEnabled(true),
            Change::SetCopyDepth(9),
        ],
    );
    let rows = stored(&copies);
    assert_eq!(
        rows.iter().map(|r| r.max_depth).collect::<Vec<_>>(),
        [None, Some(9)]
    );
    let single = after(&copies, &[Change::SetCount(1)]);
    assert_eq!(single.copy_depth, Some(9));
    assert_eq!(stored(&single).len(), 1);
    let off = after(&copies, &[Change::SetCopyDepthEnabled(false)]);
    assert_eq!(stored(&off)[1].max_depth, None);
}

/// Android's refine plan compares whole requests, keys included: a sheet
/// saved without a change gives back the very same rows.
#[test]
fn saving_an_untouched_sheet_changes_nothing() {
    let rows = written(&[
        add(named_requirement(ItemId::RingMight), 3, Some(6), None),
        add(
            Requirement {
                upgrade: UpgradeRequirement::Exact(3),
                ..named_requirement(ItemId::Spear)
            },
            2,
            None,
            Some(6),
        ),
        add(
            Requirement {
                upgrade: UpgradeRequirement::AtLeast(2),
                ..Requirement::any(ItemKind::Wand)
            },
            2,
            None,
            None,
        ),
        add(Requirement::any(ItemKind::Ring), 1, None, None),
        add(Requirement::any(ItemKind::Armor), 1, None, None),
        Edit::Join {
            source: 9,
            target: 8,
        },
        add(
            Requirement {
                blanket: true,
                source: Some(ItemSource::WandmakerReward),
                ..Requirement::any(ItemKind::Wand)
            },
            1,
            None,
            None,
        ),
        add(
            Requirement {
                trinket_transmutations: 3,
                ..named_requirement(ItemId::RatSkull)
            },
            1,
            None,
            None,
        ),
    ]);
    assert_eq!(validate(&rows), Ok(()));
    for entry in board_items(&rows) {
        for &index in &entry.members {
            let draft = sheet(&rows, rows[index].key);
            let (result, outcome) = saved(&draft);
            assert!(!result.changed, "{:?}", rows[index]);
            assert_eq!(result.rows, rows);
            assert_eq!(result.focus, Some(rows[entry.anchor()].key));
            assert_eq!(outcome, ResinOutcome::Unchanged);
        }
    }
}

#[test]
fn a_save_onto_changed_rows_checks_them_and_keeps_them() {
    // The sheet was opened on one list and saved onto another.
    let opened_on = [row(1, ItemKind::Wand)];
    let draft = after(
        &new_sheet(&opened_on),
        &[Change::SetCategory(ItemKind::Trinket)],
    );
    assert!(form(&draft).can_save);
    let now = [row(1, ItemKind::Wand), named(2, ItemId::RatSkull)];
    let SaveResult::Refused {
        draft: refused,
        form,
    } = save(&draft, &now, None)
    else {
        panic!("the trinket is taken now");
    };
    assert_eq!(form.errors, [DUPLICATE_TRINKET]);
    assert_eq!(refused.rows, now);
    assert_eq!(refused.taken_trinkets, [ItemId::RatSkull]);
    // Keys a platform never repaired are repaired, and reported.
    let broken = [row(0, ItemKind::Wand)];
    let SaveResult::Saved { result, .. } = save(&new_sheet(&broken), &broken, None) else {
        panic!("saves");
    };
    assert_eq!(result.rekeyed, [(0, 1)]);
    assert!(result.changed);
    assert_eq!(keys(&result.rows), [1, 2]);
}

// --- properties --------------------------------------------------------------

/// A sheet opened some random way on `rows`.
fn random_sheet(rng: &mut Rng, rows: &[Row]) -> Draft {
    let resin = rng.chance(40).then(|| random_resin(rng));
    let key = match rng.below(4) {
        0 => None,
        1 => Some(rng.next() % 12),
        _ if rows.is_empty() => None,
        _ => Some(rows[rng.below(rows.len())].key),
    };
    open(
        rows,
        key,
        rng.chance(20),
        resin.as_ref(),
        rng.chance(60),
        rng.chance(10),
    )
}

/// Every numeric control within its range, every value among its options.
/// A preview shows only for a savable requirement; on a sheet [`open`]
/// built (`sheet`), every savable requirement previews its chip.
fn assert_in_range(form: &Form, sheet: bool, context: &str) {
    let ranges = [
        ("tier", form.tier.min, form.tier.value, form.tier.max),
        (
            "upgrade",
            form.upgrade.min,
            form.upgrade.value,
            form.upgrade.max,
        ),
        (
            "transmutations",
            form.transmutations.min,
            form.transmutations.value,
            form.transmutations.max,
        ),
        ("count", form.stack.min, form.stack.count, form.stack.max),
        (
            "total",
            form.stack.count_levels.min,
            form.stack.count_levels.value,
            form.stack.count_levels.max,
        ),
    ];
    for (name, min, value, max) in ranges {
        assert!(
            min <= value && value <= max,
            "{name}: {value} outside {min}..={max}: {context}"
        );
    }
    for floor in [&form.floor_limit, &form.stack.copy_depth] {
        assert!(
            floor
                .options
                .iter()
                .any(|option| option.value == floor.value),
            "floor {}: {context}",
            floor.value
        );
    }
    assert!(
        form.category
            .options
            .iter()
            .any(|option| option.value == form.category.value)
    );
    assert!(
        form.kind
            .options
            .iter()
            .any(|option| option.value == form.kind.value)
    );
    assert!(
        form.source
            .options
            .iter()
            .any(|option| option.value == form.source.value)
    );
    assert!(
        form.tier
            .modes
            .iter()
            .any(|mode| mode.value == form.tier.mode)
    );
    assert!(
        form.upgrade
            .modes
            .iter()
            .any(|mode| mode.value == form.upgrade.mode)
    );
    assert_eq!(form.can_save, form.errors.is_empty(), "{context}");
    let savable = form.can_save && !form.resin_picked;
    assert!(form.preview.is_none() || savable, "{context}");
    assert!(!sheet || form.preview.is_some() == savable, "{context}");
    assert_filled(form, context);
    // The header's title is always there, and on a sheet opened from a list
    // it names the chip the preview shows.
    assert_eq!(form.resin_picked, form.title == ARCANE_RESIN, "{context}");
    if sheet && let Some(preview) = &form.preview {
        assert_eq!(preview.title, form.title, "{context}");
    }
}

/// Every control carries all its words, shown or hidden — its label, its
/// options, its value in words, the help texts the form gives — and says
/// which of its parts show only while it shows.
fn assert_filled(form: &Form, context: &str) {
    for toggle in [
        &form.uncursed,
        &form.exclude_resin,
        &form.select_trinket,
        &form.resin.include_mage_wand,
    ] {
        assert!(!toggle.label.is_empty(), "{context}");
        assert!(
            toggle
                .caption
                .as_ref()
                .is_none_or(|caption| !caption.is_empty())
        );
    }
    for (caption, name) in [
        (&form.exclude_resin.caption, "exclude_resin"),
        (&form.select_trinket.caption, "select_trinket"),
        (&form.resin.include_mage_wand.caption, "include_mage_wand"),
        (&form.transmutations.caption, "transmutations"),
        (&form.stack.count_levels.caption, "count_levels"),
    ] {
        assert!(caption.is_some(), "{name}: {context}");
    }
    for floor in [&form.floor_limit, &form.stack.copy_depth] {
        assert!(!floor.label.is_empty(), "{context}");
        assert_eq!(floor.options.len(), 21, "{context}");
        assert!(
            floor.value_label.contains(&floor.value.to_string()),
            "{context}"
        );
    }
    for (label, value_label) in [
        (&form.transmutations.label, &form.transmutations.value_label),
        (
            &form.stack.count_levels.label,
            &form.stack.count_levels.value_label,
        ),
        (&form.stack.label, &form.stack.value_label),
    ] {
        assert!(!label.is_empty() && !value_label.is_empty(), "{context}");
    }
    assert!(!form.tier.value_label.is_empty() && !form.upgrade.value_label.is_empty());
    assert_eq!(form.tier.modes.len(), 4, "{context}");
    assert_eq!(form.upgrade.modes.len(), 3, "{context}");
    assert_eq!(
        form.tier.value_visible,
        form.tier.visible && form.tier.mode != TierMode::Any,
        "{context}"
    );
    assert_eq!(
        form.upgrade.value_visible,
        form.upgrade.visible && form.upgrade.mode != UpgradeMode::Any,
        "{context}"
    );
    assert!(!form.effect.label.is_empty() && !form.effect.caption.is_empty());
    assert_eq!(form.effect.modes.len(), 3, "{context}");
    assert_eq!(
        form.effect.choices_visible,
        form.effect.visible && form.effect.mode == EffectMode::Specific,
        "{context}"
    );
    assert!(form.effect.choices.is_empty() || !form.effect.groups.is_empty());
    let resin = &form.resin;
    assert!(
        !resin.label.is_empty() && !resin.caption.is_empty(),
        "{context}"
    );
    assert_eq!(resin.modes.len(), 2, "{context}");
    assert!(resin.min <= resin.max, "{context}");
    assert_eq!(resin.include_mage_wand.visible, resin.visible, "{context}");
    for picker in [
        form.category.options.len(),
        form.kind.options.len(),
        form.weapon_type.options.len(),
        form.item.options.len(),
        form.source.options.len(),
    ] {
        assert!(picker > 0, "{context}");
    }
}

/// The sheet's header reads `form.title`, which — unlike the preview — is
/// there while the draft cannot be saved and while the resin is picked, so
/// no platform rebuilds a tier-worded title of its own.
#[test]
fn the_form_titles_the_sheet_even_while_it_cannot_save() {
    let draft = after(
        &new_sheet(&[]),
        &[
            Change::SetWeaponType(MELEE),
            Change::SetTierMode(TierMode::AtLeast),
            Change::SetTier(3),
        ],
    );
    let control = form(&draft);
    assert_eq!(control.title, "Any Tier 3+ melee weapon");
    assert_eq!(
        control.preview.as_ref().map(|chip| chip.title.as_str()),
        Some("Any Tier 3+ melee weapon")
    );

    let rows = [named(1, ItemId::RatSkull)];
    let duplicate = after(
        &new_sheet(&rows),
        &[
            Change::SetCategory(ItemKind::Trinket),
            Change::SetItem(ItemChoice::Item(ItemId::RatSkull)),
        ],
    );
    let control = form(&duplicate);
    assert_eq!(control.errors, [DUPLICATE_TRINKET]);
    assert_eq!(control.preview, None);
    assert_eq!(control.title, "Rat Skull");

    let resin = after(
        &new_sheet(&[]),
        &[
            Change::SetCategory(ItemKind::Wand),
            Change::SetItem(ItemChoice::ArcaneResin),
            Change::SetResinAmount(None),
        ],
    );
    let control = form(&resin);
    assert!(control.resin_picked);
    assert_eq!(control.errors, [RESIN_AMOUNT_RANGE]);
    assert_eq!(control.title, ARCANE_RESIN);
}

/// Random valid rows × random sheets × random changes (1,024 cases): every
/// change keeps the requirement free of problems of its own and the form in
/// range; the form is deterministic; a save agrees with the form, and what
/// it writes keeps the §3 invariant.
#[test]
fn random_changes_keep_the_form_in_range_and_every_save_emittable() {
    let mut rng = Rng::new(0x5ee7_ed17_0f0f_2026);
    for case in 0..1024 {
        let rows = apply(&random_rows(&mut rng), None, &[Edit::Normalize]).rows;
        let mut draft = random_sheet(&mut rng, &rows);
        let mut steps = Vec::new();
        for _ in 0..rng.range(0, 10) {
            let step = random_change(&mut rng, &draft);
            steps.push(step);
            draft = change(&draft, &step);
            let context = format!("case {case}: {rows:?} {steps:?}");
            let own = Requirement {
                alternative_group: None,
                ..saved_requirement(&draft)
            };
            assert_eq!(row_problems(&own), Vec::<String>::new(), "{context}");
            let shown = form(&draft);
            assert_in_range(&shown, true, &context);
            if !shown.resin_picked {
                assert!(
                    shown
                        .item
                        .options
                        .iter()
                        .any(|option| option.value == shown.item.value),
                    "{context}"
                );
            }
        }
        let context = format!("case {case}: {rows:?} {steps:?}");
        let shown = form(&draft);
        assert_eq!(form(&draft.clone()), shown, "{context}");
        let hint = rng.chance(20).then(|| rng.next() % 30);
        match save(&draft, &rows, hint) {
            SaveResult::Saved { result, resin } => {
                assert!(shown.can_save, "{context}");
                assert_emittable(&result.rows, &context);
                if let ResinOutcome::Set(state) = resin {
                    assert!(shown.resin_picked, "{context}");
                    assert!(
                        !matches!(state.amount, ResinAmount::AtLeast(0)),
                        "{context}"
                    );
                } else {
                    assert!(result.focus.is_some(), "{context}");
                }
            }
            SaveResult::Refused { draft: kept, form } => {
                assert!(!shown.can_save, "{context}");
                assert!(!form.errors.is_empty(), "{context}");
                assert_eq!(kept.rows, rows, "{context}");
            }
        }
    }
}

/// Rows the editor itself wrote: new chips saved with random stacks, and
/// random board edits.
fn editor_rows(rng: &mut Rng) -> Vec<Row> {
    let mut rows: Vec<Row> = Vec::new();
    for _ in 0..rng.range(1, 8) {
        let edit = if rows.is_empty() || rng.chance(45) {
            add(
                random_requirement(rng),
                rng.range(1, 3),
                rng.chance(30).then(|| rng.range(1, 11)),
                rng.chance(30).then(|| rng.range(1, 24)),
            )
        } else {
            random_edit(rng, &rows)
        };
        rows = apply(&rows, None, &[edit]).rows;
    }
    rows
}

/// A requirement up to the re-encodings the sheet makes of values its
/// controls cannot hold — "+0 or higher" is any upgrade, "+max or higher"
/// exactly +max, a floor limit on an empty boss floor the floor below — and
/// without the stack labels the sheet shows as a count and a total. Nothing
/// else may differ between a row and the sheet opened on it.
fn as_the_sheet_holds_it(requirement: Requirement) -> Requirement {
    let ceiling = requirement.upgrade_ceiling();
    Requirement {
        upgrade: match requirement.upgrade {
            UpgradeRequirement::AtLeast(0) => UpgradeRequirement::Any,
            UpgradeRequirement::AtLeast(upgrade) if upgrade >= ceiling => {
                UpgradeRequirement::Exact(ceiling)
            }
            upgrade => upgrade,
        },
        max_depth: requirement.max_depth.map(normalize_floor_limit),
        identity_group: None,
        level_sum: None,
        ..requirement
    }
}

/// Opens a sheet on every row of `rows` — chips, cluster members, blankets,
/// the anchors of stacks and their hidden copies — and saves it untouched:
/// the rows come back identical, `changed: false`, and the focus on the
/// chip. The sheet holds every field of the row but for the re-encodings of
/// [`as_the_sheet_holds_it`], so a save that changes something else drops
/// nothing. The one refusal an untouched sheet may meet is a duplicate
/// trinket the list already holds.
fn assert_untouched_saves_change_nothing(rows: &[Row], context: &str) {
    let entries = board_items(rows);
    for (index, row) in rows.iter().enumerate() {
        let context = format!("{context} at {}", row.key);
        let entry = entries
            .iter()
            .find(|entry| entry.members.contains(&index) || entry.extras.contains(&index))
            .expect("every row is on the board");
        let shown = if entry.members.contains(&index) {
            index
        } else {
            entry.anchor()
        };
        let draft = open(rows, Some(row.key), false, None, true, false);
        assert_eq!(draft.origin, Origin::Row(rows[shown].key), "{context}");
        assert_eq!(
            saved_requirement(&draft),
            as_the_sheet_holds_it(rows[shown].requirement),
            "{context}"
        );
        match save(&draft, rows, None) {
            SaveResult::Saved { result, resin } => {
                assert!(!result.changed, "{context} → {:?}", result.rows);
                assert_eq!(result.rows, rows, "{context}");
                assert_eq!(result.focus, Some(rows[entry.anchor()].key), "{context}");
                assert_eq!(resin, ResinOutcome::Unchanged, "{context}");
            }
            SaveResult::Refused { form, .. } => {
                assert_eq!(form.errors, [DUPLICATE_TRINKET], "{context}");
                let item = rows[shown].requirement.item;
                let named = rows
                    .iter()
                    .filter(|other| {
                        !other.requirement.blanket
                            && other.requirement.kind == ItemKind::Trinket
                            && other.requirement.item == item
                    })
                    .count();
                assert!(named > 1, "{context}");
            }
        }
    }
}

/// Every row of a list the editor wrote, or of a generated list once
/// normalized (1,024 lists, blankets, clusters and stacks among them),
/// saves back untouched exactly as it was.
#[test]
fn every_row_saves_back_untouched_exactly_as_it_was() {
    let mut rng = Rng::new(0x0dd_ba11_2026_0927);
    for case in 0..1024 {
        let rows = if case % 2 == 0 {
            editor_rows(&mut rng)
        } else {
            apply(&random_rows(&mut rng), None, &[Edit::Normalize]).rows
        };
        let context = format!("case {case}: {rows:?}");
        assert_emittable(&rows, &context);
        assert_untouched_saves_change_nothing(&rows, &context);
    }
}

/// Drafts no sheet would produce — any field any value, arbitrary rows,
/// broken keys — never make a change, the form or a save panic, and the
/// form still keeps every control in range (1,024 cases).
#[test]
fn arbitrary_drafts_never_panic_and_keep_the_form_in_range() {
    let mut rng = Rng::new(0xdead_d4af_7000_0001);
    let wild = |rng: &mut Rng| rng.pick(&[0, 1, 2, 3, 5, 9, 15, 24, 30, u8::MAX]);
    for case in 0..1024 {
        let mut rows = mixed_rows(&mut rng);
        for row in &mut rows {
            if rng.chance(10) {
                row.key = rng.pick(&[0, 1, u64::MAX]);
            }
        }
        let mut draft = random_sheet(&mut rng, &rows);
        if rng.chance(50) {
            draft.requirement = arbitrary_requirement(&mut rng);
        }
        if rng.chance(30) {
            draft.count = wild(&mut rng);
        }
        if rng.chance(30) {
            draft.total = Some(wild(&mut rng));
        }
        if rng.chance(30) {
            draft.copy_depth = Some(wild(&mut rng));
        }
        if rng.chance(30) {
            draft.tier_value = wild(&mut rng);
            draft.upgrade_value = wild(&mut rng);
            draft.floor_limit_memory = wild(&mut rng);
            draft.copy_depth_memory = wild(&mut rng);
            draft.transmutations_memory = wild(&mut rng);
        }
        if rng.chance(30) {
            draft.in_cluster = rng.chance(50);
            draft.blanket = rng.chance(50);
            draft.offer_resin = rng.chance(50);
            draft.resin_picked = rng.chance(50);
            draft.effect_mode = rng.pick(&EffectMode::ALL);
        }
        if rng.chance(30) {
            draft.resin.amount = rng.pick(&[None, Some(f64::NAN), Some(-1.0), Some(1e9)]);
            draft.resin.max_depth = Some(wild(&mut rng));
        }
        if rng.chance(20) {
            let key = rng.next() % 12;
            draft.origin = rng.pick(&[Origin::New, Origin::Resin, Origin::Row(key)]);
            draft.key = rng.chance(50).then(|| rng.pick(&[0, 1, 7, u64::MAX]));
        }
        let mut steps = Vec::new();
        for _ in 0..rng.range(0, 8) {
            let step = random_change(&mut rng, &draft);
            steps.push(step);
            draft = change(&draft, &step);
            let context = format!("case {case}: {steps:?} {draft:?}");
            assert_in_range(&form(&draft), false, &context);
        }
        let _ = save(&draft, &rows, rng.chance(30).then(|| rng.next()));
        if let SaveResult::Saved { result, .. } = save(&draft, &draft.rows, None) {
            let mut unique = keys(&result.rows);
            unique.sort_unstable();
            unique.dedup();
            assert_eq!(unique.len(), result.rows.len(), "case {case}");
            assert!(result.rows.iter().all(|row| is_valid_key(row.key)));
        }
    }
}
