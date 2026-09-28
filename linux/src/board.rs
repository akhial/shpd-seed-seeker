// SPDX-License-Identifier: GPL-3.0-or-later

//! The requirement board's side of the shared editor
//! ([`shpd_seedfinder_core::editor`]).
//!
//! The editor decides everything a board shows and does — how the rows fold
//! into chips, clusters and stacks, what every chip, tag and badge says,
//! which chips may be joined and why a join is refused, what is wrong with a
//! row. This module only reads its answers the way the GTK board needs them:
//! the view, cached per list; where a dragged chip may land; the menu's
//! either/or choices; the tooltip lines. It holds no GTK, so its tests run
//! anywhere.

use std::fmt::Write as _;
use std::rc::Rc;

use shpd_seedfinder_core::editor::{
    self, Badge, BoardView, ChipView, DropAction, DropTarget, Edit, ItemView, RelationGlyph,
    ResinChip, ResinState, Row, TagStyle,
};

/// The board view of the rows last shown. The pane redraws the board on
/// every refresh — a scope control moving included — but the rows change
/// far less often, so the editor folds them once per change of the list.
#[derive(Default)]
pub struct BoardCache {
    shown: Option<Shown>,
}

struct Shown {
    rows: Vec<Row>,
    resin: Option<ResinState>,
    view: Rc<BoardView>,
}

impl BoardCache {
    /// The view of `rows` beside the query's `resin`, folded again only when
    /// either differs from the last call's.
    pub fn view(&mut self, rows: &[Row], resin: Option<ResinState>) -> Rc<BoardView> {
        if let Some(shown) = &self.shown
            && shown.rows == rows
            && shown.resin == resin
        {
            return Rc::clone(&shown.view);
        }
        let view = Rc::new(editor::board_view(rows, resin.as_ref()));
        self.shown = Some(Shown {
            rows: rows.to_vec(),
            resin,
            view: Rc::clone(&view),
        });
        view
    }

    /// The view last handed out, for gestures on the board it drew.
    #[must_use]
    pub fn current(&self) -> Option<Rc<BoardView>> {
        self.shown.as_ref().map(|shown| Rc::clone(&shown.view))
    }

    /// The chip showing row `key` on the board last handed out, picked up.
    #[must_use]
    pub fn pick_up(&self, key: u64) -> Option<Dragged> {
        let shown = self.shown.as_ref()?;
        let (item, chip) = find_chip(&shown.view, key)?;
        Some(Dragged::new(&shown.rows, item, chip))
    }
}

/// Where a key the board holds on to — the row an open stack stepper edits —
/// went when the editor repaired the list's keys. Only a key no row could
/// keep, zero or past [`editor::MAX_KEY`], moves, to its row's new key; the
/// first row holding a duplicated key keeps it, so a valid key stays put.
#[must_use]
pub fn follow_key(key: u64, rekeyed: &[(u64, u64)]) -> u64 {
    if (1..=editor::MAX_KEY).contains(&key) {
        return key;
    }
    rekeyed
        .iter()
        .find(|&&(old, _)| old == key)
        .map_or(key, |&(_, new)| new)
}

/// The board entry showing the visible row `key`, and that row's chip.
#[must_use]
pub fn find_chip(view: &BoardView, key: u64) -> Option<(&ItemView, &ChipView)> {
    view.items.iter().find_map(|item| {
        item.chips
            .iter()
            .find(|chip| chip.key == key)
            .map(|chip| (item, chip))
    })
}

/// Where a dragged chip is released, as the board hit-tests it.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Landing {
    /// On a chip, or on a cluster's capsule outside its chips, which stands
    /// for its first member.
    Row(u64),
    /// On the empty board of one section.
    Board { blanket: bool },
    /// On the bin, which takes one item.
    Remove,
}

/// What releasing a dragged chip somewhere does.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DropAnswer {
    /// The drop sends this edit; the target lights up.
    Accept(Edit),
    /// The editor refuses this join or removal. The target is marked as
    /// refusing, and the drop still sends the edit so the editor's refusal
    /// is said.
    Refuse(Edit),
    /// Nothing happens here, and the target does not light up.
    Ignore,
}

impl DropAnswer {
    /// The edit a release sends, if any.
    #[must_use]
    pub const fn edit(self) -> Option<Edit> {
        match self {
            Self::Accept(edit) | Self::Refuse(edit) => Some(edit),
            Self::Ignore => None,
        }
    }
}

/// The chip in flight, as the drop targets under it read it: its row, its
/// section, and the editor's answers about where it may land — taken once
/// when the drag begins, so hovering asks the editor nothing. A drag moves
/// one item: the chip's stack stays behind but for the one copy it carries.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Dragged {
    pub key: u64,
    blanket: bool,
    can_detach: bool,
    join: Vec<u64>,
    refuse: Vec<u64>,
    remove_refused: bool,
}

impl Dragged {
    /// The chip `chip` of the board entry `item`, drawn from `rows`, picked
    /// up.
    #[must_use]
    pub fn new(rows: &[Row], item: &ItemView, chip: &ChipView) -> Self {
        Self {
            key: chip.key,
            blanket: item.blanket,
            can_detach: chip.can_detach,
            join: chip.join.clone(),
            refuse: chip.refuse.iter().map(|&(key, _)| key).collect(),
            remove_refused: matches!(
                editor::drop_action(rows, chip.key, DropTarget::Remove),
                DropAction::Refuse(_)
            ),
        }
    }

    /// What releasing the chip on `landing` does: onto a row it joins when
    /// the editor lists the row among the chip's joins, and is refused when
    /// it lists it among its refusals; onto the empty board of its own
    /// section a cluster member leaves its cluster while a lone chip stays
    /// where it is; onto the bin one item goes — the chip's own row, or one
    /// copy of its stack — unless the editor has no label left to write the
    /// rest with.
    #[must_use]
    pub fn drop_answer(&self, landing: Landing) -> DropAnswer {
        let source = self.key;
        match landing {
            Landing::Row(target) if self.join.contains(&target) => {
                DropAnswer::Accept(Edit::Join { source, target })
            }
            Landing::Row(target) if self.refuse.contains(&target) => {
                DropAnswer::Refuse(Edit::Join { source, target })
            }
            Landing::Board { blanket } if self.can_detach && blanket == self.blanket => {
                DropAnswer::Accept(Edit::Detach { key: source })
            }
            Landing::Remove if self.remove_refused => {
                DropAnswer::Refuse(Edit::RemoveOne { key: source })
            }
            Landing::Remove => DropAnswer::Accept(Edit::RemoveOne { key: source }),
            Landing::Row(_) | Landing::Board { .. } => DropAnswer::Ignore,
        }
    }
}

/// Which of a chip's numbers a badge shows and its stepper edits.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StackField {
    /// How many items the chip asks for.
    Count,
    /// The combined level its items reach together.
    Total,
}

/// One badge a chip wears at rest, and the stepper it opens: the number it
/// edits, where the stepper starts and how far it goes.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StackBadge<'a> {
    pub field: StackField,
    pub badge: &'a Badge,
    pub value: u8,
    pub maximum: u8,
}

/// The badges `chip` wears on the board: its own ×N (or ≤N) and Σ, as the
/// editor words them. Every badge and stepper belongs to a chip — a lone
/// chip or a cluster member alike, each member of a cluster with its own
/// stack — and nothing is drawn for a cluster as a whole. A chip in flight
/// carries one item and wears none.
#[must_use]
pub fn stack_badges(chip: &ChipView) -> Vec<StackBadge<'_>> {
    let stack = &chip.stack;
    let count = chip.badges.count.as_ref().map(|badge| StackBadge {
        field: StackField::Count,
        badge,
        value: stack.count,
        maximum: stack.count_max,
    });
    let total = chip.badges.total.as_ref().map(|badge| StackBadge {
        field: StackField::Total,
        badge,
        value: stack.total.unwrap_or(stack.default_total),
        maximum: stack.level_capacity,
    });
    count.into_iter().chain(total).collect()
}

/// The badges `chip` shows where it was picked up from while one of its
/// items is lifted away: the stack as the editor says the bin would leave
/// it, one copy fewer and its Σ capped or dropped — the chip's own
/// `remaining_badges`, which may be none at all. `None` when the chip has
/// no copies and leaves whole, so its origin stays as it was.
#[must_use]
pub fn left_behind(chip: &ChipView) -> Option<Vec<(StackField, &Badge)>> {
    let badges = chip.remaining_badges.as_ref()?;
    let count = badges.count.iter().map(|badge| (StackField::Count, badge));
    let total = badges.total.iter().map(|badge| (StackField::Total, badge));
    Some(count.chain(total).collect())
}

/// One choice of a chip's "Either/or with…" menu: a board entry of the
/// chip's section, named as the board reads it, and the row a join names.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct JoinChoice {
    pub target: u64,
    pub label: String,
}

/// The "Either/or with…" choices of `chip`, one per board entry the editor
/// lets it join or refuses it — choosing a refused one says why — in board
/// order, each under the entry's name (`Spear or Mace` for a cluster).
#[must_use]
pub fn join_choices(view: &BoardView, chip: &ChipView) -> Vec<JoinChoice> {
    view.items
        .iter()
        .filter_map(|item| {
            let target = item.members.iter().copied().find(|key| {
                chip.join.contains(key) || chip.refuse.iter().any(|(refused, _)| refused == key)
            })?;
            Some(JoinChoice {
                target,
                label: item.name.clone(),
            })
        })
        .collect()
}

/// The style class a chip's tag is tinted with: upgrades and the resin the
/// resin chip counts in the success colour, apart from the plain filters.
#[must_use]
pub const fn tag_class(style: TagStyle) -> &'static str {
    match style {
        TagStyle::Plain => "chip-tag-plain",
        TagStyle::Upgrade => "chip-tag-up",
        TagStyle::Credit => "chip-tag-credit",
    }
}

/// A chip's tooltip — everything the chip is too small to say, in the
/// editor's words: the title, what it asks of its item, how it relates to
/// the chips around it, and what is wrong with it.
#[must_use]
pub fn chip_tooltip(chip: &ChipView) -> String {
    let mut text = chip.title.clone();
    if !chip.details.is_empty() {
        let _ = write!(text, "\n{}", chip.details.join(" \u{b7} "));
    }
    for relation in &chip.relations {
        let glyph = match relation.glyph {
            RelationGlyph::Or => "or",
            RelationGlyph::Sum => "\u{3a3}",
            RelationGlyph::Times => "\u{d7}",
        };
        let _ = write!(text, "\n{glyph} {}", relation.text);
    }
    if let Some(problem) = &chip.problem {
        let _ = write!(text, "\n{problem}");
    }
    text
}

/// The resin chip's tooltip: its name, then what the query asks of the
/// resin and its donor wands.
#[must_use]
pub fn resin_tooltip(resin: &ResinChip) -> String {
    format!("{}\n{}", resin.name, resin.details.join(" \u{b7} "))
}

#[cfg(test)]
mod tests {
    use serde_json::{Value, json};
    use shpd_seedfinder_core::catalog::{ItemId, ItemKind};
    use shpd_seedfinder_core::editor::{
        self, Badge, Badges, BoardView, ChipView, Edit, EditResult, ItemView, Refusal, ResinAmount,
        ResinState, Row, TagStyle, labels,
    };
    use shpd_seedfinder_core::query::{
        ArcaneResinFilter, LevelSum, Requirement, UpgradeRequirement,
    };

    use super::{
        BoardCache, Dragged, DropAnswer, Landing, StackField, chip_tooltip, find_chip, follow_key,
        join_choices, left_behind, resin_tooltip, stack_badges, tag_class,
    };
    use crate::fixtures::{
        Fixture, decode_requirement, decode_resin, decode_row, decode_rows, encode_row, fixtures,
        keys, tags,
    };
    use crate::state::AppState;

    fn decode_edit(edit: &Value) -> Edit {
        let key = |field: &str| edit[field].as_u64().unwrap();
        let small = |field: &str| {
            edit[field]
                .as_u64()
                .map(|value| u8::try_from(value).unwrap())
        };
        match edit["type"].as_str().unwrap() {
            "normalize" => Edit::Normalize,
            "join" => Edit::Join {
                source: key("source"),
                target: key("target"),
            },
            "detach" => Edit::Detach { key: key("key") },
            "remove" => Edit::Remove { key: key("key") },
            "remove_one" => Edit::RemoveOne { key: key("key") },
            "remove_item" => Edit::RemoveItem { key: key("key") },
            "set_count" => Edit::SetCount {
                key: key("key"),
                count: small("count").unwrap(),
            },
            "set_total" => Edit::SetTotal {
                key: key("key"),
                total: small("total"),
            },
            "toggle_levels" => Edit::ToggleLevels { key: key("key") },
            "set_copy_depth" => Edit::SetCopyDepth {
                key: key("key"),
                max_depth: small("max_depth"),
            },
            "save" => Edit::Save {
                key: edit["key"].as_u64(),
                requirement: decode_requirement(&edit["requirement"]),
                count: small("count").unwrap(),
                total: small("total"),
                copy_depth: small("copy_depth"),
            },
            other => panic!("unknown edit {other}"),
        }
    }

    /// The typed answer compared with the envelope's golden one, field by
    /// field the board reads.
    fn assert_matches(name: &str, result: &EditResult, view: &BoardView, response: &Value) {
        let rows: Vec<Value> = result.rows.iter().map(encode_row).collect();
        assert_eq!(Value::Array(rows), response["rows"], "{name}: rows");
        assert_eq!(result.changed, response["changed"], "{name}: changed");
        assert_eq!(result.next_key, response["next_key"], "{name}: next_key");
        assert_eq!(
            json!(result.rekeyed),
            response["rekeyed"],
            "{name}: rekeyed"
        );
        assert_eq!(json!(result.focus), response["focus"], "{name}: focus");
        let refused = result.refused.map_or(
            Value::Null,
            |refusal| json!({ "reason": refusal.name(), "message": refusal.to_string() }),
        );
        assert_eq!(refused, response["refused"], "{name}: refused");
        let counts = json!({ "ordinary": view.counts.ordinary, "blanket": view.counts.blanket });
        assert_eq!(counts, response["counts"], "{name}: counts");
        let problems: Vec<Value> = view
            .problems
            .iter()
            .map(|problem| json!([problem.message, problem.keys]))
            .collect();
        let expected: Vec<Value> = response["problems"]
            .as_array()
            .unwrap()
            .iter()
            .map(|problem| json!([problem["message"], problem["keys"]]))
            .collect();
        assert_eq!(problems, expected, "{name}: problems");
        let items = response["items"].as_array().unwrap();
        assert_eq!(view.items.len(), items.len(), "{name}: items");
        for (item, expected) in view.items.iter().zip(items) {
            assert_item_matches(name, item, expected);
        }
        let resin = view.resin.as_ref().map_or(Value::Null, |resin| {
            json!({
                "name": resin.name,
                "tags": tags(&resin.tags),
                "uncursed": resin.uncursed,
                "tooltip": resin.tooltip,
                "details": resin.details,
                "description": resin.description,
            })
        });
        // Every field of the resin chip, its tags' styles and tooltips too.
        assert_eq!(&resin, &response["resin"], "{name}: resin");
    }

    fn assert_item_matches(name: &str, item: &ItemView, expected: &Value) {
        assert_eq!(item.id.to_string(), expected["id"], "{name}: id");
        assert_eq!(item.name, expected["name"], "{name}: entry name");
        assert_eq!(item.blanket, expected["blanket"], "{name}");
        assert_eq!(item.members, keys(&expected["members"]), "{name}");
        assert_eq!(item.extras, keys(&expected["extras"]), "{name}");
        assert_eq!(json!(item.label), expected["label"], "{name}: label");
        assert_eq!(json!(item.problem), expected["problem"], "{name}");
        let chips = expected["chips"].as_array().unwrap();
        assert_eq!(item.chips.len(), chips.len(), "{name}: chips");
        for (chip, expected) in item.chips.iter().zip(chips) {
            assert_chip_matches(name, chip, expected);
        }
    }

    /// A chip's badges as the envelope writes them.
    fn badges(badges: &Badges) -> Value {
        let badge = |badge: &Option<Badge>| {
            badge.as_ref().map_or(Value::Null, |badge| {
                json!({
                    "text": badge.text,
                    "compact_text": badge.compact_text,
                    "tooltip": badge.tooltip,
                })
            })
        };
        json!({ "count": badge(&badges.count), "total": badge(&badges.total) })
    }

    fn assert_chip_matches(name: &str, chip: &ChipView, expected: &Value) {
        assert_eq!(chip.key, expected["key"], "{name}");
        assert_eq!(chip.name, expected["name"], "{name}: name");
        assert_eq!(chip.title, expected["title"], "{name}: title");
        assert_eq!(chip.description, expected["description"], "{name}");
        assert_eq!(json!(chip.problem), expected["problem"], "{name}");
        assert_eq!(tags(&chip.tags), expected["tags"], "{name}: tags");
        assert_eq!(
            tags(&chip.trailing_tags),
            expected["trailing_tags"],
            "{name}: trailing tags"
        );
        assert_eq!(json!(chip.details), expected["details"], "{name}: details");
        assert_eq!(
            json!(chip.effect.as_ref().map(|effect| &effect.label)),
            if expected["effect"].is_null() {
                Value::Null
            } else {
                expected["effect"]["label"].clone()
            },
            "{name}: effect"
        );
        assert_eq!(chip.uncursed, expected["uncursed"], "{name}");
        assert_eq!(chip.in_cluster, expected["in_cluster"], "{name}");
        // Every badge and stepper belongs to a chip, a cluster member's too,
        // and so do the badges its origin shows while one item is lifted.
        assert_eq!(badges(&chip.badges), expected["badges"], "{name}: badges");
        assert_eq!(
            chip.remaining_badges.as_ref().map_or(Value::Null, badges),
            expected["remaining_badges"],
            "{name}: remaining badges"
        );
        assert_eq!(chip.copies, keys(&expected["copies"]), "{name}: copies");
        let stack = &chip.stack;
        let shown = json!({
            "count": stack.count,
            "max": stack.max,
            "can_grow": stack.can_grow,
            "can_change_count": stack.can_change_count,
            "count_max": stack.count_max,
            "total": stack.total,
            "can_count_levels": stack.can_count_levels,
            "level_capacity": stack.level_capacity,
            "default_total": stack.default_total,
            "copy_depth": stack.copy_depth,
            "can_set_copy_depth": stack.can_set_copy_depth,
            "count_text": stack.count_text,
            "total_text": stack.total_text,
        });
        assert_eq!(shown, expected["stack"], "{name}: stack");
        assert_eq!(chip.can_detach, expected["can_detach"], "{name}");
        assert_eq!(chip.join, keys(&expected["join"]), "{name}: join");
        let refused: Vec<Value> = chip
            .refuse
            .iter()
            .map(|&(key, refusal)| {
                json!({ "key": key, "reason": refusal.name(), "message": refusal.to_string() })
            })
            .collect();
        assert_eq!(Value::Array(refused), expected["refuse"], "{name}: refuse");
    }

    #[test]
    fn the_typed_editor_gives_the_golden_board_answers_through_the_app_codec() {
        let replayed = fixtures("requirement_board");
        assert_eq!(replayed.len(), 40);
        for Fixture {
            name,
            request,
            response,
        } in replayed
        {
            let rows = decode_rows(&request["rows"]);
            let edits: Vec<Edit> = request["edits"]
                .as_array()
                .map(|edits| edits.iter().map(decode_edit).collect())
                .unwrap_or_default();
            let result = editor::apply(&rows, request["next_key"].as_u64(), &edits);
            let view = editor::board_view(&result.rows, decode_resin(&request["resin"]).as_ref());
            assert_matches(name, &result, &view, &response);
        }
    }

    #[test]
    fn fixture_rows_survive_the_app_codec() {
        for Fixture {
            name,
            request,
            response,
        } in fixtures("requirement_board")
        {
            // What the editor writes is what the app's codec writes, byte
            // for byte as JSON values…
            for entry in response["rows"].as_array().unwrap() {
                assert_eq!(&encode_row(&decode_row(entry)), entry, "{name}");
            }
            // …and a row the app reads comes back as the same requirement,
            // however its effects were listed.
            for entry in request["rows"].as_array().unwrap() {
                let row = decode_row(entry);
                assert_eq!(decode_row(&encode_row(&row)), row, "{name}");
            }
        }
    }

    fn ring(key: u64, upgrade: UpgradeRequirement) -> Row {
        Row {
            key,
            requirement: Requirement {
                item: Some(ItemId::RingMight),
                upgrade,
                ..Requirement::any(ItemKind::Ring)
            },
        }
    }

    fn any(key: u64, kind: ItemKind) -> Row {
        Row {
            key,
            requirement: Requirement::any(kind),
        }
    }

    #[test]
    fn the_board_is_folded_once_per_list() {
        let mut cache = BoardCache::default();
        assert!(cache.current().is_none());
        let rows = vec![any(1, ItemKind::Wand), any(2, ItemKind::Ring)];
        let first = cache.view(&rows, None);
        // The same list, as another refresh redraws it, is the same view.
        assert!(std::rc::Rc::ptr_eq(
            &first,
            &cache.view(&rows.clone(), None)
        ));
        assert!(std::rc::Rc::ptr_eq(&first, &cache.current().unwrap()));
        // A change of the rows or of the resin folds again.
        let fewer = cache.view(&rows[..1], None);
        assert_eq!(fewer.items.len(), 1);
        let resin = ResinState {
            amount: ResinAmount::Auto,
            filter: ArcaneResinFilter::default(),
        };
        let with_resin = cache.view(&rows[..1], Some(resin));
        assert!(!std::rc::Rc::ptr_eq(&fewer, &with_resin));
        let chip = with_resin.resin.as_ref().unwrap();
        assert_eq!(
            resin_tooltip(chip),
            format!("Arcane Resin\n{}", chip.details.join(" · "))
        );
    }

    #[test]
    fn drops_land_where_the_chips_say() {
        // A stacked ring, a wand, and a cluster of two armors; a blanket.
        let mut blanket = any(6, ItemKind::Weapon);
        blanket.requirement.blanket = true;
        let mut armors = [any(4, ItemKind::Armor), any(5, ItemKind::Armor)];
        for row in &mut armors {
            row.requirement.alternative_group = Some(1);
        }
        let rows = vec![
            ring(1, UpgradeRequirement::Exact(2)),
            ring(2, UpgradeRequirement::Any),
            any(3, ItemKind::Wand),
            armors[0],
            armors[1],
            blanket,
        ];
        let view = editor::board_view(&rows, None);
        let picked = |key| {
            let (item, chip) = find_chip(&view, key).unwrap();
            Dragged::new(&rows, item, chip)
        };

        // A drag moves one item, so the stacked ring may join the wand — one
        // ring goes, the other stays — and the wand may join the ring.
        let ring_chip = picked(1);
        assert_eq!(
            ring_chip.drop_answer(Landing::Row(3)),
            DropAnswer::Accept(Edit::Join {
                source: 1,
                target: 3
            })
        );
        let wand = picked(3);
        assert_eq!(
            wand.drop_answer(Landing::Row(1)).edit(),
            Some(Edit::Join {
                source: 3,
                target: 1
            })
        );
        // Its own chip, a hidden copy and the other section do nothing.
        for landing in [Landing::Row(3), Landing::Row(2), Landing::Row(6)] {
            assert_eq!(wand.drop_answer(landing), DropAnswer::Ignore);
        }
        // The empty board detaches a cluster member of its own section only;
        // a lone chip stays where it is.
        let member = picked(5);
        assert_eq!(
            member.drop_answer(Landing::Board { blanket: false }),
            DropAnswer::Accept(Edit::Detach { key: 5 })
        );
        assert_eq!(
            member.drop_answer(Landing::Board { blanket: true }),
            DropAnswer::Ignore
        );
        assert_eq!(
            wand.drop_answer(Landing::Board { blanket: false }),
            DropAnswer::Ignore
        );
        // The bin takes one item of anything: one ring of the stack.
        assert_eq!(
            ring_chip.drop_answer(Landing::Remove),
            DropAnswer::Accept(Edit::RemoveOne { key: 1 })
        );
        // Its own cluster is no target for a member.
        assert_eq!(member.drop_answer(Landing::Row(4)), DropAnswer::Ignore);
    }

    /// Wildcard stacks labelled `labels`, keyed from `first_key` on: each
    /// a +1 anchor and one plain copy, as a list holds a stack of a kind.
    fn stacks(first_key: u64, labels: std::ops::RangeInclusive<u8>) -> Vec<Row> {
        let kinds = [
            ItemKind::Wand,
            ItemKind::Armor,
            ItemKind::Ring,
            ItemKind::Weapon,
        ];
        let mut key = first_key;
        let mut rows = Vec::new();
        for label in labels {
            let kind = kinds[usize::from(label) % kinds.len()];
            for upgrade in [UpgradeRequirement::Exact(1), UpgradeRequirement::Any] {
                let mut row = any(key, kind);
                row.requirement.upgrade = upgrade;
                row.requirement.identity_group = Some(label);
                rows.push(row);
                key += 1;
            }
        }
        rows
    }

    fn wand(key: u64, item: ItemId) -> Row {
        Row {
            key,
            requirement: Requirement {
                item: Some(item),
                ..Requirement::any(ItemKind::Wand)
            },
        }
    }

    #[test]
    fn a_refused_drop_is_said_by_the_editor() {
        // Every stack label is in use. Frost ×2 would keep its stack as a
        // member of the either/or Disintegration makes with it, and that
        // member stack needs a label of its own.
        let mut state = AppState::default();
        state.requirements = stacks(1, 1..=4);
        state.requirements.extend([
            wand(9, ItemId::WandFrost),
            wand(10, ItemId::WandFrost),
            wand(11, ItemId::WandDisintegration),
        ]);
        let view = editor::board_view(&state.requirements, None);
        let picked = |key| {
            let (item, chip) = find_chip(&view, key).unwrap();
            Dragged::new(&state.requirements, item, chip)
        };
        let answer = picked(11).drop_answer(Landing::Row(9));
        assert_eq!(
            answer,
            DropAnswer::Refuse(Edit::Join {
                source: 11,
                target: 9
            })
        );
        // One Frost dragged onto Disintegration leaves a plain Frost behind
        // and joins as a plain member: no label needed.
        assert_eq!(
            picked(9).drop_answer(Landing::Row(11)),
            DropAnswer::Accept(Edit::Join {
                source: 9,
                target: 11
            })
        );
        let before = state.requirements.clone();
        let result = state.apply(&[answer.edit().unwrap()]);
        assert!(!result.changed);
        assert_eq!(
            result.refused.unwrap().to_string(),
            "Every group label is in use. Remove a stack or a combined level first."
        );
        assert_eq!(state.requirements, before);
    }

    #[test]
    fn the_bin_takes_one_item_or_says_why_it_cannot() {
        // {Frost ×3 | Disintegration ×3} under label 1, beside three stacks
        // holding the other labels, and a lone Frost ×2.
        let mut cluster = [
            wand(1, ItemId::WandFrost),
            wand(2, ItemId::WandDisintegration),
            any(3, ItemKind::Wand),
            any(4, ItemKind::Wand),
        ];
        for (index, row) in cluster.iter_mut().enumerate() {
            row.requirement.identity_group = Some(1);
            if index < 2 {
                row.requirement.alternative_group = Some(1);
            }
        }
        let mut state = AppState::default();
        state.requirements = cluster.to_vec();
        state.requirements.extend(stacks(5, 2..=4));
        state
            .requirements
            .extend([wand(11, ItemId::WandFrost), wand(12, ItemId::WandFrost)]);
        let view = editor::board_view(&state.requirements, None);
        let (_, frost) = find_chip(&view, 1).unwrap();
        assert_eq!(frost.badges.count.as_ref().unwrap().text, "×3");
        let picked = |key| {
            let (item, chip) = find_chip(&view, key).unwrap();
            Dragged::new(&state.requirements, item, chip)
        };
        let (member, lone) = (picked(1), picked(11));
        // One Frost less leaves Frost ×2 and Disintegration ×3, two member
        // stacks no longer alike, and no label is left for the second.
        let refused = member.drop_answer(Landing::Remove);
        assert_eq!(refused, DropAnswer::Refuse(Edit::RemoveOne { key: 1 }));
        let before = state.requirements.clone();
        let result = state.apply(&[refused.edit().unwrap()]);
        assert_eq!(result.refused.map(Refusal::name), Some("no_free_group"));
        assert_eq!(state.requirements, before);

        // The lone Frost ×2 gives up one Frost, not its whole stack.
        let accepted = lone.drop_answer(Landing::Remove);
        assert_eq!(accepted, DropAnswer::Accept(Edit::RemoveOne { key: 11 }));
        assert!(state.apply(&[accepted.edit().unwrap()]).changed);
        let view = editor::board_view(&state.requirements, None);
        let (_, frost) = find_chip(&view, 11).unwrap();
        assert_eq!((frost.stack.count, frost.badges.count.as_ref()), (1, None));
        assert_eq!(state.requirements.len(), before.len() - 1);
    }

    #[test]
    fn a_picked_up_member_carries_one_item() {
        // Frost picked up out of {Frost ×2 | Disintegration ×2} carries one
        // Frost: on the bin it takes one copy, and Disintegration keeps its
        // own two.
        let rows = alike_members();
        let mut cache = BoardCache::default();
        cache.view(&rows, None);
        let frost = cache.pick_up(1).unwrap();
        assert!(cache.pick_up(3).is_none(), "a hidden copy has no chip");
        let edit = frost.drop_answer(Landing::Remove).edit().unwrap();
        assert_eq!(edit, Edit::RemoveOne { key: 1 });
        let result = editor::apply(&rows, None, &[edit]);
        assert!(result.changed);
        assert_eq!(result.rows.len(), 3);
        let view = cache.view(&result.rows, None);
        let (_, frost) = find_chip(&view, 1).unwrap();
        let (_, disintegration) = find_chip(&view, 2).unwrap();
        assert!(shown_badges(frost).is_empty());
        assert_eq!(
            shown_badges(disintegration),
            [(StackField::Count, "\u{d7}2", 2, 3)]
        );
    }

    #[test]
    fn repaired_keys_are_adopted_and_followed() {
        // Linux keys every list 1…n, but the editor repairs any other list
        // alike: a zero key and a later duplicate take fresh keys, the first
        // row holding a key keeps it, and an edit naming the zero key follows
        // its row.
        let mut state = AppState::default();
        state.requirements = vec![
            any(0, ItemKind::Wand),
            any(4, ItemKind::Ring),
            any(4, ItemKind::Armor),
        ];
        let result = state.apply(&[Edit::SetCount { key: 0, count: 2 }]);
        assert!(result.changed);
        assert_eq!(result.rekeyed, [(0, 5), (4, 6)]);
        assert_eq!(result.focus, Some(5));
        let keys: Vec<u64> = state.requirements.iter().map(|row| row.key).collect();
        assert_eq!(keys, [5, 7, 4, 6]);
        assert_eq!(state.claim_key(), 8);
        // A stepper left open on the zero key follows its row; one on the
        // row that kept key 4 stays there, not on the duplicate now keyed 6.
        assert_eq!(follow_key(0, &result.rekeyed), 5);
        assert_eq!(follow_key(4, &result.rekeyed), 4);
        assert_eq!(follow_key(9, &result.rekeyed), 9);
    }

    #[test]
    fn the_menu_offers_every_entry_of_the_section_once() {
        let mut spears = [any(2, ItemKind::Weapon), any(3, ItemKind::Weapon)];
        spears[0].requirement.item = Some(ItemId::Spear);
        spears[1].requirement.item = Some(ItemId::Mace);
        for row in &mut spears {
            row.requirement.alternative_group = Some(1);
        }
        let mut blanket = any(5, ItemKind::Wand);
        blanket.requirement.blanket = true;
        let rows = vec![
            ring(1, UpgradeRequirement::Exact(2)),
            spears[0],
            spears[1],
            ring(4, UpgradeRequirement::Any),
            blanket,
            any(6, ItemKind::Wand),
        ];
        let view = editor::board_view(&rows, None);
        let (_, wand) = find_chip(&view, 6).unwrap();
        let labels: Vec<(u64, String)> = join_choices(&view, wand)
            .into_iter()
            .map(|choice| (choice.target, choice.label))
            .collect();
        // The ring stack refuses the wand, but is offered so the editor can
        // say why; the cluster is one choice; the blanket is not offered.
        assert_eq!(
            labels,
            [
                (1, "Ring of Might".to_owned()),
                (2, "Spear or Mace".to_owned())
            ]
        );
        let (_, spear) = find_chip(&view, 2).unwrap();
        assert!(
            join_choices(&view, spear)
                .iter()
                .all(|choice| choice.target != 3)
        );
    }

    #[test]
    fn steppers_and_tooltips_read_the_stack_and_the_chip() {
        let rows = vec![
            ring(1, UpgradeRequirement::Exact(2)),
            ring(2, UpgradeRequirement::Any),
        ];
        let view = editor::board_view(&rows, None);
        let (_, chip) = find_chip(&view, 1).unwrap();
        assert_eq!(chip.stack.count_max, 3);
        assert_eq!(
            chip_tooltip(chip),
            "Ring of Might\nexactly +2\n× 2 of the same kind — the extra copies: any upgrade, any floor"
        );
        // In a cluster spanning two families every member still has its own
        // stack, of its own kind: the wand may grow while the ring does not.
        let mut cluster = vec![any(1, ItemKind::Wand), any(2, ItemKind::Ring)];
        for row in &mut cluster {
            row.requirement.alternative_group = Some(1);
        }
        let view = editor::board_view(&cluster, None);
        let (_, chip) = find_chip(&view, 1).unwrap();
        assert!(chip.stack.can_grow);
        assert_eq!(chip.stack.count_max, 3);
        assert_eq!(chip_tooltip(chip), "Any wand\nany upgrade\nor Any ring");
        let grown = editor::apply(&cluster, None, &[Edit::SetCount { key: 1, count: 2 }]);
        let view = editor::board_view(&grown.rows, None);
        let (_, wand) = find_chip(&view, 1).unwrap();
        let (_, ring) = find_chip(&view, 2).unwrap();
        assert_eq!((wand.stack.count, ring.stack.count), (2, 1));
        assert!(ring.badges.count.is_none());
        assert_eq!(
            chip_tooltip(wand),
            "Any wand\nany upgrade\nor Any ring\n× 2 of the same kind — the extra copies: any upgrade, any floor"
        );
    }

    /// {Frost ×2 | Disintegration ×2}: both members share label 1.
    fn alike_members() -> Vec<Row> {
        let mut rows = vec![
            wand(1, ItemId::WandFrost),
            wand(2, ItemId::WandDisintegration),
            any(3, ItemKind::Wand),
        ];
        for (index, row) in rows.iter_mut().enumerate() {
            row.requirement.identity_group = Some(1);
            if index < 2 {
                row.requirement.alternative_group = Some(1);
            }
        }
        rows
    }

    /// What `chip`'s badges show and their steppers edit.
    fn shown_badges(chip: &ChipView) -> Vec<(StackField, &str, u8, u8)> {
        stack_badges(chip)
            .into_iter()
            .map(|shown| {
                (
                    shown.field,
                    shown.badge.text.as_str(),
                    shown.value,
                    shown.maximum,
                )
            })
            .collect()
    }

    #[test]
    fn every_chip_wears_its_own_stack() {
        // Members alike each wear ×2; the cluster itself wears nothing.
        let view = editor::board_view(&alike_members(), None);
        assert_eq!(view.items.len(), 1);
        for key in [1, 2] {
            let (_, member) = find_chip(&view, key).unwrap();
            assert_eq!(shown_badges(member), [(StackField::Count, "\u{d7}2", 2, 3)]);
            assert_eq!(member.copies, [3]);
        }

        // A lone ring stack counting levels wears ≤2 and Σ ≥ 4; the Σ
        // stepper starts at the total and stops at what two rings reach.
        let mut rings = vec![
            ring(1, UpgradeRequirement::Any),
            ring(2, UpgradeRequirement::Any),
        ];
        for row in &mut rings {
            row.requirement.level_sum = Some(LevelSum {
                group: 1,
                minimum_total: 4,
            });
        }
        let view = editor::board_view(&rings, None);
        let (_, chip) = find_chip(&view, 1).unwrap();
        assert_eq!(
            shown_badges(chip),
            [
                (StackField::Count, "\u{2264}2", 2, 3),
                (StackField::Total, "\u{3a3} \u{2265} 4", 4, 8)
            ]
        );
    }

    /// What `chip`'s origin shows while one of its items is lifted away.
    fn shown_left_behind(chip: &ChipView) -> Option<Vec<(StackField, &str)>> {
        left_behind(chip).map(|badges| {
            badges
                .into_iter()
                .map(|(field, badge)| (field, badge.text.as_str()))
                .collect()
        })
    }

    #[test]
    fn a_lifted_item_leaves_its_stack_one_item_fewer() {
        // Ring of Energy +4 ×3 leaves ×2 behind, and ×2 leaves one ring,
        // which wears no badge.
        let energy = |count: u64| -> Vec<Row> {
            (1..=count)
                .map(|key| Row {
                    key,
                    requirement: Requirement {
                        item: Some(ItemId::RingEnergy),
                        upgrade: if key == 1 {
                            UpgradeRequirement::Exact(4)
                        } else {
                            UpgradeRequirement::Any
                        },
                        ..Requirement::any(ItemKind::Ring)
                    },
                })
                .collect()
        };
        let view = editor::board_view(&energy(3), None);
        let (_, chip) = find_chip(&view, 1).unwrap();
        assert_eq!(shown_badges(chip), [(StackField::Count, "\u{d7}3", 3, 3)]);
        assert_eq!(
            shown_left_behind(chip),
            Some(vec![(StackField::Count, "\u{d7}2")])
        );
        let view = editor::board_view(&energy(2), None);
        let (_, chip) = find_chip(&view, 1).unwrap();
        assert_eq!(shown_left_behind(chip), Some(Vec::new()));

        // A combined level the rest cannot reach is capped at what it can.
        let mut rings = vec![
            ring(1, UpgradeRequirement::Any),
            ring(2, UpgradeRequirement::Any),
            ring(3, UpgradeRequirement::Any),
        ];
        for row in &mut rings {
            row.requirement.level_sum = Some(LevelSum {
                group: 1,
                minimum_total: 11,
            });
        }
        let view = editor::board_view(&rings, None);
        let (_, chip) = find_chip(&view, 1).unwrap();
        assert_eq!(
            shown_left_behind(chip),
            Some(vec![
                (StackField::Count, "\u{2264}2"),
                (StackField::Total, "\u{3a3} \u{2265} 8")
            ])
        );

        // In {Frost ×2 | Disintegration} a lifted Frost leaves one Frost,
        // while Disintegration has no copies and leaves whole.
        let mut cluster = vec![
            wand(1, ItemId::WandFrost),
            wand(2, ItemId::WandDisintegration),
            any(3, ItemKind::Wand),
        ];
        for (key, row) in (1..).zip(&mut cluster) {
            if key != 2 {
                row.requirement.identity_group = Some(1);
            }
            if key != 3 {
                row.requirement.alternative_group = Some(1);
            }
        }
        let view = editor::board_view(&cluster, None);
        let (_, frost) = find_chip(&view, 1).unwrap();
        let (_, disintegration) = find_chip(&view, 2).unwrap();
        assert_eq!(shown_badges(frost), [(StackField::Count, "\u{d7}2", 2, 3)]);
        assert_eq!(shown_left_behind(frost), Some(Vec::new()));
        assert_eq!(shown_left_behind(disintegration), None);
    }

    #[test]
    fn the_resin_chip_tints_its_credit_and_explains_it_on_hover() {
        let resin = ResinState {
            amount: ResinAmount::Auto,
            filter: ArcaneResinFilter {
                include_mage_wand: true,
                max_depth: Some(9),
                ..ArcaneResinFilter::default()
            },
        };
        let view = editor::board_view(&[], Some(&resin));
        let chip = view.resin.as_ref().unwrap();
        let drawn: Vec<(&str, &str, Option<&str>)> = chip
            .tags
            .iter()
            .map(|tag| {
                (
                    tag.text.as_str(),
                    tag_class(tag.style),
                    tag.tooltip.as_deref(),
                )
            })
            .collect();
        // The resin the chip counts wears the success colour, as Linux drew
        // the amount and the Mage tag before the shared editor; the donor
        // floor stays a plain filter.
        assert_eq!(
            drawn,
            [
                (
                    labels::RESIN_AUTO,
                    "chip-tag-credit",
                    Some(labels::RESIN_AUTO_TOOLTIP)
                ),
                (
                    labels::RESIN_MAGE_TAG,
                    "chip-tag-credit",
                    Some(labels::RESIN_MAGE_TOOLTIP)
                ),
                ("F\u{2264}9", "chip-tag-plain", None),
            ]
        );
        // A fixed amount needs no explaining; a chip's tags never do.
        let fixed = ResinState {
            amount: ResinAmount::AtLeast(4),
            ..resin
        };
        let view = editor::board_view(&[ring(1, UpgradeRequirement::Exact(2))], Some(&fixed));
        assert_eq!(view.resin.as_ref().unwrap().tags[0].tooltip, None);
        let (_, ring_chip) = find_chip(&view, 1).unwrap();
        assert_eq!(ring_chip.tags[0].style, TagStyle::Upgrade);
        assert_eq!(tag_class(ring_chip.tags[0].style), "chip-tag-up");
        assert!(ring_chip.tags.iter().all(|tag| tag.tooltip.is_none()));
    }
}
