//! The requirement editor's rules, shared by every frontend.
//!
//! Each app used to carry its own port of the web board: how a flat
//! requirement list collapses into chips, either/or clusters and stacks,
//! what a drag or a badge tap writes back, and how a stack's copies are
//! encoded. The ports drifted apart in ways no parity test caught, so the
//! rules live here once. Linux calls this typed surface directly; the other
//! platforms reach it through JSON envelopes that only project it.
//!
//! # Rows
//!
//! The editor works on one flat list of [`Row`]s holding both board
//! sections (a row's [`Requirement::blanket`] says which). Every row carries
//! a platform key, stable across edits, so a board gesture can name the chip
//! it acted on however the list around it moved. Keys are integers in
//! `1..=`[`MAX_KEY`] — the largest integer a JavaScript number holds exactly
//! — and the editor repairs a list that breaks that rule (see [`apply`]).
//!
//! # Relationships
//!
//! Three relationships of the query model show on the board:
//!
//! - an *either/or cluster* is several rows sharing an
//!   [`Requirement::alternative_group`]: one slot, any member fills it;
//! - a *stack* is a chip (or a whole cluster) asking for more than one item
//!   of the same kind — the blacksmith's reforge fodder. Its extra copies
//!   never carry constraints of their own. A stack of a concrete item
//!   encodes as plain repeated requirements; a wildcard or cluster stack
//!   encodes as bare copies tied to the anchor with an
//!   [`Requirement::identity_group`];
//! - a stack with a *combined level* encodes as identical members sharing a
//!   [`Requirement::level_sum`]: each matched item counts upgrade + 1
//!   towards the total, and members are optional, so the stack reads "up to
//!   N items reaching T levels".
//!
//! [`board_items`] folds the list into that view; [`apply`] runs the edits.
//!
//! # Presentation
//!
//! [`board_view`] renders the fold into everything a board draws — entries,
//! chips with their tags and popover text, badges, join candidates, the
//! Arcane Resin chip — and [`problems()`] says what is wrong with the list,
//! row by row and between rows, in the words every platform shows.
//!
//! # The requirement sheet
//!
//! A chip opens into a sheet held as a [`Draft`]: [`open`] builds it from
//! the rows, [`change`] applies one control the user moved, [`form`] says
//! what the sheet shows — every control, the preview chip, the errors — and
//! [`save`] writes it back as an [`Edit::Save`] (or as the query's Arcane
//! Resin), refusing a save that would newly break the list.

mod board;
mod chips;
mod draft;
pub mod labels;
mod problems;
mod stack;

use std::collections::BTreeSet;

use crate::main_world::EMPTY_BOSS_FLOORS;
use crate::query::Requirement;

pub(crate) use board::is_plain_item_copy;
pub use board::{
    BoardItem, DropAction, DropTarget, Edit, EditResult, ItemKey, JoinCandidates, Refusal, apply,
    board_items, drop_action, join_candidates,
};
pub use chips::{
    Badge, Badges, BoardView, ChipView, Counts, EffectBadge, ItemView, Relation, RelationGlyph,
    ResinAmount, ResinChip, ResinState, Tag, TagStyle, board_view, chip_description, chip_details,
    chip_tags, chip_trailing_tags, effect_badge, resin_chip,
};
pub use draft::{
    Change, Choice, DRAFT_VERSION, DUPLICATE_TRINKET, Draft, EffectChoice, EffectControl,
    EffectGroup, EffectMode, FloorToggle, Form, FormMode, ItemChoice, ModeRange, Opt, Origin,
    RESIN_AMOUNT_RANGE, RangeToggle, ResinControl, ResinDraft, ResinOutcome, SaveResult,
    StackControl, TierMode, Toggle, UpgradeMode, change, form, open, save,
};
pub use labels::KindName;
pub use problems::{NO_ORDINARY_REQUIREMENT, Problem, ProblemScope, problems, row_problems};
pub use stack::{
    StackView, can_change_count, can_count_levels, can_grow, can_set_copy_depth, copy_depth,
    default_total, level_capacity, stack_view,
};

/// The most items one chip or cluster may ask for, its anchor included.
/// Beyond three the board stops reading as one thing, and no reforge chain
/// needs more. The matcher's Auto resin rule, which exempts a stack's
/// reforge copies from resin upgrades, reads stacks with the same bound.
pub const STACK_MAX: u8 = 3;

/// The largest row key: 2^53 − 1, the largest integer every platform — the
/// web's JavaScript numbers included — represents exactly.
pub const MAX_KEY: u64 = (1 << 53) - 1;

/// One requirement on the board, named by a platform key that survives
/// every edit.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct Row {
    /// Unique within the list, in `1..=`[`MAX_KEY`].
    pub key: u64,
    pub requirement: Requirement,
}

impl AsRef<Requirement> for Row {
    fn as_ref(&self) -> &Requirement {
        &self.requirement
    }
}

// The board fold reads plain query lists too (the matcher's Auto resin rule
// and its differential test), and std has no reflexive `AsRef`.
impl AsRef<Requirement> for Requirement {
    fn as_ref(&self) -> &Requirement {
        self
    }
}

/// Where a floor-limit control lands when the user moves it onto an empty
/// boss floor ([`EMPTY_BOSS_FLOORS`]). A single upward step (a stepper, an
/// arrow key, a scroll tick) continues to the next real floor, so the
/// control never sticks below the boss; every other move — single steps
/// down and typed jumps in either direction — snaps to the equivalent floor
/// below, matching [`crate::main_world::normalize_floor_limit`]. Typing "10"
/// therefore means "the first 10 floors" (≡ 9), never 11.
#[must_use]
pub fn skip_boss_floor(previous: u8, requested: u8) -> u8 {
    if !EMPTY_BOSS_FLOORS.contains(&requested) {
        requested
    } else if requested == previous.saturating_add(1) {
        requested + 1
    } else {
        requested - 1
    }
}

/// Squeezes alternative-group labels as a platform stored them into the
/// `u8` a [`Requirement`] carries. Android numbers groups with unbounded
/// integers and never compacts them, so a long session can pass 255; when
/// any label does, every group is relabelled 1, 2, … in first-appearance
/// order (which keeps each group's members together and changes nothing a
/// search sees). Returns the labels and whether any changed. Groups past the
/// 255th distinct one — more clusters than a board can show — lose their
/// label.
#[must_use]
pub fn compact_alternative_labels(labels: &[Option<u64>]) -> (Vec<Option<u8>>, bool) {
    if labels
        .iter()
        .flatten()
        .all(|&label| u8::try_from(label).is_ok())
    {
        let kept = labels
            .iter()
            .map(|label| label.and_then(|label| u8::try_from(label).ok()))
            .collect();
        return (kept, false);
    }
    let mut seen: Vec<u64> = Vec::new();
    let relabelled = labels
        .iter()
        .map(|label| {
            let label = (*label)?;
            let position = seen
                .iter()
                .position(|&known| known == label)
                .unwrap_or_else(|| {
                    seen.push(label);
                    seen.len() - 1
                });
            u8::try_from(position + 1).ok()
        })
        .collect();
    (relabelled, true)
}

/// The response's `next_key`: one past the largest key in use, or the
/// caller's hint when that is larger (a platform that pre-claims keys, as
/// Linux does, keeps its counter moving forward).
pub(crate) fn next_key(rows: &[Row], hint: Option<u64>) -> u64 {
    let largest = rows.iter().map(|row| row.key).max().unwrap_or(0);
    largest
        .saturating_add(1)
        .max(hint.unwrap_or(0).min(MAX_KEY + 1))
}

/// A fresh key for a new row: [`next_key`], or — once that would pass
/// [`MAX_KEY`] — the smallest key nothing uses.
pub(crate) fn mint_key(rows: &[Row], hint: Option<u64>) -> u64 {
    let candidate = next_key(rows, hint);
    if candidate <= MAX_KEY {
        return candidate;
    }
    let used: BTreeSet<u64> = rows.iter().map(|row| row.key).collect();
    (1..=MAX_KEY)
        .find(|key| !used.contains(key))
        .expect("a list shorter than 2^53 rows leaves a key free")
}

/// Whether a key is one the editor hands out.
pub(crate) const fn is_valid_key(key: u64) -> bool {
    key >= 1 && key <= MAX_KEY
}

/// Re-keys the rows whose key is zero, out of range, or a later duplicate,
/// in list order, minting as [`mint_key`] does. Returns the repaired list and
/// every `(old, new)` pair; the first occurrence of a key always keeps it.
pub(crate) fn repair_keys(rows: &[Row], hint: Option<u64>) -> (Vec<Row>, Vec<(u64, u64)>) {
    let mut seen = BTreeSet::new();
    let broken: Vec<usize> = rows
        .iter()
        .enumerate()
        .filter(|(_, row)| !(is_valid_key(row.key) && seen.insert(row.key)))
        .map(|(index, _)| index)
        .collect();
    let mut repaired = rows.to_vec();
    if broken.is_empty() {
        return (repaired, Vec::new());
    }
    // Mint against the keys that stay, so a repaired row never takes a key
    // a later valid row still holds.
    let mut kept: Vec<Row> = rows
        .iter()
        .enumerate()
        .filter(|(index, _)| !broken.contains(index))
        .map(|(_, row)| *row)
        .collect();
    let mut rekeyed = Vec::with_capacity(broken.len());
    for index in broken {
        let key = mint_key(&kept, hint);
        rekeyed.push((repaired[index].key, key));
        repaired[index].key = key;
        kept.push(repaired[index]);
    }
    (repaired, rekeyed)
}

#[cfg(test)]
pub(crate) mod testing;

#[cfg(test)]
mod tests {
    use super::{MAX_KEY, Row, compact_alternative_labels, mint_key, next_key, repair_keys};
    use super::{skip_boss_floor, testing::row};
    use crate::catalog::ItemKind;

    #[test]
    fn single_upward_steps_skip_forward_and_everything_else_snaps_down() {
        // Stepping up from the floor below an empty boss floor lands above it.
        assert_eq!(skip_boss_floor(4, 5), 6);
        assert_eq!(skip_boss_floor(9, 10), 11);
        assert_eq!(skip_boss_floor(14, 15), 16);
        // Stepping down lands on the equivalent floor below.
        assert_eq!(skip_boss_floor(6, 5), 4);
        assert_eq!(skip_boss_floor(11, 10), 9);
        assert_eq!(skip_boss_floor(16, 15), 14);
        // Typed jumps snap down: "10" means the first 10 floors (≡ 9), never 11.
        assert_eq!(skip_boss_floor(4, 10), 9);
        assert_eq!(skip_boss_floor(24, 15), 14);
        assert_eq!(skip_boss_floor(4, 15), 14);
        assert_eq!(skip_boss_floor(20, 5), 4);
        // Non-boss floors pass through untouched, floor 20 included.
        assert_eq!(skip_boss_floor(4, 6), 6);
        assert_eq!(skip_boss_floor(24, 1), 1);
        assert_eq!(skip_boss_floor(1, 24), 24);
        assert_eq!(skip_boss_floor(19, 20), 20);
        // A previous value at the top of the range cannot overflow.
        assert_eq!(skip_boss_floor(u8::MAX, 5), 4);
    }

    #[test]
    fn keys_mint_past_the_largest_key_or_the_hint() {
        let rows = [row(3, ItemKind::Wand), row(7, ItemKind::Ring)];
        assert_eq!(next_key(&rows, None), 8);
        assert_eq!(mint_key(&rows, None), 8);
        // A platform that pre-claimed keys keeps its counter.
        assert_eq!(next_key(&rows, Some(20)), 20);
        assert_eq!(mint_key(&rows, Some(20)), 20);
        // A stale hint never mints a key already in use.
        assert_eq!(mint_key(&rows, Some(2)), 8);
        assert_eq!(mint_key(&rows, Some(0)), 8);
        assert_eq!(mint_key(&[], None), 1);
        // At the top of the key space the smallest free key is reused.
        let full = [row(1, ItemKind::Wand), row(MAX_KEY, ItemKind::Wand)];
        assert_eq!(mint_key(&full, None), 2);
        assert_eq!(mint_key(&[row(1, ItemKind::Wand)], Some(u64::MAX)), 2);
        assert_eq!(next_key(&[], Some(u64::MAX)), MAX_KEY + 1);
    }

    #[test]
    fn broken_keys_are_repaired_in_list_order_and_reported() {
        let rows = [
            row(0, ItemKind::Wand),
            row(4, ItemKind::Ring),
            row(4, ItemKind::Armor),
            row(MAX_KEY + 1, ItemKind::Weapon),
            row(2, ItemKind::Wand),
        ];
        let (repaired, rekeyed) = repair_keys(&rows, None);
        assert_eq!(
            repaired.iter().map(|row| row.key).collect::<Vec<_>>(),
            [5, 4, 6, 7, 2]
        );
        assert_eq!(rekeyed, [(0, 5), (4, 6), (MAX_KEY + 1, 7)]);
        // The requirements themselves are untouched.
        for (before, after) in rows.iter().zip(&repaired) {
            assert_eq!(before.requirement, after.requirement);
        }
        // A sound list comes back as it was.
        let sound: Vec<Row> = repaired.clone();
        assert_eq!(repair_keys(&sound, None), (sound.clone(), Vec::new()));
        // Repairs respect a pre-claimed counter.
        let (_, rekeyed) = repair_keys(&[row(0, ItemKind::Wand)], Some(9));
        assert_eq!(rekeyed, [(0, 9)]);
    }

    #[test]
    fn wide_alternative_labels_are_relabelled_in_first_appearance_order() {
        let narrow = [Some(3), None, Some(3), Some(255)];
        assert_eq!(
            compact_alternative_labels(&narrow),
            (vec![Some(3), None, Some(3), Some(255)], false)
        );
        let wide = [Some(900), None, Some(4), Some(900), Some(4), Some(256)];
        assert_eq!(
            compact_alternative_labels(&wide),
            (
                vec![Some(1), None, Some(2), Some(1), Some(2), Some(3)],
                true
            )
        );
    }
}
