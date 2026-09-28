//! The board: the collapsed view of the flat row list, and every edit a
//! board gesture or a requirement-sheet save writes back.
//!
//! The fold and the edits follow the web design's `relations.ts`, with a
//! stack on every chip — a cluster member's as the engine's member stack —
//! drags that move one item rather than a whole stack, and the fixes listed
//! on the individual edits. Every effective edit ends in [`normalize`], so
//! a deleted anchor can never leave stale groups behind.

use std::collections::{BTreeMap, BTreeSet, HashMap, VecDeque};
use std::fmt;

use crate::catalog::{ItemId, ItemKind};
use crate::main_world::normalize_floor_limit;
use crate::query::{
    EffectRequirement, LevelSum, MAX_IDENTITY_GROUP, MAX_LEVEL_SUM_GROUP, MAX_SEARCH_DEPTH,
    Requirement, TierRequirement, UpgradeRequirement, gating_group,
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

/// One board entry: a chip, or an either/or cluster of chips. Every chip —
/// a lone chip or a cluster member — has a stack of its own, with the hidden
/// copies behind its ×N badge. Indices point into the list the entry was
/// folded from.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BoardItem {
    /// Visible row indices: one for a chip, every member for a cluster.
    pub members: Vec<usize>,
    /// The alternative group of a cluster of two or more; a lone
    /// alternative renders as a plain chip.
    pub cluster: Option<u8>,
    /// Every hidden copy of the entry, in list order: the copies of all its
    /// chips, each once.
    pub extras: Vec<usize>,
    /// Each member's own stack, in member order.
    pub stacks: Vec<ChipStack>,
}

impl BoardItem {
    /// The entry's first member: the row a lone chip's badges act on, and
    /// the row that names the entry's place in the list.
    #[must_use]
    pub fn anchor(&self) -> usize {
        self.members[0]
    }

    /// The stack of the member at row `index`.
    #[must_use]
    pub fn stack(&self, index: usize) -> Option<&ChipStack> {
        self.stacks.iter().find(|stack| stack.index == index)
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

/// One chip's stack: the hidden copies behind its ×N badge and its combined
/// level. A lone chip's copies are plain repeats of its item, bare copies
/// sharing its stack label, or members of its combined level; a cluster
/// member's are bare copies sharing its stack label, which members whose
/// stacks are alike share (the engine's member stacks).
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ChipStack {
    /// The chip's row index.
    pub index: usize,
    /// Whether the chip is a member of a cluster of two or more.
    pub in_cluster: bool,
    /// Hidden copy indices behind the chip's badge, in list order.
    pub copies: Vec<usize>,
    /// The stack's combined level, when one is set. Only a lone chip counts
    /// levels.
    pub total: Option<u8>,
}

impl ChipStack {
    /// How many items the chip asks for: its own row plus its copies.
    #[must_use]
    pub fn count(&self) -> usize {
        1 + self.copies.len()
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
/// alternatives, and every chip's copies fold into its own badge.
///
/// Three folds, as on the web: a combined-level group folds into its first
/// member; an identity group folds its lone bare copies into its one anchor
/// unit — a constrained requirement, or the members of one cluster that
/// carry the label (every member of a cluster whose stacks are alike, or
/// just the one whose stack it is) — and a group with two anchor units
/// cannot collapse, which validation reports; and a plain repeat of a named
/// item folds into the nearest earlier lone chip naming it, up to
/// [`STACK_MAX`] items. A cluster of one renders as a chip.
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

    // Identity stacks: bare copies fold into the anchor unit, as the engine
    // reads it: a constrained row, or the members of a cluster carrying the
    // label — bare ones too when only some members carry it, since those
    // decide whether the copies are needed at all. With no such unit, the
    // label's cluster anchors, else its first row.
    //
    // Only a hand-written list puts one row in a stack and a combined level
    // at once. The two folds must then never hide a row no entry shows — an
    // entry whose anchor is folded away never forms — so a stack whose
    // anchor a combined level folded leaves its copies on the board as
    // chips, and no row of a combined level, its anchor included, folds into
    // a stack. The web lost such rows from every entry.
    let groups = identity_groups(rows);
    let plain: Vec<Requirement> = if groups.is_empty() {
        Vec::new()
    } else {
        rows.iter().map(|row| *row.as_ref()).collect()
    };
    let mut identity_copies: HashMap<usize, Vec<usize>> = HashMap::new();
    for (&label, members) in &groups {
        let member_stack = gating_group(&plain, label).is_some();
        let units: BTreeSet<(Option<u8>, usize)> = members
            .iter()
            .copied()
            .filter(|&index| {
                let row = requirement(index);
                !row.is_bare() || (member_stack && row.alternative_group.is_some())
            })
            .map(|index| {
                requirement(index)
                    .alternative_group
                    .map_or((None, index), |group| (Some(group), 0))
            })
            .collect();
        if units.len() > 1 {
            continue;
        }
        let unit = units
            .first()
            .copied()
            .or_else(|| {
                members.iter().find_map(|&index| {
                    requirement(index)
                        .alternative_group
                        .map(|group| (Some(group), 0))
                })
            })
            .unwrap_or((None, members[0]));
        let anchors: Vec<usize> = match unit {
            (Some(group), _) => members
                .iter()
                .copied()
                .filter(|&index| requirement(index).alternative_group == Some(group))
                .collect(),
            (None, index) => vec![index],
        };
        let anchors: Vec<usize> = anchors
            .into_iter()
            .filter(|&index| !hidden[index])
            .collect();
        if anchors.is_empty() {
            continue;
        }
        // Only the lone bare copies fold. A copy a combined level already
        // folded stays with that stack (the web listed it twice, so a
        // hand-written list could count it twice).
        let copies: Vec<usize> = members
            .iter()
            .copied()
            .filter(|&index| {
                !anchors.contains(&index)
                    && !hidden[index]
                    && requirement(index).alternative_group.is_none()
                    && requirement(index).level_sum.is_none()
                    && requirement(index).is_bare()
            })
            .collect();
        if copies.is_empty() {
            continue;
        }
        for &index in &copies {
            hidden[index] = true;
        }
        for anchor in anchors {
            identity_copies.insert(anchor, copies.clone());
        }
    }

    let stack_of = |index: usize| {
        let mut stack = ChipStack {
            index,
            in_cluster: requirement(index).alternative_group.is_some(),
            copies: Vec::new(),
            total: None,
        };
        if let Some(sum) = requirement(index).level_sum
            && let Some(group) = sums.get(&sum.group)
            && group.anchor == index
        {
            stack.copies.extend_from_slice(&group.extras);
            stack.total = Some(group.total);
        }
        if let Some(copies) = identity_copies.get(&index) {
            stack.copies.extend_from_slice(copies);
        }
        stack
    };

    // Walk the list building chips and clusters, folding plain item repeats
    // into the nearest earlier lone chip naming the same item.
    let mut items: Vec<BoardItem> = Vec::new();
    let mut clusters: HashMap<u8, usize> = HashMap::new();
    let mut chip_by_item: HashMap<ItemId, usize> = HashMap::new();
    for index in (0..rows.len()).filter(|&index| !hidden[index]) {
        let row = requirement(index);
        if let Some(group) = row.alternative_group {
            if let Some(&position) = clusters.get(&group) {
                items[position].members.push(index);
                items[position].stacks.push(stack_of(index));
                continue;
            }
            clusters.insert(group, items.len());
            items.push(BoardItem {
                members: vec![index],
                cluster: Some(group),
                extras: Vec::new(),
                stacks: vec![stack_of(index)],
            });
            continue;
        }
        if let Some(item_id) = row.item
            && is_plain_item_copy(row, item_id)
            && let Some(&position) = chip_by_item.get(&item_id)
            && items[position].stacks[0].total.is_none()
            && items[position].stacks[0].count() < usize::from(STACK_MAX)
        {
            items[position].stacks[0].copies.push(index);
            continue;
        }
        if let Some(item_id) = row.item
            && !row.blanket
            && row.level_sum.is_none()
        {
            chip_by_item.insert(item_id, items.len());
        }
        items.push(BoardItem {
            members: vec![index],
            cluster: None,
            extras: Vec::new(),
            stacks: vec![stack_of(index)],
        });
    }
    for item in &mut items {
        // Single-member clusters render as chips.
        if item.members.len() == 1 {
            item.cluster = None;
            item.stacks[0].in_cluster = false;
        }
        for stack in &mut item.stacks {
            stack.copies.sort_unstable();
            stack.copies.dedup();
            item.extras.extend_from_slice(&stack.copies);
        }
        item.extras.sort_unstable();
        item.extras.dedup();
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

    /// The stack of the visible row at `index`.
    fn stack(&self, index: usize) -> Option<&ChipStack> {
        self.items[self.member_of[index]?].stack(index)
    }

    /// The stack of the visible row with `key`.
    fn visible(&self, rows: &[Row], key: u64) -> Option<&ChipStack> {
        self.stack(index_of(rows, key)?)
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

/// The bare copy a wildcard or member stack grows by: the anchor's family
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

/// Whether `row` may carry a stack label, and so stack. Trinkets, artifacts
/// and blankets never do — the engine rejects a blanket with one, and
/// Android's model refuses to construct a trinket or artifact with one.
fn takes_stack_label(row: &Requirement) -> bool {
    !row.blanket && !matches!(row.kind, ItemKind::Trinket | ItemKind::Artifact)
}

/// Rewrites the list into its canonical stack encoding and drops every
/// group that no longer says anything:
///
/// - a lone alternative and a lone level-sum member dissolve;
/// - a stack anchored on a lone concrete chip carries plain repeats, not
///   identity labels;
/// - members of one cluster whose stacks have the same copies share one
///   label: the first one's, whose copies stay, while the others' go;
/// - a lone identity label dissolves, and so does one only the members of
///   one cluster carry — members stepped down to ×1, or left without their
///   copies.
///
/// A member's stack label is its own: it never spreads to the other members
/// (the engine reads a label some members carry as those members' stack).
///
/// The steps run in that order so one pass is enough: a second changes
/// nothing. Rows keep their keys and their order; only the copies of a
/// merged stack go. [`apply`] then moves the labels a hand-written list held
/// out of range into it ([`relabel`]).
fn normalize(rows: &mut Vec<Row>) {
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
    merge_member_stacks(rows);
    // A label held by one row, or only by the members of one cluster, ties
    // no copy to anything: the members ask for one item each, and the label
    // would only use up one of the four and travel into shared queries.
    let mut spans: BTreeMap<u8, BTreeSet<Option<u8>>> = BTreeMap::new();
    for row in rows.iter() {
        if let Some(label) = row.requirement.identity_group {
            spans
                .entry(label)
                .or_default()
                .insert(row.requirement.alternative_group);
        }
    }
    let identities = counted(rows.iter().map(|row| row.requirement.identity_group));
    let idle: BTreeSet<u8> = spans
        .into_iter()
        .filter(|(label, clusters)| {
            identities[label] < 2
                || (clusters.len() == 1 && clusters.first().is_some_and(Option::is_some))
        })
        .map(|(label, _)| label)
        .collect();
    for row in rows.iter_mut() {
        let requirement = &mut row.requirement;
        if requirement
            .identity_group
            .is_some_and(|group| idle.contains(&group))
        {
            requirement.identity_group = None;
        }
    }
}

/// `requirement` without its stack label: what a copy asks, which two
/// member stacks compare.
const fn unlabelled(requirement: Requirement) -> Requirement {
    Requirement {
        identity_group: None,
        ..requirement
    }
}

/// Whether two stacks' copies ask the same, in any order.
fn same_copies(one: &[Requirement], other: &[Requirement]) -> bool {
    let mut left: Vec<Requirement> = other.to_vec();
    one.len() == other.len()
        && one.iter().all(|copy| {
            left.iter()
                .position(|candidate| candidate == copy)
                .map(|found| left.swap_remove(found))
                .is_some()
        })
}

/// The member stack labelled `label` on the cluster `group`, as its copies
/// (row indices) — when the label is one: carried by members of that
/// cluster and otherwise only by lone bare copies of the members' kind,
/// outside any combined level. A hand-written label spanning more is left
/// as written, and the problem list says what is wrong with it.
fn member_stack_copies(rows: &[Row], label: u8, group: u8) -> Option<Vec<usize>> {
    let mut kind = None;
    let mut copies = Vec::new();
    for (index, row) in rows.iter().enumerate() {
        let requirement = &row.requirement;
        if requirement.identity_group != Some(label) {
            continue;
        }
        if *kind.get_or_insert(requirement.kind) != requirement.kind
            || requirement.level_sum.is_some()
        {
            return None;
        }
        match requirement.alternative_group {
            Some(other) if other == group => {}
            None if requirement.is_bare() => copies.push(index),
            _ => return None,
        }
    }
    Some(copies)
}

/// Gives members of one cluster whose stacks ask for the same copies one
/// label, in first-appearance order: the first member's label and copies
/// stay, and the other stacks' copies go. `{Frost ×2 | Disintegration ×2}`
/// then reads, to the engine as on the board, "two of whichever matched".
fn merge_member_stacks(rows: &mut Vec<Row>) {
    let mut merged: BTreeMap<u8, u8> = BTreeMap::new();
    let mut doomed: BTreeSet<usize> = BTreeSet::new();
    let mut classes: BTreeMap<u8, Vec<(u8, Vec<Requirement>)>> = BTreeMap::new();
    let mut seen: BTreeSet<u8> = BTreeSet::new();
    for row in rows.iter() {
        let (Some(group), Some(label)) = (
            row.requirement.alternative_group,
            row.requirement.identity_group,
        ) else {
            continue;
        };
        if !seen.insert(label) {
            continue;
        }
        let Some(copies) = member_stack_copies(rows, label, group) else {
            continue;
        };
        if copies.is_empty() {
            continue;
        }
        let asks: Vec<Requirement> = copies
            .iter()
            .map(|&index| unlabelled(rows[index].requirement))
            .collect();
        let alike = classes.entry(group).or_default();
        if let Some((survivor, _)) = alike.iter().find(|(_, other)| same_copies(other, &asks)) {
            merged.insert(label, *survivor);
            doomed.extend(copies);
        } else {
            alike.push((label, asks));
        }
    }
    if merged.is_empty() {
        return;
    }
    for row in rows.iter_mut() {
        if let Some(label) = row.requirement.identity_group
            && let Some(&survivor) = merged.get(&label)
        {
            row.requirement.identity_group = Some(survivor);
        }
    }
    let mut index = 0;
    rows.retain(|_| {
        index += 1;
        !doomed.contains(&(index - 1))
    });
}

/// `rows` [`normalize`]d, for an edit that takes a row out of its entry or
/// reshapes a member's stack.
///
/// A list from elsewhere — a saved query, a preset, a results file — may
/// encode a stack another way than the board writes it: a named stack as
/// bare copies under a stack label, two members' alike stacks under two
/// labels. The board folds them as the canonical shape, but clearing the
/// label of the row that carries it would leave the copies to dissolve into
/// wildcards. Normalizing keeps every row's key, and every visible row, so
/// the caller finds the rows it read on the list as written — where
/// [`drop_action`] and [`join_candidates`] read them too — by key; only the
/// copies of a merged stack go.
fn canonical(rows: &[Row]) -> Vec<Row> {
    let mut rows = rows.to_vec();
    normalize(&mut rows);
    rows
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
            Self::BlanketTotal => "blanket_total",
            Self::NoFreeGroup => "no_free_group",
        }
    }
}

impl fmt::Display for Refusal {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
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
    /// Makes one item of `source` an either/or alternative of `target` (any
    /// member of a chip or cluster). A chip with copies stays where it is,
    /// with its constraints and one item fewer, and a bare copy of it joins
    /// ([`lift`]); a chip without copies joins itself. A stacked lone
    /// target keeps its stack as a member of the new cluster, and a target
    /// cluster's members keep theirs.
    Join { source: u64, target: u64 },
    /// Takes one item of a cluster member out on its own: a bare copy of
    /// it, while the member stays in the cluster one item fewer — or the
    /// member itself when it has no copies ([`lift`]).
    Detach { key: u64 },
    /// Deletes a chip with its whole stack: a cluster member with its own
    /// copies, or a lone chip's whole entry.
    Remove { key: u64 },
    /// Deletes one item of a chip's stack — a copy, or the chip itself when
    /// it has none — as a drag onto the remove target does.
    RemoveOne { key: u64 },
    /// Deletes the whole entry holding `key`: members and hidden copies.
    RemoveItem { key: u64 },
    /// Sets how many items the chip asks for, clamped to
    /// `1..=`[`STACK_MAX`].
    SetCount { key: u64, count: u8 },
    /// Sets or clears the stack's combined level, clamped to
    /// `1..=`[`level_capacity`]. Only a lone chip counts levels.
    SetTotal { key: u64, total: Option<u8> },
    /// Turns counting levels on (at [`default_total`]) or off.
    ToggleLevels { key: u64 },
    /// Sets or clears the floor limit of the chip's hidden copies; an
    /// empty boss floor snaps to the floor below it.
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
            Self::RemoveOne { key: k } => Self::RemoveOne { key: key(k) },
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
        Edit::Join { source, target } => join(rows, source, target, hint, held),
        Edit::Detach { key } => detach(rows, key, hint, held),
        Edit::Remove { key } => remove(rows, key, false),
        Edit::RemoveOne { key } => remove_one(rows, key, hint, held),
        Edit::RemoveItem { key } => remove(rows, key, true),
        Edit::SetCount { key, count } => {
            let board = Board::new(rows);
            let Some(stack) = board.visible(rows, key) else {
                return Outcome::unchanged();
            };
            let wanted = usize::from(count.clamp(1, STACK_MAX)) - 1;
            if wanted == stack.copies.len() {
                return Outcome::unchanged();
            }
            let step = if stack.in_cluster {
                if wanted > stack.copies.len() && !can_grow(rows, stack) {
                    return Outcome::unchanged();
                }
                restack(rows, key, hint, held, |mut copies, member, floor| {
                    copies.truncate(wanted);
                    copies.resize(wanted, copy_of(member, floor));
                    copies
                })
            } else {
                set_count(rows, stack, count, &mut Keys::minting(hint), None, held)
            };
            Outcome::from_step(step, Some(key))
        }
        Edit::SetTotal { key, total } => {
            let board = Board::new(rows);
            let Some(stack) = board.visible(rows, key) else {
                return Outcome::unchanged();
            };
            Outcome::from_step(set_total(rows, stack, total, None, held), Some(key))
        }
        Edit::ToggleLevels { key } => {
            let board = Board::new(rows);
            let Some(stack) = board.visible(rows, key) else {
                return Outcome::unchanged();
            };
            let total = match stack.total {
                Some(_) => None,
                None => Some(default_total(rows, stack)),
            };
            Outcome::from_step(set_total(rows, stack, total, None, held), Some(key))
        }
        Edit::SetCopyDepth { key, max_depth } => {
            let board = Board::new(rows);
            let Some(stack) = board.visible(rows, key) else {
                return Outcome::unchanged();
            };
            let step = if stack.in_cluster {
                let depth = copy_floor(max_depth);
                if stack
                    .copies
                    .iter()
                    .all(|&copy| rows[copy].requirement.max_depth == depth)
                {
                    return Outcome::unchanged();
                }
                restack(rows, key, hint, held, |copies, _, _| {
                    copies
                        .into_iter()
                        .map(|copy| Requirement {
                            max_depth: depth,
                            ..copy
                        })
                        .collect()
                })
            } else {
                set_copy_depth(rows, stack, max_depth)
            };
            Outcome::from_step(step, Some(key))
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

/// A copy floor as the controls send it, clamped into the dungeon and off
/// the empty boss floors.
fn copy_floor(max_depth: Option<u8>) -> Option<u8> {
    max_depth.map(|depth| normalize_floor_limit(depth.clamp(1, MAX_SEARCH_DEPTH)))
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

/// The index just past an entry's last row, member or copy: where the rows
/// it grows by go.
fn entry_end(item: &BoardItem) -> usize {
    item.members
        .iter()
        .chain(&item.extras)
        .max()
        .copied()
        .unwrap_or(0)
        + 1
}

/// The copy a stack of `chip` grows by, before a label ties it to its chip:
/// the chip's family and nothing else, with a floor limit of its own.
fn copy_of(chip: &Requirement, max_depth: Option<u8>) -> Requirement {
    Requirement {
        max_depth,
        ..Requirement::any(chip.kind)
    }
}

/// Sets how many items the lone chip `stack` asks for. Shrinking deletes
/// the last copies; growing adds copies after the chip's last row, of the
/// shape the stack already has: a copy of the level-sum member, a plain
/// repeat of a named chip, or a bare copy under the chip's identity label
/// (minting one, preferably `preferred_label`, when the chip has none —
/// never a `held` one). New copies keep to the floor limit the existing
/// copies carry.
fn set_count(
    rows: &[Row],
    stack: &ChipStack,
    count: u8,
    keys: &mut Keys,
    preferred_label: Option<u8>,
    held: &HeldLabels,
) -> Step {
    let wanted = usize::from(count.clamp(1, STACK_MAX)) - 1;
    if wanted == stack.copies.len() {
        return Step::Unchanged;
    }
    if wanted < stack.copies.len() {
        let mut next = without(rows, &stack.copies[wanted..]);
        normalize(&mut next);
        return Step::Rows(next);
    }
    if !can_grow(rows, stack) {
        return Step::Unchanged;
    }
    let anchor = rows[stack.index].requirement;
    let inherited = stack
        .copies
        .first()
        .and_then(|&index| rows[index].requirement.max_depth);
    let mut next = rows.to_vec();
    // A lone alternative label (a hand-written cluster of one) stays on
    // the anchor alone: copies of it would make it a cluster.
    let template = if stack.total.is_some() && anchor.level_sum.is_some() {
        Requirement {
            alternative_group: None,
            max_depth: inherited,
            ..anchor
        }
    } else if anchor.item.is_some() {
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
        next[stack.index].requirement.identity_group = Some(label);
        bare_copy(&anchor, label, inherited)
    };
    let insert_at = stack
        .copies
        .iter()
        .chain([&stack.index])
        .max()
        .copied()
        .unwrap_or(0)
        + 1;
    for offset in 0..wanted - stack.copies.len() {
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

/// Sets or clears the lone chip's combined level.
///
/// With a total the whole stack becomes identical optional members sharing
/// a level-sum label ("up to N items reaching T levels"), each open to any
/// upgrade; without one it returns to an anchor with plain repeats
/// ("exactly N of the item"). Either way every row keeps its own floor
/// limit, a placement rather than an item property: the anchor its own, the
/// copies theirs. Only a lone named ring stack of two or more counts
/// levels — levels add up across rings alone, a single member would
/// dissolve and silently drop its upgrade, and a combined level cannot sit
/// in a cluster — but clearing works on any stale sum a hand-written
/// document left. A new combined-level label is `preferred` when free, and
/// never a `held` one.
fn set_total(
    rows: &[Row],
    stack: &ChipStack,
    total: Option<u8>,
    preferred: Option<u8>,
    held: &HeldLabels,
) -> Step {
    let anchor = rows[stack.index].requirement;
    if anchor.blanket {
        return Step::Refused(Refusal::BlanketTotal);
    }
    if stack.in_cluster || anchor.item.is_none() {
        return Step::Unchanged;
    }
    let mut next = rows.to_vec();
    let Some(total) = total else {
        if stack.total.is_none() {
            return Step::Unchanged;
        }
        next[stack.index].requirement.level_sum = None;
        for &index in &stack.copies {
            next[index].requirement = plain_copy(&anchor, rows[index].requirement.max_depth);
        }
        normalize(&mut next);
        return Step::Rows(next);
    };
    if anchor.kind != ItemKind::Ring || stack.count() < 2 {
        return Step::Unchanged;
    }
    let total = total.clamp(1, level_capacity(rows, stack).max(1));
    if stack.total == Some(total) {
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
    // Combined-level members never sit in a cluster; the chip is lone, so
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
    for &index in std::iter::once(&stack.index).chain(&stack.copies) {
        next[index].requirement = Requirement {
            max_depth: rows[index].requirement.max_depth,
            ..member
        };
    }
    normalize(&mut next);
    Step::Rows(next)
}

/// Sets or clears the floor limit of the lone chip's hidden copies. The
/// anchor keeps its own limit: "the +3 one before floor 4, the rest
/// wherever" and "…the rest before floor 10" are both sayable. A
/// combined-level stack's members keep the limits they had when it started
/// counting; its copy floor is not edited while it counts.
fn set_copy_depth(rows: &[Row], stack: &ChipStack, max_depth: Option<u8>) -> Step {
    if stack.total.is_some() || stack.copies.is_empty() {
        return Step::Unchanged;
    }
    let max_depth = copy_floor(max_depth);
    if stack
        .copies
        .iter()
        .all(|&index| rows[index].requirement.max_depth == max_depth)
    {
        return Step::Unchanged;
    }
    let mut next = rows.to_vec();
    for &index in &stack.copies {
        next[index].requirement.max_depth = max_depth;
    }
    normalize(&mut next);
    Step::Rows(next)
}

/// Whether the member at `index` of `item` shares its stack label with
/// another member: their stacks are alike, and share their copies.
fn shares_label(rows: &[Row], item: &BoardItem, index: usize) -> bool {
    rows[index].requirement.identity_group.is_some_and(|label| {
        item.members
            .iter()
            .any(|&other| other != index && rows[other].requirement.identity_group == Some(label))
    })
}

/// The label of another member of `item` whose stack asks for exactly
/// `copies`, which the member at `index` can share rather than take a label
/// of its own.
fn alike_label(rows: &[Row], item: &BoardItem, index: usize, copies: &[Requirement]) -> Option<u8> {
    let group = rows[index].requirement.alternative_group?;
    item.stacks.iter().find_map(|stack| {
        let label = rows[stack.index].requirement.identity_group?;
        let theirs: Vec<Requirement> = member_stack_copies(rows, label, group)?
            .into_iter()
            .map(|copy| unlabelled(rows[copy].requirement))
            .collect();
        (stack.index != index && same_copies(&theirs, copies)).then_some(label)
    })
}

/// Gives the cluster member at `index` of `item` the stack `copies` (bare
/// copies of its kind, unlabelled, in order), as the canonical encoding
/// writes it: no label without copies; another member's label when its
/// stack is alike; its own label otherwise — keeping its own copies' rows
/// and keys in place, the last ones deleted or new ones added after the
/// entry, or, when the member shared a label, a fresh one (`preferred` when
/// free, never a `held` one) with new copies. Refuses when no label is free.
fn restack_member(
    rows: &mut Vec<Row>,
    item: &BoardItem,
    index: usize,
    copies: &[Requirement],
    keys: &mut Keys,
    (preferred, held): (Option<u8>, &HeldLabels),
) -> Result<(), Refusal> {
    let stack = item.stack(index).expect("a member of the entry");
    let current: Vec<Requirement> = stack
        .copies
        .iter()
        .map(|&copy| unlabelled(rows[copy].requirement))
        .collect();
    let label = rows[index].requirement.identity_group;
    if same_copies(&current, copies) && (copies.is_empty() || label.is_some()) {
        return Ok(());
    }
    let own: &[usize] = if shares_label(rows, item, index) {
        &[]
    } else {
        &stack.copies
    };
    let end = entry_end(item);
    let (label, doomed, added): (Option<u8>, &[usize], &[Requirement]) = if copies.is_empty() {
        (None, own, &[])
    } else if let Some(alike) = alike_label(rows, item, index, copies) {
        (Some(alike), own, &[])
    } else if !own.is_empty() {
        let kept = own.len().min(copies.len());
        for (&copy, &ask) in own.iter().zip(copies) {
            rows[copy].requirement = Requirement {
                identity_group: label,
                ..ask
            };
        }
        (label, &own[kept..], &copies[kept..])
    } else {
        let used = taken(
            rows.iter().map(|row| row.requirement.identity_group),
            &held.identity,
        );
        let Some(fresh) = label_for(&used, preferred, MAX_IDENTITY_GROUP) else {
            return Err(Refusal::NoFreeGroup);
        };
        (Some(fresh), &[], copies)
    };
    rows[index].requirement.identity_group = label;
    for (offset, &ask) in added.iter().enumerate() {
        let key = keys.take(rows);
        rows.insert(
            end + offset,
            Row {
                key,
                requirement: Requirement {
                    identity_group: label,
                    ..ask
                },
            },
        );
    }
    // The doomed copies all sit before `end`, where nothing moved.
    let mut position = 0;
    rows.retain(|_| {
        position += 1;
        !doomed.contains(&(position - 1))
    });
    Ok(())
}

/// Reshapes the stack of the cluster member `key`: `reshape` maps its
/// copies (unlabelled, in list order) — given the member and the copies'
/// floor limit — to the ones it should have, which [`restack_member`]
/// encodes on the list's canonical form.
fn restack(
    rows: &[Row],
    key: u64,
    hint: Option<u64>,
    held: &HeldLabels,
    reshape: impl FnOnce(Vec<Requirement>, &Requirement, Option<u8>) -> Vec<Requirement>,
) -> Step {
    let mut next = canonical(rows);
    let board = Board::new(&next);
    let Some((index, item)) = board.member(&next, key) else {
        return Step::Unchanged;
    };
    let stack = item.stack(index).expect("a member of the entry");
    let current: Vec<Requirement> = stack
        .copies
        .iter()
        .map(|&copy| unlabelled(next[copy].requirement))
        .collect();
    let floor = current.first().and_then(|copy| copy.max_depth);
    let member = next[index].requirement;
    let copies = reshape(current, &member, floor);
    if let Err(refusal) = restack_member(
        &mut next,
        item,
        index,
        &copies,
        &mut Keys::minting(hint),
        (None, held),
    ) {
        return Step::Refused(refusal);
    }
    normalize(&mut next);
    Step::Rows(next)
}

/// The item a drag of a chip with copies carries: one bare copy of its
/// stack — the chip's item with its melee/thrown narrowing, or its kind for
/// a wildcard stack — with the floor limit of `copy`, the copy
/// [`Edit::RemoveOne`] drops, and nothing else: no constraint, no label, no
/// combined level. It stays in the chip's section.
fn carried_copy(chip: &Requirement, copy: &Requirement) -> Requirement {
    Requirement {
        blanket: chip.blanket,
        ..plain_copy(chip, copy.max_depth)
    }
}

/// `requirement` without its group labels: a chip moving whole.
const fn unlabelled_chip(requirement: Requirement) -> Requirement {
    Requirement {
        alternative_group: None,
        identity_group: None,
        level_sum: None,
        ..requirement
    }
}

/// The requirement of the item a drag of the visible row `key` carries, as
/// [`Edit::Join`] and [`Edit::Detach`] move it ([`lift`]): a bare copy of
/// the chip, or `None` when the chip has no copies and moves itself.
///
/// The edits read the list in its canonical encoding, and so does this. A
/// chip only a list never normalized gives copies may have none there, or
/// fold into another chip's stack; the row itself then moves, which this
/// names too.
pub(crate) fn lifted(rows: &[Row], key: u64) -> Option<Requirement> {
    let written = Board::new(rows);
    let index = index_of(rows, key)?;
    if written.stack(index)?.copies.is_empty() {
        return None;
    }
    let next = canonical(rows);
    let board = Board::new(&next);
    let Some((index, item)) = board.member(&next, key) else {
        return Some(unlabelled_chip(rows[index].requirement));
    };
    let chip = &next[index].requirement;
    Some(
        match item.stack(index).and_then(|stack| stack.copies.last()) {
            Some(&last) => carried_copy(chip, &next[last].requirement),
            None => unlabelled_chip(*chip),
        },
    )
}

/// Takes one item off the chip at `index` of `item`, as [`Edit::RemoveOne`]
/// does: its last copy. A member's stack is restacked one fewer
/// ([`restack_member`]); a lone stack's last copy goes, its combined level
/// capped at what the rest can reach. A chip without copies keeps its
/// rows. Refuses when the rest of a member's stack needs a label and none
/// is free.
fn shed(
    rows: &mut Vec<Row>,
    item: &BoardItem,
    index: usize,
    keys: &mut Keys,
    held: &HeldLabels,
) -> Result<(), Refusal> {
    let stack = item.stack(index).expect("a member of the entry");
    let Some((&last, rest)) = stack.copies.split_last() else {
        return Ok(());
    };
    if stack.in_cluster {
        let copies: Vec<Requirement> = rest
            .iter()
            .map(|&copy| unlabelled(rows[copy].requirement))
            .collect();
        return restack_member(rows, item, index, &copies, keys, (None, held));
    }
    let sum = rows[index].requirement.level_sum;
    *rows = without(rows, &[last]);
    if let Some(sum) = sum {
        cap_level_sum(rows, sum.group);
    }
    Ok(())
}

/// Lifts the item a drag of the visible row `key` carries off the
/// canonical `rows`, returning them with that item's key.
///
/// A chip with copies stays where it is, with its constraints and one item
/// fewer ([`shed`], as [`Edit::RemoveOne`] leaves it), and the item is a
/// bare copy of it ([`carried_copy`]), a row right after the chip: the key
/// of the copy the chip shed, or a new one when that copy stays with alike
/// members that share it. A chip without copies is the item: it leaves its
/// cluster, stack and combined level, whose rest is capped at what it can
/// still reach. Refuses when the rest of a member's stack needs a label
/// and none is free.
fn lift(
    mut rows: Vec<Row>,
    key: u64,
    keys: &mut Keys,
    held: &HeldLabels,
) -> Result<(Vec<Row>, u64), Refusal> {
    let board = Board::new(&rows);
    let found = board.member(&rows, key);
    let copy = found.and_then(|(index, item)| {
        let last = *item.stack(index)?.copies.last()?;
        Some((index, item, last))
    });
    let Some((index, item, last)) = copy else {
        if let Some(index) = index_of(&rows, key) {
            let requirement = rows[index].requirement;
            rows[index].requirement = unlabelled_chip(requirement);
            if let Some(sum) = requirement.level_sum {
                cap_level_sum(&mut rows, sum.group);
            }
        }
        return Ok((rows, key));
    };
    let carried = carried_copy(&rows[index].requirement, &rows[last].requirement);
    let shed_key = rows[last].key;
    shed(&mut rows, item, index, keys, held)?;
    let carried_key = if rows.iter().any(|row| row.key == shed_key) {
        keys.take(&rows)
    } else {
        shed_key
    };
    let at = index_of(&rows, key).expect("the chip stays") + 1;
    rows.insert(
        at,
        Row {
            key: carried_key,
            requirement: carried,
        },
    );
    Ok((rows, carried_key))
}

/// Takes one item of a cluster member out on its own ([`lift`]): a bare
/// copy, right after the member, or the member itself, in its place.
fn detach(rows: &[Row], key: u64, hint: Option<u64>, held: &HeldLabels) -> Outcome {
    let board = Board::new(rows);
    let Some((_, item)) = board.member(rows, key) else {
        return Outcome::unchanged();
    };
    if item.cluster.is_none() {
        return Outcome::unchanged();
    }
    let next = canonical(rows);
    if Board::new(&next).member(&next, key).is_none() {
        return Outcome::unchanged();
    }
    match lift(next, key, &mut Keys::minting(hint), held) {
        Ok((next, moved)) => Outcome::rows(next, Some(moved)),
        Err(refusal) => Outcome::refused(refusal),
    }
}

/// Deletes a chip with its whole stack — a cluster member with its own
/// copies (a stack it shares stays with the others), a lone chip's whole
/// entry — or, with `whole`, the whole entry holding it.
fn remove(rows: &[Row], key: u64, whole: bool) -> Outcome {
    let written = Board::new(rows);
    let Some((index, item)) = written.member(rows, key) else {
        return Outcome::unchanged();
    };
    let next = canonical(rows);
    let board = Board::new(&next);
    // A hand-written cluster of one tied to a lone chip's stack folds into
    // that stack once normalized; its rows go as the list showed them.
    let doomed: Vec<u64> = match board.member(&next, key) {
        Some((index, item)) => doomed_rows(&next, item, index, whole),
        None => doomed_rows(rows, item, index, whole),
    };
    let next = next
        .into_iter()
        .filter(|row| !doomed.contains(&row.key))
        .collect();
    Outcome::rows(next, None)
}

/// The keys [`remove`] deletes for the visible row at `index` of `item`.
fn doomed_rows(rows: &[Row], item: &BoardItem, index: usize, whole: bool) -> Vec<u64> {
    let indices: Vec<usize> = if whole || item.cluster.is_none() {
        item.members.iter().chain(&item.extras).copied().collect()
    } else if shares_label(rows, item, index) {
        vec![index]
    } else {
        let stack = item.stack(index).expect("a member of the entry");
        std::iter::once(index).chain(stack.copies.clone()).collect()
    };
    indices.into_iter().map(|index| rows[index].key).collect()
}

/// Deletes one item of the chip `key`: its last copy ([`shed`]) — a
/// member's stack restacked on the list's canonical encoding, a lone
/// stack's combined level keeping what the rest can reach — or the chip
/// itself when it has no copies.
fn remove_one(rows: &[Row], key: u64, hint: Option<u64>, held: &HeldLabels) -> Outcome {
    let board = Board::new(rows);
    let Some(stack) = board.visible(rows, key) else {
        return Outcome::unchanged();
    };
    if stack.copies.is_empty() {
        return remove(rows, key, false);
    }
    let mut next = if stack.in_cluster {
        canonical(rows)
    } else {
        rows.to_vec()
    };
    let board = Board::new(&next);
    let Some((index, item)) = board.member(&next, key) else {
        return Outcome::unchanged();
    };
    match shed(&mut next, item, index, &mut Keys::minting(hint), held) {
        Ok(()) => Outcome::rows(next, Some(key)),
        Err(refusal) => Outcome::refused(refusal),
    }
}

/// Precomputed answers to "may this visible row join that one?", shared by
/// [`join`], [`drop_action`] and [`join_candidates`] so the three can never
/// disagree.
struct JoinRules {
    board: Board,
    /// The label a new cluster would take.
    next_group: Option<u8>,
    /// The labels a join may not take.
    held: HeldLabels,
    /// How many stack labels are free.
    free_labels: usize,
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

impl JoinRules {
    /// The rules for `rows` beside rows holding the `held` labels, which a
    /// join never takes.
    fn new(rows: &[Row], board: Board, held: &HeldLabels) -> Self {
        let used = taken(
            rows.iter().map(|row| row.requirement.identity_group),
            &held.identity,
        );
        Self {
            board,
            next_group: next_alternative_group(rows, &held.alternative),
            held: held.clone(),
            free_labels: (1..=MAX_IDENTITY_GROUP)
                .filter(|label| !used.contains(label))
                .count(),
        }
    }

    /// Whether `source` may join `target`, and into which group. Every
    /// copy keeps its own chip's kind, so a join across categories is as
    /// good as any (#190 refused one where either entry was a stack, when a
    /// cluster's stack had to name one kind for all its members).
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
        JoinCheck::Join(group)
    }

    /// [`JoinRules::check`], and — where the join needs more stack labels
    /// than are free — whether it is refused for want of one: a stacked
    /// lone target without a stack label (plain repeats, a combined level)
    /// takes one as a member, and so may the rest of a member's stack it
    /// shared with another. Those joins are run to see.
    fn verdict(&self, rows: &[Row], source: usize, target: usize) -> JoinCheck {
        let check = self.check(rows, source, target);
        let JoinCheck::Join(group) = check else {
            return check;
        };
        let (Some(from), Some(onto)) = (self.board.stack(source), self.board.stack(target)) else {
            return check;
        };
        let target_needs = !onto.in_cluster
            && onto.count() > 1
            && rows[target].requirement.identity_group.is_none();
        let source_needs = from.in_cluster
            && from.count() > 2
            && self.board.member_of[source]
                .is_some_and(|position| shares_label(rows, &self.board.items[position], source));
        if usize::from(target_needs) + usize::from(source_needs) <= self.free_labels {
            return check;
        }
        let (step, _) = joined(
            rows,
            (rows[source].key, rows[target].key, group),
            &mut Keys::minting(None),
            &self.held,
        );
        match step {
            Step::Refused(refusal) => JoinCheck::Refuse(refusal),
            Step::Unchanged | Step::Rows(_) => check,
        }
    }
}

/// One item of the chip at `source` becomes an either/or alternative of the
/// chip at `target`, and moves after the cluster's last member.
///
/// A join moves one item ([`lift`]). A source with copies stays where it
/// is, with its constraints and one item fewer — a lone stack's last copy
/// gone, a member's stack restacked, as [`Edit::RemoveOne`] leaves it — and
/// a bare copy of it joins: the chip's item, or its kind for a wildcard
/// stack, with the shed copy's floor limit and nothing else. A source
/// without copies joins itself. A stacked lone target keeps its stack as a
/// member of the new cluster; a target cluster's members keep theirs, and
/// the item joins as a ×1 member.
///
/// A combined level cannot travel into a cluster. A source counting levels
/// keeps counting one ring fewer, its total capped at what the rest can
/// still reach, or drops it when only one is left — the sheet's rule for a
/// stack stepped down; a stacked target drops it and keeps its count, its
/// rings becoming bare copies under a stack label.
///
/// Every copy keeps its own chip's kind, so categories may mix freely.
fn join(
    rows: &[Row],
    source_key: u64,
    target_key: u64,
    hint: Option<u64>,
    held: &HeldLabels,
) -> Outcome {
    let rules = JoinRules::new(rows, Board::new(rows), held);
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
    let (step, moved) = joined(
        rows,
        (source_key, target_key, group),
        &mut Keys::minting(hint),
        held,
    );
    Outcome::from_step(step, Some(moved))
}

/// The rows of the join of `source_key` onto `target_key` into `group`
/// (see [`join`]), which [`JoinRules::check`] allowed, and the key of the
/// item that joined.
fn joined(
    rows: &[Row],
    (source_key, target_key, group): (u64, u64, u8),
    keys: &mut Keys,
    held: &HeldLabels,
) -> (Step, u64) {
    let mut next = canonical(rows);
    let board = Board::new(&next);
    let shown = (
        board.member(&next, source_key),
        board.member(&next, target_key),
    );
    let folded = !matches!(shown, (Some(_), Some(_)));
    if let (Some(_), Some((target, target_item))) = shown {
        let target_stack = target_item.stack(target).expect("a member of the entry");
        if !target_stack.in_cluster {
            let anchor = next[target].requirement;
            if target_stack.count() > 1
                && (anchor.identity_group.is_none() || anchor.level_sum.is_some())
            {
                // Plain repeats and a combined level's rings become bare
                // copies under a stack label of the target's own: the one a
                // combined level's stack already carries, or a free one.
                let used = taken(
                    next.iter().map(|row| row.requirement.identity_group),
                    &held.identity,
                );
                let Some(label) = anchor
                    .identity_group
                    .or_else(|| free_group(&used, MAX_IDENTITY_GROUP))
                else {
                    return (Step::Refused(Refusal::NoFreeGroup), source_key);
                };
                let member = Requirement {
                    identity_group: Some(label),
                    level_sum: None,
                    ..anchor
                };
                next[target].requirement = member;
                for &copy in &target_stack.copies {
                    next[copy].requirement =
                        bare_copy(&member, label, next[copy].requirement.max_depth);
                }
            }
            next[target].requirement.alternative_group = Some(group);
        }
    }
    // The target's rows changed in place only, so the source is found again
    // on them, and lifts the item [`lifted`] names.
    let (mut next, moved) = match lift(next, source_key, keys, held) {
        Ok(lifted) => lifted,
        Err(refusal) => return (Step::Refused(refusal), source_key),
    };
    if folded && let Some(target) = index_of(&next, target_key) {
        // A hand-written cluster of one tied to a lone chip's stack folds
        // into that stack once normalized. The two rows still make one slot,
        // as the list showed them: the target's row joins as it is, and a
        // source folded away joins itself.
        next[target].requirement.alternative_group = Some(group);
    }
    let source = index_of(&next, moved).expect("the lifted item is a row");
    next[source].requirement.alternative_group = Some(group);
    let mut next = move_after(next, source, |row| row.alternative_group == Some(group));
    normalize(&mut next);
    (Step::Rows(next), moved)
}

/// Caps the total of the combined level labelled `group` at what its
/// members can still reach, once one of them has left it. A group left
/// with one member dissolves when the rows are normalized.
fn cap_level_sum(rows: &mut [Row], group: u8) {
    let members: Vec<usize> = (0..rows.len())
        .filter(|&index| {
            rows[index]
                .requirement
                .level_sum
                .is_some_and(|sum| sum.group == group)
        })
        .collect();
    let [anchor, ref copies @ ..] = members[..] else {
        return;
    };
    let stack = ChipStack {
        index: anchor,
        in_cluster: false,
        copies: copies.to_vec(),
        total: rows[anchor]
            .requirement
            .level_sum
            .map(|sum| sum.minimum_total),
    };
    let most = level_capacity(rows, &stack).max(1);
    for &index in &members {
        if let Some(sum) = &mut rows[index].requirement.level_sum {
            sum.minimum_total = sum.minimum_total.min(most);
        }
    }
}

/// Stores the sheet's requirement with its stack's shape (web `applyEdit`).
///
/// An edited chip's stack comes down and is rebuilt from `count`, `total`
/// and `copy_depth`: the edit may have changed the very kind the copies
/// copy. The rebuilt copies reuse the removed copies' keys in order, and the
/// stack's old identity or combined-level label is preferred, so saving an
/// unchanged chip gives back identical rows. A cluster member keeps its
/// place in its cluster and gets the stack the sheet shows as a member
/// stack ([`restack_member`]), its own copies kept in place — none when it
/// became a trinket, artifact or blanket, which never stack. The saved
/// requirement's own group labels are ignored: relationships come from the
/// row being edited, and stacks are this edit's to write.
///
/// When the saved row turns out to be a plain repeat of an earlier chip, it
/// folds into that chip and the shape is not applied; `focus` names the
/// chip it landed in either way.
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
        // A hidden copy has no sheet of its own.
        let Some(stack) = board.stack(index) else {
            return Outcome::unchanged();
        };
        let current = rows[index].requirement;
        let saved = rows[index].key;
        if saves_nothing(
            rows,
            stack,
            &current,
            &requirement,
            (count, total, copy_depth),
        ) {
            return Outcome::from_step(Step::Rows(rows.to_vec()), Some(saved));
        }
        if stack.in_cluster {
            return save_member(rows, saved, requirement, (count, copy_depth), hint, held);
        }
        preferred_identity = current.identity_group;
        preferred_sum = current.level_sum.map(|sum| sum.group);
        keys.reuse = stack.copies.iter().map(|&copy| rows[copy].key).collect();
        let mut next = rows.to_vec();
        next[index].requirement = Requirement {
            alternative_group: current.alternative_group,
            identity_group: current
                .identity_group
                .filter(|_| takes_stack_label(&requirement)),
            ..requirement
        };
        (without(&next, &stack.copies), saved)
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
    Outcome::from_step(Step::Rows(next), Some(saved))
}

/// The save of the cluster member `key`: the requirement in its place, with
/// the member stack of `count` items whose copies lie within `copy_depth`.
fn save_member(
    rows: &[Row],
    key: u64,
    requirement: Requirement,
    (count, copy_depth): (u8, Option<u8>),
    hint: Option<u64>,
    held: &HeldLabels,
) -> Outcome {
    let mut next = canonical(rows);
    let board = Board::new(&next);
    let Some((index, item)) = board.member(&next, key) else {
        return Outcome::unchanged();
    };
    let current = next[index].requirement;
    let saved = Requirement {
        alternative_group: current.alternative_group,
        identity_group: current.identity_group,
        ..requirement
    };
    next[index].requirement = saved;
    let wanted = if takes_stack_label(&saved) {
        usize::from(count.clamp(1, STACK_MAX)) - 1
    } else {
        0
    };
    let copies = vec![copy_of(&saved, copy_floor(copy_depth)); wanted];
    if let Err(refusal) = restack_member(
        &mut next,
        item,
        index,
        &copies,
        &mut Keys::minting(hint),
        (current.identity_group, held),
    ) {
        return Outcome::refused(refusal);
    }
    Outcome::rows(next, Some(key))
}

/// Whether saving `requirement` (its labels already stripped) with the
/// sheet's `(count, total, copy_depth)` onto the visible row of `stack`
/// would store what is already there: the same requirement, the same
/// count, the same combined level, and the copy floor the sheet showed
/// ([`super::copy_depth`], the first copy's). Such a save is a no-op and
/// returns the rows verbatim.
///
/// Rebuilding would not always give the rows back: copies left behind a
/// chip that joined and left a cluster would come back right after it, and
/// copies with floors of their own — a plain repeat saved with its own
/// floor folds into the earlier chip — would all take the first one's. A
/// platform comparing whole lists (Android's refine plan) would see a
/// change the user never made.
fn saves_nothing(
    rows: &[Row],
    stack: &ChipStack,
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
    if usize::from(count.clamp(1, STACK_MAX)) != stack.count() {
        return false;
    }
    let counting = total.is_some() && can_count_levels(rows, stack);
    // A counting stack's sheet holds the copy floor too, behind its switch,
    // and saves it back.
    let same_copies =
        stack.copies.is_empty() || stack_copy_depth(rows, stack) == copy_floor(copy_depth);
    match stack.total {
        Some(current_total) => counting && total == Some(current_total) && same_copies,
        None if counting => false,
        None => same_copies,
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
    let stack_of = |rows: &[Row]| {
        let index = index_of(rows, saved)?;
        Board::new(rows).stack(index).cloned()
    };
    // A row that folded into an earlier chip, or sits in a cluster, leaves
    // the shape to that entry.
    let mut stack = stack_of(rows).filter(|stack| !stack.in_cluster)?;
    let run = |rows: &mut Vec<Row>, step: Step| match step {
        Step::Unchanged => None,
        Step::Rows(next) => {
            *rows = next;
            None
        }
        Step::Refused(refusal) => Some(refusal),
    };
    if stack.total.is_some() && total.is_none() {
        let step = set_total(rows, &stack, None, None, held);
        if let Some(refusal) = run(rows, step) {
            return Some(refusal);
        }
        stack = stack_of(rows)?;
    }
    let step = set_count(rows, &stack, count, keys, preferred_identity, held);
    if let Some(refusal) = run(rows, step) {
        return Some(refusal);
    }
    stack = stack_of(rows)?;
    // The copies take their floor before counting starts, which keeps it.
    let step = set_copy_depth(rows, &stack, copy_depth);
    if let Some(refusal) = run(rows, step) {
        return Some(refusal);
    }
    stack = stack_of(rows)?;
    if total.is_some() && can_count_levels(rows, &stack) {
        let step = set_total(rows, &stack, total, preferred_sum, held);
        return run(rows, step);
    }
    None
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

/// What releasing a dragged chip does. A drag moves one item, so platforms
/// send the matching [`Edit`]: `Join` → [`Edit::Join`], `Detach` →
/// [`Edit::Detach`], `RemoveOne` → [`Edit::RemoveOne`].
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DropAction {
    Join {
        target: u64,
    },
    /// The drop is refused; hover feedback shows the refusal's message.
    Refuse(Refusal),
    Detach,
    RemoveOne,
    Nothing,
}

/// The policy for releasing the visible row `source` on `target`: onto a
/// row or a cluster it joins (or is refused, or does nothing for the same
/// row, its own cluster, or the other section); onto the empty board of its
/// own section a cluster member detaches — a lone chip stays put, so a
/// stack is never split off by a stray drop; onto the remove target one
/// item goes. A detach or removal that would need a stack label no one has
/// free is refused as the edit would be.
#[must_use]
pub fn drop_action(rows: &[Row], source: u64, target: DropTarget) -> DropAction {
    let board = Board::new(rows);
    let Some((source_index, source_item)) = board.member(rows, source) else {
        return DropAction::Nothing;
    };
    let tried = |edit: Edit, action: DropAction| {
        apply(rows, None, &[edit])
            .refused
            .map_or(action, DropAction::Refuse)
    };
    let target_index = match target {
        DropTarget::Remove => {
            return tried(Edit::RemoveOne { key: source }, DropAction::RemoveOne);
        }
        DropTarget::Board { blanket } => {
            return if source_item.cluster.is_some()
                && rows[source_index].requirement.blanket == blanket
            {
                tried(Edit::Detach { key: source }, DropAction::Detach)
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
    let rules = JoinRules::new(rows, board, &HeldLabels::default());
    match rules.verdict(rows, source_index, target_index) {
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
    join_candidates_holding(rows, items, &HeldLabels::default())
}

/// [`join_candidates`] beside rows holding the `held` labels, which a join
/// never takes: the answers agree with [`apply_holding`]'s.
pub(crate) fn join_candidates_holding(
    rows: &[Row],
    items: &[BoardItem],
    held: &HeldLabels,
) -> Vec<JoinCandidates> {
    let rules = JoinRules::new(rows, Board::from_items(rows.len(), items.to_vec()), held);
    let visible: Vec<usize> = (0..rows.len())
        .filter(|&index| rules.board.member_of[index].is_some())
        .collect();
    let mut candidates = vec![JoinCandidates::default(); rows.len()];
    for &source in &visible {
        for &target in &visible {
            match rules.verdict(rows, source, target) {
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
