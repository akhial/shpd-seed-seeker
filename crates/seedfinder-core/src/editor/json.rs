//! The requirement editor as two JSON calls, for the platforms that cannot
//! call Rust directly: the web through WebAssembly, the Apple and Windows
//! apps through the C ABI, Android through JNI.
//!
//! Both envelopes are stateless. A board request carries the whole row list
//! (and any edits to run on it) and gets back the rows, everything the board
//! draws and the list's problems; a sheet request carries the rows or an
//! opaque draft and gets back the next draft with its form, or the saved
//! rows. Nothing here decides anything: every answer comes from the typed
//! editor ([`super::apply`], [`super::board_view`], [`super::open`],
//! [`super::change`], [`super::form`], [`super::save`]), and this module only
//! reads requests into its types and writes its results out, so a platform on
//! the envelopes and Linux on the typed surface cannot drift apart.
//!
//! Two things exist only on the wire and are handled here:
//!
//! - *Unreadable rows.* The web's store keeps whatever it was given, so a row
//!   can name an item a later catalog renamed. Such a row is carried through
//!   verbatim, shown as an `Unknown requirement` chip with the reason, left
//!   out of every relationship and removable — rather than failing the whole
//!   board and blanking both sections. Its key and group labels stay its
//!   own: the editor mints neither a key nor a label it holds, and it counts
//!   towards the list-level problem.
//! - *Wide alternative labels.* Android numbers groups with unbounded
//!   integers; labels past 255 are compacted
//!   ([`super::compact_alternative_labels`]).
//!
//! A request the envelopes cannot read never panics or throws: the answer is
//! `{"error": "<message>"}`, with the `"key"` of the row at fault when there
//! is one. `docs/requirement-editor.md` specifies the format, and the golden
//! fixtures in `tests/fixtures/editor/` pin it.

use std::collections::{BTreeSet, HashMap};

use serde::Deserialize;
use serde_json::{Map, Value};

use crate::catalog::{
    Effect, ItemId, ItemKind, WeaponCategory, item, item_by_stable_id, kind_name,
};
use crate::json_query::{
    FileItemKind, FileItemSource, FileRequirement, convert_requirement, requirement_object,
};
use crate::model::{ItemSource, source_name};
use crate::query::{ARCANE_RESIN_MAX, ARCANE_RESIN_MIN, ArcaneResinFilter, Requirement};

use super::board::{HeldLabels, apply_holding};
use super::chips::board_view_beside;
use super::draft::save_holding;
use super::labels::{count_text, total_text};
use super::problems::Unread;
use super::{
    Badge, Badges, BoardView, Change, ChipView, Choice, Draft, Edit, EffectBadge, EffectChoice,
    EffectControl, EffectGroup, EffectMode, FloorToggle, Form, FormMode, ItemChoice, ItemView,
    KindName, ModeRange, Opt, Origin, Problem, ProblemScope, RangeToggle, Refusal, Relation,
    RelationGlyph, ResinAmount, ResinChip, ResinControl, ResinDraft, ResinOutcome, ResinState, Row,
    STACK_MAX, SaveResult, StackControl, StackView, Tag, TagStyle, TierMode, Toggle, UpgradeMode,
    change, compact_alternative_labels, form, next_key, open, redirects, repair_keys,
};

/// The chip name of a row the codec could not read.
pub const UNKNOWN_REQUIREMENT: &str = "Unknown requirement";

/// Answers a board request: runs its edits on its rows and returns the rows
/// with everything the board draws.
///
/// Request: `{"rows": [ROW], "next_key"?: KEY, "edits"?: [EDIT], "resin"?:
/// RESIN|null}`. Response: `{"rows", "next_key", "changed", "rekeyed",
/// "focus", "refused", "items", "counts", "problems", "resin"}` — see
/// `docs/requirement-editor.md`. Never panics on bad input; a request it
/// cannot read answers `{"error": message}`.
#[must_use]
pub fn requirement_board(request: &str) -> String {
    respond(board(request))
}

/// Answers a requirement-sheet request: `open` a sheet on the rows,
/// `change` one control of a draft, or `save` a draft onto the rows.
///
/// Open and change answer `{"draft": DRAFT, "form": FORM}`, where DRAFT is a
/// string the platform stores and sends back untouched; a save answers
/// `{"saved": {...}}`, or the draft and form again when the draft cannot be
/// saved. Never panics on bad input; a request it cannot read — including a
/// draft of another version — answers `{"error": message}`, plus `"key"`
/// when the row the sheet was asked to open, or to save onto, cannot be
/// read.
#[must_use]
pub fn requirement_editor(request: &str) -> String {
    respond(editor(request))
}

fn respond(result: Result<Value, Failure>) -> String {
    result.unwrap_or_else(Failure::into_value).to_string()
}

// --- failures --------------------------------------------------------------

/// A request an envelope cannot answer.
#[derive(Debug)]
struct Failure {
    message: String,
    /// The row at fault, when the failure is one row's.
    key: Option<u64>,
}

impl Failure {
    fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            key: None,
        }
    }

    fn at(key: u64, message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            key: Some(key),
        }
    }

    fn into_value(self) -> Value {
        let mut fields = vec![("error", self.message.into())];
        if let Some(key) = self.key {
            fields.push(("key", key.into()));
        }
        object(fields)
    }
}

/// Parses a request body, forgiving the byte-order mark some platform
/// writers prepend.
fn parse<T: for<'de> Deserialize<'de>>(text: &str) -> Result<T, Failure> {
    serde_json::from_str(text.strip_prefix('\u{feff}').unwrap_or(text))
        .map_err(|error| Failure::new(format!("invalid request: {error}")))
}

// --- rows ------------------------------------------------------------------

/// One row of a request: read into the editor's types, or kept verbatim.
#[derive(Clone, Debug)]
enum Entry {
    Row(Row),
    Raw(Raw),
}

/// A row the codec could not read. Only the web's lenient store produces
/// them; the envelopes carry them through untouched so one bad row never
/// blanks a board, and only a removal applies to them.
#[derive(Clone, Debug)]
struct Raw {
    key: u64,
    /// The row as it came, written back with its (possibly repaired) key.
    object: Map<String, Value>,
    /// Why it could not be read, as the chip and the problem list say it.
    message: String,
}

impl Raw {
    /// Which board section the row claims, read loosely: the chip has to sit
    /// somewhere.
    fn blanket(&self) -> bool {
        self.object.get("blanket") == Some(&Value::Bool(true))
    }

    /// The group labels the row holds — alternative, identity, combined
    /// level — read loosely: `None` for one that is missing or no label at
    /// all. The row takes part in no relationship, but the labels it holds
    /// are not free for the editor to hand out.
    fn labels(&self) -> [Option<u8>; 3] {
        let label = |value: Option<&Value>| {
            value
                .and_then(Value::as_u64)
                .and_then(|label| u8::try_from(label).ok())
        };
        [
            label(self.object.get("alternative_group")),
            label(self.object.get("identity_group")),
            label(
                self.object
                    .get("level_sum")
                    .and_then(|sum| sum.get("group")),
            ),
        ]
    }
}

/// The group labels held by the unreadable rows among `entries` that
/// `live` says are still in the list.
fn held_labels(entries: &[Entry], live: impl Fn(u64) -> bool) -> HeldLabels {
    let mut held = HeldLabels::default();
    for entry in entries {
        if let Entry::Raw(raw) = entry
            && live(raw.key)
        {
            let [alternative, identity, level_sum] = raw.labels();
            held.alternative.extend(alternative);
            held.identity.extend(identity);
            held.level_sum.extend(level_sum);
        }
    }
    held
}

/// The sections the unreadable rows among `entries` sit in.
fn unread(entries: &[Entry]) -> Unread {
    let mut unread = Unread::default();
    for entry in entries {
        if let Entry::Raw(raw) = entry {
            if raw.blanket() {
                unread.blanket = true;
            } else {
                unread.ordinary = true;
            }
        }
    }
    unread
}

impl Entry {
    const fn key(&self) -> u64 {
        match self {
            Self::Row(row) => row.key,
            Self::Raw(raw) => raw.key,
        }
    }

    fn set_key(&mut self, key: u64) {
        match self {
            Self::Row(row) => row.key = key,
            Self::Raw(raw) => raw.key = key,
        }
    }
}

/// Reads a request's rows. A row that is not an object, or whose key is not
/// a whole number, fails the request — nothing could name it; a row whose
/// requirement cannot be read is kept as a [`Raw`] row. Alternative labels
/// are read as the platform sent them and compacted into `u8` labels when
/// any passes 255 — an unreadable row's among them, so it keeps to the
/// cluster it was written in; the flag says whether that happened.
fn read_rows(values: Vec<Value>) -> Result<(Vec<Entry>, bool), Failure> {
    let mut entries = Vec::with_capacity(values.len());
    let mut labels: Vec<Option<u64>> = Vec::with_capacity(values.len());
    for (index, value) in values.into_iter().enumerate() {
        let position = index + 1;
        let Value::Object(object) = value else {
            return Err(Failure::new(format!(
                "row {position}: a row must be an object"
            )));
        };
        let Some(key) = object.get("key").and_then(Value::as_u64) else {
            return Err(Failure::new(format!(
                "row {position}: key must be a whole number"
            )));
        };
        match read_row(&object) {
            Ok((requirement, label)) => {
                labels.push(label);
                entries.push(Entry::Row(Row { key, requirement }));
            }
            Err(error) => {
                labels.push(object.get("alternative_group").and_then(Value::as_u64));
                entries.push(Entry::Raw(Raw {
                    key,
                    object,
                    message: format!("This requirement cannot be read: {error}."),
                }));
            }
        }
    }
    let (compacted, relabelled) = compact_alternative_labels(&labels);
    for ((entry, label), sent) in entries.iter_mut().zip(compacted).zip(labels) {
        match entry {
            Entry::Row(row) => row.requirement.alternative_group = label,
            // An unreadable row is rewritten only when the labels were.
            Entry::Raw(raw) if relabelled && sent.is_some() => {
                let field = "alternative_group".to_owned();
                match label {
                    Some(label) => raw.object.insert(field, label.into()),
                    None => raw.object.remove(&field),
                };
            }
            Entry::Raw(_) => {}
        }
    }
    Ok((entries, relabelled))
}

/// The requirement of a ROW object and its alternative label as sent.
fn read_row(object: &Map<String, Value>) -> Result<(Requirement, Option<u64>), String> {
    let label = match object.get("alternative_group") {
        None | Some(Value::Null) => None,
        Some(value) => Some(
            value
                .as_u64()
                .ok_or_else(|| "alternative_group must be a whole number".to_owned())?,
        ),
    };
    Ok((read_requirement(object)?, label))
}

/// The requirement a `ROW` (or `ROW_WITHOUT_KEY`) object holds: the canonical
/// requirement object of the query document, read by the document's own
/// reader once the row's `key` and `alternative_group` are set aside. The
/// requirement is not validated — the problem list says what is wrong.
fn read_requirement(object: &Map<String, Value>) -> Result<Requirement, String> {
    let mut fields = object.clone();
    fields.remove("key");
    fields.remove("alternative_group");
    let file: FileRequirement =
        serde_json::from_value(Value::Object(fields)).map_err(|error| error.to_string())?;
    convert_requirement(file, None)
}

/// A row as the envelopes write it: the canonical requirement object exactly
/// as the query document writes it, its `key`, and its `alternative_group`
/// when it has one.
pub(crate) fn write_row(row: &Row) -> Value {
    let mut object = requirement_fields(&row.requirement);
    object.insert("key".to_owned(), row.key.into());
    Value::Object(object)
}

/// A `ROW_WITHOUT_KEY`: the requirement object and its alternative label.
fn requirement_fields(requirement: &Requirement) -> Map<String, Value> {
    let mut object = requirement_object(requirement);
    if let Some(group) = requirement.alternative_group {
        object.insert("alternative_group".to_owned(), group.into());
    }
    object
}

fn write_entry(entry: &Entry) -> Value {
    match entry {
        Entry::Row(row) => write_row(row),
        Entry::Raw(raw) => {
            let mut object = raw.object.clone();
            object.insert("key".to_owned(), raw.key.into());
            Value::Object(object)
        }
    }
}

/// The readable rows, in order.
fn typed(entries: &[Entry]) -> Vec<Row> {
    entries
        .iter()
        .filter_map(|entry| match entry {
            Entry::Row(row) => Some(*row),
            Entry::Raw(_) => None,
        })
        .collect()
}

/// Every row with its key alone, so the editor's key rules run over the
/// unreadable rows too.
fn stand_ins(entries: &[Entry]) -> Vec<Row> {
    entries
        .iter()
        .map(|entry| Row {
            key: entry.key(),
            requirement: Requirement::any(ItemKind::Weapon),
        })
        .collect()
}

/// Repairs every row's key — unreadable rows included, since they share the
/// key space — exactly as [`super::apply`] repairs a list, and returns the
/// `(old, new)` pairs.
fn repair(entries: &mut [Entry], hint: Option<u64>) -> Vec<(u64, u64)> {
    let (repaired, rekeyed) = repair_keys(&stand_ins(entries), hint);
    for (entry, row) in entries.iter_mut().zip(repaired) {
        entry.set_key(row.key);
    }
    rekeyed
}

/// The key hint for the readable rows alone: past every unreadable row's
/// key too, so a key the editor mints never lands on one.
fn readable_hint(entries: &[Entry], hint: Option<u64>) -> Option<u64> {
    let unreadable = entries
        .iter()
        .filter(|entry| matches!(entry, Entry::Raw(_)))
        .map(Entry::key)
        .max();
    match (unreadable, hint) {
        (None, hint) => hint,
        (Some(largest), hint) => Some(hint.unwrap_or(0).max(largest.saturating_add(1))),
    }
}

/// Puts the unreadable rows of `original` back among the readable rows an
/// edit returned. Each goes right before the readable row that followed it
/// and survived — so the copies a stack grows, which land right after their
/// anchor, stay together — else right after the one that preceded it (a
/// new row is appended after it), else first. Only the rows in `live` come
/// back; the others were removed.
fn merge(original: &[Entry], rows: Vec<Row>, live: &BTreeSet<u64>) -> Vec<Entry> {
    let surviving: BTreeSet<u64> = rows.iter().map(|row| row.key).collect();
    let survivor = |entry: &Entry| match entry {
        Entry::Row(row) if surviving.contains(&row.key) => Some(row.key),
        Entry::Row(_) | Entry::Raw(_) => None,
    };
    let mut before: HashMap<u64, Vec<Raw>> = HashMap::new();
    let mut after: HashMap<u64, Vec<Raw>> = HashMap::new();
    let mut first: Vec<Raw> = Vec::new();
    for (index, entry) in original.iter().enumerate() {
        let Entry::Raw(raw) = entry else {
            continue;
        };
        if !live.contains(&raw.key) {
            continue;
        }
        if let Some(next) = original[index + 1..].iter().find_map(survivor) {
            before.entry(next).or_default().push(raw.clone());
        } else if let Some(previous) = original[..index].iter().rev().find_map(survivor) {
            after.entry(previous).or_default().push(raw.clone());
        } else {
            first.push(raw.clone());
        }
    }
    let mut merged: Vec<Entry> = first.into_iter().map(Entry::Raw).collect();
    for row in rows {
        let key = row.key;
        merged.extend(
            before
                .remove(&key)
                .unwrap_or_default()
                .into_iter()
                .map(Entry::Raw),
        );
        merged.push(Entry::Row(row));
        merged.extend(
            after
                .remove(&key)
                .unwrap_or_default()
                .into_iter()
                .map(Entry::Raw),
        );
    }
    merged
}

/// Moves an unreadable row off a key the editor handed to a new row. The
/// editor mints past every unreadable key, so this only happens once the
/// key space is exhausted and it falls back to the smallest free key.
/// Returns the moves as `(old, new)`: a platform reference to the old key
/// meant the unreadable row.
fn separate(entries: &mut [Entry]) -> Vec<(u64, u64)> {
    let readable: BTreeSet<u64> = entries
        .iter()
        .filter(|entry| matches!(entry, Entry::Row(_)))
        .map(Entry::key)
        .collect();
    let mut moved = Vec::new();
    for index in 0..entries.len() {
        let key = entries[index].key();
        if matches!(entries[index], Entry::Raw(_)) && readable.contains(&key) {
            let used: BTreeSet<u64> = entries.iter().map(Entry::key).collect();
            let Some(free) = (1..=super::MAX_KEY).find(|key| !used.contains(key)) else {
                continue;
            };
            entries[index].set_key(free);
            moved.push((key, free));
        }
    }
    moved
}

/// The response's `next_key` over every row.
fn next_key_of(entries: &[Entry], hint: Option<u64>) -> u64 {
    next_key(&stand_ins(entries), hint)
}

// --- the board envelope ----------------------------------------------------

/// Reads an optional field, taking `null` for its default as a missing field
/// is: platform encoders (System.Text.Json, kotlinx with explicit nulls)
/// write an unset nullable property as `null`.
fn nullable<'de, D, T>(deserializer: D) -> Result<T, D::Error>
where
    D: serde::Deserializer<'de>,
    T: Default + Deserialize<'de>,
{
    Option::<T>::deserialize(deserializer).map(Option::unwrap_or_default)
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct BoardRequest {
    rows: Vec<Value>,
    #[serde(default)]
    next_key: Option<u64>,
    #[serde(default, deserialize_with = "nullable")]
    edits: Vec<WireEdit>,
    #[serde(default)]
    resin: Option<WireResin>,
}

/// One EDIT: `{"type": "set_count", "key": 3, "count": 2}`. Every variant is
/// a struct, even one without fields: serde refuses unknown fields only
/// there, and a stray field is a platform bug worth an error.
#[derive(Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
enum WireEdit {
    Normalize {},
    Join {
        source: u64,
        target: u64,
    },
    Detach {
        key: u64,
    },
    Remove {
        key: u64,
    },
    RemoveOne {
        key: u64,
    },
    RemoveItem {
        key: u64,
    },
    SetCount {
        key: u64,
        count: u8,
    },
    SetTotal {
        key: u64,
        total: Option<u8>,
    },
    ToggleLevels {
        key: u64,
    },
    SetCopyDepth {
        key: u64,
        max_depth: Option<u8>,
    },
    Save {
        key: Option<u64>,
        requirement: Map<String, Value>,
        count: u8,
        total: Option<u8>,
        copy_depth: Option<u8>,
    },
}

impl WireEdit {
    fn edit(self) -> Result<Edit, String> {
        Ok(match self {
            Self::Normalize {} => Edit::Normalize,
            Self::Join { source, target } => Edit::Join { source, target },
            Self::Detach { key } => Edit::Detach { key },
            Self::Remove { key } => Edit::Remove { key },
            Self::RemoveOne { key } => Edit::RemoveOne { key },
            Self::RemoveItem { key } => Edit::RemoveItem { key },
            Self::SetCount { key, count } => Edit::SetCount { key, count },
            Self::SetTotal { key, total } => Edit::SetTotal { key, total },
            Self::ToggleLevels { key } => Edit::ToggleLevels { key },
            Self::SetCopyDepth { key, max_depth } => Edit::SetCopyDepth { key, max_depth },
            Self::Save {
                key,
                requirement,
                count,
                total,
                copy_depth,
            } => Edit::Save {
                key,
                requirement: read_requirement(&requirement)?,
                count,
                total,
                copy_depth,
            },
        })
    }
}

fn board(request: &str) -> Result<Value, Failure> {
    let request: BoardRequest = parse(request)?;
    let hint = request.next_key;
    let (mut entries, relabelled) = read_rows(request.rows)?;
    let edits = request
        .edits
        .into_iter()
        .enumerate()
        .map(|(index, edit)| {
            edit.edit()
                .map_err(|error| Failure::new(format!("edit {}: {error}", index + 1)))
        })
        .collect::<Result<Vec<_>, _>>()?;
    let resin = request.resin.map(WireResin::state).transpose()?;
    let mut rekeyed = repair(&mut entries, hint);
    let map = redirects(&rekeyed);
    let edits: Vec<Edit> = edits.into_iter().map(|edit| edit.resolved(&map)).collect();

    let mut run = Run::new(&entries, hint);
    run.edits(&edits);
    let mut merged = merge(&entries, run.rows, &run.unreadable);
    rekeyed.extend(separate(&mut merged));
    let rows = typed(&merged);
    let held = held_labels(&merged, |_| true);
    let view = board_view_beside(&rows, resin.as_ref(), (unread(&merged), &held));
    Ok(object(vec![
        ("rows", merged.iter().map(write_entry).collect()),
        ("next_key", next_key_of(&merged, hint).into()),
        (
            "changed",
            (run.changed || relabelled || !rekeyed.is_empty()).into(),
        ),
        ("rekeyed", pairs(&rekeyed)),
        ("focus", run.focus.into()),
        ("refused", run.refused.map_or(Value::Null, refusal)),
        ("items", items(&merged, &view)),
        ("counts", counts(&merged, &view)),
        ("problems", problems(&merged, &view.problems)),
        ("resin", view.resin.as_ref().map_or(Value::Null, resin_chip)),
    ]))
}

/// A board request's edits in progress. Edits on readable rows run through
/// [`super::apply`] in batches; an edit naming an unreadable row is a
/// removal of it or nothing, since the row takes part in no relationship.
struct Run<'a> {
    entries: &'a [Entry],
    rows: Vec<Row>,
    /// The unreadable rows still in the list.
    unreadable: BTreeSet<u64>,
    hint: Option<u64>,
    /// Whether the list differs from the request's. A batch reports its own
    /// net change, and batches are split only by an unreadable row's
    /// removal, itself a change — so this says what one [`super::apply`]
    /// of the whole request would.
    changed: bool,
    focus: Option<u64>,
    refused: Option<Refusal>,
}

impl<'a> Run<'a> {
    fn new(entries: &'a [Entry], hint: Option<u64>) -> Self {
        Self {
            entries,
            rows: typed(entries),
            unreadable: entries
                .iter()
                .filter(|entry| matches!(entry, Entry::Raw(_)))
                .map(Entry::key)
                .collect(),
            hint: readable_hint(entries, hint),
            changed: false,
            focus: None,
            refused: None,
        }
    }

    /// Runs `edits` in order: a list without unreadable rows is one
    /// [`super::apply`], so the envelope answers exactly as the typed call
    /// would.
    fn edits(&mut self, edits: &[Edit]) {
        let mut batch = Vec::new();
        for &edit in edits {
            match self.touch(&edit) {
                Touch::Readable => batch.push(edit),
                Touch::Ignored => {}
                Touch::Remove(key) => {
                    self.flush(&mut batch);
                    if self.refused.is_some() {
                        return;
                    }
                    self.unreadable.remove(&key);
                    self.changed = true;
                    self.focus = None;
                }
            }
        }
        self.flush(&mut batch);
    }

    fn flush(&mut self, batch: &mut Vec<Edit>) {
        if batch.is_empty() || self.refused.is_some() {
            batch.clear();
            return;
        }
        let held = held_labels(self.entries, |key| self.unreadable.contains(&key));
        let result = apply_holding(&self.rows, self.hint, batch, &held);
        batch.clear();
        // An edit that did nothing leaves the focus of the one before it.
        if result.changed || result.focus.is_some() {
            self.focus = result.focus;
        }
        self.changed |= result.changed;
        self.refused = result.refused;
        self.rows = result.rows;
    }

    /// Whether `edit` names an unreadable row.
    fn touch(&self, edit: &Edit) -> Touch {
        let named: Vec<u64> = match *edit {
            Edit::Normalize => Vec::new(),
            Edit::Join { source, target } => vec![source, target],
            Edit::Remove { key } | Edit::RemoveOne { key } | Edit::RemoveItem { key } => {
                return if self.unreadable.contains(&key) {
                    Touch::Remove(key)
                } else {
                    Touch::Readable
                };
            }
            Edit::Detach { key }
            | Edit::SetCount { key, .. }
            | Edit::SetTotal { key, .. }
            | Edit::ToggleLevels { key }
            | Edit::SetCopyDepth { key, .. } => vec![key],
            Edit::Save { key, .. } => key.into_iter().collect(),
        };
        if named.iter().any(|key| self.unreadable.contains(key)) {
            Touch::Ignored
        } else {
            Touch::Readable
        }
    }
}

enum Touch {
    Readable,
    Ignored,
    Remove(u64),
}

fn pairs(rekeyed: &[(u64, u64)]) -> Value {
    rekeyed
        .iter()
        .map(|&(old, new)| Value::from(vec![old, new]))
        .collect()
}

/// Every row's position in the response's list, by key.
fn positions(entries: &[Entry]) -> HashMap<u64, usize> {
    entries
        .iter()
        .enumerate()
        .map(|(index, entry)| (entry.key(), index))
        .collect()
}

/// The board's entries in list order, the unreadable rows' among them.
fn items(entries: &[Entry], view: &BoardView) -> Value {
    let position = positions(entries);
    let at = |key: u64| position.get(&key).copied().unwrap_or(usize::MAX);
    let mut items: Vec<(usize, Value)> = view
        .items
        .iter()
        .map(|entry| (at(entry.members[0]), item_view(entry)))
        .collect();
    for entry in entries {
        if let Entry::Raw(raw) = entry {
            items.push((at(raw.key), unreadable_item(raw)));
        }
    }
    items.sort_by_key(|(position, _)| *position);
    Value::Array(items.into_iter().map(|(_, item)| item).collect())
}

fn counts(entries: &[Entry], view: &BoardView) -> Value {
    let (mut ordinary, mut blanket) = (view.counts.ordinary, view.counts.blanket);
    for entry in entries {
        if let Entry::Raw(raw) = entry {
            if raw.blanket() {
                blanket += 1;
            } else {
                ordinary += 1;
            }
        }
    }
    object(vec![
        ("ordinary", ordinary.into()),
        ("blanket", blanket.into()),
    ])
}

/// The list's problems, each unreadable row's among the rows' own in list
/// order.
fn problems(entries: &[Entry], found: &[Problem]) -> Value {
    let position = positions(entries);
    let at = |keys: &[u64]| {
        keys.first()
            .and_then(|key| position.get(key))
            .copied()
            .unwrap_or(usize::MAX)
    };
    let mut rows: Vec<(usize, Value)> = found
        .iter()
        .filter(|problem| problem.scope == ProblemScope::Row)
        .map(|problem| (at(&problem.keys), problem_value(problem)))
        .collect();
    for entry in entries {
        if let Entry::Raw(raw) = entry {
            rows.push((
                at(&[raw.key]),
                object(vec![
                    ("message", raw.message.as_str().into()),
                    ("keys", vec![raw.key].into()),
                    ("scope", "row".into()),
                ]),
            ));
        }
    }
    rows.sort_by_key(|(position, _)| *position);
    let rest = found
        .iter()
        .filter(|problem| problem.scope != ProblemScope::Row)
        .map(problem_value);
    Value::Array(
        rows.into_iter()
            .map(|(_, value)| value)
            .chain(rest)
            .collect(),
    )
}

// --- the sheet envelope ----------------------------------------------------

#[derive(Deserialize)]
#[serde(tag = "op", rename_all = "snake_case", deny_unknown_fields)]
enum EditorRequest {
    Open {
        rows: Vec<Value>,
        #[serde(default)]
        key: Option<u64>,
        #[serde(default, deserialize_with = "nullable")]
        blanket: bool,
        #[serde(default)]
        resin: Option<WireResin>,
        #[serde(default, deserialize_with = "nullable")]
        offer_resin: bool,
        #[serde(default, deserialize_with = "nullable")]
        open_resin: bool,
    },
    Change {
        draft: String,
        change: WireChange,
    },
    Save {
        draft: String,
        rows: Vec<Value>,
        #[serde(default)]
        next_key: Option<u64>,
    },
}

fn editor(request: &str) -> Result<Value, Failure> {
    match parse(request)? {
        EditorRequest::Open {
            rows,
            key,
            blanket,
            resin,
            offer_resin,
            open_resin,
        } => {
            let (entries, _) = read_rows(rows)?;
            let resin = resin.map(WireResin::state).transpose()?;
            if !open_resin && let Some(key) = key {
                refuse_unreadable(&entries, key)?;
            }
            let draft = open(
                &typed(&entries),
                key,
                blanket,
                resin.as_ref(),
                offer_resin,
                open_resin,
            );
            Ok(sheet(&draft, &form(&draft)))
        }
        EditorRequest::Change {
            draft,
            change: wire,
        } => {
            let draft = read_draft(&draft)?;
            let next = change(&draft, &wire.change(draft.requirement.kind)?);
            Ok(sheet(&next, &form(&next)))
        }
        EditorRequest::Save {
            draft,
            rows,
            next_key,
        } => save_sheet(&read_draft(&draft)?, rows, next_key),
    }
}

/// Fails when the first row holding `key` is unreadable: it has no sheet.
fn refuse_unreadable(entries: &[Entry], key: u64) -> Result<(), Failure> {
    match entries.iter().find(|entry| entry.key() == key) {
        Some(Entry::Raw(raw)) => Err(Failure::at(key, raw.message.clone())),
        _ => Ok(()),
    }
}

/// Saves `draft` onto the request's rows. The rows' keys are repaired here,
/// with the unreadable rows among them, and the draft's own key read through
/// the same repair, so the typed save sees a list whose keys are already
/// sound.
fn save_sheet(draft: &Draft, rows: Vec<Value>, hint: Option<u64>) -> Result<Value, Failure> {
    let (mut entries, relabelled) = read_rows(rows)?;
    let mut rekeyed = repair(&mut entries, hint);
    let map = redirects(&rekeyed);
    let follow = |key: u64| map.get(&key).copied().unwrap_or(key);
    let draft = Draft {
        key: draft.key.map(follow),
        origin: match draft.origin {
            Origin::Row(key) => Origin::Row(follow(key)),
            origin => origin,
        },
        ..draft.clone()
    };
    if let Some(key) = draft.key {
        refuse_unreadable(&entries, key)?;
    }
    let held = held_labels(&entries, |_| true);
    match save_holding(
        &draft,
        &typed(&entries),
        readable_hint(&entries, hint),
        &held,
    ) {
        SaveResult::Saved { result, resin } => {
            let live: BTreeSet<u64> = entries
                .iter()
                .filter(|entry| matches!(entry, Entry::Raw(_)))
                .map(Entry::key)
                .collect();
            let mut merged = merge(&entries, result.rows, &live);
            rekeyed.extend(result.rekeyed);
            rekeyed.extend(separate(&mut merged));
            let resin = match resin {
                ResinOutcome::Unchanged => Value::Null,
                ResinOutcome::Set(state) => object(vec![("set", resin_value(&state))]),
                ResinOutcome::Clear => object(vec![("clear", true.into())]),
            };
            let saved = object(vec![
                ("rows", merged.iter().map(write_entry).collect()),
                ("next_key", next_key_of(&merged, hint).into()),
                (
                    "changed",
                    (result.changed || relabelled || !rekeyed.is_empty()).into(),
                ),
                ("rekeyed", pairs(&rekeyed)),
                ("focus", result.focus.into()),
                ("resin", resin),
            ]);
            Ok(object(vec![("saved", saved)]))
        }
        SaveResult::Refused { draft, form } => Ok(sheet(&draft, &form)),
    }
}

/// `{"draft": DRAFT, "form": FORM}`.
fn sheet(draft: &Draft, form: &Form) -> Value {
    object(vec![
        ("draft", write_draft(draft).into()),
        ("form", form_value(form)),
    ])
}

/// One CHANGE: `{"type": "set_item", "value": "spear"}`; `value` has one
/// type per change.
#[derive(Deserialize)]
#[serde(
    tag = "type",
    content = "value",
    rename_all = "snake_case",
    deny_unknown_fields
)]
enum WireChange {
    SetCategory(WireFamily),
    SetWeaponType(WireWeaponType),
    SetKind(FileItemKind),
    SetItem(Option<String>),
    SetTierMode(WireTierMode),
    SetTier(u8),
    SetUpgradeMode(WireUpgradeMode),
    SetUpgrade(u8),
    SetEffectMode(WireEffectMode),
    ToggleEffect(String),
    SetUncursed(bool),
    SetSource(Option<FileItemSource>),
    SetFloorLimitEnabled(bool),
    SetFloorLimit(u8),
    SetExcludeResin(bool),
    SetTransmutationsEnabled(bool),
    SetTransmutations(u8),
    SetSelectTrinket(bool),
    SetCount(u8),
    SetCopyDepthEnabled(bool),
    SetCopyDepth(u8),
    SetCountLevels(bool),
    SetTotal(u8),
    SetResinAuto(bool),
    SetResinAmount(Option<f64>),
    SetIncludeMageWand(bool),
}

impl WireChange {
    /// The change, reading an effect name in the draft's `family` first
    /// (no name is shared between weapons and armor).
    fn change(self, family: ItemKind) -> Result<Change, Failure> {
        Ok(match self {
            Self::SetCategory(family) => Change::SetCategory(family.kind()),
            Self::SetWeaponType(category) => Change::SetWeaponType(category.category()),
            Self::SetKind(kind) => {
                let (kind, category) = kind.decompose();
                Change::SetKind(kind, category)
            }
            Self::SetItem(choice) => Change::SetItem(item_choice(choice.as_deref())?),
            Self::SetTierMode(mode) => Change::SetTierMode(mode.mode()),
            Self::SetTier(tier) => Change::SetTier(tier),
            Self::SetUpgradeMode(mode) => Change::SetUpgradeMode(mode.mode()),
            Self::SetUpgrade(upgrade) => Change::SetUpgrade(upgrade),
            Self::SetEffectMode(mode) => Change::SetEffectMode(mode.mode()),
            Self::ToggleEffect(name) => Change::ToggleEffect(
                [family, ItemKind::Weapon, ItemKind::Armor]
                    .into_iter()
                    .find_map(|kind| Effect::from_wire_name(kind, &name))
                    .ok_or_else(|| Failure::new(format!("unknown effect '{name}'")))?,
            ),
            Self::SetUncursed(value) => Change::SetUncursed(value),
            Self::SetSource(source) => Change::SetSource(source.map(ItemSource::from)),
            Self::SetFloorLimitEnabled(value) => Change::SetFloorLimitEnabled(value),
            Self::SetFloorLimit(value) => Change::SetFloorLimit(value),
            Self::SetExcludeResin(value) => Change::SetExcludeResin(value),
            Self::SetTransmutationsEnabled(value) => Change::SetTransmutationsEnabled(value),
            Self::SetTransmutations(value) => Change::SetTransmutations(value),
            Self::SetSelectTrinket(value) => Change::SetSelectTrinket(value),
            Self::SetCount(value) => Change::SetCount(value),
            Self::SetCopyDepthEnabled(value) => Change::SetCopyDepthEnabled(value),
            Self::SetCopyDepth(value) => Change::SetCopyDepth(value),
            Self::SetCountLevels(value) => Change::SetCountLevels(value),
            Self::SetTotal(value) => Change::SetTotal(value),
            Self::SetResinAuto(value) => Change::SetResinAuto(value),
            Self::SetResinAmount(value) => Change::SetResinAmount(value),
            Self::SetIncludeMageWand(value) => Change::SetIncludeMageWand(value),
        })
    }
}

/// The item picker's value: `null` for the wildcard, `"arcane_resin"`, or an
/// item's stable id.
const ARCANE_RESIN_CHOICE: &str = "arcane_resin";

fn item_choice(value: Option<&str>) -> Result<ItemChoice, Failure> {
    match value {
        None => Ok(ItemChoice::Any),
        Some(ARCANE_RESIN_CHOICE) => Ok(ItemChoice::ArcaneResin),
        Some(stable_id) => item_by_stable_id(stable_id)
            .map(|definition| ItemChoice::Item(definition.id))
            .ok_or_else(|| Failure::new(format!("unknown item '{stable_id}'"))),
    }
}

fn item_choice_value(choice: ItemChoice) -> Value {
    match choice {
        ItemChoice::Any => Value::Null,
        ItemChoice::Item(item_id) => item(item_id).stable_id.into(),
        ItemChoice::ArcaneResin => ARCANE_RESIN_CHOICE.into(),
    }
}

/// The six families by their document names.
#[derive(Clone, Copy, Deserialize)]
#[serde(rename_all = "snake_case")]
enum WireFamily {
    Weapon,
    Armor,
    Wand,
    Ring,
    Trinket,
    Artifact,
}

impl WireFamily {
    const fn kind(self) -> ItemKind {
        match self {
            Self::Weapon => ItemKind::Weapon,
            Self::Armor => ItemKind::Armor,
            Self::Wand => ItemKind::Wand,
            Self::Ring => ItemKind::Ring,
            Self::Trinket => ItemKind::Trinket,
            Self::Artifact => ItemKind::Artifact,
        }
    }
}

#[derive(Clone, Copy, Deserialize)]
#[serde(rename_all = "snake_case")]
enum WireWeaponType {
    Any,
    Melee,
    Thrown,
}

impl WireWeaponType {
    const fn category(self) -> Option<WeaponCategory> {
        match self {
            Self::Any => None,
            Self::Melee => Some(WeaponCategory::Melee),
            Self::Thrown => Some(WeaponCategory::Thrown),
        }
    }
}

const fn weapon_type_name(category: Option<WeaponCategory>) -> &'static str {
    match category {
        None => "any",
        Some(WeaponCategory::Melee) => "melee",
        Some(WeaponCategory::Thrown) => "thrown",
    }
}

#[derive(Clone, Copy, Deserialize)]
#[serde(rename_all = "snake_case")]
enum WireTierMode {
    Any,
    Exact,
    AtLeast,
    AtMost,
}

impl WireTierMode {
    const fn mode(self) -> TierMode {
        match self {
            Self::Any => TierMode::Any,
            Self::Exact => TierMode::Exact,
            Self::AtLeast => TierMode::AtLeast,
            Self::AtMost => TierMode::AtMost,
        }
    }
}

const fn tier_mode_name(mode: TierMode) -> &'static str {
    match mode {
        TierMode::Any => "any",
        TierMode::Exact => "exact",
        TierMode::AtLeast => "at_least",
        TierMode::AtMost => "at_most",
    }
}

#[derive(Clone, Copy, Deserialize)]
#[serde(rename_all = "snake_case")]
enum WireUpgradeMode {
    Any,
    Exact,
    AtLeast,
}

impl WireUpgradeMode {
    const fn mode(self) -> UpgradeMode {
        match self {
            Self::Any => UpgradeMode::Any,
            Self::Exact => UpgradeMode::Exact,
            Self::AtLeast => UpgradeMode::AtLeast,
        }
    }
}

const fn upgrade_mode_name(mode: UpgradeMode) -> &'static str {
    match mode {
        UpgradeMode::Any => "any",
        UpgradeMode::Exact => "exact",
        UpgradeMode::AtLeast => "at_least",
    }
}

#[derive(Clone, Copy, Deserialize)]
#[serde(rename_all = "snake_case")]
enum WireEffectMode {
    Any,
    AnyEnchantment,
    Specific,
}

impl WireEffectMode {
    const fn mode(self) -> EffectMode {
        match self {
            Self::Any => EffectMode::Any,
            Self::AnyEnchantment => EffectMode::AnyEnchantment,
            Self::Specific => EffectMode::Specific,
        }
    }
}

const fn effect_mode_name(mode: EffectMode) -> &'static str {
    match mode {
        EffectMode::Any => "any",
        EffectMode::AnyEnchantment => "any_enchantment",
        EffectMode::Specific => "specific",
    }
}

const fn effect_group_name(group: EffectGroup) -> &'static str {
    match group {
        EffectGroup::Enchantment => "enchantment",
        EffectGroup::Curse => "curse",
    }
}

// --- resin -----------------------------------------------------------------

/// RESIN: `{"amount": 1..65535 | "auto", "filter": {"uncursed",
/// "max_depth", "source", "include_mage_wand"}}`.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct WireResin {
    amount: Value,
    #[serde(default, deserialize_with = "nullable")]
    filter: WireResinFilter,
}

/// The resin filter; a field left out or `null` takes the engine's default,
/// uncursed donors included.
#[derive(Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct WireResinFilter {
    #[serde(default)]
    uncursed: Option<bool>,
    #[serde(default)]
    max_depth: Option<u8>,
    #[serde(default)]
    source: Option<FileItemSource>,
    #[serde(default, deserialize_with = "nullable")]
    include_mage_wand: bool,
}

impl WireResin {
    fn state(self) -> Result<ResinState, Failure> {
        let amount = match &self.amount {
            Value::String(word) if word == "auto" => ResinAmount::Auto,
            value => value
                .as_u64()
                .and_then(|amount| u16::try_from(amount).ok())
                .filter(|&amount| amount >= ARCANE_RESIN_MIN)
                .map(ResinAmount::AtLeast)
                .ok_or_else(|| {
                    Failure::new(format!(
                        "resin amount must be a whole number from {ARCANE_RESIN_MIN} to \
                         {ARCANE_RESIN_MAX}, or \"auto\""
                    ))
                })?,
        };
        Ok(ResinState {
            amount,
            filter: ArcaneResinFilter {
                include_mage_wand: self.filter.include_mage_wand,
                uncursed: self.filter.uncursed.unwrap_or(true),
                max_depth: self.filter.max_depth,
                source: self.filter.source.map(ItemSource::from),
            },
        })
    }
}

fn resin_value(state: &ResinState) -> Value {
    let ResinState { amount, filter } = state;
    let ArcaneResinFilter {
        include_mage_wand,
        uncursed,
        max_depth,
        source,
    } = filter;
    let amount = match amount {
        ResinAmount::Auto => Value::from("auto"),
        ResinAmount::AtLeast(amount) => Value::from(*amount),
    };
    object(vec![
        ("amount", amount),
        (
            "filter",
            object(vec![
                ("uncursed", (*uncursed).into()),
                ("max_depth", (*max_depth).into()),
                ("source", source_value(*source)),
                ("include_mage_wand", (*include_mage_wand).into()),
            ]),
        ),
    ])
}

// --- the draft -------------------------------------------------------------

/// The draft as its opaque string holds it: every [`Draft`] field, the
/// requirement and rows in the row codec.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
#[allow(clippy::struct_excessive_bools)] // The draft's own independent flags.
struct WireDraft {
    v: u8,
    origin: WireOrigin,
    key: Option<u64>,
    requirement: Map<String, Value>,
    tier_value: u8,
    upgrade_value: u8,
    effect_mode: WireEffectMode,
    count: u8,
    total: Option<u8>,
    copy_depth: Option<u8>,
    floor_limit_memory: u8,
    copy_depth_memory: u8,
    transmutations_memory: u8,
    in_cluster: bool,
    blanket: bool,
    offer_resin: bool,
    taken_trinkets: Vec<String>,
    resin_picked: bool,
    resin: WireResinDraft,
    query_resin: Option<WireResin>,
    rows: Vec<Value>,
}

#[derive(Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
enum WireOrigin {
    New {},
    Row { key: u64 },
    Resin {},
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct WireResinDraft {
    auto: bool,
    amount: Option<f64>,
    include_mage_wand: bool,
    uncursed: bool,
    max_depth: Option<u8>,
    source: Option<FileItemSource>,
}

/// The message of a draft this build cannot read; the platform reopens the
/// sheet from its rows.
const UNREADABLE_DRAFT: &str = "The draft cannot be read; reopen the sheet";

fn read_draft(text: &str) -> Result<Draft, Failure> {
    let unreadable = |detail: String| Failure::new(format!("{UNREADABLE_DRAFT}: {detail}"));
    let value: Value = serde_json::from_str(text).map_err(|error| unreadable(error.to_string()))?;
    let version = value.get("v").and_then(Value::as_u64);
    if version != Some(u64::from(super::DRAFT_VERSION)) {
        return Err(unreadable(format!(
            "it is version {}, this editor reads version {}",
            version.map_or_else(|| "?".to_owned(), |version| version.to_string()),
            super::DRAFT_VERSION
        )));
    }
    let wire: WireDraft =
        serde_json::from_value(value).map_err(|error| unreadable(error.to_string()))?;
    let (requirement, label) = read_row(&wire.requirement).map_err(unreadable)?;
    let (entries, _) = read_rows(wire.rows).map_err(|failure| unreadable(failure.message))?;
    let rows = entries
        .into_iter()
        .map(|entry| match entry {
            Entry::Row(row) => Ok(row),
            Entry::Raw(raw) => Err(unreadable(raw.message)),
        })
        .collect::<Result<Vec<_>, _>>()?;
    let query_resin = wire
        .query_resin
        .map(WireResin::state)
        .transpose()
        .map_err(|failure| unreadable(failure.message))?;
    let taken_trinkets = wire
        .taken_trinkets
        .iter()
        .map(|stable_id| {
            item_by_stable_id(stable_id)
                .map(|definition| definition.id)
                .ok_or_else(|| unreadable(format!("unknown item '{stable_id}'")))
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(Draft {
        v: wire.v,
        origin: match wire.origin {
            WireOrigin::New {} => Origin::New,
            WireOrigin::Row { key } => Origin::Row(key),
            WireOrigin::Resin {} => Origin::Resin,
        },
        key: wire.key,
        requirement: Requirement {
            alternative_group: label.and_then(|label| u8::try_from(label).ok()),
            ..requirement
        },
        tier_value: wire.tier_value,
        upgrade_value: wire.upgrade_value,
        effect_mode: wire.effect_mode.mode(),
        count: wire.count,
        total: wire.total,
        copy_depth: wire.copy_depth,
        floor_limit_memory: wire.floor_limit_memory,
        copy_depth_memory: wire.copy_depth_memory,
        transmutations_memory: wire.transmutations_memory,
        in_cluster: wire.in_cluster,
        blanket: wire.blanket,
        offer_resin: wire.offer_resin,
        taken_trinkets,
        resin_picked: wire.resin_picked,
        resin: ResinDraft {
            auto: wire.resin.auto,
            amount: wire.resin.amount,
            include_mage_wand: wire.resin.include_mage_wand,
            uncursed: wire.resin.uncursed,
            max_depth: wire.resin.max_depth,
            source: wire.resin.source.map(ItemSource::from),
        },
        query_resin,
        rows,
    })
}

/// The draft's opaque string.
fn write_draft(draft: &Draft) -> String {
    let Draft {
        v,
        origin,
        key,
        requirement,
        tier_value,
        upgrade_value,
        effect_mode,
        count,
        total,
        copy_depth,
        floor_limit_memory,
        copy_depth_memory,
        transmutations_memory,
        in_cluster,
        blanket,
        offer_resin,
        taken_trinkets,
        resin_picked,
        resin,
        query_resin,
        rows,
    } = draft;
    let ResinDraft {
        auto,
        amount,
        include_mage_wand,
        uncursed,
        max_depth,
        source,
    } = resin;
    object(vec![
        ("v", (*v).into()),
        ("origin", origin_value(*origin)),
        ("key", (*key).into()),
        (
            "requirement",
            Value::Object(requirement_fields(requirement)),
        ),
        ("tier_value", (*tier_value).into()),
        ("upgrade_value", (*upgrade_value).into()),
        ("effect_mode", effect_mode_name(*effect_mode).into()),
        ("count", (*count).into()),
        ("total", (*total).into()),
        ("copy_depth", (*copy_depth).into()),
        ("floor_limit_memory", (*floor_limit_memory).into()),
        ("copy_depth_memory", (*copy_depth_memory).into()),
        ("transmutations_memory", (*transmutations_memory).into()),
        ("in_cluster", (*in_cluster).into()),
        ("blanket", (*blanket).into()),
        ("offer_resin", (*offer_resin).into()),
        (
            "taken_trinkets",
            taken_trinkets
                .iter()
                .map(|&item_id| item(item_id).stable_id)
                .collect(),
        ),
        ("resin_picked", (*resin_picked).into()),
        (
            "resin",
            object(vec![
                ("auto", (*auto).into()),
                ("amount", (*amount).into()),
                ("include_mage_wand", (*include_mage_wand).into()),
                ("uncursed", (*uncursed).into()),
                ("max_depth", (*max_depth).into()),
                ("source", source_value(*source)),
            ]),
        ),
        (
            "query_resin",
            query_resin.as_ref().map_or(Value::Null, resin_value),
        ),
        ("rows", rows.iter().map(write_row).collect()),
    ])
    .to_string()
}

fn origin_value(origin: Origin) -> Value {
    match origin {
        Origin::New => object(vec![("type", "new".into())]),
        Origin::Row(key) => object(vec![("type", "row".into()), ("key", key.into())]),
        Origin::Resin => object(vec![("type", "resin".into())]),
    }
}

// --- writing the board -----------------------------------------------------

/// A JSON object from its fields. The envelopes write hundreds of fields per
/// answer; built in one loop rather than through `json!`'s inlined insertion
/// per field, they cost the browser's editor module — which the web app
/// downloads before its first render — a fraction of the code.
fn object(fields: Vec<(&str, Value)>) -> Value {
    Value::Object(
        fields
            .into_iter()
            .map(|(key, value)| (key.to_owned(), value))
            .collect(),
    )
}

fn source_value(source: Option<ItemSource>) -> Value {
    source.map(source_name).into()
}

fn item_value(item_id: Option<ItemId>) -> Value {
    item_id.map(|item_id| item(item_id).stable_id).into()
}

fn refusal(refusal: Refusal) -> Value {
    object(vec![
        ("reason", refusal.name().into()),
        ("message", refusal.to_string().into()),
    ])
}

fn problem_value(problem: &Problem) -> Value {
    let Problem {
        message,
        keys,
        scope,
    } = problem;
    let scope = match scope {
        ProblemScope::Row => "row",
        ProblemScope::Group => "group",
        ProblemScope::List => "list",
    };
    object(vec![
        ("message", message.as_str().into()),
        ("keys", keys.as_slice().into()),
        ("scope", scope.into()),
    ])
}

fn tags(tags: &[Tag]) -> Value {
    tags.iter()
        .map(
            |Tag {
                 text,
                 style,
                 tooltip,
             }| {
                let style = match style {
                    TagStyle::Plain => "plain",
                    TagStyle::Upgrade => "upgrade",
                    TagStyle::Credit => "credit",
                };
                object(vec![
                    ("text", text.as_str().into()),
                    ("style", style.into()),
                    ("tooltip", tooltip.as_deref().into()),
                ])
            },
        )
        .collect()
}

fn stack_value(stack: &StackView) -> Value {
    let StackView {
        count,
        max,
        can_grow,
        can_change_count,
        count_max,
        total,
        can_count_levels,
        level_capacity,
        default_total,
        copy_depth,
        can_set_copy_depth,
        count_text,
        total_text,
    } = stack;
    object(vec![
        ("count", (*count).into()),
        ("max", (*max).into()),
        ("can_grow", (*can_grow).into()),
        ("can_change_count", (*can_change_count).into()),
        ("count_max", (*count_max).into()),
        ("total", (*total).into()),
        ("can_count_levels", (*can_count_levels).into()),
        ("level_capacity", (*level_capacity).into()),
        ("default_total", (*default_total).into()),
        ("copy_depth", (*copy_depth).into()),
        ("can_set_copy_depth", (*can_set_copy_depth).into()),
        ("count_text", count_text.as_str().into()),
        ("total_text", total_text.as_str().into()),
    ])
}

fn badge_value(badge: Option<&Badge>) -> Value {
    badge.map_or(Value::Null, |badge| {
        let Badge {
            text,
            compact_text,
            tooltip,
        } = badge;
        object(vec![
            ("text", text.as_str().into()),
            ("compact_text", compact_text.as_str().into()),
            ("tooltip", tooltip.as_str().into()),
        ])
    })
}

fn item_view(view: &ItemView) -> Value {
    let ItemView {
        id,
        blanket,
        cluster,
        label,
        name,
        members,
        extras,
        chips,
        problem,
    } = view;
    object(vec![
        ("id", id.to_string().into()),
        ("blanket", (*blanket).into()),
        ("cluster", (*cluster).into()),
        ("label", label.as_deref().into()),
        ("name", name.as_str().into()),
        ("members", members.as_slice().into()),
        ("extras", extras.as_slice().into()),
        ("chips", chips.iter().map(chip_value).collect()),
        ("problem", problem.as_deref().into()),
    ])
}

fn chip_value(chip: &ChipView) -> Value {
    let ChipView {
        key,
        name,
        title,
        item: item_id,
        kind,
        family,
        tags: leading,
        trailing_tags,
        effect,
        uncursed,
        details,
        relations,
        badges: Badges { count, total },
        copies,
        stack,
        description,
        problem,
        in_cluster,
        can_detach,
        join,
        refuse,
    } = chip;
    let relations = relations
        .iter()
        .map(|Relation { glyph, text }| {
            let glyph = match glyph {
                RelationGlyph::Or => "or",
                RelationGlyph::Sum => "sum",
                RelationGlyph::Times => "times",
            };
            object(vec![
                ("glyph", glyph.into()),
                ("text", text.as_str().into()),
            ])
        })
        .collect();
    let refuse = refuse
        .iter()
        .map(|&(key, reason)| {
            object(vec![
                ("key", key.into()),
                ("reason", reason.name().into()),
                ("message", reason.to_string().into()),
            ])
        })
        .collect();
    object(vec![
        ("key", (*key).into()),
        ("name", name.as_str().into()),
        ("title", title.as_str().into()),
        ("item", item_value(*item_id)),
        ("kind", kind.name().into()),
        ("family", kind_name(*family).into()),
        ("tags", tags(leading)),
        ("trailing_tags", tags(trailing_tags)),
        ("effect", effect.as_ref().map_or(Value::Null, effect_value)),
        ("uncursed", (*uncursed).into()),
        ("details", details.as_slice().into()),
        ("relations", relations),
        ("description", description.as_str().into()),
        ("problem", problem.as_deref().into()),
        (
            "badges",
            object(vec![
                ("count", badge_value(count.as_ref())),
                ("total", badge_value(total.as_ref())),
            ]),
        ),
        ("copies", copies.as_slice().into()),
        ("stack", stack_value(stack)),
        ("in_cluster", (*in_cluster).into()),
        ("can_detach", (*can_detach).into()),
        ("join", join.as_slice().into()),
        ("refuse", refuse),
    ])
}

fn effect_value(effect: &EffectBadge) -> Value {
    let EffectBadge {
        label,
        effects,
        any_enchantment,
        curses_only,
    } = effect;
    object(vec![
        ("label", label.as_str().into()),
        (
            "effects",
            effects.iter().map(|effect| effect.wire_name()).collect(),
        ),
        ("any_enchantment", (*any_enchantment).into()),
        ("curses_only", (*curses_only).into()),
    ])
}

fn resin_chip(chip: &ResinChip) -> Value {
    let ResinChip {
        name,
        tags: leading,
        uncursed,
        tooltip,
        details,
        description,
    } = chip;
    object(vec![
        ("name", name.as_str().into()),
        ("tags", tags(leading)),
        ("uncursed", (*uncursed).into()),
        ("tooltip", tooltip.as_deref().into()),
        ("details", details.as_slice().into()),
        ("description", description.as_str().into()),
    ])
}

/// The standalone entry of an unreadable row: a chip with no kind, no
/// relationships and no stack, whose problem says why it cannot be read.
/// Only `remove`, `remove_one` and `remove_item` act on it.
fn unreadable_item(raw: &Raw) -> Value {
    let none = || Value::Array(Vec::new());
    let stack = object(vec![
        ("count", 1.into()),
        ("max", STACK_MAX.into()),
        ("can_grow", false.into()),
        ("can_change_count", false.into()),
        ("count_max", 1.into()),
        ("total", Value::Null),
        ("can_count_levels", false.into()),
        ("level_capacity", 1.into()),
        ("default_total", 1.into()),
        ("copy_depth", Value::Null),
        ("can_set_copy_depth", false.into()),
        ("count_text", count_text(1, false).into()),
        ("total_text", total_text(0).into()),
    ]);
    let chip = object(vec![
        ("key", raw.key.into()),
        ("name", UNKNOWN_REQUIREMENT.into()),
        ("title", UNKNOWN_REQUIREMENT.into()),
        ("item", Value::Null),
        ("kind", Value::Null),
        ("family", Value::Null),
        ("tags", none()),
        ("trailing_tags", none()),
        ("effect", Value::Null),
        ("uncursed", false.into()),
        ("details", none()),
        ("relations", none()),
        ("description", UNKNOWN_REQUIREMENT.into()),
        ("problem", raw.message.as_str().into()),
        (
            "badges",
            object(vec![("count", Value::Null), ("total", Value::Null)]),
        ),
        ("copies", none()),
        ("stack", stack),
        ("in_cluster", false.into()),
        ("can_detach", false.into()),
        ("join", none()),
        ("refuse", none()),
    ]);
    object(vec![
        ("id", super::ItemKey::Chip(raw.key).to_string().into()),
        ("blanket", raw.blanket().into()),
        ("cluster", Value::Null),
        ("label", Value::Null),
        ("name", UNKNOWN_REQUIREMENT.into()),
        ("members", vec![raw.key].into()),
        ("extras", none()),
        ("chips", vec![chip].into()),
        ("problem", raw.message.as_str().into()),
    ])
}

// --- writing the form ------------------------------------------------------

fn opt<T>(option: &Opt<T>, value: impl Fn(&T) -> Value) -> Value {
    let Opt {
        value: choice,
        label,
        group,
        hidden,
    } = option;
    object(vec![
        ("value", value(choice)),
        ("label", label.as_str().into()),
        ("group", group.as_deref().into()),
        ("hidden", (*hidden).into()),
    ])
}

fn choice_value<T>(control: &Choice<T>, value: impl Fn(&T) -> Value) -> Value {
    let Choice {
        visible,
        value: chosen,
        options,
    } = control;
    object(vec![
        ("visible", (*visible).into()),
        ("value", value(chosen)),
        (
            "options",
            options.iter().map(|option| opt(option, &value)).collect(),
        ),
    ])
}

fn mode_range<M>(control: &ModeRange<M>, mode_name: impl Fn(&M) -> Value) -> Value {
    let ModeRange {
        visible,
        mode,
        modes,
        value_visible,
        value,
        min,
        max,
        value_label,
    } = control;
    object(vec![
        ("visible", (*visible).into()),
        ("mode", mode_name(mode)),
        (
            "modes",
            modes.iter().map(|option| opt(option, &mode_name)).collect(),
        ),
        ("value_visible", (*value_visible).into()),
        ("value", (*value).into()),
        ("min", (*min).into()),
        ("max", (*max).into()),
        ("value_label", value_label.as_str().into()),
    ])
}

fn toggle(control: &Toggle) -> Value {
    let Toggle {
        visible,
        value,
        label,
        caption,
    } = control;
    object(vec![
        ("visible", (*visible).into()),
        ("value", (*value).into()),
        ("label", label.as_str().into()),
        ("caption", caption.as_deref().into()),
    ])
}

fn floor_toggle(control: &FloorToggle) -> Value {
    let FloorToggle {
        visible,
        enabled,
        value,
        options,
        label,
        value_label,
    } = control;
    object(vec![
        ("visible", (*visible).into()),
        ("enabled", (*enabled).into()),
        ("value", (*value).into()),
        (
            "options",
            options
                .iter()
                .map(|option| opt(option, |floor| Value::from(*floor)))
                .collect(),
        ),
        ("label", label.as_str().into()),
        ("value_label", value_label.as_str().into()),
    ])
}

fn range_toggle(control: &RangeToggle) -> Value {
    let RangeToggle {
        visible,
        enabled,
        value,
        min,
        max,
        label,
        caption,
        caption_visible,
        value_label,
    } = control;
    object(vec![
        ("visible", (*visible).into()),
        ("enabled", (*enabled).into()),
        ("value", (*value).into()),
        ("min", (*min).into()),
        ("max", (*max).into()),
        ("label", label.as_str().into()),
        ("caption", caption.as_deref().into()),
        ("caption_visible", (*caption_visible).into()),
        ("value_label", value_label.as_str().into()),
    ])
}

fn effect_control(control: &EffectControl) -> Value {
    let EffectControl {
        visible,
        label,
        mode,
        modes,
        choices_visible,
        choices,
        groups,
        caption,
    } = control;
    let choices = choices
        .iter()
        .map(
            |EffectChoice {
                 value,
                 label,
                 group,
                 selected,
             }| {
                object(vec![
                    ("value", value.wire_name().into()),
                    ("label", label.as_str().into()),
                    ("group", effect_group_name(*group).into()),
                    ("selected", (*selected).into()),
                ])
            },
        )
        .collect();
    object(vec![
        ("visible", (*visible).into()),
        ("label", label.as_str().into()),
        ("mode", effect_mode_name(*mode).into()),
        (
            "modes",
            modes
                .iter()
                .map(|option| opt(option, |mode| effect_mode_name(*mode).into()))
                .collect(),
        ),
        ("choices_visible", (*choices_visible).into()),
        ("choices", choices),
        (
            "groups",
            groups
                .iter()
                .map(|option| opt(option, |group| effect_group_name(*group).into()))
                .collect(),
        ),
        ("caption", caption.as_str().into()),
    ])
}

fn stack_control(control: &StackControl) -> Value {
    let StackControl {
        visible,
        label,
        count,
        min,
        max,
        value_label,
        copy_depth,
        count_levels,
    } = control;
    object(vec![
        ("visible", (*visible).into()),
        ("label", label.as_str().into()),
        ("count", (*count).into()),
        ("min", (*min).into()),
        ("max", (*max).into()),
        ("value_label", value_label.as_str().into()),
        ("copy_depth", floor_toggle(copy_depth)),
        ("count_levels", range_toggle(count_levels)),
    ])
}

fn resin_control(control: &ResinControl) -> Value {
    let ResinControl {
        visible,
        label,
        auto,
        modes,
        caption,
        amount,
        min,
        max,
        include_mage_wand,
    } = control;
    object(vec![
        ("visible", (*visible).into()),
        ("label", label.as_str().into()),
        ("auto", (*auto).into()),
        (
            "modes",
            modes
                .iter()
                .map(|option| opt(option, |auto| Value::from(*auto)))
                .collect(),
        ),
        ("caption", caption.as_str().into()),
        ("amount", (*amount).into()),
        ("min", (*min).into()),
        ("max", (*max).into()),
        ("include_mage_wand", toggle(include_mage_wand)),
    ])
}

fn form_value(form: &Form) -> Value {
    let Form {
        v,
        mode,
        origin,
        blanket,
        in_cluster,
        resin_picked,
        title,
        preview,
        category,
        kind,
        weapon_type,
        item: item_control,
        tier,
        upgrade,
        effect,
        uncursed,
        source,
        floor_limit,
        exclude_resin,
        transmutations,
        select_trinket,
        stack,
        resin,
        errors,
        can_save,
    } = form;
    let mode = match mode {
        FormMode::New => "new",
        FormMode::Edit => "edit",
    };
    object(vec![
        ("v", (*v).into()),
        ("mode", mode.into()),
        ("origin", origin_value(*origin)),
        ("blanket", (*blanket).into()),
        ("in_cluster", (*in_cluster).into()),
        ("resin_picked", (*resin_picked).into()),
        ("title", title.as_str().into()),
        ("preview", preview.as_ref().map_or(Value::Null, chip_value)),
        (
            "category",
            choice_value(category, |kind| kind_name(*kind).into()),
        ),
        (
            "kind",
            choice_value(kind, |kind: &KindName| kind.name().into()),
        ),
        (
            "weapon_type",
            choice_value(weapon_type, |category| weapon_type_name(*category).into()),
        ),
        (
            "item",
            choice_value(item_control, |choice| item_choice_value(*choice)),
        ),
        (
            "tier",
            mode_range(tier, |mode| tier_mode_name(*mode).into()),
        ),
        (
            "upgrade",
            mode_range(upgrade, |mode| upgrade_mode_name(*mode).into()),
        ),
        ("effect", effect_control(effect)),
        ("uncursed", toggle(uncursed)),
        (
            "source",
            choice_value(source, |source| source_value(*source)),
        ),
        ("floor_limit", floor_toggle(floor_limit)),
        ("exclude_resin", toggle(exclude_resin)),
        ("transmutations", range_toggle(transmutations)),
        ("select_trinket", toggle(select_trinket)),
        ("stack", stack_control(stack)),
        ("resin", resin_control(resin)),
        ("errors", errors.as_slice().into()),
        ("can_save", (*can_save).into()),
    ])
}

#[cfg(test)]
mod tests;
