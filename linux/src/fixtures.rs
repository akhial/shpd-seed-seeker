// SPDX-License-Identifier: GPL-3.0-or-later

//! The shared editor's golden request/response pairs
//! (`crates/seedfinder-core/tests/fixtures/editor`), read through the app's
//! own query codec, for the board's and the sheet's bridge tests. The files
//! pin what the JSON envelopes answer; Linux calls the typed API they
//! project, so the tests decode each request into typed values, ask the
//! typed editor, and compare its answer with the golden one.

use serde_json::{Value, json};
use shpd_seedfinder_core::editor::{ResinState, Row, Tag, TagStyle};
use shpd_seedfinder_core::json_query;
use shpd_seedfinder_core::query::Requirement;

use crate::state::AppState;

/// One golden request/response pair.
pub struct Fixture {
    pub name: &'static str,
    pub request: Value,
    pub response: Value,
}

macro_rules! golden {
    ($($name:literal),* $(,)?) => {
        /// Every fixture the bridge tests replay, as its file holds it.
        const GOLDEN: &[(&str, &str)] = &[$((
            $name,
            include_str!(concat!(
                "../../crates/seedfinder-core/tests/fixtures/editor/",
                $name,
                ".json"
            )),
        )),*];
    };
}

// The error fixtures and those with unreadable rows are left out: the typed
// API has no text to misread, and a Linux list holds only readable rows.
// The key-repair fixture is replayed with its wide label narrowed
// ([`narrowed`]).
golden![
    "board-tour",
    "board-empty",
    "board-key-repair",
    "board-join-refused",
    "board-join-leaves-copies",
    "board-join-one-copy",
    "board-join-member-moves-one",
    "board-join-across-categories",
    "board-join-onto-stacked-chip",
    "board-join-onto-stacked-cluster",
    "board-join-combined-level",
    "board-join-onto-combined-level",
    "board-join-hand-written-stack",
    "board-join-round-trip",
    "board-detach",
    "board-detach-one-copy",
    "board-remove-member",
    "board-remove-one-member",
    "board-remove-one-stack",
    "board-remove-item",
    "board-stack-concrete",
    "board-stack-wildcard",
    "board-stack-cluster",
    "board-stack-member",
    "board-member-count-one",
    "board-cluster-alike-stacks",
    "board-remaining-badges",
    "board-stack-total",
    "board-stack-total-grow-copy-floor",
    "board-copy-depth",
    "board-toggle-levels",
    "board-toggle-levels-copy-floor",
    "board-toggle-levels-off-copy-floor",
    "board-blanket-total-refused",
    "board-save-new",
    "board-save-unchanged",
    "board-problems",
    "board-problems-blankets-only",
    "board-normalize-labels",
    "board-resin-credit",
    "editor-open-new",
    "editor-open-row",
    "editor-open-member",
    "editor-open-blanket",
    "editor-change-item",
    "editor-change-effect",
    "editor-change-count",
    "editor-save",
    "editor-save-member",
    "editor-save-refused",
    "editor-save-untouched",
    "editor-save-untouched-repairs",
    "editor-resin-open",
    "editor-resin-open-new",
    "editor-resin-pick",
    "editor-resin-mage-wand",
    "editor-resin-amount-invalid",
    "editor-resin-save-set",
    "editor-resin-save-clear",
    "editor-resin-save-untouched",
];

/// The fixtures of one envelope, in the order above.
pub fn fixtures(envelope: &str) -> Vec<Fixture> {
    GOLDEN
        .iter()
        .map(|&(name, text)| {
            let document: Value = serde_json::from_str(text).expect("fixtures are JSON");
            (name, narrowed(name, document))
        })
        .filter(|(_, document)| document["envelope"] == envelope)
        .map(|(name, document)| Fixture {
            name,
            request: document["request"].clone(),
            response: document["response"].clone(),
        })
        .collect()
}

/// A fixture as a Linux list can hold it. A row keeps its alternative label
/// in a byte, and the key-repair fixture labels its cluster 300 — only
/// Android numbers clusters that high — for the editor to compact to 1. It
/// is replayed labelled 1 already, so it still pins the key repair, and its
/// golden answer holds as it is.
fn narrowed(name: &str, mut document: Value) -> Value {
    if name == "board-key-repair" {
        for row in document["request"]["rows"].as_array_mut().unwrap() {
            if row["alternative_group"] == 300 {
                row["alternative_group"] = json!(1);
            }
        }
    }
    document
}

/// One requirement through the app's own codec: the canonical query
/// document it saves, shares and loads.
pub fn decode_requirement(entry: &Value) -> Requirement {
    let document = json!({ "requirements": [entry] }).to_string();
    json_query::decode_unvalidated(&document)
        .expect("fixture rows are readable")
        .requirements[0]
}

/// A row's requirement with its alternative label. The document writes
/// clusters as `any_of` entries; a row carries its label instead.
pub fn decode_labelled(entry: &Value) -> Requirement {
    let mut fields = entry.as_object().expect("a row is an object").clone();
    fields.remove("key");
    let group = fields
        .remove("alternative_group")
        .map(|group| u8::try_from(group.as_u64().unwrap()).unwrap());
    Requirement {
        alternative_group: group,
        ..decode_requirement(&Value::Object(fields))
    }
}

/// A fixture row as the board holds it.
pub fn decode_row(entry: &Value) -> Row {
    Row {
        key: entry["key"].as_u64().expect("a row has a key"),
        requirement: decode_labelled(entry),
    }
}

pub fn decode_rows(rows: &Value) -> Vec<Row> {
    rows.as_array()
        .expect("rows are a list")
        .iter()
        .map(decode_row)
        .collect()
}

/// A row as the envelope writes it, from the entry the app's codec writes
/// for its requirement.
pub fn encode_row(row: &Row) -> Value {
    let mut state = AppState::default();
    state.requirements = vec![Row {
        key: row.key,
        requirement: Requirement {
            alternative_group: None,
            ..row.requirement
        },
    }];
    let mut entry = json_query::encode(&state.unvalidated_query())["requirements"][0].clone();
    entry["key"] = json!(row.key);
    if let Some(group) = row.requirement.alternative_group {
        entry["alternative_group"] = json!(group);
    }
    entry
}

/// A RESIN as the query's resin condition, read through the query codec.
pub fn decode_resin(resin: &Value) -> Option<ResinState> {
    if resin.is_null() {
        return None;
    }
    let document = json!({
        "requirements": [],
        "arcane_resin": resin["amount"],
        "arcane_resin_filter": resin["filter"],
    });
    let state = AppState::from_query(
        &json_query::decode_unvalidated(&document.to_string()).expect("a resin condition"),
    );
    state.resin()
}

/// The query's resin condition as a RESIN.
pub fn encode_resin(resin: &ResinState) -> Value {
    let mut state = AppState::default();
    state.set_resin(Some(*resin));
    let document = json_query::encode(&state.unvalidated_query());
    let filter = &document["arcane_resin_filter"];
    json!({
        "amount": document["arcane_resin"],
        "filter": {
            "include_mage_wand": filter["include_mage_wand"].as_bool().unwrap_or(false),
            "max_depth": filter["max_depth"],
            "source": filter["source"],
            "uncursed": filter["uncursed"].as_bool().unwrap_or(true),
        },
    })
}

pub fn keys(value: &Value) -> Vec<u64> {
    value
        .as_array()
        .unwrap()
        .iter()
        .map(|key| key.as_u64().unwrap())
        .collect()
}

/// Tags as the envelope writes them: text, style and hover text.
pub fn tags(tags: &[Tag]) -> Value {
    tags.iter()
        .map(|tag| {
            let style = match tag.style {
                TagStyle::Plain => "plain",
                TagStyle::Upgrade => "upgrade",
                TagStyle::Credit => "credit",
            };
            json!({ "text": tag.text, "style": style, "tooltip": tag.tooltip })
        })
        .collect()
}
