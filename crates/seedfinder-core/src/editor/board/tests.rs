//! The board's fold and edits.
//!
//! These port the union of the platform relation suites — web
//! `relations.test.ts` and the relation cases of `blankets`, `trinkets` and
//! `artifacts.test.tsx`; Android `RelationsTest.kt` (with the #190 cases and
//! the four stack documents the web writes); Swift `RelationsTests.swift`;
//! Windows `QueryRelationshipsTests.cs`; Linux `relations.rs` — deduplicated
//! and expressed in keys. Where the older suites pinned a stack following
//! its chip into a cluster, the cases pin the one-item join instead: a bare
//! copy joins and the chip stays behind with its constraints, one item
//! fewer, while a stacked target keeps its stack as a member. #190's refusal of a join across categories with a
//! stack is lifted — every copy keeps its own chip's kind — and the cases
//! that pinned it now pin the join.

use super::super::testing::{
    Rng, assert_emittable, named, query, random_edit, random_requirement, random_rows,
    requirements, row, validate, with,
};
use super::*;
use crate::catalog::{ItemId, ItemKind, WeaponCategory};
use crate::editor::{
    STACK_MAX, can_change_count, can_count_levels, can_grow, copy_depth, level_capacity, problems,
};
use crate::query::{
    LevelSum, QueryError, Requirement, SumGroup, TierRequirement, UpgradeRequirement,
};

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

/// Every chip's count, entry by entry and member by member.
fn counts(rows: &[Row]) -> Vec<usize> {
    board_items(rows)
        .iter()
        .flat_map(|item| item.stacks.iter().map(ChipStack::count))
        .collect()
}

/// The stack of the visible row `key`, or of the entry's first chip when
/// `key` is a hidden copy.
fn chip(rows: &[Row], key: u64) -> ChipStack {
    let item = entry(rows, key);
    let index = index_of(rows, key).expect("the key is in the list");
    item.stack(index).unwrap_or(&item.stacks[0]).clone()
}

/// A lone chip's stack: the entry's first chip.
trait Lone {
    fn count(&self) -> usize;
    fn total(&self) -> Option<u8>;
}

impl Lone for BoardItem {
    fn count(&self) -> usize {
        self.stacks[0].count()
    }

    fn total(&self) -> Option<u8> {
        self.stacks[0].total
    }
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

/// `row` as a member of the either/or group `group`, under the stack label
/// `label` if any.
fn member(row: Row, group: u8, label: Option<u8>) -> Row {
    with(row, |r| {
        r.alternative_group = Some(group);
        r.identity_group = label;
    })
}

/// A bare wand copy under the stack label `label`.
fn bare_wand(key: u64, label: u8) -> Row {
    with(row(key, ItemKind::Wand), |r| r.identity_group = Some(label))
}

/// A row's key, alternative label, stack label and combined total.
type RowLabels = (u64, Option<u8>, Option<u8>, Option<u8>);

/// Every row's [`RowLabels`].
fn shape(rows: &[Row]) -> Vec<RowLabels> {
    rows.iter()
        .map(|row| {
            let r = row.requirement;
            (
                row.key,
                r.alternative_group,
                r.identity_group,
                r.level_sum.map(|sum| sum.minimum_total),
            )
        })
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

/// The web suite joined a combined-level pair onto a shuriken, which #190
/// refused while a cluster's stack had to name one kind. Every copy now
/// keeps its own chip's kind, so the join goes through across categories as
/// within one: the joined ring drops the level sum and its other member stays
/// behind — a chip of its own, since a combined level of one says nothing.
#[test]
fn joining_a_combined_level_stack_drops_its_total_and_leaving_a_pair_dissolves_it() {
    for other in [ItemId::Shuriken, ItemId::RingWealth] {
        let pair = [
            sum(named(1, ItemId::RingMight), 1, 3),
            sum(named(2, ItemId::RingMight), 1, 3),
            named(3, other),
        ];
        let joined = edited(
            &pair,
            &[Edit::Join {
                source: 1,
                target: 3,
            }],
        );
        // One ring joins, a bare copy: the anchor stays, its pair dissolved.
        assert!(joined.iter().all(|row| row.requirement.level_sum.is_none()));
        assert_eq!(keys(&joined), [1, 3, 2]);
        assert_eq!(counts(&joined), [1, 1, 1]);
        assert_eq!(validate(&joined), Ok(()));
        let out = edited(&joined, &[Edit::Detach { key: 3 }]);
        assert!(
            out.iter()
                .all(|row| row.requirement.alternative_group.is_none())
        );
    }
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
    assert_eq!(board[0].total(), None);
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

/// Every chip has a stack of its own, a cluster member's too: its copies are
/// bare copies under a label only it carries — the engine's member stack,
/// "two Runic Blades, or a War Hammer" — and members whose stacks are alike
/// share one label, "two of whichever matched".
#[test]
fn a_cluster_member_grows_a_stack_of_its_own() {
    let base = edited(
        &[named(1, ItemId::RunicBlade), named(2, ItemId::WarHammer)],
        &[Edit::Join {
            source: 2,
            target: 1,
        }],
    );
    let result = run(&base, &[Edit::SetCount { key: 1, count: 3 }]);
    assert_eq!(result.focus, Some(1));
    let next = result.rows;
    assert_eq!(
        shape(&next),
        [
            (1, Some(1), Some(1), None),
            (2, Some(1), None, None),
            (3, None, Some(1), None),
            (4, None, Some(1), None),
        ]
    );
    assert!(next[2..].iter().all(|row| row.requirement.is_bare()));
    assert_eq!(validate(&next), Ok(()));
    let board = board_items(&next);
    assert_eq!(board.len(), 1);
    assert!(board[0].cluster.is_some());
    assert_eq!(counts(&next), [3, 1]);
    assert_eq!(board[0].extras, [2, 3]);
    // The other member's stepper is its own, under a label of its own.
    let both = edited(&next, &[Edit::SetCount { key: 2, count: 2 }]);
    assert_eq!(counts(&both), [3, 2]);
    assert_eq!(
        labels_of(&both),
        [Some(1), Some(2), Some(1), Some(1), Some(2)]
    );
    assert_eq!(validate(&both), Ok(()));
    // Alike, the two share one label and one set of copies.
    let alike = edited(&next, &[Edit::SetCount { key: 2, count: 3 }]);
    assert_eq!(keys(&alike), [1, 2, 3, 4]);
    assert_eq!(labels_of(&alike), [Some(1); 4]);
    assert_eq!(counts(&alike), [3, 3]);
    // Removing a member removes its own stack; the survivor becomes a chip.
    assert_eq!(keys(&edited(&next, &[Edit::Remove { key: 1 }])), [2]);
    let kept = edited(&next, &[Edit::Remove { key: 2 }]);
    assert_eq!(counts(&kept), [3]);
    assert_eq!(labels_of(&kept), [None; 3]);
    assert_eq!(validate(&kept), Ok(()));

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
    assert_eq!(labels_of(&wands), [Some(1), None, Some(1), Some(1)]);
    assert_eq!(counts(&wands), [3, 1]);
    assert_eq!(validate(&wands), Ok(()));
}

/// A join moves one item. The web, and the core after it, traded a stacked
/// target's plain repeats for bare copies the whole cluster shared, which
/// turned "two Spears, and a Mace" into "two of the same item, Spear or
/// Mace". A stacked target now keeps its stack as a member — "two Spears, or
/// a Mace" — and a stacked source stays, one item fewer, while a bare copy
/// of it joins.
#[test]
fn a_stacked_target_keeps_its_stack_and_a_stacked_source_leaves_its_copies() {
    let base = edited(
        &[named(1, ItemId::Spear), named(2, ItemId::Mace)],
        &[Edit::SetCount { key: 1, count: 2 }],
    );
    assert_eq!(keys(&base), [1, 3, 2]);
    let next = edited(
        &base,
        &[Edit::Join {
            source: 2,
            target: 1,
        }],
    );
    assert_eq!(
        shape(&next),
        [
            (1, Some(1), Some(1), None),
            (2, Some(1), None, None),
            (3, None, Some(1), None),
        ]
    );
    assert_eq!(
        next[2].requirement,
        Requirement {
            identity_group: Some(1),
            ..Requirement::any(ItemKind::Weapon)
        }
    );
    assert_eq!(members(&next), [vec![0, 1]]);
    assert_eq!(counts(&next), [2, 1]);
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
    // The +2 ring stays; a bare copy of it joins.
    assert_eq!(keys(&rings), [1, 9, 10]);
    assert_eq!(rings[0], exact(named(1, ItemId::RingMight), 2));
    assert_eq!(rings[2].requirement.item, Some(ItemId::RingMight));
    assert_eq!(rings[2].requirement.upgrade, UpgradeRequirement::Any);
    assert_eq!(counts(&rings), [1, 1, 1]);
    assert_eq!(validate(&rings), Ok(()));
}

/// Android #190 refused a join across categories when either entry was a
/// stack: a cluster's stack had to name one kind for its copies, and "spear
/// or wand" names none. Every copy now keeps its own chip's kind — `{Wand of
/// Frost ×2 | Plate Armor}` is two Frosts or the armor — so such a join
/// moves one item as any other does.
#[test]
#[allow(clippy::too_many_lines)] // Four joins, each pinned whole.
fn a_join_across_categories_keeps_every_stack_with_its_own_kind() {
    let stacked = edited(
        &[
            exact(row(1, ItemKind::Weapon), 3),
            exact(named(9, ItemId::WandFireblast), 3),
        ],
        &[Edit::SetCount { key: 1, count: 3 }],
    );
    assert_eq!(labels_of(&stacked), [Some(1), Some(1), Some(1), None]);
    // The stacked weapon joins the wand: a bare weapon joins, and the +3
    // weapon stays, a stack of two.
    assert_eq!(
        drop_action(&stacked, 1, DropTarget::Row(9)),
        DropAction::Join { target: 9 }
    );
    let joined = edited(
        &stacked,
        &[Edit::Join {
            source: 1,
            target: 9,
        }],
    );
    assert_eq!(
        shape(&joined),
        [
            (1, None, Some(1), None),
            (10, None, Some(1), None),
            (9, Some(1), None, None),
            (11, Some(1), None, None),
        ]
    );
    assert_eq!(joined[3], member(row(11, ItemKind::Weapon), 1, None));
    assert_eq!(counts(&joined), [2, 1, 1]);
    assert!(problems(&joined).is_empty());
    // The wand joins the stacked weapon, which keeps its ×3 as a member.
    let joined = edited(
        &stacked,
        &[Edit::Join {
            source: 9,
            target: 1,
        }],
    );
    assert_eq!(
        shape(&joined),
        [
            (1, Some(1), Some(1), None),
            (9, Some(1), None, None),
            (10, None, Some(1), None),
            (11, None, Some(1), None),
        ]
    );
    assert_eq!(counts(&joined), [3, 1]);
    assert!(problems(&joined).is_empty());

    // A counted ring and a plate armor, either way round.
    for count in 2..=3 {
        let mut ring = edited(
            &[],
            &[saved(
                exact(named(0, ItemId::RingEnergy), 4).requirement,
                count,
                None,
                Some(20),
            )],
        );
        ring.push(exact(named(9, ItemId::PlateArmor), 3));
        let onto_ring = edited(
            &ring,
            &[Edit::Join {
                source: 9,
                target: 1,
            }],
        );
        let copy = Requirement {
            identity_group: Some(1),
            max_depth: Some(20),
            ..Requirement::any(ItemKind::Ring)
        };
        assert!(
            onto_ring
                .iter()
                .filter(|row| row.requirement.alternative_group.is_none())
                .all(|row| row.requirement == copy)
        );
        assert_eq!(counts(&onto_ring), [usize::from(count), 1]);
        assert!(problems(&onto_ring).is_empty());
        let onto_armor = edited(
            &ring,
            &[Edit::Join {
                source: 1,
                target: 9,
            }],
        );
        assert_eq!(counts(&onto_armor), [usize::from(count) - 1, 1, 1]);
        assert!(problems(&onto_armor).is_empty());
    }

    // {Wand of Frost ×2 | Plate Armor}, as the engine reads it.
    let frost = edited(
        &[named(1, ItemId::WandFrost), named(2, ItemId::PlateArmor)],
        &[
            Edit::SetCount { key: 1, count: 2 },
            Edit::Join {
                source: 2,
                target: 1,
            },
        ],
    );
    assert_eq!(
        frost,
        [
            member(named(1, ItemId::WandFrost), 1, Some(1)),
            member(named(2, ItemId::PlateArmor), 1, None),
            bare_wand(3, 1),
        ]
    );
    assert_eq!(validate(&frost), Ok(()));
}

/// Android #190: another chip may name the same ring with its own stack;
/// joining one chip touches nothing of the other's.
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

/// Dropping a ring on a counted ring joins the stack whole, as a member: its
/// plain repeats become bare copies under a label of its own, each with its
/// floor limit.
#[test]
fn dropping_a_ring_on_a_counted_ring_keeps_its_stack_and_its_copies_floor() {
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
    assert_eq!(keys(&joined), [1, 9, 2, 3]);
    let board = board_items(&joined);
    assert_eq!(board.len(), 1);
    assert_eq!(board[0].members, [0, 1]);
    assert_eq!(counts(&joined), [3, 1]);
    assert_eq!(joined[0].requirement.upgrade, UpgradeRequirement::Exact(4));
    assert_eq!(copy_depth(&joined, &board[0].stacks[0]), Some(9));
    assert!(joined[2..].iter().all(|row| {
        row.requirement
            == Requirement {
                identity_group: Some(1),
                max_depth: Some(9),
                ..Requirement::any(ItemKind::Ring)
            }
    }));
    assert_eq!(validate(&joined), Ok(()));
}

/// A cluster spanning two categories used to be barred from stacking: one
/// label had to name one kind for every member's copies. Each member now
/// stacks copies of its own kind.
#[test]
fn every_member_of_a_cluster_spanning_categories_grows_its_own_stack() {
    let mixed = edited(
        &[row(1, ItemKind::Wand), named(2, ItemId::Spear)],
        &[Edit::Join {
            source: 1,
            target: 2,
        }],
    );
    assert!(can_grow(&mixed, &chip(&mixed, 1)));
    assert!(can_change_count(&mixed, &chip(&mixed, 1)));
    let grown = edited(
        &mixed,
        &[
            Edit::SetCount { key: 1, count: 2 },
            Edit::SetCount { key: 2, count: 3 },
        ],
    );
    assert_eq!(counts(&grown), [3, 2]);
    let copies = |label| {
        grown
            .iter()
            .filter(|row| {
                row.requirement.alternative_group.is_none()
                    && row.requirement.identity_group == Some(label)
            })
            .map(|row| row.requirement.kind)
            .collect::<Vec<_>>()
    };
    assert_eq!(copies(1), [ItemKind::Wand]);
    assert_eq!(copies(2), [ItemKind::Weapon, ItemKind::Weapon]);
    assert!(problems(&grown).is_empty());
}

/// #190 refused a counted ring joining the ring member of a cluster that
/// spans categories. A bare copy of the ring joins; the ring stays behind,
/// one item fewer.
#[test]
fn a_counted_ring_joins_the_ring_member_of_a_mixed_category_cluster() {
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
        DropAction::Join { target: 1 }
    );
    let joined = edited(
        &counted,
        &[Edit::Join {
            source: 9,
            target: 1,
        }],
    );
    assert_eq!(keys(&joined), [1, 2, 11, 9, 10]);
    assert_eq!(counts(&joined), [1, 1, 1, 2]);
    assert!(problems(&joined).is_empty());
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
    // A bare copy of the Spear leaves, on the key of the copy it was, and
    // lands last; the Spear stays in the cluster at ×1, its label gone with
    // its copy.
    let result = run(&base, &[Edit::Detach { key: 1 }]);
    assert_eq!(result.focus, Some(3));
    let spear = result.rows[index_of(&result.rows, 1).unwrap()].requirement;
    assert_eq!(spear.alternative_group, Some(1));
    assert_eq!(spear.identity_group, None);
    assert_eq!(
        shape(&result.rows),
        [
            (1, Some(1), None, None),
            (2, Some(1), None, None),
            (3, None, None, None),
        ]
    );
    assert_eq!(counts(&result.rows), [1, 1, 1]);
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
    // Fireblast lands last, after the wand stack.
    assert_eq!(keys(&wands), [1, 3, 2]);
    assert_eq!(counts(&wands), [2, 1]);
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

/// Members whose stacks are alike share a label; stepping one down gives it
/// none, and the other keeps the label and the copies. A member at ×1
/// carries no label, so none of the four goes to it.
#[test]
fn a_member_stepped_down_to_one_drops_its_stack_label() {
    let cluster = [
        member(named(1, ItemId::WandFrost), 1, Some(1)),
        member(named(2, ItemId::WandDisintegration), 1, Some(1)),
        bare_wand(3, 1),
    ];
    assert_eq!(counts(&cluster), [2, 2]);
    let result = run(&cluster, &[Edit::SetCount { key: 1, count: 1 }]);
    assert_eq!(
        shape(&result.rows),
        [
            (1, Some(1), None, None),
            (2, Some(1), Some(1), None),
            (3, None, Some(1), None),
        ]
    );
    assert_eq!(counts(&result.rows), [1, 2]);
    let both = edited(&result.rows, &[Edit::SetCount { key: 2, count: 1 }]);
    assert_eq!(
        shape(&both),
        [(1, Some(1), None, None), (2, Some(1), None, None)]
    );
    // A hand-written leftover label goes the same way.
    let leftover = run(&cluster[..2], &[Edit::Normalize]);
    assert!(leftover.changed);
    assert_eq!(leftover.rows, both);
    // The label is free for the next stack.
    let regrown = edited(&both, &[Edit::SetCount { key: 2, count: 2 }]);
    assert_eq!(labels_of(&regrown), [None, Some(1), Some(1)]);
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
    assert_eq!(board[0].total(), Some(3));
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
        assert!(!can_count_levels(&stacked, &chip(&stacked, 1)));
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
    assert!(can_count_levels(&loaded, &chip(&loaded, 1)));
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
    assert_eq!(board[0].total(), Some(4));
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
        assert_eq!((board[0].total(), board[0].count()), (Some(4), 2));
    }
}

#[test]
fn totals_clamp_to_what_the_stack_can_reach_and_toggle_from_the_count() {
    let three = edited(
        &[named(1, ItemId::RingMight)],
        &[Edit::SetCount { key: 1, count: 3 }],
    );
    // Three any-upgrade rings reach 5 + 3 + 3 levels: one vault ring.
    assert_eq!(level_capacity(&three, &chip(&three, 1)), 11);
    let high = edited(
        &three,
        &[Edit::SetTotal {
            key: 1,
            total: Some(99),
        }],
    );
    assert_eq!(entry(&high, 1).total(), Some(11));
    let low = edited(
        &three,
        &[Edit::SetTotal {
            key: 1,
            total: Some(0),
        }],
    );
    assert_eq!(entry(&low, 1).total(), Some(1));
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
    assert_eq!(entry(&toggled, 1).total(), Some(3));
    let off = edited(&toggled, &[Edit::ToggleLevels { key: 1 }]);
    assert_eq!(entry(&off, 1).total(), None);
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
    assert!(!can_grow(&blanket, &chip(&blanket, 1)));
    assert!(!run(&blanket, &[Edit::SetCount { key: 1, count: 2 }]).changed);
}

#[test]
fn counting_levels_keeps_the_copies_own_floor_limit_both_ways() {
    let rows = [
        floor(named(1, ItemId::RingEnergy), 9),
        floor(named(2, ItemId::RingEnergy), 20),
    ];
    let counting = edited(&rows, &[Edit::ToggleLevels { key: 1 }]);
    assert_eq!(
        counting,
        [
            floor(sum(named(1, ItemId::RingEnergy), 1, 2), 9),
            floor(sum(named(2, ItemId::RingEnergy), 1, 2), 20),
        ]
    );
    assert_eq!(copy_depth(&counting, &chip(&counting, 1)), Some(20));
    let changed = edited(
        &counting,
        &[Edit::SetTotal {
            key: 1,
            total: Some(5),
        }],
    );
    assert_eq!(changed[1].requirement.max_depth, Some(20));
    assert_eq!(edited(&counting, &[Edit::ToggleLevels { key: 1 }]), rows);
    // Copies without a limit stay without one while the anchor keeps its.
    let open = [rows[0], named(2, ItemId::RingEnergy)];
    let round = edited(
        &open,
        &[Edit::ToggleLevels { key: 1 }, Edit::ToggleLevels { key: 1 }],
    );
    assert_eq!(round, open);
}

/// A counting stack grown from the board adds copies with the copies' floor
/// limit, not the anchor's, as the sheet does: the anchor's floor is its
/// own placement.
#[test]
fn growing_a_counting_stack_gives_new_copies_the_copies_floor() {
    let counting = [
        floor(sum(named(1, ItemId::RingEnergy), 1, 3), 9),
        floor(sum(named(2, ItemId::RingEnergy), 1, 3), 20),
    ];
    let grown = edited(&counting, &[Edit::SetCount { key: 1, count: 3 }]);
    assert_eq!(keys(&grown), [1, 2, 3]);
    assert_eq!(grown[2].requirement, counting[1].requirement);

    let rows = [
        floor(named(1, ItemId::RingEnergy), 9),
        floor(named(2, ItemId::RingEnergy), 20),
    ];
    let round = edited(
        &rows,
        &[
            Edit::ToggleLevels { key: 1 },
            Edit::SetCount { key: 1, count: 3 },
            Edit::ToggleLevels { key: 1 },
        ],
    );
    assert_eq!(
        round,
        [rows[0], rows[1], floor(named(3, ItemId::RingEnergy), 20)]
    );
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

/// A stack's copies need not sit right after their chip: a chip that joined
/// a cluster and left it again leaves its plain repeats where they were.
/// Rebuilding the stack on an unchanged save would move them back behind the
/// chip; the save is a no-op instead, whatever order the list is in.
#[test]
fn an_unchanged_save_keeps_copies_where_the_list_has_them() {
    let frost = named(1, ItemId::WandFrost);
    let rows = [
        frost,
        row(2, ItemKind::Ring),
        floor(named(3, ItemId::WandFrost), 9),
    ];
    assert_eq!(counts(&rows), [2, 1]);
    let result = run(&rows, &[resaved(1, frost.requirement, 2, None, Some(9))]);
    assert!(!result.changed, "{:?}", result.rows);
    assert_eq!(result.rows, rows);
    assert_eq!(result.focus, Some(1));
    // Copies with floors of their own: a plain repeat saved with its own
    // floor folds into the earlier chip. The sheet shows the first copy's
    // floor, and saving that changes none of them.
    let own = [
        frost,
        floor(named(2, ItemId::WandFrost), 9),
        row(3, ItemKind::Ring),
        floor(named(4, ItemId::WandFrost), 4),
    ];
    assert_eq!(counts(&own), [3, 1]);
    let result = run(&own, &[resaved(1, frost.requirement, 3, None, Some(9))]);
    assert!(!result.changed, "{:?}", result.rows);
    // A different copy floor, count or requirement still rebuilds the stack.
    for edit in [
        resaved(1, frost.requirement, 2, None, None),
        resaved(1, frost.requirement, 3, None, Some(9)),
        resaved(1, exact(frost, 2).requirement, 2, None, Some(9)),
    ] {
        let result = run(&rows, &[edit]);
        assert!(result.changed, "{edit:?}");
        assert_eq!(keys(&result.rows)[..2], [1, 3], "{edit:?}");
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
/// crashed. A stack label now belongs to the member whose stack it is and
/// never spreads, and a member that becomes a trinket, artifact or blanket
/// sheds its own stack; the other members keep theirs. #190 refused that
/// save while a cluster's stack had to name one kind for all its members.
#[test]
fn a_stack_label_never_lands_on_a_trinket_artifact_or_blanket_member() {
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
    assert_eq!(labels_of(&stacked), [Some(1), None, Some(1)]);
    let trinkets = [
        named(0, ItemId::HornOfPlenty).requirement,
        named(0, ItemId::MimicTooth).requirement,
    ];
    for requirement in trinkets {
        // The wand member becomes a trinket beside the Fireblast stack.
        let result = run(&stacked, &[resaved(1, requirement, 1, None, None)]);
        assert_eq!(result.refused, None);
        assert_emittable(&result.rows, "a member became a trinket");
        assert_eq!(labels_of(&result.rows), [Some(1), None, Some(1)]);
        assert!(problems(&result.rows).is_empty());
        // Just as a drag of that item onto the cluster joins it.
        let beside: Vec<Row> = stacked
            .iter()
            .copied()
            .chain([Row {
                key: 9,
                requirement,
            }])
            .collect();
        assert_eq!(
            drop_action(&beside, 9, DropTarget::Row(2)),
            DropAction::Join { target: 2 }
        );
        // The stacked member becoming a trinket sheds its stack, whatever
        // count the sheet sent.
        let result = run(&stacked, &[resaved(2, requirement, 2, None, None)]);
        assert_emittable(&result.rows, "a stacked member became a trinket");
        assert_eq!(keys(&result.rows), [2, 1]);
        assert_eq!(labels_of(&result.rows), [None, None]);
    }
    let blanket_wand = Requirement {
        blanket: true,
        ..Requirement::any(ItemKind::Wand)
    };
    let result = run(&stacked, &[resaved(1, blanket_wand, 1, None, None)]);
    assert_emittable(&result.rows, "a member became a blanket");
    let member = result.rows[index_of(&result.rows, 1).unwrap()].requirement;
    assert_eq!(member.identity_group, None);
    assert!(member.alternative_group.is_some());
    // The stack stays with the member whose stack it is.
    let fireblast = result.rows[index_of(&result.rows, 2).unwrap()].requirement;
    assert_eq!(fireblast.identity_group, Some(1));
    // Saving that member unchanged keeps its stack together.
    let again = run(&result.rows, &[resaved(2, fireblast, 2, None, None)]);
    assert!(!again.changed, "{:?}", again.rows);

    // A hand-written cluster holding a trinket and a label on one member is
    // that member's stack: normalizing leaves it as written.
    let written = [
        with(named(1, ItemId::MimicTooth), |r| {
            r.alternative_group = Some(1);
        }),
        with(named(2, ItemId::WandFireblast), |r| {
            r.alternative_group = Some(1);
            r.identity_group = Some(1);
        }),
        with(row(3, ItemKind::Wand), |r| r.alternative_group = Some(1)),
        with(row(4, ItemKind::Wand), |r| r.identity_group = Some(1)),
    ];
    assert!(!run(&written, &[Edit::Normalize]).changed);
    assert_eq!(counts(&written), [1, 2, 1]);
    assert_eq!(validate(&written), Ok(()));
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
    assert_eq!(copy_depth(&rows, &board[0].stacks[0]), Some(9));
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
fn a_joining_copy_and_the_copies_left_behind_keep_their_floor() {
    let mut rows = edited(&[], &[saved(ring(None), 3, None, Some(7))]);
    rows.push(named(9, ItemId::RingHaste));
    let joined = edited(
        &rows,
        &[Edit::Join {
            source: 1,
            target: 9,
        }],
    );
    assert_eq!(keys(&joined), [1, 2, 9, 3]);
    let left = entry(&joined, 1);
    assert_eq!((left.cluster, left.count()), (None, 2));
    assert_eq!(copy_depth(&joined, &left.stacks[0]), Some(7));
    assert_eq!(joined[0].requirement.max_depth, None);
    assert_eq!(
        joined[3].requirement,
        Requirement {
            max_depth: Some(7),
            alternative_group: Some(1),
            ..ring(None)
        }
    );
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

/// Pinned for #190, which refused these joins; lifted with member stacks.
/// The web, Swift, Windows and Linux suites expected the ring's repeat to
/// stay behind as a standalone chip while the ring joined the wand, and a
/// wildcard wand stack to leave its copies when its chip joined a spear:
/// the one-item join, which now carries a bare copy and leaves the chip.
#[test]
fn a_counted_chip_joins_a_chip_of_another_category_and_leaves_its_copies() {
    let mut rows = edited(&[], &[saved(ring(None), 2, None, None)]);
    rows.push(row(9, ItemKind::Wand));
    let joined = edited(
        &rows,
        &[Edit::Join {
            source: 1,
            target: 9,
        }],
    );
    assert_eq!(
        shape(&joined),
        [
            (1, None, None, None),
            (9, Some(1), None, None),
            (2, Some(1), None, None),
        ]
    );
    assert_eq!(joined[0], named(1, ItemId::RingMight));
    assert_eq!(validate(&joined), Ok(()));

    let mut rows = edited(
        &[],
        &[saved(Requirement::any(ItemKind::Wand), 3, None, None)],
    );
    rows.push(named(9, ItemId::Spear));
    let joined = edited(
        &rows,
        &[Edit::Join {
            source: 1,
            target: 9,
        }],
    );
    assert_eq!(counts(&joined), [2, 1, 1]);
    assert_eq!(
        requirements(&joined)[..2],
        [Requirement {
            identity_group: Some(1),
            ..Requirement::any(ItemKind::Wand)
        }; 2]
    );
    assert_eq!(validate(&joined), Ok(()));
}

#[test]
fn a_mixed_join_drops_leftover_labels_and_deletes_nothing() {
    // A hand-written cluster of ×1 members holding a stack label (the
    // editor's own edits drop such a label); joining another category drops
    // it as normalizing does.
    let leftover = [
        with(named(1, ItemId::Spear), |r| {
            r.alternative_group = Some(1);
            r.identity_group = Some(1);
        }),
        with(named(2, ItemId::Mace), |r| {
            r.alternative_group = Some(1);
            r.identity_group = Some(1);
        }),
    ];
    let mut rows = leftover.to_vec();
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
    assert_eq!(counts(&joined), [1, 1, 1]);
    assert_eq!(validate(&joined), Ok(()));
}

#[test]
fn identical_blanket_chips_stay_separate_from_stacks_and_from_the_other_section() {
    let frost = named(1, ItemId::WandFrost);
    let blanket = |key| with(named(key, ItemId::WandFrost), |r| r.blanket = true);
    let rows = [frost, blanket(2), blanket(3)];
    assert_eq!(board_items(&rows).len(), 3);
    assert!(!can_grow(&rows, &chip(&rows, 2)));
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
    assert!(board[0].stacks.iter().all(|stack| !can_grow(&rows, stack)));
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
    assert!(!can_grow(&rows, &chip(&rows, 1)));
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
    assert!(!can_grow(&rows, &board_items(&rows)[0].stacks[0]));
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

/// Every row of `rows` is a member or a hidden copy of exactly one entry.
fn assert_every_row_shown(rows: &[Row], context: &str) {
    let mut claims = vec![0_usize; rows.len()];
    for item in board_items(rows) {
        for &index in item.members.iter().chain(&item.extras) {
            claims[index] += 1;
        }
    }
    assert!(
        claims.iter().all(|&claims| claims == 1),
        "claims {claims:?}: {context}"
    );
}

/// A hand-written list can anchor a stack on a combined level's copy, or a
/// combined level on a stack's bare copy. The web folded the other group's
/// rows into an entry that never formed, so they showed nowhere, no removal
/// reached them and their problems blamed no chip. A stack whose anchor a
/// combined level folded now leaves its copies on the board, and a combined
/// level's anchor never folds into a stack.
#[test]
fn a_stack_and_a_combined_level_never_fold_a_row_out_of_sight() {
    // The stack's anchor is a copy of the combined level.
    let rows = [
        sum(named(1, ItemId::RingMight), 1, 2),
        with(sum(named(2, ItemId::RingMight), 1, 2), |r| {
            r.identity_group = Some(2);
        }),
        with(row(3, ItemKind::Ring), |r| r.identity_group = Some(2)),
    ];
    assert_eq!(validate(&rows), Ok(()));
    assert_every_row_shown(&rows, "a stack anchored on a combined-level copy");
    let board = board_items(&rows);
    assert_eq!(board.len(), 2);
    assert_eq!(
        (&board[0].members, &board[0].extras, board[0].total()),
        (&vec![0], &vec![1], Some(2))
    );
    assert_eq!(board[1].members, [2]);
    let removed = run(&rows, &[Edit::Remove { key: 3 }]);
    assert!(removed.changed);
    assert_eq!(keys(&removed.rows), [1, 2]);
    let whole = run(&rows, &[Edit::RemoveItem { key: 1 }]);
    assert_eq!(keys(&whole.rows), [3]);

    // The same, the stack's constrained member after the combined level's
    // anchor (the fuzz case the envelope review found).
    let rows = [
        with(row(3, ItemKind::Ring), |r| r.identity_group = Some(2)),
        sum(named(4, ItemId::RingHaste), 1, 3),
        with(sum(named(5, ItemId::RingHaste), 1, 3), |r| {
            r.identity_group = Some(2);
        }),
    ];
    assert_eq!(validate(&rows), Ok(()));
    assert_every_row_shown(&rows, "a stack anchored on a later combined-level copy");
    assert_eq!(members(&rows), [vec![0], vec![1]]);
    assert!(run(&rows, &[Edit::Remove { key: 3 }]).changed);

    // The combined level's anchor is a bare copy of a stack: it stays a chip
    // carrying its own copies.
    let rows = [
        with(named(1, ItemId::RingMight), |r| r.identity_group = Some(1)),
        with(sum(row(2, ItemKind::Ring), 1, 2), |r| {
            r.identity_group = Some(1);
        }),
        sum(row(3, ItemKind::Ring), 1, 2),
    ];
    assert_eq!(validate(&rows), Ok(()));
    assert_every_row_shown(&rows, "a combined level anchored on a stack copy");
    let board = board_items(&rows);
    assert_eq!(board.len(), 2);
    assert_eq!(board[0].members, [0]);
    assert_eq!(
        (&board[1].members, &board[1].extras, board[1].total()),
        (&vec![1], &vec![2], Some(2))
    );
    // Normalizing keeps the combined level rather than turning its anchor
    // into a plain repeat of the named ring, which would drop it.
    let normalized = run(&rows, &[Edit::Normalize]);
    assert!(!normalized.changed, "{:?}", normalized.rows);
    assert!(
        normalized
            .rows
            .iter()
            .skip(1)
            .all(|row| row.requirement.level_sum.is_some())
    );
    let removed = run(&rows, &[Edit::RemoveItem { key: 2 }]);
    assert_eq!(keys(&removed.rows), [1]);
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
        DropAction::RemoveOne
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
    // The counted ring may join the wand and the other ring, and they it:
    // every copy keeps its own chip's kind.
    assert_eq!(of(1).join, [2, 3]);
    assert!(of(1).refuse.is_empty());
    assert_eq!(of(2).join, [1, 3]);
    assert!(of(2).refuse.is_empty());
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

// --- a join moves one item ------------------------------------------------

/// The reported list: Disintegration ×2 dragged onto Frost moved the whole
/// stack into the group, whose count became Disintegration's second copy —
/// "two of the same wand, Frost or Disintegration". One Disintegration now
/// joins, a bare copy, while the chip stays behind with the other; detaching
/// the joined one folds the two back together.
#[test]
fn a_stacked_chip_dragged_onto_a_chip_joins_one_copy_and_detaching_it_folds_back() {
    let rows = [
        named(1, ItemId::WandDisintegration),
        named(2, ItemId::WandLightning),
        exact(named(3, ItemId::RingEnergy), 4),
        floor(named(4, ItemId::RingEnergy), 20),
        floor(named(5, ItemId::RingEnergy), 20),
        exact(named(6, ItemId::PlateArmor), 3),
        named(7, ItemId::WandFrost),
        exact(row(8, ItemKind::Wand), 3),
        named(20, ItemId::WandDisintegration),
    ];
    assert_eq!(counts(&rows), [2, 1, 3, 1, 1, 1]);
    let result = run(
        &rows,
        &[Edit::Join {
            source: 1,
            target: 7,
        }],
    );
    // The copy that joined is the stack's last: row 20.
    assert_eq!(result.focus, Some(20));
    assert_eq!(keys(&result.rows), [1, 2, 3, 4, 5, 6, 7, 20, 8]);
    assert_eq!(
        shape(&result.rows)[6..],
        [
            (7, Some(1), None, None),
            (20, Some(1), None, None),
            (8, None, None, None),
        ]
    );
    assert_eq!(result.rows[0], rows[0]);
    assert_eq!(counts(&result.rows), [1, 1, 3, 1, 1, 1, 1]);
    assert_eq!(validate(&result.rows), Ok(()));

    // Detached, it lands last, as a new Disintegration would, and folds
    // back into the stack it came from.
    let detached = run(&result.rows, &[Edit::Detach { key: 20 }]);
    assert_eq!(detached.focus, Some(1));
    assert_eq!(keys(&detached.rows), [1, 2, 3, 4, 5, 6, 7, 8, 20]);
    assert!(
        detached
            .rows
            .iter()
            .all(|row| row.requirement.alternative_group.is_none())
    );
    let disintegration = entry(&detached.rows, 1);
    assert_eq!(disintegration.count(), 2);
    assert_eq!(detached.rows[disintegration.extras[0]].key, 20);
    assert_eq!(counts(&detached.rows), [2, 1, 3, 1, 1, 1]);
    assert_eq!(requirements(&detached.rows)[..7], requirements(&rows)[..7]);

    // The same round trip with the stack first: the detached copy folds
    // back into the chip it came from.
    let pair = [
        named(1, ItemId::WandDisintegration),
        named(2, ItemId::WandDisintegration),
        named(3, ItemId::WandFrost),
    ];
    let joined = edited(
        &pair,
        &[Edit::Join {
            source: 1,
            target: 3,
        }],
    );
    assert_eq!(keys(&joined), [1, 3, 2]);
    assert_eq!(counts(&joined), [1, 1, 1]);
    let back = edited(&joined, &[Edit::Detach { key: 2 }]);
    assert_eq!(keys(&back), [1, 3, 2]);
    assert_eq!(
        requirements(&back),
        requirements(&[pair[0], pair[2], pair[1]])
    );
    assert_eq!(counts(&back), [2, 1]);
}

/// The reported drag: Ring of Energy +4 ×3 onto Disintegration moved the
/// +4 row into the group while the bin took a bare copy, so the lifted chip
/// and its origin both read +4. Every drop now carries the copy the bin
/// takes: a bare Ring of Energy joins, the chip stays +4 ×2, and detaching
/// the copy folds it back. A member's copy leaves the same way.
#[test]
fn the_reported_ring_joins_as_a_bare_copy_and_folds_back() {
    let rows = [
        exact(named(1, ItemId::RingEnergy), 4),
        named(2, ItemId::RingEnergy),
        named(3, ItemId::RingEnergy),
        named(4, ItemId::WandDisintegration),
    ];
    assert_eq!(counts(&rows), [3, 1]);
    let joined = run(
        &rows,
        &[Edit::Join {
            source: 1,
            target: 4,
        }],
    );
    assert_eq!(joined.focus, Some(3));
    assert_eq!(
        joined.rows,
        [
            rows[0],
            rows[1],
            member(rows[3], 1, None),
            member(rows[2], 1, None),
        ]
    );
    assert_eq!(counts(&joined.rows), [2, 1, 1]);
    // The bin takes the very copy the join carried.
    let removed = edited(&rows, &[Edit::RemoveOne { key: 1 }]);
    assert_eq!(removed, [rows[0], rows[1], rows[3]]);
    // Detached again, the copy folds back into the +4 stack.
    let back = run(&joined.rows, &[Edit::Detach { key: 3 }]);
    assert_eq!(back.focus, Some(1));
    assert_eq!(back.rows, [rows[0], rows[1], rows[3], rows[2]]);
    assert_eq!(counts(&back.rows), [3, 1]);

    // Frost of `{Frost +2 ×2 | Disintegration}` detached: a bare Frost
    // leaves, and the member stays +2 — what the bin leaves it.
    let frost = [
        member(exact(named(1, ItemId::WandFrost), 2), 1, Some(1)),
        member(named(2, ItemId::WandDisintegration), 1, None),
        bare_wand(3, 1),
    ];
    let detached = run(&frost, &[Edit::Detach { key: 1 }]);
    assert_eq!(detached.focus, Some(3));
    assert_eq!(
        detached.rows,
        [
            member(exact(named(1, ItemId::WandFrost), 2), 1, None),
            member(named(2, ItemId::WandDisintegration), 1, None),
            named(3, ItemId::WandFrost),
        ]
    );
    let removed = edited(&frost, &[Edit::RemoveOne { key: 1 }]);
    assert_eq!(removed, detached.rows[..2]);
}

#[test]
fn every_kind_of_stack_stays_one_item_fewer_when_a_copy_of_it_joins() {
    // Three Disintegrations: the last copy joins with its floor, and the
    // chip stays a stack of two.
    let three = [
        named(1, ItemId::WandFrost),
        named(2, ItemId::WandDisintegration),
        floor(named(3, ItemId::WandDisintegration), 9),
        floor(named(4, ItemId::WandDisintegration), 9),
    ];
    let joined = edited(
        &three,
        &[Edit::Join {
            source: 2,
            target: 1,
        }],
    );
    assert_eq!(keys(&joined), [1, 4, 2, 3]);
    assert_eq!(joined[2..], three[1..3]);
    assert_eq!(
        joined[1],
        member(floor(named(4, ItemId::WandDisintegration), 9), 1, None)
    );
    assert_eq!(members(&joined), [vec![0, 1], vec![2]]);
    assert_eq!(counts(&joined), [1, 1, 2]);
    assert_eq!(copy_depth(&joined, &chip(&joined, 2)), Some(9));

    // A wildcard stack: a bare wand joins, the constrained chip stays.
    let wildcard = edited(
        &[
            named(1, ItemId::WandFrost),
            exact(row(2, ItemKind::Wand), 3),
        ],
        &[Edit::SetCount { key: 2, count: 2 }],
    );
    assert_eq!(
        shape(&wildcard),
        [
            (1, None, None, None),
            (2, None, Some(1), None),
            (3, None, Some(1), None)
        ]
    );
    let joined = edited(
        &wildcard,
        &[Edit::Join {
            source: 2,
            target: 1,
        }],
    );
    assert_eq!(
        shape(&joined),
        [
            (1, Some(1), None, None),
            (3, Some(1), None, None),
            (2, None, None, None)
        ]
    );
    assert_eq!(
        joined[1],
        member(row(3, ItemKind::Wand), 1, None),
        "the wildcard's kind and nothing else"
    );
    assert_eq!(joined[2].requirement.upgrade, UpgradeRequirement::Exact(3));
    assert_eq!(counts(&joined), [1, 1, 1]);

    // Three of a wildcard: the chip stays a stack of two.
    let wildcard = edited(&wildcard, &[Edit::SetCount { key: 2, count: 3 }]);
    let joined = edited(
        &wildcard,
        &[Edit::Join {
            source: 2,
            target: 1,
        }],
    );
    assert_eq!(counts(&joined), [1, 1, 2]);
    assert_eq!(validate(&joined), Ok(()));

    // Rings within floor 20: a plain copy joins Might with its floor 20,
    // and the +4 keeps the other.
    let rings = [
        exact(named(1, ItemId::RingEnergy), 4),
        floor(named(2, ItemId::RingEnergy), 20),
        floor(named(3, ItemId::RingEnergy), 20),
        named(4, ItemId::RingMight),
    ];
    let joined = edited(
        &rings,
        &[Edit::Join {
            source: 1,
            target: 4,
        }],
    );
    assert_eq!(keys(&joined), [1, 2, 4, 3]);
    assert_eq!(joined[..2], rings[..2]);
    let left = entry(&joined, 1);
    assert_eq!(
        (left.count(), copy_depth(&joined, &left.stacks[0])),
        (2, Some(20))
    );
    assert_eq!(joined[3], member(rings[2], 1, None));
    assert_eq!(counts(&joined), [2, 1, 1]);
    assert_eq!(validate(&joined), Ok(()));
}

/// A drop onto a stacked lone chip joins that chip with its whole stack,
/// which becomes its stack as a member: Disintegration onto Frost ×2 gives
/// `{Frost ×2 | Disintegration}` — the previous pass gave `{Frost |
/// Disintegration}` and a Frost of its own. A drop onto a cluster joins it
/// as a ×1 member, and its members keep their stacks.
#[test]
fn a_drop_onto_a_stack_keeps_the_targets_stack() {
    let frosts = [
        named(1, ItemId::WandFrost),
        named(2, ItemId::WandFrost),
        named(3, ItemId::WandDisintegration),
    ];
    let joined = edited(
        &frosts,
        &[Edit::Join {
            source: 3,
            target: 1,
        }],
    );
    assert_eq!(
        joined,
        [
            member(named(1, ItemId::WandFrost), 1, Some(1)),
            member(named(3, ItemId::WandDisintegration), 1, None),
            bare_wand(2, 1),
        ]
    );
    assert_eq!(counts(&joined), [2, 1]);
    assert!(problems(&joined).is_empty());

    let cluster = [
        member(named(1, ItemId::WandFrost), 1, Some(1)),
        member(named(2, ItemId::WandLightning), 1, Some(1)),
        bare_wand(3, 1),
        named(4, ItemId::WandDisintegration),
        named(5, ItemId::WandDisintegration),
    ];
    assert_eq!(counts(&cluster), [2, 2, 2]);
    assert_eq!(
        drop_action(&cluster, 4, DropTarget::Cluster(1)),
        DropAction::Join { target: 1 }
    );
    // One Disintegration joins, a copy of the stack; the chip stays ×1.
    for target in [1, 2] {
        let result = run(&cluster, &[Edit::Join { source: 4, target }]);
        assert_eq!(result.focus, Some(5));
        assert_eq!(
            shape(&result.rows),
            [
                (1, Some(1), Some(1), None),
                (2, Some(1), Some(1), None),
                (5, Some(1), None, None),
                (3, None, Some(1), None),
                (4, None, None, None),
            ]
        );
        assert_eq!(result.rows[3], cluster[2]);
        assert_eq!(result.rows[4], cluster[3]);
        assert_eq!(entry(&result.rows, 1).members.len(), 3);
        assert_eq!(counts(&result.rows), [2, 2, 1, 1]);
        assert!(problems(&result.rows).is_empty());
    }
}

/// A combined-level stack losing a ring to a join as the source keeps its
/// combined level on the rings left behind, capped at what they can still
/// reach, or drops it when only one is left. As the target it keeps its
/// count as a member's stack and drops the combined level, which cannot
/// travel into a cluster.
#[test]
fn a_combined_level_stack_keeps_what_its_rest_can_reach_or_its_count_as_a_member() {
    let energy = |key, total| sum(named(key, ItemId::RingEnergy), 1, total);
    let might = named(4, ItemId::RingMight);
    for (total, kept) in [(11, 8), (6, 6)] {
        let rows = [energy(1, total), energy(2, total), energy(3, total), might];
        assert_eq!(level_capacity(&rows, &chip(&rows, 1)), 11);
        let context = format!("Σ ≥ {total}");
        let result = run(
            &rows,
            &[Edit::Join {
                source: 1,
                target: 4,
            }],
        );
        assert_eq!(result.refused, None, "{context}");
        let left = entry(&result.rows, 1);
        assert_eq!((left.count(), left.total()), (2, Some(kept)), "{context}");
        assert_eq!(
            result.rows[index_of(&result.rows, 3).unwrap()].requirement,
            Requirement {
                alternative_group: Some(1),
                ..named(0, ItemId::RingEnergy).requirement
            },
            "{context}"
        );
        assert_eq!(counts(&result.rows), [2, 1, 1], "{context}");
        assert!(problems(&result.rows).is_empty(), "{context}");

        let result = run(
            &rows,
            &[Edit::Join {
                source: 4,
                target: 1,
            }],
        );
        assert_eq!(
            result.rows,
            [
                member(named(1, ItemId::RingEnergy), 1, Some(1)),
                member(might, 1, None),
                with(row(2, ItemKind::Ring), |r| r.identity_group = Some(1)),
                with(row(3, ItemKind::Ring), |r| r.identity_group = Some(1)),
            ],
            "{context}"
        );
        assert_eq!(chip(&result.rows, 1).total, None, "{context}");
        assert_eq!(counts(&result.rows), [3, 1], "{context}");
        assert!(problems(&result.rows).is_empty(), "{context}");
    }
    // A pair leaves one ring, which drops the combined level.
    let pair = [energy(1, 4), energy(2, 4), might];
    let joined = edited(
        &pair,
        &[Edit::Join {
            source: 1,
            target: 4,
        }],
    );
    assert!(joined.iter().all(|row| row.requirement.level_sum.is_none()));
    assert_eq!(
        joined[index_of(&joined, 1).unwrap()],
        named(1, ItemId::RingEnergy)
    );
    assert_eq!(counts(&joined), [1, 1, 1]);
    let joined = edited(
        &pair,
        &[Edit::Join {
            source: 4,
            target: 1,
        }],
    );
    assert!(joined.iter().all(|row| row.requirement.level_sum.is_none()));
    assert_eq!(counts(&joined), [2, 1]);
}

/// A join removes no row but the copies of member stacks it makes alike:
/// one Frost of `{Frost ×3 | Disintegration ×2}` joining leaves `{Frost ×2 |
/// Disintegration ×2}`, whose two stacks share one label and its copies,
/// so Frost's own copy left goes (the other one joined). The cluster asks
/// for what it did.
#[test]
fn a_join_merges_the_member_stacks_it_makes_alike() {
    let rows = [
        member(named(1, ItemId::WandFrost), 1, Some(1)),
        member(named(2, ItemId::WandDisintegration), 1, Some(2)),
        bare_wand(3, 1),
        bare_wand(4, 1),
        bare_wand(5, 2),
        named(6, ItemId::WandLightning),
    ];
    assert_eq!(counts(&rows), [3, 2, 1]);
    let joined = edited(
        &rows,
        &[Edit::Join {
            source: 1,
            target: 6,
        }],
    );
    assert_eq!(
        joined,
        [
            member(named(1, ItemId::WandFrost), 1, Some(2)),
            member(named(2, ItemId::WandDisintegration), 1, Some(2)),
            bare_wand(5, 2),
            member(named(6, ItemId::WandLightning), 2, None),
            member(named(4, ItemId::WandFrost), 2, None),
        ]
    );
    assert_eq!(counts(&joined), [2, 2, 1, 1]);
}

/// A combined-level stack written with a stack label of its own keeps that
/// label as a joined target: it needs no free one, so the join goes through
/// with every other label taken, and it is not renumbered when labels are
/// free.
#[test]
fn a_labelled_combined_level_target_keeps_its_own_label() {
    let ring = |key, label| {
        with(sum(row(key, ItemKind::Ring), 1, 5), |r| {
            r.identity_group = Some(label);
        })
    };
    let copy = |key, kind, label| with(row(key, kind), |r| r.identity_group = Some(label));
    let might = named(3, ItemId::RingMight);
    let taken = [
        copy(4, ItemKind::Wand, 2),
        copy(5, ItemKind::Wand, 2),
        copy(6, ItemKind::Armor, 3),
        copy(7, ItemKind::Armor, 3),
        copy(8, ItemKind::Weapon, 4),
        copy(9, ItemKind::Weapon, 4),
    ];
    let full: Vec<Row> = [ring(1, 1), ring(2, 1), might]
        .into_iter()
        .chain(taken)
        .collect();
    let join = Edit::Join {
        source: 3,
        target: 1,
    };
    assert_eq!(
        drop_action(&full, 3, DropTarget::Row(1)),
        DropAction::Join { target: 1 }
    );
    assert!(
        join_candidates(&full, &board_items(&full))[2]
            .join
            .contains(&1)
    );
    let result = run(&full, &[join]);
    assert_eq!(result.refused, None);
    let joined = [
        member(row(1, ItemKind::Ring), 1, Some(1)),
        member(might, 1, None),
        copy(2, ItemKind::Ring, 1),
    ];
    assert_eq!(result.rows[..3], joined);
    assert_eq!(result.rows[3..], taken);
    assert_eq!(counts(&result.rows), [2, 1, 2, 2, 2]);

    let free = [ring(1, 3), ring(2, 3), might];
    let joined = joined.map(|row| {
        with(row, |r| {
            r.identity_group = r.identity_group.map(|_| 3);
        })
    });
    assert_eq!(edited(&free, &[join]), joined);
}

/// A member leaving a cluster — detached, or dragged onto another chip —
/// takes one item of its stack: a bare copy, while the member stays in its
/// place with its constraints, one item fewer. Frost out of `{Frost +2 ×2 |
/// Disintegration}` gives `{Frost +2 | Disintegration}` and Frost.
#[test]
#[allow(clippy::too_many_lines)] // Four stacks, each pinned whole.
fn a_member_leaving_a_cluster_takes_one_item_of_its_stack() {
    let own = [
        member(named(1, ItemId::WandFrost), 1, Some(1)),
        member(named(2, ItemId::WandDisintegration), 1, None),
        bare_wand(3, 1),
    ];
    assert_eq!(counts(&own), [2, 1]);
    let detached = run(&own, &[Edit::Detach { key: 1 }]);
    assert_eq!(detached.focus, Some(3));
    assert_eq!(
        detached.rows,
        [
            member(named(1, ItemId::WandFrost), 1, None),
            member(named(2, ItemId::WandDisintegration), 1, None),
            named(3, ItemId::WandFrost),
        ]
    );
    assert_eq!(counts(&detached.rows), [1, 1, 1]);
    assert_eq!(
        drop_action(&own, 1, DropTarget::Board { blanket: false }),
        DropAction::Detach
    );
    // Constraints stay with the member; the copy keeps its floor limit.
    let upgraded = [
        member(exact(named(1, ItemId::WandFrost), 2), 1, Some(1)),
        own[1],
        floor(bare_wand(3, 1), 9),
    ];
    let detached = edited(&upgraded, &[Edit::Detach { key: 1 }]);
    assert_eq!(
        [detached[0], detached[2]],
        [
            member(exact(named(1, ItemId::WandFrost), 2), 1, None),
            floor(named(3, ItemId::WandFrost), 9),
        ]
    );

    // Alike stacks share their copies, so the copy that leaves is a new
    // row, and the shared copy stays with Frost.
    let shared = [
        member(named(1, ItemId::WandFrost), 1, Some(1)),
        member(named(2, ItemId::WandDisintegration), 1, Some(1)),
        bare_wand(3, 1),
    ];
    assert_eq!(counts(&shared), [2, 2]);
    let detached = run(&shared, &[Edit::Detach { key: 2 }]);
    assert_eq!(detached.focus, Some(4));
    assert_eq!(
        detached.rows,
        [
            member(named(1, ItemId::WandFrost), 1, Some(1)),
            member(named(2, ItemId::WandDisintegration), 1, None),
            bare_wand(3, 1),
            named(4, ItemId::WandDisintegration),
        ]
    );
    assert_eq!(counts(&detached.rows), [2, 1, 1]);
    // At ×3 the member keeps a stack of its own, under a label of its own.
    let three = [shared[0], shared[1], shared[2], bare_wand(4, 1)];
    let detached = edited(&three, &[Edit::Detach { key: 2 }]);
    assert_eq!(
        shape(&detached),
        [
            (1, Some(1), Some(1), None),
            (2, Some(1), Some(2), None),
            (3, None, Some(1), None),
            (4, None, Some(1), None),
            (5, None, Some(2), None),
            (6, None, None, None),
        ]
    );
    assert_eq!(counts(&detached), [3, 2, 1]);
    assert!(problems(&detached).is_empty());

    // A member dragged onto another chip moves one item the same way.
    let frost = [
        member(exact(named(1, ItemId::WandFrost), 2), 1, Some(1)),
        member(named(2, ItemId::WandDisintegration), 1, Some(1)),
        bare_wand(3, 1),
        exact(row(7, ItemKind::Wand), 3),
    ];
    let result = run(
        &frost,
        &[Edit::Join {
            source: 1,
            target: 7,
        }],
    );
    assert_eq!(result.focus, Some(8));
    assert_eq!(
        shape(&result.rows),
        [
            (1, Some(1), None, None),
            (2, Some(1), Some(1), None),
            (3, None, Some(1), None),
            (7, Some(2), None, None),
            (8, Some(2), None, None),
        ]
    );
    assert_eq!(
        result.rows[0],
        member(exact(named(1, ItemId::WandFrost), 2), 1, None)
    );
    assert_eq!(result.rows[4], member(named(8, ItemId::WandFrost), 2, None));
    assert_eq!(counts(&result.rows), [1, 2, 1, 1]);
    assert!(problems(&result.rows).is_empty());
}

/// `Remove` is the chip menu's: a chip with its whole stack. `RemoveOne`
/// is the remove target's, which a drag of one item reaches: one copy of a
/// stack, or the chip itself when it has none.
#[test]
fn remove_takes_a_chips_stack_and_remove_one_takes_one_item() {
    let own = [
        member(exact(named(1, ItemId::WandFrost), 2), 1, Some(1)),
        member(named(2, ItemId::WandDisintegration), 1, None),
        bare_wand(3, 1),
        bare_wand(4, 1),
        exact(named(5, ItemId::RingEnergy), 2),
        floor(named(6, ItemId::RingEnergy), 9),
    ];
    assert_eq!(counts(&own), [3, 1, 2]);
    // A member ×3 steps down to ×2, keeping its constraints and label.
    let result = run(&own, &[Edit::RemoveOne { key: 1 }]);
    assert_eq!(result.focus, Some(1));
    assert_eq!(keys(&result.rows), [1, 2, 3, 5, 6]);
    assert_eq!(counts(&result.rows), [2, 1, 2]);
    // A member ×1 leaves its cluster, removed; a cluster of one dissolves.
    let result = run(&own, &[Edit::RemoveOne { key: 2 }]);
    assert_eq!(result.focus, None);
    assert_eq!(counts(&result.rows), [3, 2]);
    assert_eq!(labels_of(&result.rows), [None; 5]);
    // A lone stack sheds its last copy; a lone chip without copies goes.
    let result = run(&own, &[Edit::RemoveOne { key: 5 }]);
    assert_eq!(keys(&result.rows), [1, 2, 3, 4, 5]);
    assert_eq!(result.focus, Some(5));
    let result = run(&result.rows, &[Edit::RemoveOne { key: 5 }]);
    assert_eq!(keys(&result.rows), [1, 2, 3, 4]);
    // Hidden copies and unknown keys are no target.
    for key in [3, 6, 99] {
        assert!(!run(&own, &[Edit::RemoveOne { key }]).changed, "{key}");
    }
    // The menu's Remove takes a member's own stack whole, a lone chip's
    // whole entry.
    assert_eq!(keys(&edited(&own, &[Edit::Remove { key: 1 }])), [2, 5, 6]);
    assert_eq!(
        keys(&edited(&own, &[Edit::Remove { key: 5 }])),
        [1, 2, 3, 4]
    );
    // A stack members share stays with the others.
    let shared = [
        member(named(1, ItemId::WandFrost), 1, Some(1)),
        member(named(2, ItemId::WandDisintegration), 1, Some(1)),
        bare_wand(3, 1),
    ];
    assert_eq!(
        edited(&shared, &[Edit::Remove { key: 1 }]),
        [
            named(2, ItemId::WandDisintegration),
            named(3, ItemId::WandDisintegration)
        ]
    );
    assert_eq!(
        edited(&shared, &[Edit::RemoveOne { key: 1 }]),
        [
            member(named(1, ItemId::WandFrost), 1, None),
            shared[1],
            shared[2]
        ]
    );
    // A combined level losing a ring keeps what the rest can reach.
    let counting = [
        sum(named(1, ItemId::RingEnergy), 1, 11),
        sum(named(2, ItemId::RingEnergy), 1, 11),
        sum(named(3, ItemId::RingEnergy), 1, 11),
    ];
    let result = run(&counting, &[Edit::RemoveOne { key: 1 }]);
    assert_eq!(entry(&result.rows, 1).total(), Some(8));
    assert!(problems(&result.rows).is_empty());
    let result = run(&result.rows, &[Edit::RemoveOne { key: 1 }]);
    assert_eq!(result.rows, [named(1, ItemId::RingEnergy)]);
    assert_eq!(
        drop_action(&counting, 1, DropTarget::Remove),
        DropAction::RemoveOne
    );
}

/// A list from elsewhere may write a named stack as bare copies under a
/// stack label; the board shows it as the canonical shape. A row taken out
/// of such an entry — joined, detached or removed — leaves its copies what
/// the board showed them to be, not wildcards whose label dissolved with
/// the row that carried it.
#[test]
fn a_row_leaving_a_hand_written_stack_leaves_the_copies_the_board_showed() {
    // Disintegration ×2 as an anchor and a bare copy, dragged onto Frost.
    let source = [
        with(named(1, ItemId::WandDisintegration), |r| {
            r.identity_group = Some(1);
        }),
        bare_wand(2, 1),
        named(7, ItemId::WandFrost),
    ];
    assert_eq!(counts(&source), [2, 1]);
    let join = Edit::Join {
        source: 1,
        target: 7,
    };
    let joined = edited(&source, &[join]);
    assert_eq!(
        joined,
        [
            named(1, ItemId::WandDisintegration),
            member(named(7, ItemId::WandFrost), 1, None),
            member(named(2, ItemId::WandDisintegration), 1, None),
        ]
    );
    assert_eq!(joined, edited(&source, &[Edit::Normalize, join]));
    // A constrained anchor stays, its copy a plain repeat of its item.
    let upgraded = [
        with(exact(named(1, ItemId::WandDisintegration), 3), |r| {
            r.identity_group = Some(1);
        }),
        bare_wand(2, 1),
        bare_wand(3, 1),
        named(7, ItemId::WandFrost),
    ];
    let joined = edited(&upgraded, &[join]);
    assert_eq!(
        joined[..2],
        [
            exact(named(1, ItemId::WandDisintegration), 3),
            named(2, ItemId::WandDisintegration),
        ]
    );
    assert_eq!(
        joined[3],
        member(named(3, ItemId::WandDisintegration), 1, None)
    );
    assert_eq!(counts(&joined), [2, 1, 1]);

    // The same stack as the target keeps its stack as a member.
    let target = [
        named(1, ItemId::WandDisintegration),
        with(named(7, ItemId::WandFrost), |r| r.identity_group = Some(1)),
        bare_wand(8, 1),
    ];
    assert_eq!(
        edited(&target, &[join]),
        [
            member(named(7, ItemId::WandFrost), 1, Some(1)),
            member(named(1, ItemId::WandDisintegration), 1, None),
            bare_wand(8, 1),
        ]
    );

    // Two members' alike stacks under two labels read as one shared stack.
    let twice = [
        member(named(1, ItemId::WandFrost), 1, Some(1)),
        member(named(2, ItemId::WandDisintegration), 1, Some(2)),
        bare_wand(3, 1),
        bare_wand(4, 2),
    ];
    assert_eq!(counts(&twice), [2, 2]);
    let normalized = edited(&twice, &[Edit::Normalize]);
    assert_eq!(
        normalized,
        [
            twice[0],
            member(named(2, ItemId::WandDisintegration), 1, Some(1)),
            twice[2]
        ]
    );
    assert_eq!(
        edited(&twice, &[Edit::Detach { key: 2 }]),
        edited(&normalized, &[Edit::Detach { key: 2 }])
    );
}

/// A list never normalized — a resumed search, a preset — is drawn as
/// written, and a removal takes what the board draws under the chip: never
/// a hand-written stack that the canonical fold would give it. The review's
/// case: the bin on a lone Mace +1 beside a hand-written Mace ×2 took all
/// three Maces.
#[test]
fn a_removal_on_a_list_never_normalized_takes_what_the_board_draws() {
    let mace = |key: u64| named(key, ItemId::Mace);
    let rows = [
        exact(mace(3), 1),
        with(mace(1), |r| r.identity_group = Some(1)),
        with(row(2, ItemKind::Weapon), |r| r.identity_group = Some(1)),
        named(9, ItemId::WandFrost),
    ];
    assert_eq!(members(&rows), [vec![0], vec![1], vec![3]]);
    assert_eq!(counts(&rows), [1, 2, 1]);
    // Normalized, the hand-written stack is two plain repeats of the Mace
    // +1 before it: the canonical fold that removals once read.
    assert_eq!(counts(&edited(&rows, &[Edit::Normalize])), [3, 1]);
    let left = [mace(1), mace(2), rows[3]];
    for edit in [Edit::RemoveOne { key: 3 }, Edit::Remove { key: 3 }] {
        let result = run(&rows, &[edit]);
        assert_eq!(result.rows, left, "{edit:?}");
        assert_eq!(counts(&result.rows), [2, 1], "{edit:?}");
    }
    // The hand-written stack goes as drawn: one copy, or the whole chip.
    assert_eq!(
        counts(&edited(&rows, &[Edit::RemoveOne { key: 1 }])),
        [2, 1]
    );
    assert_eq!(
        edited(&rows, &[Edit::Remove { key: 1 }]),
        [exact(mace(3), 1), rows[3]]
    );
}

/// A detached item lands where saving it anew would: last in its section,
/// folding into the nearest earlier alike lone chip with room, never into a
/// chip after it.
#[test]
fn a_detached_item_lands_where_saving_it_anew_would() {
    let frost = |key: u64| named(key, ItemId::WandFrost);
    let rows = [
        member(exact(frost(1), 2), 1, Some(1)),
        member(named(2, ItemId::WandDisintegration), 1, None),
        bare_wand(3, 1),
        frost(5),
        frost(6),
        frost(7),
    ];
    assert_eq!(counts(&rows), [2, 1, 3]);
    let detached = run(&rows, &[Edit::Detach { key: 1 }]);
    // The Frost ×3 after the group keeps its stack, and the bare Frost
    // lands after it as a chip of its own, which the focus names.
    assert_eq!(detached.focus, Some(3));
    assert_eq!(
        detached.rows,
        [
            member(exact(frost(1), 2), 1, None),
            member(named(2, ItemId::WandDisintegration), 1, None),
            frost(5),
            frost(6),
            frost(7),
            frost(3),
        ]
    );
    assert_eq!(counts(&detached.rows), [1, 1, 3, 1]);
    assert_eq!(entry(&detached.rows, 5).count(), 3);
    let anew = run(
        &rows,
        &[
            Edit::RemoveOne { key: 1 },
            saved(frost(0).requirement, 1, None, None),
        ],
    );
    assert_eq!(anew.focus, Some(8));
    assert_eq!(counts(&anew.rows), counts(&detached.rows));

    // A ring member without copies leaves itself, and folds into the Ring
    // of Energy +4 ×2 after its group as a new ring would.
    let energy = |key: u64| named(key, ItemId::RingEnergy);
    let rings = [
        member(named(1, ItemId::WandDisintegration), 1, None),
        member(energy(2), 1, None),
        exact(energy(3), 4),
        energy(4),
        named(8, ItemId::WandLightning),
    ];
    assert_eq!(counts(&rings), [1, 1, 2, 1]);
    let detached = run(&rings, &[Edit::Detach { key: 2 }]);
    assert_eq!(detached.focus, Some(3));
    assert_eq!(keys(&detached.rows), [1, 3, 4, 8, 2]);
    assert_eq!(counts(&detached.rows), [1, 3, 1]);
}

/// What a canonical list asks for, entry by entry, blind to where rows sit
/// and to which chip a plain repeat folds into: each row of a named lone
/// chip's plain stack on its own, any other lone stack as its rows together
/// (with its total), a cluster as each member with its copies. Rows are read
/// without their labels, as `Debug` text so they sort.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
enum Demand {
    Lone(Vec<String>, Option<u8>),
    Cluster(Vec<(String, Vec<String>)>),
}

fn text(requirement: &Requirement) -> String {
    format!(
        "{:?}",
        Requirement {
            identity_group: None,
            alternative_group: None,
            level_sum: None,
            ..*requirement
        }
    )
}

fn texts(requirements: &[Requirement]) -> Vec<String> {
    let mut texts: Vec<String> = requirements.iter().map(text).collect();
    texts.sort();
    texts
}

/// The demands of a lone stack: `rows`, its anchor first.
fn lone_demands(rows: &[Requirement], total: Option<u8>) -> Vec<Demand> {
    if total.is_none() && rows[0].item.is_some() {
        rows.iter()
            .map(|row| Demand::Lone(vec![text(row)], None))
            .collect()
    } else {
        vec![Demand::Lone(texts(rows), total)]
    }
}

/// A cluster's demand: each member with its copies.
fn cluster_demand(stacks: &[(Requirement, Vec<Requirement>)]) -> Demand {
    let mut stacks: Vec<(String, Vec<String>)> = stacks
        .iter()
        .map(|(chip, copies)| (text(chip), texts(copies)))
        .collect();
    stacks.sort();
    Demand::Cluster(stacks)
}

fn requirements_at(rows: &[Row], indices: &[usize]) -> Vec<Requirement> {
    indices
        .iter()
        .map(|&index| rows[index].requirement)
        .collect()
}

/// Every chip of `item` with its copies.
fn stacks_of(rows: &[Row], item: &BoardItem) -> Vec<(Requirement, Vec<Requirement>)> {
    item.stacks
        .iter()
        .map(|stack| {
            (
                rows[stack.index].requirement,
                requirements_at(rows, &stack.copies),
            )
        })
        .collect()
}

/// The demands of one board entry.
fn item_demands(rows: &[Row], item: &BoardItem) -> Vec<Demand> {
    if item.cluster.is_some() {
        return vec![cluster_demand(&stacks_of(rows, item))];
    }
    let stack = &item.stacks[0];
    let all: Vec<usize> = std::iter::once(stack.index)
        .chain(stack.copies.iter().copied())
        .collect();
    lone_demands(&requirements_at(rows, &all), stack.total)
}

fn demands(rows: &[Row]) -> Vec<Demand> {
    let mut all: Vec<Demand> = board_items(rows)
        .iter()
        .flat_map(|item| item_demands(rows, item))
        .collect();
    all.sort();
    all
}

/// A member's stack as a lone chip once its cluster dissolves: a named
/// chip's copies become plain repeats of it.
fn dissolved(chip: Requirement, copies: &[Requirement]) -> Vec<Demand> {
    let rows: Vec<Requirement> = std::iter::once(chip)
        .chain(copies.iter().map(|copy| {
            if chip.item.is_some() {
                plain_copy(&chip, copy.max_depth)
            } else {
                *copy
            }
        }))
        .collect();
    lone_demands(&rows, None)
}

/// What a join of the visible row `source` onto the visible row `target` of
/// a canonical list should leave it asking for: the source's chip one item
/// fewer — a lone stack's last copy gone (a combined level capped at what
/// the rest can still reach), a member's last copy gone, a chip without
/// copies gone from its entry — and the two in one cluster: the target's
/// members with their stacks, or a lone target with its whole stack as a
/// member (plain repeats and a combined level's rings as bare copies), and
/// the item that moved as a ×1 member: a bare copy of the source's item or
/// kind with the last copy's floor limit, or the source itself. Every other
/// entry asks for what it did.
fn joined_demands(rows: &[Row], source: usize, target: usize) -> Vec<Demand> {
    let items = board_items(rows);
    let owner = |index: usize| {
        items
            .iter()
            .position(|item| item.members.contains(&index))
            .expect("a visible row")
    };
    let (from, onto) = (owner(source), owner(target));
    let mut expected: Vec<Demand> = items
        .iter()
        .enumerate()
        .filter(|&(position, _)| position != from && position != onto)
        .flat_map(|(_, item)| item_demands(rows, item))
        .collect();
    let source_item = &items[from];
    let source_stack = source_item.stack(source).expect("a chip");
    let chip = rows[source].requirement;
    let mut copies = requirements_at(rows, &source_stack.copies);
    let moved = copies.pop().map_or(chip, |last| carried_copy(&chip, &last));
    if source_item.cluster.is_some() {
        let stacks: Vec<(Requirement, Vec<Requirement>)> = stacks_of(rows, source_item)
            .into_iter()
            .zip(&source_item.stacks)
            .filter(|(_, stack)| stack.index != source || !stack.copies.is_empty())
            .map(|((member, others), stack)| {
                if stack.index == source {
                    (member, copies.clone())
                } else {
                    (member, others)
                }
            })
            .collect();
        if let [(chip, copies)] = &stacks[..] {
            expected.extend(dissolved(*chip, copies));
        } else {
            expected.push(cluster_demand(&stacks));
        }
    } else if !source_stack.copies.is_empty() {
        let left: Vec<Requirement> = std::iter::once(chip).chain(copies).collect();
        let reach = SumGroup {
            members: u16::try_from(left.len()).expect("a short stack"),
            minimum_total: 0,
            capacity: left
                .iter()
                .map(|ring| u16::from(ring.maximum_level()))
                .sum(),
        }
        .attainable_capacity();
        let total = source_stack
            .total
            .filter(|_| left.len() > 1)
            .map(|total| total.min(u8::try_from(reach).unwrap_or(u8::MAX)));
        expected.extend(lone_demands(&left, total));
    }
    let mut stacks = if items[onto].cluster.is_some() {
        stacks_of(rows, &items[onto])
    } else {
        let stack = &items[onto].stacks[0];
        let chip = rows[target].requirement;
        let copies = stack
            .copies
            .iter()
            .map(|&copy| copy_of(&chip, rows[copy].requirement.max_depth))
            .collect();
        vec![(chip, copies)]
    };
    stacks.push((moved, Vec::new()));
    expected.push(cluster_demand(&stacks));
    expected.sort();
    expected
}

/// A canonical list the board's own edits built: a few random rows, then
/// counts, joins, detaches, removals, combined levels and copy floors.
fn random_board(rng: &mut Rng) -> Vec<Row> {
    let mut rows: Vec<Row> = (1..=u64::from(rng.range(2, 6)))
        .map(|key| Row {
            key,
            requirement: random_requirement(rng),
        })
        .collect();
    for _ in 0..rng.range(1, 7) {
        if rows.is_empty() {
            break;
        }
        let key = |rng: &mut Rng, rows: &[Row]| rows[rng.below(rows.len())].key;
        let picked = key(rng, &rows);
        let edit = match rng.below(8) {
            0 | 1 => Edit::SetCount {
                key: picked,
                count: rng.range(2, STACK_MAX),
            },
            2..=4 => Edit::Join {
                source: picked,
                target: key(rng, &rows),
            },
            5 => Edit::ToggleLevels { key: picked },
            6 => Edit::Detach { key: picked },
            _ => Edit::SetCopyDepth {
                key: picked,
                max_depth: Some(rng.range(1, 24)),
            },
        };
        rows = run(&rows, &[edit]).rows;
    }
    rows
}

/// `rows` with every lone named stack written as a list from elsewhere may
/// write it — the anchor and bare copies under a free stack label — which
/// the board folds the same way.
fn with_bare_copies(rows: &[Row]) -> Vec<Row> {
    let mut encoded = rows.to_vec();
    for item in board_items(rows) {
        let stack = &item.stacks[0];
        let anchor = rows[stack.index].requirement;
        if item.cluster.is_some()
            || stack.total.is_some()
            || stack.copies.is_empty()
            || anchor.item.is_none()
            || !takes_stack_label(&anchor)
        {
            continue;
        }
        let used = taken(
            encoded.iter().map(|row| row.requirement.identity_group),
            &BTreeSet::new(),
        );
        let Some(label) = free_group(&used, MAX_IDENTITY_GROUP) else {
            break;
        };
        encoded[stack.index].requirement.identity_group = Some(label);
        for &index in &stack.copies {
            encoded[index].requirement =
                bare_copy(&anchor, label, rows[index].requirement.max_depth);
        }
    }
    encoded
}

/// The drop policy joins `source` onto `target` in `rows`, and the join
/// writes `after`.
fn assert_joins_to(rows: &[Row], (source, target): (u64, u64), after: &[Row], context: &str) {
    assert_eq!(
        drop_action(rows, source, DropTarget::Row(target)),
        DropAction::Join { target },
        "{context} from {rows:?}"
    );
    assert_eq!(
        run(rows, &[Edit::Join { source, target }]).rows,
        after,
        "{context} from {rows:?}"
    );
}

/// Whether a chip without copies keeps a stack or combined-level label.
fn idle_label(rows: &[Row]) -> bool {
    board_items(rows).iter().any(|item| {
        item.stacks
            .iter()
            .filter(|stack| stack.copies.is_empty())
            .any(|stack| {
                let requirement = rows[stack.index].requirement;
                requirement.identity_group.is_some() || requirement.level_sum.is_some()
            })
    })
}

/// No chip without copies keeps a stack or combined-level label.
fn assert_no_idle_label(rows: &[Row], context: &str) {
    assert!(!idle_label(rows), "{context}");
}

/// Canonical valid lists built by board edits, each joined every way its
/// visible rows allow (1,024 lists, per the test budget): the join keeps
/// every key but adds one only for the copy a member sharing its stack
/// carries; every entry but the two joined asks for what it did, the
/// source's chip gives up one item — a bare copy, or itself when it has
/// none — and the target keeps its stack — no copy is orphaned;
/// no chip without copies keeps a stack or combined-level label; and the
/// result is valid and canonical. The same list with its named stacks
/// written as bare copies joins to the very same rows.
#[test]
fn a_join_moves_one_item_and_leaves_every_copy_where_it_belongs() {
    let mut rng = Rng::new(0x0a1e_c0de_d15a_2026);
    let mut joins = 0;
    let mut member_sources = 0;
    let mut stacked_targets = 0;
    let mut encoded_joins = 0;
    for case in 0..1024 {
        let rows = random_board(&mut rng);
        if validate(&rows).is_err() {
            continue;
        }
        assert!(
            !run(&rows, &[Edit::Normalize]).changed,
            "case {case}: {rows:?}"
        );
        let encoded = with_bare_copies(&rows);
        assert_eq!(counts(&encoded), counts(&rows), "case {case}: {encoded:?}");
        let board = Board::new(&rows);
        let visible: Vec<usize> = board
            .items
            .iter()
            .flat_map(|item| item.members.clone())
            .collect();
        for &source in &visible {
            for &target in &visible {
                let (source_key, target_key) = (rows[source].key, rows[target].key);
                let result = run(
                    &rows,
                    &[Edit::Join {
                        source: source_key,
                        target: target_key,
                    }],
                );
                let context = format!("case {case}: {source_key} onto {target_key} of {rows:?}");
                match drop_action(&rows, source_key, DropTarget::Row(target_key)) {
                    DropAction::Join { .. } => {}
                    DropAction::Refuse(refusal) => {
                        assert_eq!(result.refused, Some(refusal), "{context}");
                        assert_eq!(result.rows, rows, "{context}");
                        continue;
                    }
                    _ => {
                        assert!(!result.changed, "{context}");
                        continue;
                    }
                }
                joins += 1;
                let (from, onto) = (
                    board.stack(source).expect("a chip"),
                    board.stack(target).expect("a chip"),
                );
                member_sources += usize::from(from.in_cluster && from.count() > 1);
                stacked_targets += usize::from(!onto.in_cluster && onto.count() > 1);
                let after = &result.rows;
                let context = format!("{context} → {after:?}");
                let shared = from.in_cluster
                    && board.member_of[source].is_some_and(|position| {
                        shares_label(&rows, &board.items[position], source)
                    });
                let before_keys: BTreeSet<u64> = keys(&rows).into_iter().collect();
                let after_keys: BTreeSet<u64> = keys(after).into_iter().collect();
                assert!(after_keys.is_superset(&before_keys), "{context}");
                assert!(shared || after_keys == before_keys, "{context}");
                assert_eq!(
                    demands(after),
                    joined_demands(&rows, source, target),
                    "{context}"
                );
                let moved = result.focus.expect("a join follows the item it moved");
                let joined = entry(after, moved);
                assert!(joined.cluster.is_some(), "{context}");
                assert!(
                    joined
                        .members
                        .contains(&index_of(after, target_key).unwrap()),
                    "{context}"
                );
                assert_no_idle_label(after, &context);
                assert_eq!(validate(after), Ok(()), "{context}");
                assert_emittable(after, &context);
                assert!(!run(after, &[Edit::Normalize]).changed, "{context}");
                if encoded != rows {
                    encoded_joins += 1;
                    assert_joins_to(&encoded, (source_key, target_key), after, &context);
                }
            }
        }
    }
    assert!(joins > 2000, "only {joins} joins");
    assert!(member_sources > 100, "only {member_sources} member sources");
    assert!(
        stacked_targets > 200,
        "only {stacked_targets} stacked targets"
    );
    assert!(
        encoded_joins > 200,
        "only {encoded_joins} joins of bare copies"
    );
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
    let rows: Vec<Row> = rows
        .into_iter()
        .chain([with(named(9, ItemId::RingMight), |r| r.blanket = true)])
        .collect();
    let result = run(
        &rows,
        &[
            Edit::Join {
                source: 2,
                target: 1,
            },
            Edit::ToggleLevels { key: 9 },
            Edit::Remove { key: 4 },
        ],
    );
    assert_eq!(result.refused, Some(Refusal::BlanketTotal));
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

/// `changed` compares the final rows with the request's, so edits that
/// undo each other within one request report no change — Android writes the
/// rows back only when it is set. The focus stays where the last edit that
/// applied left it, as an unchanged save names its chip.
#[test]
fn edits_that_undo_each_other_change_nothing() {
    let rows = [named(1, ItemId::WandFrost)];
    let there_and_back = [
        Edit::SetCount { key: 1, count: 2 },
        Edit::SetCount { key: 1, count: 1 },
    ];
    let result = run(&rows, &there_and_back);
    assert_eq!(result.rows, rows);
    assert!(!result.changed);
    assert_eq!((result.focus, result.refused), (Some(1), None));
    assert_eq!(result.next_key, 2);

    let unchanged = run(&rows, &[resaved(1, rows[0].requirement, 1, None, None)]);
    assert_eq!(unchanged.rows, rows);
    assert_eq!((unchanged.changed, unchanged.focus), (false, Some(1)));

    // Only a key repair makes such a request a change.
    let broken = [Row { key: 0, ..rows[0] }];
    let repaired = run(&broken, &there_and_back);
    assert!(repaired.changed);
    assert_eq!(repaired.rekeyed, [(0, 1)]);
    assert_eq!(repaired.rows, rows);
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
    assert_eq!(entry(&result.rows, 1).total(), Some(3));
}

/// A member's stack takes a label of its own unless it is alike another
/// member's; with none of the four free, the edit that needs one is refused
/// — a count, a detach or a removal of one item that leaves a stack its
/// members shared — and the drop policy says so. An edit that needs no new
/// label goes through: stepping a member down to ×1, or up to a stack alike
/// another member's.
#[test]
fn member_stacks_run_out_of_labels_with_a_refusal() {
    let stacked = |key, label| with(row(key, ItemKind::Ring), |r| r.identity_group = Some(label));
    let others = |labels: &[u8]| -> Vec<Row> {
        labels
            .iter()
            .flat_map(|&label| {
                let key = u64::from(label) * 10;
                [stacked(key, label), stacked(key + 1, label)]
            })
            .collect()
    };
    // {Frost ×3 | Disintegration ×3} sharing label 1, beside three stacks.
    let mut shared = vec![
        member(named(1, ItemId::WandFrost), 1, Some(1)),
        member(named(2, ItemId::WandDisintegration), 1, Some(1)),
        bare_wand(3, 1),
        bare_wand(4, 1),
    ];
    shared.extend(others(&[2, 3, 4]));
    assert!(problems(&shared).is_empty());
    for edit in [
        Edit::SetCount { key: 1, count: 2 },
        Edit::Detach { key: 1 },
        Edit::RemoveOne { key: 1 },
        Edit::SetCopyDepth {
            key: 1,
            max_depth: Some(9),
        },
    ] {
        let result = run(&shared, &[edit]);
        assert_eq!(result.refused, Some(Refusal::NoFreeGroup), "{edit:?}");
        assert_eq!(result.rows, shared, "{edit:?}");
    }
    assert_eq!(
        drop_action(&shared, 1, DropTarget::Board { blanket: false }),
        DropAction::Refuse(Refusal::NoFreeGroup)
    );
    assert_eq!(
        drop_action(&shared, 1, DropTarget::Remove),
        DropAction::Refuse(Refusal::NoFreeGroup)
    );
    // Down to ×1 needs no label.
    let one = edited(&shared, &[Edit::SetCount { key: 1, count: 1 }]);
    assert_eq!(counts(&one)[..2], [1, 3]);
    // Up to a stack alike another member's shares its label, freeing one.
    let mut apart = vec![
        member(named(1, ItemId::WandFrost), 1, Some(1)),
        member(named(2, ItemId::WandDisintegration), 1, Some(2)),
        bare_wand(3, 1),
        bare_wand(4, 2),
        bare_wand(5, 2),
    ];
    apart.extend(others(&[3, 4]));
    let alike = edited(&apart, &[Edit::SetCount { key: 1, count: 3 }]);
    assert_eq!(counts(&alike)[..2], [3, 3]);
    assert_eq!(labels_of(&alike)[..4], [Some(2), Some(2), Some(2), Some(2)]);
    // A lone named stack joined as a target needs one too.
    let mut lone = vec![
        named(1, ItemId::WandFrost),
        named(2, ItemId::WandFrost),
        named(3, ItemId::WandDisintegration),
    ];
    lone.extend(others(&[1, 2, 3, 4]));
    let result = run(
        &lone,
        &[Edit::Join {
            source: 3,
            target: 1,
        }],
    );
    assert_eq!(result.refused, Some(Refusal::NoFreeGroup));
    assert_eq!(
        drop_action(&lone, 3, DropTarget::Row(1)),
        DropAction::Refuse(Refusal::NoFreeGroup)
    );
    // The other way round moves one Frost and needs none.
    assert_eq!(
        drop_action(&lone, 1, DropTarget::Row(3)),
        DropAction::Join { target: 3 }
    );
    assert_emittable(
        &edited(
            &lone,
            &[Edit::Join {
                source: 1,
                target: 3,
            }],
        ),
        "no label",
    );
}

/// The document codec reads any byte as a stack or combined-level label,
/// and the engine searches any label but 0, while the portable formats and
/// every platform's model stop at four. Normalizing moves a label out of
/// range onto a free one, as wide alternative labels are compacted, and
/// never merges two groups to make them fit.
#[test]
fn normalizing_moves_labels_out_of_range_onto_free_ones() {
    let stacked = |key, label| with(row(key, ItemKind::Wand), |r| r.identity_group = Some(label));
    // Linux's hand-edited state: a stack labelled 7.
    let rows = [stacked(1, 7), stacked(2, 7)];
    assert_eq!(
        problems(&rows)
            .iter()
            .map(|problem| problem.message.as_str())
            .collect::<Vec<_>>(),
        ["A stack group must be 1 through 4."; 2]
    );
    let result = run(&rows, &[Edit::Normalize]);
    assert!(result.changed);
    assert_eq!(keys(&result.rows), [1, 2]);
    assert_eq!(
        result
            .rows
            .iter()
            .map(|row| row.requirement.identity_group)
            .collect::<Vec<_>>(),
        [Some(1); 2]
    );
    assert!(problems(&result.rows).is_empty());
    assert_eq!(validate(&result.rows), Ok(()));
    assert!(!run(&result.rows, &[Edit::Normalize]).changed);

    // Labels in range stay; each group out of range takes the lowest free
    // label in the order it first appears, the reserved 0 included.
    let rows = [
        stacked(1, 9),
        stacked(2, 2),
        stacked(3, 9),
        stacked(4, 2),
        stacked(5, 0),
        stacked(6, 0),
        with(named(7, ItemId::RingMight), |r| {
            r.level_sum = Some(LevelSum {
                group: 200,
                minimum_total: 2,
            });
        }),
        with(named(8, ItemId::RingMight), |r| {
            r.level_sum = Some(LevelSum {
                group: 200,
                minimum_total: 2,
            });
        }),
        with(row(9, ItemKind::Armor), |r| r.alternative_group = Some(0)),
        with(row(10, ItemKind::Ring), |r| r.alternative_group = Some(0)),
    ];
    let result = run(&rows, &[Edit::Normalize]);
    let labels = |pick: fn(&Requirement) -> Option<u8>| {
        result
            .rows
            .iter()
            .map(|row| pick(&row.requirement))
            .collect::<Vec<_>>()
    };
    assert_eq!(
        labels(|r| r.identity_group),
        [
            Some(1),
            Some(2),
            Some(1),
            Some(2),
            Some(3),
            Some(3),
            None,
            None,
            None,
            None
        ]
    );
    assert_eq!(
        labels(|r| r.level_sum.map(|sum| sum.group))[6..8],
        [Some(1), Some(1)]
    );
    assert_eq!(labels(|r| r.alternative_group)[8..], [Some(1), Some(1)]);
    assert!(problems(&result.rows).is_empty(), "{:?}", result.rows);
    assert_emittable(&result.rows, "relabelled");

    // Five stacks and four labels: the fifth keeps its own, and says so.
    let rows: Vec<Row> = (0..5)
        .flat_map(|group: u8| {
            let key = u64::from(group) * 2 + 1;
            [stacked(key, 10 + group), stacked(key + 1, 10 + group)]
        })
        .collect();
    let result = run(&rows, &[Edit::Normalize]);
    assert_eq!(
        labels_of(&result.rows),
        [1, 1, 2, 2, 3, 3, 4, 4, 14, 14].map(Some)
    );
    assert_eq!(
        problems(&result.rows)
            .iter()
            .map(|problem| problem.keys.clone())
            .collect::<Vec<_>>(),
        [[9], [10]]
    );
    assert_eq!(counts(&result.rows), [2; 5]);

    // Any effective edit leaves the list in range; a no-op leaves it alone.
    let rows = [stacked(1, 7), stacked(2, 7), row(3, ItemKind::Ring)];
    assert!(!run(&rows, &[Edit::Detach { key: 3 }]).changed);
    let grown = run(&rows, &[Edit::SetCount { key: 3, count: 2 }]);
    assert_eq!(labels_of(&grown.rows), [Some(2), Some(2), Some(1), Some(1)]);
    assert_eq!(grown.focus, Some(3));
}

fn labels_of(rows: &[Row]) -> Vec<Option<u8>> {
    rows.iter()
        .map(|row| row.requirement.identity_group)
        .collect()
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

    // The web's cluster ×3 is each member ×3: their alike stacks share one
    // label and one set of copies.
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
            Edit::SetCount { key: 2, count: 3 },
        ],
    );
    assert_eq!(keys(&cluster), [2, 1, 3, 4]);
    assert_eq!(counts(&cluster), [3, 3]);

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
        let stacks = |rows: &[Row]| {
            board_items(rows)
                .iter()
                .flat_map(|item| item.stacks.iter().map(|stack| (stack.count(), stack.total)))
                .collect::<Vec<_>>()
        };
        assert_eq!(stacks(&decoded), stacks(rows));
        assert_eq!(validate(&decoded), Ok(()));
    }
}

// --- properties ---------------------------------------------------------

/// Random valid rows × random edit sequences (1,024 cases, per the test
/// budget): nothing panics, every emitted list keeps the §3 invariant —
/// labels in 1–4, or the edit is refused — and shows every row, `changed`
/// says exactly whether the rows differ, a no-op or a refusal returns the
/// rows verbatim, the focus names a visible row, whatever an edit wrote is
/// canonical and keeps no label on a chip without copies, a sequence equals
/// its edits applied one by one, and edits are deterministic.
#[test]
fn random_edits_on_valid_rows_keep_every_row_emittable() {
    let mut rng = Rng::new(0x5eed_5eed_c0de_2024);
    for case in 0..1024 {
        let rows = random_rows(&mut rng);
        assert_emittable(&rows, "the generator");
        assert_every_row_shown(&rows, "the generator");
        let edits: Vec<Edit> = (0..rng.range(1, 4))
            .map(|_| random_edit(&mut rng, &rows))
            .collect();
        let hint = rng.chance(20).then(|| rng.next() % 30);
        let result = apply(&rows, hint, &edits);
        let context = format!("case {case}: {rows:?} {edits:?}");
        assert_emittable(&result.rows, &context);
        assert_every_row_shown(&result.rows, &context);
        assert!(result.rekeyed.is_empty(), "{context}");
        assert!(
            result.next_key > result.rows.iter().map(|row| row.key).max().unwrap_or(0),
            "{context}"
        );
        assert_eq!(result.changed, result.rows != rows, "{context}");
        assert_eq!(apply(&rows, hint, &edits), result, "{context}");
        // Whatever an edit wrote is canonical: normalizing it again is a
        // no-op.
        assert!(
            !result.changed || !apply(&result.rows, hint, &[Edit::Normalize]).changed,
            "{context}"
        );

        // One edit at a time: each step keeps the invariant, a refusal or a
        // no-op leaves its input alone, and the steps add up to the sequence.
        let mut current = rows.clone();
        let mut refused = None;
        for &edit in &edits {
            let step = apply(&current, hint, &[edit]);
            let context = format!("{context} at {edit:?}");
            assert_emittable(&step.rows, &context);
            // An edit that wrote a valid list left no label on a chip
            // without copies — unless a hand-written list tied a lone chip
            // to a cluster's members with one, which no board shape says
            // and which the edit carried through.
            if step.changed && validate(&step.rows).is_ok() && !idle_label(&current) {
                assert_no_idle_label(&step.rows, &context);
            }
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
                        let joined = entry(&step.rows, step.focus.unwrap_or(source));
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
/// folds at once — never make an edit panic, the board shows every row
/// before and after, `changed` says exactly whether the rows differ or keys
/// were repaired, and the keys always come back unique and in range (1,024
/// cases).
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
        assert_every_row_shown(&rows, &context);
        assert_every_row_shown(&result.rows, &context);
        assert_eq!(unique.len(), result.rows.len(), "{context}");
        assert!(
            result.rows.iter().all(|row| super::is_valid_key(row.key)),
            "{context}"
        );
        assert_eq!(
            result.changed,
            !result.rekeyed.is_empty() || result.rows != rows,
            "{context}"
        );
        let _ = join_candidates(&result.rows, &board_items(&result.rows));
    }
}

/// Random valid lists written under other labels — one distinct byte per
/// group, the reserved 0 among them — read as the very board their original
/// reads, and normalizing writes every label back in range (1,024 cases).
#[test]
fn a_list_under_any_labels_normalizes_to_the_same_board() {
    let mut rng = Rng::new(0x1abe_15a1_1b17_2026);
    // A distinct byte for each label 1..=4 the generator writes.
    let spread = |rng: &mut Rng| {
        let mut labels = [0_u8; 5];
        for index in 1..labels.len() {
            labels[index] = loop {
                let label = u8::try_from(rng.below(256)).expect("a byte");
                if !labels[1..index].contains(&label) {
                    break label;
                }
            };
        }
        labels
    };
    let at = |rows: &[Row], indices: &[usize]| -> Vec<u64> {
        indices.iter().map(|&index| rows[index].key).collect()
    };
    let shape = |rows: &[Row]| {
        board_items(rows)
            .iter()
            .map(|item| {
                (
                    at(rows, &item.members),
                    item.stacks
                        .iter()
                        .map(|stack| (at(rows, &stack.copies), stack.total))
                        .collect::<Vec<_>>(),
                    item.cluster.is_some(),
                )
            })
            .collect::<Vec<_>>()
    };
    let sorted = |rows: &[Row]| {
        let mut found: Vec<(Vec<u64>, String)> = problems(rows)
            .into_iter()
            .map(|problem| (problem.keys, problem.message))
            .collect();
        found.sort();
        found
    };
    for case in 0..1024 {
        let rows = random_rows(&mut rng);
        let (identity, sum, alternative) = (spread(&mut rng), spread(&mut rng), spread(&mut rng));
        let wide: Vec<Row> = rows
            .iter()
            .map(|row| {
                with(*row, |r| {
                    r.identity_group = r.identity_group.map(|label| identity[usize::from(label)]);
                    if let Some(level) = &mut r.level_sum {
                        level.group = sum[usize::from(level.group)];
                    }
                    r.alternative_group = r
                        .alternative_group
                        .map(|label| alternative[usize::from(label)]);
                })
            })
            .collect();
        let context = format!("case {case}: {wide:?}");
        let narrow = run(&rows, &[Edit::Normalize]).rows;
        let normalized = run(&wide, &[Edit::Normalize]);
        assert_emittable(&normalized.rows, &context);
        assert_eq!(keys(&normalized.rows), keys(&rows), "{context}");
        assert_eq!(shape(&wide), shape(&rows), "{context}");
        assert_eq!(shape(&normalized.rows), shape(&narrow), "{context}");
        assert_eq!(sorted(&normalized.rows), sorted(&narrow), "{context}");
        assert!(
            !run(&normalized.rows, &[Edit::Normalize]).changed,
            "{context}"
        );
    }
}

// --- what the engine reads ------------------------------------------------

/// The items the meaning test draws requirements and worlds from: few
/// enough that a world often holds what a board asks for.
const POOL: [ItemId; 7] = [
    ItemId::WandFrost,
    ItemId::WandDisintegration,
    ItemId::WandLightning,
    ItemId::RingMight,
    ItemId::RingEnergy,
    ItemId::Spear,
    ItemId::PlateArmor,
];

fn pool_requirement(rng: &mut Rng) -> Requirement {
    let id = rng.pick(&POOL);
    Requirement {
        item: rng.chance(65).then_some(id),
        upgrade: if rng.chance(25) {
            UpgradeRequirement::AtLeast(1)
        } else {
            UpgradeRequirement::Any
        },
        max_depth: rng.chance(20).then(|| rng.range(3, 9)),
        ..Requirement::any(crate::catalog::item(id).kind)
    }
}

fn pool_world(rng: &mut Rng) -> crate::model::GeneratedWorld {
    use crate::model::{Accessibility, GeneratedWorld, ItemSource, WorldItem};
    let items = (0..rng.range(3, 8))
        .map(|_| WorldItem {
            item: rng.pick(&POOL),
            upgrade: rng.range(0, 3),
            effect: None,
            cursed: rng.chance(15),
            depth: rng.range(1, 10),
            source: ItemSource::Heap,
            accessibility: Accessibility::Independent,
            secret: false,
        })
        .collect();
    GeneratedWorld {
        floor_rooms: Vec::new(),
        artifact_decks: Vec::new(),
        feelings: Vec::new(),
        seed: crate::seed::DungeonSeed::MIN,
        items,
        quests: crate::quests::QuestSummary::default(),
        ring_gems: crate::run::RingGems::UNSHUFFLED,
    }
}

/// A list the board's own edits built from pool requirements: counts,
/// joins, detaches, removals of one item or of a chip's stack, copy floors,
/// combined levels and their totals, and sheet saves — a member's or a lone
/// chip's count and copy floor, sometimes as another item or category.
fn pool_board(rng: &mut Rng) -> Vec<Row> {
    let mut rows: Vec<Row> = (1..=u64::from(rng.range(2, 5)))
        .map(|key| Row {
            key,
            requirement: pool_requirement(rng),
        })
        .collect();
    for _ in 0..rng.range(2, 8) {
        if rows.is_empty() {
            break;
        }
        let key = |rng: &mut Rng, rows: &[Row]| rows[rng.below(rows.len())].key;
        let picked = key(rng, &rows);
        let edit = match rng.below(14) {
            0..=2 => Edit::SetCount {
                key: picked,
                count: rng.range(2, STACK_MAX),
            },
            3..=5 => Edit::Join {
                source: picked,
                target: key(rng, &rows),
            },
            6 => Edit::Detach { key: picked },
            7 => Edit::RemoveOne { key: picked },
            8 => Edit::Remove { key: picked },
            9 => Edit::SetCopyDepth {
                key: picked,
                max_depth: rng.chance(70).then(|| rng.range(2, 6)),
            },
            10 => Edit::ToggleLevels { key: picked },
            11 => Edit::SetTotal {
                key: picked,
                total: rng.chance(80).then(|| rng.range(1, 9)),
            },
            _ => {
                let own = rows[index_of(&rows, picked).expect("a listed key")].requirement;
                let requirement = if rng.chance(60) {
                    Requirement {
                        max_depth: rng.chance(20).then(|| rng.range(3, 9)),
                        ..own
                    }
                } else {
                    pool_requirement(rng)
                };
                Edit::Save {
                    key: Some(picked),
                    requirement,
                    count: rng.range(1, 3),
                    total: None,
                    copy_depth: rng.chance(30).then(|| rng.range(2, 6)),
                }
            }
        };
        rows = run(&rows, &[edit]).rows;
    }
    rows
}

/// What the board shows `rows` to ask for, as the queries any one of which
/// a world must match — read off the fold, not the labels: every lone
/// chip's stack as its row with bare copies of its kind tied to it by a
/// label of its own (a combined level as its members), and every cluster as
/// each way of filling it — one member, with its copies tied to it the same
/// way.
fn board_meaning(rows: &[Row]) -> Vec<Vec<Requirement>> {
    let mut label = 0_u8;
    let mut stack = |stack: &ChipStack| -> Vec<Requirement> {
        let chip = Requirement {
            alternative_group: None,
            identity_group: None,
            ..rows[stack.index].requirement
        };
        let copies = stack.copies.iter().map(|&copy| rows[copy].requirement);
        if stack.total.is_some() {
            return std::iter::once(chip).chain(copies).collect();
        }
        if stack.copies.is_empty() {
            return vec![chip];
        }
        label += 1;
        std::iter::once(Requirement {
            identity_group: Some(label),
            ..chip
        })
        .chain(copies.map(|copy| Requirement {
            identity_group: Some(label),
            max_depth: copy.max_depth,
            ..Requirement::any(chip.kind)
        }))
        .collect()
    };
    let mut variants: Vec<Vec<Requirement>> = vec![Vec::new()];
    for item in board_items(rows) {
        let options: Vec<Vec<Requirement>> = item.stacks.iter().map(&mut stack).collect();
        variants = variants
            .iter()
            .flat_map(|variant| {
                options
                    .iter()
                    .map(move |option| variant.iter().chain(option).copied().collect())
            })
            .collect();
    }
    variants
}

/// A list written elsewhere with member stacks — a label on some members
/// of a cluster, on all of them, or alike stacks under labels of their own —
/// beside a lone chip or two.
fn written_board(rng: &mut Rng) -> Vec<Row> {
    let mut list: Vec<Requirement> = Vec::new();
    let mut copies: Vec<Requirement> = Vec::new();
    let mut label = 0_u8;
    for group in 1..=rng.range(1, 2) {
        // Each stack so far: its kind, label, copy count and copy floor.
        let mut stacks: Vec<(ItemKind, u8, usize, Option<u8>)> = Vec::new();
        for _ in 0..rng.range(2, 3) {
            let mut member = Requirement {
                alternative_group: Some(group),
                ..pool_requirement(rng)
            };
            let alike = stacks
                .iter()
                .find(|&&(kind, ..)| kind == member.kind)
                .copied();
            if rng.chance(70) && label < MAX_IDENTITY_GROUP {
                match alike {
                    // Share an alike stack's label.
                    Some((_, shared, ..)) if rng.chance(40) => {
                        member.identity_group = Some(shared);
                    }
                    // A label of its own: alike copies now and then.
                    _ => {
                        label += 1;
                        let (count, floor) = match alike {
                            Some((_, _, count, floor)) if rng.chance(50) => (count, floor),
                            _ => (rng.below(2) + 1, rng.chance(30).then_some(5)),
                        };
                        member.identity_group = Some(label);
                        stacks.push((member.kind, label, count, floor));
                        copies.extend((0..count).map(|_| Requirement {
                            identity_group: Some(label),
                            max_depth: floor,
                            ..Requirement::any(member.kind)
                        }));
                    }
                }
            }
            list.push(member);
        }
    }
    for _ in 0..rng.below(2) {
        list.push(pool_requirement(rng));
    }
    list.extend(copies);
    list.into_iter()
        .zip(1..)
        .map(|(requirement, key)| Row { key, requirement })
        .collect()
}

/// Every board the editor writes means to the engine what it shows: a list
/// built by random board edits ([`pool_board`]) matches a generated world
/// exactly when one of the board's own readings does. A list written
/// elsewhere ([`written_board`]) reads the same way on the board, and
/// normalizing it changes nothing a search sees. 128 cases of one list of
/// each kind against 4 worlds (1,024 matches), the worlds drawn afresh
/// every 16 cases. Member stacks are the engine's: a member's copies count
/// only when it fills its cluster's slot.
#[test]
fn every_board_the_editor_writes_means_to_the_engine_what_it_shows() {
    let mut rng = Rng::new(0x3ea0_1b0a_2d5e_e5e5);
    let mut worlds = Vec::new();
    let (mut matched, mut missed, mut member_stacks, mut lists) = (0, 0, 0, 0);
    for case in 0..128 {
        if case % 16 == 0 {
            worlds = (0..4).map(|_| pool_world(&mut rng)).collect();
        }
        for (edited, rows) in [
            (true, pool_board(&mut rng)),
            (false, written_board(&mut rng)),
        ] {
            if validate(&rows).is_err() {
                continue;
            }
            lists += 1;
            let normalized = run(&rows, &[Edit::Normalize]).rows;
            assert!(!edited || normalized == rows, "case {case}: {rows:?}");
            let written = query(requirements(&rows));
            let stored = query(requirements(&normalized));
            member_stacks +=
                usize::from(!crate::query::stack_gates(&written.requirements).is_empty());
            let meaning: Vec<_> = board_meaning(&rows).into_iter().map(query).collect();
            for (index, world) in worlds.iter().enumerate() {
                let context = format!("case {case}, world {index}: {rows:?}");
                let shown = meaning.iter().any(|variant| variant.matches(world));
                assert_eq!(written.matches(world), shown, "{context}");
                assert_eq!(stored.matches(world), shown, "{context} → {normalized:?}");
                if shown {
                    matched += 1;
                } else {
                    missed += 1;
                }
            }
        }
    }
    assert!(lists > 200, "only {lists} valid lists");
    assert!(
        matched > 100 && missed > 100,
        "{matched} matched, {missed} missed"
    );
    assert!(
        member_stacks > 40,
        "only {member_stacks} lists with member stacks"
    );
}
