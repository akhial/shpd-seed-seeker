//! The board: the collapsed view of the flat row list, and every edit a
//! board gesture or a requirement-sheet save writes back.
//!
//! The fold and the edits follow the web design's `relations.ts`, with the
//! join policy Android settled on in #190 and the fixes listed on the
//! individual edits. Every effective edit ends in [`normalize`], so a deleted
//! anchor can never leave stale groups behind.

use std::collections::{BTreeMap, BTreeSet, HashMap, VecDeque};
use std::fmt;

use crate::catalog::{ItemId, ItemKind};
use crate::main_world::normalize_floor_limit;
use crate::query::{
    EffectRequirement, LevelSum, MAX_IDENTITY_GROUP, MAX_LEVEL_SUM_GROUP, MAX_SEARCH_DEPTH,
    Requirement, TierRequirement, UpgradeRequirement,
};

use super::stack::{
    can_count_levels, can_grow, copy_depth as stack_copy_depth, default_total, level_capacity,
};
use super::{Row, STACK_MAX, is_valid_key, mint_key, next_key, redirects, repair_keys};

/// A board entry's identity, stable while the entry survives an edit: a chip
/// is named by its anchor row's key, a cluster by its alternative group.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ItemKey {
    Chip(u64),
    Cluster(u8),
}

impl fmt::Display for ItemKey {
    /// The envelopes' string form: `r17` for a chip, `c3` for a cluster.
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Chip(key) => write!(formatter, "r{key}"),
            Self::Cluster(group) => write!(formatter, "c{group}"),
        }
    }
}

/// One board entry: a chip, or an either/or cluster of chips, with the
/// hidden copies behind its stack badge. Indices point into the list the
/// entry was folded from.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BoardItem {
    /// Visible row indices: one for a chip, every member for a cluster.
    pub members: Vec<usize>,
    /// The alternative group of a cluster of two or more; a lone
    /// alternative renders as a plain chip.
    pub cluster: Option<u8>,
    /// Hidden copy indices behind the stack badge, in list order.
    pub extras: Vec<usize>,
    /// The stack's combined level, when one is set.
    pub total: Option<u8>,
}

impl BoardItem {
    /// The row the badges and the editor act on: the first member.
    #[must_use]
    pub fn anchor(&self) -> usize {
        self.members[0]
    }

    /// How many items this asks for: its anchor plus the hidden copies.
    #[must_use]
    pub fn count(&self) -> usize {
        1 + self.extras.len()
    }

    /// The entry's identity among `rows`, the list it was folded from.
    #[must_use]
    pub fn key(&self, rows: &[Row]) -> ItemKey {
        match self.cluster {
            Some(group) => ItemKey::Cluster(group),
            None => ItemKey::Chip(rows[self.anchor()].key),
        }
    }
}

/// Whether `copy` is a plain repeat of the named `item`: the same item and
/// nothing else, so it folds into the stack of an earlier chip naming it.
///
/// The fields are checked one by one rather than through
/// [`Requirement::is_bare`]: a narrowed melee/thrown stack keeps its
/// narrowing on every repeat and must still fold, and a floor limit is a
/// placement bound, not an item property, so a repeat carrying one folds
/// too. Trinkets and artifacts never stack, so their repeats stay chips.
pub(crate) fn is_plain_item_copy(copy: &Requirement, item: ItemId) -> bool {
    !copy.blanket
        && !matches!(copy.kind, ItemKind::Trinket | ItemKind::Artifact)
        && copy.item == Some(item)
        && copy.tier == TierRequirement::Any
        && copy.upgrade == UpgradeRequirement::Any
        && copy.effect == EffectRequirement::Any
        && !copy.require_uncursed
        && !copy.exclude_resin
        && copy.source.is_none()
        && copy.identity_group.is_none()
        && copy.alternative_group.is_none()
        && copy.level_sum.is_none()
}

/// One combined-level group as the board reads it: the first member
/// anchors, the rest fold into its badge.
struct SumAnchor {
    anchor: usize,
    extras: Vec<usize>,
    total: u8,
}

/// The board's collapsed view of the flat list: clusters group
/// alternatives, and a stack's copies fold into their anchor's badge.
///
/// Three folds, as on the web: a combined-level group folds into its first
/// member; an identity group folds its lone bare copies into the one
/// constrained unit (a requirement, or a whole cluster) — a group with two
/// constrained units cannot collapse, and validation reports it; and a
/// plain repeat of a named item folds into the nearest earlier chip naming
/// it, up to [`STACK_MAX`] items. A cluster of one renders as a chip.
///
/// Every row of any list is a member or a hidden copy of exactly one entry,
/// so nothing the list holds is out of sight or out of reach of a removal.
#[must_use]
#[allow(clippy::too_many_lines)] // Three collapses read best in one pass.
pub fn board_items<R: AsRef<Requirement>>(rows: &[R]) -> Vec<BoardItem> {
    let requirement = |index: usize| rows[index].as_ref();
    let mut hidden = vec![false; rows.len()];

    // Combined-level groups: the first member anchors, the rest fold away.
    let mut sums: BTreeMap<u8, SumAnchor> = BTreeMap::new();
    for index in 0..rows.len() {
        let Some(sum) = requirement(index).level_sum else {
            continue;
        };
        sums.entry(sum.group)
            .and_modify(|group| group.extras.push(index))
            .or_insert_with(|| SumAnchor {
                anchor: index,
                extras: Vec::new(),
                total: sum.minimum_total,
            });
    }
    for group in sums.values() {
        for &index in &group.extras {
            hidden[index] = true;
        }
    }

    // Identity stacks: bare copies fold into the constrained unit, or into
    // the first member when every member is bare.
    //
    // Only a hand-written list puts one row in a stack and a combined level
    // at once. The two folds must then never hide a row no entry shows — an
    // entry whose anchor is folded away never forms — so a stack whose
    // anchor a combined level folded leaves its copies on the board as
    // chips, and no row of a combined level, its anchor included, folds into
    // a stack. The web lost such rows from every entry.
    let mut identity_extras: HashMap<usize, Vec<usize>> = HashMap::new();
    for members in identity_groups(rows).values() {
        let constrained: Vec<usize> = members
            .iter()
            .copied()
            .filter(|&index| !requirement(index).is_bare())
            .collect();
        let units: BTreeSet<(Option<u8>, usize)> = constrained
            .iter()
            .map(|&index| {
                requirement(index)
                    .alternative_group
                    .map_or((None, index), |group| (Some(group), 0))
            })
            .collect();
        if units.len() > 1 {
            continue;
        }
        let anchor = constrained.first().copied().unwrap_or(members[0]);
        if hidden[anchor] {
            continue;
        }
        // A cluster anchor labels every member; fold only the lone bare copies.
        // A copy a combined level already folded stays with that stack (the
        // web listed it twice, so a hand-written list could count it twice).
        let extras: Vec<usize> = members
            .iter()
            .copied()
            .filter(|&index| {
                index != anchor
                    && !hidden[index]
                    && requirement(index).alternative_group.is_none()
                    && requirement(index).level_sum.is_none()
                    && requirement(index).is_bare()
            })
            .collect();
        if extras.is_empty() {
            continue;
        }
        for &index in &extras {
            hidden[index] = true;
        }
        identity_extras.insert(anchor, extras);
    }

    let attach = |item: &mut BoardItem, anchor: usize| {
        if let Some(sum) = requirement(anchor).level_sum
            && let Some(group) = sums.get(&sum.group)
            && group.anchor == anchor
        {
            item.extras.extend_from_slice(&group.extras);
            item.total = Some(group.total);
        }
        if let Some(extras) = identity_extras.get(&anchor) {
            item.extras.extend_from_slice(extras);
        }
    };

    // Walk the list building chips and clusters, folding plain item repeats
    // into the nearest earlier chip naming the same item.
    let mut items: Vec<BoardItem> = Vec::new();
    let mut clusters: HashMap<u8, usize> = HashMap::new();
    let mut chip_by_item: HashMap<ItemId, usize> = HashMap::new();
    for index in (0..rows.len()).filter(|&index| !hidden[index]) {
        let row = requirement(index);
        if let Some(group) = row.alternative_group {
            if let Some(&position) = clusters.get(&group) {
                items[position].members.push(index);
                attach(&mut items[position], index);
                continue;
            }
            let mut item = BoardItem {
                members: vec![index],
                cluster: Some(group),
                extras: Vec::new(),
                total: None,
            };
            attach(&mut item, index);
            clusters.insert(group, items.len());
            items.push(item);
            continue;
        }
        if let Some(item_id) = row.item
            && is_plain_item_copy(row, item_id)
            && let Some(&position) = chip_by_item.get(&item_id)
            && items[position].total.is_none()
            && items[position].count() < usize::from(STACK_MAX)
        {
            items[position].extras.push(index);
            continue;
        }
        let mut item = BoardItem {
            members: vec![index],
            cluster: None,
            extras: Vec::new(),
            total: None,
        };
        attach(&mut item, index);
        if let Some(item_id) = row.item
            && !row.blanket
            && row.level_sum.is_none()
        {
            chip_by_item.insert(item_id, items.len());
        }
        items.push(item);
    }
    // Single-member clusters render as chips.
    for item in &mut items {
        if item.members.len() == 1 {
            item.cluster = None;
        }
    }
    items
}

/// Row indices of every identity group, by label.
fn identity_groups<R: AsRef<Requirement>>(rows: &[R]) -> BTreeMap<u8, Vec<usize>> {
    let mut groups: BTreeMap<u8, Vec<usize>> = BTreeMap::new();
    for (index, row) in rows.iter().enumerate() {
        if let Some(group) = row.as_ref().identity_group {
            groups.entry(group).or_default().push(index);
        }
    }
    groups
}

/// The board fold with, for every row, the entry that shows or hides it.
struct Board {
    items: Vec<BoardItem>,
    /// For each row: the entry it is a member of, if it is visible.
    member_of: Vec<Option<usize>>,
    /// For each row: the entry it is a member or hidden copy of. The fold
    /// claims every row, so this is `None` only for a list the board was
    /// not folded from.
    owner: Vec<Option<usize>>,
}

impl Board {
    fn new(rows: &[Row]) -> Self {
        Self::from_items(rows.len(), board_items(rows))
    }

    fn from_items(len: usize, items: Vec<BoardItem>) -> Self {
        let mut member_of = vec![None; len];
        let mut owner = vec![None; len];
        for (position, item) in items.iter().enumerate() {
            for &index in &item.members {
                member_of[index] = Some(position);
                owner[index] = Some(position);
            }
            for &index in &item.extras {
                owner[index] = Some(position);
            }
        }
        Self {
            items,
            member_of,
            owner,
        }
    }

    /// The visible row with `key` and its entry. Lookups never reach a
    /// hidden copy: a gesture names what the user sees.
    fn member(&self, rows: &[Row], key: u64) -> Option<(usize, &BoardItem)> {
        let index = index_of(rows, key)?;
        let position = self.member_of[index]?;
        Some((index, &self.items[position]))
    }

    fn count_of(&self, index: usize) -> usize {
        self.member_of[index].map_or(1, |position| self.items[position].count())
    }
}

fn index_of(rows: &[Row], key: u64) -> Option<usize> {
    rows.iter().position(|row| row.key == key)
}

/// The lowest label from 1 to `maximum` not `taken`.
fn free_group(taken: &BTreeSet<u8>, maximum: u8) -> Option<u8> {
    (1..=maximum).find(|group| !taken.contains(group))
}

/// The label a new cluster takes: one past the largest in the whole list
/// (both sections, so a blanket cluster never shares an ordinary one's
/// label) and past every `held` one, or the smallest free label once 255 is
/// taken.
fn next_alternative_group(rows: &[Row], held: &BTreeSet<u8>) -> Option<u8> {
    let taken = taken(
        rows.iter().map(|row| row.requirement.alternative_group),
        held,
    );
    let largest = taken.last().copied().unwrap_or(0);
    largest
        .checked_add(1)
        .or_else(|| free_group(&taken, u8::MAX))
}

/// The bare copy a wildcard or cluster stack grows by: the anchor's family
/// and nothing else, tied to the anchor by `identity_group`. It may carry
/// its own floor limit, the one bound that is a placement rather than an
/// item property.
///
/// The copy names the broad family, never the anchor's melee/thrown
/// narrowing: the identity label already forces it to be the very item the
/// anchor matched, and the engine reads a narrowed copy as a second
/// constrained member of the stack.
fn bare_copy(anchor: &Requirement, identity_group: u8, max_depth: Option<u8>) -> Requirement {
    Requirement {
        identity_group: Some(identity_group),
        max_depth,
        ..Requirement::any(anchor.kind)
    }
}

/// The plain repeat a concrete stack of the anchor's item grows by. It is
/// built from the defaults — never copying the anchor's resin exclusion,
/// blanket flag, trinket selection or transmutations — and keeps only the
/// item and its melee/thrown narrowing.
fn plain_copy(anchor: &Requirement, max_depth: Option<u8>) -> Requirement {
    Requirement {
        weapon_category: anchor.weapon_category,
        item: anchor.item,
        max_depth,
        ..Requirement::any(anchor.kind)
    }
}

/// Whether a cluster's stack label may be spread onto `row`. Trinkets,
/// artifacts and blankets never carry a stack label — the engine rejects a
/// blanket with one, and Android's model refuses to construct a trinket or
/// artifact with one — so a cluster holding such a member stacks without it.
fn takes_stack_label(row: &Requirement) -> bool {
    !row.blanket && !matches!(row.kind, ItemKind::Trinket | ItemKind::Artifact)
}

/// Rewrites the list into its canonical stack encoding and drops every
/// group that no longer says anything:
///
/// - a cluster that holds an identity label spreads it to every member that
///   can carry one;
/// - a lone alternative and a lone level-sum member dissolve;
/// - a stack anchored on a lone concrete chip carries plain repeats, not
///   identity labels;
/// - a lone identity label dissolves.
///
/// The steps run in that order so one pass is enough: a second changes
/// nothing. Rows keep their keys and their order. [`apply`] then moves the
/// labels a hand-written list held out of range into it ([`relabel`]).
fn normalize(rows: &mut [Row]) {
    let mut cluster_label: BTreeMap<u8, u8> = BTreeMap::new();
    for row in rows.iter() {
        if let (Some(cluster), Some(label)) = (
            row.requirement.alternative_group,
            row.requirement.identity_group,
        ) && takes_stack_label(&row.requirement)
        {
            cluster_label.insert(cluster, label);
        }
    }
    for row in rows.iter_mut() {
        if let Some(cluster) = row.requirement.alternative_group
            && let Some(&label) = cluster_label.get(&cluster)
            && takes_stack_label(&row.requirement)
        {
            row.requirement.identity_group = Some(label);
        }
    }
    // Groups of one say nothing. A cluster or a combined level of one goes
    // first, so the chip it leaves is seen as the lone chip it now is.
    let alternatives = counted(rows.iter().map(|row| row.requirement.alternative_group));
    let sums = counted(
        rows.iter()
            .map(|row| row.requirement.level_sum.map(|sum| sum.group)),
    );
    for row in rows.iter_mut() {
        let requirement = &mut row.requirement;
        if requirement
            .alternative_group
            .is_some_and(|group| alternatives[&group] < 2)
        {
            requirement.alternative_group = None;
        }
        if requirement
            .level_sum
            .is_some_and(|sum| sums[&sum.group] < 2)
        {
            requirement.level_sum = None;
        }
    }
    // A stack anchored on a lone concrete chip encodes as plain repeats —
    // unless a copy also counts a combined level, sits in a cluster, or is
    // of another kind (a hand-written list): as a plain repeat of the anchor
    // it would lose what the board shows of it, so the group stays as
    // written and the problem list says what is wrong with it.
    for members in identity_groups(rows).values() {
        let constrained: Vec<usize> = members
            .iter()
            .copied()
            .filter(|&index| !rows[index].requirement.is_bare())
            .collect();
        let [anchor_index] = constrained[..] else {
            continue;
        };
        let anchor = rows[anchor_index].requirement;
        if anchor.item.is_none()
            || anchor.alternative_group.is_some()
            || members.iter().any(|&index| {
                let member = &rows[index].requirement;
                index != anchor_index
                    && (member.level_sum.is_some()
                        || member.alternative_group.is_some()
                        || member.kind != anchor.kind)
            })
        {
            continue;
        }
        for &index in members {
            rows[index].requirement = if index == anchor_index {
                Requirement {
                    identity_group: None,
                    ..anchor
                }
            } else {
                plain_copy(&anchor, rows[index].requirement.max_depth)
            };
        }
    }
    let identities = counted(rows.iter().map(|row| row.requirement.identity_group));
    for row in rows.iter_mut() {
        let requirement = &mut row.requirement;
        if requirement
            .identity_group
            .is_some_and(|group| identities[&group] < 2)
        {
            requirement.identity_group = None;
        }
    }
}

/// Moves every group label a hand-written list holds out of range onto a
/// free one in range: a stack or combined-level label outside 1–4
/// ([`MAX_IDENTITY_GROUP`], [`MAX_LEVEL_SUM_GROUP`]) — the document codec
/// reads any byte — and the reserved either/or label 0. Labels in range
/// stay; each group out of range takes the lowest label no row and no
/// `held` label uses, in first-appearance order, the way wide alternative
/// labels are compacted ([`super::compact_alternative_labels`]), so distinct
/// groups stay distinct. A group no label is left for keeps its own, and the
/// problem list reports it: merging two stacks to fit would change what the
/// list asks for. Returns whether any label moved.
fn relabel(rows: &mut [Row], held: &HeldLabels) -> bool {
    let identity = relabel_groups(
        rows,
        &held.identity,
        MAX_IDENTITY_GROUP,
        |requirement| requirement.identity_group,
        |requirement, label| requirement.identity_group = Some(label),
    );
    let level_sum = relabel_groups(
        rows,
        &held.level_sum,
        MAX_LEVEL_SUM_GROUP,
        |requirement| requirement.level_sum.map(|sum| sum.group),
        |requirement, label| {
            if let Some(sum) = &mut requirement.level_sum {
                sum.group = label;
            }
        },
    );
    let alternative = relabel_groups(
        rows,
        &held.alternative,
        u8::MAX,
        |requirement| requirement.alternative_group,
        |requirement, label| requirement.alternative_group = Some(label),
    );
    identity || level_sum || alternative
}

/// [`relabel`] for one kind of label, read by `label` and written by `set`,
/// whose range is `1..=maximum`.
fn relabel_groups(
    rows: &mut [Row],
    held: &BTreeSet<u8>,
    maximum: u8,
    label: impl Fn(&Requirement) -> Option<u8>,
    set: impl Fn(&mut Requirement, u8),
) -> bool {
    let mut used = taken(rows.iter().map(|row| label(&row.requirement)), held);
    let mut moved: BTreeMap<u8, u8> = BTreeMap::new();
    let mut changed = false;
    for row in rows.iter_mut() {
        let Some(old) = label(&row.requirement).filter(|old| !(1..=maximum).contains(old)) else {
            continue;
        };
        let new = *moved.entry(old).or_insert_with(|| {
            free_group(&used, maximum).map_or(old, |new| {
                used.insert(new);
                new
            })
        });
        if new != old {
            set(&mut row.requirement, new);
            changed = true;
        }
    }
    changed
}

fn counted(groups: impl IntoIterator<Item = Option<u8>>) -> BTreeMap<u8, usize> {
    let mut counts: BTreeMap<u8, usize> = BTreeMap::new();
    for group in groups.into_iter().flatten() {
        *counts.entry(group).or_default() += 1;
    }
    counts
}

/// Moves the row at `from` after the last row matching `after`.
fn move_after(mut rows: Vec<Row>, from: usize, after: impl Fn(&Requirement) -> bool) -> Vec<Row> {
    let moving = rows.remove(from);
    let insert_at = rows
        .iter()
        .rposition(|row| after(&row.requirement))
        .map_or(0, |last| last + 1);
    rows.insert(insert_at, moving);
    rows
}

fn without(rows: &[Row], doomed: &[usize]) -> Vec<Row> {
    rows.iter()
        .enumerate()
        .filter(|(index, _)| !doomed.contains(index))
        .map(|(_, row)| *row)
        .collect()
}

/// Why an edit changed nothing although it could have.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum Refusal {
    /// A join would put a stack into a cluster spanning categories. A copy
    /// has to name the kind it copies, and "spear or wand" names none, so
    /// the join is refused rather than splitting off or deleting copies.
    MixedCategoryStack,
    /// A blanket requirement cannot count levels together.
    BlanketTotal,
    /// Every identity or combined-level label (A–D), or every alternative
    /// label, is already in use.
    NoFreeGroup,
}

impl Refusal {
    /// The stable snake-case name the envelopes carry.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::MixedCategoryStack => "mixed_category_stack",
            Self::BlanketTotal => "blanket_total",
            Self::NoFreeGroup => "no_free_group",
        }
    }
}

impl fmt::Display for Refusal {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::MixedCategoryStack => "Copies can only be grouped with the same item type.",
            Self::BlanketTotal => "A blanket requirement cannot count levels together.",
            Self::NoFreeGroup => {
                "Every group label is in use. Remove a stack or a combined level first."
            }
        })
    }
}

/// One board or sheet edit. Keys name visible rows — a chip, or a member of
/// a cluster — never a stack's hidden copies; an edit naming an unknown key
/// changes nothing.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Edit {
    /// Rewrites the list into its canonical encoding, stack and
    /// combined-level labels in 1–4 among it. Platforms send it once when
    /// they import a list; every other edit that changes the rows ends in
    /// it too.
    Normalize,
    /// Makes `source` an either/or alternative of `target` (any member of a
    /// chip or cluster).
    Join { source: u64, target: u64 },
    /// Pulls a cluster member out of its cluster; it leaves the cluster's
    /// stack behind.
    Detach { key: u64 },
    /// Deletes a cluster member, or the whole entry of a chip.
    Remove { key: u64 },
    /// Deletes the whole entry holding `key`: members and hidden copies.
    RemoveItem { key: u64 },
    /// Sets how many items the entry asks for, clamped to
    /// `1..=`[`STACK_MAX`].
    SetCount { key: u64, count: u8 },
    /// Sets or clears the stack's combined level, clamped to
    /// `1..=`[`level_capacity`].
    SetTotal { key: u64, total: Option<u8> },
    /// Turns counting levels on (at [`default_total`]) or off.
    ToggleLevels { key: u64 },
    /// Sets or clears the floor limit of the stack's hidden copies; an
    /// empty boss floor snaps to the floor above it.
    SetCopyDepth { key: u64, max_depth: Option<u8> },
    /// Stores a requirement from the sheet with its stack's shape. `key`
    /// names the edited row; `None`, or a key not in the list, adds a new
    /// row at the end (with that key, when one is given). A stack that finds
    /// no free label refuses the whole save. Saving what is already there —
    /// the same requirement, count, combined level and copy floor — changes
    /// nothing.
    Save {
        key: Option<u64>,
        requirement: Requirement,
        count: u8,
        total: Option<u8>,
        copy_depth: Option<u8>,
    },
}

impl Edit {
    /// The edit with its keys read through a key repair's [`redirects`]: a
    /// key that was re-keyed names its first occurrence's new key.
    pub(crate) fn resolved(self, map: &HashMap<u64, u64>) -> Self {
        let key = |key: u64| map.get(&key).copied().unwrap_or(key);
        match self {
            Self::Normalize => Self::Normalize,
            Self::Join { source, target } => Self::Join {
                source: key(source),
                target: key(target),
            },
            Self::Detach { key: k } => Self::Detach { key: key(k) },
            Self::Remove { key: k } => Self::Remove { key: key(k) },
            Self::RemoveItem { key: k } => Self::RemoveItem { key: key(k) },
            Self::SetCount { key: k, count } => Self::SetCount { key: key(k), count },
            Self::SetTotal { key: k, total } => Self::SetTotal { key: key(k), total },
            Self::ToggleLevels { key: k } => Self::ToggleLevels { key: key(k) },
            Self::SetCopyDepth { key: k, max_depth } => Self::SetCopyDepth {
                key: key(k),
                max_depth,
            },
            Self::Save {
                key: k,
                requirement,
                count,
                total,
                copy_depth,
            } => Self::Save {
                key: k.map(key),
                requirement,
                count,
                total,
                copy_depth,
            },
        }
    }
}

/// What [`apply`] returns.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EditResult {
    /// The list after every applied edit.
    pub rows: Vec<Row>,
    /// The key the platform should hand out next (see [`apply`]).
    pub next_key: u64,
    /// Whether `rows` differ from the request's rows (a key repair counts).
    /// Edits that undo each other within one request change nothing.
    pub changed: bool,
    /// Every key repair made before the edits ran, as `(old, new)`.
    pub rekeyed: Vec<(u64, u64)>,
    /// The row the platform should follow — scroll to, highlight, announce:
    /// the joined source, the detached row, the anchor of the entry an edit
    /// reshaped or a save landed in — as the last edit that applied left
    /// it. A save names its entry even when it stored what was already
    /// there, so a closing sheet can return to its chip; `None` after a
    /// removal and when no edit applied.
    pub focus: Option<u64>,
    /// Why the sequence stopped early, if it did.
    pub refused: Option<Refusal>,
}

/// Applies `edits` in order to `rows`.
///
/// Keys are repaired first: a zero, out-of-range or later-duplicate key is
/// re-keyed in list order (reported in [`EditResult::rekeyed`]), and the
/// edits' own keys are read through the same mapping — the first occurrence
/// of a key keeps it. New rows take `max(largest key, next_key − 1) + 1`, or
/// the smallest free key once that passes [`super::MAX_KEY`].
///
/// An edit that changes nothing, or is refused, leaves the rows exactly as
/// they were (it does not normalize); a refusal also stops the sequence,
/// keeping earlier edits. A request without edits returns the rows verbatim
/// after key repair only. [`EditResult::changed`] compares the final rows
/// with the repaired request's, so edits that undo each other report no
/// change and a platform writes nothing back. The focus of an effective
/// edit always names a visible row.
///
/// Every effective edit leaves the list canonical, and so moves a stack,
/// combined-level or either/or label a hand-written list held out of range
/// onto a free label in range; a group no label is left for keeps its own.
#[must_use]
pub fn apply(rows: &[Row], next_key_hint: Option<u64>, edits: &[Edit]) -> EditResult {
    apply_holding(rows, next_key_hint, edits, &HeldLabels::default())
}

/// [`apply`] on a list beside rows the editor cannot read, whose group
/// labels no edit may take (see [`HeldLabels`]).
pub(crate) fn apply_holding(
    rows: &[Row],
    next_key_hint: Option<u64>,
    edits: &[Edit],
    held: &HeldLabels,
) -> EditResult {
    let (repaired, rekeyed) = repair_keys(rows, next_key_hint);
    let map = redirects(&rekeyed);
    let mut current = repaired.clone();
    let mut focus = None;
    let mut refused = None;
    for edit in edits {
        let outcome = apply_one(&current, next_key_hint, edit.resolved(&map), held);
        if let Some(mut next) = outcome.rows {
            // A moved label can move where a hand-written list's overlapping
            // groups fold a row, so the focus is found again.
            focus = if relabel(&mut next, held) {
                outcome.focus.and_then(|key| visible_focus(&next, key))
            } else {
                outcome.focus
            };
            current = next;
        }
        if outcome.refused.is_some() {
            refused = outcome.refused;
            break;
        }
    }
    EditResult {
        next_key: next_key(&current, next_key_hint),
        changed: !rekeyed.is_empty() || current != repaired,
        rows: current,
        rekeyed,
        focus,
        refused,
    }
}

/// Group labels held by rows beside the list that the editor cannot read —
/// the unreadable rows the JSON envelopes carry through. Such a row takes
/// part in no relationship, so a label it holds is neither its cluster's nor
/// its stack's to the editor, but no longer free either: a new cluster,
/// stack or combined level never takes one, or an edit would tie the rows
/// it made to a row the user cannot see into.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct HeldLabels {
    pub(crate) alternative: BTreeSet<u8>,
    pub(crate) identity: BTreeSet<u8>,
    pub(crate) level_sum: BTreeSet<u8>,
}

/// Every label `labels` use, with the `held` ones.
fn taken(labels: impl IntoIterator<Item = Option<u8>>, held: &BTreeSet<u8>) -> BTreeSet<u8> {
    labels
        .into_iter()
        .flatten()
        .chain(held.iter().copied())
        .collect()
}

/// What one edit did: new rows (`None` when it was a no-op or refused), the
/// row to follow, and a refusal.
#[derive(Default)]
struct Outcome {
    rows: Option<Vec<Row>>,
    focus: Option<u64>,
    refused: Option<Refusal>,
}

impl Outcome {
    fn unchanged() -> Self {
        Self::default()
    }

    fn refused(refusal: Refusal) -> Self {
        Self {
            refused: Some(refusal),
            ..Self::default()
        }
    }

    /// Normalized `rows`, following `focus`.
    fn rows(mut rows: Vec<Row>, focus: Option<u64>) -> Self {
        normalize(&mut rows);
        Self::from_step(Step::Rows(rows), focus)
    }

    /// A stack primitive's result as a whole edit, following `focus`.
    fn from_step(step: Step, focus: Option<u64>) -> Self {
        match step {
            Step::Unchanged => Self::unchanged(),
            Step::Refused(refusal) => Self::refused(refusal),
            Step::Rows(rows) => Self {
                focus: focus.and_then(|key| visible_focus(&rows, key)),
                rows: Some(rows),
                refused: None,
            },
        }
    }
}

/// The row a platform can follow for `key` after an edit: `key` itself while
/// it is visible, else the anchor of the entry that now hides it. Normalizing
/// can fold a row away — a bare copy sharing a label with a lone named chip
/// becomes that chip's plain repeat — and a focus on a hidden copy would
/// name nothing on the board.
fn visible_focus(rows: &[Row], key: u64) -> Option<u64> {
    let index = index_of(rows, key)?;
    let board = Board::new(rows);
    if board.member_of[index].is_some() {
        return Some(key);
    }
    board.owner[index].map(|position| rows[board.items[position].anchor()].key)
}

/// A stack primitive's result: normalized rows, nothing to do, or a refusal.
enum Step {
    Unchanged,
    Rows(Vec<Row>),
    Refused(Refusal),
}

fn apply_one(rows: &[Row], hint: Option<u64>, edit: Edit, held: &HeldLabels) -> Outcome {
    match edit {
        Edit::Normalize => Outcome::rows(rows.to_vec(), None),
        Edit::Join { source, target } => join(rows, source, target, held),
        Edit::Detach { key } => detach(rows, key),
        Edit::Remove { key } => remove(rows, key, false),
        Edit::RemoveItem { key } => remove(rows, key, true),
        Edit::SetCount { key, count } => {
            let board = Board::new(rows);
            let Some((_, item)) = board.member(rows, key) else {
                return Outcome::unchanged();
            };
            let anchor = rows[item.anchor()].key;
            let step = set_count(rows, item, count, &mut Keys::minting(hint), None, held);
            Outcome::from_step(step, Some(anchor))
        }
        Edit::SetTotal { key, total } => {
            let board = Board::new(rows);
            let Some((_, item)) = board.member(rows, key) else {
                return Outcome::unchanged();
            };
            let anchor = rows[item.anchor()].key;
            Outcome::from_step(set_total(rows, item, total, None, held), Some(anchor))
        }
        Edit::ToggleLevels { key } => {
            let board = Board::new(rows);
            let Some((_, item)) = board.member(rows, key) else {
                return Outcome::unchanged();
            };
            let anchor = rows[item.anchor()].key;
            let total = match item.total {
                Some(_) => None,
                None => Some(default_total(rows, item)),
            };
            Outcome::from_step(set_total(rows, item, total, None, held), Some(anchor))
        }
        Edit::SetCopyDepth { key, max_depth } => {
            let board = Board::new(rows);
            let Some((_, item)) = board.member(rows, key) else {
                return Outcome::unchanged();
            };
            let anchor = rows[item.anchor()].key;
            Outcome::from_step(set_copy_depth(rows, item, max_depth), Some(anchor))
        }
        Edit::Save {
            key,
            requirement,
            count,
            total,
            copy_depth,
        } => save(
            rows,
            (hint, held),
            key,
            requirement,
            (count, total, copy_depth),
        ),
    }
}

/// Where a stack primitive takes the keys of the copies it adds: first the
/// keys a save freed by taking the old stack down (so saving an unchanged
/// chip gives back identical rows), then fresh ones.
struct Keys {
    reuse: VecDeque<u64>,
    hint: Option<u64>,
}

impl Keys {
    fn minting(hint: Option<u64>) -> Self {
        Self {
            reuse: VecDeque::new(),
            hint,
        }
    }

    fn take(&mut self, rows: &[Row]) -> u64 {
        // A freed key is free by construction; the check keeps keys unique
        // even if a hand-written list made the old stack list a row twice.
        while let Some(key) = self.reuse.pop_front() {
            if rows.iter().all(|row| row.key != key) {
                return key;
            }
        }
        mint_key(rows, self.hint)
    }
}

/// `preferred` when it is in range and not `taken`, else the lowest free
/// label.
fn label_for(taken: &BTreeSet<u8>, preferred: Option<u8>, maximum: u8) -> Option<u8> {
    preferred
        .filter(|label| (1..=maximum).contains(label) && !taken.contains(label))
        .or_else(|| free_group(taken, maximum))
}

/// Sets how many items `item` asks for. Shrinking deletes the last copies;
/// growing adds copies after the entry's last row, of the shape the stack
/// already has: a copy of the level-sum member, a plain repeat of a lone
/// named chip, or a bare copy under the entry's identity label (minting one,
/// preferably `preferred_label`, when the entry has none — never a `held`
/// one). New copies keep to the floor limit the existing copies carry.
fn set_count(
    rows: &[Row],
    item: &BoardItem,
    count: u8,
    keys: &mut Keys,
    preferred_label: Option<u8>,
    held: &HeldLabels,
) -> Step {
    let wanted = usize::from(count.clamp(1, STACK_MAX)) - 1;
    if wanted == item.extras.len() {
        return Step::Unchanged;
    }
    if wanted < item.extras.len() {
        let mut next = without(rows, &item.extras[wanted..]);
        normalize(&mut next);
        return Step::Rows(next);
    }
    // Shrinking a cluster that spans categories is fine; growing one is not.
    if !can_grow(rows, item) {
        return Step::Unchanged;
    }
    let anchor = rows[item.anchor()].requirement;
    let inherited = item
        .extras
        .first()
        .and_then(|&index| rows[index].requirement.max_depth);
    let mut next = rows.to_vec();
    // A lone alternative label (a hand-written cluster of one) stays on
    // the anchor alone: copies of it would make it a cluster.
    let template = if item.total.is_some() && anchor.level_sum.is_some() {
        Requirement {
            alternative_group: None,
            ..anchor
        }
    } else if item.cluster.is_none() && anchor.item.is_some() {
        plain_copy(&anchor, inherited)
    } else {
        let used = taken(
            next.iter().map(|row| row.requirement.identity_group),
            &held.identity,
        );
        let Some(label) = anchor
            .identity_group
            .or_else(|| label_for(&used, preferred_label, MAX_IDENTITY_GROUP))
        else {
            return Step::Refused(Refusal::NoFreeGroup);
        };
        for &index in &item.members {
            next[index].requirement.identity_group = Some(label);
        }
        bare_copy(&anchor, label, inherited)
    };
    let insert_at = item
        .members
        .iter()
        .chain(&item.extras)
        .max()
        .copied()
        .unwrap_or(0)
        + 1;
    for offset in 0..wanted - item.extras.len() {
        let key = keys.take(&next);
        next.insert(
            insert_at + offset,
            Row {
                key,
                requirement: template,
            },
        );
    }
    normalize(&mut next);
    Step::Rows(next)
}

/// Sets or clears the stack's combined level.
///
/// With a total the whole stack becomes identical optional members sharing
/// a level-sum label ("up to N items reaching T levels"), each open to any
/// upgrade; without one it returns to an anchor with plain repeats
/// ("exactly N of the item"). Only a lone named ring stack of two or more
/// counts levels — levels add up across rings alone, and a single member
/// would dissolve and silently drop its upgrade — but clearing works on any
/// stale sum a hand-written document left. A new combined-level label is
/// `preferred` when free, and never a `held` one.
fn set_total(
    rows: &[Row],
    item: &BoardItem,
    total: Option<u8>,
    preferred: Option<u8>,
    held: &HeldLabels,
) -> Step {
    let anchor = rows[item.anchor()].requirement;
    if anchor.blanket {
        return Step::Refused(Refusal::BlanketTotal);
    }
    if item.cluster.is_some() || anchor.item.is_none() {
        return Step::Unchanged;
    }
    let mut next = rows.to_vec();
    let Some(total) = total else {
        if item.total.is_none() {
            return Step::Unchanged;
        }
        next[item.anchor()].requirement.level_sum = None;
        for &index in &item.extras {
            next[index].requirement = plain_copy(&anchor, None);
        }
        normalize(&mut next);
        return Step::Rows(next);
    };
    if anchor.kind != ItemKind::Ring || item.count() < 2 {
        return Step::Unchanged;
    }
    let total = total.clamp(1, level_capacity(rows, item).max(1));
    if item.total == Some(total) {
        return Step::Unchanged;
    }
    let used = taken(
        rows.iter()
            .map(|row| row.requirement.level_sum.map(|sum| sum.group)),
        &held.level_sum,
    );
    let Some(group) = anchor
        .level_sum
        .map(|sum| sum.group)
        .or_else(|| label_for(&used, preferred, MAX_LEVEL_SUM_GROUP))
    else {
        return Step::Refused(Refusal::NoFreeGroup);
    };
    // Combined-level members never sit in a cluster; `item` is none, so
    // any alternative label here is a lone one that would dissolve anyway.
    let member = Requirement {
        upgrade: UpgradeRequirement::Any,
        identity_group: None,
        alternative_group: None,
        level_sum: Some(LevelSum {
            group,
            minimum_total: total,
        }),
        ..anchor
    };
    for &index in std::iter::once(&item.anchor()).chain(&item.extras) {
        next[index].requirement = member;
    }
    normalize(&mut next);
    Step::Rows(next)
}

/// Sets or clears the floor limit of the stack's hidden copies. The anchor
/// keeps its own limit: "the +3 one before floor 4, the rest wherever" and
/// "…the rest before floor 10" are both sayable. A combined-level stack has
/// identical members and no lone copies to bound.
fn set_copy_depth(rows: &[Row], item: &BoardItem, max_depth: Option<u8>) -> Step {
    if item.total.is_some() || item.extras.is_empty() {
        return Step::Unchanged;
    }
    let max_depth = max_depth.map(|depth| normalize_floor_limit(depth.clamp(1, MAX_SEARCH_DEPTH)));
    if item
        .extras
        .iter()
        .all(|&index| rows[index].requirement.max_depth == max_depth)
    {
        return Step::Unchanged;
    }
    let mut next = rows.to_vec();
    for &index in &item.extras {
        next[index].requirement.max_depth = max_depth;
    }
    normalize(&mut next);
    Step::Rows(next)
}

/// Pulls a cluster member out of its cluster; it leaves its stack behind.
fn detach(rows: &[Row], key: u64) -> Outcome {
    let board = Board::new(rows);
    let Some((index, item)) = board.member(rows, key) else {
        return Outcome::unchanged();
    };
    if item.cluster.is_none() {
        return Outcome::unchanged();
    }
    let mut next = rows.to_vec();
    next[index].requirement.alternative_group = None;
    next[index].requirement.identity_group = None;
    Outcome::rows(next, Some(key))
}

/// Deletes one cluster member (the cluster and its stack live on without
/// it), or — for a chip, or with `whole` — the entry with its hidden copies.
fn remove(rows: &[Row], key: u64, whole: bool) -> Outcome {
    let board = Board::new(rows);
    let Some((index, item)) = board.member(rows, key) else {
        return Outcome::unchanged();
    };
    let doomed: Vec<usize> = if item.cluster.is_some() && !whole {
        vec![index]
    } else {
        item.members.iter().chain(&item.extras).copied().collect()
    };
    Outcome::rows(without(rows, &doomed), None)
}

/// Precomputed answers to "may this visible row join that one?", shared by
/// [`join`], [`drop_action`] and [`join_candidates`] so the three can never
/// disagree.
struct JoinRules {
    board: Board,
    /// The families present in each cluster, as bit sets over `ItemKind`.
    cluster_families: HashMap<u8, u8>,
    /// The label a new cluster would take.
    next_group: Option<u8>,
}

/// What joining two visible rows would do.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum JoinCheck {
    /// Same row, already one cluster, or the other board section.
    Nothing,
    Refuse(Refusal),
    /// Join into this alternative group.
    Join(u8),
}

const fn family_bit(kind: ItemKind) -> u8 {
    1 << kind as u8
}

impl JoinRules {
    /// The rules for `rows` beside rows holding the `held` alternative
    /// labels, which a new cluster never takes.
    fn new(rows: &[Row], board: Board, held: &BTreeSet<u8>) -> Self {
        let mut cluster_families: HashMap<u8, u8> = HashMap::new();
        for row in rows {
            if let Some(group) = row.requirement.alternative_group {
                *cluster_families.entry(group).or_default() |= family_bit(row.requirement.kind);
            }
        }
        Self {
            board,
            cluster_families,
            next_group: next_alternative_group(rows, held),
        }
    }

    /// Android's `canJoinAlternatives` after #190: a join that would mix
    /// categories is refused when either side's entry is a stack, because a
    /// copy has to name the kind it copies and "spear or wand" names none.
    fn check(&self, rows: &[Row], source: usize, target: usize) -> JoinCheck {
        let (from, onto) = (&rows[source].requirement, &rows[target].requirement);
        if source == target || from.blanket != onto.blanket {
            return JoinCheck::Nothing;
        }
        let Some(group) = onto.alternative_group.or(self.next_group) else {
            return JoinCheck::Refuse(Refusal::NoFreeGroup);
        };
        if from.alternative_group == Some(group) {
            return JoinCheck::Nothing;
        }
        let mixed = onto.kind != from.kind
            || onto.alternative_group.is_some_and(|group| {
                self.cluster_families.get(&group).copied().unwrap_or(0) & !family_bit(from.kind)
                    != 0
            });
        if mixed && (self.board.count_of(source) > 1 || self.board.count_of(target) > 1) {
            return JoinCheck::Refuse(Refusal::MixedCategoryStack);
        }
        JoinCheck::Join(group)
    }
}

/// The chip at `source` becomes an either/or alternative of the chip at
/// `target`, and moves after the cluster's last member.
///
/// A combined level cannot travel into a cluster and is dropped from both.
/// Within one category a plain-repeat stack keeps its copies by trading its
/// *own* entry's repeats for bare copies under an identity label, which the
/// cluster's members then share (another chip may name the same item with
/// its own stack; its repeats stay put). A mixed-category join of two
/// uncounted chips clears any leftover identity labels and deletes nothing;
/// one involving a stack is refused (Android #190).
fn join(rows: &[Row], source_key: u64, target_key: u64, held: &HeldLabels) -> Outcome {
    let rules = JoinRules::new(rows, Board::new(rows), &held.alternative);
    let (Some((source, _)), Some((target, _))) = (
        rules.board.member(rows, source_key),
        rules.board.member(rows, target_key),
    ) else {
        return Outcome::unchanged();
    };
    let group = match rules.check(rows, source, target) {
        JoinCheck::Nothing => return Outcome::unchanged(),
        JoinCheck::Refuse(refusal) => return Outcome::refused(refusal),
        JoinCheck::Join(group) => group,
    };
    let members: Vec<usize> = (0..rows.len())
        .filter(|&index| {
            index == source
                || index == target
                || rows[index].requirement.alternative_group == Some(group)
        })
        .collect();
    let first_kind = rows[members[0]].requirement.kind;
    let one_category = members
        .iter()
        .all(|&index| rows[index].requirement.kind == first_kind);
    let mut next = rows.to_vec();
    if one_category {
        // Trade plain repeats for identity copies so the stack survives.
        for index in [source, target] {
            let anchor = next[index].requirement;
            let Some(item_id) = anchor.item else {
                continue;
            };
            if anchor.blanket || anchor.identity_group.is_some() {
                continue;
            }
            let copies: Vec<usize> = rules.board.member_of[index]
                .map(|position| rules.board.items[position].extras.clone())
                .unwrap_or_default()
                .into_iter()
                .filter(|&copy| is_plain_item_copy(&next[copy].requirement, item_id))
                .collect();
            if copies.is_empty() {
                continue;
            }
            let used = taken(
                next.iter().map(|row| row.requirement.identity_group),
                &held.identity,
            );
            let Some(label) = free_group(&used, MAX_IDENTITY_GROUP) else {
                continue;
            };
            next[index].requirement.identity_group = Some(label);
            for copy in copies {
                next[copy].requirement =
                    bare_copy(&anchor, label, next[copy].requirement.max_depth);
            }
        }
    } else {
        // Stacks were refused above, so only leftover labels remain.
        clear_identity_labels(&mut next, &members);
    }
    for index in [source, target] {
        next[index].requirement.alternative_group = Some(group);
        next[index].requirement.level_sum = None;
    }
    let moved = move_after(next, source, |row| row.alternative_group == Some(group));
    Outcome::rows(moved, Some(source_key))
}

/// Clears every identity label the rows at `members` hold, wherever it
/// appears, so a cluster spanning categories shares no identity. Only an
/// uncounted cluster gets here, whose labels are leftovers that say nothing.
fn clear_identity_labels(rows: &mut [Row], members: &[usize]) {
    let labels: BTreeSet<u8> = members
        .iter()
        .filter_map(|&index| rows[index].requirement.identity_group)
        .collect();
    for row in rows {
        if row
            .requirement
            .identity_group
            .is_some_and(|label| labels.contains(&label))
        {
            row.requirement.identity_group = None;
        }
    }
}

/// Stores the sheet's requirement with its stack's shape (web `applyEdit`).
///
/// An edited chip's stack comes down and is rebuilt from `count`, `total`
/// and `copy_depth`: the edit may have changed the very kind the copies
/// copy. The rebuilt copies reuse the removed copies' keys in order, and the
/// stack's old identity or combined-level label is preferred, so saving an
/// unchanged chip gives back identical rows. A cluster member leaves the
/// stack to its cluster and keeps its place and its stack label in it —
/// unless it became a trinket, artifact or blanket, which never carry one.
/// The saved requirement's own group labels are ignored: relationships come
/// from the row being edited, and stacks are this edit's to write.
///
/// A member moved into a category the rest of its cluster does not share
/// follows the join rule (#190): with a stack the save is refused
/// ([`Refusal::MixedCategoryStack`]), since a copy has to name the kind it
/// copies; without one the cluster's leftover stack labels are cleared, as
/// a join across categories clears them.
///
/// When the saved row turns out to be a plain repeat of an earlier chip, it
/// folds into that chip and the shape is not applied; `focus` names the
/// entry it landed in either way.
fn save(
    rows: &[Row],
    (hint, held): (Option<u64>, &HeldLabels),
    key: Option<u64>,
    requirement: Requirement,
    (count, total, copy_depth): (u8, Option<u8>, Option<u8>),
) -> Outcome {
    let requirement = Requirement {
        identity_group: None,
        level_sum: None,
        alternative_group: None,
        ..requirement
    };
    let mut keys = Keys::minting(hint);
    let mut preferred_identity = None;
    let mut preferred_sum = None;
    let (mut next, saved) = if let Some(index) = key.and_then(|key| index_of(rows, key)) {
        let board = Board::new(rows);
        let Some(position) = board.member_of[index] else {
            // A hidden copy has no sheet of its own.
            return Outcome::unchanged();
        };
        let item = &board.items[position];
        let current = rows[index].requirement;
        if saves_nothing(
            rows,
            item,
            &current,
            &requirement,
            (count, total, copy_depth),
        ) {
            let focus = rows[item.anchor()].key;
            return Outcome::from_step(Step::Rows(rows.to_vec()), Some(focus));
        }
        let mut next = rows.to_vec();
        let doomed: &[usize] = if item.cluster.is_some() {
            let mixes = requirement.kind != current.kind
                && item.members.iter().any(|&member| {
                    member != index && rows[member].requirement.kind != requirement.kind
                });
            if mixes {
                if item.count() > 1 {
                    return Outcome::refused(Refusal::MixedCategoryStack);
                }
                clear_identity_labels(&mut next, &item.members);
            }
            &[]
        } else {
            preferred_identity = current.identity_group;
            preferred_sum = current.level_sum.map(|sum| sum.group);
            &item.extras
        };
        keys.reuse = doomed.iter().map(|&copy| rows[copy].key).collect();
        next[index].requirement = Requirement {
            alternative_group: current.alternative_group,
            identity_group: next[index]
                .requirement
                .identity_group
                .filter(|_| takes_stack_label(&requirement)),
            ..requirement
        };
        (without(&next, doomed), rows[index].key)
    } else {
        let key = key
            .filter(|&key| is_valid_key(key))
            .unwrap_or_else(|| mint_key(rows, hint));
        let mut next = rows.to_vec();
        next.push(Row { key, requirement });
        (next, key)
    };
    normalize(&mut next);
    // A stack no label is free for refuses the whole save: storing the
    // requirement without the stack the sheet showed would quietly drop the
    // copies, and a refused edit leaves the rows as they were, so the sheet
    // can stay open on them with the reason.
    if let Some(refusal) = reshape(
        &mut next,
        saved,
        (count, total, copy_depth),
        &mut keys,
        (preferred_identity, preferred_sum),
        held,
    ) {
        return Outcome::refused(refusal);
    }
    let board = Board::new(&next);
    let focus = index_of(&next, saved)
        .and_then(|index| board.owner[index])
        .map(|position| next[board.items[position].anchor()].key);
    Outcome::from_step(Step::Rows(next), focus)
}

/// Whether saving `requirement` (its labels already stripped) with the
/// sheet's `(count, total, copy_depth)` onto the visible row `current`, a
/// member of `item`, would store what is already there: the same
/// requirement, and — for a lone chip — the same count, the same combined
/// level, or the copy floor the sheet showed ([`super::copy_depth`], the
/// first copy's). Such a save is a no-op and returns the rows verbatim.
///
/// Rebuilding would not always give the rows back: copies left behind a
/// chip that joined and left a cluster would come back right after it, and
/// copies with floors of their own — a plain repeat saved with its own
/// floor folds into the earlier chip — would all take the first one's. A
/// platform comparing whole lists (Android's refine plan) would see a
/// change the user never made.
fn saves_nothing(
    rows: &[Row],
    item: &BoardItem,
    current: &Requirement,
    requirement: &Requirement,
    (count, total, copy_depth): (u8, Option<u8>, Option<u8>),
) -> bool {
    let unlabelled = Requirement {
        identity_group: None,
        level_sum: None,
        alternative_group: None,
        ..*current
    };
    if unlabelled != *requirement {
        return false;
    }
    // A cluster member's stack belongs to the cluster; the save only
    // replaces the requirement.
    if item.cluster.is_some() {
        return true;
    }
    if usize::from(count.clamp(1, STACK_MAX)) != item.count() {
        return false;
    }
    let counting = total.is_some() && can_count_levels(rows, item);
    match item.total {
        Some(current_total) => counting && total == Some(current_total),
        None if counting => false,
        None => {
            let depth =
                copy_depth.map(|depth| normalize_floor_limit(depth.clamp(1, MAX_SEARCH_DEPTH)));
            item.extras.is_empty() || stack_copy_depth(rows, item) == depth
        }
    }
}

/// Gives the saved lone chip the stack shape the sheet asked for. Returns a
/// refusal when a needed label is not free (the caller then discards `rows`).
fn reshape(
    rows: &mut Vec<Row>,
    saved: u64,
    (count, total, copy_depth): (u8, Option<u8>, Option<u8>),
    keys: &mut Keys,
    (preferred_identity, preferred_sum): (Option<u8>, Option<u8>),
    held: &HeldLabels,
) -> Option<Refusal> {
    let item_of = |rows: &[Row]| {
        let index = index_of(rows, saved)?;
        let board = Board::new(rows);
        let position = board.member_of[index]?;
        Some(board.items[position].clone())
    };
    // A row that folded into an earlier chip, or sits in a cluster, leaves
    // the shape to that entry.
    let mut item = item_of(rows).filter(|item| item.cluster.is_none())?;
    let run = |rows: &mut Vec<Row>, step: Step| match step {
        Step::Unchanged => None,
        Step::Rows(next) => {
            *rows = next;
            None
        }
        Step::Refused(refusal) => Some(refusal),
    };
    if item.total.is_some() && total.is_none() {
        let step = set_total(rows, &item, None, None, held);
        if let Some(refusal) = run(rows, step) {
            return Some(refusal);
        }
        item = item_of(rows)?;
    }
    let step = set_count(rows, &item, count, keys, preferred_identity, held);
    if let Some(refusal) = run(rows, step) {
        return Some(refusal);
    }
    item = item_of(rows)?;
    let step = if total.is_some() && can_count_levels(rows, &item) {
        set_total(rows, &item, total, preferred_sum, held)
    } else {
        set_copy_depth(rows, &item, copy_depth)
    };
    run(rows, step)
}

/// Where a dragged chip was released.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DropTarget {
    /// On a visible row: a chip, or one member of a cluster.
    Row(u64),
    /// On a cluster's own capsule, outside its members.
    Cluster(u8),
    /// On the empty board of one section.
    Board { blanket: bool },
    /// On the remove target.
    Remove,
}

/// What releasing a dragged chip does. Platforms send the matching [`Edit`]:
/// `Join` → [`Edit::Join`], `Detach` → [`Edit::Detach`], `Remove` →
/// [`Edit::Remove`].
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DropAction {
    Join {
        target: u64,
    },
    /// The drop is refused; hover feedback shows the refusal's message.
    Refuse(Refusal),
    Detach,
    Remove,
    Nothing,
}

/// The policy for releasing the visible row `source` on `target`: onto a
/// row or a cluster it joins (or is refused, or does nothing for the same
/// row, its own cluster, or the other section); onto the empty board of its
/// own section a cluster member detaches — a lone chip stays put, so a
/// stack is never split off by a stray drop.
#[must_use]
pub fn drop_action(rows: &[Row], source: u64, target: DropTarget) -> DropAction {
    let board = Board::new(rows);
    let Some((source_index, source_item)) = board.member(rows, source) else {
        return DropAction::Nothing;
    };
    let target_index = match target {
        DropTarget::Remove => return DropAction::Remove,
        DropTarget::Board { blanket } => {
            return if source_item.cluster.is_some()
                && rows[source_index].requirement.blanket == blanket
            {
                DropAction::Detach
            } else {
                DropAction::Nothing
            };
        }
        DropTarget::Row(key) => match board.member(rows, key) {
            Some((index, _)) => index,
            None => return DropAction::Nothing,
        },
        DropTarget::Cluster(group) => {
            match board.items.iter().find(|item| item.cluster == Some(group)) {
                Some(item) => item.anchor(),
                None => return DropAction::Nothing,
            }
        }
    };
    match JoinRules::new(rows, board, &BTreeSet::new()).check(rows, source_index, target_index) {
        JoinCheck::Nothing => DropAction::Nothing,
        JoinCheck::Refuse(refusal) => DropAction::Refuse(refusal),
        JoinCheck::Join(_) => DropAction::Join {
            target: rows[target_index].key,
        },
    }
}

/// The visible rows one visible row may join, and those it is refused by.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct JoinCandidates {
    /// Keys of the rows a join onto would succeed, in list order.
    pub join: Vec<u64>,
    /// Keys of the rows a join onto would be refused, with the reason.
    pub refuse: Vec<(u64, Refusal)>,
}

/// Every visible row's [`JoinCandidates`], aligned with `rows` (hidden
/// copies get none and are never candidates). `items` must be
/// [`board_items`] of `rows`; the answers come from one fold, so menus
/// ("Either/or with…"), pick mode, accessibility actions and drag hover
/// feedback need no per-gesture call.
#[must_use]
pub fn join_candidates(rows: &[Row], items: &[BoardItem]) -> Vec<JoinCandidates> {
    let rules = JoinRules::new(
        rows,
        Board::from_items(rows.len(), items.to_vec()),
        &BTreeSet::new(),
    );
    let visible: Vec<usize> = (0..rows.len())
        .filter(|&index| rules.board.member_of[index].is_some())
        .collect();
    let mut candidates = vec![JoinCandidates::default(); rows.len()];
    for &source in &visible {
        for &target in &visible {
            match rules.check(rows, source, target) {
                JoinCheck::Nothing => {}
                JoinCheck::Refuse(refusal) => {
                    candidates[source].refuse.push((rows[target].key, refusal));
                }
                JoinCheck::Join(_) => candidates[source].join.push(rows[target].key),
            }
        }
    }
    candidates
}

#[cfg(test)]
mod tests;
