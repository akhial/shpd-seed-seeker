//! The envelopes: the row codec, board and sheet requests end to end, the
//! error envelopes, unreadable rows, key repair and label compaction, and
//! the properties that keep the projection honest — the board envelope says
//! exactly what the typed editor says, no request panics, and every row an
//! envelope writes reads back.

use std::collections::BTreeSet;

use serde_json::{Value, json};

use super::super::testing::{
    Rng, mixed_rows, named, random_change, random_edit, random_resin, random_rows, row, with,
};
use super::super::{
    Change, DRAFT_VERSION, Edit, MAX_KEY, NO_ORDINARY_REQUIREMENT, RESIN_AMOUNT_RANGE, Row, apply,
    board_view, open,
};
use super::*;
use crate::catalog::{ItemId, ItemKind};
use crate::query::{LevelSum, UpgradeRequirement};

// --- helpers -------------------------------------------------------------

fn board_json(request: &Value) -> Value {
    serde_json::from_str(&requirement_board(&request.to_string())).expect("the board answers JSON")
}

fn editor_json(request: &Value) -> Value {
    serde_json::from_str(&requirement_editor(&request.to_string())).expect("the sheet answers JSON")
}

fn rows_json(rows: &[Row]) -> Value {
    Value::Array(rows.iter().map(write_row).collect())
}

/// Reads rows an envelope wrote; every one must be readable.
fn read_back(rows: &Value) -> Vec<Row> {
    let (entries, relabelled) = read_rows(rows.as_array().expect("rows").clone()).unwrap();
    assert!(!relabelled, "{rows}");
    entries
        .into_iter()
        .map(|entry| match entry {
            Entry::Row(row) => row,
            Entry::Raw(raw) => panic!("unreadable row written: {raw:?}"),
        })
        .collect()
}

fn keys_of(rows: &Value) -> Vec<u64> {
    rows.as_array()
        .expect("rows")
        .iter()
        .map(|row| row["key"].as_u64().expect("a key"))
        .collect()
}

fn exact(mut row: Row, upgrade: u8) -> Row {
    row.requirement.upgrade = UpgradeRequirement::Exact(upgrade);
    row
}

/// An EDIT as a platform writes it.
fn wire_edit(edit: &Edit) -> Value {
    match *edit {
        Edit::Normalize => json!({"type": "normalize"}),
        Edit::Join { source, target } => {
            json!({"type": "join", "source": source, "target": target})
        }
        Edit::Detach { key } => json!({"type": "detach", "key": key}),
        Edit::Remove { key } => json!({"type": "remove", "key": key}),
        Edit::RemoveItem { key } => json!({"type": "remove_item", "key": key}),
        Edit::SetCount { key, count } => json!({"type": "set_count", "key": key, "count": count}),
        Edit::SetTotal { key, total } => json!({"type": "set_total", "key": key, "total": total}),
        Edit::ToggleLevels { key } => json!({"type": "toggle_levels", "key": key}),
        Edit::SetCopyDepth { key, max_depth } => {
            json!({"type": "set_copy_depth", "key": key, "max_depth": max_depth})
        }
        Edit::Save {
            key,
            requirement,
            count,
            total,
            copy_depth,
        } => json!({
            "type": "save",
            "key": key,
            "requirement": Value::Object(requirement_fields(&requirement)),
            "count": count,
            "total": total,
            "copy_depth": copy_depth,
        }),
    }
}

/// A CHANGE as a platform writes it.
fn wire_change(change: &Change) -> Value {
    let (kind, value) = match *change {
        Change::SetCategory(kind) => ("set_category", json!(kind_name(kind))),
        Change::SetWeaponType(category) => ("set_weapon_type", json!(weapon_type_name(category))),
        Change::SetKind(kind, category) => ("set_kind", json!(KindName::of(kind, category).name())),
        Change::SetItem(choice) => ("set_item", item_choice_value(choice)),
        Change::SetTierMode(mode) => ("set_tier_mode", json!(tier_mode_name(mode))),
        Change::SetTier(tier) => ("set_tier", json!(tier)),
        Change::SetUpgradeMode(mode) => ("set_upgrade_mode", json!(upgrade_mode_name(mode))),
        Change::SetUpgrade(upgrade) => ("set_upgrade", json!(upgrade)),
        Change::SetEffectMode(mode) => ("set_effect_mode", json!(effect_mode_name(mode))),
        Change::ToggleEffect(effect) => ("toggle_effect", json!(effect.wire_name())),
        Change::SetUncursed(value) => ("set_uncursed", json!(value)),
        Change::SetSource(source) => ("set_source", source_value(source)),
        Change::SetFloorLimitEnabled(value) => ("set_floor_limit_enabled", json!(value)),
        Change::SetFloorLimit(value) => ("set_floor_limit", json!(value)),
        Change::SetExcludeResin(value) => ("set_exclude_resin", json!(value)),
        Change::SetTransmutationsEnabled(value) => ("set_transmutations_enabled", json!(value)),
        Change::SetTransmutations(value) => ("set_transmutations", json!(value)),
        Change::SetSelectTrinket(value) => ("set_select_trinket", json!(value)),
        Change::SetCount(value) => ("set_count", json!(value)),
        Change::SetCopyDepthEnabled(value) => ("set_copy_depth_enabled", json!(value)),
        Change::SetCopyDepth(value) => ("set_copy_depth", json!(value)),
        Change::SetCountLevels(value) => ("set_count_levels", json!(value)),
        Change::SetTotal(value) => ("set_total", json!(value)),
        Change::SetResinAuto(value) => ("set_resin_auto", json!(value)),
        Change::SetResinAmount(value) => ("set_resin_amount", json!(value)),
        Change::SetIncludeMageWand(value) => ("set_include_mage_wand", json!(value)),
    };
    json!({"type": kind, "value": value})
}

/// A request's draft after `changes`, through the envelope.
fn changed_draft(draft: &Value, changes: &[Value]) -> Value {
    changes.iter().fold(draft.clone(), |draft, change| {
        let response = editor_json(&json!({"op": "change", "draft": draft, "change": change}));
        assert!(response.get("error").is_none(), "{change}: {response}");
        response["draft"].clone()
    })
}

// --- the row codec -------------------------------------------------------

#[test]
fn rows_read_back_exactly_as_written() {
    let mut rng = Rng::new(0x0c0d_ec0d_ec0d_ec01);
    for case in 0..1_024 {
        let rows = random_rows(&mut rng);
        let written = rows_json(&rows);
        assert_eq!(read_back(&written), rows, "case {case}: {written}");
    }
}

#[test]
fn a_row_is_the_document_requirement_object_with_its_key_and_label() {
    let ring = with(exact(named(7, ItemId::RingMight), 2), |requirement| {
        requirement.alternative_group = Some(3);
        requirement.level_sum = Some(LevelSum {
            group: 1,
            minimum_total: 4,
        });
    });
    assert_eq!(
        write_row(&ring),
        json!({
            "key": 7,
            "kind": "ring",
            "item": "ring_might",
            "upgrade": 2,
            "alternative_group": 3,
            "level_sum": {"group": 1, "at_least": 4},
        })
    );
    // The document's own spellings read too, and write back canonically.
    let (entries, _) = read_rows(vec![json!({
        "key": 1,
        "item": "ring_might",
        "upgrade": {"exact": 2},
        "tier": "any",
    })])
    .unwrap();
    let [Entry::Row(read)] = &entries[..] else {
        panic!("{entries:?}");
    };
    assert_eq!(
        write_row(read),
        json!({"key": 1, "kind": "ring", "item": "ring_might", "upgrade": 2})
    );
}

/// The four stack shapes Android's suite captured from the web encoder, as
/// the board envelope writes them: the same objects, each with its key.
#[test]
fn the_four_stack_shapes_travel_as_the_web_writes_them() {
    let might = |key, upgrade| exact(named(key, ItemId::RingMight), upgrade);
    let rows_after = |rows: &[Row], edits: &[Value]| {
        let response = board_json(&json!({"rows": rows_json(rows), "edits": edits}));
        assert!(response.get("error").is_none(), "{response}");
        response["rows"].clone()
    };
    let concrete = rows_after(
        &[might(1, 2)],
        &[json!({"type": "set_count", "key": 1, "count": 3})],
    );
    assert_eq!(
        concrete,
        json!([
            {"key": 1, "kind": "ring", "item": "ring_might", "upgrade": 2},
            {"key": 2, "kind": "ring", "item": "ring_might"},
            {"key": 3, "kind": "ring", "item": "ring_might"},
        ])
    );
    let wildcard = rows_after(
        &[exact(row(1, ItemKind::Wand), 3)],
        &[json!({"type": "set_count", "key": 1, "count": 3})],
    );
    assert_eq!(
        wildcard,
        json!([
            {"key": 1, "kind": "wand", "upgrade": 3, "identity_group": 1},
            {"key": 2, "kind": "wand", "identity_group": 1},
            {"key": 3, "kind": "wand", "identity_group": 1},
        ])
    );
    let totalled = rows_after(
        &[might(1, 2)],
        &[
            json!({"type": "set_count", "key": 1, "count": 2}),
            json!({"type": "set_total", "key": 1, "total": 3}),
        ],
    );
    assert_eq!(
        totalled,
        json!([
            {"key": 1, "kind": "ring", "item": "ring_might", "level_sum": {"group": 1, "at_least": 3}},
            {"key": 2, "kind": "ring", "item": "ring_might", "level_sum": {"group": 1, "at_least": 3}},
        ])
    );
    let cluster = rows_after(
        &[
            exact(row(1, ItemKind::Wand), 3),
            exact(named(2, ItemId::WandFireblast), 3),
        ],
        &[
            json!({"type": "join", "source": 1, "target": 2}),
            json!({"type": "set_count", "key": 1, "count": 3}),
        ],
    );
    assert_eq!(
        cluster,
        json!([
            {"key": 2, "kind": "wand", "item": "wand_fireblast", "upgrade": 3,
             "identity_group": 1, "alternative_group": 1},
            {"key": 1, "kind": "wand", "upgrade": 3, "identity_group": 1, "alternative_group": 1},
            {"key": 3, "kind": "wand", "identity_group": 1},
            {"key": 4, "kind": "wand", "identity_group": 1},
        ])
    );
}

// --- the board envelope --------------------------------------------------

/// The board envelope is a projection: for every list and every edit
/// sequence it answers exactly what [`apply`] and [`board_view`] say.
#[test]
fn the_board_envelope_says_what_the_typed_editor_says() {
    let mut rng = Rng::new(0x0b0a_4d0b_0a4d_2026);
    for case in 0..1_024 {
        let mut rows = random_rows(&mut rng);
        // Now and then a key a platform forgot to repair.
        if !rows.is_empty() && rng.chance(15) {
            let index = rng.below(rows.len());
            rows[index].key = rng.pick(&[0, 1, MAX_KEY + 1]);
        }
        let hint = rng.chance(20).then(|| rng.next() % 40);
        let edits: Vec<Edit> = (0..rng.below(4))
            .map(|_| random_edit(&mut rng, &rows))
            .collect();
        let resin = rng.chance(30).then(|| random_resin(&mut rng));
        let context = format!("case {case}: {rows:?} {edits:?}");
        let mut request = json!({
            "rows": rows_json(&rows),
            "edits": edits.iter().map(wire_edit).collect::<Vec<_>>(),
            "resin": resin.as_ref().map(resin_value),
        });
        if let Some(hint) = hint {
            request["next_key"] = json!(hint);
        }
        let response = board_json(&request);
        assert!(response.get("error").is_none(), "{context}: {response}");

        // Save requirements travel without their labels, exactly as the
        // typed save ignores them.
        let result = apply(&rows, hint, &edits);
        assert_eq!(read_back(&response["rows"]), result.rows, "{context}");
        assert_eq!(response["next_key"], json!(result.next_key), "{context}");
        assert_eq!(response["changed"], json!(result.changed), "{context}");
        assert_eq!(response["rekeyed"], pairs(&result.rekeyed), "{context}");
        assert_eq!(response["focus"], json!(result.focus), "{context}");
        assert_eq!(
            response["refused"],
            result.refused.map_or(Value::Null, refusal),
            "{context}"
        );
        let view = board_view(&result.rows, resin.as_ref());
        assert_eq!(
            response["items"],
            Value::Array(view.items.iter().map(item_view).collect()),
            "{context}"
        );
        assert_eq!(
            response["problems"],
            Value::Array(view.problems.iter().map(problem_value).collect()),
            "{context}"
        );
        assert_eq!(
            response["counts"],
            json!({"ordinary": view.counts.ordinary, "blanket": view.counts.blanket}),
            "{context}"
        );
        assert_eq!(
            response["resin"],
            view.resin.as_ref().map_or(Value::Null, resin_chip),
            "{context}"
        );
    }
}

#[test]
fn a_request_without_edits_echoes_its_rows() {
    let rows = [
        exact(named(1, ItemId::RingMight), 2),
        row(2, ItemKind::Wand),
    ];
    let response = board_json(&json!({"rows": rows_json(&rows)}));
    assert_eq!(response["rows"], rows_json(&rows));
    assert_eq!(response["changed"], json!(false));
    assert_eq!(response["rekeyed"], json!([]));
    assert_eq!(response["focus"], Value::Null);
    assert_eq!(response["refused"], Value::Null);
    assert_eq!(response["next_key"], json!(3));
    assert_eq!(response["counts"], json!({"ordinary": 2, "blanket": 0}));
    assert_eq!(response["resin"], Value::Null);
    let ids: Vec<&Value> = response["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|item| &item["id"])
        .collect();
    assert_eq!(ids, [&json!("r1"), &json!("r2")]);
    // The two chips may join each other: the menu reads it off the chip.
    assert_eq!(response["items"][0]["chips"][0]["join"], json!([2]));
    // A platform's counter survives.
    let response = board_json(&json!({"rows": rows_json(&rows), "next_key": 40}));
    assert_eq!(response["next_key"], json!(40));
}

#[test]
fn a_refused_join_answers_the_reason_and_leaves_the_rows() {
    let rows = [
        exact(named(1, ItemId::RingMight), 2),
        named(2, ItemId::RingMight),
        row(3, ItemKind::Wand),
    ];
    let request = json!({
        "rows": rows_json(&rows),
        "edits": [{"type": "join", "source": 3, "target": 1}],
    });
    let response = board_json(&request);
    assert_eq!(response["rows"], rows_json(&rows));
    assert_eq!(response["changed"], json!(false));
    assert_eq!(
        response["refused"],
        json!({
            "reason": "mixed_category_stack",
            "message": "Copies can only be grouped with the same item type.",
        })
    );
    // The chips say so up front, so a platform refuses the drop before it
    // sends one.
    let wand = &response["items"][1]["chips"][0];
    assert_eq!(wand["key"], json!(3));
    assert_eq!(
        wand["refuse"],
        json!([{
            "key": 1,
            "reason": "mixed_category_stack",
            "message": "Copies can only be grouped with the same item type.",
        }])
    );
}

#[test]
fn broken_keys_are_repaired_and_the_edits_follow_them() {
    let rows = [
        row(0, ItemKind::Wand),
        row(4, ItemKind::Ring),
        row(4, ItemKind::Armor),
    ];
    let response = board_json(&json!({
        "rows": rows_json(&rows),
        "edits": [{"type": "set_count", "key": 0, "count": 2}],
    }));
    assert_eq!(response["rekeyed"], json!([[0, 5], [4, 6]]));
    assert_eq!(response["changed"], json!(true));
    // The stack grew on the row that had key 0.
    assert_eq!(keys_of(&response["rows"]), [5, 7, 4, 6]);
    assert_eq!(response["focus"], json!(5));
    assert_eq!(response["items"][0]["extras"], json!([7]));
}

#[test]
fn wide_alternative_labels_are_compacted() {
    let wand = |key: u64, label: u64| {
        let mut object = write_row(&row(key, ItemKind::Wand));
        object["alternative_group"] = json!(label);
        object
    };
    let response = board_json(&json!({
        "rows": [wand(1, 300), wand(2, 300), wand(3, 9_000), wand(4, 9_000)],
    }));
    assert_eq!(response["changed"], json!(true));
    let labels: Vec<&Value> = response["rows"]
        .as_array()
        .unwrap()
        .iter()
        .map(|row| &row["alternative_group"])
        .collect();
    assert_eq!(labels, [&json!(1), &json!(1), &json!(2), &json!(2)]);
    assert_eq!(response["items"][0]["id"], json!("c1"));
    assert_eq!(response["items"][1]["id"], json!("c2"));
}

/// Linux's hand-edited state: a stack labelled 7, which Start refused and
/// nothing on the board could repair. Normalizing moves it into range, past
/// the labels an unreadable row holds.
#[test]
fn normalize_moves_stack_labels_into_range() {
    let stacked = |key: u64| json!({"key": key, "kind": "wand", "identity_group": 7});
    let request = json!({"rows": [stacked(1), stacked(2)], "edits": [{"type": "normalize"}]});
    let response = board_json(&request);
    assert_eq!(response["changed"], json!(true), "{response}");
    assert_eq!(
        response["rows"],
        json!([
            {"key": 1, "kind": "wand", "identity_group": 1},
            {"key": 2, "kind": "wand", "identity_group": 1},
        ])
    );
    assert_eq!(response["problems"], json!([]));
    assert_eq!(response["items"][0]["extras"], json!([2]));
    // Without the edit the rows come back as they were, problems and all.
    let untouched = board_json(&json!({"rows": [stacked(1), stacked(2)]}));
    assert_eq!(untouched["changed"], json!(false));
    assert_eq!(
        untouched["problems"][0]["message"],
        json!("A stack group must be 1 through 4.")
    );

    let unknown = json!({"key": 3, "kind": "wand", "item": "wand_of_wonders", "identity_group": 1});
    let response = board_json(&json!({
        "rows": [stacked(1), stacked(2), unknown],
        "edits": [{"type": "normalize"}],
    }));
    let labels: Vec<&Value> = response["rows"]
        .as_array()
        .unwrap()
        .iter()
        .map(|row| &row["identity_group"])
        .collect();
    assert_eq!(labels, [&json!(2), &json!(2), &json!(1)], "{response}");
}

#[test]
fn unreadable_rows_are_carried_through_and_only_removed() {
    let unknown = json!({"key": 9, "kind": "wand", "item": "wand_of_wonders", "upgrade": 3});
    let rows = json!([
        write_row(&row(1, ItemKind::Wand)),
        unknown,
        write_row(&named(3, ItemId::RingMight)),
    ]);
    let response = board_json(&json!({"rows": rows}));
    assert!(response.get("error").is_none(), "{response}");
    assert_eq!(response["rows"], rows);
    assert_eq!(response["changed"], json!(false));
    assert_eq!(response["next_key"], json!(10));
    assert_eq!(response["counts"], json!({"ordinary": 3, "blanket": 0}));
    let message = "This requirement cannot be read: unknown item 'wand_of_wonders'.";
    let item = &response["items"][1];
    assert_eq!(item["id"], json!("r9"));
    assert_eq!(item["members"], json!([9]));
    assert_eq!(item["problem"], json!(message));
    let chip = &item["chips"][0];
    assert_eq!(chip["name"], json!(UNKNOWN_REQUIREMENT));
    assert_eq!(chip["kind"], Value::Null);
    assert_eq!(chip["problem"], json!(message));
    assert_eq!(chip["join"], json!([]));
    // No relationship reaches it.
    assert_eq!(response["items"][0]["chips"][0]["join"], json!([3]));
    assert_eq!(
        response["problems"],
        json!([{"message": message, "keys": [9], "scope": "row"}])
    );

    // Edits naming it do nothing, except removing it.
    for edit in [
        json!({"type": "join", "source": 1, "target": 9}),
        json!({"type": "join", "source": 9, "target": 3}),
        json!({"type": "set_count", "key": 9, "count": 2}),
        json!({"type": "detach", "key": 9}),
        json!({"type": "save", "key": 9, "requirement": {"kind": "ring"}, "count": 1}),
    ] {
        let response = board_json(&json!({"rows": rows, "edits": [edit]}));
        assert_eq!(response["rows"], rows, "{edit}");
        assert_eq!(response["changed"], json!(false), "{edit}");
    }
    for remove in ["remove", "remove_item"] {
        let response = board_json(&json!({
            "rows": rows,
            "edits": [
                {"type": "set_count", "key": 1, "count": 2},
                {"type": remove, "key": 9},
            ],
        }));
        assert_eq!(keys_of(&response["rows"]), [1, 10, 3], "{remove}");
        assert_eq!(response["changed"], json!(true));
        assert_eq!(response["focus"], Value::Null);
    }
    // Edits around it keep it where it was; new keys pass it.
    let response = board_json(&json!({
        "rows": rows,
        "edits": [
            {"type": "set_count", "key": 1, "count": 3},
            {"type": "save", "key": null, "requirement": {"kind": "armor"}, "count": 1},
        ],
    }));
    assert_eq!(keys_of(&response["rows"]), [1, 10, 11, 9, 3, 12]);
    assert_eq!(response["rows"][3], unknown);
    assert_eq!(response["focus"], json!(12));

    // A sheet cannot open on it, but opens on everything else around it,
    // and saves leave it be.
    let error = editor_json(&json!({"op": "open", "rows": rows, "key": 9}));
    assert_eq!(error, json!({"error": message, "key": 9}));
    let opened = editor_json(&json!({"op": "open", "rows": rows, "key": 3}));
    assert!(opened.get("error").is_none(), "{opened}");
    let draft = changed_draft(
        &opened["draft"],
        &[json!({"type": "set_count", "value": 2})],
    );
    let saved = editor_json(&json!({"op": "save", "draft": draft, "rows": rows}));
    assert_eq!(keys_of(&saved["saved"]["rows"]), [1, 9, 3, 10], "{saved}");
    assert_eq!(saved["saved"]["rows"][1], unknown);
    // A sheet opened on a row that has since become unreadable cannot save
    // onto it; the error names the key, as opening it would.
    let readable = json!([
        write_row(&row(1, ItemKind::Wand)),
        write_row(&row(9, ItemKind::Ring))
    ]);
    let opened = editor_json(&json!({"op": "open", "rows": readable, "key": 9}));
    let saved = editor_json(&json!({"op": "save", "draft": opened["draft"], "rows": rows}));
    assert_eq!(saved, json!({"error": message, "key": 9}));
}

/// An unreadable row is a row of the list: the list-level problem counts it
/// in its section, as `counts` does, rather than judging the readable rows
/// alone.
#[test]
fn unreadable_rows_count_towards_the_list_level_problem() {
    let list_problem = json!({"message": NO_ORDINARY_REQUIREMENT, "keys": [], "scope": "list"});
    let has_list_problem = |response: &Value| {
        response["problems"]
            .as_array()
            .unwrap()
            .contains(&list_problem)
    };
    let unknown = json!({"key": 1, "kind": "wand", "item": "wand_of_wonders"});
    let blanket = write_row(&with(row(2, ItemKind::Wand), |r| r.blanket = true));
    let response = board_json(&json!({"rows": [unknown, blanket]}));
    assert_eq!(response["counts"], json!({"ordinary": 1, "blanket": 1}));
    assert!(!has_list_problem(&response), "{response}");
    // Removing the unreadable ordinary row leaves the blankets alone.
    let response = board_json(&json!({
        "rows": [unknown, blanket],
        "edits": [{"type": "remove", "key": 1}],
    }));
    assert_eq!(response["counts"], json!({"ordinary": 0, "blanket": 1}));
    assert!(has_list_problem(&response), "{response}");
    // An unreadable blanket alone is a blanket without an ordinary row.
    let unknown_blanket =
        json!({"key": 1, "kind": "wand", "item": "wand_of_wonders", "blanket": true});
    let response = board_json(&json!({"rows": [unknown_blanket]}));
    assert_eq!(response["counts"], json!({"ordinary": 0, "blanket": 1}));
    assert!(has_list_problem(&response), "{response}");
    let response = board_json(&json!({"rows": []}));
    assert_eq!(response["problems"], json!([]));
}

/// The group labels an unreadable row holds are its own: a new cluster,
/// stack or combined level never takes one — a board edit or a sheet save
/// would otherwise tie the rows it made to a row no one can see into — and
/// label compaction relabels it with the cluster it was written in.
#[test]
fn an_unreadable_rows_group_labels_are_never_handed_out() {
    let label = |response: &Value, key: u64, field: &str| -> Value {
        let row = response["rows"]
            .as_array()
            .unwrap()
            .iter()
            .find(|row| row["key"] == json!(key))
            .expect("the row is there");
        match field {
            "level_sum" => row["level_sum"]["group"].clone(),
            field => row[field].clone(),
        }
    };

    let rows = json!([
        write_row(&row(1, ItemKind::Wand)),
        write_row(&row(2, ItemKind::Ring)),
        {"key": 3, "kind": "wand", "item": "wand_of_wonders", "alternative_group": 1},
    ]);
    let joined = board_json(&json!({
        "rows": rows,
        "edits": [{"type": "join", "source": 1, "target": 2}],
    }));
    assert_eq!(label(&joined, 1, "alternative_group"), json!(2), "{joined}");
    assert_eq!(label(&joined, 2, "alternative_group"), json!(2));
    assert_eq!(label(&joined, 3, "alternative_group"), json!(1));

    let unknown_stack =
        json!({"key": 3, "kind": "wand", "item": "wand_of_wonders", "identity_group": 1});
    let rows = json!([write_row(&exact(row(1, ItemKind::Wand), 3)), unknown_stack]);
    let stacked = board_json(&json!({
        "rows": rows,
        "edits": [{"type": "set_count", "key": 1, "count": 2}],
    }));
    assert_eq!(label(&stacked, 1, "identity_group"), json!(2), "{stacked}");
    assert_eq!(label(&stacked, 4, "identity_group"), json!(2));
    assert!(stacked["rows"].as_array().unwrap().contains(&unknown_stack));
    // The sheet's save holds them too.
    let opened = editor_json(&json!({"op": "open", "rows": rows, "key": 1}));
    let draft = changed_draft(
        &opened["draft"],
        &[json!({"type": "set_count", "value": 2})],
    );
    let saved = editor_json(&json!({"op": "save", "draft": draft, "rows": rows}));
    assert_eq!(
        label(&saved["saved"], 1, "identity_group"),
        json!(2),
        "{saved}"
    );

    let rows = json!([
        write_row(&named(1, ItemId::RingMight)),
        write_row(&named(2, ItemId::RingMight)),
        {"key": 3, "kind": "ring", "item": "ring_of_nothing", "level_sum": {"group": 1, "at_least": 3}},
    ]);
    let counted = board_json(&json!({
        "rows": rows,
        "edits": [{"type": "toggle_levels", "key": 1}],
    }));
    assert_eq!(label(&counted, 1, "level_sum"), json!(2), "{counted}");
    assert_eq!(label(&counted, 2, "level_sum"), json!(2));
    assert_eq!(label(&counted, 3, "level_sum"), json!(1));

    // Wide labels compact with the unreadable row's among them.
    let wide = |key: u64| json!({"key": key, "kind": "wand", "alternative_group": 300});
    let rows = json!([
        wide(1),
        wide(2),
        {"key": 3, "kind": "wand", "item": "wand_of_wonders", "alternative_group": 300},
        {"key": 4, "kind": "wand", "item": "wand_of_wonders", "alternative_group": 7},
    ]);
    let compacted = board_json(&json!({"rows": rows}));
    assert_eq!(compacted["changed"], json!(true));
    let labels: Vec<&Value> = compacted["rows"]
        .as_array()
        .unwrap()
        .iter()
        .map(|row| &row["alternative_group"])
        .collect();
    assert_eq!(labels, [&json!(1), &json!(1), &json!(1), &json!(2)]);
    assert_eq!(compacted["items"][0]["members"], json!([1, 2]));
}

/// Platform encoders write an unset nullable property as `null`
/// (System.Text.Json by default, kotlinx with explicit nulls): every
/// optional request field takes `null` for its default.
#[test]
fn optional_request_fields_may_be_null() {
    let response = board_json(&json!({"rows": [], "next_key": null, "edits": null, "resin": null}));
    assert_eq!(response["rows"], json!([]), "{response}");
    let defaults = board_json(&json!({"rows": [], "resin": {"amount": 2}}))["resin"].clone();
    assert_eq!(defaults["uncursed"], json!(true));
    for filter in [
        json!(null),
        json!({"uncursed": null, "max_depth": null, "source": null, "include_mage_wand": null}),
    ] {
        let response = board_json(&json!({"rows": [], "resin": {"amount": 2, "filter": filter}}));
        assert_eq!(response["resin"], defaults, "{filter}");
    }
    let plain = editor_json(&json!({"op": "open", "rows": []}));
    let nulls = editor_json(&json!({
        "op": "open", "rows": [], "key": null, "blanket": null, "resin": null,
        "offer_resin": null, "open_resin": null,
    }));
    assert_eq!(nulls, plain);
    let saved = editor_json(&json!({
        "op": "save", "draft": plain["draft"], "rows": [], "next_key": null,
    }));
    assert_eq!(keys_of(&saved["saved"]["rows"]), [1], "{saved}");
}

#[test]
fn a_row_the_editor_mints_never_takes_an_unreadable_rows_key() {
    // Only an exhausted key space falls back to the smallest free key; the
    // unreadable row then moves, and says so.
    let rows = json!([
        write_row(&row(MAX_KEY, ItemKind::Wand)),
        {"key": 1, "kind": "wand", "item": "nope"},
    ]);
    let response = board_json(&json!({
        "rows": rows,
        "edits": [{"type": "save", "key": null, "requirement": {"kind": "ring"}, "count": 1}],
    }));
    assert_eq!(keys_of(&response["rows"]), [MAX_KEY, 2, 1]);
    assert_eq!(response["rekeyed"], json!([[1, 2]]));
    assert_eq!(response["focus"], json!(1));
}

#[test]
fn bad_requests_answer_an_error() {
    let board_cases = [
        ("not json", "invalid request"),
        ("{}", "missing field `rows`"),
        (r#"{"rows":[],"surprise":1}"#, "unknown field `surprise`"),
        (r#"{"rows":[1]}"#, "row 1: a row must be an object"),
        (
            r#"{"rows":[{"kind":"wand"}]}"#,
            "row 1: key must be a whole number",
        ),
        (
            r#"{"rows":[{"key":1,"kind":"wand"},{"key":-2,"kind":"wand"}]}"#,
            "row 2: key must be a whole number",
        ),
        (r#"{"rows":[],"next_key":-1}"#, "invalid request"),
        (r#"{"rows":[],"edits":[{"type":"explode"}]}"#, "explode"),
        (
            r#"{"rows":[],"edits":[{"type":"remove","key":1,"extra":2}]}"#,
            "extra",
        ),
        (
            r#"{"rows":[],"edits":[{"type":"normalize","bogus":1}]}"#,
            "bogus",
        ),
        (
            r#"{"rows":[],"edits":[{"type":"set_count","key":1,"count":300}]}"#,
            "invalid request",
        ),
        (
            r#"{"rows":[],"edits":[{"type":"save","key":null,"requirement":{"item":"nope"},"count":1}]}"#,
            "edit 1: unknown item 'nope'",
        ),
        (
            r#"{"rows":[],"resin":{"amount":0}}"#,
            "resin amount must be a whole number from 1 to 65535",
        ),
        (
            r#"{"rows":[],"resin":{"amount":"lots"}}"#,
            "resin amount must be a whole number from 1 to 65535",
        ),
        (
            r#"{"rows":[],"resin":{"amount":2,"filter":{"source":"attic"}}}"#,
            "attic",
        ),
    ];
    for (request, expected) in board_cases {
        let response: Value = serde_json::from_str(&requirement_board(request)).unwrap();
        let message = response["error"]
            .as_str()
            .unwrap_or_else(|| panic!("{request}"));
        assert!(message.contains(expected), "{request}: {message}");
        assert_eq!(response.as_object().unwrap().len(), 1, "{request}");
    }

    let opened = editor_json(&json!({"op": "open", "rows": []}));
    let draft = opened["draft"].as_str().unwrap();
    let change = |change: Value| json!({"op": "change", "draft": draft, "change": change});
    let editor_cases = [
        (json!({"op": "dance"}), "dance"),
        (json!({"rows": []}), "invalid request"),
        (json!({"op": "open"}), "missing field `rows`"),
        (
            json!({"op": "change", "draft": "garbage", "change": {"type": "set_tier", "value": 3}}),
            "The draft cannot be read",
        ),
        (
            json!({"op": "save", "draft": "{}", "rows": []}),
            "The draft cannot be read",
        ),
        (change(json!({"type": "set_tier"})), "invalid request"),
        (
            change(json!({"type": "set_tier", "value": "3"})),
            "invalid request",
        ),
        (change(json!({"type": "fly", "value": 3})), "fly"),
        (
            change(json!({"type": "set_category", "value": "melee_weapon"})),
            "melee_weapon",
        ),
        (
            change(json!({"type": "toggle_effect", "value": "Sparkly"})),
            "unknown effect 'Sparkly'",
        ),
        (
            change(json!({"type": "set_item", "value": "wand_of_wonders"})),
            "unknown item 'wand_of_wonders'",
        ),
    ];
    for (request, expected) in editor_cases {
        let response = editor_json(&request);
        let message = response["error"]
            .as_str()
            .unwrap_or_else(|| panic!("{request}"));
        assert!(message.contains(expected), "{request}: {message}");
    }
}

#[test]
fn a_leading_byte_order_mark_is_forgiven() {
    let response: Value =
        serde_json::from_str(&requirement_board("\u{feff}{\"rows\":[]}")).unwrap();
    assert_eq!(response["rows"], json!([]));
    assert_eq!(response["next_key"], json!(1));
}

// --- the sheet envelope --------------------------------------------------

#[test]
fn a_sheet_opens_changes_and_saves_through_the_envelope() {
    let rows = [exact(named(1, ItemId::RingMight), 2)];
    let opened = editor_json(&json!({"op": "open", "rows": rows_json(&rows), "key": 1}));
    let form = &opened["form"];
    assert_eq!(form["v"], json!(1));
    assert_eq!(form["mode"], json!("edit"));
    assert_eq!(form["origin"], json!({"type": "row", "key": 1}));
    assert_eq!(form["item"]["value"], json!("ring_might"));
    assert_eq!(form["upgrade"]["mode"], json!("exact"));
    assert_eq!(form["upgrade"]["value"], json!(2));
    assert_eq!(form["stack"]["count"], json!(1));
    assert_eq!(form["preview"]["title"], json!("Ring of Might"));
    assert_eq!(form["preview"]["key"], json!(0));
    assert_eq!(form["can_save"], json!(true));
    // The draft is an opaque string of this version.
    let draft: Value = serde_json::from_str(opened["draft"].as_str().unwrap()).unwrap();
    assert_eq!(draft["v"], json!(DRAFT_VERSION));

    let draft = changed_draft(
        &opened["draft"],
        &[
            json!({"type": "set_count", "value": 2}),
            json!({"type": "set_count_levels", "value": true}),
            json!({"type": "set_total", "value": 5}),
        ],
    );
    let saved = editor_json(&json!({"op": "save", "draft": draft, "rows": rows_json(&rows)}));
    assert_eq!(
        saved,
        json!({"saved": {
            "rows": [
                {"key": 1, "kind": "ring", "item": "ring_might", "level_sum": {"group": 1, "at_least": 5}},
                {"key": 2, "kind": "ring", "item": "ring_might", "level_sum": {"group": 1, "at_least": 5}},
            ],
            "next_key": 3,
            "changed": true,
            "rekeyed": [],
            "focus": 1,
            "resin": null,
        }})
    );
}

#[test]
fn a_refused_save_answers_the_draft_and_its_reasons() {
    let rows = [named(1, ItemId::RatSkull)];
    let opened = editor_json(&json!({"op": "open", "rows": rows_json(&rows)}));
    let draft = changed_draft(
        &opened["draft"],
        &[
            json!({"type": "set_category", "value": "trinket"}),
            json!({"type": "set_item", "value": "rat_skull"}),
        ],
    );
    let refused = editor_json(&json!({"op": "save", "draft": draft, "rows": rows_json(&rows)}));
    assert!(refused.get("saved").is_none(), "{refused}");
    assert_eq!(
        refused["form"]["errors"],
        json!([super::super::DUPLICATE_TRINKET])
    );
    assert_eq!(refused["form"]["can_save"], json!(false));
    assert!(refused["draft"].is_string());
}

#[test]
fn resin_flows_through_the_envelope() {
    let wand = row(1, ItemKind::Wand);
    let rows = rows_json(&[wand]);
    // A wand chip turned into Arcane Resin sets the query's resin and
    // leaves the board.
    let opened = editor_json(&json!({"op": "open", "rows": rows, "key": 1, "offer_resin": true}));
    let draft = changed_draft(
        &opened["draft"],
        &[json!({"type": "set_item", "value": "arcane_resin"})],
    );
    let saved = editor_json(&json!({"op": "save", "draft": draft, "rows": rows}));
    assert_eq!(saved["saved"]["rows"], json!([]));
    assert_eq!(
        saved["saved"]["resin"],
        json!({"set": {
            "amount": 2,
            "filter": {"uncursed": true, "max_depth": null, "source": null, "include_mage_wand": false},
        }})
    );

    // The resin chip saved as a wand clears the query's resin.
    let resin = json!({"amount": "auto", "filter": {"uncursed": false, "max_depth": 9}});
    let opened = editor_json(&json!({
        "op": "open", "rows": rows, "resin": resin, "open_resin": true,
    }));
    assert_eq!(opened["form"]["origin"], json!({"type": "resin"}));
    assert_eq!(opened["form"]["item"]["value"], json!("arcane_resin"));
    assert_eq!(opened["form"]["resin"]["auto"], json!(true));
    assert_eq!(opened["form"]["uncursed"]["value"], json!(false));
    assert_eq!(opened["form"]["floor_limit"]["value"], json!(9));
    let draft = changed_draft(
        &opened["draft"],
        &[json!({"type": "set_item", "value": "wand_frost"})],
    );
    let saved = editor_json(&json!({"op": "save", "draft": draft, "rows": rows}));
    assert_eq!(saved["saved"]["resin"], json!({"clear": true}));
    assert_eq!(
        saved["saved"]["rows"],
        json!([
            {"key": 1, "kind": "wand"},
            {"key": 2, "kind": "wand", "item": "wand_frost"},
        ])
    );

    // An amount that is not one keeps the sheet open with the reason.
    let draft = changed_draft(
        &opened["draft"],
        &[
            json!({"type": "set_resin_auto", "value": false}),
            json!({"type": "set_resin_amount", "value": null}),
        ],
    );
    let refused = editor_json(&json!({"op": "save", "draft": draft, "rows": rows}));
    assert_eq!(refused["form"]["errors"], json!([RESIN_AMOUNT_RANGE]));
    assert_eq!(refused["form"]["resin"]["amount"], Value::Null);
}

#[test]
fn a_draft_from_another_version_is_refused() {
    let opened = editor_json(&json!({"op": "open", "rows": []}));
    let mut draft: Value = serde_json::from_str(opened["draft"].as_str().unwrap()).unwrap();
    draft["v"] = json!(2);
    let response = editor_json(&json!({
        "op": "change",
        "draft": draft.to_string(),
        "change": {"type": "set_uncursed", "value": true},
    }));
    assert_eq!(
        response,
        json!({"error": "The draft cannot be read; reopen the sheet: it is version 2, this editor reads version 1"})
    );
}

#[test]
fn a_saved_sheet_follows_the_keys_the_save_repaired() {
    // The sheet opened on the platform's own keys; the save repairs them
    // and the draft's key with them.
    let rows = rows_json(&[row(0, ItemKind::Wand), row(4, ItemKind::Ring)]);
    let opened = editor_json(&json!({"op": "open", "rows": rows, "key": 0}));
    assert_eq!(opened["form"]["origin"], json!({"type": "row", "key": 0}));
    let draft = changed_draft(
        &opened["draft"],
        &[json!({"type": "set_uncursed", "value": true})],
    );
    let saved = editor_json(&json!({"op": "save", "draft": draft, "rows": rows, "next_key": 20}));
    assert_eq!(
        saved["saved"]["rows"],
        json!([
            {"key": 20, "kind": "wand", "uncursed": true},
            {"key": 4, "kind": "ring"},
        ])
    );
    assert_eq!(saved["saved"]["rekeyed"], json!([[0, 20]]));
    assert_eq!(saved["saved"]["focus"], json!(20));
    assert_eq!(saved["saved"]["next_key"], json!(21));
}

// --- properties ----------------------------------------------------------

#[test]
fn drafts_read_back_exactly_as_written() {
    let mut rng = Rng::new(0x0d4a_f70d_4af7_0001);
    for case in 0..1_024 {
        let rows = random_rows(&mut rng);
        let resin = rng.chance(40).then(|| random_resin(&mut rng));
        let key = (!rows.is_empty() && rng.chance(70)).then(|| rows[rng.below(rows.len())].key);
        let mut draft = open(
            &rows,
            key,
            rng.chance(20),
            resin.as_ref(),
            rng.chance(60),
            rng.chance(10),
        );
        for _ in 0..rng.below(6) {
            let step = random_change(&mut rng, &draft);
            draft = super::super::change(&draft, &step);
        }
        let written = write_draft(&draft);
        assert_eq!(
            read_draft(&written).unwrap(),
            draft,
            "case {case}: {written}"
        );
    }
}

/// Unreadable rows a web store can hold.
const UNREADABLE: [&str; 4] = [
    r#"{"kind":"wand","item":"wand_of_wonders"}"#,
    r#"{"kind":"weapon","upgarde":2}"#,
    r#"{"item":"sword","effect":"Thorns"}"#,
    r#"{"upgrade":2}"#,
];

/// A list a platform might send: rows valid or not, some unreadable, some
/// keys broken, some alternative labels wide.
fn wild_rows(rng: &mut Rng) -> (Vec<Value>, Vec<Value>) {
    let rows = if rng.chance(50) {
        mixed_rows(rng)
    } else {
        random_rows(rng)
    };
    let mut values: Vec<Value> = rows.iter().map(write_row).collect();
    for _ in 0..rng.below(3) {
        if rng.chance(50) {
            let mut value: Value = serde_json::from_str(rng.pick(&UNREADABLE)).unwrap();
            value["key"] = json!(rng.next() % 14);
            let at = rng.below(values.len() + 1);
            values.insert(at, value);
        }
    }
    for value in &mut values {
        if rng.chance(8) {
            value["key"] = json!(rng.pick(&[0, 1, 2, MAX_KEY, MAX_KEY + 1, u64::MAX]));
        }
        if value.get("alternative_group").is_some() && rng.chance(20) {
            value["alternative_group"] = json!(250 + rng.below(20));
        }
    }
    // Arbitrary requirements the document cannot spell (an effect of the
    // other family, say) arrive unreadable too.
    let unreadable = values
        .iter()
        .filter(|value| read_row(value.as_object().unwrap()).is_err())
        .cloned()
        .collect();
    (values, unreadable)
}

/// Every row an envelope writes reads back, or is an unreadable row it was
/// sent, verbatim but for its key and — when wide labels were compacted —
/// its alternative label; and keys stay unique and in range.
fn assert_rows_sound(rows: &Value, unreadable: &[Value], context: &str) {
    let rows = rows.as_array().expect("rows");
    let keys: BTreeSet<u64> = rows.iter().filter_map(|row| row["key"].as_u64()).collect();
    assert_eq!(keys.len(), rows.len(), "duplicate keys: {context}");
    assert!(
        keys.iter().all(|key| (1..=MAX_KEY).contains(key)),
        "{context}"
    );
    for row in rows {
        let object = row.as_object().expect("an object");
        if read_row(object).is_ok() {
            continue;
        }
        let unlabelled = |object: &Map<String, Value>| {
            let mut object = object.clone();
            object.remove("key");
            object.remove("alternative_group");
            object
        };
        let sent = unlabelled(object);
        assert!(
            unreadable
                .iter()
                .any(|value| unlabelled(value.as_object().unwrap()) == sent),
            "{row} was never sent: {context}"
        );
    }
}

#[test]
fn no_request_panics_and_every_written_row_reads_back() {
    let mut rng = Rng::new(0x5afe_f00d_2026_0928);
    for case in 0..1_024 {
        let (values, unreadable) = wild_rows(&mut rng);
        let typed_keys: Vec<u64> = values
            .iter()
            .filter_map(|value| value["key"].as_u64())
            .collect();
        let pseudo: Vec<Row> = typed_keys
            .iter()
            .map(|&key| row(key, ItemKind::Wand))
            .collect();
        let edits: Vec<Value> = (0..rng.below(5))
            .map(|_| match rng.below(6) {
                0 => json!({"type": "remove", "key": rng.pick(&typed_keys.clone().into_iter().chain([0]).collect::<Vec<_>>())}),
                _ => wire_edit(&random_edit(&mut rng, &pseudo)),
            })
            .collect();
        let resin = rng.chance(30).then(|| resin_value(&random_resin(&mut rng)));
        let request = json!({
            "rows": values,
            "edits": edits,
            "resin": resin,
            "next_key": rng.chance(20).then(|| rng.next() % 40),
        });
        let context = format!("case {case}: {request}");
        let response = board_json(&request);
        assert!(response.get("error").is_none(), "{context}: {response}");
        assert_rows_sound(&response["rows"], &unreadable, &context);
        // The entries cover every row once.
        let mut covered: Vec<u64> = Vec::new();
        for item in response["items"].as_array().unwrap() {
            for key in item["members"]
                .as_array()
                .unwrap()
                .iter()
                .chain(item["extras"].as_array().unwrap())
            {
                covered.push(key.as_u64().unwrap());
            }
        }
        covered.sort_unstable();
        let mut keys = keys_of(&response["rows"]);
        keys.sort_unstable();
        // A hand-written list can leave a row no fold claims; never twice.
        let unique: BTreeSet<u64> = covered.iter().copied().collect();
        assert_eq!(unique.len(), covered.len(), "{context}");
        assert!(unique.iter().all(|key| keys.contains(key)), "{context}");
        assert!(
            keys.iter()
                .all(|&key| response["next_key"].as_u64().unwrap() > key),
            "{context}"
        );

        // A sheet on the same list, moved a few times and saved.
        let rows = response["rows"].clone();
        let keys = keys_of(&rows);
        let key = (!keys.is_empty() && rng.chance(70)).then(|| keys[rng.below(keys.len())]);
        let opened = editor_json(&json!({
            "op": "open",
            "rows": rows,
            "key": key,
            "blanket": rng.chance(20),
            "resin": rng.chance(30).then(|| resin_value(&random_resin(&mut rng))),
            "offer_resin": rng.chance(60),
            "open_resin": rng.chance(10),
        }));
        if let Some(error) = opened.get("error") {
            // Only an unreadable row refuses a sheet.
            assert_eq!(opened["key"], json!(key), "{context}: {error}");
            continue;
        }
        let mut draft = opened["draft"].clone();
        for _ in 0..rng.below(6) {
            let typed = read_draft(draft.as_str().unwrap()).unwrap();
            let step = wire_change(&random_change(&mut rng, &typed));
            let response = editor_json(&json!({"op": "change", "draft": draft, "change": step}));
            assert!(
                response.get("error").is_none(),
                "{context}: {step}: {response}"
            );
            draft = response["draft"].clone();
        }
        let saved = editor_json(&json!({"op": "save", "draft": draft, "rows": rows}));
        assert!(saved.get("error").is_none(), "{context}: {saved}");
        if let Some(saved) = saved.get("saved") {
            assert_rows_sound(&saved["rows"], &unreadable, &context);
        } else {
            assert_eq!(saved["form"]["can_save"], json!(false), "{context}");
        }
    }
}
