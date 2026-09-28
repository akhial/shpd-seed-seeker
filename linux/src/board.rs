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
    self, BoardView, ChipView, Edit, ItemView, RelationGlyph, ResinChip, ResinState, Row, StackView,
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
    /// On the bin.
    Remove,
}

/// What releasing a dragged chip somewhere does.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DropAnswer {
    /// The drop sends this edit; the target lights up.
    Accept(Edit),
    /// The editor refuses this join. The target is marked as refusing, and
    /// the drop still sends the join so the editor's refusal is said.
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
/// when the drag begins, so hovering asks the editor nothing.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Dragged {
    pub key: u64,
    blanket: bool,
    can_detach: bool,
    join: Vec<u64>,
    refuse: Vec<u64>,
}

impl Dragged {
    /// The chip `chip` of the board entry `item`, picked up.
    #[must_use]
    pub fn new(item: &ItemView, chip: &ChipView) -> Self {
        Self {
            key: chip.key,
            blanket: item.blanket,
            can_detach: chip.can_detach,
            join: chip.join.clone(),
            refuse: chip.refuse.iter().map(|&(key, _)| key).collect(),
        }
    }

    /// What releasing the chip on `landing` does: onto a row it joins when
    /// the editor lists the row among the chip's joins, and is refused when
    /// it lists it among its refusals; onto the empty board of its own
    /// section a cluster member leaves its cluster while a lone chip stays
    /// where it is; onto the bin it is removed.
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
            Landing::Remove => DropAnswer::Accept(Edit::Remove { key: source }),
            Landing::Row(_) | Landing::Board { .. } => DropAnswer::Ignore,
        }
    }
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
/// order, a cluster named by its members (`Spear or Mace`).
#[must_use]
pub fn join_choices(view: &BoardView, chip: &ChipView) -> Vec<JoinChoice> {
    view.items
        .iter()
        .filter_map(|item| {
            let target = item.members.iter().copied().find(|key| {
                chip.join.contains(key) || chip.refuse.iter().any(|(refused, _)| refused == key)
            })?;
            let names: Vec<&str> = item.chips.iter().map(|chip| chip.name.as_str()).collect();
            Some(JoinChoice {
                target,
                label: names.join(" or "),
            })
        })
        .collect()
}

/// The most items the count stepper and the "How many" menu offer: the
/// stack's bound while the entry can grow, else only the copies it may shed.
#[must_use]
pub const fn count_limit(stack: &StackView) -> u8 {
    if stack.can_grow {
        stack.max
    } else {
        stack.count
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
    use shpd_seedfinder_core::catalog::{ItemId, ItemKind};
    use shpd_seedfinder_core::editor::{self, Edit, ResinAmount, ResinState, Row};
    use shpd_seedfinder_core::query::{ArcaneResinFilter, Requirement, UpgradeRequirement};

    use super::{
        BoardCache, Dragged, DropAnswer, Landing, chip_tooltip, count_limit, find_chip,
        join_choices, resin_tooltip,
    };
    use crate::state::AppState;

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
            Dragged::new(item, chip)
        };

        // The stacked ring cannot join the wand; the wand may join the ring.
        let ring_chip = picked(1);
        assert_eq!(
            ring_chip.drop_answer(Landing::Row(3)),
            DropAnswer::Refuse(Edit::Join {
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
        // The bin takes anything.
        assert_eq!(
            ring_chip.drop_answer(Landing::Remove),
            DropAnswer::Accept(Edit::Remove { key: 1 })
        );
        // Its own cluster is no target for a member.
        assert_eq!(member.drop_answer(Landing::Row(4)), DropAnswer::Ignore);
    }

    #[test]
    fn a_refused_drop_is_said_by_the_editor() {
        let mut state = AppState::default();
        state.requirements = vec![
            ring(1, UpgradeRequirement::Exact(2)),
            ring(2, UpgradeRequirement::Any),
            any(3, ItemKind::Wand),
        ];
        let view = editor::board_view(&state.requirements, None);
        let (item, chip) = find_chip(&view, 1).unwrap();
        let edit = Dragged::new(item, chip)
            .drop_answer(Landing::Row(3))
            .edit()
            .unwrap();
        let before = state.requirements.clone();
        let result = state.apply(&[edit]);
        assert!(!result.changed);
        assert_eq!(
            result.refused.unwrap().to_string(),
            "Copies can only be grouped with the same item type."
        );
        assert_eq!(state.requirements, before);
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
        let (item, chip) = find_chip(&view, 1).unwrap();
        assert_eq!(count_limit(&item.stack), 3);
        assert_eq!(
            chip_tooltip(chip),
            "Ring of Might\nexactly +2\n× 2 of the same kind — the extra copies: any upgrade, any floor"
        );
        // A cluster spanning two families sheds copies but grows none.
        let mut cluster = vec![any(1, ItemKind::Wand), any(2, ItemKind::Ring)];
        for row in &mut cluster {
            row.requirement.alternative_group = Some(1);
        }
        let view = editor::board_view(&cluster, None);
        let (item, chip) = find_chip(&view, 1).unwrap();
        assert!(!item.stack.can_grow);
        assert_eq!(count_limit(&item.stack), 1);
        assert_eq!(chip_tooltip(chip), "Any wand\nany upgrade\nor Any ring");
    }
}
