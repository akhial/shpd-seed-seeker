//! The board's fold and edits.
//!
//! These port the union of the platform relation suites — web
//! `relations.test.ts` and the relation cases of `blankets`, `trinkets` and
//! `artifacts.test.tsx`; Android `RelationsTest.kt` (with the #190 cases and
//! the four stack documents the web writes); Swift `RelationsTests.swift`;
//! Windows `QueryRelationshipsTests.cs`; Linux `relations.rs` — deduplicated
//! and expressed in keys. Where the older suites pinned the pre-#190 join
//! (a stack following its chip into a cluster of another category), the
//! cases pin #190's refusal instead and say so.

use super::super::testing::{
    Rng, assert_emittable, named, query, random_edit, random_rows, requirements, row, validate,
    with,
};
use super::*;
use crate::catalog::{ItemId, ItemKind, WeaponCategory};
use crate::editor::{
    STACK_MAX, can_change_count, can_count_levels, can_grow, copy_depth, level_capacity,
};
use crate::query::{LevelSum, QueryError, Requirement, TierRequirement, UpgradeRequirement};

fn keys(rows: &[Row]) -> Vec<u64> {
    rows.iter().map(|row| row.key).collect()
}

fn run(rows: &[Row], edits: &[Edit]) -> EditResult {
    apply(rows, None, edits)
}

/// The rows after `edits`, which must all apply.
fn edited(rows: &[Row], edits: &[Edit]) -> Vec<Row> {
    let result = run(rows, edits);
    assert_eq!(result.refused, None, "{edits:?}");
    result.rows
}

/// The board entry showing or hiding the row `key`.
fn entry(rows: &[Row], key: u64) -> BoardItem {
    let index = index_of(rows, key).expect("the key is in the list");
    board_items(rows)
        .into_iter()
        .find(|item| item.members.contains(&index) || item.extras.contains(&index))
        .expect("an entry holds the row")
}

fn members(rows: &[Row]) -> Vec<Vec<usize>> {
    board_items(rows)
        .into_iter()
        .map(|item| item.members)
        .collect()
}

fn counts(rows: &[Row]) -> Vec<usize> {
    board_items(rows).iter().map(BoardItem::count).collect()
}

fn exact(row: Row, upgrade: u8) -> Row {
    with(row, |r| r.upgrade = UpgradeRequirement::Exact(upgrade))
}

fn at_least(row: Row, upgrade: u8) -> Row {
    with(row, |r| r.upgrade = UpgradeRequirement::AtLeast(upgrade))
}

fn floor(row: Row, depth: u8) -> Row {
    with(row, |r| r.max_depth = Some(depth))
}

fn sum(row: Row, group: u8, minimum_total: u8) -> Row {
    with(row, |r| {
        r.level_sum = Some(LevelSum {
            group,
            minimum_total,
        });
    })
}

/// A new chip saved from the sheet with a stack shape.
fn saved(requirement: Requirement, count: u8, total: Option<u8>, copy_depth: Option<u8>) -> Edit {
    Edit::Save {
        key: None,
        requirement,
        count,
        total,
        copy_depth,
    }
}

/// An edit of the row `key` from the sheet.
fn resaved(
    key: u64,
    requirement: Requirement,
    count: u8,
    total: Option<u8>,
    copy_depth: Option<u8>,
) -> Edit {
    Edit::Save {
        key: Some(key),
        requirement,
        count,
        total,
        copy_depth,
    }
}

fn ring(upgrade: Option<u8>) -> Requirement {
    let might = named(0, ItemId::RingMight);
    upgrade
        .map_or(might, |upgrade| exact(might, upgrade))
        .requirement
}

#[cfg(feature = "json-query")]
fn document(rows: &[Row]) -> serde_json::Value {
    crate::json_query::encode(&query(requirements(rows)))
}

/// The list after a trip through the query document, keyed 1..=n.
#[cfg(feature = "json-query")]
fn reloaded(rows: &[Row]) -> Vec<Row> {
    let text = document(rows).to_string();
    crate::json_query::decode_unvalidated(&text)
        .expect("the document decodes")
        .requirements
        .into_iter()
        .zip(1..)
        .map(|(requirement, key)| Row { key, requirement })
        .collect()
}

// --- either/or clusters -------------------------------------------------

#[test]
fn dropping_a_chip_on_another_makes_one_slot_placed_after_the_target() {
    let base = [
        named(1, ItemId::Spear),
        row(2, ItemKind::Armor),
        named(3, ItemId::Shuriken),
    ];
    let result = run(
        &base,
        &[Edit::Join {
            source: 3,
            target: 1,
        }],
    );
    assert!(result.changed);
    assert_eq!(result.focus, Some(3));
    assert_eq!(keys(&result.rows), [1, 3, 2]);
    let group = result.rows[0].requirement.alternative_group;
    assert!(group.is_some());
    assert_eq!(result.rows[1].requirement.alternative_group, group);
    assert_eq!(members(&result.rows), [vec![0, 1], vec![2]]);
    assert_eq!(validate(&result.rows), Ok(()));
    #[cfg(feature = "json-query")]
    assert!(document(&result.rows)["requirements"][0]["any_of"].is_array());

    // Dragging the first chip onto the last moves it next to its target.
    let start = [
        named(1, ItemId::RingMight),
        exact(named(2, ItemId::Sword), 1),
        exact(row(3, ItemKind::Wand), 3),
    ];
    let joined = edited(
        &start,
        &[Edit::Join {
            source: 1,
            target: 3,
        }],
    );
    assert_eq!(keys(&joined), [2, 3, 1]);
    assert_eq!(
        joined
            .iter()
            .map(|row| row.requirement.alternative_group)
            .collect::<Vec<_>>(),
        [None, Some(1), Some(1)]
    );
    assert_eq!(query(requirements(&joined)).slot_count(), 2);
    assert_eq!(board_items(&joined).len(), 2);
}

/// The web suite joined a combined-level pair onto a shuriken. Under #190
/// that mixed-category join of a stack is refused; within one category the
/// join still drops the level sum — and, canonically until the follow-up,
/// leaves the other member behind as a chip of its own.
#[test]
fn joining_a_combined_level_stack_drops_its_total_and_leaving_a_pair_dissolves_it() {
    let pair = [
        sum(named(1, ItemId::RingMight), 1, 3),
        sum(named(2, ItemId::RingMight), 1, 3),
        named(3, ItemId::Shuriken),
    ];
    let refused = run(
        &pair,
        &[Edit::Join {
            source: 1,
            target: 3,
        }],
    );
    assert_eq!(refused.refused, Some(Refusal::MixedCategoryStack));
    assert!(!refused.changed);
    assert_eq!(refused.rows, pair);

    let rings = [
        sum(named(1, ItemId::RingMight), 1, 3),
        sum(named(2, ItemId::RingMight), 1, 3),
        named(3, ItemId::RingWealth),
    ];
    let joined = edited(
        &rings,
        &[Edit::Join {
            source: 1,
            target: 3,
        }],
    );
    assert!(joined.iter().all(|row| row.requirement.level_sum.is_none()));
    assert_eq!(keys(&joined), [2, 3, 1]);
    assert_eq!(counts(&joined), [1, 1]);
    assert_eq!(validate(&joined), Ok(()));
    let out = edited(&joined, &[Edit::Detach { key: 3 }]);
    assert!(
        out.iter()
            .all(|row| row.requirement.alternative_group.is_none())
    );
}

// --- stacks ---------------------------------------------------------------

#[test]
fn a_concrete_stack_encodes_as_plain_repeats_with_no_identity_group() {
    let base = [
        exact(named(1, ItemId::RingMight), 2),
        row(2, ItemKind::Wand),
    ];
    let result = run(&base, &[Edit::SetCount { key: 1, count: 3 }]);
    assert_eq!(result.focus, Some(1));
    let next = result.rows;
    assert_eq!(keys(&next), [1, 3, 4, 2]);
    assert_eq!(
        next.iter()
            .filter(|row| row.requirement.item == Some(ItemId::RingMight))
            .count(),
        3
    );
    assert!(
        next.iter()
            .all(|row| row.requirement.identity_group.is_none())
    );
    // The copies name the same item and nothing else.
    assert_eq!(
        next[..3]
            .iter()
            .map(|row| row.requirement.upgrade)
            .collect::<Vec<_>>(),
        [
            UpgradeRequirement::Exact(2),
            UpgradeRequirement::Any,
            UpgradeRequirement::Any
        ]
    );
    // The board folds the repeats back into one ×3 chip.
    let board = board_items(&next);
    assert_eq!(board.len(), 2);
    assert_eq!(board[0].count(), 3);
    assert_eq!(board[0].total, None);
    assert_eq!(validate(&next), Ok(()));
    assert_emittable(&next, "concrete stack");
    // The round trip through the document keeps the stack.
    #[cfg(feature = "json-query")]
    assert_eq!(counts(&reloaded(&next)), [3, 1]);
}

#[test]
fn a_wildcard_stack_encodes_as_bare_copies_sharing_an_identity_group() {
    let base = [at_least(row(1, ItemKind::Wand), 1)];
    let next = edited(&base, &[Edit::SetCount { key: 1, count: 3 }]);
    assert_eq!(next.len(), 3);
    assert!(
        next.iter()
            .all(|row| row.requirement.identity_group == Some(1))
    );
    assert!(next[1..].iter().all(|row| {
        row.requirement.kind == ItemKind::Wand
            && row.requirement.item.is_none()
            && row.requirement.upgrade == UpgradeRequirement::Any
            && row.requirement.is_bare()
    }));
    assert_eq!(validate(&next), Ok(()));
    assert_eq!(counts(&next), [3]);
    // Shrinking to one dissolves the group entirely.
    let shrunk = edited(&next, &[Edit::SetCount { key: 1, count: 1 }]);
    assert_eq!(keys(&shrunk), [1]);
    assert_eq!(shrunk[0].requirement.identity_group, None);
}

#[test]
fn a_narrowed_weapon_stack_grows_by_family_wide_copies_the_engine_reads_as_bare() {
    let melee = with(row(1, ItemKind::Weapon), |r| {
        r.weapon_category = Some(WeaponCategory::Melee);
        r.tier = TierRequirement::Exact(4);
        r.upgrade = UpgradeRequirement::Exact(5);
    });
    let next = edited(&[melee], &[Edit::SetCount { key: 1, count: 3 }]);
    assert_eq!(
        next[0].requirement.weapon_category,
        Some(WeaponCategory::Melee)
    );
    assert!(next[1..].iter().all(|row| {
        row.requirement.kind == ItemKind::Weapon
            && row.requirement.weapon_category.is_none()
            && row.requirement.is_bare()
    }));
    assert!(
        next.iter()
            .all(|row| row.requirement.identity_group == Some(1))
    );
    assert_eq!(validate(&next), Ok(()));
    // The board folds the copies back into the anchor's badge.
    assert_eq!(counts(&next), [3]);

    let thrown = with(row(1, ItemKind::Weapon), |r| {
        r.weapon_category = Some(WeaponCategory::Thrown);
        r.upgrade = UpgradeRequirement::AtLeast(1);
    });
    let next = edited(&[], &[saved(thrown.requirement, 2, None, None)]);
    assert_eq!(next[1].requirement.weapon_category, None);
    assert_eq!(validate(&next), Ok(()));
}

#[test]
fn a_narrowed_named_stack_keeps_its_narrowing_on_every_repeat() {
    // Plain repeats keep the melee/thrown kind (Linux dropped it), and a
    // narrowed repeat still folds: the plain-copy check ignores it.
    let spear = with(exact(named(1, ItemId::Spear), 2), |r| {
        r.weapon_category = Some(WeaponCategory::Melee);
    });
    let next = edited(&[spear], &[Edit::SetCount { key: 1, count: 2 }]);
    assert_eq!(
        next[1].requirement.weapon_category,
        Some(WeaponCategory::Melee)
    );
    assert_eq!(next[1].requirement.item, Some(ItemId::Spear));
    assert_eq!(counts(&next), [2]);
    assert_eq!(validate(&next), Ok(()));
}

/// Swift copied the anchor into its copies, resin exclusion included, so
/// excluded copies escaped the stack and took resin budget.
#[test]
fn resin_exclusions_stay_on_the_anchor_and_excluded_repeats_are_not_hidden() {
    for item_id in [None, Some(ItemId::WandLightning)] {
        let anchor = with(row(1, ItemKind::Wand), |r| {
            r.item = item_id;
            r.exclude_resin = true;
        });
        let grown = edited(&[anchor], &[Edit::SetCount { key: 1, count: 3 }]);
        assert_eq!(
            grown
                .iter()
                .map(|row| row.requirement.exclude_resin)
                .collect::<Vec<_>>(),
            [true, false, false]
        );
        assert_eq!(counts(&grown), [3]);
        assert_eq!(validate(&grown), Ok(()));
    }
    let ordinary = named(1, ItemId::WandLightning);
    let excluded = with(named(2, ItemId::WandLightning), |r| r.exclude_resin = true);
    assert_eq!(counts(&[ordinary, excluded]), [1, 1]);
}

#[test]
fn copies_never_carry_the_anchors_blanket_selection_or_transmutations() {
    // Copies are built from the defaults, not cloned from the anchor.
    let anchor = with(exact(named(1, ItemId::WandFrost), 2), |r| {
        r.source = Some(crate::model::ItemSource::Shop);
        r.require_uncursed = true;
        r.max_depth = Some(6);
    });
    let grown = edited(&[anchor], &[Edit::SetCount { key: 1, count: 2 }]);
    assert_eq!(
        grown[1].requirement,
        Requirement {
            item: Some(ItemId::WandFrost),
            ..Requirement::any(ItemKind::Wand)
        }
    );
}

#[test]
fn an_either_or_cluster_anchors_a_stack_and_every_member_carries_the_label() {
    let base = edited(
        &[named(1, ItemId::RunicBlade), named(2, ItemId::WarHammer)],
        &[Edit::Join {
            source: 2,
            target: 1,
        }],
    );
    let next = edited(&base, &[Edit::SetCount { key: 1, count: 3 }]);
    assert_eq!(next.len(), 4);
    assert!(
        next.iter()
            .all(|row| row.requirement.identity_group == Some(1))
    );
    assert_eq!(
        next.iter()
            .filter(|row| row.requirement.alternative_group.is_some())
            .count(),
        2
    );
    assert_eq!(validate(&next), Ok(()));
    let board = board_items(&next);
    assert_eq!(board.len(), 1);
    assert!(board[0].cluster.is_some());
    assert_eq!(board[0].count(), 3);
    // The count badge works from any member.
    assert_eq!(
        edited(&next, &[Edit::SetCount { key: 2, count: 2 }]).len(),
        3
    );
    // Removing one cluster member keeps the stack on the survivor.
    let dissolved = edited(&next, &[Edit::Remove { key: 2 }]);
    assert_eq!(counts(&dissolved), [3]);
    assert_eq!(validate(&dissolved), Ok(()));

    // A wildcard and a named wand make a cluster the same way.
    let wands = edited(
        &[
            exact(row(1, ItemKind::Wand), 3),
            exact(named(2, ItemId::WandFireblast), 3),
        ],
        &[
            Edit::Join {
                source: 1,
                target: 2,
            },
            Edit::SetCount { key: 2, count: 3 },
        ],
    );
    assert!(
        wands
            .iter()
            .all(|row| row.requirement.identity_group == Some(1))
    );
    let item = &board_items(&wands)[0];
    assert_eq!((item.members.len(), item.count()), (2, 3));
    assert_eq!(validate(&wands), Ok(()));
}

#[test]
fn a_plain_repeat_stack_trades_its_copies_for_labels_when_it_joins_a_cluster() {
    let base = edited(
        &[named(1, ItemId::Spear), named(2, ItemId::Mace)],
        &[Edit::SetCount { key: 1, count: 2 }],
    );
    let next = edited(
        &base,
        &[Edit::Join {
            source: 2,
            target: 1,
        }],
    );
    // The copy is now a bare weapon tied to the whole cluster.
    let copies: Vec<&Row> = next
        .iter()
        .filter(|row| row.requirement.item.is_none())
        .collect();
    assert_eq!(copies.len(), 1);
    assert!(copies[0].requirement.identity_group.is_some());
    assert!(
        next.iter()
            .filter(|row| row.requirement.alternative_group.is_some())
            .all(|row| row.requirement.identity_group == copies[0].requirement.identity_group)
    );
    assert_eq!(validate(&next), Ok(()));

    // The same holds for rings, with the counted chip as the source.
    let rings = edited(
        &[
            exact(named(1, ItemId::RingMight), 2),
            exact(named(9, ItemId::RingEnergy), 1),
        ],
        &[
            Edit::SetCount { key: 1, count: 2 },
            Edit::Join {
                source: 1,
                target: 9,
            },
        ],
    );
    assert_eq!(validate(&rings), Ok(()));
    assert_eq!(counts(&rings), [2]);
}

/// Android #190: a stack cannot follow its chip into a cluster spanning
/// categories, so the drop is refused and the stack stays intact.
#[test]
fn a_cross_category_drop_leaves_a_labelled_stack_intact() {
    let stacked = edited(
        &[
            exact(row(1, ItemKind::Weapon), 3),
            exact(named(9, ItemId::WandFireblast), 3),
        ],
        &[Edit::SetCount { key: 1, count: 3 }],
    );
    assert_eq!(
        stacked
            .iter()
            .filter(|row| row.requirement.identity_group.is_some())
            .count(),
        3
    );
    for (source, target) in [(1, 9), (9, 1)] {
        let result = run(&stacked, &[Edit::Join { source, target }]);
        assert_eq!(result.refused, Some(Refusal::MixedCategoryStack));
        assert!(!result.changed);
        assert_eq!(result.rows, stacked);
        assert_eq!(
            drop_action(&stacked, source, DropTarget::Row(target)),
            DropAction::Refuse(Refusal::MixedCategoryStack)
        );
    }
}

#[test]
fn dropping_armor_on_a_counted_ring_does_not_split_off_its_copies() {
    for count in 2..=3 {
        let mut stacked = edited(
            &[],
            &[saved(
                exact(named(0, ItemId::RingEnergy), 4).requirement,
                count,
                None,
                Some(20),
            )],
        );
        stacked.push(exact(named(9, ItemId::PlateArmor), 3));
        for (source, target) in [(9, 1), (1, 9)] {
            let result = run(&stacked, &[Edit::Join { source, target }]);
            assert_eq!(result.refused, Some(Refusal::MixedCategoryStack));
            assert_eq!(result.rows, stacked);
        }
    }
}

/// Android #190: another chip may name the same ring with its own stack;
/// joining one chip trades only the repeats of its own entry.
#[test]
fn joining_a_ring_only_takes_the_copies_belonging_to_that_chip() {
    let counted = {
        let mut rows = edited(
            &[exact(named(1, ItemId::RingMight), 2)],
            &[Edit::SetCount { key: 1, count: 2 }],
        );
        rows.push(exact(named(9, ItemId::RingMight), 4));
        rows.push(exact(named(10, ItemId::RingEnergy), 1));
        rows
    };
    assert_eq!(keys(&counted), [1, 2, 9, 10]);
    for (source, target) in [(9, 10), (10, 9)] {
        let joined = edited(&counted, &[Edit::Join { source, target }]);
        let untouched = entry(&joined, 1);
        assert_eq!(untouched.count(), 2);
        assert_eq!(
            untouched
                .members
                .iter()
                .chain(&untouched.extras)
                .map(|&index| joined[index])
                .collect::<Vec<_>>(),
            counted[..2]
        );
        let cluster = entry(&joined, 9);
        assert!(cluster.cluster.is_some());
        assert_eq!(cluster.count(), 1);
        assert_eq!(validate(&joined), Ok(()));
    }
}

#[test]
fn dropping_another_ring_on_a_counted_ring_keeps_its_copies_and_their_floor_limit() {
    let mut counted = edited(
        &[],
        &[saved(
            exact(named(0, ItemId::RingEnergy), 4).requirement,
            3,
            None,
            Some(9),
        )],
    );
    counted.push(exact(named(9, ItemId::RingMight), 2));
    let joined = edited(
        &counted,
        &[Edit::Join {
            source: 9,
            target: 1,
        }],
    );
    let board = board_items(&joined);
    assert_eq!(board.len(), 1);
    let item = &board[0];
    assert_eq!(
        item.members
            .iter()
            .map(|&index| joined[index].key)
            .collect::<Vec<_>>(),
        [1, 9]
    );
    assert_eq!(item.count(), 3);
    assert_eq!(
        item.extras
            .iter()
            .map(|&index| joined[index].requirement.max_depth)
            .collect::<Vec<_>>(),
        [Some(9), Some(9)]
    );
    assert_eq!(
        joined[item.anchor()].requirement.upgrade,
        UpgradeRequirement::Exact(4)
    );
    assert_eq!(validate(&joined), Ok(()));
}

#[test]
fn a_cluster_spanning_two_categories_cannot_grow_a_stack() {
    let mixed = edited(
        &[row(1, ItemKind::Wand), named(2, ItemId::Spear)],
        &[Edit::Join {
            source: 1,
            target: 2,
        }],
    );
    let cluster = entry(&mixed, 1);
    assert_eq!(cluster.members.len(), 2);
    assert!(!can_grow(&mixed, &cluster));
    assert!(!can_change_count(&mixed, &cluster));
    let result = run(&mixed, &[Edit::SetCount { key: 1, count: 2 }]);
    assert!(!result.changed);
    assert_eq!(result.rows, mixed);
    // A cluster of one category is still free to stack.
    let weapons = edited(
        &[named(1, ItemId::Mace), named(2, ItemId::Spear)],
        &[Edit::Join {
            source: 1,
            target: 2,
        }],
    );
    assert!(can_grow(&weapons, &entry(&weapons, 1)));
}

#[test]
fn a_counted_ring_cannot_join_the_ring_member_of_a_mixed_category_cluster() {
    let mixed = edited(
        &[
            exact(named(1, ItemId::RingMight), 2),
            exact(row(2, ItemKind::Wand), 3),
        ],
        &[Edit::Join {
            source: 2,
            target: 1,
        }],
    );
    let mut counted = mixed;
    counted.push(exact(named(9, ItemId::RingMight), 2));
    let counted = edited(&counted, &[Edit::SetCount { key: 9, count: 3 }]);
    assert_eq!(
        drop_action(&counted, 9, DropTarget::Row(1)),
        DropAction::Refuse(Refusal::MixedCategoryStack)
    );
    let result = run(
        &counted,
        &[Edit::Join {
            source: 9,
            target: 1,
        }],
    );
    assert_eq!(result.refused, Some(Refusal::MixedCategoryStack));
    assert_eq!(result.rows, counted);
}

#[test]
fn deleting_the_anchor_deletes_its_copies_and_leaves_no_stale_groups() {
    let wildcard = edited(
        &[row(1, ItemKind::Wand), row(2, ItemKind::Armor)],
        &[Edit::SetCount { key: 1, count: 3 }],
    );
    let after = edited(&wildcard, &[Edit::RemoveItem { key: 1 }]);
    assert_eq!(keys(&after), [2]);
    assert_eq!(after[0].requirement.kind, ItemKind::Armor);
    assert!(
        after
            .iter()
            .all(|row| row.requirement.identity_group.is_none())
    );
    // Removing a lone chip removes its entry too.
    assert_eq!(keys(&edited(&wildcard, &[Edit::Remove { key: 1 }])), [2]);

    let total = edited(
        &[named(1, ItemId::RingMight)],
        &[
            Edit::SetCount { key: 1, count: 2 },
            Edit::SetTotal {
                key: 1,
                total: Some(3),
            },
        ],
    );
    let result = run(&total, &[Edit::RemoveItem { key: 1 }]);
    assert!(result.rows.is_empty());
    assert_eq!(result.focus, None);
}

#[test]
fn ejecting_a_member_from_a_stacked_cluster_strips_its_label() {
    let base = edited(
        &[named(1, ItemId::Spear), named(2, ItemId::Mace)],
        &[
            Edit::Join {
                source: 2,
                target: 1,
            },
            Edit::SetCount { key: 1, count: 2 },
        ],
    );
    let result = run(&base, &[Edit::Detach { key: 1 }]);
    assert_eq!(result.focus, Some(1));
    let spear = result.rows[index_of(&result.rows, 1).unwrap()].requirement;
    assert_eq!(spear.alternative_group, None);
    assert_eq!(spear.identity_group, None);
    assert_eq!(validate(&result.rows), Ok(()));

    let wands = edited(
        &[
            exact(row(1, ItemKind::Wand), 3),
            exact(named(2, ItemId::WandFireblast), 3),
        ],
        &[
            Edit::Join {
                source: 1,
                target: 2,
            },
            Edit::SetCount { key: 1, count: 2 },
            Edit::Detach { key: 2 },
        ],
    );
    let loose = wands[index_of(&wands, 2).unwrap()].requirement;
    assert_eq!(
        (loose.alternative_group, loose.identity_group),
        (None, None)
    );
    assert_eq!(validate(&wands), Ok(()));
}

#[test]
fn detach_and_remove_apply_to_cluster_members_only_as_members() {
    let lone = [named(1, ItemId::Spear), named(2, ItemId::Mace)];
    let result = run(&lone, &[Edit::Detach { key: 1 }]);
    assert!(!result.changed);
    assert_eq!(result.focus, None);
    // A hidden copy is never a target, even for a removal.
    let stacked = edited(&lone, &[Edit::SetCount { key: 1, count: 2 }]);
    let copy = stacked[1].key;
    for edit in [
        Edit::Remove { key: copy },
        Edit::RemoveItem { key: copy },
        Edit::Detach { key: copy },
        Edit::SetCount {
            key: copy,
            count: 3,
        },
    ] {
        assert!(!run(&stacked, &[edit]).changed, "{edit:?}");
    }
}

// --- combined levels ----------------------------------------------------

#[test]
fn a_total_turns_the_stack_into_identical_optional_members() {
    let base = edited(
        &[exact(named(1, ItemId::RingMight), 2)],
        &[Edit::SetCount { key: 1, count: 2 }],
    );
    let next = edited(
        &base,
        &[Edit::SetTotal {
            key: 1,
            total: Some(3),
        }],
    );
    assert_eq!(next.len(), 2);
    assert!(next.iter().all(|row| row.requirement.level_sum
        == Some(LevelSum {
            group: 1,
            minimum_total: 3
        })));
    // The total speaks for the stack: per-member upgrades reset to any.
    assert!(
        next.iter()
            .all(|row| row.requirement.upgrade == UpgradeRequirement::Any
                && row.requirement.identity_group.is_none())
    );
    let board = board_items(&next);
    assert_eq!(board.len(), 1);
    assert_eq!(board[0].total, Some(3));
    assert_eq!(board[0].count(), 2);
    assert_eq!(validate(&next), Ok(()));
    #[cfg(feature = "json-query")]
    assert_eq!(
        document(&next)["requirements"][0]["level_sum"],
        serde_json::json!({"group": 1, "at_least": 3})
    );
    // Clearing the total returns to plain repeats.
    let cleared = edited(
        &next,
        &[Edit::SetTotal {
            key: 1,
            total: None,
        }],
    );
    assert!(
        cleared
            .iter()
            .all(|row| row.requirement.level_sum.is_none())
    );
    assert_eq!(counts(&cleared), [2]);
}

#[test]
fn only_a_ring_stack_counts_levels_but_a_stale_sum_still_clears() {
    // A wand's or a spear's power does not add up the way a ring's does, so
    // a total changes nothing and the stack stays plain repeats.
    for item_id in [ItemId::WandFireblast, ItemId::Spear] {
        let stacked = edited(
            &[exact(named(1, item_id), 3)],
            &[Edit::SetCount { key: 1, count: 2 }],
        );
        assert!(!can_count_levels(&stacked, &entry(&stacked, 1)));
        for edit in [
            Edit::SetTotal {
                key: 1,
                total: Some(3),
            },
            Edit::SetTotal {
                key: 1,
                total: None,
            },
            Edit::ToggleLevels { key: 1 },
        ] {
            let result = run(&stacked, &[edit]);
            assert!(!result.changed, "{edit:?}");
            assert_eq!(result.refused, None);
        }
    }
    // A stale non-ring sum from a hand-written document still clears.
    let loaded = [
        sum(named(1, ItemId::WandFireblast), 1, 3),
        sum(named(2, ItemId::WandFireblast), 1, 3),
    ];
    assert!(can_count_levels(&loaded, &entry(&loaded, 1)));
    let cleared = edited(&loaded, &[Edit::ToggleLevels { key: 1 }]);
    assert!(
        cleared
            .iter()
            .all(|row| row.requirement.level_sum.is_none())
    );
    assert_eq!(counts(&cleared), [2]);
    assert_eq!(validate(&cleared), Ok(()));
}

#[test]
fn a_loaded_level_sum_document_collapses_back_into_one_chip() {
    let loaded = [
        sum(named(1, ItemId::RingMight), 2, 4),
        sum(named(2, ItemId::RingMight), 2, 4),
        row(3, ItemKind::Wand),
    ];
    let board = board_items(&loaded);
    assert_eq!(board.len(), 2);
    assert_eq!(board[0].total, Some(4));
    assert_eq!(board[0].count(), 2);
    assert_eq!(board[0].extras, [1]);
    #[cfg(feature = "json-query")]
    {
        let document = r#"{"requirements":[
            {"kind":"ring","item":"ring_might","level_sum":{"group":2,"at_least":4}},
            {"kind":"ring","item":"ring_might","level_sum":{"group":2,"at_least":4}},
            {"kind":"wand"}]}"#;
        let decoded = crate::json_query::decode(document).unwrap().requirements;
        let board = board_items(&decoded);
        assert_eq!(board.len(), 2);
        assert_eq!((board[0].total, board[0].count()), (Some(4), 2));
    }
}

#[test]
fn totals_clamp_to_what_the_stack_can_reach_and_toggle_from_the_count() {
    let three = edited(
        &[named(1, ItemId::RingMight)],
        &[Edit::SetCount { key: 1, count: 3 }],
    );
    // Three any-upgrade rings reach 5 + 3 + 3 levels: one vault ring.
    assert_eq!(level_capacity(&three, &entry(&three, 1)), 11);
    let high = edited(
        &three,
        &[Edit::SetTotal {
            key: 1,
            total: Some(99),
        }],
    );
    assert_eq!(entry(&high, 1).total, Some(11));
    let low = edited(
        &three,
        &[Edit::SetTotal {
            key: 1,
            total: Some(0),
        }],
    );
    assert_eq!(entry(&low, 1).total, Some(1));
    // Setting the total it already has changes nothing.
    assert!(
        !run(
            &high,
            &[Edit::SetTotal {
                key: 1,
                total: Some(11)
            }]
        )
        .changed
    );
    // Turning counting on starts at one level per item; off clears it.
    let toggled = edited(&three, &[Edit::ToggleLevels { key: 1 }]);
    assert_eq!(entry(&toggled, 1).total, Some(3));
    let off = edited(&toggled, &[Edit::ToggleLevels { key: 1 }]);
    assert_eq!(entry(&off, 1).total, None);
    assert_eq!(counts(&off), [3]);
    // A lone ring cannot count levels: its upgrade would silently vanish.
    let lone = [exact(named(1, ItemId::RingMight), 2)];
    assert!(
        !run(
            &lone,
            &[Edit::SetTotal {
                key: 1,
                total: Some(2)
            }]
        )
        .changed
    );
}

/// A blanket constrains the items ordinary requirements reserve; it cannot
/// count levels (every platform but the web already refused).
#[test]
fn a_blanket_refuses_a_total_and_never_stacks() {
    let blanket = [with(named(1, ItemId::RingMight), |r| r.blanket = true)];
    for edit in [
        Edit::SetTotal {
            key: 1,
            total: Some(3),
        },
        Edit::SetTotal {
            key: 1,
            total: None,
        },
        Edit::ToggleLevels { key: 1 },
    ] {
        let result = run(&blanket, &[edit]);
        assert_eq!(result.refused, Some(Refusal::BlanketTotal), "{edit:?}");
        assert_eq!(result.rows, blanket);
    }
    assert_eq!(Refusal::BlanketTotal.name(), "blanket_total");
    assert!(!can_grow(&blanket, &entry(&blanket, 1)));
    assert!(!run(&blanket, &[Edit::SetCount { key: 1, count: 2 }]).changed);
}

// --- the editor round trip ----------------------------------------------

#[test]
fn the_editor_applies_count_and_total_and_rebuilds_the_stack() {
    let mut rows = edited(&[], &[saved(ring(None), 2, Some(3), None)]);
    assert_eq!(keys(&rows), [1, 2]);
    assert!(
        rows.iter()
            .all(|row| row.requirement.level_sum.map(|sum| sum.minimum_total) == Some(3))
    );
    // Raising the count keeps the total; clearing it returns plain repeats.
    rows = edited(&rows, &[resaved(1, rows[0].requirement, 3, Some(5), None)]);
    assert_eq!(rows.len(), 3);
    assert!(
        rows.iter()
            .all(|row| row.requirement.level_sum.map(|sum| sum.minimum_total) == Some(5))
    );
    rows = edited(&rows, &[resaved(1, rows[0].requirement, 2, None, None)]);
    assert_eq!(rows.len(), 2);
    assert!(rows.iter().all(|row| row.requirement.level_sum.is_none()));
    assert!(
        rows.iter()
            .all(|row| row.requirement.item == Some(ItemId::RingMight))
    );
    assert_eq!(validate(&rows), Ok(()));

    // From an exact +2 ring: a total, then back to the lone +2 ring.
    let start = [exact(named(1, ItemId::RingMight), 2)];
    let edited_rows = edited(&start, &[resaved(1, ring(Some(2)), 3, Some(4), None)]);
    assert_eq!(edited_rows.len(), 3);
    assert!(
        edited_rows
            .iter()
            .all(|row| row.requirement.level_sum.map(|sum| sum.minimum_total) == Some(4))
    );
    assert_eq!(validate(&edited_rows), Ok(()));
    let shrunk = edited(&edited_rows, &[resaved(1, ring(Some(2)), 1, None, None)]);
    assert_eq!(shrunk.len(), 1);
    assert_eq!(shrunk[0].requirement.level_sum, None);
    assert_eq!(shrunk[0].requirement.upgrade, UpgradeRequirement::Exact(2));
}

#[test]
fn the_editor_rebuilds_the_copies_when_the_edit_changes_the_category() {
    let rows = edited(
        &[],
        &[saved(Requirement::any(ItemKind::Wand), 3, None, None)],
    );
    assert!(
        rows.iter()
            .all(|row| row.requirement.kind == ItemKind::Wand)
    );
    // The old copies named wands; the edited chip asks for rings, so the
    // stack comes down and is rebuilt rather than keeping stale wands.
    let result = run(
        &rows,
        &[resaved(1, Requirement::any(ItemKind::Ring), 3, None, None)],
    );
    assert_eq!(result.rows.len(), 3);
    assert!(
        result
            .rows
            .iter()
            .all(|row| row.requirement.kind == ItemKind::Ring)
    );
    // The rebuilt copies keep the removed copies' keys.
    assert_eq!(keys(&result.rows), keys(&rows));
    assert_eq!(result.focus, Some(1));
    assert_eq!(validate(&result.rows), Ok(()));
}

#[test]
fn shrinking_a_level_sum_stack_from_the_editor_drops_its_orphaned_members() {
    let rows = edited(&[], &[saved(ring(None), 3, Some(4), None)]);
    assert_eq!(rows.len(), 3);
    let shrunk = edited(&rows, &[resaved(1, ring(None), 1, None, None)]);
    assert_eq!(keys(&shrunk), [1]);
    assert_eq!(shrunk[0].requirement.level_sum, None);
}

/// Swift: the sheet hands back a plain row; the relationships are the
/// save's to write, so re-saving a combined-level stack from a row carrying
/// no level sum still lands on one.
#[test]
fn the_editors_plain_row_rebuilds_a_combined_level_stack() {
    let mut rows = edited(&[], &[saved(ring(None), 2, Some(4), None)]);
    assert_eq!(
        rows.iter()
            .map(|row| row.requirement.level_sum.map(|sum| sum.minimum_total))
            .collect::<Vec<_>>(),
        [Some(4), Some(4)]
    );
    rows = edited(&rows, &[resaved(1, ring(None), 3, Some(6), None)]);
    assert_eq!(rows.len(), 3);
    assert!(rows.iter().all(|row| row.requirement.level_sum
        == Some(LevelSum {
            group: 1,
            minimum_total: 6
        })));
    assert_eq!(validate(&rows), Ok(()));
    // Saving again with counting off returns plain repeats, and the copies
    // take the floor limit the sheet gave them.
    rows = edited(&rows, &[resaved(1, ring(None), 3, None, Some(4))]);
    assert!(rows.iter().all(|row| row.requirement.level_sum.is_none()));
    assert_eq!(
        rows.iter()
            .map(|row| row.requirement.max_depth)
            .collect::<Vec<_>>(),
        [None, Some(4), Some(4)]
    );
    assert_eq!(counts(&rows), [3]);
    assert_eq!(validate(&rows), Ok(()));
}

#[test]
fn a_new_chip_is_appended_with_a_fresh_or_pre_claimed_key() {
    let start = [named(1, ItemId::RingMight)];
    let result = run(
        &start,
        &[saved(Requirement::any(ItemKind::Wand), 1, None, None)],
    );
    assert_eq!(keys(&result.rows), [1, 2]);
    assert_eq!(result.focus, Some(2));
    assert_eq!(result.next_key, 3);
    assert_eq!(board_items(&result.rows).len(), 2);
    // A key the platform already claimed (Linux) is kept, and the counter
    // it passes keeps new copies clear of its other claims.
    let result = apply(
        &start,
        Some(10),
        &[resaved(7, Requirement::any(ItemKind::Wand), 2, None, None)],
    );
    assert_eq!(keys(&result.rows), [1, 7, 10]);
    assert_eq!(result.next_key, 11);
    // New rows land at the end whatever their section.
    let blanket = Requirement {
        blanket: true,
        ..Requirement::any(ItemKind::Wand)
    };
    let ordinary = Requirement::any(ItemKind::Armor);
    let rows = edited(
        &start,
        &[
            saved(blanket, 1, None, None),
            saved(ordinary, 1, None, None),
        ],
    );
    assert_eq!(
        rows.iter()
            .map(|row| row.requirement.blanket)
            .collect::<Vec<_>>(),
        [false, true, false]
    );
}

/// A list with the stack shape the sheet saves its first row with.
type Shape = (Vec<Row>, u8, Option<u8>, Option<u8>);

/// Android's refine plan compares whole requests, keys included: saving an
/// untouched chip must give back the very same rows.
#[test]
fn saving_an_unchanged_chip_gives_back_identical_rows_and_keys() {
    let spear = exact(named(1, ItemId::Spear), 3);
    // Rows, then the sheet's count, total and copy floor.
    let shapes: Vec<Shape> = vec![
        // [A, c, c, B]: a concrete stack with a copy floor, then another chip.
        (
            vec![
                spear,
                floor(named(2, ItemId::Spear), 6),
                floor(named(3, ItemId::Spear), 6),
                row(4, ItemKind::Armor),
            ],
            3,
            None,
            Some(6),
        ),
        // A wildcard stack under label 3 while labels 1 and 2 are free.
        (
            vec![
                with(at_least(row(5, ItemKind::Wand), 2), |r| {
                    r.identity_group = Some(3);
                }),
                with(row(6, ItemKind::Wand), |r| r.identity_group = Some(3)),
            ],
            2,
            None,
            None,
        ),
        // A combined-level stack under label 4.
        (
            vec![
                sum(named(7, ItemId::RingMight), 4, 6),
                sum(named(8, ItemId::RingMight), 4, 6),
                sum(named(9, ItemId::RingMight), 4, 6),
            ],
            3,
            Some(6),
            None,
        ),
    ];
    for (rows, count, total, copy_depth) in shapes {
        assert_eq!(validate(&rows), Ok(()));
        let anchor = rows[0];
        let result = run(
            &rows,
            &[resaved(
                anchor.key,
                anchor.requirement,
                count,
                total,
                copy_depth,
            )],
        );
        assert!(!result.changed, "{rows:?} → {:?}", result.rows);
        assert_eq!(result.rows, rows);
        assert_eq!(result.focus, Some(anchor.key));
    }
}

/// Linux M1: Linux found the saved chip through hidden copies, so an edit
/// that turned a chip into a plain repeat of an earlier one deleted it (at
/// ×1) or restacked the earlier chip (at ×3). The repeat folds into the
/// earlier chip and the count is left alone, as on every other platform.
#[test]
fn an_edit_that_folds_into_an_earlier_chip_keeps_it_and_ignores_its_count() {
    let base = [exact(named(1, ItemId::Spear), 3), named(2, ItemId::Mace)];
    let plain_spear = named(0, ItemId::Spear).requirement;
    for count in [1, 3] {
        let result = run(&base, &[resaved(2, plain_spear, count, None, None)]);
        assert_eq!(keys(&result.rows), [1, 2]);
        assert_eq!(counts(&result.rows), [2]);
        assert_eq!(result.focus, Some(1));
    }
    let added = run(
        &[exact(named(1, ItemId::Spear), 3)],
        &[saved(plain_spear, 1, None, None)],
    );
    assert_eq!(keys(&added.rows), [1, 2]);
    assert_eq!(counts(&added.rows), [2]);
    assert_eq!(added.focus, Some(1));
}

/// Android M3: the sheet kept a cluster member's cluster when it became a
/// trinket or artifact, and normalizing spread the cluster's stack label
/// onto it — a row Android's model refuses to construct, so the app
/// crashed. The label now never spreads onto a row that cannot stack.
#[test]
fn a_cluster_label_never_spreads_onto_a_trinket_artifact_or_blanket() {
    let stacked = edited(
        &[row(1, ItemKind::Wand), named(2, ItemId::WandFireblast)],
        &[
            Edit::Join {
                source: 1,
                target: 2,
            },
            Edit::SetCount { key: 2, count: 2 },
        ],
    );
    assert!(
        stacked
            .iter()
            .all(|row| row.requirement.identity_group == Some(1))
    );
    let blanket_wand = Requirement {
        blanket: true,
        ..Requirement::any(ItemKind::Wand)
    };
    for requirement in [
        named(0, ItemId::HornOfPlenty).requirement,
        named(0, ItemId::MimicTooth).requirement,
        blanket_wand,
    ] {
        let result = run(&stacked, &[resaved(1, requirement, 1, None, None)]);
        assert_emittable(&result.rows, "a member became unstackable");
        let member = result.rows[index_of(&result.rows, 1).unwrap()].requirement;
        assert_eq!(member.identity_group, None);
        assert!(member.alternative_group.is_some());
        // The stack stays with the member that can carry it.
        let fireblast = result.rows[index_of(&result.rows, 2).unwrap()].requirement;
        assert_eq!(fireblast.identity_group, Some(1));
        // Saving that member unchanged keeps the stack together.
        let again = run(&result.rows, &[resaved(2, fireblast, 1, None, None)]);
        assert!(!again.changed, "{:?}", again.rows);
    }
}

#[test]
fn a_saved_requirements_own_group_labels_are_ignored() {
    let base = [named(1, ItemId::Spear), named(2, ItemId::Mace)];
    let labelled = Requirement {
        alternative_group: Some(9),
        identity_group: Some(2),
        ..named(0, ItemId::Sword).requirement
    };
    let rows = edited(
        &base,
        &[
            resaved(2, labelled, 1, None, None),
            saved(labelled, 1, None, None),
        ],
    );
    assert!(rows.iter().all(|row| {
        row.requirement.alternative_group.is_none() && row.requirement.identity_group.is_none()
    }));
}

// --- copy floor limits --------------------------------------------------

#[test]
fn the_anchor_and_its_copies_carry_independent_floor_limits() {
    let anchor = floor(exact(named(0, ItemId::PlateArmor), 3), 4).requirement;
    let rows = edited(&[], &[saved(anchor, 2, None, Some(9))]);
    assert_eq!(
        rows.iter()
            .map(|row| row.requirement.max_depth)
            .collect::<Vec<_>>(),
        [Some(4), Some(9)]
    );
    // Still one chip: a repeat carrying only a floor limit folds into its stack.
    let board = board_items(&rows);
    assert_eq!(board.len(), 1);
    assert_eq!(board[0].count(), 2);
    assert_eq!(copy_depth(&rows, &board[0]), Some(9));
    assert_eq!(validate(&rows), Ok(()));
    #[cfg(feature = "json-query")]
    {
        let round = reloaded(&rows);
        assert_eq!(
            round
                .iter()
                .map(|row| row.requirement.max_depth)
                .collect::<Vec<_>>(),
            [Some(4), Some(9)]
        );
        assert_eq!(board_items(&round).len(), 1);
    }
}

#[test]
fn unlimited_copies_stay_unlimited_while_the_anchor_is_floor_bound() {
    let anchor = floor(exact(row(0, ItemKind::Armor), 3), 4).requirement;
    let rows = edited(&[], &[saved(anchor, 2, None, None)]);
    assert_eq!(rows[0].requirement.max_depth, Some(4));
    assert_eq!(rows[1].requirement.max_depth, None);
    assert_eq!(
        rows[1].requirement.identity_group,
        rows[0].requirement.identity_group
    );
    assert_eq!(validate(&rows), Ok(()));
}

#[test]
fn a_wildcard_stack_limits_its_bare_copies_without_constraining_them_otherwise() {
    let anchor = at_least(row(0, ItemKind::Wand), 2).requirement;
    let rows = edited(&[], &[saved(anchor, 2, None, Some(9))]);
    assert!(rows[1..].iter().all(|row| {
        row.requirement.max_depth == Some(9) && row.requirement.upgrade == UpgradeRequirement::Any
    }));
    assert_eq!(validate(&rows), Ok(()));
    // Growing the stack from the chip badge keeps the copies' floor.
    let grown = edited(&rows, &[Edit::SetCount { key: 1, count: 3 }]);
    assert_eq!(grown.len(), 3);
    assert!(
        grown[1..]
            .iter()
            .all(|row| row.requirement.max_depth == Some(9))
    );
}

#[test]
fn editing_away_the_limit_clears_it_from_every_copy() {
    let longsword = named(0, ItemId::Longsword).requirement;
    let rows = edited(&[], &[saved(longsword, 3, None, Some(6))]);
    assert!(
        rows[1..]
            .iter()
            .all(|row| row.requirement.max_depth == Some(6))
    );
    let cleared = edited(&rows, &[resaved(1, longsword, 3, None, None)]);
    assert!(
        cleared
            .iter()
            .all(|row| row.requirement.max_depth.is_none())
    );
    assert_eq!(counts(&cleared), [3]);
}

#[test]
fn the_copies_keep_their_floor_when_the_stack_follows_its_chip_into_a_cluster() {
    let mut rows = edited(&[], &[saved(ring(None), 2, None, Some(7))]);
    rows.push(named(9, ItemId::RingHaste));
    let joined = edited(
        &rows,
        &[Edit::Join {
            source: 1,
            target: 9,
        }],
    );
    let copy = joined
        .iter()
        .find(|row| row.requirement.item.is_none())
        .expect("the repeat became a bare copy");
    assert_eq!(copy.requirement.max_depth, Some(7));
    assert_eq!(validate(&joined), Ok(()));
}

#[test]
fn the_copies_floor_snaps_off_empty_boss_floors_and_into_range() {
    let rows = edited(
        &[named(1, ItemId::Longsword)],
        &[Edit::SetCount { key: 1, count: 2 }],
    );
    for (requested, stored) in [(10, 9), (5, 4), (15, 14), (20, 20), (0, 1), (30, 24)] {
        let limited = edited(
            &rows,
            &[Edit::SetCopyDepth {
                key: 1,
                max_depth: Some(requested),
            }],
        );
        assert_eq!(
            limited[1].requirement.max_depth,
            Some(stored),
            "{requested}"
        );
    }
    // The same value twice, a lone chip, and a combined-level stack: no-ops.
    let limited = edited(
        &rows,
        &[Edit::SetCopyDepth {
            key: 1,
            max_depth: Some(9),
        }],
    );
    let again = Edit::SetCopyDepth {
        key: 1,
        max_depth: Some(9),
    };
    assert!(!run(&limited, &[again]).changed);
    assert!(!run(&[named(1, ItemId::Longsword)], &[again]).changed);
    let counted = edited(&[], &[saved(ring(None), 2, Some(3), None)]);
    assert!(!run(&counted, &[again]).changed);
}

// --- categories and sections --------------------------------------------

/// Pinned for #190. The web, Swift, Windows and Linux suites expected the
/// ring's repeat to stay behind as a standalone chip while the ring joined
/// the wand; the join is now refused so the stack stays whole.
#[test]
fn a_counted_chip_is_refused_a_cluster_of_another_category() {
    let mut rows = edited(&[], &[saved(ring(None), 2, None, None)]);
    rows.push(row(9, ItemKind::Wand));
    let result = run(
        &rows,
        &[Edit::Join {
            source: 1,
            target: 9,
        }],
    );
    assert_eq!(result.refused, Some(Refusal::MixedCategoryStack));
    assert_eq!(result.rows, rows);
    assert_eq!(
        Refusal::MixedCategoryStack.to_string(),
        "Copies can only be grouped with the same item type."
    );
}

/// Pinned for #190. The older suites expected a wildcard wand stack to drop
/// its copies when its chip joined a spear; the join is now refused.
#[test]
fn a_wildcard_stack_is_refused_a_cluster_of_another_category() {
    let mut rows = edited(
        &[],
        &[saved(Requirement::any(ItemKind::Wand), 3, None, None)],
    );
    rows.push(named(9, ItemId::Spear));
    let result = run(
        &rows,
        &[Edit::Join {
            source: 1,
            target: 9,
        }],
    );
    assert_eq!(result.refused, Some(Refusal::MixedCategoryStack));
    assert_eq!(result.rows, rows);
}

#[test]
fn an_uncounted_mixed_join_clears_leftover_labels_and_deletes_nothing() {
    // A cluster that shrank back to ×1 keeps its stack label (the canonical
    // leftover until the follow-up); joining another category clears it.
    let leftover = edited(
        &[named(1, ItemId::Spear), named(2, ItemId::Mace)],
        &[
            Edit::Join {
                source: 2,
                target: 1,
            },
            Edit::SetCount { key: 1, count: 2 },
            Edit::SetCount { key: 1, count: 1 },
        ],
    );
    assert!(
        leftover
            .iter()
            .all(|row| row.requirement.identity_group == Some(1))
    );
    let mut rows = leftover;
    rows.push(row(9, ItemKind::Wand));
    let joined = edited(
        &rows,
        &[Edit::Join {
            source: 9,
            target: 1,
        }],
    );
    assert_eq!(joined.len(), 3);
    assert!(
        joined
            .iter()
            .all(|row| row.requirement.identity_group.is_none())
    );
    assert_eq!(counts(&joined), [1]);
    assert_eq!(validate(&joined), Ok(()));
}

#[test]
fn identical_blanket_chips_stay_separate_from_stacks_and_from_the_other_section() {
    let frost = named(1, ItemId::WandFrost);
    let blanket = |key| with(named(key, ItemId::WandFrost), |r| r.blanket = true);
    let rows = [frost, blanket(2), blanket(3)];
    assert_eq!(board_items(&rows).len(), 3);
    assert!(!can_grow(&rows, &entry(&rows, 2)));
    // An ordinary chip never joins a blanket one.
    assert_eq!(
        drop_action(&rows, 1, DropTarget::Row(2)),
        DropAction::Nothing
    );
    assert!(
        !run(
            &rows,
            &[Edit::Join {
                source: 1,
                target: 2
            }]
        )
        .changed
    );
    // The two sections number their clusters from one sequence, so an
    // ordinary cluster and a blanket one never share a label.
    let rows = [
        frost,
        named(4, ItemId::WandLightning),
        blanket(2),
        blanket(3),
    ];
    let joined = edited(
        &rows,
        &[
            Edit::Join {
                source: 4,
                target: 1,
            },
            Edit::Join {
                source: 3,
                target: 2,
            },
        ],
    );
    let label = |key| {
        joined[index_of(&joined, key).unwrap()]
            .requirement
            .alternative_group
    };
    assert!(label(1).is_some() && label(2).is_some());
    assert_ne!(label(1), label(2));
    assert_eq!(validate(&joined), Ok(()));
    // Saving a blanket member keeps it a blanket.
    let with_source = Requirement {
        source: Some(crate::model::ItemSource::WandmakerReward),
        ..joined[index_of(&joined, 2).unwrap()].requirement
    };
    let saved_rows = edited(&joined, &[resaved(2, with_source, 1, None, None)]);
    let stored = saved_rows[index_of(&saved_rows, 2).unwrap()].requirement;
    assert!(stored.blanket);
    assert_eq!(stored.source, with_source.source);
    assert_eq!(stored.alternative_group, label(2));
}

#[test]
fn named_trinkets_join_into_an_either_or_slot_that_cannot_stack() {
    let rows = edited(
        &[named(1, ItemId::MimicTooth), named(2, ItemId::RatSkull)],
        &[Edit::Join {
            source: 2,
            target: 1,
        }],
    );
    assert_eq!(validate(&rows), Ok(()));
    let board = board_items(&rows);
    assert_eq!(board.len(), 1);
    assert!(!can_grow(&rows, &board[0]));
    #[cfg(feature = "json-query")]
    assert_eq!(
        document(&rows)["requirements"].as_array().map(Vec::len),
        Some(1)
    );
}

/// Linux's plain-copy check missed trinkets and artifacts, so joining two
/// Ethereal Chains labelled them as a stack the engine refuses.
#[test]
fn artifact_repeats_stay_separate_and_join_without_stack_labels() {
    let rows = [
        named(1, ItemId::EtherealChains),
        floor(named(2, ItemId::EtherealChains), 14),
    ];
    assert_eq!(board_items(&rows).len(), 2);
    assert!(!can_grow(&rows, &entry(&rows, 1)));
    assert!(!run(&rows, &[Edit::SetCount { key: 1, count: 2 }]).changed);
    let joined = edited(
        &rows,
        &[Edit::Join {
            source: 1,
            target: 2,
        }],
    );
    assert_eq!(board_items(&joined).len(), 1);
    assert!(joined.iter().all(|row| {
        row.requirement.item == Some(ItemId::EtherealChains)
            && row.requirement.identity_group.is_none()
    }));
    assert_emittable(&joined, "artifact join");

    // An artifact cluster keeps each member's own floor limit.
    let rows = edited(
        &[
            floor(named(1, ItemId::UnstableSpellbook), 14),
            floor(named(2, ItemId::EtherealChains), 4),
        ],
        &[Edit::Join {
            source: 2,
            target: 1,
        }],
    );
    assert!(!can_grow(&rows, &board_items(&rows)[0]));
    let mut depths: Vec<_> = rows.iter().map(|row| row.requirement.max_depth).collect();
    depths.sort_unstable();
    assert_eq!(depths, [Some(4), Some(14)]);
    assert_eq!(validate(&rows), Ok(()));
    #[cfg(feature = "json-query")]
    {
        let mut round: Vec<_> = reloaded(&rows)
            .iter()
            .map(|row| row.requirement.max_depth)
            .collect();
        round.sort_unstable();
        assert_eq!(round, [Some(4), Some(14)]);
    }
}

// --- the board's fold ---------------------------------------------------

/// Two chips of one item fold only while the second is a plain repeat; the
/// board counts entries, the engine counts slots.
#[test]
fn the_board_counts_chips_and_clusters_not_rows() {
    let rows = [
        exact(named(1, ItemId::Longsword), 2),
        named(2, ItemId::Longsword),
        exact(named(3, ItemId::Longsword), 1),
        row(4, ItemKind::Armor),
    ];
    assert_eq!(counts(&rows), [2, 1, 1]);
    assert_eq!(query(requirements(&rows)).slot_count(), 4);
    let rows = [
        named(1, ItemId::Spear),
        named(2, ItemId::Spear),
        row(3, ItemKind::Wand),
    ];
    assert_eq!(board_items(&rows).len(), 2);
    assert_eq!(query(requirements(&rows)).slot_count(), 3);
}

#[test]
fn plain_repeats_fold_up_to_the_stack_limit_and_then_start_a_new_chip() {
    let rows: Vec<Row> = (1..=4).map(|key| named(key, ItemId::WandFrost)).collect();
    assert_eq!(counts(&rows), [usize::from(STACK_MAX), 1]);
    // A repeat folds into the nearest earlier chip naming its item.
    let rows = [
        exact(named(1, ItemId::WandFrost), 2),
        exact(named(2, ItemId::WandFrost), 3),
        named(3, ItemId::WandFrost),
    ];
    assert_eq!(members(&rows), [vec![0], vec![1]]);
    assert_eq!(board_items(&rows)[1].extras, [2]);
}

#[test]
fn entries_are_named_by_anchor_key_or_cluster_label() {
    let rows = edited(
        &[
            named(17, ItemId::Spear),
            named(4, ItemId::Mace),
            row(8, ItemKind::Wand),
        ],
        &[Edit::Join {
            source: 4,
            target: 17,
        }],
    );
    let board = board_items(&rows);
    assert_eq!(board[0].key(&rows), ItemKey::Cluster(1));
    assert_eq!(board[1].key(&rows), ItemKey::Chip(8));
    assert_eq!(ItemKey::Chip(17).to_string(), "r17");
    assert_eq!(ItemKey::Cluster(3).to_string(), "c3");
    // A cluster of one (a hand-written document) renders as a chip.
    let lone = [with(named(5, ItemId::Spear), |r| {
        r.alternative_group = Some(2);
    })];
    let board = board_items(&lone);
    assert_eq!(board[0].cluster, None);
    assert_eq!(board[0].key(&lone), ItemKey::Chip(5));
}

#[test]
fn a_group_with_two_constrained_members_does_not_collapse() {
    let rows = [
        with(exact(row(1, ItemKind::Wand), 3), |r| {
            r.identity_group = Some(1);
        }),
        with(exact(row(2, ItemKind::Wand), 2), |r| {
            r.identity_group = Some(1);
        }),
        with(row(3, ItemKind::Wand), |r| r.identity_group = Some(1)),
    ];
    assert_eq!(counts(&rows), [1, 1, 1]);
    assert_eq!(
        validate(&rows),
        Err(QueryError::OverconstrainedIdentityGroup)
    );
}

/// A hand-written list can put a row in a combined level and a stack label
/// at once. The combined level folds it first; the stack must not fold it a
/// second time, or the badge overcounts and a save that rebuilds the stack
/// hands the row's key out twice.
#[test]
fn a_row_in_a_combined_level_and_a_stack_folds_once() {
    let rows = [
        with(sum(row(1, ItemKind::Ring), 4, 3), |r| {
            r.identity_group = Some(1);
        }),
        with(sum(row(2, ItemKind::Ring), 4, 3), |r| {
            r.identity_group = Some(1);
        }),
        with(row(3, ItemKind::Ring), |r| r.identity_group = Some(1)),
    ];
    let board = board_items(&rows);
    assert_eq!(board.len(), 1);
    assert_eq!(board[0].extras, [1, 2]);
    let result = run(
        &rows,
        &[resaved(1, Requirement::any(ItemKind::Wand), 3, None, None)],
    );
    assert_eq!(keys(&result.rows), [1, 2, 3]);
    assert_emittable(&result.rows, "a doubly folded row");
}

// --- the drop policy and join candidates ----------------------------------

/// Linux detached on every background drop, which split a lone chip's
/// wildcard stack; only a cluster member dropped on its own section's board
/// detaches.
#[test]
fn a_background_drop_detaches_cluster_members_only() {
    let rows = edited(
        &[
            row(1, ItemKind::Wand),
            named(2, ItemId::Spear),
            named(3, ItemId::Mace),
        ],
        &[
            Edit::SetCount { key: 1, count: 2 },
            Edit::Join {
                source: 3,
                target: 2,
            },
        ],
    );
    let ordinary = DropTarget::Board { blanket: false };
    assert_eq!(drop_action(&rows, 1, ordinary), DropAction::Nothing);
    assert_eq!(drop_action(&rows, 2, ordinary), DropAction::Detach);
    assert_eq!(
        drop_action(&rows, 2, DropTarget::Board { blanket: true }),
        DropAction::Nothing
    );
    assert_eq!(
        drop_action(&rows, 1, DropTarget::Remove),
        DropAction::Remove
    );
    // Unknown and hidden rows cannot be dragged.
    let copy = rows[1].key;
    assert_eq!(drop_action(&rows, copy, ordinary), DropAction::Nothing);
    assert_eq!(
        drop_action(&rows, 99, DropTarget::Remove),
        DropAction::Nothing
    );
}

#[test]
fn drops_onto_rows_and_clusters_join_or_do_nothing() {
    let rows = edited(
        &[
            named(1, ItemId::Spear),
            named(2, ItemId::Mace),
            named(3, ItemId::Sword),
        ],
        &[Edit::Join {
            source: 2,
            target: 1,
        }],
    );
    assert_eq!(
        drop_action(&rows, 1, DropTarget::Row(1)),
        DropAction::Nothing
    );
    assert_eq!(
        drop_action(&rows, 1, DropTarget::Row(2)),
        DropAction::Nothing
    );
    assert_eq!(
        drop_action(&rows, 1, DropTarget::Cluster(1)),
        DropAction::Nothing
    );
    assert_eq!(
        drop_action(&rows, 3, DropTarget::Cluster(1)),
        DropAction::Join { target: 1 }
    );
    assert_eq!(
        drop_action(&rows, 3, DropTarget::Row(2)),
        DropAction::Join { target: 2 }
    );
    assert_eq!(
        drop_action(&rows, 3, DropTarget::Cluster(7)),
        DropAction::Nothing
    );
    assert_eq!(
        drop_action(&rows, 1, DropTarget::Row(3)),
        DropAction::Join { target: 3 }
    );
}

#[test]
fn join_candidates_list_visible_rows_joinable_or_refused() {
    let rows = edited(
        &[
            named(1, ItemId::RingMight),
            row(2, ItemKind::Wand),
            named(3, ItemId::RingEnergy),
            with(row(4, ItemKind::Wand), |r| r.blanket = true),
            with(row(5, ItemKind::Ring), |r| r.blanket = true),
        ],
        &[Edit::SetCount { key: 1, count: 2 }],
    );
    let copy = rows[1].key;
    let candidates = join_candidates(&rows, &board_items(&rows));
    let of = |key| &candidates[index_of(&rows, key).unwrap()];
    // The counted ring may join the other ring, not the wand.
    assert_eq!(of(1).join, [3]);
    assert_eq!(of(1).refuse, [(2, Refusal::MixedCategoryStack)]);
    // The uncounted wand joins the energy ring but not the counted ring.
    assert_eq!(of(2).join, [3]);
    assert_eq!(of(2).refuse, [(1, Refusal::MixedCategoryStack)]);
    // Blankets see only their own section; hidden copies see nothing.
    assert_eq!(of(4).join, [5]);
    assert!(of(4).refuse.is_empty());
    assert_eq!(*of(copy), JoinCandidates::default());
    assert!(candidates.iter().all(|c| !c.join.contains(&copy)));
    // They agree with the drop policy pair by pair.
    for source in keys(&rows) {
        for target in keys(&rows) {
            let expected = match drop_action(&rows, source, DropTarget::Row(target)) {
                DropAction::Join { .. } => (true, false),
                DropAction::Refuse(_) => (false, true),
                _ => (false, false),
            };
            let c = of(source);
            assert_eq!(
                (
                    c.join.contains(&target),
                    c.refuse.iter().any(|&(key, _)| key == target)
                ),
                expected,
                "{source} → {target}"
            );
        }
    }
}

/// Normalizing can fold the row an edit acted on into another entry: here a
/// hand-written lone alternative shares a stack label with a named ring, so
/// growing it rewrites it into that ring's plain repeat. The focus follows
/// the row to the entry that now hides it rather than naming a hidden copy.
#[test]
fn the_focus_follows_a_row_that_normalizing_folds_away() {
    let rows = [
        with(named(2, ItemId::RingEnergy), |r| r.identity_group = Some(3)),
        with(row(3, ItemKind::Ring), |r| {
            r.identity_group = Some(3);
            r.alternative_group = Some(3);
        }),
    ];
    assert_eq!(validate(&rows), Ok(()));
    assert_eq!(counts(&rows), [1, 1]);
    let result = run(&rows, &[Edit::SetCount { key: 3, count: 3 }]);
    assert!(result.changed);
    assert_eq!(counts(&result.rows), [3, 1]);
    assert!(entry(&result.rows, 3).extras.contains(&1));
    assert_eq!(result.focus, Some(2));
    assert_emittable(&result.rows, "folded focus");
}

// --- the edit sequence --------------------------------------------------

#[test]
fn a_refusal_stops_the_sequence_and_keeps_earlier_edits() {
    let rows = edited(
        &[
            named(1, ItemId::Spear),
            named(2, ItemId::Mace),
            row(3, ItemKind::Wand),
            named(4, ItemId::Sword),
        ],
        &[Edit::SetCount { key: 3, count: 2 }],
    );
    let result = run(
        &rows,
        &[
            Edit::Join {
                source: 2,
                target: 1,
            },
            Edit::Join {
                source: 3,
                target: 1,
            },
            Edit::Remove { key: 4 },
        ],
    );
    assert_eq!(result.refused, Some(Refusal::MixedCategoryStack));
    assert!(result.changed);
    assert_eq!(result.focus, Some(2));
    assert!(index_of(&result.rows, 4).is_some());
    assert_eq!(entry(&result.rows, 1).members.len(), 2);
}

#[test]
fn a_no_op_edit_returns_the_rows_verbatim_without_normalizing() {
    // A lone alternative label: not canonical, but no edit touched it.
    let rows = [
        with(named(1, ItemId::Spear), |r| r.alternative_group = Some(5)),
        row(2, ItemKind::Wand),
    ];
    for edits in [
        &[][..],
        &[Edit::Detach { key: 2 }],
        &[Edit::Remove { key: 99 }],
        &[Edit::Join {
            source: 1,
            target: 1,
        }],
        &[Edit::SetCopyDepth {
            key: 2,
            max_depth: Some(3),
        }],
    ] {
        let result = run(&rows, edits);
        assert!(!result.changed, "{edits:?}");
        assert_eq!(result.rows, rows);
        assert_eq!(result.next_key, 3);
    }
    let normalized = run(&rows, &[Edit::Normalize]);
    assert!(normalized.changed);
    assert_eq!(normalized.rows[0].requirement.alternative_group, None);
    assert_eq!(normalized.focus, None);
    assert!(!run(&normalized.rows, &[Edit::Normalize]).changed);
}

#[test]
fn broken_keys_are_repaired_and_the_edits_follow_the_repair() {
    let rows = [
        row(0, ItemKind::Wand),
        row(3, ItemKind::Armor),
        row(3, ItemKind::Ring),
        row(0, ItemKind::Weapon),
    ];
    let result = run(&rows, &[]);
    assert!(result.changed);
    assert_eq!(keys(&result.rows), [4, 3, 5, 6]);
    assert_eq!(result.rekeyed, [(0, 4), (3, 5), (0, 6)]);
    assert_eq!(result.next_key, 7);
    // A reference to a repaired key names its first occurrence; a duplicate
    // key still names the row that kept it.
    let result = run(&rows, &[Edit::Remove { key: 0 }, Edit::Remove { key: 3 }]);
    assert_eq!(
        result
            .rows
            .iter()
            .map(|row| (row.key, row.requirement.kind))
            .collect::<Vec<_>>(),
        [(5, ItemKind::Ring), (6, ItemKind::Weapon)]
    );
}

#[test]
fn stack_and_level_labels_run_out_at_four_with_a_refusal() {
    let mut rows: Vec<Row> = Vec::new();
    for key in [1, 11, 21, 31] {
        rows.push(row(key, ItemKind::Wand));
        rows = edited(&rows, &[Edit::SetCount { key, count: 2 }]);
    }
    rows.push(row(20, ItemKind::Armor));
    let result = run(&rows, &[Edit::SetCount { key: 20, count: 2 }]);
    assert_eq!(result.refused, Some(Refusal::NoFreeGroup));
    assert_eq!(result.rows, rows);
    // A save that cannot build its stack is refused whole, like any other
    // refused edit: the rows stay as they were, so the sheet can stay open.
    let armor = exact(row(0, ItemKind::Armor), 2).requirement;
    for edit in [
        resaved(20, armor, 2, None, None),
        saved(armor, 2, None, None),
    ] {
        let result = run(&rows, &[edit]);
        assert_eq!(result.refused, Some(Refusal::NoFreeGroup));
        assert!(!result.changed);
        assert_eq!(result.rows, rows);
    }
    // The same save without the stack goes through.
    let result = run(&rows, &[resaved(20, armor, 1, None, None)]);
    assert_eq!(result.refused, None);
    assert_eq!(
        result.rows[index_of(&result.rows, 20).unwrap()].requirement,
        armor
    );

    let mut rings: Vec<Row> = Vec::new();
    for (key, item_id) in [
        (1, ItemId::RingMight),
        (4, ItemId::RingHaste),
        (7, ItemId::RingEnergy),
        (10, ItemId::RingWealth),
        (13, ItemId::RingArcana),
    ] {
        rings.push(named(key, item_id));
        rings = edited(&rings, &[Edit::SetCount { key, count: 2 }]);
    }
    for key in [1, 4, 7, 10] {
        rings = edited(&rings, &[Edit::ToggleLevels { key }]);
    }
    let arcana = named(0, ItemId::RingArcana).requirement;
    for edit in [
        Edit::ToggleLevels { key: 13 },
        resaved(13, arcana, 2, Some(2), None),
        // A new ring (not a plain repeat of the Arcana stack, which it would
        // fold into) needs a fifth label too.
        saved(ring(Some(1)), 2, Some(2), None),
    ] {
        let result = run(&rings, &[edit]);
        assert_eq!(result.refused, Some(Refusal::NoFreeGroup), "{edit:?}");
        assert_eq!(result.rows, rings);
    }
    assert_emittable(&rings, "four combined levels");
    // Re-saving a counted ring keeps its own label, free or not.
    let might = rings[index_of(&rings, 1).unwrap()].requirement;
    let result = run(&rings, &[resaved(1, might, 2, Some(3), None)]);
    assert_eq!(result.refused, None);
    assert_eq!(entry(&result.rows, 1).total, Some(3));
}

// --- the stack documents the web writes -----------------------------------

/// The four stack shapes as the web board writes them, pinned from both
/// sides: the rows the edits produce, and — with the document codec — the
/// documents Android's suite captured from the web encoder.
#[test]
fn the_four_stack_shapes_match_the_web_documents() {
    let might = |upgrade| exact(named(1, ItemId::RingMight), upgrade);
    let concrete = edited(&[might(2)], &[Edit::SetCount { key: 1, count: 3 }]);
    let copy = named(0, ItemId::RingMight).requirement;
    assert_eq!(requirements(&concrete), [might(2).requirement, copy, copy]);

    let wildcard = edited(
        &[exact(row(1, ItemKind::Wand), 3)],
        &[Edit::SetCount { key: 1, count: 3 }],
    );
    let bare = Requirement {
        identity_group: Some(1),
        ..Requirement::any(ItemKind::Wand)
    };
    assert_eq!(
        requirements(&wildcard),
        [
            Requirement {
                upgrade: UpgradeRequirement::Exact(3),
                ..bare
            },
            bare,
            bare
        ]
    );

    let totalled = edited(
        &[might(2)],
        &[
            Edit::SetCount { key: 1, count: 2 },
            Edit::SetTotal {
                key: 1,
                total: Some(3),
            },
        ],
    );
    let member = sum(named(0, ItemId::RingMight), 1, 3).requirement;
    assert_eq!(requirements(&totalled), [member, member]);

    let cluster = edited(
        &[
            exact(row(1, ItemKind::Wand), 3),
            exact(named(2, ItemId::WandFireblast), 3),
        ],
        &[
            Edit::Join {
                source: 1,
                target: 2,
            },
            Edit::SetCount { key: 1, count: 3 },
        ],
    );
    assert_eq!(keys(&cluster)[..2], [2, 1]);

    #[cfg(feature = "json-query")]
    for (rows, web) in [
        (
            &concrete,
            r#"{"requirements":[{"kind":"ring","item":"ring_might","upgrade":2},
                {"kind":"ring","item":"ring_might"},{"kind":"ring","item":"ring_might"}]}"#,
        ),
        (
            &wildcard,
            r#"{"requirements":[{"kind":"wand","upgrade":3,"identity_group":1},
                {"kind":"wand","identity_group":1},{"kind":"wand","identity_group":1}]}"#,
        ),
        (
            &totalled,
            r#"{"requirements":[
                {"kind":"ring","item":"ring_might","level_sum":{"group":1,"at_least":3}},
                {"kind":"ring","item":"ring_might","level_sum":{"group":1,"at_least":3}}]}"#,
        ),
        (
            &cluster,
            r#"{"requirements":[{"any_of":[
                {"kind":"wand","item":"wand_fireblast","upgrade":3,"identity_group":1},
                {"kind":"wand","upgrade":3,"identity_group":1}]},
                {"kind":"wand","identity_group":1},{"kind":"wand","identity_group":1}]}"#,
        ),
    ] {
        let web: serde_json::Value = serde_json::from_str(web).unwrap();
        assert_eq!(document(rows), web);
        // Every shape survives the trip back through the document.
        let decoded = reloaded(rows);
        assert_eq!(
            board_items(&decoded)
                .iter()
                .map(|item| (item.count(), item.total))
                .collect::<Vec<_>>(),
            board_items(rows.as_slice())
                .iter()
                .map(|item| (item.count(), item.total))
                .collect::<Vec<_>>()
        );
        assert_eq!(validate(&decoded), Ok(()));
    }
}

// --- properties ---------------------------------------------------------

/// Random valid rows × random edit sequences (1,024 cases, per the test
/// budget): nothing panics, every emitted list keeps the §3 invariant, a
/// no-op or a refusal returns the rows verbatim, the focus names a visible
/// row, a sequence equals its edits applied one by one, and edits are
/// deterministic.
#[test]
fn random_edits_on_valid_rows_keep_every_row_emittable() {
    let mut rng = Rng::new(0x5eed_5eed_c0de_2024);
    for case in 0..1024 {
        let rows = random_rows(&mut rng);
        assert_emittable(&rows, "the generator");
        let edits: Vec<Edit> = (0..rng.range(1, 4))
            .map(|_| random_edit(&mut rng, &rows))
            .collect();
        let hint = rng.chance(20).then(|| rng.next() % 30);
        let result = apply(&rows, hint, &edits);
        let context = format!("case {case}: {rows:?} {edits:?}");
        assert_emittable(&result.rows, &context);
        assert!(result.rekeyed.is_empty(), "{context}");
        assert!(
            result.next_key > result.rows.iter().map(|row| row.key).max().unwrap_or(0),
            "{context}"
        );
        if !result.changed {
            assert_eq!(result.rows, rows, "{context}");
        }
        assert_eq!(apply(&rows, hint, &edits), result, "{context}");

        // One edit at a time: each step keeps the invariant, a refusal or a
        // no-op leaves its input alone, and the steps add up to the sequence.
        let mut current = rows.clone();
        let mut refused = None;
        for &edit in &edits {
            let step = apply(&current, hint, &[edit]);
            let context = format!("{context} at {edit:?}");
            assert_emittable(&step.rows, &context);
            if step.refused.is_some() || !step.changed {
                assert_eq!(step.rows, current, "{context}");
            }
            if let Some(focus) = step.focus {
                let index = index_of(&step.rows, focus).expect("the focus is a row");
                assert!(
                    board_items(&step.rows)
                        .iter()
                        .any(|item| item.members.contains(&index)),
                    "{context}"
                );
            }
            if let Edit::Join { source, target } = edit {
                // The drop policy predicts the join exactly. On a canonical
                // list the two rows then share a cluster; a hand-written list
                // may tie a cluster member to a lone chip's stack, and
                // normalizing then folds the member into that stack instead.
                let canonical = !apply(&current, hint, &[Edit::Normalize]).changed;
                match drop_action(&current, source, DropTarget::Row(target)) {
                    DropAction::Join { .. } => {
                        assert!(step.changed && step.refused.is_none(), "{context}");
                        let joined = entry(&step.rows, source);
                        let target = index_of(&step.rows, target).unwrap();
                        assert!(
                            !canonical
                                || (joined.cluster.is_some() && joined.members.contains(&target)),
                            "{context}"
                        );
                    }
                    DropAction::Refuse(refusal) => {
                        assert_eq!(step.refused, Some(refusal), "{context}");
                    }
                    action => {
                        assert_eq!(action, DropAction::Nothing, "{context}");
                        assert!(!step.changed && step.refused.is_none(), "{context}");
                    }
                }
            }
            current = step.rows;
            if step.refused.is_some() {
                refused = step.refused;
                break;
            }
        }
        assert_eq!((current, refused), (result.rows.clone(), result.refused));

        // Candidates never name hidden copies and agree with the drop policy.
        let items = board_items(&result.rows);
        let candidates = join_candidates(&result.rows, &items);
        for (index, candidate) in candidates.iter().enumerate() {
            for &target in &candidate.join {
                assert_eq!(
                    drop_action(
                        &result.rows,
                        result.rows[index].key,
                        DropTarget::Row(target)
                    ),
                    DropAction::Join { target },
                    "{context}"
                );
            }
        }
    }
}

/// Arbitrary rows — invalid ones, broken keys, labels past four, rows in two
/// folds at once — never make an edit panic, and the keys always come back
/// unique and in range (1,024 cases).
#[test]
fn random_edits_on_arbitrary_rows_never_panic() {
    let mut rng = Rng::new(0xbad_f00d_dead_beef);
    for case in 0..1024 {
        let mut rows = random_rows(&mut rng);
        for row in &mut rows {
            if rng.chance(15) {
                row.key = rng.pick(&[0, 1, 2, u64::MAX, super::super::MAX_KEY + 1]);
            }
            let requirement = &mut row.requirement;
            match rng.below(8) {
                0 => requirement.identity_group = Some(rng.range(0, 255)),
                1 => {
                    requirement.level_sum = Some(LevelSum {
                        group: rng.range(0, 9),
                        minimum_total: rng.range(0, 40),
                    });
                }
                2 => requirement.alternative_group = Some(rng.range(0, 255)),
                3 => requirement.kind = rng.pick(&[ItemKind::Trinket, ItemKind::Ring]),
                4 => requirement.max_depth = Some(rng.range(0, 60)),
                _ => {}
            }
        }
        let edits: Vec<Edit> = (0..rng.range(1, 4))
            .map(|_| random_edit(&mut rng, &rows))
            .collect();
        let result = apply(&rows, None, &edits);
        let mut unique: Vec<u64> = keys(&result.rows);
        unique.sort_unstable();
        unique.dedup();
        let context = format!("case {case}: {rows:?} {edits:?}");
        assert_eq!(unique.len(), result.rows.len(), "{context}");
        assert!(
            result.rows.iter().all(|row| super::is_valid_key(row.key)),
            "{context}"
        );
        if !result.changed {
            assert_eq!(keys(&result.rows), keys(&rows), "{context}");
        }
        let _ = join_candidates(&result.rows, &board_items(&result.rows));
    }
}
